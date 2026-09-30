//! Working-directory diffs (GHD `lib/git/diff.ts` + `lib/diff-parser.ts`):
//! text, image, submodule and large-diff detection, plus the blob / file
//! readers that back hunk expansion (`fileContents.newContents`).
//!
//! Deviations: a renamed file can diff against `HEAD:<old path>`
//! (`174-renamed-diff-against-head`); a mode-only change carries the modes
//! (`173-file-mode-change-message`); a symbolic link's working copy is its
//! target path (`176-symlink-contents`).

use std::path::Path;
use std::sync::Arc;

use corvane_models::{
    Diff, DiffHunk, DiffLine, DiffLineKind, DiffWarnings, FileStatusKind, ImageBlob,
    LineEndingsChange, SubmoduleDiff, SubmoduleStatus, WorkingDirectoryFileChange,
    image_media_type,
};

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// GHD `MaxReasonableDiffSize`: beyond this the diff is `LargeText` and only
/// rendered on request.
pub const MAX_DIFF_BYTES: usize = 3 * 1024 * 1024;
/// GHD `MaxDiffBufferSize` (70 MB): beyond this nothing is rendered.
pub const UNRENDERABLE_BYTES: usize = 70 * 1024 * 1024;
/// Line budget after which a diff is `LargeText`.
pub const MAX_DIFF_LINES: usize = 50_000;

/// `git diff` for one working-directory file, compared against HEAD (or an
/// empty file for new/untracked files), exactly like GHD. `hide_whitespace`
/// adds `-w` (Diff Settings › Hide Whitespace Changes).
///
/// A renamed file is diffed index-to-working-tree like GHD, which hides
/// staged edits; `renamed_against_head` (Corvane `174-renamed-diff-against-head`)
/// diffs `HEAD:<old path>` to the working copy instead (`HEAD -M -- old new`),
/// the change the commit will record, falling back to GHD's diff when git
/// does not pair the two paths as one rename.
pub fn working_directory_diff(
    git: Arc<GitBinary>,
    workdir: &Path,
    file: &WorkingDirectoryFileChange,
    hide_whitespace: bool,
    renamed_against_head: bool,
) -> Result<Diff> {
    let mut args = vec!["diff"];
    if hide_whitespace {
        args.push("-w");
    }
    args.extend(["--no-ext-diff", "--patch-with-raw", "-z", "--no-color"]);
    let base = || {
        GitCommand::new(git.clone())
            .args(&args)
            .current_dir(workdir)
    };
    let mut cmd = base();
    let is_submodule = file.status.submodule;
    let mut rename_out = None;
    if !is_submodule && file.status.kind.is_new_or_untracked() {
        // `--no-index` exits 1 when files differ, which is the normal case.
        cmd = cmd
            .args(["--no-index", "--", "/dev/null"])
            .arg(&file.path)
            .allow_exit_code(1);
    } else if file.status.kind == FileStatusKind::Renamed {
        if renamed_against_head && let Some(old_path) = &file.old_path {
            let out = base()
                .args(["-M", "HEAD", "--"])
                .arg(old_path)
                .arg(&file.path)
                .run()?;
            // one patch: git paired the paths (a delete plus an add otherwise)
            if String::from_utf8_lossy(&out.stdout)
                .matches("diff --git ")
                .count()
                == 1
            {
                rename_out = Some(out);
            }
        }
        cmd = cmd.args(["--"]).arg(&file.path);
    } else {
        cmd = cmd.args(["HEAD", "--"]).arg(&file.path);
    }
    let out = match rename_out {
        Some(out) => out,
        None => cmd.run()?,
    };
    if is_submodule {
        return Ok(submodule_diff(
            git,
            workdir,
            &file.path,
            file.status.submodule_status.unwrap_or_default(),
            file.status.kind,
            &out.stdout,
        ));
    }
    let diff = parse_raw_diff_with_warnings(&out.stdout, &out.stderr);
    Ok(match diff {
        Diff::Binary => {
            let previous_path = file.old_path.as_deref().unwrap_or(&file.path);
            image_diff(
                &file.path,
                file.status.kind,
                || std::fs::read(workdir.join(&file.path)).ok(),
                || blob_bytes(git.clone(), workdir, "HEAD", previous_path).ok(),
            )
        }
        other => other,
    })
}

/// Corvane `279-copy-diff`: the working-directory changes of `files` as one
/// patch `git apply` takes (`--binary`), against `base` (`HEAD`, or
/// [`crate::NULL_TREE_SHA`] on an unborn branch). Tracked files come first,
/// in one `git diff`, then each new / untracked file against `/dev/null`.
pub fn working_directory_patch(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
    base: &str,
) -> Result<String> {
    const ARGS: [&str; 4] = ["diff", "--no-ext-diff", "--no-color", "--binary"];
    let (untracked, tracked): (Vec<_>, Vec<_>) = files
        .iter()
        .partition(|f| !f.status.submodule && f.status.kind.is_new_or_untracked());
    let mut patch = Vec::new();
    if !tracked.is_empty() {
        let paths = tracked
            .iter()
            .flat_map(|f| std::iter::once(&f.path).chain(f.old_path.as_ref()));
        let out = GitCommand::new(git.clone())
            .args(ARGS)
            .args([base, "--"])
            .args(paths)
            .current_dir(workdir)
            .run()?;
        patch.extend(out.stdout);
    }
    for f in untracked {
        // `--no-index` exits 1 when the files differ
        let out = GitCommand::new(git.clone())
            .args(ARGS)
            .args(["--no-index", "--", "/dev/null"])
            .arg(&f.path)
            .current_dir(workdir)
            .allow_exit_code(1)
            .run()?;
        patch.extend(out.stdout);
    }
    Ok(String::from_utf8_lossy(&patch).into_owned())
}

/// GHD `getImageDiff`: a binary change of a known image type becomes an
/// image diff; anything else stays `Binary`.
pub fn image_diff(
    path: &str,
    kind: FileStatusKind,
    current: impl FnOnce() -> Option<Vec<u8>>,
    previous: impl FnOnce() -> Option<Vec<u8>>,
) -> Diff {
    let Some(media_type) = image_media_type(path) else {
        return Diff::Binary;
    };
    let blob = |bytes: Vec<u8>| ImageBlob {
        bytes,
        media_type: media_type.to_string(),
    };
    let current = (kind != FileStatusKind::Deleted)
        .then(current)
        .flatten()
        .map(blob);
    let previous = (!kind.is_new_or_untracked())
        .then(previous)
        .flatten()
        .map(blob);
    if current.is_none() && previous.is_none() {
        return Diff::Binary;
    }
    Diff::Image { previous, current }
}

/// GHD `buildSubmoduleDiff`: the `Subproject commit` lines of the patch plus
/// `submodule.<path>.url` from the repository config.
pub fn submodule_diff(
    git: Arc<GitBinary>,
    workdir: &Path,
    path: &str,
    status: SubmoduleStatus,
    kind: FileStatusKind,
    patch: &[u8],
) -> Diff {
    let url = crate::config::local_config_value(git, workdir, &format!("submodule.{path}.url"));
    let (mut old_sha, mut new_sha) = (None, None);
    if status.commit_changed || kind == FileStatusKind::New || kind == FileStatusKind::Deleted {
        let text = String::from_utf8_lossy(patch);
        let sha = |rest: &str| rest.trim_end().trim_end_matches("-dirty").to_string();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("-Subproject commit ") {
                old_sha.get_or_insert_with(|| sha(rest));
            } else if let Some(rest) = line.strip_prefix("+Subproject commit ") {
                new_sha.get_or_insert_with(|| sha(rest));
            }
        }
    }
    Diff::Submodule(SubmoduleDiff {
        path: path.to_string(),
        full_path: workdir.join(path),
        url,
        old_sha,
        new_sha,
        status,
    })
}

/// `git show <commitish>:<path>` - the raw bytes of a blob (`getBlobContents`).
pub fn blob_bytes(
    git: Arc<GitBinary>,
    workdir: &Path,
    commitish: &str,
    path: &str,
) -> Result<Vec<u8>> {
    let out = GitCommand::new(git)
        .args(["show", &format!("{commitish}:{path}")])
        .current_dir(workdir)
        .run()?;
    Ok(out.stdout)
}

/// File contents as lines for hunk expansion; a trailing newline does not
/// produce an empty last line.
pub fn file_lines(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

/// The working copy of `path` as lines (`None` when unreadable, e.g. deleted).
///
/// GHD reads through a symbolic link, which hangs on a link to a FIFO or a
/// device and loads a huge target whole; `symlinks_as_links` (Corvane
/// `176-symlink-contents`) reads the link's target path instead, the one
/// line git records and diffs for a link.
pub fn working_file_lines(
    workdir: &Path,
    path: &str,
    symlinks_as_links: bool,
) -> Option<Vec<String>> {
    let full = workdir.join(path);
    if symlinks_as_links
        && std::fs::symlink_metadata(&full).is_ok_and(|m| m.file_type().is_symlink())
    {
        let target = std::fs::read_link(&full).ok()?;
        return Some(file_lines(target.to_string_lossy().as_bytes()));
    }
    std::fs::read(full).ok().map(|b| file_lines(&b))
}

/// A committed blob as lines (`None` when the path is not in that commit).
pub fn blob_lines(
    git: Arc<GitBinary>,
    workdir: &Path,
    commitish: &str,
    path: &str,
) -> Option<Vec<String>> {
    blob_bytes(git, workdir, commitish, path)
        .ok()
        .map(|b| file_lines(&b))
}

/// GHD `parseLineEndingsWarning`: git's stderr notice that the working copy's
/// line endings will be converted on checkout (both the classic and the
/// git ≥ 2.37 wording).
pub fn parse_line_endings_warning(stderr: &str) -> Option<LineEndingsChange> {
    for line in stderr.lines() {
        let line = line.trim();
        let rest = line.strip_prefix("warning: ")?;
        // "in the working copy of 'x', CRLF will be replaced by LF the next time…"
        let rest = match rest.find(", ") {
            Some(ix) if rest.starts_with("in the working copy of") => &rest[ix + 2..],
            _ => rest,
        };
        let (from, tail) = rest.split_once(" will be replaced by ")?;
        let to = tail.split([' ', '.']).next().unwrap_or("");
        let valid = |s: &str| matches!(s, "CRLF" | "LF" | "CR");
        if valid(from) && valid(to) {
            return Some(LineEndingsChange {
                from: from.to_string(),
                to: to.to_string(),
            });
        }
    }
    None
}

/// GHD `HiddenBidiCharsRegex`.
pub fn has_hidden_bidi_chars(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
}

/// [`parse_raw_diff`] plus the warnings derived from git's stderr.
pub fn parse_raw_diff_with_warnings(stdout: &[u8], stderr: &str) -> Diff {
    let line_endings = parse_line_endings_warning(stderr);
    match parse_raw_diff(stdout) {
        Diff::Text {
            hunks,
            mut warnings,
        } => {
            warnings.line_endings = line_endings;
            Diff::Text { hunks, warnings }
        }
        Diff::LargeText {
            hunks,
            mut warnings,
        } => {
            warnings.line_endings = line_endings;
            Diff::LargeText { hunks, warnings }
        }
        other => other,
    }
}

/// Parse `--patch-with-raw -z` output: skip the raw header block, then the
/// unified diff. Handles binary and oversize diffs.
pub fn parse_raw_diff(stdout: &[u8]) -> Diff {
    if stdout.len() > UNRENDERABLE_BYTES {
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
    match parse_unified(patch) {
        Diff::Text { hunks, warnings } if stdout.len() > MAX_DIFF_BYTES => {
            Diff::LargeText { hunks, warnings }
        }
        other => other,
    }
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
    let (mut old_mode, mut new_mode) = (None, None);

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
            // file header lines before the first hunk
            if let Some(mode) = line.strip_prefix("old mode ") {
                old_mode = Some(mode.to_string());
            } else if let Some(mode) = line.strip_prefix("new mode ") {
                new_mode = Some(mode.to_string());
            }
            continue;
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
    let mode_change = old_mode.zip(new_mode);
    if hunks.is_empty() && mode_change.is_none() {
        return Diff::Empty;
    }
    let warnings = DiffWarnings {
        hidden_bidi: hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .any(|l| has_hidden_bidi_chars(&l.text)),
        line_endings: None,
        mode_change,
    };
    if truncated {
        Diff::LargeText { hunks, warnings }
    } else {
        Diff::Text { hunks, warnings }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_directory_patch_applies() {
        use std::process::Command;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(path)
                    .status()
                    .unwrap()
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "commit.gpgsign", "false"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "user.email", "t@example.com"]);
        std::fs::write(path.join("a.txt"), "one\n").unwrap();
        std::fs::write(path.join("skip.txt"), "same\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        std::fs::write(path.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(path.join("skip.txt"), "changed\n").unwrap();
        std::fs::write(path.join("new.txt"), "fresh\n").unwrap();
        let git = Arc::new(crate::find_git().unwrap());
        let status = crate::get_status(git.clone(), path, None).unwrap();
        let files: Vec<_> = status
            .files
            .into_iter()
            .filter(|f| f.path != "skip.txt")
            .collect();
        let patch = working_directory_patch(git, path, &files, "HEAD").unwrap();
        assert!(patch.contains("+two"));
        assert!(patch.contains("+fresh"));
        assert!(!patch.contains("skip.txt"));
        // the patch reverses cleanly onto the working tree
        std::fs::write(path.join("p.diff"), &patch).unwrap();
        run(&["apply", "--check", "-R", "p.diff"]);
    }

    const SAMPLE: &str = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,4 @@\n one\n-two\n+TWO\n+three\n four\n\\ No newline at end of file\n";

    #[test]
    fn parses_hunks_and_line_numbers() {
        let diff = parse_unified(SAMPLE);
        let Diff::Text { hunks, .. } = diff else {
            panic!("expected text diff")
        };
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
            let diff = working_directory_diff(git.clone(), path, file, false, false).unwrap();
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

    #[test]
    fn mode_only_change_is_a_text_diff_without_hunks() {
        let raw = ":100644 100755 aaa aaa M\0a.sh\0\ndiff --git a/a.sh b/a.sh\nold mode 100644\nnew mode 100755\n";
        let Diff::Text { hunks, warnings } = parse_raw_diff(raw.as_bytes()) else {
            panic!("text diff expected")
        };
        assert!(hunks.is_empty());
        assert_eq!(
            warnings.mode_change,
            Some(("100644".to_string(), "100755".to_string()))
        );
        let with_hunks = SAMPLE.replacen(
            "index 1..2 100644\n",
            "old mode 100644\nnew mode 100755\n",
            1,
        );
        let Diff::Text { hunks, warnings } = parse_unified(&with_hunks) else {
            panic!("text diff expected")
        };
        assert_eq!(hunks.len(), 1);
        assert!(warnings.mode_change.is_some());
    }

    #[test]
    fn symlink_lines_are_the_target() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("real.txt"), "one\ntwo\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("real.txt", dir.path().join("link")).unwrap();
        #[cfg(not(unix))]
        return;
        assert_eq!(
            working_file_lines(dir.path(), "link", false),
            Some(vec!["one".to_string(), "two".to_string()])
        );
        assert_eq!(
            working_file_lines(dir.path(), "link", true),
            Some(vec!["real.txt".to_string()])
        );
        assert_eq!(
            working_file_lines(dir.path(), "real.txt", true).map(|l| l.len()),
            Some(2)
        );
    }

    #[test]
    fn parses_line_endings_warnings() {
        let old = "warning: CRLF will be replaced by LF in a.txt.\nThe file will have its original line endings in your working directory\n";
        assert_eq!(
            parse_line_endings_warning(old),
            Some(LineEndingsChange {
                from: "CRLF".into(),
                to: "LF".into()
            })
        );
        let new = "warning: in the working copy of 'a.txt', CRLF will be replaced by LF the next time Git touches it\n";
        assert_eq!(
            parse_line_endings_warning(new).map(|c| c.to),
            Some("LF".to_string())
        );
        assert_eq!(parse_line_endings_warning("warning: something else"), None);
        assert!(has_hidden_bidi_chars("abc\u{202E}def"));
        assert!(!has_hidden_bidi_chars("plain"));
    }
}
