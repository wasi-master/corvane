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

/// Corvane addition (flag `815`): `git revert --no-commit` over `shas`, in
/// the order given (newest first reverts cleanly), leaving the combined
/// inverse staged for the user to commit. `-m 1` is passed when any of them
/// is a merge (git accepts it for ordinary commits too). Meant for a clean
/// working tree: when a revert fails (a conflict), the index and working
/// tree are reset to `HEAD` (never moved by `--no-commit`) and the sequencer
/// state is dropped, so nothing is left half-reverted.
pub fn revert_commits_no_commit(
    git: Arc<GitBinary>,
    workdir: &Path,
    shas: &[String],
    any_merge: bool,
) -> Result<()> {
    let mut cmd = GitCommand::new(git.clone())
        .args(["revert", "--no-commit"])
        .current_dir(workdir);
    if any_merge {
        cmd = cmd.args(["-m", "1"]);
    }
    if let Err(err) = cmd.args(shas).run() {
        let _ = GitCommand::new(git.clone())
            .args(["reset", "--merge", "HEAD"])
            .current_dir(workdir)
            .run();
        let _ = GitCommand::new(git)
            .args(["revert", "--quit"])
            .current_dir(workdir)
            .run();
        return Err(err);
    }
    Ok(())
}

/// Corvane addition (flag `820`): `git cherry-pick --no-commit` of `shas`
/// (oldest first) onto the current branch, leaving their changes staged.
/// Like [`revert_commits_no_commit`]: meant for a clean working tree, and a
/// failed pick (a conflict) resets the index and working tree to `HEAD` and
/// drops the sequencer state.
pub fn cherry_pick_no_commit(
    git: Arc<GitBinary>,
    workdir: &Path,
    shas: &[String],
    any_merge: bool,
) -> Result<()> {
    let mut cmd = GitCommand::new(git.clone())
        .args(["cherry-pick", "--no-commit"])
        .current_dir(workdir);
    if any_merge {
        cmd = cmd.args(["-m", "1"]);
    }
    if let Err(err) = cmd.args(shas).run() {
        let _ = GitCommand::new(git.clone())
            .args(["reset", "--merge", "HEAD"])
            .current_dir(workdir)
            .run();
        let _ = GitCommand::new(git)
            .args(["cherry-pick", "--quit"])
            .current_dir(workdir)
            .run();
        return Err(err);
    }
    Ok(())
}

/// Corvane addition (flag `821`): one patch per commit of `shas` (oldest
/// first) in `dir`, `git format-patch -1 <sha> -o <dir>`, numbered in that
/// order. Returns the files written.
pub fn format_patches(
    git: Arc<GitBinary>,
    workdir: &Path,
    shas: &[String],
    dir: &Path,
) -> Result<Vec<std::path::PathBuf>> {
    let mut written = Vec::new();
    for (i, sha) in shas.iter().enumerate() {
        let out = GitCommand::new(git.clone())
            .args(["format-patch", "-1", "--no-signature", "-o"])
            .arg(dir.to_string_lossy().as_ref())
            .arg(format!("--start-number={}", i + 1))
            .arg(sha.as_str())
            .current_dir(workdir)
            .run()?;
        written.extend(
            out.stdout_string()?
                .lines()
                .filter(|l| !l.is_empty())
                .map(std::path::PathBuf::from),
        );
    }
    Ok(written)
}

/// Corvane addition (flag `814`): undo one file's changes from `sha` in the
/// working tree - the file's diff against the first parent (the empty tree
/// for a root commit), `-M` so a rename goes back to `old_path`, applied in
/// reverse with `git apply -R`. git checks the whole patch before writing,
/// so a working file whose lines no longer match is left untouched and the
/// error says which hunk failed; local changes elsewhere in the file stay.
/// Nothing is staged or committed.
pub fn revert_file_in_commit(
    git: Arc<GitBinary>,
    workdir: &Path,
    sha: &str,
    path: &str,
    old_path: Option<&str>,
) -> Result<()> {
    let diff = |base: &str| {
        let mut cmd = GitCommand::new(git.clone())
            .args(["diff", "--binary", "--no-color", "--no-ext-diff", "-M"])
            .args([base, sha, "--", path])
            .current_dir(workdir);
        if let Some(old) = old_path {
            cmd = cmd.arg(old);
        }
        cmd.run()
    };
    let patch = match diff(&format!("{sha}^")) {
        Ok(out) => out.stdout,
        Err(err) if crate::log::is_bad_revision(&err) => diff(crate::log::NULL_TREE_SHA)?.stdout,
        Err(err) => return Err(err),
    };
    if patch.is_empty() {
        return Ok(());
    }
    GitCommand::new(git)
        .args(["apply", "-R", "--binary", "-"])
        .stdin(patch)
        .current_dir(workdir)
        .run()?;
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
/// A non-empty `message` (flag `823`) is kept as typed, `#` lines included.
pub fn create_tag(
    git: Arc<GitBinary>,
    workdir: &Path,
    name: &str,
    sha: &str,
    message: &str,
) -> Result<()> {
    let mut cmd = GitCommand::new(git)
        .args(["tag", "-a", "-m", message])
        .current_dir(workdir);
    if !message.is_empty() {
        cmd = cmd.arg("--cleanup=whitespace");
    }
    cmd.args([name, sha]).run()?;
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
        // the code under test commits too; a CI runner has no identity
        run(&["config", "user.name", "T"]);
        run(&["config", "user.email", "t@example.com"]);
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

        create_tag(git.clone(), path, "v1", &first.sha, "").unwrap();
        let tagged = crate::get_commits(path, "HEAD", 0, 10).unwrap();
        assert_eq!(tagged[1].tags, vec!["v1".to_string()]);
        delete_tag(git.clone(), path, "v1").unwrap();

        create_tag(git.clone(), path, "v2", &first.sha, "Release\n\n#12 fixed").unwrap();
        let out = GitCommand::new(git.clone())
            .args(["tag", "-l", "--format=%(contents)", "v2"])
            .current_dir(path)
            .run()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim_end(),
            "Release\n\n#12 fixed"
        );
        delete_tag(git.clone(), path, "v2").unwrap();

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

    #[test]
    fn cherry_pick_without_committing_and_format_patch() {
        let (dir, git) = repo();
        let path = dir.path();
        let run = |args: &[&str]| {
            GitCommand::new(git.clone())
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .run()
                .unwrap();
        };
        run(&["checkout", "-q", "-b", "side", "HEAD~1"]);
        std::fs::write(path.join("b.txt"), "b\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "side b"]);
        std::fs::write(path.join("c.txt"), "c\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "side c"]);
        let side = crate::get_commits(path, "HEAD", 0, 10).unwrap();
        let picks = vec![side[1].sha.clone(), side[0].sha.clone()];
        let out = tempfile::tempdir().unwrap();
        let files = format_patches(git.clone(), path, &picks, out.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].to_string_lossy().contains("0001-side-b"));
        assert!(files[1].to_string_lossy().contains("0002-side-c"));
        run(&["checkout", "-q", "main"]);
        cherry_pick_no_commit(git.clone(), path, &picks, false).unwrap();
        assert!(path.join("b.txt").exists() && path.join("c.txt").exists());
        assert_eq!(crate::get_commits(path, "HEAD", 0, 10).unwrap().len(), 2);
        // a conflicting pick leaves nothing behind
        run(&["reset", "-q", "--hard"]);
        run(&["checkout", "-q", "side"]);
        std::fs::write(path.join("a.txt"), "side\n").unwrap();
        run(&["commit", "-q", "-am", "side a"]);
        let sha = crate::head_sha(git.clone(), path).unwrap();
        run(&["checkout", "-q", "main"]);
        assert!(cherry_pick_no_commit(git.clone(), path, &[sha], false).is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "two\n"
        );
        assert!(!crate::cherry_pick_head_found(path));
    }

    #[test]
    fn revert_one_file_of_a_commit() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "three\n").unwrap();
        std::fs::write(path.join("b.txt"), "b\n").unwrap();
        let git_run = |args: &[&str]| {
            GitCommand::new(git.clone())
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .run()
                .unwrap();
        };
        git_run(&["add", "."]);
        git_run(&["commit", "-q", "-m", "third"]);
        let commits = crate::get_commits(path, "HEAD", 0, 10).unwrap();
        // only a.txt goes back; b.txt (added by the same commit) stays
        revert_file_in_commit(git.clone(), path, &commits[0].sha, "a.txt", None).unwrap();
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "two\n"
        );
        assert!(path.join("b.txt").exists());
        // an older commit's change no longer matches the working file: refused, untouched
        std::fs::write(path.join("a.txt"), "local\n").unwrap();
        assert!(revert_file_in_commit(git.clone(), path, &commits[1].sha, "a.txt", None).is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "local\n"
        );
        // a file the commit added goes away
        revert_file_in_commit(git.clone(), path, &commits[0].sha, "b.txt", None).unwrap();
        assert!(!path.join("b.txt").exists());
    }

    #[test]
    fn revert_several_without_committing() {
        let (dir, git) = repo();
        let path = dir.path();
        std::fs::write(path.join("a.txt"), "three\n").unwrap();
        std::fs::write(path.join("b.txt"), "b\n").unwrap();
        let git_run = |args: &[&str]| {
            GitCommand::new(git.clone())
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .run()
                .unwrap();
        };
        git_run(&["add", "."]);
        git_run(&["commit", "-q", "-m", "third"]);
        let commits = crate::get_commits(path, "HEAD", 0, 10).unwrap();
        let head = commits[0].sha.clone();
        let newest_first = vec![commits[0].sha.clone(), commits[1].sha.clone()];

        // oldest first conflicts on a.txt: everything is rolled back
        let oldest_first: Vec<String> = newest_first.iter().rev().cloned().collect();
        assert!(revert_commits_no_commit(git.clone(), path, &oldest_first, false).is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "three\n"
        );
        assert!(path.join("b.txt").exists());
        assert!(!path.join(".git/sequencer").exists());

        revert_commits_no_commit(git.clone(), path, &newest_first, false).unwrap();
        assert_eq!(crate::head_sha(git.clone(), path).unwrap(), head);
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "one\n"
        );
        assert!(!path.join("b.txt").exists());
    }
}
