//! Staging, committing, discarding and undoing - GHD `lib/git/{update-index,
//! reset,commit,checkout-index}.ts` and `app-store._commitIncludedChanges`.

use std::path::Path;
use std::sync::Arc;

use corvane_models::{DiffSelectionType, FileStatusKind, WorkingDirectoryFileChange};
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
/// (GHD `stageFiles`). Partially-selected files are staged separately by
/// `stage_partial_files` (`apply --cached`).
pub fn stage_files(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
) -> Result<()> {
    let mut paths: Vec<u8> = Vec::new();
    for file in files {
        if file.selection.kind() != DiffSelectionType::All {
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

/// Corvane `715-assume-unchanged`: `update-index --[no-]assume-unchanged`
/// for tracked `paths`, so git stops (or resumes) reporting their changes.
pub fn set_assume_unchanged(
    git: Arc<GitBinary>,
    workdir: &Path,
    paths: &[String],
    assume: bool,
) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut stdin: Vec<u8> = Vec::new();
    for path in paths {
        stdin.extend_from_slice(path.as_bytes());
        stdin.push(0);
    }
    GitCommand::new(git)
        .args([
            "update-index",
            if assume {
                "--assume-unchanged"
            } else {
                "--no-assume-unchanged"
            },
            "-z",
            "--stdin",
        ])
        .current_dir(workdir)
        .stdin(stdin)
        .run()?;
    Ok(())
}

/// Paths marked assume-unchanged (`ls-files -v`: a lower-case tag).
pub fn assume_unchanged_paths(git: Arc<GitBinary>, workdir: &Path) -> Result<Vec<String>> {
    let out = GitCommand::new(git)
        .args(["ls-files", "-v", "-z"])
        .current_dir(workdir)
        .run()?;
    Ok(out
        .stdout
        .split(|&b| b == 0)
        .filter_map(|entry| {
            let (tag, path) = (entry.first()?, entry.get(2..)?);
            tag.is_ascii_lowercase()
                .then(|| String::from_utf8_lossy(path).into_owned())
        })
        .collect())
}

/// `git add -- <paths>` (the tutorial repository's README).
pub fn add_paths(git: Arc<GitBinary>, workdir: &Path, paths: &[&str]) -> Result<()> {
    let mut args = vec!["add", "--"];
    args.extend_from_slice(paths);
    GitCommand::new(git).args(args).current_dir(workdir).run()?;
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
    // in-process (a `git rev-parse` spawn otherwise)
    if let Some(id) = crate::handle::open(workdir)
        .ok()
        .and_then(|repo| repo.head_id().ok().map(|id| id.detach()))
    {
        return Ok(id.to_string());
    }
    let out = GitCommand::new(git)
        .args(["rev-parse", "HEAD"])
        .current_dir(workdir)
        .run()?;
    Ok(out.stdout_string()?.trim().to_string())
}

/// GHD `mergeTrailers`: `git interpret-trailers --no-divider --trailer k=v …`
/// appends the trailers to a commit message (folding into an existing
/// trailer block when there is one).
pub fn merge_trailers(
    git: Arc<GitBinary>,
    workdir: &Path,
    message: &str,
    trailers: &[(String, String)],
) -> Result<String> {
    if trailers.is_empty() {
        return Ok(message.to_string());
    }
    let mut cmd = GitCommand::new(git)
        .args(["interpret-trailers", "--no-divider"])
        .current_dir(workdir);
    for (token, value) in trailers {
        cmd = cmd.arg("--trailer").arg(format!("{token}={value}"));
    }
    let out = cmd.stdin(message.as_bytes().to_vec()).run()?;
    out.stdout_string()
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
///
/// With `clean_submodules` (Corvane, flag `discard-submodule-changes`), a
/// submodule entry with changes inside also has its modified files checked
/// out and its untracked (not ignored) files moved to the Trash, so the entry
/// goes away; GHD leaves such a submodule dirty.
pub fn discard_changes(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
    move_to_trash: bool,
    clean_submodules: bool,
) -> Result<()> {
    let mut tracked: Vec<&str> = Vec::new();
    if clean_submodules {
        for file in files {
            if let Some(sub) = file.status.submodule_status {
                discard_inside_submodule(
                    git.clone(),
                    &workdir.join(&file.path),
                    sub,
                    move_to_trash,
                )?;
            }
        }
    }
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

/// Check out a submodule's modified files and trash its untracked ones.
fn discard_inside_submodule(
    git: Arc<GitBinary>,
    submodule: &Path,
    status: corvane_models::SubmoduleStatus,
    move_to_trash: bool,
) -> Result<()> {
    if !submodule.is_dir() {
        return Ok(());
    }
    if status.modified_changes {
        GitCommand::new(git.clone())
            .args(["checkout", "-f", "-q", "--", "."])
            .current_dir(submodule)
            .run()?;
    }
    if status.untracked_changes {
        let out = GitCommand::new(git)
            .args([
                "ls-files",
                "--others",
                "--exclude-standard",
                "--directory",
                "-z",
            ])
            .current_dir(submodule)
            .run()?;
        let text = String::from_utf8_lossy(&out.stdout);
        for path in text.split('\0').filter(|p| !p.is_empty()) {
            let full = submodule.join(path.trim_end_matches('/'));
            if !move_to_trash || trash::delete(&full).is_err() {
                let _ = std::fs::remove_file(&full).or_else(|_| std::fs::remove_dir_all(&full));
            }
        }
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
                f.selection = corvane_models::DiffSelection::none();
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
    fn assume_unchanged_round_trip() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "one\n").unwrap();
        std::fs::write(path.join("b c.txt"), "two\n").unwrap();
        let status = crate::get_status(git.clone(), path, None).unwrap();
        stage_files(git.clone(), path, &status.files).unwrap();
        commit(git.clone(), path, "init\n", &CommitOptions::default()).unwrap();
        std::fs::write(path.join("a.txt"), "dirty\n").unwrap();
        std::fs::write(path.join("b c.txt"), "dirty\n").unwrap();
        let both = vec!["a.txt".to_string(), "b c.txt".to_string()];
        set_assume_unchanged(git.clone(), path, &both, true).unwrap();
        assert_eq!(assume_unchanged_paths(git.clone(), path).unwrap(), both);
        let status = crate::get_status(git.clone(), path, None).unwrap();
        assert!(status.files.is_empty());
        set_assume_unchanged(git.clone(), path, &both, false).unwrap();
        assert!(
            assume_unchanged_paths(git.clone(), path)
                .unwrap()
                .is_empty()
        );
        let status = crate::get_status(git, path, None).unwrap();
        assert_eq!(status.files.len(), 2);
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
        discard_changes(git.clone(), path, &status.files, false, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "one\n"
        );
        assert!(!path.join("new.txt").exists());
        let status = crate::get_status(git, path, None).unwrap();
        assert!(status.files.is_empty());
    }

    #[test]
    fn discard_cleans_inside_a_submodule() {
        let (sub_dir, git) = repo();
        let git_in = |dir: &Path, args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(["-c", "protocol.file.allow=always"])
                    .args(args)
                    .current_dir(dir)
                    .status()
                    .unwrap()
                    .success()
            )
        };
        std::fs::write(sub_dir.path().join("lib.txt"), "lib\n").unwrap();
        git_in(sub_dir.path(), &["add", "."]);
        git_in(sub_dir.path(), &["commit", "-q", "-m", "lib"]);
        let (dir, _) = repo();
        let path = dir.path();
        let url = sub_dir.path().to_string_lossy().into_owned();
        git_in(path, &["submodule", "add", "-q", &url, "sub"]);
        git_in(path, &["commit", "-q", "-m", "add sub"]);
        let sub = path.join("sub");
        std::fs::write(sub.join("lib.txt"), "dirty\n").unwrap();
        std::fs::write(sub.join("junk.txt"), "x\n").unwrap();
        std::fs::create_dir(sub.join("junkdir")).unwrap();
        std::fs::write(sub.join("junkdir/more.txt"), "y\n").unwrap();
        let status = crate::get_status(git.clone(), path, None).unwrap();
        assert_eq!(status.files.len(), 1);
        let hidden = crate::get_status_with(
            git.clone(),
            path,
            None,
            crate::StatusOptions {
                ignore_submodules: crate::IgnoreSubmodules::Dirty,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(hidden.files.is_empty());
        // GHD behaviour: the submodule stays dirty
        discard_changes(git.clone(), path, &status.files, false, false).unwrap();
        assert!(sub.join("junk.txt").exists());
        discard_changes(git.clone(), path, &status.files, false, true).unwrap();
        assert_eq!(
            std::fs::read_to_string(sub.join("lib.txt")).unwrap(),
            "lib\n"
        );
        assert!(!sub.join("junk.txt").exists());
        assert!(!sub.join("junkdir").exists());
        let status = crate::get_status(git, path, None).unwrap();
        assert!(status.files.is_empty());
    }
}
