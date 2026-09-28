//! Staging, committing, discarding and undoing - GHD `lib/git/{update-index,
//! reset,commit,checkout-index}.ts` and `app-store._commitIncludedChanges`.

use std::path::Path;
use std::sync::Arc;

use corvane_models::{DiffSelection, FileStatusKind, WorkingDirectoryFileChange};
use tracing::info;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

#[derive(Clone, Debug, Default)]
pub struct CommitOptions {
    pub amend: bool,
    pub no_verify: bool,
    pub signoff: bool,
    pub allow_empty: bool,
}

/// `git reset -- .` (GHD `unstageAll`). On an unborn branch there is no HEAD
/// to reset to, so fall back to clearing the index.
pub fn unstage_all(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    let out = GitCommand::new(git.clone())
        .args(["reset", "--", "."])
        .current_dir(workdir)
        .allow_exit_code(128)
        .run()?;
    if !out.status.success() {
        GitCommand::new(git)
            .args(["rm", "-r", "--cached", "-q", "--ignore-unmatch", "--", "."])
            .current_dir(workdir)
            .allow_exit_code(128)
            .run()?;
    }
    Ok(())
}

/// Stage every fully-included file with `update-index --add --remove --replace`
/// (GHD `stageFiles`). Partially-selected files are handled by the patch path
/// (`apply --cached`, later milestone); for now they are staged whole.
pub fn stage_files(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
) -> Result<()> {
    let mut paths: Vec<u8> = Vec::new();
    for file in files {
        if file.selection == DiffSelection::None {
            continue;
        }
        if let Some(old) = &file.old_path {
            paths.extend_from_slice(old.as_bytes());
            paths.push(0);
        }
        paths.extend_from_slice(file.path.as_bytes());
        paths.push(0);
    }
    if paths.is_empty() {
        return Ok(());
    }
    GitCommand::new(git)
        .args([
            "update-index",
            "--add",
            "--remove",
            "--replace",
            "-z",
            "--stdin",
        ])
        .current_dir(workdir)
        .stdin(paths)
        .run()?;
    Ok(())
}

/// `git commit -F -` with the message on stdin (GHD `createCommit`). Returns the new HEAD sha.
pub fn commit(
    git: Arc<GitBinary>,
    workdir: &Path,
    message: &str,
    opts: &CommitOptions,
) -> Result<String> {
    let mut args = vec!["commit", "-F", "-"];
    if opts.amend {
        args.push("--amend");
    }
    if opts.no_verify {
        args.push("--no-verify");
    }
    if opts.signoff {
        args.push("--signoff");
    }
    if opts.allow_empty {
        args.push("--allow-empty");
    }
    GitCommand::new(git.clone())
        .args(args)
        .current_dir(workdir)
        .stdin(message.as_bytes().to_vec())
        .run()?;
    let sha = head_sha(git, workdir)?;
    info!(%sha, "created commit");
    Ok(sha)
}

pub fn head_sha(git: Arc<GitBinary>, workdir: &Path) -> Result<String> {
    let out = GitCommand::new(git)
        .args(["rev-parse", "HEAD"])
        .current_dir(workdir)
        .run()?;
    Ok(out.stdout_string()?.trim().to_string())
}

/// Summary + blank line + description, as GHD formats the message.
pub fn format_message(summary: &str, description: &str) -> String {
    let summary = summary.trim();
    let description = description.trim();
    if description.is_empty() {
        format!("{summary}\n")
    } else {
        format!("{summary}\n\n{description}\n")
    }
}

/// `git reset --soft HEAD^` (GHD `undoCommit` keeps the changes in the working
/// directory). A root commit is undone by deleting the branch's HEAD ref.
pub fn undo_last_commit(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    let has_parent = GitCommand::new(git.clone())
        .args(["rev-parse", "--verify", "--quiet", "HEAD^"])
        .current_dir(workdir)
        .allow_exit_code(1)
        .run()?
        .status
        .success();
    if has_parent {
        GitCommand::new(git)
            .args(["reset", "--soft", "HEAD^"])
            .current_dir(workdir)
            .run()?;
    } else {
        GitCommand::new(git.clone())
            .args(["update-ref", "-d", "HEAD"])
            .current_dir(workdir)
            .run()?;
        // keep the files: clear the index back to "unstaged"
        unstage_all(git, workdir)?;
    }
    Ok(())
}

/// Discard working-directory changes (GHD `discardChanges`): tracked files are
/// reset and checked out from HEAD; new/untracked files go to the Trash
/// (`moveToTrash`, so a discard is recoverable) or are deleted.
pub fn discard_changes(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
    move_to_trash: bool,
) -> Result<()> {
    let mut tracked: Vec<&str> = Vec::new();
    for file in files {
        match file.status.kind {
            FileStatusKind::New | FileStatusKind::Untracked => {
                let full = workdir.join(&file.path);
                if !move_to_trash || trash::delete(&full).is_err() {
                    let _ = std::fs::remove_file(&full).or_else(|_| std::fs::remove_dir_all(&full));
                }
            }
            _ => {
                tracked.push(&file.path);
                if let Some(old) = &file.old_path {
                    tracked.push(old);
                }
            }
        }
    }
    if !tracked.is_empty() {
        let mut list: Vec<u8> = Vec::new();
        for p in &tracked {
            list.extend_from_slice(p.as_bytes());
            list.push(0);
        }
        GitCommand::new(git.clone())
            .args(["reset", "-q", "--", "."])
            .current_dir(workdir)
            .allow_exit_code(128)
            .run()?;
        GitCommand::new(git)
            .args(["checkout-index", "-f", "-u", "-q", "--stdin", "-z"])
            .current_dir(workdir)
            .stdin(list)
            .run()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn repo() -> (tempfile::TempDir, Arc<GitBinary>) {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(dir.path())
                    .status()
                    .unwrap()
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "commit.gpgsign", "false"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "user.email", "t@example.com"]);
        (dir, Arc::new(crate::find_git().unwrap()))
    }

    #[test]
    fn stage_commit_undo_round_trip() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "one\n").unwrap();
        std::fs::write(path.join("b.txt"), "two\n").unwrap();
        let mut status = crate::get_status(git.clone(), path, None).unwrap();
        // exclude b.txt
        for f in &mut status.files {
            if f.path == "b.txt" {
                f.selection = DiffSelection::None;
            }
        }
        unstage_all(git.clone(), path).unwrap();
        stage_files(git.clone(), path, &status.files).unwrap();
        let sha = commit(
            git.clone(),
            path,
            &format_message("Add a", "details"),
            &CommitOptions::default(),
        )
        .unwrap();
        assert_eq!(sha.len(), 40);
        let after = crate::get_status(git.clone(), path, None).unwrap();
        let paths: Vec<_> = after.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["b.txt"]);
        let log = Command::new("git")
            .args(["log", "-1", "--format=%B"])
            .current_dir(path)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&log.stdout).trim(),
            "Add a\n\ndetails"
        );

        // second commit then undo keeps changes
        std::fs::write(path.join("a.txt"), "changed\n").unwrap();
        let status = crate::get_status(git.clone(), path, None).unwrap();
        unstage_all(git.clone(), path).unwrap();
        stage_files(git.clone(), path, &status.files).unwrap();
        commit(git.clone(), path, "second\n", &CommitOptions::default()).unwrap();
        undo_last_commit(git.clone(), path).unwrap();
        let head = head_sha(git.clone(), path).unwrap();
        assert_eq!(head, sha);
        let status = crate::get_status(git.clone(), path, None).unwrap();
        assert!(status.files.iter().any(|f| f.path == "a.txt"));

        // undo the root commit: files stay, HEAD unborn
        undo_last_commit(git.clone(), path).unwrap();
        let info = crate::open_repository(path).unwrap();
        assert!(matches!(info.tip, corvane_models::Tip::Unborn { .. }));
        assert!(path.join("a.txt").exists());
    }

    #[test]
    fn discard_restores_and_removes() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "one\n").unwrap();
        let status = crate::get_status(git.clone(), path, None).unwrap();
        stage_files(git.clone(), path, &status.files).unwrap();
        commit(git.clone(), path, "init\n", &CommitOptions::default()).unwrap();
        std::fs::write(path.join("a.txt"), "dirty\n").unwrap();
        std::fs::write(path.join("new.txt"), "x\n").unwrap();
        let status = crate::get_status(git.clone(), path, None).unwrap();
        discard_changes(git.clone(), path, &status.files, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "one\n"
        );
        assert!(!path.join("new.txt").exists());
        let status = crate::get_status(git, path, None).unwrap();
        assert!(status.files.is_empty());
    }
}
