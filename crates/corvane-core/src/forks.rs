//! Forks - GHD `ui/forks/create-fork-dialog.tsx` (`CreateFork` popup),
//! `app-store.ts#_convertRepositoryToFork`, `_updateRepositoryWorkflowPreferences`,
//! `git-store.ts#addUpstreamRemoteIfNeeded` / `ensureUpstreamRemoteURL`.
//!
//! Deviation: GHD decides the fork suggestion from the repository's API
//! `permissions`; Corvane offers the fork when a push is refused with
//! "Permission denied" for a GitHub repository the user is signed in to.

use corvane_github::Client;
use corvane_models::{ForkContributionTarget, GitHubRepository, url_matches_remote};
use gpui_kit::App;
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::Popup;

/// `UpstreamRemoteName`
pub const UPSTREAM_REMOTE_NAME: &str = "upstream";

impl Dispatcher {
    /// `_showCreateForkDialog`: only with an account for the repository.
    pub fn show_create_fork_dialog(id: u64, cx: &mut App) {
        let ok = {
            let s = Self::state(cx).read(cx);
            s.repository(id)
                .and_then(|r| r.github.as_ref())
                .is_some_and(|gh| s.account_for(&gh.endpoint).is_some())
        };
        if ok {
            Self::show_popup(Popup::CreateFork { repo: id }, cx);
        }
    }

    /// `CreateForkDialog.onSubmit` + `_convertRepositoryToFork`: fork through
    /// the API, point `origin` at the fork, keep the original as `upstream`,
    /// then ask how the fork will be used. `then` gets the API error, if any.
    pub fn create_fork(
        id: u64,
        then: impl FnOnce(Option<String>, &mut App) + 'static,
        cx: &mut App,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            then(Some("The repository is not available.".into()), cx);
            return;
        };
        let (github, remote) = {
            let s = Self::state(cx).read(cx);
            (
                s.repository(id).and_then(|r| r.github.clone()),
                Self::current_remote_in(s, id),
            )
        };
        let (Some(github), Some(remote)) = (github, remote) else {
            then(Some("The repository has no GitHub remote.".into()), cx);
            return;
        };
        let Some((endpoint, token, _)) = Self::api_for(&github, cx) else {
            then(Some("You are not signed in to GitHub.".into()), cx);
            return;
        };
        let original = github.clone();
        spawn_bg(
            cx,
            move || -> Result<GitHubRepository, String> {
                let client = Client::new(endpoint, token);
                let mut fork = client
                    .fork_repository(&original.owner, &original.name)
                    .map_err(|err| err.to_string())?;
                if fork.parent.is_none() {
                    fork.parent = Some(Box::new(original.clone()));
                }
                // `setRemoteURL(defaultRemote, fork.clone_url)` then
                // `ensureUpstreamRemoteURL(originalUrl)`
                corvane_git::set_remote_url(git.clone(), &workdir, &remote.name, &fork.clone_url)
                    .map_err(|err| err.to_string())?;
                if let Err(err) = corvane_git::add_remote(
                    git.clone(),
                    &workdir,
                    UPSTREAM_REMOTE_NAME,
                    &remote.url,
                ) && let Err(err) = corvane_git::set_remote_url(
                    git.clone(),
                    &workdir,
                    UPSTREAM_REMOTE_NAME,
                    &remote.url,
                )
                .map_err(|_| err)
                {
                    warn!(%err, "could not set the upstream remote");
                }
                Ok(fork)
            },
            move |result, cx| match result {
                Ok(fork) => {
                    info!(id, fork = %fork.full_name(), "repository converted to a fork");
                    Self::state(cx).update(cx, |s, cx| {
                        if let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) {
                            repo.github = Some(fork);
                            repo.fork_contribution_target = None;
                        }
                        crate::dispatcher::persist_repositories(s);
                        cx.notify();
                    });
                    Self::refresh_repository(id, cx);
                    then(None, cx);
                    Self::show_popup(Popup::ChooseForkSettings { repo: id }, cx);
                }
                Err(err) => then(Some(err), cx),
            },
        );
    }

    /// `_updateRepositoryWorkflowPreferences({ forkContributionTarget })`
    pub fn set_fork_contribution_target(id: u64, target: ForkContributionTarget, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) {
                repo.fork_contribution_target = Some(target);
            }
            crate::dispatcher::persist_repositories(s);
            cx.notify();
        });
        // the pull request / issue target changed with it
        Self::ensure_pull_requests(id, cx);
    }

    /// `addUpstreamRemoteIfNeeded`: a fork gets an `upstream` remote for its
    /// parent unless one of the remotes already points there (or the name
    /// is taken by something else).
    pub(crate) fn add_upstream_remote_if_needed(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (parent_url, remotes) = {
            let s = Self::state(cx).read(cx);
            let Some(parent) = s
                .repository(id)
                .and_then(|r| r.github.as_ref())
                .and_then(|gh| gh.parent.as_ref())
            else {
                return;
            };
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            if rs.upstream_checked {
                return;
            }
            (
                parent.clone_url.clone(),
                rs.info
                    .as_ref()
                    .map(|i| i.remotes.clone())
                    .unwrap_or_default(),
            )
        };
        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).upstream_checked = true);
        if remotes
            .iter()
            .any(|r| url_matches_remote(&r.url, &parent_url))
        {
            return;
        }
        if remotes.iter().any(|r| r.name == UPSTREAM_REMOTE_NAME) {
            warn!(
                id,
                "a remote named upstream already exists and does not point at the parent"
            );
            return;
        }
        spawn_bg(
            cx,
            move || corvane_git::add_remote(git, &workdir, UPSTREAM_REMOTE_NAME, &parent_url),
            move |result, cx| match result {
                Ok(()) => Self::refresh_repository(id, cx),
                Err(err) => warn!(id, %err, "could not add the upstream remote"),
            },
        );
    }
}
