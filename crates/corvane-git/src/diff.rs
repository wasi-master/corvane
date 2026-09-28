//! Working-directory diffs (GHD `lib/git/diff.ts` + `lib/diff-parser.ts`).

use std::path::Path;
use std::sync::Arc;

use corvane_models::{Diff, DiffHunk, DiffLine, DiffLineKind, WorkingDirectoryFileChange};

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// GHD `MaxDiffBufferSize` (70 MB) is the git buffer; the UI refuses to render
/// beyond `MaxReasonableDiffSize` (3 MB) and this many lines.
pub const MAX_DIFF_BYTES: usize = 3 * 1024 * 1024;
pub const MAX_DIFF_LINES: usize = 50_000;

/// `git diff` for one working-directory file, compared against HEAD (or an
/// empty file for new/untracked files), exactly like GHD.
pub fn working_directory_diff(
    git: Arc<GitBinary>,
    workdir: &Path,
    file: &WorkingDirectoryFileChange,
) -> Result<Diff> {
    if file.status.submodule {
        return Ok(Diff::Submodule);
    }
    let mut cmd = GitCommand::new(git)
        .args([
            "diff",
            "--no-ext-diff",
            "--patch-with-raw",
            "-z",
            "--no-color",
        ])
        .current_dir(workdir);
    if file.status.kind.is_new_or_untracked() {
        // `--no-index` exits 1 when files differ, which is the normal case.
        cmd = cmd
            .args(["--no-index", "--", "/dev/null"])
            .arg(&file.path)
            .allow_exit_code(1);
    } else {
        cmd = cmd.args(["HEAD", "--"]).arg(&file.path);
    }
    let out = cmd.run()?;
    Ok(parse_raw_diff(&out.stdout))
}

/// Parse `--patch-with-raw -z` output: skip the raw header block, then the
/// unified diff. Handles binary and oversize diffs.
pub fn parse_raw_diff(stdout: &[u8]) -> Diff {
    if stdout.len() > MAX_DIFF_BYTES {
        return Diff::TooLarge;
    }
    let text = String::from_utf8_lossy(stdout);
    // With -z the raw section is NUL separated; the patch starts at "diff --git".
    let patch_start = text.find("diff --git").unwrap_or(text.len());
    let patch = &text[patch_start..];
    if patch.trim().is_empty() {
        return Diff::Empty;
    }
    if patch.contains("\nBinary files ") || patch.starts_with("Binary files ") {
        return Diff::Binary;
    }
    parse_unified(patch)
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    // @@ -a[,b] +c[,d] @@ ...
    let rest = line.strip_prefix("@@ -")?;
    let (ranges, _) = rest.split_once(" @@")?;
    let (old, new) = ranges.split_once(" +")?;
    let parse_range = |r: &str| -> Option<(u32, u32)> {
        match r.split_once(',') {
            Some((s, n)) => Some((s.parse().ok()?, n.parse().ok()?)),
            None => Some((r.parse().ok()?, 1)),
        }
    };
    let (os, ol) = parse_range(old)?;
    let (ns, nl) = parse_range(new)?;
    Some((os, ol, ns, nl))
}

pub fn parse_unified(patch: &str) -> Diff {
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut current: Option<DiffHunk> = None;
    let mut old_no = 0u32;
    let mut new_no = 0u32;
    let mut total_lines = 0usize;
    let mut truncated = false;

    for line in patch.split_inclusive('\n') {
        let line = line.strip_suffix('\n').unwrap_or(line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some((os, ol, ns, nl)) = parse_hunk_header(line) {
            if let Some(h) = current.take() {
                hunks.push(h);
            }
            old_no = os;
            new_no = ns;
            current = Some(DiffHunk {
                unified_diff_start: hunks.iter().map(|h| h.lines.len() as u32).sum(),
                header: line.to_string(),
                old_start: os,
                old_lines: ol,
                new_start: ns,
                new_lines: nl,
                lines: vec![DiffLine {
                    kind: DiffLineKind::Hunk,
                    text: line.to_string(),
                    old_line: None,
                    new_line: None,
                    no_trailing_newline: false,
                }],
            });
            continue;
        }
        let Some(hunk) = current.as_mut() else {
            continue; // file header lines before the first hunk
        };
        if line.starts_with("\\ No newline at end of file") {
            if let Some(last) = hunk.lines.last_mut() {
                last.no_trailing_newline = true;
            }
            continue;
        }
        total_lines += 1;
        if total_lines > MAX_DIFF_LINES {
            truncated = true;
            break;
        }
        let (kind, text) = match line.chars().next() {
            Some('+') => (DiffLineKind::Add, &line[1..]),
            Some('-') => (DiffLineKind::Delete, &line[1..]),
            Some(' ') => (DiffLineKind::Context, &line[1..]),
            None => (DiffLineKind::Context, ""),
            _ => continue,
        };
        let (old_line, new_line) = match kind {
            DiffLineKind::Add => {
                let n = new_no;
                new_no += 1;
                (None, Some(n))
            }
            DiffLineKind::Delete => {
                let o = old_no;
                old_no += 1;
                (Some(o), None)
            }
            _ => {
                let (o, n) = (old_no, new_no);
                old_no += 1;
                new_no += 1;
                (Some(o), Some(n))
            }
        };
        hunk.lines.push(DiffLine {
            kind,
            text: text.to_string(),
            old_line,
            new_line,
            no_trailing_newline: false,
        });
    }
    if let Some(h) = current.take() {
        hunks.push(h);
    }
    if hunks.is_empty() {
        Diff::Empty
    } else {
        Diff::Text { hunks, truncated }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,4 @@\n one\n-two\n+TWO\n+three\n four\n\\ No newline at end of file\n";

    #[test]
    fn parses_hunks_and_line_numbers() {
        let diff = parse_unified(SAMPLE);
        let Diff::Text { hunks, truncated } = diff else {
            panic!("expected text diff")
        };
        assert!(!truncated);
        assert_eq!(hunks.len(), 1);
        let h = &hunks[0];
        assert_eq!(
            (h.old_start, h.old_lines, h.new_start, h.new_lines),
            (1, 3, 1, 4)
        );
        assert_eq!(h.lines[0].kind, DiffLineKind::Hunk);
        let kinds: Vec<_> = h.lines[1..].iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            vec![
                DiffLineKind::Context,
                DiffLineKind::Delete,
                DiffLineKind::Add,
                DiffLineKind::Add,
                DiffLineKind::Context
            ]
        );
        assert_eq!(h.lines[2].old_line, Some(2));
        assert_eq!(h.lines[3].new_line, Some(2));
        assert_eq!(h.lines[4].new_line, Some(3));
        assert_eq!(h.lines[5].old_line, Some(3));
        assert_eq!(h.lines[5].new_line, Some(4));
        assert!(h.lines[5].no_trailing_newline);
    }

    #[test]
    fn raw_prefix_binary_and_empty() {
        let raw = ":100644 100644 aaa bbb M\0a.txt\0\ndiff --git a/a.txt b/a.txt\nBinary files a/a.txt and b/a.txt differ\n";
        assert_eq!(parse_raw_diff(raw.as_bytes()), Diff::Binary);
        assert_eq!(parse_raw_diff(b""), Diff::Empty);
        let raw = format!(":100644 100644 aaa bbb M\0a.txt\0\n{SAMPLE}");
        assert!(matches!(parse_raw_diff(raw.as_bytes()), Diff::Text { .. }));
    }

    #[test]
    fn live_diff_for_untracked_and_modified() {
        use std::process::Command;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(path)
                    .env("GIT_AUTHOR_NAME", "T")
                    .env("GIT_AUTHOR_EMAIL", "t@example.com")
                    .env("GIT_COMMITTER_NAME", "T")
                    .env("GIT_COMMITTER_EMAIL", "t@example.com")
                    .status()
                    .unwrap()
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "commit.gpgsign", "false"]);
        std::fs::write(path.join("a.txt"), "one\ntwo\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        std::fs::write(path.join("a.txt"), "one\nTWO\n").unwrap();
        std::fs::write(path.join("b.txt"), "new\n").unwrap();
        let git = Arc::new(crate::find_git().unwrap());
        let status = crate::status::get_status(git.clone(), path, None).unwrap();
        for file in &status.files {
            let diff = working_directory_diff(git.clone(), path, file).unwrap();
            let Diff::Text { hunks, .. } = diff else {
                panic!("text diff for {}", file.path)
            };
            assert_eq!(hunks.len(), 1);
            if file.path == "b.txt" {
                assert_eq!(hunks[0].lines[1].kind, DiffLineKind::Add);
                assert_eq!(file.status.kind, corvane_models::FileStatusKind::Untracked);
            }
        }
    }
}
