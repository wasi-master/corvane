//! Alive subscriptions and the events they deliver - GHD
//! `lib/stores/alive-store.ts` (one subscription per signed-in GitHub.com
//! account while notifications are enabled) and the Alive half of
//! `lib/stores/notifications-store.ts` (`handleAliveEvent`: only events for
//! the selected repository's pull requests that are in the pull request
//! cache; the review / comment / checks are fetched from the API, then the
//! notification goes through `Dispatcher::notify_pull_request_event`).
//!
//! Each subscription is a thread running `corvane_github::alive::run_session`;
//! events cross to the foreground through an `async_channel`. GitHub
//! Enterprise has no Alive service (`supportsAliveSessions` is dotcom only).
//!
//! Testing: `CORVANE_POPUP=alive:<review|comment|checks-failed>[:api]`
//! feeds a sample event for the selected repository through the same
//! handler, with sample review / comment / checks in place of the API
//! (`:api` fetches for real, which needs an account).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use corvane_github::alive::{AliveEvent, CommentSubtype};
use corvane_github::api::ApiPullRequestReviewState;
use corvane_github::{Client, Endpoint};
use corvane_models::{Account, GitHubRepository, RefCheck};
use gpui_kit::{App, AsyncApp};
use tracing::{debug, info, warn};

use crate::dispatcher::Dispatcher;
use crate::notifications::{
    NotificationKind, PullRequestNotification, TestNotificationType, is_valid_notification_review,
};
use crate::remote::spawn_bg;

/// A running subscription; dropping it stops the thread.
pub struct AliveHandle {
    stop: Arc<AtomicBool>,
    pub login: String,
}

impl Drop for AliveHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// `AliveStore` + the dedup sets of `NotificationsStore`.
#[derive(Default)]
pub struct AliveState {
    /// Endpoint → subscription.
    pub sessions: HashMap<String, AliveHandle>,
    sender: Option<async_channel::Sender<AliveEvent>>,
    /// `skipCommitShas`: commits already judged (not the user's, or unknown).
    pub skip_commit_shas: HashSet<String>,
    /// `skipCheckRuns`: check runs a notification was already shown for.
    pub skip_check_runs: HashSet<u64>,
}

/// Where the review / comment / checks of an event come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliveEventData {
    /// The API, as in GHD.
    Api,
    /// `corvane_core::samples` (the dev hook: no account needed).
    Sample,
}

impl Dispatcher {
    /// Start delivering Alive events to the foreground and subscribe for the
    /// signed-in accounts. Call once at launch; `sync_alive_subscriptions`
    /// follows account and settings changes afterwards.
    pub fn start_alive(cx: &mut App) {
        let (tx, rx) = async_channel::unbounded::<AliveEvent>();
        Self::state(cx).update(cx, |s, _| s.alive.sender = Some(tx));
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok(event) = rx.recv().await {
                cx.update(|cx| Self::handle_alive_event(event, AliveEventData::Api, cx));
            }
        })
        .detach();
        Self::sync_alive_subscriptions(cx);
    }

    /// `AliveStore.setEnabled` + `subscribeToAccounts`: one subscription per
    /// GitHub.com account while notifications are enabled, none otherwise.
    pub fn sync_alive_subscriptions(cx: &mut App) {
        let state = Self::state(cx);
        let (enabled, accounts, sender) = {
            let s = state.read(cx);
            (
                s.settings.notifications_enabled,
                s.accounts
                    .iter()
                    .filter(|a| a.is_dotcom())
                    .cloned()
                    .collect::<Vec<Account>>(),
                s.alive.sender.clone(),
            )
        };
        let Some(sender) = sender else {
            return;
        };
        let wanted: HashMap<String, Account> = if enabled {
            accounts
                .into_iter()
                .map(|a| (a.endpoint.clone(), a))
                .collect()
        } else {
            HashMap::new()
        };
        state.update(cx, |s, _| {
            // sign-outs and a disabled setting stop their threads
            s.alive.sessions.retain(|endpoint, handle| {
                let keep = wanted
                    .get(endpoint)
                    .is_some_and(|a| a.login == handle.login);
                if !keep {
                    info!(endpoint, login = %handle.login, "unsubscribed from the Alive channel");
                }
                keep
            });
        });
        for (endpoint, account) in wanted {
            if state.read(cx).alive.sessions.contains_key(&endpoint) {
                continue;
            }
            let stop = Arc::new(AtomicBool::new(false));
            state.update(cx, |s, _| {
                s.alive.sessions.insert(
                    endpoint.clone(),
                    AliveHandle {
                        stop: stop.clone(),
                        login: account.login.clone(),
                    },
                );
            });
            let sender = sender.clone();
            let name = format!("alive-{}", account.login);
            let spawned = std::thread::Builder::new().name(name).spawn(move || {
                let token = match corvane_platform::keychain::token(&account.host(), &account.login)
                {
                    Ok(Some(token)) => token,
                    _ => {
                        debug!(login = %account.login, "no token for the Alive subscription");
                        return;
                    }
                };
                let client = Client::new(Endpoint::from_api_base(&account.endpoint), token);
                let channel = match client.alive_desktop_channel() {
                    Ok(Some(channel)) => channel,
                    Ok(None) => {
                        info!(login = %account.login, "no Alive channel for this account");
                        return;
                    }
                    Err(err) => {
                        warn!(%err, login = %account.login, "Alive channel request failed");
                        return;
                    }
                };
                let url_client = Client::new(
                    Endpoint::from_api_base(&account.endpoint),
                    client_token(&account),
                );
                corvane_github::alive::run_session(
                    channel,
                    move || match url_client.alive_websocket_url() {
                        Ok(url) => url,
                        Err(err) => {
                            warn!(%err, "Alive web socket request failed");
                            // keep retrying with backoff rather than giving up
                            Some(String::new())
                        }
                    },
                    stop,
                    move |event| {
                        let _ = sender.send_blocking(event);
                    },
                );
            });
            if let Err(err) = spawned {
                warn!(%err, "could not start the Alive subscription thread");
                state.update(cx, |s, _| {
                    s.alive.sessions.remove(&endpoint);
                });
            }
        }
    }

    /// `handleAliveEvent`: an event for the selected repository's cached
    /// pull request becomes a notification once its review / comment /
    /// checks are known.
    pub fn handle_alive_event(event: AliveEvent, data: AliveEventData, cx: &mut App) {
        let state = Self::state(cx);
        let (id, github, pull_request, account, repo_path) = {
            let s = state.read(cx);
            let Some(id) = s.selected else {
                debug!("Alive event without a selected repository");
                return;
            };
            let Some(repo) = s.repository(id) else { return };
            // `isValidRepositoryForEvent`: the repository the pull request
            // belongs to (a fork's parent when contributing upstream)
            let Some(github) = repo.non_fork_github().cloned() else {
                debug!("Alive event for a repository without a GitHub remote");
                return;
            };
            let (owner, name) = event.repository();
            if !(github.owner.eq_ignore_ascii_case(owner) && github.name.eq_ignore_ascii_case(name))
            {
                debug!(
                    owner,
                    name, "Alive event for a repository that is not selected"
                );
                return;
            }
            // "If the PR is not in cache, it probably means the user didn't
            // work on it recently, so we don't want to show a notification."
            let number = event.pull_request_number();
            let Some(pull_request) = s
                .pull_requests_for(id)
                .iter()
                .find(|pr| pr.number == number)
                .cloned()
            else {
                debug!(number, "Alive event for a pull request that is not cached");
                return;
            };
            (
                id,
                github,
                pull_request,
                s.account_for(
                    &repo
                        .github
                        .as_ref()
                        .map(|g| g.endpoint.clone())
                        .unwrap_or_default(),
                )
                .cloned(),
                repo.path.clone(),
            )
        };
        // what the API lookups need, before `notify` takes ownership
        let (owner, name, number, git_ref) = (
            github.owner.clone(),
            github.name.clone(),
            pull_request.number,
            pull_request.commit_ref(),
        );
        let notify = move |kind: NotificationKind, cx: &mut App| {
            Self::notify_pull_request_event(
                PullRequestNotification {
                    repo: id,
                    owner: github.owner.clone(),
                    name: github.name.clone(),
                    pull_request: pull_request.clone(),
                    kind,
                },
                cx,
            );
        };
        match event {
            AliveEvent::Comment {
                subtype,
                comment_id,
                ..
            } => match data {
                AliveEventData::Sample => notify(
                    NotificationKind::PullRequestComment {
                        comment: crate::samples::comment(),
                    },
                    cx,
                ),
                AliveEventData::Api => {
                    let Some(client) = api_client(account.as_ref()) else {
                        return;
                    };
                    spawn_bg(
                        cx,
                        move || match subtype {
                            CommentSubtype::IssueComment => {
                                client.issue_comment(&owner, &name, &comment_id)
                            }
                            CommentSubtype::ReviewComment => {
                                client.pull_request_review_comment(&owner, &name, &comment_id)
                            }
                        },
                        move |result, cx| match result {
                            Ok(Some(comment)) => {
                                notify(NotificationKind::PullRequestComment { comment }, cx)
                            }
                            Ok(None) => debug!("comment of the Alive event was not found"),
                            Err(err) => warn!(%err, "could not fetch the comment"),
                        },
                    );
                }
            },
            AliveEvent::ReviewSubmit {
                review_id, state, ..
            } => match data {
                AliveEventData::Sample => {
                    let review_state = match state.as_str() {
                        "APPROVED" => ApiPullRequestReviewState::Approved,
                        "COMMENTED" => ApiPullRequestReviewState::Commented,
                        _ => ApiPullRequestReviewState::ChangesRequested,
                    };
                    notify(
                        NotificationKind::PullRequestReview {
                            review: crate::samples::review(review_state),
                        },
                        cx,
                    )
                }
                AliveEventData::Api => {
                    let Some(client) = api_client(account.as_ref()) else {
                        return;
                    };
                    spawn_bg(
                        cx,
                        move || client.pull_request_review(&owner, &name, number, &review_id),
                        move |result, cx| match result {
                            Ok(Some(review)) if is_valid_notification_review(&review) => {
                                notify(NotificationKind::PullRequestReview { review }, cx)
                            }
                            Ok(_) => debug!("review of the Alive event was not found or not shown"),
                            Err(err) => warn!(%err, "could not fetch the review"),
                        },
                    );
                }
            },
            AliveEvent::ChecksFailed {
                commit_sha,
                check_suite_id,
                ..
            } => {
                if state.read(cx).alive.skip_commit_shas.contains(&commit_sha) {
                    return;
                }
                if data == AliveEventData::Sample {
                    notify(
                        NotificationKind::ChecksFailed {
                            commit_sha,
                            checks: crate::samples::failed_checks(),
                        },
                        cx,
                    );
                    return;
                }
                let Some(account) = account else {
                    return;
                };
                let Some(client) = api_client(Some(&account)) else {
                    return;
                };
                let emails: Vec<String> = account.emails.iter().map(|e| e.to_lowercase()).collect();
                let sha = commit_sha.clone();
                spawn_bg(
                    cx,
                    move || -> ChecksLookup {
                        // only commits the user authored (pushed from here) count
                        let author = corvane_git::get_commits(&repo_path, &sha, 0, 1)
                            .ok()
                            .and_then(|c| c.into_iter().next())
                            .map(|c| c.author.email.to_lowercase());
                        match author {
                            None => ChecksLookup::UnknownCommit,
                            Some(email) if !emails.contains(&email) => ChecksLookup::NotTheUsers,
                            Some(_) => {
                                match crate::commit_status::fetch_ref_checks(
                                    &client, &owner, &name, &git_ref,
                                ) {
                                    Some(checks) => ChecksLookup::Checks(checks),
                                    None => ChecksLookup::NoChecks,
                                }
                            }
                        }
                    },
                    move |lookup, cx| {
                        let state = Self::state(cx);
                        let checks = match lookup {
                            ChecksLookup::UnknownCommit | ChecksLookup::NotTheUsers => {
                                state.update(cx, |s, _| {
                                    s.alive.skip_commit_shas.insert(commit_sha.clone());
                                });
                                return;
                            }
                            ChecksLookup::NoChecks => return,
                            ChecksLookup::Checks(checks) => checks,
                        };
                        // one notification per check suite, even when jobs re-run
                        let suite_runs: Vec<u64> = checks
                            .iter()
                            .filter(|c| c.check_suite_id == Some(check_suite_id))
                            .map(|c| c.id)
                            .collect();
                        let already = {
                            let s = state.read(cx);
                            !suite_runs.is_empty()
                                && suite_runs
                                    .iter()
                                    .all(|id| s.alive.skip_check_runs.contains(id))
                        };
                        if already {
                            return;
                        }
                        let failed = checks
                            .iter()
                            .filter(|c| {
                                c.conclusion == Some(corvane_models::CheckConclusion::Failure)
                            })
                            .count();
                        // checks that were just restarted have no failures yet
                        if failed == 0 {
                            return;
                        }
                        state.update(cx, |s, _| {
                            s.alive.skip_check_runs.extend(checks.iter().map(|c| c.id));
                        });
                        notify(NotificationKind::ChecksFailed { commit_sha, checks }, cx);
                    },
                );
            }
        }
    }

    /// `CORVANE_POPUP=alive:<kind>[:api]`: an event of `kind` for the selected
    /// repository's first cached pull request, through `handle_alive_event`.
    pub fn simulate_alive_event(
        id: u64,
        kind: TestNotificationType,
        data: AliveEventData,
        cx: &mut App,
    ) {
        let (github, number) = {
            let s = Self::state(cx).read(cx);
            let Some(repo) = s.repository(id) else { return };
            let Some(github) = repo.non_fork_github().cloned() else {
                warn!("the repository has no GitHub remote; use CORVANE_POPUP=pr-list first");
                return;
            };
            let Some(pr) = s.pull_requests_for(id).first() else {
                warn!("no cached pull requests; use CORVANE_POPUP=pr-list first");
                return;
            };
            (github, pr.number)
        };
        let event = sample_event(kind, &github, number);
        info!(?event, "simulating an Alive event");
        Self::handle_alive_event(event, data, cx);
    }
}

/// What the background lookup of a checks-failed event found.
enum ChecksLookup {
    UnknownCommit,
    NotTheUsers,
    NoChecks,
    Checks(Vec<RefCheck>),
}

fn client_token(account: &Account) -> String {
    corvane_platform::keychain::token(&account.host(), &account.login)
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// The API client for the account that owns the repository's endpoint.
fn api_client(account: Option<&Account>) -> Option<Client> {
    let account = account?;
    let token = client_token(account);
    if token.is_empty() {
        debug!(login = %account.login, "no token for the Alive event lookup");
        return None;
    }
    Some(Client::new(
        Endpoint::from_api_base(&account.endpoint),
        token,
    ))
}

/// The sample event of each kind (what GitHub would push).
pub fn sample_event(
    kind: TestNotificationType,
    github: &GitHubRepository,
    number: u64,
) -> AliveEvent {
    let (owner, repo) = (github.owner.clone(), github.name.clone());
    match kind {
        TestNotificationType::PullRequestReview => AliveEvent::ReviewSubmit {
            timestamp: 0,
            owner,
            repo,
            pull_request_number: number,
            state: "CHANGES_REQUESTED".into(),
            review_id: "1".into(),
        },
        TestNotificationType::PullRequestComment => AliveEvent::Comment {
            subtype: CommentSubtype::IssueComment,
            timestamp: 0,
            owner,
            repo,
            pull_request_number: number,
            comment_id: "1".into(),
        },
        TestNotificationType::ChecksFailed => AliveEvent::ChecksFailed {
            timestamp: 0,
            owner,
            repo,
            pull_request_number: number,
            check_suite_id: 1,
            commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
        },
    }
}
