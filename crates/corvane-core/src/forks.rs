//! Forks - GHD `ui/forks/create-fork-dialog.tsx` (`CreateFork` popup),
//! `app-store.ts#_convertRepositoryToFork`, `_updateRepositoryWorkflowPreferences`,
//! `git-store.ts#addUpstreamRemoteIfNeeded` / `ensureUpstreamRemoteURL` /
//! `updateExistingUpstreamRemote`, and the `UpstreamAlreadyExists` popup's
//! Update / Ignore (`app-store.ts#_updateExistingUpstreamRemote`,
//! `_ignoreExistingUpstreamRemote`).
//!
//! The repository's API record, with the user's `permissions`, is refreshed
//! on selection and after signing in (`repositoryWithRefreshedGitHubRepository`)
//! and persisted with the repository. Without write access the commit form
//! suggests a fork and a push opens `CreateFork` before git runs (GHD runs the
//! push and offers the fork when it fails authentication,
//! `insufficientGitHubRepoPermissions`). Repositories whose permissions are
//! still unknown fall back to offering the fork after a push refused with
//! "Permission denied".

use corvane_github::Client;
use corvane_models::{ForkContributionTarget, GitHubRepository, url_matches_remote};
use gpui_kit::App;
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::Popup;

/// `UpstreamRemoteName`
pub const UPSTREAM_REMOTE_NAME: &str = "upstream";

/// GHD `getIgnoreExistingUpstreamRemoteKey` (a localStorage key there, a
/// store key here).
fn ignore_existing_upstream_key(id: u64) -> String {
    format!("repository/{id}/ignoreExistingUpstreamRemote")
}

impl Dispatcher {
    /// `repositoryWithRefreshedGitHubRepository`: re-read the repository's
    /// API record (parent, default branch, `permissions`) with the account
    /// for its endpoint and persist it. A failed request keeps what is stored.
    pub fn refresh_github_repository(id: u64, cx: &mut App) {
        let Some(github) = Self::state(cx)
            .read(cx)
            .repository(id)
            .and_then(|r| r.github.clone())
        else {
            return;
        };
        let Some((endpoint, token, _)) = Self::api_for(&github, cx) else {
            return;
        };
        let (owner, name) = (github.owner.clone(), github.name.clone());
        spawn_bg(
            cx,
            move || Client::new(endpoint, token).repository(&owner, &name),
            move |result, cx| match result {
                Ok(fresh) => {
                    let changed = Self::state(cx).update(cx, |s, cx| {
                        let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) else {
                            return false;
                        };
                        if repo.github.as_ref() == Some(&fresh) {
                            return false;
                        }
                        info!(id, permissions = ?fresh.permissions, "refreshed GitHub repository");
                        repo.github = Some(fresh);
                        crate::dispatcher::persist_repositories(s);
                        cx.notify();
                        true
                    });
                    if changed {
                        Self::refresh_branch_protection(id, cx);
                    }
                }
                Err(err) => warn!(id, %err, "could not refresh the GitHub repository"),
            },
        );
    }

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
        // `getIgnoreExistingUpstreamRemote`: the user picked Ignore once
        let ignored = Self::state(cx)
            .read(cx)
            .store
            .get::<bool>(&ignore_existing_upstream_key(id))
            .ok()
            .flatten()
            .unwrap_or(false);
        if ignored {
            return;
        }
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
        if let Some(existing) = remotes.iter().find(|r| r.name == UPSTREAM_REMOTE_NAME) {
            // `UpstreamAlreadyExistsError` → `upstreamAlreadyExistsHandler`
            Self::show_popup(
                Popup::UpstreamAlreadyExists {
                    repo: id,
                    existing_url: existing.url.clone(),
                },
                cx,
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

    /// `UpstreamAlreadyExists` › Update (`updateExistingUpstreamRemote`):
    /// point `upstream` at the parent's clone URL.
    pub fn update_existing_upstream_remote(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(parent_url) = Self::state(cx)
            .read(cx)
            .repository(id)
            .and_then(|r| r.github.as_ref())
            .and_then(|gh| gh.parent.as_ref())
            .map(|p| p.clone_url.clone())
        else {
            return;
        };
        spawn_bg(
            cx,
            move || corvane_git::set_remote_url(git, &workdir, UPSTREAM_REMOTE_NAME, &parent_url),
            move |result, cx| {
                if let Err(err) = result {
                    Self::show_error("Could not update the upstream remote", err.to_string(), cx);
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// `UpstreamAlreadyExists` › Ignore: never check this repository again.
    pub fn ignore_existing_upstream_remote(id: u64, cx: &mut App) {
        let result = Self::state(cx)
            .read(cx)
            .store
            .set(&ignore_existing_upstream_key(id), &true);
        if let Err(err) = result {
            warn!(id, %err, "could not remember to ignore the upstream remote");
        }
    }
}
