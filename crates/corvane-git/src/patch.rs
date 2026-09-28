//! Partial staging - GHD `lib/patch-formatter.ts` (`formatPatch`) and
//! `lib/git/apply.ts` (`applyPatchToIndex`).

use std::path::Path;
use std::sync::Arc;

use corvane_models::{
    Diff, DiffHunk, DiffLineKind, DiffSelectionType, FileStatusKind, WorkingDirectoryFileChange,
};

use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::process::GitCommand;

fn format_patch_header(from: Option<&str>, to: Option<&str>) -> String {
    let from = from.map(|p| format!("a/{p}")).unwrap_or("/dev/null".into());
    let to = to.map(|p| format!("b/{p}")).unwrap_or("/dev/null".into());
    format!("--- {from}\n+++ {to}\n")
}

fn format_hunk_header(old_start: u32, old_count: u32, new_start: u32, new_count: u32) -> String {
    let before = if old_count == 1 {
        old_start.to_string()
    } else {
        format!("{old_start},{old_count}")
    };
    let after = if new_count == 1 {
        new_start.to_string()
    } else {
        format!("{new_start},{new_count}")
    };
    format!("@@ -{before} +{after} @@\n")
}

/// Build a unified diff containing only the selected additions/deletions of
/// `file`, suitable for `git apply --cached --unidiff-zero`. Returns `None`
/// when nothing is selected.
pub fn format_patch(file: &WorkingDirectoryFileChange, hunks: &[DiffHunk]) -> Option<String> {
    let is_new = matches!(
        file.status.kind,
        FileStatusKind::New | FileStatusKind::Untracked
    );
    let mut patch = String::new();
    for hunk in hunks {
        let mut buf = String::new();
        let mut old_count = 0u32;
        let mut new_count = 0u32;
        let mut any_change = false;
        for (index, line) in hunk.lines.iter().enumerate() {
            let absolute = hunk.unified_diff_start + index as u32;
            match line.kind {
                DiffLineKind::Hunk => continue,
                DiffLineKind::Context => {
                    buf.push(' ');
                    buf.push_str(&line.text);
                    buf.push('\n');
                    old_count += 1;
                    new_count += 1;
                }
                DiffLineKind::Add | DiffLineKind::Delete
                    if file.selection.is_selected(absolute) =>
                {
                    buf.push(if line.kind == DiffLineKind::Add {
                        '+'
                    } else {
                        '-'
                    });
                    buf.push_str(&line.text);
                    buf.push('\n');
                    if line.kind == DiffLineKind::Add {
                        new_count += 1;
                    } else {
                        old_count += 1;
                    }
                    any_change = true;
                }
                // Unselected lines in a new file never existed as far as
                // this patch is concerned; an unselected addition is dropped.
                DiffLineKind::Add => continue,
                _ if is_new => continue,
                // An unselected deletion stays in the file: context line.
                DiffLineKind::Delete => {
                    buf.push(' ');
                    buf.push_str(&line.text);
                    buf.push('\n');
                    old_count += 1;
                    new_count += 1;
                }
            }
            if line.no_trailing_newline {
                buf.push_str("\\ No newline at end of file\n");
            }
        }
        if !any_change {
            continue;
        }
        patch.push_str(&format_hunk_header(
            hunk.old_start,
            old_count,
            hunk.new_start,
            new_count,
        ));
        patch.push_str(&buf);
    }
    if patch.is_empty() {
        return None;
    }
    let header = if is_new {
        format_patch_header(None, Some(&file.path))
    } else {
        format_patch_header(Some(&file.path), Some(&file.path))
    };
    Some(header + &patch)
}

/// `git apply --cached --unidiff-zero --whitespace=nowarn` with the partial patch.
pub fn apply_patch_to_index(
    git: Arc<GitBinary>,
    workdir: &Path,
    file: &WorkingDirectoryFileChange,
    diff: &Diff,
) -> Result<()> {
    if file.status.kind == FileStatusKind::Renamed
        && let Some(old_path) = &file.old_path
    {
        // Recreate the rename in the index (the index was just reset):
        // `git mv` by hand, then apply the patch against the new path.
        GitCommand::new(git.clone())
            .args(["add", "--update", "--", old_path])
            .current_dir(workdir)
            .run()?;
        let out = GitCommand::new(git.clone())
            .args(["ls-tree", "HEAD", "--", old_path])
            .current_dir(workdir)
            .run()?;
        let text = out.stdout_string()?;
        let info = text.split('\t').next().unwrap_or_default();
        let mut parts = info.split(' ');
        let (mode, _, oid) = (
            parts.next().unwrap_or_default(),
            parts.next(),
            parts.next().unwrap_or_default(),
        );
        GitCommand::new(git.clone())
            .args([
                "update-index",
                "--add",
                "--cacheinfo",
                mode,
                oid,
                &file.path,
            ])
            .current_dir(workdir)
            .run()?;
    }
    let hunks = match diff {
        Diff::Text { hunks, .. } => hunks,
        Diff::Binary | Diff::Submodule => {
            return Err(GitError::Gix(format!(
                "Can't create partial commit in binary file: {}",
                file.path
            )));
        }
        Diff::TooLarge => {
            return Err(GitError::Gix(format!(
                "File diff is too large to generate a partial commit: {}",
                file.path
            )));
        }
        Diff::Empty => return Ok(()),
    };
    let Some(patch) = format_patch(file, hunks) else {
        return Ok(());
    };
    GitCommand::new(git)
        .args([
            "apply",
            "--cached",
            "--unidiff-zero",
            "--whitespace=nowarn",
            "-",
        ])
        .current_dir(workdir)
        .stdin(patch.into_bytes())
        .run()?;
    Ok(())
}

/// Stage the partially-selected files by patch (GHD `stageFiles`, step 3).
pub fn stage_partial_files(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
) -> Result<()> {
    for file in files
        .iter()
        .filter(|f| f.selection.kind() == DiffSelectionType::Partial)
    {
        let diff = crate::diff::working_directory_diff(git.clone(), workdir, file)?;
        apply_patch_to_index(git.clone(), workdir, file, &diff)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvane_models::{DiffSelection, FileStatus, GitStatusEntry};
    use std::process::Command;

    fn file(
        path: &str,
        kind: FileStatusKind,
        selection: DiffSelection,
    ) -> WorkingDirectoryFileChange {
        WorkingDirectoryFileChange {
            path: path.into(),
            old_path: None,
            status: FileStatus {
                kind,
                submodule: false,
                index: GitStatusEntry::Unchanged,
                working_tree: GitStatusEntry::Modified,
                score: None,
                code: String::new(),
            },
            selection,
        }
    }

    #[test]
    fn formats_selected_lines_only() {
        let diff = crate::parse_unified(
            "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n@@ -10,2 +10,3 @@\n ten\n+eleven\n twelve\n",
        );
        let Diff::Text { hunks, .. } = &diff else {
            panic!("text diff expected")
        };
        // lines: 0 hunk,1 ctx,2 del,3 add,4 ctx | 5 hunk,6 ctx,7 add,8 ctx
        let sel = DiffSelection::none().with_line(3, true);
        let f = file("f", FileStatusKind::Modified, sel);
        let patch = format_patch(&f, hunks).unwrap();
        assert_eq!(
            patch,
            "--- a/f\n+++ b/f\n@@ -1,3 +1,4 @@\n one\n two\n+TWO\n three\n"
        );
        let none = file("f", FileStatusKind::Modified, DiffSelection::none());
        assert!(format_patch(&none, hunks).is_none());
        let sel = DiffSelection::all().with_line(7, false);
        let f = file("f", FileStatusKind::Modified, sel);
        let patch = format_patch(&f, hunks).unwrap();
        assert_eq!(
            patch,
            "--- a/f\n+++ b/f\n@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n"
        );
    }

    #[test]
    fn new_file_drops_unselected_lines() {
        let diff = crate::parse_unified("--- /dev/null\n+++ b/n\n@@ -0,0 +1,3 @@\n+a\n+b\n+c\n");
        let Diff::Text { hunks, .. } = &diff else {
            panic!("text diff expected")
        };
        let sel = DiffSelection::all().with_line(2, false);
        let f = file("n", FileStatusKind::New, sel);
        assert_eq!(
            format_patch(&f, hunks).unwrap(),
            "--- /dev/null\n+++ b/n\n@@ -0,0 +1,2 @@\n+a\n+c\n"
        );
    }

    #[test]
    fn partial_commit_round_trip() {
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
        std::fs::write(path.join("f.txt"), "one\ntwo\nthree\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        std::fs::write(path.join("f.txt"), "ONE\ntwo\nTHREE\n").unwrap();
        let git = Arc::new(crate::find_git().unwrap());
        let mut status = crate::get_status(git.clone(), path, None).unwrap();
        let file = &mut status.files[0];
        let diff = crate::working_directory_diff(git.clone(), path, file).unwrap();
        // select only the first change (lines: 0 hunk, 1 del ONE, 2 add ONE, 3 ctx, 4 del, 5 add)
        file.selection = DiffSelection::none().with_range(1, 2, true);
        assert_eq!(file.selection.kind(), DiffSelectionType::Partial);
        crate::unstage_all(git.clone(), path).unwrap();
        crate::stage_files(git.clone(), path, &status.files).unwrap();
        stage_partial_files(git.clone(), path, &status.files).unwrap();
        let _ = diff;
        crate::commit(git.clone(), path, "partial\n", &Default::default()).unwrap();
        let shown = Command::new("git")
            .args(["show", "HEAD:f.txt"])
            .current_dir(path)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&shown.stdout), "ONE\ntwo\nthree\n");
        let after = crate::get_status(git, path, None).unwrap();
        assert_eq!(after.files.len(), 1, "the other change stays unstaged");
    }
}
