//! Network operations - GHD `lib/git/{fetch,pull,push,remote,lfs}.ts` and
//! `lib/progress/{git,fetch,pull,push}.ts`.
//!
//! Authentication: every remote command gets `GIT_ASKPASS` pointing at the
//! Corvane binary itself (see `crates/corvane/src/askpass.rs`), which answers
//! git's username/password prompts from the macOS Keychain. GHD runs a
//! credential-helper trampoline over a socket instead; the askpass helper is
//! simpler and keeps tokens out of the environment. SSH remotes are left to
//! the user's ssh-agent, as in GHD.

use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use corvane_models::Remote;

use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::paths::git_dir;
use crate::process::GitCommand;

// ---------------------------------------------------------------------------
// progress
// ---------------------------------------------------------------------------

/// GHD `IGitProgressInfo`: one parsed `--progress` line.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressLine {
    pub title: String,
    pub value: u64,
    pub total: Option<u64>,
    pub done: bool,
    pub text: String,
}

/// GHD `parse` in `lib/progress/git.ts`:
/// `remote: Compressing objects:  14% (159/1133)` → title/value/total.
pub fn parse_progress_line(line: &str) -> Option<ProgressLine> {
    let title_len = line.rfind(": ")?;
    if title_len == 0 {
        return None;
    }
    let title = &line[..title_len];
    let rest = line[title_len + 2..].trim();
    if rest.is_empty() {
        return None;
    }
    let mut parts = rest.split(", ");
    let first = parts.next()?;
    let (value, total) = if first.chars().all(|c| c.is_ascii_digit()) {
        (first.parse::<u64>().ok()?, None)
    } else {
        // "14% (159/1133)"
        let (_, rest) = first.split_once("% (")?;
        let rest = rest.strip_suffix(')')?;
        let (value, total) = rest.split_once('/')?;
        (value.parse::<u64>().ok()?, Some(total.parse::<u64>().ok()?))
    };
    let done = parts.any(|p| p == "done.");
    Some(ProgressLine {
        title: title.to_string(),
        value,
        total,
        done,
        text: line.to_string(),
    })
}

/// GHD `GitProgressParser`: weighted steps → overall 0..1.
pub struct ProgressParser {
    steps: Vec<(&'static str, f32)>,
    step_index: usize,
    last_percent: f32,
}

impl ProgressParser {
    pub fn new(steps: &[(&'static str, f32)]) -> Self {
        let total: f32 = steps.iter().map(|(_, w)| w).sum();
        Self {
            steps: steps.iter().map(|(t, w)| (*t, w / total)).collect(),
            step_index: 0,
            last_percent: 0.,
        }
    }

    pub fn fetch() -> Self {
        Self::new(&[
            ("remote: Compressing objects", 0.1),
            ("Receiving objects", 0.7),
            ("Resolving deltas", 0.2),
        ])
    }

    pub fn pull() -> Self {
        Self::new(&[
            ("remote: Compressing objects", 0.1),
            ("Receiving objects", 0.7),
            ("Resolving deltas", 0.15),
            ("Checking out files", 0.15),
        ])
    }

    pub fn push() -> Self {
        Self::new(&[
            ("Compressing objects", 0.2),
            ("Writing objects", 0.7),
            ("remote: Resolving deltas", 0.1),
        ])
    }

    /// Returns `(percent, description)`; `None` for lines that are not
    /// progress of a known step (they are still useful as context text).
    pub fn parse(&mut self, line: &str) -> Option<(f32, String)> {
        let progress = parse_progress_line(line)?;
        let mut percent = 0.;
        for (i, (title, weight)) in self.steps.iter().enumerate() {
            if i >= self.step_index && progress.title == *title {
                if let Some(total) = progress.total.filter(|t| *t > 0) {
                    percent += weight * (progress.value as f32 / total as f32);
                }
                self.step_index = i;
                self.last_percent = percent;
                return Some((percent, progress.text));
            }
            percent += weight;
        }
        None
    }

    pub fn last_percent(&self) -> f32 {
        self.last_percent
    }
}

/// `(percent 0..1, description)` reported while a remote operation runs.
pub type ProgressFn<'a> = &'a mut dyn FnMut(f32, String);

// ---------------------------------------------------------------------------
// errors
// ---------------------------------------------------------------------------

/// The remote-operation failures Corvane reacts to (dugite's `GitError`s).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteFailure {
    /// `! [rejected] … (fetch first)` / `non-fast-forward`.
    PushNotFastForward,
    /// HTTPS authentication failed or a username could not be read.
    AuthenticationFailed,
    /// `remote: Repository not found.`
    RepositoryNotFound,
    /// `remote: Permission to … denied`
    PermissionDenied,
    /// Protected branch / push rejected by a remote hook.
    ProtectedBranch,
    /// `GITHUB PUSH PROTECTION`: secret scanning blocked the push.
    PushWithSecretDetected,
    /// The token lacks the `workflow` scope for a workflow file change.
    MissingWorkflowScope,
    /// An organization enforces SAML SSO; the token must be re-authorized.
    SamlReauthRequired,
    Other,
}

/// Classify a failed remote command from its stderr.
pub fn classify_remote_failure(stderr: &str) -> RemoteFailure {
    let s = stderr;
    if s.contains("GITHUB PUSH PROTECTION") && s.contains("Push cannot contain secrets") {
        return RemoteFailure::PushWithSecretDetected;
    }
    if s.contains("without `workflow` scope") {
        return RemoteFailure::MissingWorkflowScope;
    }
    if s.contains("organization has enabled or enforced SAML SSO") {
        return RemoteFailure::SamlReauthRequired;
    }
    if s.contains("Authentication failed for")
        || s.contains("could not read Username for")
        || s.contains("could not read Password for")
        || s.contains("Permission denied (publickey")
        || s.contains("Invalid username or password")
    {
        return RemoteFailure::AuthenticationFailed;
    }
    if s.contains("Repository not found") || s.contains("repository not found") {
        return RemoteFailure::RepositoryNotFound;
    }
    if s.contains("Permission to") && s.contains("denied") {
        return RemoteFailure::PermissionDenied;
    }
    if s.contains("protected branch") || s.contains("GH006") {
        return RemoteFailure::ProtectedBranch;
    }
    if s.contains("[rejected]")
        && (s.contains("fetch first")
            || s.contains("non-fast-forward")
            || s.contains("stale info")
            || s.contains("remote contains work"))
    {
        return RemoteFailure::PushNotFastForward;
    }
    RemoteFailure::Other
}

pub fn remote_failure(err: &GitError) -> RemoteFailure {
    match err {
        GitError::Failed { stderr, .. } => classify_remote_failure(stderr),
        _ => RemoteFailure::Other,
    }
}

// ---------------------------------------------------------------------------
// askpass environment
// ---------------------------------------------------------------------------

/// Environment for commands that may need credentials: `GIT_ASKPASS` is this
/// binary in askpass mode; `logins` maps hosts to the account to use
/// (`github.com=octocat;ghe.corp=me`).
pub struct AskpassEnv {
    pub program: std::path::PathBuf,
    pub logins: String,
}

impl AskpassEnv {
    pub fn current_exe(logins: String) -> Option<Self> {
        std::env::current_exe()
            .ok()
            .map(|program| Self { program, logins })
    }

    fn apply(&self, cmd: GitCommand) -> GitCommand {
        cmd.env("GIT_ASKPASS", &self.program)
            .env("CORVANE_ASKPASS", "1")
            .env("CORVANE_ASKPASS_LOGINS", &self.logins)
            // Older gits only consult GIT_ASKPASS when SSH_ASKPASS is also
            // unset; make sure the user's shell setup does not interfere.
            .env_remove("SSH_ASKPASS")
    }
}

fn remote_command(git: Arc<GitBinary>, workdir: &Path, askpass: Option<&AskpassEnv>) -> GitCommand {
    let cmd = GitCommand::new(git).current_dir(workdir);
    match askpass {
        Some(a) => a.apply(cmd),
        None => cmd,
    }
}

// ---------------------------------------------------------------------------
// remotes
// ---------------------------------------------------------------------------

/// GHD `getRemotes` via the CLI (`remote -v`, fetch URLs), sorted by name.
pub fn get_remotes(git: Arc<GitBinary>, workdir: &Path) -> Result<Vec<Remote>> {
    let out = GitCommand::new(git)
        .args(["remote", "-v"])
        .current_dir(workdir)
        .run()?;
    let text = out.stdout_string()?;
    let mut remotes: Vec<Remote> = text
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once('\t')?;
            let url = rest.strip_suffix(" (fetch)")?;
            Some(Remote {
                name: name.to_string(),
                url: url.to_string(),
            })
        })
        .collect();
    remotes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(remotes)
}

/// GHD `findDefaultRemote`: `origin`, else the first remote.
pub fn find_default_remote(remotes: &[Remote]) -> Option<&Remote> {
    remotes
        .iter()
        .find(|r| r.name == "origin")
        .or_else(|| remotes.first())
}

pub fn add_remote(git: Arc<GitBinary>, workdir: &Path, name: &str, url: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["remote", "add", name, url])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

pub fn set_remote_url(git: Arc<GitBinary>, workdir: &Path, name: &str, url: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["remote", "set-url", name, url])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

pub fn remove_remote(git: Arc<GitBinary>, workdir: &Path, name: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["remote", "remove", name])
        .current_dir(workdir)
        .allow_exit_code(2)
        .allow_exit_code(128)
        .run()?;
    Ok(())
}

/// GHD `updateRemoteHEAD`: `remote set-head -a <remote>` (best effort).
pub fn update_remote_head(
    git: Arc<GitBinary>,
    workdir: &Path,
    remote: &str,
    askpass: Option<&AskpassEnv>,
) -> Result<()> {
    remote_command(git, workdir, askpass)
        .args(["remote", "set-head", "-a", remote])
        .allow_exit_code(1)
        .allow_exit_code(128)
        .run()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// fetch / pull / push
// ---------------------------------------------------------------------------

/// GHD `fetch`: `fetch --progress --prune --recurse-submodules=on-demand <remote>`.
pub fn fetch(
    git: Arc<GitBinary>,
    workdir: &Path,
    remote: &str,
    askpass: Option<&AskpassEnv>,
    on_progress: ProgressFn<'_>,
) -> Result<()> {
    let mut parser = ProgressParser::fetch();
    remote_command(git, workdir, askpass)
        .args([
            "fetch",
            "--progress",
            "--prune",
            "--recurse-submodules=on-demand",
            remote,
        ])
        .run_streaming(|line| {
            if let Some((percent, text)) = parser.parse(line) {
                on_progress(percent, text);
            }
        })?;
    Ok(())
}

/// GHD `fetchRefspec`
pub fn fetch_refspec(
    git: Arc<GitBinary>,
    workdir: &Path,
    remote: &str,
    refspec: &str,
    askpass: Option<&AskpassEnv>,
) -> Result<()> {
    remote_command(git, workdir, askpass)
        .args(["fetch", remote, refspec])
        .allow_exit_code(128)
        .run()?;
    Ok(())
}

/// `pull.rebase` is set to anything (GHD `pullWithRebase`).
pub fn pull_with_rebase(git: Arc<GitBinary>, workdir: &Path) -> bool {
    config_value(git, workdir, "pull.rebase").is_some_and(|v| {
        !matches!(
            v.to_ascii_lowercase().as_str(),
            "false" | "no" | "off" | "0"
        )
    })
}

pub fn config_value(git: Arc<GitBinary>, workdir: &Path, key: &str) -> Option<String> {
    GitCommand::new(git)
        .args(["config", "--get", key])
        .current_dir(workdir)
        .allow_exit_code(1)
        .run()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| o.stdout_string().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// GHD `pull`: `pull [--ff] --recurse-submodules --progress <remote>`.
pub fn pull(
    git: Arc<GitBinary>,
    workdir: &Path,
    remote: &str,
    askpass: Option<&AskpassEnv>,
    on_progress: ProgressFn<'_>,
) -> Result<()> {
    let mut parser = ProgressParser::pull();
    let mut args: Vec<&str> = vec!["-c", "rebase.backend=merge", "pull"];
    let pull_ff = config_value(git.clone(), workdir, "pull.ff");
    if pull_ff.is_none() {
        args.push("--ff");
    }
    args.extend(["--recurse-submodules", "--progress", remote]);
    remote_command(git, workdir, askpass)
        .args(&args)
        .env("GIT_EDITOR", ":")
        .run_streaming(|line| {
            if let Some((percent, text)) = parser.parse(line) {
                on_progress(percent, text);
            }
        })?;
    Ok(())
}

/// GHD `push`: `push <remote> <local>[:<remote branch>] [tags…]
/// [--set-upstream | --force-with-lease] --progress`.
#[allow(clippy::too_many_arguments)]
pub fn push(
    git: Arc<GitBinary>,
    workdir: &Path,
    remote: &str,
    local_branch: &str,
    remote_branch: Option<&str>,
    tags: &[String],
    force_with_lease: bool,
    askpass: Option<&AskpassEnv>,
    on_progress: ProgressFn<'_>,
) -> Result<()> {
    let mut parser = ProgressParser::push();
    let refspec = match remote_branch {
        Some(rb) => format!("{local_branch}:{rb}"),
        None => local_branch.to_string(),
    };
    let mut args: Vec<String> = vec!["push".into(), remote.into(), refspec];
    args.extend(tags.iter().cloned());
    if remote_branch.is_none() {
        args.push("--set-upstream".into());
    } else if force_with_lease {
        args.push("--force-with-lease".into());
    }
    args.push("--progress".into());
    remote_command(git, workdir, askpass)
        .args(&args)
        .run_streaming(|line| {
            if let Some((percent, text)) = parser.parse(line) {
                on_progress(percent, text);
            }
        })?;
    Ok(())
}

/// GHD `getBranchesDifferingFromUpstream` + `fastForwardBranches`: local
/// branches that are strictly behind their upstream get fast-forwarded with
/// `fetch . --show-forced-updates --no-write-fetch-head --stdin`.
pub fn fast_forward_branches(git: Arc<GitBinary>, workdir: &Path) -> Result<usize> {
    // the checked-out branch is left to `pull`; git refuses to update it here
    let head = GitCommand::new(git.clone())
        .args(["symbolic-ref", "-q", "HEAD"])
        .current_dir(workdir)
        .allow_exit_code(1)
        .run()
        .ok()
        .and_then(|o| o.stdout_string().ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let out = GitCommand::new(git.clone())
        .args([
            "for-each-ref",
            "--format=%(refname)%00%(upstream)%00%(upstream:trackshort)",
            "refs/heads",
        ])
        .current_dir(workdir)
        .run()?;
    let text = out.stdout_string()?;
    let pairs: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let mut cols = line.split('\0');
            let local = cols.next()?;
            let upstream = cols.next()?;
            let track = cols.next().unwrap_or("");
            (!upstream.is_empty() && track == "<" && local != head)
                .then(|| format!("{upstream}:{local}"))
        })
        .collect();
    if pairs.is_empty() {
        return Ok(0);
    }
    GitCommand::new(git)
        .args([
            "fetch",
            ".",
            "--show-forced-updates",
            "--no-write-fetch-head",
            "--stdin",
        ])
        .env("GIT_REFLOG_ACTION", "pull")
        .current_dir(workdir)
        .stdin(pairs.join("\n").into_bytes())
        .allow_exit_code(1)
        .run()?;
    Ok(pairs.len())
}

/// GHD `updateLastFetched`: mtime of a non-empty `FETCH_HEAD`.
pub fn last_fetched(workdir: &Path) -> Option<SystemTime> {
    let meta = std::fs::metadata(git_dir(workdir).join("FETCH_HEAD")).ok()?;
    (meta.len() > 0).then(|| meta.modified().ok()).flatten()
}

// ---------------------------------------------------------------------------
// LFS
// ---------------------------------------------------------------------------

/// `git lfs` is installed.
pub fn lfs_available(git: Arc<GitBinary>) -> bool {
    GitCommand::new(git).args(["lfs", "version"]).run().is_ok()
}

/// GHD `isUsingLFS`: `lfs track --json` reports tracked patterns.
pub fn is_using_lfs(git: Arc<GitBinary>, workdir: &Path) -> bool {
    let out = GitCommand::new(git)
        .args(["lfs", "track", "--json"])
        .env("GIT_LFS_TRACK_NO_INSTALL_HOOKS", "1")
        .current_dir(workdir)
        .run();
    match out.and_then(|o| o.stdout_string()) {
        Ok(text) => text.contains("\"tracked\": true") || text.contains("\"tracked\":true"),
        Err(_) => false,
    }
}

/// The repository's `pre-push` hook was written by Git LFS.
pub fn lfs_hooks_installed(workdir: &Path) -> bool {
    std::fs::read_to_string(git_dir(workdir).join("hooks/pre-push"))
        .map(|s| s.contains("git lfs") || s.contains("git-lfs"))
        .unwrap_or(false)
}

/// GHD `installLFSHooks(repository, force = true)`
pub fn install_lfs_hooks(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    GitCommand::new(git)
        .args(["lfs", "install", "--force"])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_lines() {
        let p = parse_progress_line("remote: Compressing objects:  14% (159/1133)").unwrap();
        assert_eq!(
            (p.title.as_str(), p.value, p.total),
            ("remote: Compressing objects", 159, Some(1133))
        );
        assert!(!p.done);
        let p = parse_progress_line("Checking out files: 100% (728/728), done.").unwrap();
        assert!(p.done);
        let p = parse_progress_line("remote: Counting objects: 123").unwrap();
        assert_eq!((p.value, p.total), (123, None));
        assert!(parse_progress_line("Everything up-to-date").is_none());
    }

    #[test]
    fn progress_parser_weights_steps() {
        let mut parser = ProgressParser::fetch();
        let (percent, _) = parser.parse("Receiving objects:  50% (5/10)").unwrap();
        assert!((percent - 0.45).abs() < 0.001, "{percent}");
        let (percent, _) = parser.parse("Resolving deltas: 100% (3/3), done.").unwrap();
        assert!((percent - 1.0).abs() < 0.001, "{percent}");
        // earlier steps are ignored once a later one was seen
        assert!(
            parser
                .parse("remote: Compressing objects: 10% (1/10)")
                .is_none()
        );
    }

    #[test]
    fn classifies_failures() {
        assert_eq!(
            classify_remote_failure(
                "fatal: Authentication failed for 'https://github.com/a/b.git/'"
            ),
            RemoteFailure::AuthenticationFailed
        );
        assert_eq!(
            classify_remote_failure(
                " ! [rejected]        main -> main (fetch first)\nerror: failed to push some refs\nhint: Updates were rejected because the remote contains work that you do not have locally."
            ),
            RemoteFailure::PushNotFastForward
        );
        assert_eq!(
            classify_remote_failure(
                "remote: Repository not found.\nfatal: repository 'https://github.com/a/b.git/' not found"
            ),
            RemoteFailure::RepositoryNotFound
        );
        assert_eq!(
            classify_remote_failure("fatal: unable to access"),
            RemoteFailure::Other
        );
    }

    fn run(path: &Path, args: &[&str]) {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .status()
                .unwrap()
                .success(),
            "git {args:?}"
        );
    }

    #[test]
    fn fetch_pull_push_against_a_local_bare_remote() {
        let git = Arc::new(crate::find_git().unwrap());
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        let work = dir.path().join("work");
        let other = dir.path().join("other");
        run(
            dir.path(),
            &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()],
        );
        run(
            dir.path(),
            &["init", "-q", "-b", "main", work.to_str().unwrap()],
        );
        run(&work, &["config", "commit.gpgsign", "false"]);
        std::fs::write(work.join("a.txt"), "one\n").unwrap();
        run(&work, &["add", "."]);
        run(&work, &["commit", "-q", "-m", "first"]);
        add_remote(git.clone(), &work, "origin", bare.to_str().unwrap()).unwrap();
        let remotes = get_remotes(git.clone(), &work).unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(find_default_remote(&remotes).unwrap().name, "origin");
        // publish the branch (sets the upstream)
        let mut seen = 0;
        push(
            git.clone(),
            &work,
            "origin",
            "main",
            None,
            &[],
            false,
            None,
            &mut |_, _| seen += 1,
        )
        .unwrap();
        assert_eq!(
            config_value(git.clone(), &work, "branch.main.remote").as_deref(),
            Some("origin")
        );
        // a tracking branch that is not checked out gets fast-forwarded later
        run(&work, &["branch", "--track", "mirror", "origin/main"]);
        // a second clone pushes a new commit; the first one is then behind
        run(
            dir.path(),
            &[
                "clone",
                "-q",
                bare.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        );
        run(&other, &["config", "commit.gpgsign", "false"]);
        std::fs::write(other.join("b.txt"), "two\n").unwrap();
        run(&other, &["add", "."]);
        run(&other, &["commit", "-q", "-m", "second"]);
        run(&other, &["push", "-q", "origin", "main"]);
        fetch(git.clone(), &work, "origin", None, &mut |_, _| {}).unwrap();
        assert!(last_fetched(&work).is_some());
        let ab = crate::symmetric_ahead_behind(git.clone(), &work, "main", "origin/main")
            .unwrap()
            .unwrap();
        assert_eq!((ab.ahead, ab.behind), (0, 1));
        // fast-forwarding updates `mirror` but leaves the checked-out `main` alone
        assert_eq!(fast_forward_branches(git.clone(), &work).unwrap(), 1);
        let ab = crate::symmetric_ahead_behind(git.clone(), &work, "mirror", "origin/main")
            .unwrap()
            .unwrap();
        assert_eq!((ab.ahead, ab.behind), (0, 0));
        let ab = crate::symmetric_ahead_behind(git.clone(), &work, "main", "origin/main")
            .unwrap()
            .unwrap();
        assert_eq!((ab.ahead, ab.behind), (0, 1));
        pull(git.clone(), &work, "origin", None, &mut |_, _| {}).unwrap();
        assert!(work.join("b.txt").exists());
        // diverge, then a plain push is rejected as non-fast-forward
        std::fs::write(other.join("c.txt"), "three\n").unwrap();
        run(&other, &["add", "."]);
        run(&other, &["commit", "-q", "-m", "third"]);
        run(&other, &["push", "-q", "origin", "main"]);
        std::fs::write(work.join("d.txt"), "four\n").unwrap();
        run(&work, &["add", "."]);
        run(&work, &["commit", "-q", "-m", "fourth"]);
        let err = push(
            git.clone(),
            &work,
            "origin",
            "main",
            Some("main"),
            &[],
            false,
            None,
            &mut |_, _| {},
        )
        .unwrap_err();
        assert_eq!(remote_failure(&err), RemoteFailure::PushNotFastForward);
        // pull merges the remote work in
        pull(git.clone(), &work, "origin", None, &mut |_, _| {}).unwrap();
        assert!(work.join("c.txt").exists());
        push(
            git,
            &work,
            "origin",
            "main",
            Some("main"),
            &[],
            false,
            None,
            &mut |_, _| {},
        )
        .unwrap();
    }
}
