//! Remote operations - GHD `app-store.ts` `performFetch` / `performPull` /
//! `performPush` / `_publishRepository`, `dispatcher.ts` `confirmOrForcePush`,
//! the `BackgroundFetcher` (every hour, at least 5 minutes apart) and the
//! `RepositoryIndicatorUpdater` (every 15 minutes), plus the Git LFS
//! initialisation prompt (`InitializeLFS`).

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use corvane_git::{AskpassEnv, RemoteFailure};
use corvane_models::{Account, AheadBehind, Remote, Tip};
use gpui_kit::{App, AsyncApp};
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::persistence::StoreExt;
use crate::state::{Popup, RetryAction};

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

fn spawn_bg<T: Send + 'static>(
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

    /// `GIT_ASKPASS` environment: one login per host from the signed-in
    /// accounts and the generic credentials the user saved.
    fn askpass_env(cx: &App) -> Option<AskpassEnv> {
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
        if recommended {
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

    fn end_network(id: u64, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            rs.push_pull_in_progress = false;
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
        match corvane_git::remote_failure(&err) {
            RemoteFailure::PushNotFastForward => {
                Self::show_popup(Popup::PushNeedsPull { repo: id }, cx);
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
            _ => Self::show_error(title, err.to_string(), cx),
        }
    }

    // ---- fetch ----

    /// `_fetch(FetchType::UserInitiatedTask | BackgroundTask)`
    pub fn fetch(id: u64, background: bool, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(remote) = Self::current_remote(id, cx) else {
            return;
        };
        if !Self::begin_network(id, cx) {
            return;
        }
        let askpass = Self::askpass_env(cx);
        let title = format!("Fetching {}", remote.name);
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
        let remote_name = remote.name.clone();
        let remote_url = remote.url.clone();
        Self::run_network(
            id,
            cx,
            move |report| {
                let result = corvane_git::fetch(
                    git.clone(),
                    &workdir,
                    &remote_name,
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
                if result.is_ok() {
                    report(PushPullProgress {
                        kind: PushPullKind::Generic,
                        title: "Refreshing Repository".into(),
                        description: Some("Fast-forwarding branches".into()),
                        value: 0.9,
                    });
                    let _ = corvane_git::fast_forward_branches(git, &workdir);
                }
                result
            },
            move |result, cx| {
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
            },
        );
    }

    // ---- pull ----

    /// `_pull`
    pub fn pull(id: u64, cx: &mut App) {
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
        let askpass = Self::askpass_env(cx);
        let title = format!("Pulling {}", remote.name);
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
                let result = corvane_git::pull(
                    git.clone(),
                    &workdir,
                    &remote_name,
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
                if result.is_ok() {
                    let _ = corvane_git::update_remote_head(
                        git.clone(),
                        &workdir,
                        &remote_name,
                        askpass.as_ref(),
                    );
                    report(PushPullProgress {
                        kind: PushPullKind::Generic,
                        title: "Refreshing Repository".into(),
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

    // ---- push ----

    /// `_push` (+ `performPush`): publish the branch when it has no upstream.
    pub fn push(id: u64, force_with_lease: bool, branch: Option<String>, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(remote) = Self::current_remote(id, cx) else {
            Self::show_popup(Popup::PublishRepository { repo: id }, cx);
            return;
        };
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
            return;
        }
        let Some(branch) = branch else {
            return;
        };
        if !Self::begin_network(id, cx) {
            return;
        }
        if force_with_lease {
            Self::state(cx).update(cx, |s, _| {
                s.repo_state_mut(id)
                    .force_push_branches
                    .remove(branch.name_without_remote());
            });
        }
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
        let local = branch.name.clone();
        let remote_branch = branch
            .upstream_short()
            .and_then(|u| u.split_once('/').map(|(_, b)| b.to_string()));
        let remote_url = remote.url.clone();
        let retry = RetryAction::Push {
            force_with_lease,
            branch: Some(branch.name.clone()),
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
                    &[],
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
                    let _ = corvane_git::fetch(
                        git.clone(),
                        &workdir,
                        &remote_name,
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
                        title: "Refreshing Repository".into(),
                        description: Some("Fast-forwarding branches".into()),
                        value: 0.9,
                    });
                    let _ = corvane_git::fast_forward_branches(git, &workdir);
                }
                result
            },
            move |result, cx| {
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
        spawn_bg(
            cx,
            move || {
                let client = corvane_github::Client::new(endpoint, token);
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
        spawn_bg(
            cx,
            move || {
                corvane_git::lfs_available(git.clone())
                    && corvane_git::is_using_lfs(git, &workdir)
                    && !corvane_git::lfs_hooks_installed(&workdir)
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
        cx.spawn(async move |cx: &mut AsyncApp| {
            cx.background_executor()
                .timer(Duration::from_secs(45))
                .await;
            loop {
                cx.update(Self::refresh_indicators);
                cx.background_executor()
                    .timer(INDICATOR_REFRESH_INTERVAL)
                    .await;
            }
        })
        .detach();
    }

    /// Fetch the selected GitHub repository when its last fetch is older than
    /// the interval (`shouldBackgroundFetch`).
    fn background_fetch_tick(cx: &mut App) {
        let (id, last_fetched, busy) = {
            let s = Self::state(cx).read(cx);
            let Some(id) = s.selected else { return };
            let Some(repo) = s.repository(id) else { return };
            if repo.github.is_none() {
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

    /// `refreshIndicatorForRepository` for every repository: changed files
    /// and ahead/behind, shown in the repository list.
    pub fn refresh_indicators(cx: &mut App) {
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
                    out.insert(
                        id,
                        RepoIndicator {
                            ahead_behind,
                            changed_files: changed,
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
