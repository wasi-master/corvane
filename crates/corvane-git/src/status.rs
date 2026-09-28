//! `git status --porcelain=2 -z` (GHD `lib/status-parser.ts` + `lib/git/status.ts`).

use std::path::Path;
use std::sync::Arc;

use corvane_models::{
    AheadBehind, DiffSelection, FileStatus, FileStatusKind, GitStatusEntry,
    WorkingDirectoryFileChange, WorkingDirectoryStatus,
};

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// GHD `conflictStatusCodes`.
const CONFLICT_CODES: &[&str] = &["DD", "AU", "UD", "UA", "DU", "AA", "UU"];

/// Run status and build the model. `previous` carries over per-file selections.
pub fn get_status(
    git: Arc<GitBinary>,
    workdir: &Path,
    previous: Option<&WorkingDirectoryStatus>,
) -> Result<WorkingDirectoryStatus> {
    let out = GitCommand::new(git)
        .args([
            "status",
            "--untracked-files=all",
            "--branch",
            "--porcelain=2",
            "-z",
        ])
        .current_dir(workdir)
        .run()?;
    let mut status = parse_porcelain_v2(&out.stdout);
    status.merge_head_found = workdir.join(".git/MERGE_HEAD").exists();
    status.rebase_in_progress =
        workdir.join(".git/rebase-merge").exists() || workdir.join(".git/rebase-apply").exists();
    if let Some(prev) = previous {
        for file in &mut status.files {
            if let Some(old) = prev.files.iter().find(|f| f.path == file.path) {
                file.selection = old.selection;
            }
        }
    }
    Ok(status)
}

/// Parse NUL-separated porcelain v2 output.
pub fn parse_porcelain_v2(stdout: &[u8]) -> WorkingDirectoryStatus {
    let text = String::from_utf8_lossy(stdout);
    let mut fields = text.split('\0').filter(|f| !f.is_empty()).peekable();
    let mut status = WorkingDirectoryStatus::default();

    while let Some(field) = fields.next() {
        let mut parts = field.splitn(2, ' ');
        let tag = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("");
        match tag {
            "#" => parse_header(rest, &mut status),
            "1" => {
                // <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>
                let cols: Vec<&str> = rest.splitn(8, ' ').collect();
                if cols.len() == 8 {
                    let code = cols[0];
                    let sub = cols[1];
                    push_file(&mut status, cols[7], None, code, sub, None);
                }
            }
            "2" => {
                // <XY> <sub> <mH> <mI> <mW> <hH> <hI> <Xscore> <path>\0<origPath>
                let cols: Vec<&str> = rest.splitn(9, ' ').collect();
                if cols.len() == 9 {
                    let code = cols[0];
                    let sub = cols[1];
                    let score = cols[7].trim_start_matches(['R', 'C']).parse::<u8>().ok();
                    let orig = fields.next().map(str::to_string);
                    push_file(&mut status, cols[8], orig, code, sub, score);
                }
            }
            "u" => {
                // <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>
                let cols: Vec<&str> = rest.splitn(10, ' ').collect();
                if cols.len() == 10 {
                    push_file(&mut status, cols[9], None, cols[0], cols[1], None);
                }
            }
            "?" => push_file(&mut status, rest, None, "??", "N...", None),
            "!" => {}
            _ => {}
        }
    }
    status
}

fn parse_header(rest: &str, status: &mut WorkingDirectoryStatus) {
    let mut parts = rest.splitn(2, ' ');
    let key = parts.next().unwrap_or("");
    let value = parts.next().unwrap_or("").trim();
    match key {
        "branch.head" if value != "(detached)" => status.branch = Some(value.to_string()),
        "branch.upstream" => status.upstream = Some(value.to_string()),
        "branch.ab" => {
            // "+A -B"
            let mut ab = AheadBehind::default();
            for token in value.split_whitespace() {
                if let Some(a) = token.strip_prefix('+') {
                    ab.ahead = a.parse().unwrap_or(0);
                } else if let Some(b) = token.strip_prefix('-') {
                    ab.behind = b.parse().unwrap_or(0);
                }
            }
            status.ahead_behind = Some(ab);
        }
        _ => {}
    }
}

fn push_file(
    status: &mut WorkingDirectoryStatus,
    path: &str,
    old_path: Option<String>,
    code: &str,
    sub: &str,
    score: Option<u8>,
) {
    let Some(file_status) = map_status(code, sub, score) else {
        return;
    };
    status.files.push(WorkingDirectoryFileChange {
        path: path.to_string(),
        old_path,
        status: file_status,
        selection: DiffSelection::All,
    });
}

fn entry(c: char) -> GitStatusEntry {
    match c {
        'M' | 'T' => GitStatusEntry::Modified,
        'A' => GitStatusEntry::Added,
        'D' => GitStatusEntry::Deleted,
        'R' => GitStatusEntry::Renamed,
        'C' => GitStatusEntry::Copied,
        'U' => GitStatusEntry::Unmerged,
        '?' => GitStatusEntry::Untracked,
        _ => GitStatusEntry::Unchanged,
    }
}

/// GHD `mapStatus` + `convertToAppStatus`.
pub fn map_status(code: &str, sub: &str, score: Option<u8>) -> Option<FileStatus> {
    let mut chars = code.chars();
    let x = chars.next()?;
    let y = chars.next()?;
    let index = entry(x);
    let working_tree = entry(y);
    let submodule = sub.starts_with('S');

    let kind = if code == "??" {
        FileStatusKind::Untracked
    } else if CONFLICT_CODES.contains(&code) {
        FileStatusKind::Conflicted
    } else {
        match (x, y) {
            ('R', _) | (_, 'R') => FileStatusKind::Renamed,
            ('C', _) | (_, 'C') => FileStatusKind::Copied,
            // GHD: added-then-deleted / added-then-modified count as New
            ('A', _) | (_, 'A') => FileStatusKind::New,
            ('D', _) | (_, 'D') => FileStatusKind::Deleted,
            ('M', _) | (_, 'M') | ('T', _) | (_, 'T') => FileStatusKind::Modified,
            _ => return None,
        }
    };
    Some(FileStatus {
        kind,
        index,
        working_tree,
        score,
        code: code.to_string(),
        submodule,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headers_and_entries() {
        let raw = concat!(
            "# branch.oid abc\0",
            "# branch.head main\0",
            "# branch.upstream origin/main\0",
            "# branch.ab +2 -1\0",
            "1 .M N... 100644 100644 100644 aaa bbb src/lib.rs\0",
            "1 A. N... 000000 100644 100644 000 ccc new.txt\0",
            "1 D. N... 100644 000000 000000 ddd 000 gone.txt\0",
            "2 R. N... 100644 100644 100644 eee eee R100 b.txt\0a.txt\0",
            "u UU N... 100644 100644 100644 100644 f1 f2 f3 conflict.txt\0",
            "? untracked.md\0",
            "! ignored.log\0",
        );
        let s = parse_porcelain_v2(raw.as_bytes());
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!(
            s.ahead_behind,
            Some(AheadBehind {
                ahead: 2,
                behind: 1
            })
        );
        let kinds: Vec<(String, FileStatusKind)> = s
            .files
            .iter()
            .map(|f| (f.path.clone(), f.status.kind))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("src/lib.rs".into(), FileStatusKind::Modified),
                ("new.txt".into(), FileStatusKind::New),
                ("gone.txt".into(), FileStatusKind::Deleted),
                ("b.txt".into(), FileStatusKind::Renamed),
                ("conflict.txt".into(), FileStatusKind::Conflicted),
                ("untracked.md".into(), FileStatusKind::Untracked),
            ]
        );
        let renamed = &s.files[3];
        assert_eq!(renamed.old_path.as_deref(), Some("a.txt"));
        assert_eq!(renamed.status.score, Some(100));
        assert_eq!(s.files[0].status.index, GitStatusEntry::Unchanged);
        assert_eq!(s.files[0].status.working_tree, GitStatusEntry::Modified);
        assert!(s.has_conflicts());
        assert_eq!(s.include_all(), Some(true));
    }

    #[test]
    fn detached_head_has_no_branch() {
        let s = parse_porcelain_v2(b"# branch.oid abc\0# branch.head (detached)\0");
        assert!(s.branch.is_none());
        assert!(s.files.is_empty());
        assert_eq!(s.include_all(), Some(true));
    }

    #[test]
    fn live_status_on_temp_repo() {
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
        std::fs::write(path.join("a.txt"), "one\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        std::fs::write(path.join("a.txt"), "two\n").unwrap();
        std::fs::write(path.join("b.txt"), "new\n").unwrap();

        let git = Arc::new(crate::find_git().unwrap());
        let s = get_status(git, path, None).unwrap();
        assert_eq!(s.branch.as_deref(), Some("main"));
        let mut paths: Vec<_> = s
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.status.kind))
            .collect();
        paths.sort_by(|a, b| a.0.cmp(b.0));
        assert_eq!(
            paths,
            vec![
                ("a.txt", FileStatusKind::Modified),
                ("b.txt", FileStatusKind::Untracked)
            ]
        );
    }
}
