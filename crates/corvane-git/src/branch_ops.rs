//! Branch, merge and stash operations - GHD `lib/git/{branch,checkout,
//! reflog,merge,stash}.ts` and `lib/find-default-branch.ts`.

use std::path::Path;
use std::sync::Arc;

use corvane_models::{Branch, BranchKind, StashEntry};

use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::process::GitCommand;

/// `createBranch`: `git branch [--no-track] <name> [<start point>]`.
pub fn create_branch(
    git: Arc<GitBinary>,
    workdir: &Path,
    name: &str,
    start_point: Option<&str>,
    no_track: bool,
) -> Result<()> {
    let mut cmd = GitCommand::new(git).args(["branch"]).current_dir(workdir);
    if no_track {
        cmd = cmd.arg("--no-track");
    }
    cmd = cmd.arg(name);
    if let Some(start) = start_point {
        cmd = cmd.arg(start);
    }
    cmd.run()?;
    Ok(())
}

/// Text git prints when a checkout would clobber local changes
/// (`isLocalChangesOverwrittenError`).
pub fn is_local_changes_overwritten(err: &GitError) -> bool {
    let text = err.to_string();
    text.contains("would be overwritten by checkout")
        || text.contains("Please commit your changes or stash them before you switch branches")
}

/// `checkoutBranch`: local branches by name; remote branches become a
/// tracking local branch (`checkout -b <short> <remote/short>`).
pub fn checkout_branch(git: Arc<GitBinary>, workdir: &Path, branch: &Branch) -> Result<()> {
    let mut cmd = GitCommand::new(git).args(["checkout"]).current_dir(workdir);
    cmd = match branch.kind {
        BranchKind::Local => cmd.arg(&branch.name).arg("--"),
        BranchKind::Remote => cmd
            .arg("-b")
            .arg(branch.name_without_remote())
            .arg(&branch.name)
            .arg("--"),
    };
    cmd.run()?;
    Ok(())
}

/// `git checkout -b <name>`: creates the branch from HEAD, or renames an
/// unborn branch (GHD's create-branch behaviour on unborn repositories).
pub fn checkout_new_branch(git: Arc<GitBinary>, workdir: &Path, name: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["checkout", "-b", name])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `renameBranch`: `git branch -m <old> <new>`.
pub fn rename_branch(git: Arc<GitBinary>, workdir: &Path, old: &str, new: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["branch", "-m", old, new])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `deleteLocalBranch`: `git branch -D <name>`.
pub fn delete_local_branch(git: Arc<GitBinary>, workdir: &Path, name: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["branch", "-D", name])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `deleteRemoteBranch`: `git push <remote> :<name>`.
pub fn delete_remote_branch(
    git: Arc<GitBinary>,
    workdir: &Path,
    remote: &str,
    name: &str,
) -> Result<()> {
    GitCommand::new(git)
        .args(["push", remote])
        .arg(format!(":{name}"))
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `getRecentBranches`: branch names from the last checkouts in HEAD's reflog.
pub fn recent_branches(git: Arc<GitBinary>, workdir: &Path, limit: usize) -> Result<Vec<String>> {
    let out = GitCommand::new(git)
        .args([
            "log",
            "-g",
            "--no-abbrev-commit",
            "--pretty=oneline",
            "HEAD",
            "-n",
            "2500",
            "--",
        ])
        .current_dir(workdir)
        .allow_exit_code(128)
        .run()?;
    if !out.status.success() {
        // unborn branch: no reflog yet
        return Ok(Vec::new());
    }
    Ok(parse_recent_branches(&out.stdout_string()?, limit))
}

/// Parse `log -g --pretty=oneline` lines: `<sha> checkout: moving from A to B`
/// (and `renamed refs/heads/A to refs/heads/B`, whose source is excluded).
pub fn parse_recent_branches(text: &str, limit: usize) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut excluded: Vec<String> = Vec::new();
    for line in text.lines() {
        // `<sha> HEAD@{n}: checkout: moving from A to B` / `…: Branch: renamed A to B`
        let (renamed, rest) = if let Some(i) = line.find("checkout: moving from ") {
            (false, &line[i + "checkout: moving from ".len()..])
        } else if let Some(i) = line.find("renamed ") {
            (true, &line[i + "renamed ".len()..])
        } else {
            continue;
        };
        let Some((from, to)) = rest.rsplit_once(" to ") else {
            continue;
        };
        let strip = |s: &str| s.strip_prefix("refs/heads/").unwrap_or(s).to_string();
        let (from, to) = (strip(from), strip(to));
        if renamed {
            excluded.push(from);
        }
        if !excluded.contains(&to) && !names.contains(&to) {
            names.push(to);
        }
        if names.len() == limit {
            break;
        }
    }
    names
}

/// `getRemoteHEAD`: the branch `refs/remotes/<remote>/HEAD` points at.
pub fn remote_head(git: Arc<GitBinary>, workdir: &Path, remote: &str) -> Result<Option<String>> {
    let out = GitCommand::new(git)
        .args(["symbolic-ref", "-q"])
        .arg(format!("refs/remotes/{remote}/HEAD"))
        .current_dir(workdir)
        .allow_exit_code(1)
        .allow_exit_code(128)
        .run()?;
    if !out.status.success() {
        return Ok(None);
    }
    let target = out.stdout_string()?.trim().to_string();
    Ok(target
        .strip_prefix(&format!("refs/remotes/{remote}/"))
        .map(|s| s.to_string()))
}

/// `getDefaultBranch`: `init.defaultBranch` from the global config, else `main`.
pub fn configured_default_branch(git: Arc<GitBinary>) -> String {
    GitCommand::new(git)
        .args(["config", "--global", "init.defaultBranch"])
        .allow_exit_code(1)
        .run()
        .ok()
        .and_then(|o| o.stdout_string().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "main".to_string())
}

/// `findDefaultBranch`: prefer a local branch tracking the remote's default,
/// then a local branch named like it, then the remote branch itself.
pub fn find_default_branch<'a>(
    branches: &'a [Branch],
    remote: Option<&str>,
    remote_head: Option<&str>,
    configured: &str,
) -> Option<&'a Branch> {
    let default_name = remote_head.unwrap_or(configured);
    let remote_ref = match (remote, remote_head) {
        (Some(r), Some(h)) => Some(format!("{r}/{h}")),
        _ => None,
    };
    let mut local_hit = None;
    let mut local_tracking_hit: Option<&Branch> = None;
    let mut remote_hit = None;
    for branch in branches {
        match branch.kind {
            BranchKind::Local => {
                if branch.name == default_name {
                    local_hit = Some(branch);
                }
                if let Some(remote_ref) = &remote_ref
                    && branch
                        .upstream
                        .as_deref()
                        .map(|u| u.strip_prefix("refs/remotes/").unwrap_or(u))
                        == Some(remote_ref.as_str())
                    && (local_tracking_hit.is_none() || branch.name == default_name)
                {
                    local_tracking_hit = Some(branch);
                }
            }
            BranchKind::Remote => {
                if remote_ref.as_deref() == Some(branch.name.as_str()) {
                    remote_hit = Some(branch);
                }
            }
        }
    }
    local_tracking_hit.or(local_hit).or(remote_hit)
}

/// Outcome of `git merge` (GHD `MergeResult`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeOutcome {
    Success,
    AlreadyUpToDate,
    /// The merge stopped on conflicts (`MERGE_HEAD` is set).
    Conflicts,
    Failed(String),
}

/// `merge(repository, branch, { squash })`.
pub fn merge_branch(
    git: Arc<GitBinary>,
    workdir: &Path,
    branch: &str,
    squash: bool,
) -> Result<MergeOutcome> {
    let mut cmd = GitCommand::new(git.clone())
        .args(["merge"])
        .current_dir(workdir);
    if squash {
        cmd = cmd.arg("--squash");
    }
    let out = cmd.arg(branch).allow_exit_code(1).run()?;
    if !out.status.success() {
        let git_dir = crate::paths::git_dir(workdir);
        let stdout = out.stdout_string().unwrap_or_default();
        // a squash merge has no MERGE_HEAD: it leaves SQUASH_MSG and conflicted files
        if git_dir.join("MERGE_HEAD").exists()
            || (squash && git_dir.join("SQUASH_MSG").exists())
            || stdout.contains("Automatic merge failed")
        {
            return Ok(MergeOutcome::Conflicts);
        }
        return Ok(MergeOutcome::Failed(out.stderr.trim().to_string()));
    }
    if squash {
        GitCommand::new(git)
            .args(["commit", "--no-edit"])
            .current_dir(workdir)
            .run()?;
    }
    let stdout = out.stdout_string()?;
    Ok(if stdout.trim() == "Already up to date." {
        MergeOutcome::AlreadyUpToDate
    } else {
        MergeOutcome::Success
    })
}

/// `git merge --abort`
pub fn abort_merge(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    GitCommand::new(git)
        .args(["merge", "--abort"])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// Commits in `other` that are not in `base` (`getAheadBehind(base..other)`).
pub fn commits_ahead(git: Arc<GitBinary>, workdir: &Path, base: &str, other: &str) -> Result<u32> {
    let out = GitCommand::new(git)
        .args(["rev-list", "--count"])
        .arg(format!("{base}..{other}"))
        .current_dir(workdir)
        .run()?;
    Ok(out.stdout_string()?.trim().parse().unwrap_or(0))
}

/// GHD `DesktopStashEntryMarker`
pub const DESKTOP_STASH_MARKER: &str = "!!GitHub_Desktop";

pub fn desktop_stash_message(branch: &str) -> String {
    format!("{DESKTOP_STASH_MARKER}<{branch}>")
}

/// `createDesktopStashEntry`: stage untracked files first so they are
/// included, then `stash push -m !!GitHub_Desktop<branch>`. Returns false
/// when there was nothing to stash.
pub fn create_desktop_stash(git: Arc<GitBinary>, workdir: &Path, branch: &str) -> Result<bool> {
    // stage untracked files (only those) so `stash push` picks them up
    let untracked = GitCommand::new(git.clone())
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(workdir)
        .run()?;
    let paths: Vec<u8> = untracked.stdout.clone();
    if paths.iter().any(|b| *b != 0) {
        GitCommand::new(git.clone())
            .args(["update-index", "--add", "-z", "--stdin"])
            .current_dir(workdir)
            .stdin(paths)
            .run()?;
    }
    let out = GitCommand::new(git)
        .args(["stash", "push", "-m", &desktop_stash_message(branch)])
        .current_dir(workdir)
        .allow_exit_code(1)
        .run()?;
    let stdout = out.stdout_string()?;
    if stdout.trim() == "No local changes to save" {
        return Ok(false);
    }
    if !out.status.success() {
        let stderr = out.stderr.clone();
        if stderr.lines().any(|l| l.starts_with("error: ")) {
            return Err(GitError::Failed {
                args: "stash push".into(),
                code: out.status.code(),
                stderr,
            });
        }
    }
    Ok(true)
}

/// `getStashes`: every entry of `refs/stash`; Corvane/GHD entries carry the
/// branch they were made on in their message.
pub fn get_stashes(git: Arc<GitBinary>, workdir: &Path) -> Result<(Vec<StashEntry>, usize)> {
    let out = GitCommand::new(git)
        .args([
            "log",
            "-g",
            "-z",
            "--format=%gD%x00%H%x00%gs%x00%T%x00%P",
            "refs/stash",
            "--",
        ])
        .current_dir(workdir)
        .allow_exit_code(128)
        .run()?;
    if !out.status.success() {
        return Ok((Vec::new(), 0));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut entries = Vec::new();
    let mut total = 0usize;
    for record in text.split('\0').collect::<Vec<_>>().chunks(5) {
        if record.len() < 5 || record[0].trim().is_empty() {
            continue;
        }
        total += 1;
        let message = record[2].trim();
        let branch = message
            .rsplit_once(DESKTOP_STASH_MARKER)
            .and_then(|(_, tail)| tail.strip_prefix('<'))
            .and_then(|t| t.strip_suffix('>'))
            .map(|s| s.to_string());
        entries.push(StashEntry {
            name: record[0].trim().to_string(),
            sha: record[1].trim().to_string(),
            branch,
            message: message.to_string(),
            tree: record[3].trim().to_string(),
            parents: record[4]
                .split_whitespace()
                .map(|s| s.to_string())
                .collect(),
        });
    }
    Ok((entries, total))
}

/// `getStashedFiles`: `stash show <sha> --raw --numstat -z`.
pub fn stashed_files(
    git: Arc<GitBinary>,
    workdir: &Path,
    sha: &str,
) -> Result<corvane_models::ChangesetData> {
    let out = GitCommand::new(git)
        .args([
            "stash",
            "show",
            sha,
            "--raw",
            "--numstat",
            "-z",
            "--format=format:",
            "--no-color",
            "--",
        ])
        .current_dir(workdir)
        .run()?;
    Ok(crate::log::parse_raw_log_with_numstat(&out.stdout, sha))
}

/// `popStashEntry`: `git stash pop --quiet <name>`.
pub fn pop_stash(git: Arc<GitBinary>, workdir: &Path, name: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["stash", "pop", "--quiet", name])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `dropDesktopStashEntry`: `git stash drop <name>`.
pub fn drop_stash(git: Arc<GitBinary>, workdir: &Path, name: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["stash", "drop", name])
        .current_dir(workdir)
        .run()?;
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
        std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "first"]);
        (dir, Arc::new(crate::find_git().unwrap()))
    }

    #[test]
    fn parses_recent_branches() {
        let text = "\
aaaa checkout: moving from main to feature\n\
bbbb Branch: renamed refs/heads/tmp to refs/heads/feature\n\
cccc checkout: moving from feature to tmp\n\
dddd checkout: moving from main to other\n\
eeee commit: something\n";
        assert_eq!(
            parse_recent_branches(text, 5),
            vec!["feature".to_string(), "other".to_string()]
        );
        assert_eq!(parse_recent_branches(text, 1), vec!["feature".to_string()]);
    }

    #[test]
    fn branch_lifecycle_and_merge() {
        let (dir, git) = repo();
        let path = dir.path();
        create_branch(git.clone(), path, "feature", None, false).unwrap();
        let info = crate::open_repository(path).unwrap();
        let feature = info
            .branches
            .iter()
            .find(|b| b.name == "feature")
            .cloned()
            .unwrap();
        assert!(feature.tip_time.is_some());
        checkout_branch(git.clone(), path, &feature).unwrap();
        std::fs::write(path.join("b.txt"), "b\n").unwrap();
        crate::stage_files(
            git.clone(),
            path,
            &crate::get_status(git.clone(), path, None).unwrap().files,
        )
        .unwrap();
        crate::commit(git.clone(), path, "feat\n", &Default::default()).unwrap();
        let main = Branch {
            name: "main".into(),
            kind: BranchKind::Local,
            full_name: "refs/heads/main".into(),
            tip: None,
            upstream: None,
            tip_time: None,
            remote_name: None,
        };
        checkout_branch(git.clone(), path, &main).unwrap();
        assert_eq!(
            recent_branches(git.clone(), path, 5).unwrap(),
            vec!["main", "feature"]
        );
        assert_eq!(
            commits_ahead(git.clone(), path, "main", "feature").unwrap(),
            1
        );
        assert_eq!(
            merge_branch(git.clone(), path, "feature", false).unwrap(),
            MergeOutcome::Success
        );
        assert_eq!(
            merge_branch(git.clone(), path, "feature", false).unwrap(),
            MergeOutcome::AlreadyUpToDate
        );
        rename_branch(git.clone(), path, "feature", "renamed").unwrap();
        delete_local_branch(git.clone(), path, "renamed").unwrap();
        let info = crate::open_repository(path).unwrap();
        assert!(info.branches.iter().all(|b| b.name != "renamed"));
        let configured = configured_default_branch(git.clone());
        assert!(!configured.is_empty());
        let default = find_default_branch(&info.branches, None, None, "main").unwrap();
        assert_eq!(default.name, "main");
    }

    #[test]
    fn desktop_stash_round_trip() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "changed\n").unwrap();
        std::fs::write(path.join("new.txt"), "n\n").unwrap();
        assert!(create_desktop_stash(git.clone(), path, "main").unwrap());
        assert!(
            crate::get_status(git.clone(), path, None)
                .unwrap()
                .files
                .is_empty()
        );
        let (entries, total) = get_stashes(git.clone(), path).unwrap();
        assert_eq!(total, 1);
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        pop_stash(git.clone(), path, &entries[0].name).unwrap();
        let files = crate::get_status(git.clone(), path, None).unwrap().files;
        assert_eq!(files.len(), 2);
        assert!(create_desktop_stash(git.clone(), path, "main").unwrap());
        let (entries, _) = get_stashes(git.clone(), path).unwrap();
        drop_stash(git.clone(), path, &entries[0].name).unwrap();
        assert_eq!(get_stashes(git, path).unwrap().1, 0);
    }

    #[test]
    fn merge_conflict_is_reported() {
        let (dir, git) = repo();
        let path = dir.path();
        create_branch(git.clone(), path, "other", None, false).unwrap();
        std::fs::write(path.join("a.txt"), "main change\n").unwrap();
        crate::commit(git.clone(), path, "main\n", &Default::default()).unwrap_or_default();
        let run = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .status()
                .unwrap()
        };
        run(&["commit", "-q", "-am", "main change"]);
        run(&["checkout", "-q", "other"]);
        std::fs::write(path.join("a.txt"), "other change\n").unwrap();
        run(&["commit", "-q", "-am", "other change"]);
        run(&["checkout", "-q", "main"]);
        assert_eq!(
            merge_branch(git.clone(), path, "other", false).unwrap(),
            MergeOutcome::Conflicts
        );
        abort_merge(git, path).unwrap();
    }

    #[test]
    fn stash_lists_files_and_pops() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "changed\n").unwrap();
        std::fs::write(path.join("new.txt"), "untracked\n").unwrap();
        create_desktop_stash(git.clone(), path, "main").unwrap();
        let (stashes, count) = get_stashes(git.clone(), path).unwrap();
        assert_eq!(count, 1);
        let stash = stashes
            .iter()
            .find(|s| s.branch.as_deref() == Some("main"))
            .unwrap();
        let files = stashed_files(git.clone(), path, &stash.sha).unwrap().files;
        let mut names: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        names.sort();
        assert_eq!(names, ["a.txt", "new.txt"]);
        pop_stash(git.clone(), path, &stash.name).unwrap();
        assert_eq!(get_stashes(git, path).unwrap().1, 0);
        assert!(path.join("new.txt").exists());
    }
}
