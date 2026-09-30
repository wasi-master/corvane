//! Pull requests: the per-GitHub-repository cache (GHD
//! `lib/stores/pull-request-store.ts` and `pull-request-coordinator.ts`),
//! the 30-minute updater (`stores/helpers/pull-request-updater.ts`), the
//! association with the current branch (`lib/helpers/pull-request-matching.ts`)
//! and the pull request checkout (`app-store.ts#_checkoutPullRequest`,
//! `_findPullRequestBranch`).
//!
//! Deviation: GHD keeps the pull requests in IndexedDB keyed by the GitHub
//! repository's database id; Corvane keys the redb entry by endpoint +
//! `owner/name` and embeds the head/base repositories in each record.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use corvane_github::{ApiPullRequest, Client, GitHubError};
use corvane_models::{
    Branch, BranchKind, GitHubRepository, PullRequest, PullRequestRef, Remote, url_matches_remote,
};
use gpui_kit::{App, AsyncApp};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::AppState;

/// `PullRequestInterval`: check for new or updated pull requests every 30 minutes.
const PULL_REQUEST_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// `appIsFocused`, as far as the pull request updater cares.
static APP_FOCUSED: AtomicBool = AtomicBool::new(false);
/// Bumped to stop the running updater (`stopPullRequestUpdater`).
static UPDATER_GENERATION: AtomicU64 = AtomicU64::new(0);
/// `MaxPullRequestRefreshFrequency`: never more often than every 2 minutes.
const MAX_REFRESH_FREQUENCY: Duration = Duration::from_secs(2 * 60);
/// `fetchUpdatedPullRequests(maxResults)`: past this many updated PRs the
/// open list is refetched from scratch.
const MAX_UPDATED_RESULTS: usize = 320;
/// `ForkedRemotePrefix`: remotes Desktop adds to check out PRs from forks.
pub const FORKED_REMOTE_PREFIX: &str = "github-desktop-";

/// `forkPullRequestRemoteName`
pub fn fork_pull_request_remote_name(owner: &str) -> String {
    format!("{FORKED_REMOTE_PREFIX}{owner}")
}

/// `BranchesTab`: the tab shown in the branch foldout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BranchesTab {
    #[default]
    Branches,
    PullRequests,
}

/// The open pull requests of one GitHub repository.
#[derive(Debug, Default)]
pub struct PullRequestCache {
    /// Open pull requests, newest number first.
    pub pull_requests: Vec<PullRequest>,
    pub loading: bool,
    loaded: bool,
    pub last_refreshed: Option<Instant>,
    /// `pullRequestsLastUpdated`: the newest `updated_at` seen.
    last_updated: Option<String>,
}

pub type PullRequestCaches = HashMap<String, PullRequestCache>;

#[derive(Serialize, Deserialize)]
struct PersistedPullRequests {
    pull_requests: Vec<PullRequest>,
    last_updated: Option<String>,
}

/// Cache key of a GitHub repository (endpoint + lower-cased `owner/name`).
pub fn cache_key(gh: &GitHubRepository) -> String {
    format!(
        "{}/{}/{}",
        gh.endpoint,
        gh.owner.to_lowercase(),
        gh.name.to_lowercase()
    )
}

fn store_key(key: &str) -> String {
    format!("pull-requests:{key}")
}

/// `IAPIPullRequest` → `PullRequest` (+ whether it is still open).
pub(crate) fn convert_pull_request(client: &Client, pr: ApiPullRequest) -> (PullRequest, bool) {
    let open = pr.state == "open";
    let convert_ref = |r: corvane_github::api::ApiPullRequestRef| PullRequestRef {
        ref_name: r.ref_name,
        sha: r.sha,
        repository: r.repo.map(|repo| client.convert(repo)),
    };
    (
        PullRequest {
            number: pr.number,
            title: pr.title,
            created_at: pr.created_at,
            updated_at: pr.updated_at,
            head: convert_ref(pr.head),
            base: convert_ref(pr.base),
            author: pr.user.login,
            draft: pr.draft,
            body: pr.body.unwrap_or_default(),
        },
        open,
    )
}

/// `findAssociatedPullRequest`: the open PR whose head is the branch's
/// upstream in the matching remote.
pub fn find_associated_pull_request<'a>(
    branch: &Branch,
    pull_requests: &'a [PullRequest],
    remotes: &[Remote],
) -> Option<&'a PullRequest> {
    let upstream = branch.upstream_short()?;
    let (remote_name, ref_name) = upstream.split_once('/')?;
    let remote = remotes.iter().find(|r| r.name == remote_name)?;
    pull_requests.iter().find(|pr| {
        pr.head.ref_name == ref_name
            && pr
                .head
                .repository
                .as_ref()
                .is_some_and(|r| url_matches_remote(&r.clone_url, &remote.url))
    })
}

/// `findForkedRemotesToPrune`: Desktop-added fork remotes no open PR and
/// no local branch still uses.
pub fn forked_remotes_to_prune(
    remotes: &[Remote],
    pull_requests: &[PullRequest],
    branches: &[Branch],
) -> Vec<String> {
    remotes
        .iter()
        .filter(|r| r.name.starts_with(FORKED_REMOTE_PREFIX))
        .filter(|r| {
            !pull_requests.iter().any(|pr| {
                pr.head
                    .repository
                    .as_ref()
                    .is_some_and(|h| url_matches_remote(&h.clone_url, &r.url))
            })
        })
        .filter(|r| {
            !branches
                .iter()
                .any(|b| b.upstream_remote_name() == Some(r.name.as_str()))
        })
        .map(|r| r.name.clone())
        .collect()
}

impl AppState {
    /// The cached open pull requests of a repository (its fork target).
    pub fn pull_requests_for(&self, id: u64) -> &[PullRequest] {
        self.repository(id)
            .and_then(|r| r.non_fork_github())
            .and_then(|gh| self.pull_requests.get(&cache_key(gh)))
            .map(|c| c.pull_requests.as_slice())
            .unwrap_or(&[])
    }

    /// `isLoadingPullRequests`
    pub fn pull_requests_loading(&self, id: u64) -> bool {
        self.repository(id)
            .and_then(|r| r.non_fork_github())
            .and_then(|gh| self.pull_requests.get(&cache_key(gh)))
            .is_some_and(|c| c.loading)
    }

    /// `branchesState.currentPullRequest`
    pub fn current_pull_request(&self, id: u64) -> Option<&PullRequest> {
        let info = self.repo_states.get(&id)?.info.as_ref()?;
        let branch = info.current_branch()?;
        find_associated_pull_request(branch, self.pull_requests_for(id), &info.remotes)
    }
}

/// What the checkout found or created for a pull request.
struct PullRequestBranch {
    branch: Branch,
}

impl Dispatcher {
    /// `_changeBranchesTab`
    pub fn change_branches_tab(tab: BranchesTab, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if s.branches_tab != tab {
                s.branches_tab = tab;
                cx.notify();
            }
        });
    }

    /// Load the persisted list (once) and refresh it if it is stale.
    pub fn ensure_pull_requests(id: u64, cx: &mut App) {
        Self::refresh_pull_requests(id, false, cx);
    }

    /// `refreshPullRequests`: the first fetch takes every open pull
    /// request, later ones ask for everything updated since the newest
    /// cached one and drop what closed. `force` skips the 2-minute throttle
    /// (the list's refresh button).
    pub fn refresh_pull_requests(id: u64, force: bool, cx: &mut App) {
        let Some(target) = Self::state(cx)
            .read(cx)
            .repository(id)
            .and_then(|r| r.non_fork_github().cloned())
        else {
            return;
        };
        let key = cache_key(&target);
        let (skip, since) = Self::state(cx).update(cx, |s, cx| {
            let store = s.store.clone();
            let cache = s.pull_requests.entry(key.clone()).or_default();
            if !cache.loaded {
                cache.loaded = true;
                if let Ok(Some(persisted)) = store.get::<PersistedPullRequests>(&store_key(&key)) {
                    cache.pull_requests = persisted.pull_requests;
                    cache.last_updated = persisted.last_updated;
                }
                cx.notify();
            }
            if cache.loading
                || (!force
                    && cache
                        .last_refreshed
                        .is_some_and(|t| t.elapsed() < MAX_REFRESH_FREQUENCY))
            {
                return (true, None);
            }
            cache.loading = true;
            cache.last_refreshed = Some(Instant::now());
            cx.notify();
            (false, cache.last_updated.clone())
        });
        if skip {
            return;
        }
        let Some((endpoint, token, _)) = Self::api_for(&target, cx) else {
            Self::state(cx).update(cx, |s, cx| {
                if let Some(c) = s.pull_requests.get_mut(&key) {
                    c.loading = false;
                }
                cx.notify();
            });
            return;
        };
        let (owner, name) = (target.owner.clone(), target.name.clone());
        let api_base = target.endpoint.clone();
        spawn_bg(
            cx,
            move || {
                let client = Client::new(endpoint, token);
                let fetched = match since.as_deref() {
                    Some(since) => {
                        match client.pull_requests_updated_since(
                            &owner,
                            &name,
                            since,
                            MAX_UPDATED_RESULTS,
                        ) {
                            Ok(Some(prs)) => Ok((prs, false)),
                            Ok(None) => client.open_pull_requests(&owner, &name).map(|p| (p, true)),
                            Err(err) => Err(err),
                        }
                    }
                    None => client.open_pull_requests(&owner, &name).map(|p| (p, true)),
                };
                fetched
                    .map(|(prs, full)| {
                        let prs: Vec<(PullRequest, bool)> = prs
                            .into_iter()
                            .map(|pr| convert_pull_request(&client, pr))
                            .collect();
                        (prs, full)
                    })
                    .map_err(|err| (matches!(err, GitHubError::Auth(_)), err.to_string()))
            },
            move |result, cx| {
                let auth_failed = Self::state(cx).update(cx, |s, cx| {
                    let store = s.store.clone();
                    let cache = s.pull_requests.entry(key.clone()).or_default();
                    cache.loading = false;
                    let mut auth_failed = false;
                    match result {
                        Ok((fetched, full)) => {
                            if full {
                                cache.pull_requests.clear();
                            }
                            let mut newest = cache.last_updated.clone();
                            for (pr, open) in fetched {
                                if newest.as_deref().is_none_or(|n| pr.updated_at.as_str() > n) {
                                    newest = Some(pr.updated_at.clone());
                                }
                                cache.pull_requests.retain(|p| p.number != pr.number);
                                if open && pr.head.repository.is_some() {
                                    cache.pull_requests.push(pr);
                                }
                            }
                            cache
                                .pull_requests
                                .sort_by_key(|p| std::cmp::Reverse(p.number));
                            cache.last_updated = newest;
                            let persisted = PersistedPullRequests {
                                pull_requests: cache.pull_requests.clone(),
                                last_updated: cache.last_updated.clone(),
                            };
                            if let Err(err) = store.set(&store_key(&key), &persisted) {
                                warn!(%err, "could not persist pull requests");
                            }
                            info!(count = cache.pull_requests.len(), "pull requests refreshed");
                        }
                        Err((auth, err)) => {
                            warn!(%err, "could not refresh pull requests");
                            auth_failed = auth;
                        }
                    }
                    cx.notify();
                    auth_failed
                });
                if auth_failed {
                    Self::token_invalidated(&api_base, cx);
                }
                Self::prune_forked_remotes(id, cx);
                Self::subscribe_current_pull_request_status(id, cx);
            },
        );
    }

    /// Launch: the window starts focused, so start the updater for the
    /// selected repository.
    pub fn start_pull_request_updater(cx: &mut App) {
        APP_FOCUSED.store(true, Ordering::Relaxed);
        Self::restart_pull_request_updater(cx);
    }

    /// `_setAppFocusState`: the updater only runs while the app is focused.
    pub fn set_app_focus_state(focused: bool, cx: &mut App) {
        if APP_FOCUSED.swap(focused, Ordering::Relaxed) == focused {
            return;
        }
        if focused {
            Self::restart_pull_request_updater(cx);
        } else {
            UPDATER_GENERATION.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// `startPullRequestUpdater` / `PullRequestUpdater`: stops the running
    /// updater and, while the app is focused, starts one for the selected
    /// repository. The first tick comes once the last refresh is two
    /// minutes old (`MaxPullRequestRefreshFrequency`), then every 30 minutes.
    pub(crate) fn restart_pull_request_updater(cx: &mut App) {
        let generation = UPDATER_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        if !APP_FOCUSED.load(Ordering::Relaxed) {
            return;
        }
        let Some(id) = Self::state(cx).read(cx).selected else {
            return;
        };
        if Self::pull_requests_last_refreshed(id, cx).is_none() {
            return;
        }
        let running = move || UPDATER_GENERATION.load(Ordering::Relaxed) == generation;
        cx.spawn(async move |cx: &mut AsyncApp| {
            let mut timeout = MAX_REFRESH_FREQUENCY;
            loop {
                // `scheduleTick`: due = timeout − time since the last refresh
                let Some(last) = cx.update(|cx| Self::pull_requests_last_refreshed(id, cx)) else {
                    return;
                };
                let since = last.map(|t| t.elapsed());
                let due = since.map_or(Duration::ZERO, |since| timeout.saturating_sub(since));
                cx.background_executor().timer(due).await;
                if !running() {
                    return;
                }
                // `tick`
                let Some(last) = cx.update(|cx| Self::pull_requests_last_refreshed(id, cx)) else {
                    return;
                };
                let since = last.map(|t| t.elapsed());
                timeout = PULL_REQUEST_INTERVAL;
                if since.is_some_and(|since| since < MAX_REFRESH_FREQUENCY) {
                    continue;
                }
                cx.update(|cx| Self::refresh_pull_requests(id, false, cx));
            }
        })
        .detach();
    }

    /// `getLastRefreshed`: `None` when `id` has no GitHub repository,
    /// `Some(None)` when its pull requests were never refreshed.
    fn pull_requests_last_refreshed(id: u64, cx: &App) -> Option<Option<Instant>> {
        let s = Self::state(cx).read(cx);
        let gh = s.repository(id)?.non_fork_github()?;
        Some(
            s.pull_requests
                .get(&cache_key(gh))
                .and_then(|c| c.last_refreshed),
        )
    }

    /// `_showPullRequestByPR`: the pull request page in the browser.
    pub fn open_pull_request(pr: &PullRequest, cx: &mut App) {
        if let Some(url) = pr.html_url() {
            Self::open_url(&url, cx);
        }
    }

    /// "Switch to Pull Request" in the notification dialogs: close the
    /// dialog, `selectRepository`, then `checkoutPullRequest` once the
    /// repository's branches and remotes are loaded.
    pub fn switch_to_pull_request(id: u64, pr: PullRequest, cx: &mut App) {
        Self::close_popup(cx);
        Self::select_repository(id, cx);
        let loaded = move |cx: &App| {
            Self::state(cx)
                .read(cx)
                .repo_states
                .get(&id)
                .is_some_and(|rs| rs.info.is_some())
        };
        if loaded(cx) {
            Self::checkout_pull_request(id, pr, cx);
            return;
        }
        cx.spawn(async move |cx| {
            for _ in 0..100 {
                if cx.update(|cx| loaded(cx)) {
                    break;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
            }
            cx.update(|cx| {
                if loaded(cx) {
                    Self::checkout_pull_request(id, pr, cx);
                }
            });
        })
        .detach();
    }

    /// `_showPullRequest`: the current branch's pull request in the browser.
    pub fn show_pull_request(id: u64, cx: &mut App) {
        let pr = Self::state(cx).read(cx).current_pull_request(id).cloned();
        if let Some(pr) = pr {
            Self::open_pull_request(&pr, cx);
        }
    }

    /// `_checkoutPullRequest`: find the pull request's branch
    /// (`find_pull_request_branch`), then check it out like any other.
    pub fn checkout_pull_request(id: u64, pr: PullRequest, cx: &mut App) {
        Self::find_pull_request_branch(id, pr, cx, move |result, cx| match result {
            Ok(branch) => Self::checkout_branch(id, branch.name, None, cx),
            Err(message) => Self::show_error("Could not check out the pull request", message, cx),
        });
    }

    /// `startCherryPickWithPullRequest` (`onDropOntoPullRequest`): commits
    /// dropped on a pull request in the Pull Requests tab are cherry-picked
    /// onto its branch, found like a checkout would (fetching a fork remote
    /// and creating `pr/<n>` when needed). GHD only logs when the branch
    /// cannot be determined; Corvane also says so.
    pub fn cherry_pick_to_pull_request(id: u64, pr: PullRequest, cx: &mut App) {
        Self::find_pull_request_branch(id, pr, cx, move |result, cx| match result {
            Ok(branch) => Self::cherry_pick_to_branch(id, branch.name, cx),
            Err(message) => {
                Self::end_mco(id, cx);
                // `306-cherry-pick-pr-branch-error`: GHD only logs
                if Self::state(cx)
                    .read(cx)
                    .flags
                    .bool(crate::flags::ids::CHERRY_PICK_PR_BRANCH_ERROR)
                {
                    Self::show_error(
                        "Could not cherry-pick onto the pull request",
                        format!("Could not determine the pull request's branch: {message}"),
                        cx,
                    );
                } else {
                    warn!(%message, "could not determine the pull request's branch");
                }
            }
        });
    }

    /// `_findPullRequestBranch`: find the remote that hosts the PR head
    /// (adding a `github-desktop-<owner>` remote for forks), fetch it if the
    /// branch is unknown, create `pr/<n>` for fork branches, record the
    /// branch in the repository state and hand it to `on_found`.
    fn find_pull_request_branch(
        id: u64,
        pr: PullRequest,
        cx: &mut App,
        on_found: impl FnOnce(Result<Branch, String>, &mut App) + 'static,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(head_repo) = pr.head.repository.clone() else {
            on_found(
                Err("The pull request's head repository no longer exists.".to_string()),
                cx,
            );
            return;
        };
        let (remotes, branches, default_remote, has_parent) = {
            let s = Self::state(cx).read(cx);
            let info = s.repo_states.get(&id).and_then(|rs| rs.info.as_ref());
            (
                info.map(|i| i.remotes.clone()).unwrap_or_default(),
                info.map(|i| i.branches.clone()).unwrap_or_default(),
                Self::current_remote_in(s, id).map(|r| r.name),
                s.repository(id)
                    .and_then(|r| r.github.as_ref())
                    .is_some_and(|gh| gh.parent.is_some()),
            )
        };
        let askpass = Self::askpass_env(cx);
        let number = pr.number;
        let head_ref = pr.head.ref_name.clone();
        let head_url = head_repo.clone_url.clone();
        let head_owner = head_repo.owner.clone();
        spawn_bg(
            cx,
            move || -> Result<PullRequestBranch, String> {
                let remote = match remotes
                    .iter()
                    .find(|r| url_matches_remote(&head_url, &r.url))
                    .cloned()
                {
                    Some(remote) => remote,
                    None => {
                        let name = fork_pull_request_remote_name(&head_owner);
                        corvane_git::add_remote(git.clone(), &workdir, &name, &head_url).map_err(
                            |err| format!("Couldn't find PR branch, adding remote failed: {err}"),
                        )?;
                        Remote {
                            name,
                            url: head_url.clone(),
                        }
                    }
                };
                let remote_ref = format!("{}/{}", remote.name, head_ref);
                if let Some(local) = branches.iter().find(|b| {
                    b.kind == BranchKind::Local && b.upstream_short() == Some(remote_ref.as_str())
                }) {
                    return Ok(PullRequestBranch {
                        branch: local.clone(),
                    });
                }
                let find_remote_branch = |branches: &[Branch]| {
                    branches
                        .iter()
                        .find(|b| b.kind == BranchKind::Remote && b.name == remote_ref)
                        .cloned()
                };
                let mut existing = find_remote_branch(&branches);
                if existing.is_none() {
                    if let Err(err) = corvane_git::fetch(
                        git.clone(),
                        &workdir,
                        &remote.name,
                        askpass.as_ref(),
                        &mut |_, _| {},
                    ) {
                        warn!(%err, remote = %remote.name, "failed fetching remote");
                    }
                    existing = corvane_git::open_repository(&workdir)
                        .ok()
                        .and_then(|info| find_remote_branch(&info.branches));
                }
                let Some(existing) = existing else {
                    return Err(format!(
                        "Couldn't find branch '{head_ref}' in remote '{}'. A common reason for \
                         this is that the PR author has deleted their branch or their forked \
                         repository.",
                        remote.name
                    ));
                };
                let is_fork_remote = Some(remote.name.as_str()) != default_remote.as_deref()
                    && !(has_parent && remote.name == "upstream");
                if !is_fork_remote {
                    return Ok(PullRequestBranch { branch: existing });
                }
                let name = format!("pr/{number}");
                corvane_git::create_branch(git.clone(), &workdir, &name, Some(&remote_ref), false)
                    .map_err(|err| err.to_string())?;
                let branch = corvane_git::open_repository(&workdir)
                    .ok()
                    .and_then(|info| {
                        info.branches
                            .into_iter()
                            .find(|b| b.kind == BranchKind::Local && b.name == name)
                    })
                    .ok_or_else(|| format!("Could not create the branch {name}."))?;
                Ok(PullRequestBranch { branch })
            },
            move |result, cx| match result {
                Ok(found) => {
                    let branch = found.branch.clone();
                    Self::state(cx).update(cx, |s, cx| {
                        if let Some(info) = s.repo_state_mut(id).info.as_mut()
                            && !info
                                .branches
                                .iter()
                                .any(|b| b.name == found.branch.name && b.kind == found.branch.kind)
                        {
                            info.branches.push(found.branch);
                            cx.notify();
                        }
                    });
                    on_found(Ok(branch), cx);
                }
                Err(message) => on_found(Err(message), cx),
            },
        );
    }

    /// `pruneForkedRemotes`: drop Desktop-added fork remotes nothing uses.
    fn prune_forked_remotes(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let to_prune = {
            let s = Self::state(cx).read(cx);
            let Some(info) = s.repo_states.get(&id).and_then(|rs| rs.info.as_ref()) else {
                return;
            };
            forked_remotes_to_prune(&info.remotes, s.pull_requests_for(id), &info.branches)
        };
        if to_prune.is_empty() {
            return;
        }
        spawn_bg(
            cx,
            move || {
                for name in &to_prune {
                    if let Err(err) = corvane_git::remove_remote(git.clone(), &workdir, name) {
                        warn!(%err, remote = %name, "could not prune fork remote");
                    }
                }
            },
            move |_, cx| Self::refresh_repository(id, cx),
        );
    }

    /// An API call answered 401: the token was revoked (`InvalidatedToken`).
    /// The account is signed out and the user offered to sign in again.
    pub(crate) fn token_invalidated(api_base: &str, cx: &mut App) {
        let account = Self::state(cx).read(cx).account_for(api_base).cloned();
        let Some(account) = account else {
            return;
        };
        warn!(login = %account.login, endpoint = %api_base, "account token invalidated");
        Self::sign_out(api_base.to_string(), cx);
        Self::show_popup(crate::state::Popup::InvalidatedToken { account }, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gh(owner: &str, name: &str) -> GitHubRepository {
        GitHubRepository {
            endpoint: "https://api.github.com".into(),
            owner: owner.into(),
            name: name.into(),
            html_url: format!("https://github.com/{owner}/{name}"),
            clone_url: format!("https://github.com/{owner}/{name}.git"),
            default_branch: None,
            private: false,
            fork: false,
            parent: None,
            archived: false,
            permissions: None,
        }
    }

    fn pr(number: u64, head_ref: &str, head: GitHubRepository) -> PullRequest {
        PullRequest {
            number,
            title: "t".into(),
            created_at: String::new(),
            updated_at: String::new(),
            head: PullRequestRef {
                ref_name: head_ref.into(),
                sha: String::new(),
                repository: Some(head),
            },
            base: PullRequestRef {
                ref_name: "main".into(),
                sha: String::new(),
                repository: Some(gh("octocat", "hello")),
            },
            author: "octocat".into(),
            draft: false,
            body: String::new(),
        }
    }

    fn branch(name: &str, upstream: Option<&str>, kind: BranchKind) -> Branch {
        Branch {
            name: name.into(),
            kind,
            full_name: format!("refs/heads/{name}"),
            tip: None,
            upstream: upstream.map(|u| format!("refs/remotes/{u}")),
            tip_time: None,
        }
    }

    #[test]
    fn associates_the_current_branch_with_its_pull_request() {
        let remotes = vec![Remote {
            name: "origin".into(),
            url: "git@github.com:octocat/hello.git".into(),
        }];
        let prs = vec![
            pr(1, "feature", gh("octocat", "hello")),
            pr(2, "feature", gh("someone", "hello")),
        ];
        let b = branch("feature", Some("origin/feature"), BranchKind::Local);
        assert_eq!(
            find_associated_pull_request(&b, &prs, &remotes).map(|p| p.number),
            Some(1)
        );
        let unpublished = branch("feature", None, BranchKind::Local);
        assert!(find_associated_pull_request(&unpublished, &prs, &remotes).is_none());
    }

    #[test]
    fn prunes_unused_fork_remotes() {
        let remotes = vec![
            Remote {
                name: "github-desktop-alice".into(),
                url: "https://github.com/alice/hello.git".into(),
            },
            Remote {
                name: "github-desktop-bob".into(),
                url: "https://github.com/bob/hello.git".into(),
            },
            Remote {
                name: "github-desktop-carol".into(),
                url: "https://github.com/carol/hello.git".into(),
            },
            Remote {
                name: "origin".into(),
                url: "https://github.com/octocat/hello.git".into(),
            },
        ];
        let prs = vec![pr(3, "x", gh("alice", "hello"))];
        let branches = vec![branch(
            "pr/4",
            Some("github-desktop-bob/x"),
            BranchKind::Local,
        )];
        assert_eq!(
            forked_remotes_to_prune(&remotes, &prs, &branches),
            vec!["github-desktop-carol".to_string()]
        );
    }
}
