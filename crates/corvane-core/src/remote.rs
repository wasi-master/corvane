//! Remote operations - GHD `app-store.ts` `performFetch` / `performPull` /
//! `performPush` / `_publishRepository`, `dispatcher.ts` `confirmOrForcePush`,
//! the `BackgroundFetcher` (every hour, at least 5 minutes apart) and the
//! `RepositoryIndicatorUpdater` (every 15 minutes), plus the Git LFS
//! initialisation prompt (`InitializeLFS`).
//!
//! Deviations (flags): a failed force push keeps the "Force push"
//! recommendation (`258-force-push-kept-on-failure`; GHD clears it first).
//! The background fetch can be off or cover any remote
//! (`244-background-fetch`; GHD: GitHub repositories only).
//! Fetch can prune tags deleted on the remote (`248-fetch-prune-tags`).
//! The LFS check can read `.gitattributes` instead of running
//! `git lfs track` (`905-lfs-detect-by-attributes`).
//! The background fetch can run without progress in the push/pull button,
//! and a push, pull or fetch asked for meanwhile waits for it
//! (`245-push-during-background-fetch`; GHD disables the button).
//! A local branch that is not checked out can be fast-forwarded from its
//! upstream (`857-update-branch-from-upstream`).
//! Repository › Fetch All Repositories fetches every listed repository
//! (`247-fetch-all-repositories`).
//! Indicators refresh right after launch and on opening the repository list
//! (`217-prompt-indicator-refresh`; GHD waits for the 15-minute updater).
//! A pull skips `remote set-head -a` while the remote's HEAD resolves
//! (`251-remote-head-once`; GHD runs it after every pull).
//! Fetch can extend the commit-graph (`249-fetch-writes-commit-graph`).
//! Fetch and pull can leave submodules alone (`250-sync-skips-submodules`).
//! The background fetch can fast-forward a clean branch that is only behind
//! (`246-background-fetch-fast-forwards`).
//! Force push is also recommended after a rewrite outside Corvane
//! (`260-force-push-after-outside-rewrite`).
//! A fetch or pull blocked by a stale remote-tracking ref prunes the remote
//! and retries once (`252-prune-stale-refs-and-retry`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

use corvane_git::{AskpassEnv, RemoteFailure};
use corvane_models::{Account, AheadBehind, Remote, Tip};
use gpui_kit::{App, AsyncApp};
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::persistence::StoreExt;
use crate::state::{Popup, RetryAction};

/// GHD `app-store.ts` progress title after a fetch/pull/push
/// (`Refreshing ${__DARWIN__ ? 'Repository' : 'repository'}`).
const REFRESHING_REPOSITORY: &str = if cfg!(target_os = "macos") {
    "Refreshing Repository"
} else {
    "Refreshing repository"
};

/// GHD `Progress` for the push/pull button.
#[derive(Clone, Debug, PartialEq)]
pub struct PushPullProgress {
    pub kind: PushPullKind,
    pub title: String,
    pub description: Option<String>,
    /// 0..=1
    pub value: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushOutcome {
    /// git pushed.
    Pushed,
    /// git ran and failed (the error dialog is up).
    Failed,
    /// Nothing was pushed: no remote (Publish opened), a fork was offered,
    /// an unborn / detached tip, or another network operation is running.
    NotAttempted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushPullKind {
    Push,
    Pull,
    Fetch,
    Generic,
}

/// Sidebar indicators (`ILocalRepositoryState`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoIndicator {
    pub ahead_behind: Option<AheadBehind>,
    pub changed_files: usize,
    /// The checked-out branch, for `214-repository-list-branch`.
    pub branch: Option<String>,
}

/// GHD `ForcePushBranchState`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForcePushState {
    NotAvailable,
    Available,
    Recommended,
}

const BACKGROUND_FETCH_INTERVAL: Duration = Duration::from_secs(60 * 60);
const BACKGROUND_FETCH_MINIMUM: Duration = Duration::from_secs(5 * 60);
const INDICATOR_REFRESH_INTERVAL: Duration = Duration::from_secs(15 * 60);
const INDICATOR_REFRESH_MINIMUM: Duration = Duration::from_secs(60);

thread_local! {
    /// When [`Dispatcher::refresh_indicators`] last started (main thread).
    static LAST_INDICATOR_REFRESH: std::cell::Cell<Option<Instant>> =
        const { std::cell::Cell::new(None) };
}

pub(crate) fn spawn_bg<T: Send + 'static>(
    cx: &mut App,
    work: impl FnOnce() -> T + Send + 'static,
    then: impl FnOnce(T, &mut App) + 'static,
) {
    let task = cx.background_executor().spawn(async move { work() });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let result = task.await;
        cx.update(|cx| then(result, cx));
    })
    .detach();
}

impl Dispatcher {
    // ---- helpers ----

    /// Settings › Advanced › Use Git Credential Manager: only for remotes that
    /// are not GitHub (GHD `useExternalCredentialHelper`). Also arms the
    /// stalled-transfer timeout of flag `network-stall-timeout` (0 = none).
    fn arm_credential_helper(remote_url: &str, cx: &App) {
        let s = Self::state(cx).read(cx);
        let host = host_of(remote_url);
        let github = host == "github.com" || s.accounts.iter().any(|a| a.host() == host);
        corvane_git::set_credential_helper(s.settings.use_external_credential_helper && !github);
        corvane_git::set_network_stall_timeout(
            u32::try_from(s.flags.number(crate::flags::ids::NETWORK_STALL_TIMEOUT)).unwrap_or(0),
        );
    }

    /// `GIT_ASKPASS` environment: one login per host from the signed-in
    /// accounts and the generic credentials the user saved.
    pub(crate) fn askpass_env(cx: &App) -> Option<AskpassEnv> {
        let s = Self::state(cx).read(cx);
        let mut logins: Vec<String> = s
            .accounts
            .iter()
            .map(|a| format!("{}={}", a.host(), a.login))
            .collect();
        logins.extend(
            s.generic_logins
                .iter()
                .map(|(host, user)| format!("{host}={user}")),
        );
        AskpassEnv::current_exe(logins.join(";"))
    }

    /// GHD `currentRemote`: the branch's upstream remote, else `origin`, else
    /// the first remote.
    pub fn current_remote(id: u64, cx: &App) -> Option<Remote> {
        Self::current_remote_in(Self::state(cx).read(cx), id)
    }

    pub fn current_remote_in(s: &crate::state::AppState, id: u64) -> Option<Remote> {
        let info = s.repo_states.get(&id)?.info.as_ref()?;
        let upstream_remote = info
            .current_branch()
            .and_then(|b| b.upstream_remote_name().map(str::to_string));
        upstream_remote
            .and_then(|name| info.remotes.iter().find(|r| r.name == name))
            .or_else(|| corvane_git::find_default_remote(&info.remotes))
            .cloned()
    }

    /// Flag `826`: delete a tag that may have been pushed - from `remote`
    /// first (`push --delete`, so a failure keeps the local tag), then
    /// locally; with `remote` `None` only locally.
    pub fn delete_pushed_tag(id: u64, tag: String, remote: Option<Remote>, cx: &mut App) {
        let Some(remote) = remote else {
            return Self::delete_tag(id, tag, cx);
        };
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        if !Self::begin_network(id, cx) {
            return;
        }
        Self::arm_credential_helper(&remote.url, cx);
        let askpass = Self::askpass_env(cx);
        Self::set_progress(
            id,
            Some(PushPullProgress {
                kind: PushPullKind::Push,
                title: format!("Deleting tag {tag} from {}", remote.name),
                description: None,
                value: 0.,
            }),
            cx,
        );
        let tag_for_task = tag.clone();
        Self::run_network(
            id,
            cx,
            move |_| {
                corvane_git::delete_remote_tag(
                    git,
                    &workdir,
                    &remote.name,
                    &tag_for_task,
                    askpass.as_ref(),
                )
            },
            move |result, cx| match result {
                Ok(()) => Self::delete_tag(id, tag, cx),
                Err(err) => Self::show_error("Could not delete tag", err.to_string(), cx),
            },
        );
    }

    /// GHD `getCurrentBranchForcePushState`
    pub fn force_push_state(id: u64, cx: &App) -> ForcePushState {
        Self::force_push_state_in(Self::state(cx).read(cx), id)
    }

    pub fn force_push_state_in(s: &crate::state::AppState, id: u64) -> ForcePushState {
        let Some(rs) = s.repo_states.get(&id) else {
            return ForcePushState::NotAvailable;
        };
        let Some(ab) = rs.ahead_behind else {
            return ForcePushState::NotAvailable;
        };
        if ab.ahead == 0 || ab.behind == 0 {
            return ForcePushState::NotAvailable;
        }
        let recommended = rs
            .info
            .as_ref()
            .and_then(|i| i.current_branch())
            .is_some_and(|b| rs.force_push_branches.get(b.name_without_remote()) == b.tip.as_ref());
        // `260-force-push-after-outside-rewrite`: GHD recommends a force
        // push only after its own amend or rebase
        let rewritten_outside = rs.upstream_rewritten
            && s.flags
                .bool(crate::flags::ids::FORCE_PUSH_AFTER_OUTSIDE_REWRITE);
        if recommended || rewritten_outside {
            ForcePushState::Recommended
        } else {
            ForcePushState::Available
        }
    }

    fn set_progress(id: u64, progress: Option<PushPullProgress>, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).push_pull_progress = progress;
            cx.notify();
        });
    }

    /// GHD `withPushPullFetch`: one network operation at a time per repository.
    fn begin_network(id: u64, cx: &mut App) -> bool {
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.push_pull_in_progress {
                return false;
            }
            rs.push_pull_in_progress = true;
            cx.notify();
            true
        })
    }

    /// `245-push-during-background-fetch`: a push, pull or fetch asked for
    /// while a background fetch runs waits for it (GHD disables the button
    /// and drops the request).
    fn behind_background_fetch(id: u64, cx: &App) -> bool {
        Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .is_some_and(|r| r.push_pull_in_progress && r.quiet_background_fetch)
    }

    /// Run `then` once the repository's network operation finished.
    fn after_network(id: u64, cx: &mut App, then: impl FnOnce(&mut App) + 'static) {
        cx.spawn(async move |cx: &mut AsyncApp| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let busy = cx.update(|cx| {
                    Self::state(cx)
                        .read(cx)
                        .repo_states
                        .get(&id)
                        .is_some_and(|r| r.push_pull_in_progress)
                });
                if !busy {
                    break;
                }
            }
            cx.update(then);
        })
        .detach();
    }

    fn end_network(id: u64, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            rs.push_pull_in_progress = false;
            rs.quiet_background_fetch = false;
            rs.push_pull_progress = None;
            cx.notify();
        });
    }

    /// Run a network operation on a background thread, mirroring progress
    /// into the push/pull button, then handle the outcome.
    fn run_network<T: Send + 'static>(
        id: u64,
        cx: &mut App,
        work: impl FnOnce(&mut dyn FnMut(PushPullProgress)) -> T + Send + 'static,
        then: impl FnOnce(T, &mut App) + 'static,
    ) {
        let (tx, rx) = async_channel::unbounded::<PushPullProgress>();
        let task = cx.background_executor().spawn(async move {
            let mut report = |p: PushPullProgress| {
                let _ = tx.send_blocking(p);
            };
            work(&mut report)
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok(progress) = rx.recv().await {
                cx.update(|cx| Self::set_progress(id, Some(progress), cx));
            }
            let result = task.await;
            cx.update(|cx| {
                Self::end_network(id, cx);
                then(result, cx);
            });
        })
        .detach();
    }

    /// The remote-specific error dialogs (`pushNeedsPullHandler`,
    /// `gitAuthenticationErrorHandler`); everything else is a plain error.
    fn handle_remote_error(
        id: u64,
        title: &str,
        err: corvane_git::GitError,
        remote_url: String,
        retry: RetryAction,
        background: bool,
        cx: &mut App,
    ) {
        if background {
            warn!(id, %err, "background remote operation failed");
            return;
        }
        let stderr = match &err {
            corvane_git::GitError::Failed { stderr, .. } => stderr.clone(),
            _ => String::new(),
        };
        let github = Self::state(cx)
            .read(cx)
            .repository(id)
            .and_then(|r| r.github.clone());
        match corvane_git::remote_failure(&err) {
            RemoteFailure::PushNotFastForward => {
                Self::show_popup(Popup::PushNeedsPull { repo: id }, cx);
            }
            // `secretScanningPushProtectionErrorHandler`
            RemoteFailure::PushWithSecretDetected => {
                let secrets = crate::push_errors::secret_scan_results(
                    &crate::push_errors::remote_message(&stderr),
                );
                if secrets.is_empty() {
                    Self::show_error(title, err.to_string(), cx);
                } else {
                    Self::show_popup(
                        Popup::PushProtectionError {
                            repo: id,
                            secrets,
                            bypassed: Vec::new(),
                        },
                        cx,
                    );
                }
            }
            // `refusedWorkflowUpdate`
            RemoteFailure::MissingWorkflowScope if github.is_some() => {
                match crate::push_errors::rejected_workflow_path(&stderr) {
                    Some(rejected_path) => Self::show_popup(
                        Popup::PushRejectedDueToMissingWorkflowScope {
                            repo: id,
                            rejected_path,
                        },
                        cx,
                    ),
                    None => Self::show_error(title, err.to_string(), cx),
                }
            }
            // `samlReauthRequired`
            RemoteFailure::SamlReauthRequired if github.is_some() => {
                let organization = crate::push_errors::saml_organization(
                    &crate::push_errors::remote_message(&stderr),
                );
                match (organization, github) {
                    (Some(organization), Some(gh)) => Self::show_popup(
                        Popup::SAMLReauthRequired {
                            repo: id,
                            organization,
                            endpoint: gh.endpoint,
                            retry: Some(retry),
                        },
                        cx,
                    ),
                    _ => Self::show_error(title, err.to_string(), cx),
                }
            }
            // `insufficientGitHubRepoPermissions`: offer a fork. With
            // `303-fork-before-push` known read-only repositories never get
            // here (see `push_then`); this covers repositories whose
            // permissions were never fetched, and every read-only one when
            // the flag is off (GHD's path).
            RemoteFailure::PermissionDenied
                if matches!(retry, RetryAction::Push { .. })
                    && github.as_ref().is_some_and(|gh| {
                        let s = Self::state(cx).read(cx);
                        (gh.permissions.is_none() || !gh.has_write_permission())
                            && s.account_for(&gh.endpoint).is_some()
                            && !Self::fork_offer_blocked(s, gh)
                    }) =>
            {
                Self::show_create_fork_dialog(id, cx);
            }
            RemoteFailure::AuthenticationFailed => {
                let host = host_of(&remote_url);
                let username = {
                    let s = Self::state(cx).read(cx);
                    s.accounts
                        .iter()
                        .find(|a| a.host() == host)
                        .map(|a| a.login.clone())
                        .or_else(|| s.generic_logins.get(&host).cloned())
                };
                Self::show_popup(
                    Popup::GenericGitAuthentication {
                        repo: id,
                        remote_url,
                        host,
                        username,
                        retry,
                    },
                    cx,
                );
            }
            _ => {
                // `255-plain-language-remote-errors`: say what went wrong
                // before git's message
                let plain = Self::state(cx)
                    .read(cx)
                    .flags
                    .bool(crate::flags::ids::PLAIN_LANGUAGE_REMOTE_ERRORS)
                    .then(|| crate::push_errors::plain_remote_error(&err))
                    .flatten();
                Self::show_error(title, plain.unwrap_or_else(|| err.to_string()), cx)
            }
        }
    }

    // ---- fetch ----

    /// `252-prune-stale-refs-and-retry`: when a fetch or pull failed because
    /// a stale remote-tracking ref blocks a new one, run `git remote prune`
    /// and say to try once more (`retry` is cleared). GHD shows the error.
    fn prune_before_retry<T>(
        retry: &mut bool,
        result: &Result<T, corvane_git::GitError>,
        git: &std::sync::Arc<corvane_git::GitBinary>,
        workdir: &std::path::Path,
        remote: &str,
        askpass: Option<&AskpassEnv>,
    ) -> bool {
        if !std::mem::take(retry)
            || !result
                .as_ref()
                .is_err_and(corvane_git::is_stale_remote_ref_failure)
        {
            return false;
        }
        info!(remote, "stale remote-tracking ref; pruning and retrying");
        corvane_git::prune_remote(git.clone(), workdir, remote, askpass).is_ok()
    }

    /// The Corvane additions to a fetch of repository `id`.
    fn fetch_options(s: &crate::state::AppState, id: u64) -> corvane_git::FetchOptions {
        corvane_git::FetchOptions {
            // `248-fetch-prune-tags`: drop tags deleted on the remote, but
            // never while tags created here wait to be pushed (they would
            // be lost)
            prune_tags: s.flags.bool(crate::flags::ids::FETCH_PRUNE_TAGS)
                && s.repository(id).is_some_and(|r| r.tags_to_push.is_empty()),
            // `249-fetch-writes-commit-graph`
            write_commit_graph: s.flags.bool(crate::flags::ids::FETCH_WRITES_COMMIT_GRAPH),
            // `250-sync-skips-submodules`
            skip_submodules: s.flags.bool(crate::flags::ids::SYNC_SKIPS_SUBMODULES),
        }
    }

    /// `_fetch(FetchType::UserInitiatedTask | BackgroundTask)`
    pub fn fetch(id: u64, background: bool, cx: &mut App) {
        Self::fetch_remote_then(id, None, background, |_, _| {}, cx);
    }

    /// `fetch` from `remote` (default: the current branch's remote), then
    /// `then(fetched)` once it finished or did not start.
    pub fn fetch_remote_then(
        id: u64,
        remote: Option<&str>,
        background: bool,
        then: impl FnOnce(bool, &mut App) + 'static,
        cx: &mut App,
    ) {
        if !background && Self::behind_background_fetch(id, cx) {
            let remote = remote.map(str::to_string);
            return Self::after_network(id, cx, move |cx| {
                Self::fetch_remote_then(id, remote.as_deref(), false, then, cx)
            });
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return then(false, cx);
        };
        let remote = match remote {
            Some(name) => Self::state(cx)
                .read(cx)
                .repo_states
                .get(&id)
                .and_then(|r| r.info.as_ref())
                .and_then(|i| i.remotes.iter().find(|r| r.name == name))
                .cloned(),
            None => Self::current_remote(id, cx),
        };
        let Some(remote) = remote else {
            return then(false, cx);
        };
        if !Self::begin_network(id, cx) {
            return then(false, cx);
        }
        // `245-push-during-background-fetch`: the background fetch leaves the
        // push/pull button alone
        let quiet = background
            && Self::state(cx)
                .read(cx)
                .flags
                .bool(crate::flags::ids::PUSH_DURING_BACKGROUND_FETCH);
        Self::arm_credential_helper(&remote.url, cx);
        let askpass = Self::askpass_env(cx);
        let title = format!("Fetching {}", remote.name);
        if quiet {
            Self::state(cx).update(cx, |s, _| {
                s.repo_state_mut(id).quiet_background_fetch = true;
            });
        } else {
            Self::set_progress(
                id,
                Some(PushPullProgress {
                    kind: PushPullKind::Fetch,
                    title: title.clone(),
                    description: None,
                    value: 0.,
                }),
                cx,
            );
        }
        let remote_name = remote.name.clone();
        let remote_url = remote.url.clone();
        let options = Self::fetch_options(Self::state(cx).read(cx), id);
        let prune_retry = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::PRUNE_STALE_REFS_AND_RETRY);
        let fast_forward_current = background
            && Self::state(cx)
                .read(cx)
                .flags
                .bool(crate::flags::ids::BACKGROUND_FETCH_FAST_FORWARDS);
        Self::run_network(
            id,
            cx,
            move |report| {
                let mut report = |progress| {
                    if !quiet {
                        report(progress)
                    }
                };
                let mut retry = prune_retry;
                let result = loop {
                    let result = corvane_git::fetch_with(
                        git.clone(),
                        &workdir,
                        &remote_name,
                        options,
                        askpass.as_ref(),
                        &mut |value, text| {
                            report(PushPullProgress {
                                kind: PushPullKind::Fetch,
                                title: title.clone(),
                                description: Some(text),
                                value: value * 0.9,
                            })
                        },
                    );
                    if !Self::prune_before_retry(
                        &mut retry,
                        &result,
                        &git,
                        &workdir,
                        &remote_name,
                        askpass.as_ref(),
                    ) {
                        break result;
                    }
                };
                if result.is_ok() {
                    report(PushPullProgress {
                        kind: PushPullKind::Generic,
                        title: REFRESHING_REPOSITORY.into(),
                        description: Some("Fast-forwarding branches".into()),
                        value: 0.9,
                    });
                    let _ = corvane_git::fast_forward_branches(git.clone(), &workdir);
                    // `246-background-fetch-fast-forwards`: a clean branch
                    // that is only behind catches up (GHD leaves it for Pull)
                    if fast_forward_current {
                        match corvane_git::fast_forward_if_only_behind(git, &workdir) {
                            Ok(true) => info!(id, "fast-forwarded after background fetch"),
                            Ok(false) => {}
                            Err(err) => warn!(id, %err, "fast-forward after fetch failed"),
                        }
                    }
                }
                result
            },
            move |result, cx| {
                let fetched = result.is_ok();
                if let Err(err) = result {
                    Self::handle_remote_error(
                        id,
                        "Could not fetch",
                        err,
                        remote_url,
                        RetryAction::Fetch,
                        background,
                        cx,
                    );
                }
                Self::refresh_repository(id, cx);
                then(fetched, cx);
            },
        );
    }

    /// Repository › Fetch All Repositories (`247-fetch-all-repositories`;
    /// GHD has none): fetch every listed repository with a remote, one at a
    /// time on a background thread, skipping those with a network operation
    /// running; failures are collected into one error.
    pub fn fetch_all_repositories(cx: &mut App) {
        static RUNNING: AtomicBool = AtomicBool::new(false);
        let (git, repos, use_helper, github_hosts, selected) = {
            let s = Self::state(cx).read(cx);
            if !s.flags.bool(crate::flags::ids::FETCH_ALL_REPOSITORIES) {
                return;
            }
            let Some(git) = s.git.clone() else { return };
            let use_helper = s.settings.use_external_credential_helper;
            let repos: Vec<_> = s
                .repositories
                .iter()
                .filter(|r| !r.missing)
                .filter(|r| {
                    !s.repo_states
                        .get(&r.id)
                        .is_some_and(|rs| rs.push_pull_in_progress)
                })
                .map(|r| (r.name(), r.path.clone(), Self::fetch_options(s, r.id)))
                .collect();
            let github_hosts: Vec<String> = std::iter::once("github.com".to_string())
                .chain(s.accounts.iter().map(|a| a.host()))
                .collect();
            (git, repos, use_helper, github_hosts, s.selected)
        };
        if RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }
        let askpass = Self::askpass_env(cx);
        spawn_bg(
            cx,
            move || {
                let mut failures = Vec::new();
                for (name, path, options) in repos {
                    let Ok(info) = corvane_git::open_repository(&path) else {
                        continue;
                    };
                    let upstream_remote = info
                        .current_branch()
                        .and_then(|b| b.upstream_remote_name().map(str::to_string));
                    let Some(remote) = upstream_remote
                        .and_then(|n| info.remotes.iter().find(|r| r.name == n))
                        .or_else(|| corvane_git::find_default_remote(&info.remotes))
                        .cloned()
                    else {
                        continue;
                    };
                    corvane_git::set_credential_helper(
                        use_helper && !github_hosts.contains(&host_of(&remote.url)),
                    );
                    match corvane_git::fetch_with(
                        git.clone(),
                        &info.workdir,
                        &remote.name,
                        options,
                        askpass.as_ref(),
                        &mut |_, _| {},
                    ) {
                        Ok(()) => {
                            let _ = corvane_git::fast_forward_branches(git.clone(), &info.workdir);
                        }
                        Err(err) => failures.push(format!("{name}: {err}")),
                    }
                }
                failures
            },
            move |failures, cx| {
                RUNNING.store(false, Ordering::SeqCst);
                if !failures.is_empty() {
                    Self::show_error(
                        "Could not fetch all repositories",
                        failures.join("\n\n"),
                        cx,
                    );
                }
                if let Some(id) = selected {
                    Self::refresh_repository(id, cx);
                }
                Self::refresh_indicators(cx);
            },
        );
    }

    // ---- pull ----

    /// `_pull`
    pub fn pull(id: u64, cx: &mut App) {
        if Self::behind_background_fetch(id, cx) {
            return Self::after_network(id, cx, move |cx| Self::pull(id, cx));
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(remote) = Self::current_remote(id, cx) else {
            Self::show_error("Could not pull", "The repository has no remotes.", cx);
            return;
        };
        let tip = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.info.as_ref())
            .map(|i| i.tip.clone());
        match tip {
            Some(Tip::Unborn { .. }) => {
                Self::show_error("Could not pull", "The current branch is unborn.", cx);
                return;
            }
            Some(Tip::Detached { .. }) => {
                Self::show_error(
                    "Could not pull",
                    "The current repository is in a detached HEAD state.",
                    cx,
                );
                return;
            }
            _ => {}
        }
        if !Self::begin_network(id, cx) {
            return;
        }
        Self::arm_credential_helper(&remote.url, cx);
        let askpass = Self::askpass_env(cx);
        let title = format!("Pulling {}", remote.name);
        let keep_remote_head = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::REMOTE_HEAD_ONCE);
        let skip_submodules = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::SYNC_SKIPS_SUBMODULES);
        let prune_retry = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::PRUNE_STALE_REFS_AND_RETRY);
        Self::set_progress(
            id,
            Some(PushPullProgress {
                kind: PushPullKind::Pull,
                title: title.clone(),
                description: None,
                value: 0.,
            }),
            cx,
        );
        let remote_name = remote.name.clone();
        let remote_url = remote.url.clone();
        Self::run_network(
            id,
            cx,
            move |report| {
                let mut retry = prune_retry;
                let result = loop {
                    let result = corvane_git::pull(
                        git.clone(),
                        &workdir,
                        &remote_name,
                        skip_submodules,
                        askpass.as_ref(),
                        &mut |value, text| {
                            report(PushPullProgress {
                                kind: PushPullKind::Pull,
                                title: title.clone(),
                                description: Some(text),
                                value: value * 0.6,
                            })
                        },
                    );
                    if !Self::prune_before_retry(
                        &mut retry,
                        &result,
                        &git,
                        &workdir,
                        &remote_name,
                        askpass.as_ref(),
                    ) {
                        break result;
                    }
                };
                // `251-remote-head-once`: `set-head -a` asks the server for
                // every ref, which takes minutes on huge repositories; skip
                // it while the remote's HEAD already resolves
                if result.is_ok()
                    && !(keep_remote_head
                        && corvane_git::remote_head_resolves(git.clone(), &workdir, &remote_name))
                {
                    let _ = corvane_git::update_remote_head(
                        git.clone(),
                        &workdir,
                        &remote_name,
                        askpass.as_ref(),
                    );
                }
                if result.is_ok() {
                    report(PushPullProgress {
                        kind: PushPullKind::Generic,
                        title: REFRESHING_REPOSITORY.into(),
                        description: Some("Fast-forwarding branches".into()),
                        value: 0.9,
                    });
                    let _ = corvane_git::fast_forward_branches(git.clone(), &workdir);
                }
                let status = corvane_git::get_status(git, &workdir, None).ok();
                (result, status)
            },
            move |(result, status), cx| {
                if let Some(status) = status {
                    Self::state(cx).update(cx, |s, cx| {
                        let rs = s.repo_state_mut(id);
                        rs.conflict_state =
                            crate::mco::derive_conflict_state(&status, rs.conflict_state.as_ref());
                        rs.status = Some(status);
                        cx.notify();
                    });
                }
                if let Err(err) = result {
                    // merge / rebase conflicts from a pull show the conflicts
                    // dialog through the refresh (`mergeConflictHandler`)
                    let conflicted = Self::state(cx)
                        .read(cx)
                        .repo_states
                        .get(&id)
                        .is_some_and(|r| r.conflict_state.is_some());
                    if !conflicted {
                        Self::handle_remote_error(
                            id,
                            "Could not pull",
                            err,
                            remote_url,
                            RetryAction::Pull,
                            false,
                            cx,
                        );
                    }
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    // ---- update a branch from its upstream ----

    /// The branch list's "Update from <upstream>" (`857-update-branch-from-upstream`;
    /// GHD has none): fast-forward a local branch that is not checked out.
    pub fn update_branch_from_upstream(id: u64, name: String, cx: &mut App) {
        let target = {
            let s = Self::state(cx).read(cx);
            if !s.flags.bool(crate::flags::ids::UPDATE_BRANCH_FROM_UPSTREAM) {
                return;
            }
            let info = s.repo_states.get(&id).and_then(|r| r.info.as_ref());
            info.and_then(|info| {
                let branch = info
                    .branches
                    .iter()
                    .find(|b| b.name == name && b.kind == corvane_models::BranchKind::Local)?;
                if info.current_branch().is_some_and(|c| c.name == name) {
                    return None;
                }
                let remote = branch.upstream_remote_name()?;
                let remote_branch = branch
                    .upstream_short()?
                    .strip_prefix(remote)?
                    .strip_prefix('/')?
                    .to_string();
                let url = info.remotes.iter().find(|r| r.name == remote)?.url.clone();
                Some((
                    remote.to_string(),
                    remote_branch,
                    url,
                    branch.upstream_short()?.to_string(),
                ))
            })
        };
        let Some((remote, remote_branch, remote_url, upstream)) = target else {
            return;
        };
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        if !Self::begin_network(id, cx) {
            return;
        }
        Self::arm_credential_helper(&remote_url, cx);
        let askpass = Self::askpass_env(cx);
        Self::set_progress(
            id,
            Some(PushPullProgress {
                kind: PushPullKind::Fetch,
                title: format!("Updating {name} from {upstream}"),
                description: None,
                value: 0.,
            }),
            cx,
        );
        let local = name.clone();
        Self::run_network(
            id,
            cx,
            move |_| {
                corvane_git::fast_forward_branch_from_remote(
                    git,
                    &workdir,
                    &remote,
                    &remote_branch,
                    &local,
                    askpass.as_ref(),
                )
            },
            move |result, cx| {
                if let Err(err) = result {
                    let title = "Could not update branch";
                    if corvane_git::remote_failure(&err) == RemoteFailure::PushNotFastForward {
                        Self::show_error(
                            title,
                            format!(
                                "{name} has commits that are not on {upstream}, so it cannot be \
                                 fast-forwarded. Check it out and pull instead."
                            ),
                            cx,
                        );
                    } else {
                        Self::handle_remote_error(
                            id,
                            title,
                            err,
                            remote_url,
                            RetryAction::Fetch,
                            false,
                            cx,
                        );
                    }
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    // ---- push ----

    /// `_push` (+ `performPush`): publish the branch when it has no upstream.
    pub fn push(id: u64, force_with_lease: bool, branch: Option<String>, cx: &mut App) {
        Self::push_then(id, force_with_lease, branch, |_, _| {}, cx);
    }

    /// `push`, then `then(outcome)` once it finished (or did not start).
    pub fn push_then(
        id: u64,
        force_with_lease: bool,
        branch: Option<String>,
        then: impl FnOnce(PushOutcome, &mut App) + 'static,
        cx: &mut App,
    ) {
        Self::push_inner(id, force_with_lease, branch, None, then, cx);
    }

    /// Corvane addition (flag `816`, history "Push Up to This Commit"):
    /// push the current branch's upstream only up to `sha`,
    /// `push <remote> <sha>:refs/heads/<upstream branch>`. No force, so a
    /// commit that is not ahead of the upstream is refused by git; unpushed
    /// tags stay behind (they may point past `sha`).
    pub fn push_up_to(id: u64, sha: String, cx: &mut App) {
        Self::push_inner(id, false, None, Some(sha), |_, _| {}, cx);
    }

    pub(crate) fn push_inner(
        id: u64,
        force_with_lease: bool,
        branch: Option<String>,
        up_to: Option<String>,
        then: impl FnOnce(PushOutcome, &mut App) + 'static,
        cx: &mut App,
    ) {
        if Self::behind_background_fetch(id, cx) {
            return Self::after_network(id, cx, move |cx| {
                Self::push_inner(id, force_with_lease, branch, up_to, then, cx)
            });
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return then(PushOutcome::NotAttempted, cx);
        };
        let Some(remote) = Self::current_remote(id, cx) else {
            Self::show_popup(Popup::PublishRepository { repo: id }, cx);
            return then(PushOutcome::NotAttempted, cx);
        };
        // no write access: suggest a fork before git runs (GHD pushes and
        // offers it after the auth failure, `insufficientGitHubRepoPermissions`;
        // `303-fork-before-push` off takes that path)
        let read_only = {
            let s = Self::state(cx).read(cx);
            s.flags.bool(crate::flags::ids::FORK_BEFORE_PUSH)
                && s.repository(id)
                    .and_then(|r| r.github.as_ref())
                    .is_some_and(|gh| {
                        !gh.has_write_permission()
                            && s.account_for(&gh.endpoint).is_some()
                            && !Self::fork_offer_blocked(s, gh)
                    })
        };
        if read_only {
            Self::show_create_fork_dialog(id, cx);
            return then(PushOutcome::NotAttempted, cx);
        }
        let (branch, tip_error) = {
            let s = Self::state(cx).read(cx);
            let info = s.repo_states.get(&id).and_then(|r| r.info.as_ref());
            match branch {
                Some(name) => (
                    info.and_then(|i| i.branches.iter().find(|b| b.name == name).cloned()),
                    None,
                ),
                None => match info.map(|i| &i.tip) {
                    Some(Tip::Valid { branch }) => (Some(branch.clone()), None),
                    Some(Tip::Unborn { .. }) => (None, Some("The current branch is unborn.")),
                    Some(Tip::Detached { .. }) => (
                        None,
                        Some("The current repository is in a detached HEAD state."),
                    ),
                    _ => (None, None),
                },
            }
        };
        if let Some(message) = tip_error {
            Self::show_error("Could not push", message, cx);
            return then(PushOutcome::NotAttempted, cx);
        }
        let Some(branch) = branch else {
            return then(PushOutcome::NotAttempted, cx);
        };
        if up_to.is_some() && branch.upstream.is_none() {
            Self::show_error(
                "Could not push",
                "The current branch has not been published yet.",
                cx,
            );
            return then(PushOutcome::NotAttempted, cx);
        }
        if !Self::begin_network(id, cx) {
            return then(PushOutcome::NotAttempted, cx);
        }
        // GHD clears the "force push recommended" mark before the push runs,
        // so a failed force push leaves a plain Push button (desktop#16352);
        // `258-force-push-kept-on-failure` clears it after success only
        let keep_force_push_on_failure = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::FORCE_PUSH_KEPT_ON_FAILURE);
        let force_push_branch = branch.name_without_remote().to_string();
        if force_with_lease && !keep_force_push_on_failure {
            Self::state(cx).update(cx, |s, _| {
                s.repo_state_mut(id)
                    .force_push_branches
                    .remove(&force_push_branch);
            });
        }
        Self::arm_credential_helper(&remote.url, cx);
        let askpass = Self::askpass_env(cx);
        let remote_name = branch
            .upstream_remote_name()
            .map(str::to_string)
            .unwrap_or_else(|| remote.name.clone());
        let title = format!("Pushing to {remote_name}");
        Self::set_progress(
            id,
            Some(PushPullProgress {
                kind: PushPullKind::Push,
                title: title.clone(),
                description: None,
                value: 0.,
            }),
            cx,
        );
        let local = up_to.clone().unwrap_or_else(|| branch.name.clone());
        let remote_branch = branch
            .upstream_short()
            .and_then(|u| u.split_once('/').map(|(_, b)| b.to_string()))
            .map(|b| match up_to {
                Some(_) => format!("refs/heads/{b}"),
                None => b,
            });
        let remote_url = remote.url.clone();
        // GHD `pushRepo(…, gitStore.tagsToPush)`: unpushed tags ride along
        // (not on a partial push: they may point past its commit)
        let tags: Vec<String> = Self::state(cx)
            .read(cx)
            .repository(id)
            .filter(|_| up_to.is_none())
            .map(|r| r.tags_to_push.clone())
            .unwrap_or_default();
        let pushed_tags = !tags.is_empty();
        // GHD's plain fetch after a push, plus `249` / `250`
        let fetch_options = corvane_git::FetchOptions {
            prune_tags: false,
            ..Self::fetch_options(Self::state(cx).read(cx), id)
        };
        let retry = RetryAction::Push {
            force_with_lease,
            branch: Some(branch.name.clone()),
            up_to,
        };
        Self::run_network(
            id,
            cx,
            move |report| {
                let result = corvane_git::push(
                    git.clone(),
                    &workdir,
                    &remote_name,
                    &local,
                    remote_branch.as_deref(),
                    &tags,
                    force_with_lease,
                    askpass.as_ref(),
                    &mut |value, text| {
                        report(PushPullProgress {
                            kind: PushPullKind::Push,
                            title: title.clone(),
                            description: Some(text),
                            value: value * 0.65,
                        })
                    },
                );
                if result.is_ok() {
                    report(PushPullProgress {
                        kind: PushPullKind::Fetch,
                        title: format!("Fetching {remote_name}"),
                        description: None,
                        value: 0.65,
                    });
                    let _ = corvane_git::fetch_with(
                        git.clone(),
                        &workdir,
                        &remote_name,
                        fetch_options,
                        askpass.as_ref(),
                        &mut |value, text| {
                            report(PushPullProgress {
                                kind: PushPullKind::Fetch,
                                title: format!("Fetching {remote_name}"),
                                description: Some(text),
                                value: 0.65 + value * 0.25,
                            })
                        },
                    );
                    report(PushPullProgress {
                        kind: PushPullKind::Generic,
                        title: REFRESHING_REPOSITORY.into(),
                        description: Some("Fast-forwarding branches".into()),
                        value: 0.9,
                    });
                    let _ = corvane_git::fast_forward_branches(git, &workdir);
                }
                result
            },
            move |result, cx| {
                let pushed = result.is_ok();
                if pushed && force_with_lease && keep_force_push_on_failure {
                    Self::state(cx).update(cx, |s, _| {
                        s.repo_state_mut(id)
                            .force_push_branches
                            .remove(&force_push_branch);
                    });
                }
                // `clearTagsToPush` once the push went through
                if pushed && pushed_tags {
                    Self::update_tags_to_push(id, cx, Vec::clear);
                }
                if let Err(err) = result {
                    Self::handle_remote_error(
                        id,
                        "Could not push",
                        err,
                        remote_url,
                        retry,
                        false,
                        cx,
                    );
                }
                Self::refresh_repository(id, cx);
                then(
                    if pushed {
                        PushOutcome::Pushed
                    } else {
                        PushOutcome::Failed
                    },
                    cx,
                );
            },
        );
    }

    /// The toolbar button's main click (`PushPullButton.renderButton`).
    pub fn push_pull_action(id: u64, cx: &mut App) {
        let (has_remote, tip, upstream, ab) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            let info = rs.and_then(|r| r.info.as_ref());
            (
                info.is_some_and(|i| !i.remotes.is_empty()),
                info.map(|i| i.tip.clone()),
                info.and_then(|i| i.current_branch())
                    .and_then(|b| b.upstream.clone()),
                rs.and_then(|r| r.ahead_behind),
            )
        };
        if !has_remote {
            Self::show_popup(Popup::PublishRepository { repo: id }, cx);
            return;
        }
        match tip {
            Some(Tip::Unborn { .. }) => return Self::fetch(id, false, cx),
            Some(Tip::Detached { .. }) | Some(Tip::Unknown) | None => return,
            Some(Tip::Valid { .. }) => {}
        }
        if upstream.is_none() {
            return Self::push(id, false, None, cx);
        }
        match ab {
            Some(ab) if ab.ahead == 0 && ab.behind == 0 => Self::fetch(id, false, cx),
            _ if Self::force_push_state(id, cx) == ForcePushState::Recommended => {
                Self::confirm_or_force_push(id, cx)
            }
            Some(ab) if ab.behind > 0 => Self::pull(id, cx),
            _ => Self::push(id, false, None, cx),
        }
    }

    /// `confirmOrForcePush`
    pub fn confirm_or_force_push(id: u64, cx: &mut App) {
        let upstream = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.info.as_ref())
            .and_then(|i| i.current_branch())
            .and_then(|b| b.upstream_short().map(str::to_string));
        let Some(upstream) = upstream else {
            warn!(id, "no upstream branch to force push");
            return;
        };
        if Self::state(cx).read(cx).settings.confirm_force_push {
            Self::show_popup(
                Popup::ConfirmForcePush {
                    repo: id,
                    upstream_branch: upstream,
                },
                cx,
            );
        } else {
            Self::push(id, true, None, cx);
        }
    }

    // ---- publish ----

    /// `_publishRepository`: create the GitHub repository, add `origin`, push.
    pub fn publish_repository(
        id: u64,
        name: String,
        description: String,
        private: bool,
        account: Account,
        org: Option<String>,
        cx: &mut App,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(token) = corvane_platform::keychain::token(&account.host(), &account.login)
            .ok()
            .flatten()
        else {
            Self::show_error(
                "Could not publish",
                "The account's token is missing from the keychain. Sign in again.",
                cx,
            );
            return;
        };
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).publishing = true;
            cx.notify();
        });
        let endpoint = corvane_github::Endpoint::from_api_base(&account.endpoint);
        let error_details = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::API_ERROR_DETAILS);
        spawn_bg(
            cx,
            move || {
                let client =
                    corvane_github::Client::new(endpoint, token).with_error_details(error_details);
                let repo = client
                    .create_repository(org.as_deref(), &name, &description, private)
                    .map_err(|e| e.to_string())?;
                corvane_git::add_remote(git, &workdir, "origin", &repo.clone_url)
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>(repo)
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.repo_state_mut(id).publishing = false;
                    cx.notify();
                });
                match result {
                    Ok(repo) => {
                        info!(id, name = %repo.name, "published repository");
                        Self::state(cx).update(cx, |s, cx| {
                            if let Some(r) = s.repositories.iter_mut().find(|r| r.id == id) {
                                r.github = Some(repo);
                            }
                            let _ = s.store.save_repositories(&s.repositories);
                            cx.notify();
                        });
                        Self::close_popup(cx);
                        Self::refresh_repository(id, cx);
                        // push the current branch (and set its upstream)
                        Self::push_after_publish(id, cx);
                    }
                    Err(message) => Self::show_error("Could not publish repository", message, cx),
                }
            },
        );
    }

    fn push_after_publish(id: u64, cx: &mut App) {
        // the remote list is refreshed asynchronously; push once it is there
        cx.spawn(async move |cx: &mut AsyncApp| {
            for _ in 0..20 {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let ready = cx.update(|cx| {
                    Self::state(cx)
                        .read(cx)
                        .repo_states
                        .get(&id)
                        .and_then(|r| r.info.as_ref())
                        .is_some_and(|i| !i.remotes.is_empty() && !r_loading(cx, id))
                });
                if ready {
                    cx.update(|cx| Self::push(id, false, None, cx));
                    return;
                }
            }
        })
        .detach();
    }

    // ---- generic credentials ----

    /// `GenericGitAuthentication` › Save: keychain + retry the operation.
    pub fn save_generic_credentials(
        host: String,
        username: String,
        password: String,
        id: u64,
        retry: RetryAction,
        cx: &mut App,
    ) {
        if let Err(err) =
            corvane_platform::keychain::store_generic_password(&host, &username, &password)
        {
            Self::show_error("Could not save credentials", err.to_string(), cx);
            return;
        }
        Self::state(cx).update(cx, |s, _| {
            s.generic_logins.insert(host.clone(), username.clone());
            let _ = s.store.save_generic_logins(&s.generic_logins);
        });
        Self::close_popup(cx);
        Self::perform_retry(id, retry, cx);
    }

    // ---- LFS ----

    /// GHD `_addRepositories` › `InitializeLFS`: offer to install the hooks
    /// when the repository tracks paths with LFS but has no hooks yet.
    pub fn check_lfs(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let already_asked = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .is_some_and(|r| r.lfs_checked);
        if already_asked {
            return;
        }
        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).lfs_checked = true);
        // `905-lfs-detect-by-attributes`: read the .gitattributes files instead
        // of `git lfs track`, which walks the whole worktree
        let by_attributes = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::LFS_DETECT_BY_ATTRIBUTES);
        spawn_bg(
            cx,
            move || {
                if by_attributes {
                    corvane_git::is_using_lfs_by_attributes(git.clone(), &workdir)
                        && !corvane_git::lfs_hooks_installed(&workdir)
                        && corvane_git::lfs_available(git)
                } else {
                    corvane_git::lfs_available(git.clone())
                        && corvane_git::is_using_lfs(git, &workdir)
                        && !corvane_git::lfs_hooks_installed(&workdir)
                }
            },
            move |needs_init, cx| {
                if needs_init && Self::state(cx).read(cx).popup.is_none() {
                    Self::show_popup(Popup::InitializeLFS { repos: vec![id] }, cx);
                }
            },
        );
    }

    /// `_installLFSHooks`
    pub fn install_lfs_hooks(repos: Vec<u64>, cx: &mut App) {
        let contexts: Vec<_> = repos
            .iter()
            .filter_map(|id| Self::repo_context(*id, cx))
            .collect();
        spawn_bg(
            cx,
            move || {
                let mut errors = Vec::new();
                for (git, workdir) in contexts {
                    if let Err(err) = corvane_git::install_lfs_hooks(git, &workdir) {
                        errors.push(err.to_string());
                    }
                }
                errors
            },
            move |errors, cx| {
                if !errors.is_empty() {
                    Self::show_error("Could not initialize Git LFS", errors.join("\n"), cx);
                }
            },
        );
    }

    // ---- background fetch + indicators ----

    /// Start the periodic background fetch and sidebar indicator refresh
    /// (`BackgroundFetcher`, `RepositoryIndicatorUpdater`). Call once.
    pub fn start_background_tasks(cx: &mut App) {
        cx.spawn(async move |cx: &mut AsyncApp| {
            // skew the first run so several instances do not sync up
            cx.background_executor()
                .timer(Duration::from_secs(20))
                .await;
            loop {
                cx.update(Self::background_fetch_tick);
                cx.background_executor()
                    .timer(BACKGROUND_FETCH_MINIMUM)
                    .await;
            }
        })
        .detach();
        // `217-prompt-indicator-refresh`: the first indicator refresh runs
        // right after launch (GHD's updater starts on its delayed cadence)
        let first_indicators = if Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::PROMPT_INDICATOR_REFRESH)
        {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(45)
        };
        cx.spawn(async move |cx: &mut AsyncApp| {
            cx.background_executor().timer(first_indicators).await;
            loop {
                cx.update(Self::refresh_indicators);
                cx.background_executor()
                    .timer(INDICATOR_REFRESH_INTERVAL)
                    .await;
            }
        })
        .detach();
    }

    /// Fetch the selected GitHub repository (see `244-background-fetch`) when
    /// its last fetch is older than the interval (`shouldBackgroundFetch`).
    fn background_fetch_tick(cx: &mut App) {
        let (id, last_fetched, busy) = {
            let s = Self::state(cx).read(cx);
            let Some(id) = s.selected else { return };
            let Some(repo) = s.repository(id) else { return };
            // GHD fetches GitHub repositories only; `244-background-fetch`
            // can also turn it off or extend it to any remote
            let fetch = match s.flags.text(crate::flags::ids::BACKGROUND_FETCH) {
                "off" => false,
                "any" => true,
                _ => repo.github.is_some(),
            };
            if !fetch {
                return;
            }
            let rs = s.repo_states.get(&id);
            (
                id,
                rs.and_then(|r| r.last_fetched),
                rs.is_some_and(|r| r.push_pull_in_progress || r.mco.is_some()),
            )
        };
        if busy {
            return;
        }
        let due = match last_fetched {
            None => true,
            Some(at) => SystemTime::now()
                .duration_since(at)
                .map(|d| d >= BACKGROUND_FETCH_INTERVAL)
                .unwrap_or(true),
        };
        if due {
            info!(id, "background fetch");
            Self::fetch(id, true, cx);
        }
    }

    /// [`Self::refresh_indicators`] unless indicators were refreshed less
    /// than a minute ago (`217-prompt-indicator-refresh`, on opening the
    /// repository list).
    pub fn refresh_indicators_if_stale(cx: &mut App) {
        let fresh = LAST_INDICATOR_REFRESH
            .with(|last| last.get())
            .is_some_and(|at| at.elapsed() < INDICATOR_REFRESH_MINIMUM);
        if !fresh {
            Self::refresh_indicators(cx);
        }
    }

    /// `refreshIndicatorForRepository` for every repository: changed files
    /// and ahead/behind, shown in the repository list.
    pub fn refresh_indicators(cx: &mut App) {
        LAST_INDICATOR_REFRESH.with(|last| last.set(Some(Instant::now())));
        let enabled = Self::state(cx)
            .read(cx)
            .settings
            .repository_indicators_enabled;
        if !enabled {
            Self::state(cx).update(cx, |s, cx| {
                if !s.indicators.is_empty() {
                    s.indicators.clear();
                    cx.notify();
                }
            });
            return;
        }
        let (git, repos) = {
            let s = Self::state(cx).read(cx);
            let Some(git) = s.git.clone() else { return };
            (
                git,
                s.repositories
                    .iter()
                    .filter(|r| !r.missing)
                    .map(|r| (r.id, r.path.clone()))
                    .collect::<Vec<_>>(),
            )
        };
        spawn_bg(
            cx,
            move || {
                let mut out: HashMap<u64, RepoIndicator> = HashMap::new();
                for (id, path) in repos {
                    let Ok(info) = corvane_git::open_repository(&path) else {
                        continue;
                    };
                    let changed = corvane_git::get_status(git.clone(), &info.workdir, None)
                        .map(|st| st.files.len())
                        .unwrap_or(0);
                    let ahead_behind = info.current_branch().and_then(|b| {
                        corvane_git::ahead_behind(git.clone(), &info.workdir, b)
                            .ok()
                            .flatten()
                    });
                    let branch = info.current_branch().map(|b| b.name.clone());
                    out.insert(
                        id,
                        RepoIndicator {
                            ahead_behind,
                            changed_files: changed,
                            branch,
                        },
                    );
                }
                out
            },
            move |indicators, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.indicators = indicators;
                    cx.notify();
                });
            },
        );
    }

    /// Clone dialog: fetch the account's repositories (`ApiRepositoriesStore.loadRepositories`).
    pub fn load_api_repositories(account: Account, cx: &mut App) {
        let endpoint = account.endpoint.clone();
        let already = Self::state(cx).update(cx, |s, cx| {
            if s.api_repositories_loading.contains(&endpoint) {
                return true;
            }
            // `ApiRepositoriesStore`: the last list shows right away while
            // the fresh one loads
            if !s.api_repositories.contains_key(&endpoint)
                && let Ok(Some(cached)) =
                    s.store
                        .get::<Vec<corvane_models::GitHubRepository>>(&format!(
                            "api-repositories:{endpoint}"
                        ))
            {
                s.api_repositories.insert(endpoint.clone(), cached);
            }
            s.api_repositories_loading.insert(endpoint.clone());
            cx.notify();
            false
        });
        if already {
            return;
        }
        let Some(token) = corvane_platform::keychain::token(&account.host(), &account.login)
            .ok()
            .flatten()
        else {
            Self::state(cx).update(cx, |s, cx| {
                s.api_repositories_loading.remove(&endpoint);
                cx.notify();
            });
            return;
        };
        let api = corvane_github::Endpoint::from_api_base(&endpoint);
        let endpoint_for_result = endpoint.clone();
        spawn_bg(
            cx,
            move || {
                corvane_github::Client::new(api, token)
                    .user_repositories()
                    .map_err(|e| e.to_string())
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.api_repositories_loading.remove(&endpoint_for_result);
                    match result {
                        Ok(repos) => {
                            if let Err(err) = s
                                .store
                                .set(&format!("api-repositories:{endpoint_for_result}"), &repos)
                            {
                                warn!(%err, "could not cache the repository list");
                            }
                            s.api_repositories
                                .insert(endpoint_for_result.clone(), repos);
                        }
                        Err(err) => warn!(%err, "could not load repositories"),
                    }
                    cx.notify();
                });
            },
        );
    }

    /// GHD `CreateFork` is out of scope; a plain "no write access" hint.
    pub fn remote_url_host(url: &str) -> String {
        host_of(url)
    }
}

fn r_loading(cx: &App, id: u64) -> bool {
    Dispatcher::state(cx)
        .read(cx)
        .repo_states
        .get(&id)
        .is_some_and(|r| r.loading)
}

/// `github.com` from `https://github.com/a/b.git` or `git@github.com:a/b.git`.
pub fn host_of(url: &str) -> String {
    let without_scheme = url.split("://").nth(1).unwrap_or(url);
    let without_user = without_scheme
        .split_once('@')
        .map(|(_, h)| h)
        .unwrap_or(without_scheme);
    without_user
        .split(['/', ':'])
        .next()
        .unwrap_or(without_user)
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_of_urls() {
        assert_eq!(host_of("https://github.com/a/b.git"), "github.com");
        assert_eq!(host_of("git@GitHub.com:a/b.git"), "github.com");
        assert_eq!(host_of("https://user@ghe.corp:8443/a/b"), "ghe.corp");
    }
}
