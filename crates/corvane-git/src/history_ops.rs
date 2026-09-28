//! History operations - GHD `lib/git/{revert,reset,checkout,tag}.ts`.

use std::path::Path;
use std::sync::Arc;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// `revertCommit`: `git revert [-m 1] <sha>` (mainline 1 for merge commits).
pub fn revert_commit(git: Arc<GitBinary>, workdir: &Path, sha: &str, is_merge: bool) -> Result<()> {
    let mut cmd = GitCommand::new(git).args(["revert"]).current_dir(workdir);
    if is_merge {
        cmd = cmd.args(["-m", "1"]);
    }
    cmd.arg(sha).run()?;
    Ok(())
}

/// `GitResetMode`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetMode {
    Hard,
    Mixed,
    Soft,
}

/// `reset(repository, mode, ref)`
pub fn reset_to(git: Arc<GitBinary>, workdir: &Path, mode: ResetMode, sha: &str) -> Result<()> {
    let mut cmd = GitCommand::new(git).args(["reset"]).current_dir(workdir);
    match mode {
        ResetMode::Hard => cmd = cmd.arg("--hard"),
        ResetMode::Mixed => {}
        ResetMode::Soft => cmd = cmd.arg("--soft"),
    }
    cmd.arg(sha).run()?;
    Ok(())
}

/// `checkoutCommit`: detached HEAD at `sha`.
pub fn checkout_commit(git: Arc<GitBinary>, workdir: &Path, sha: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["checkout", sha])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `createTag`: annotated tag with an empty message, as GHD creates them.
pub fn create_tag(git: Arc<GitBinary>, workdir: &Path, name: &str, sha: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["tag", "-a", "-m", "", name, sha])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `deleteTag`
pub fn delete_tag(git: Arc<GitBinary>, workdir: &Path, name: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["tag", "-d", name])
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
        std::fs::write(dir.path().join("a.txt"), "two\n").unwrap();
        run(&["commit", "-q", "-am", "second"]);
        (dir, Arc::new(crate::find_git().unwrap()))
    }

    #[test]
    fn revert_reset_checkout_tag() {
        let (dir, git) = repo();
        let path = dir.path();
        let commits = crate::get_commits(path, "HEAD", 0, 10).unwrap();
        let (second, first) = (&commits[0], &commits[1]);

        create_tag(git.clone(), path, "v1", &first.sha).unwrap();
        let tagged = crate::get_commits(path, "HEAD", 0, 10).unwrap();
        assert_eq!(tagged[1].tags, vec!["v1".to_string()]);
        delete_tag(git.clone(), path, "v1").unwrap();

        revert_commit(git.clone(), path, &second.sha, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "one\n"
        );
        assert_eq!(crate::get_commits(path, "HEAD", 0, 10).unwrap().len(), 3);

        reset_to(git.clone(), path, ResetMode::Mixed, &first.sha).unwrap();
        assert_eq!(crate::head_sha(git.clone(), path).unwrap(), first.sha);

        checkout_commit(git.clone(), path, &second.sha).unwrap();
        let info = crate::open_repository(path).unwrap();
        assert!(matches!(info.tip, corvane_models::Tip::Detached { .. }));
    }
}
