//! Pull request notifications - GHD `lib/stores/notifications-store.ts`
//! (the `showNotification` path of `handlePullRequestReviewSubmitEvent`,
//! `handlePullRequestCommentEvent`, `handleChecksFailedEvent`),
//! `lib/notifications/show-notification.ts` / `notification-handler.ts` and
//! the click callbacks in `lib/stores/app-store.ts`
//! (`onPullRequestReviewSubmitNotification`, `onPullRequestCommentNotification`,
//! `onChecksFailedNotification`).
//!
//! A [`PullRequestNotification`] is posted through
//! `corvane_platform::notifications` with GHD's title and body; a click
//! brings the window forward and opens the matching dialog
//! (`PullRequestReview`, `PullRequestComment`, `PullRequestChecksFailed`, or
//! the CI status popover when the pull request's branch is checked out).
//!
//! Deviations: nothing produces events yet (GHD's Alive websocket is not
//! ported); only the Test Notifications dialog posts them. The notification
//! carries the dialog's data as JSON in its `userInfo`, so a click from an
//! earlier session opens the dialog without GHD's API round trip
//! (`onNotificationEventReceived` re-fetches the review / comment / checks).

use std::sync::atomic::{AtomicU64, Ordering};

use corvane_github::api::{ApiIssueComment, ApiPullRequestReview, ApiPullRequestReviewState};
use corvane_models::{CheckConclusion, PullRequest, RefCheck};
use gpui_kit::{App, AsyncApp};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::dispatcher::Dispatcher;
use crate::state::{AppState, Popup};

/// What happened on the pull request (GHD `DesktopAliveEvent` types).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NotificationKind {
    /// `pr-review-submit`
    #[serde(rename = "pr-review-submit")]
    PullRequestReview { review: ApiPullRequestReview },
    /// `pr-comment`
    #[serde(rename = "pr-comment")]
    PullRequestComment { comment: ApiIssueComment },
    /// `pr-checks-failed`
    #[serde(rename = "pr-checks-failed")]
    ChecksFailed {
        commit_sha: String,
        checks: Vec<RefCheck>,
    },
}

/// GHD `TestNotificationType`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestNotificationType {
    PullRequestReview,
    PullRequestComment,
    ChecksFailed,
}

impl TestNotificationType {
    pub const ALL: [TestNotificationType; 3] = [
        TestNotificationType::PullRequestReview,
        TestNotificationType::PullRequestComment,
        TestNotificationType::ChecksFailed,
    ];

    /// GHD `getTypeFriendlyName`
    pub fn friendly_name(self) -> &'static str {
        match self {
            TestNotificationType::PullRequestReview => "Pull Request Review",
            TestNotificationType::PullRequestComment => "Pull Request Comment",
            TestNotificationType::ChecksFailed => "Pull Request Checks Failed",
        }
    }
}

/// One notification: the repository, its pull request and the event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestNotification {
    /// Local repository id.
    pub repo: u64,
    /// GHD event `owner` / `repo`: finds the repository again when the id
    /// no longer exists (removed and added back).
    pub owner: String,
    pub name: String,
    pub pull_request: PullRequest,
    pub kind: NotificationKind,
}

/// GHD `isValidNotificationPullRequestReview`
pub fn is_valid_notification_review(review: &ApiPullRequestReview) -> bool {
    matches!(
        review.state,
        ApiPullRequestReviewState::Approved
            | ApiPullRequestReviewState::ChangesRequested
            | ApiPullRequestReviewState::Commented
    )
}

/// GHD `getVerbForPullRequestReview`
pub fn review_verb(state: ApiPullRequestReviewState) -> &'static str {
    match state {
        ApiPullRequestReviewState::Approved => "approved",
        ApiPullRequestReviewState::ChangesRequested => "requested changes on",
        _ => "reviewed",
    }
}

/// GHD `truncateWithEllipsis`: at most `max` characters (variation
/// selectors stay with their character), then `…`.
pub fn truncate_with_ellipsis(text: &str, max: usize) -> String {
    let mut characters: Vec<String> = Vec::new();
    for c in text.chars() {
        if ('\u{FE00}'..='\u{FE0F}').contains(&c) {
            if let Some(last) = characters.last_mut() {
                last.push(c);
            }
        } else {
            characters.push(c.to_string());
        }
    }
    if characters.len() <= max {
        return text.to_string();
    }
    format!("{}…", characters[..max].concat())
}

/// GHD `shortenSHA`
fn short_sha(sha: &str) -> &str {
    sha.get(..9).unwrap_or(sha)
}

impl PullRequestNotification {
    /// The notification title (GHD `title`).
    pub fn title(&self) -> String {
        match &self.kind {
            NotificationKind::PullRequestReview { review } => format!(
                "@{} {} your pull request",
                review.user.login,
                review_verb(review.state)
            ),
            NotificationKind::PullRequestComment { comment } => {
                format!("@{} commented on your pull request", comment.user.login)
            }
            NotificationKind::ChecksFailed { .. } => "Pull Request checks failed".to_string(),
        }
    }

    /// The notification body (GHD `body`).
    pub fn body(&self) -> String {
        let pr = &self.pull_request;
        match &self.kind {
            NotificationKind::PullRequestReview { review } => format!(
                "{} #{}\n{}",
                pr.title,
                pr.number,
                truncate_with_ellipsis(&review.body, 50)
            ),
            NotificationKind::PullRequestComment { comment } => format!(
                "{} #{}\n{}",
                pr.title,
                pr.number,
                truncate_with_ellipsis(&comment.body, 50)
            ),
            NotificationKind::ChecksFailed { commit_sha, checks } => {
                let failed = checks
                    .iter()
                    .filter(|c| c.conclusion == Some(CheckConclusion::Failure))
                    .count();
                let plural = if failed == 1 {
                    "check was"
                } else {
                    "checks were"
                };
                format!(
                    "{} #{} ({})\n{failed} {plural} not successful.",
                    pr.title,
                    pr.number,
                    short_sha(commit_sha)
                )
            }
        }
    }
}

/// A process-unique notification identifier.
fn next_identifier() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "corvane-{nanos}-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// Should a click switch the repository / check out the pull request branch?
/// (`shouldChangeRepository` / `shouldCheckoutBranch`)
fn switch_flags(s: &AppState, repo: u64, pr: &PullRequest) -> (bool, bool) {
    let current_branch = s
        .repo_states
        .get(&repo)
        .and_then(|rs| rs.info.as_ref())
        .and_then(|info| info.current_branch())
        .map(|b| b.name.clone());
    let should_change_repository = s.selected != Some(repo);
    let should_checkout_branch = current_branch
        .as_deref()
        .is_some_and(|b| b != pr.head.ref_name);
    (should_change_repository, should_checkout_branch)
}

impl Dispatcher {
    /// GHD `initializeRendererNotificationHandler`: route notification clicks
    /// to [`Dispatcher::notification_clicked`]; `focus_window` brings the
    /// (possibly hidden) window forward first (`focusWindow`).
    pub fn listen_for_notification_clicks(focus_window: impl Fn(&mut App) + 'static, cx: &mut App) {
        let (tx, rx) =
            async_channel::unbounded::<corvane_platform::notifications::NotificationClick>();
        corvane_platform::notifications::install_click_handler(move |click| {
            let _ = tx.try_send(click);
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok(click) = rx.recv().await {
                cx.update(|cx| {
                    focus_window(cx);
                    debug!(identifier = %click.identifier, "notification clicked");
                    if let Some(payload) = click.payload.as_deref() {
                        Self::notification_payload_clicked(payload, cx);
                    }
                });
            }
        })
        .detach();
    }

    /// A click on a notification whose `userInfo` carried `payload` (GHD
    /// `onNotificationEventReceived` for notifications of earlier sessions).
    pub fn notification_payload_clicked(payload: &str, cx: &mut App) {
        match serde_json::from_str::<PullRequestNotification>(payload) {
            Ok(notification) => Self::notification_clicked(notification, cx),
            Err(err) => warn!(%err, "unreadable notification payload"),
        }
    }

    /// A pull request event for the user (GHD `handleAliveEvent`), shown
    /// only while Settings › Notifications › "Enable notifications" is on.
    pub fn notify_pull_request_event(notification: PullRequestNotification, cx: &mut App) {
        if !Self::state(cx).read(cx).settings.notifications_enabled {
            debug!("notifications are disabled");
            return;
        }
        Self::post_pull_request_event(notification, cx);
    }

    /// The `userInfo` payload posted for `notification`.
    pub fn notification_payload(notification: &PullRequestNotification) -> Option<String> {
        serde_json::to_string(notification)
            .map_err(|err| warn!(%err, "could not serialize the notification payload"))
            .ok()
    }

    /// GHD `simulateAliveEvent` (Test Notifications): the event goes through
    /// the same path without the Settings gate, which in GHD only switches
    /// the Alive subscription.
    pub fn simulate_pull_request_event(notification: PullRequestNotification, cx: &mut App) {
        Self::post_pull_request_event(notification, cx);
    }

    /// GHD `isValidRepositoryForEvent` + `showNotification`: only events of
    /// the selected repository are shown.
    fn post_pull_request_event(notification: PullRequestNotification, cx: &mut App) {
        if Self::state(cx).read(cx).selected != Some(notification.repo) {
            debug!(
                repo = notification.repo,
                "notification for a repository that is not selected"
            );
            return;
        }
        if let NotificationKind::PullRequestReview { review } = &notification.kind
            && !is_valid_notification_review(review)
        {
            return;
        }
        let (title, body) = (notification.title(), notification.body());
        let Some(payload) = Self::notification_payload(&notification) else {
            return;
        };
        let identifier = next_identifier();
        info!(%identifier, %title, "showing notification");
        corvane_platform::notifications::show(
            &identifier,
            &title,
            &body,
            Some(&payload),
            |result| {
                if let Err(err) = result {
                    warn!(%err, "notification not shown");
                }
            },
        );
    }

    /// The notification's click callback (GHD `onPullRequestReviewSubmitNotification`,
    /// `onPullRequestCommentNotification`, `onChecksFailedNotification`):
    /// select the repository when none is, then open the matching dialog.
    pub fn notification_clicked(notification: PullRequestNotification, cx: &mut App) {
        let state = Self::state(cx);
        let repo = {
            let s = state.read(cx);
            s.repository(notification.repo)
                .or_else(|| {
                    s.repositories.iter().find(|r| {
                        r.github.as_ref().is_some_and(|g| {
                            (g.owner == notification.owner && g.name == notification.name)
                                || g.parent.as_ref().is_some_and(|p| {
                                    p.owner == notification.owner && p.name == notification.name
                                })
                        })
                    })
                })
                .map(|r| r.id)
        };
        let Some(repo) = repo else {
            warn!(
                owner = %notification.owner,
                name = %notification.name,
                "notification for a repository that is no longer added"
            );
            return;
        };
        if state.read(cx).selected.is_none() {
            Self::select_repository(repo, cx);
        }
        let (should_change_repository, should_checkout_branch) =
            switch_flags(state.read(cx), repo, &notification.pull_request);
        let pull_request = notification.pull_request;
        let popup = match notification.kind {
            NotificationKind::PullRequestReview { review } => Popup::PullRequestReview {
                repo,
                pull_request,
                review,
                should_checkout_branch,
                should_change_repository,
            },
            NotificationKind::PullRequestComment { comment } => Popup::PullRequestComment {
                repo,
                pull_request,
                comment,
                should_checkout_branch,
                should_change_repository,
            },
            NotificationKind::ChecksFailed { checks, .. } => {
                if should_change_repository {
                    Popup::PullRequestChecksFailed {
                        repo,
                        pull_request,
                        checks,
                        should_change_repository: true,
                    }
                } else if !should_checkout_branch
                    && state
                        .read(cx)
                        .repo_states
                        .get(&repo)
                        .and_then(|rs| rs.info.as_ref())
                        .and_then(|info| info.current_branch())
                        .is_some()
                {
                    // the pull request's branch is checked out: the CI
                    // check-run popover under the toolbar badge
                    Self::set_show_ci_status_popover(true, cx);
                    return;
                } else {
                    Popup::PullRequestChecksFailed {
                        repo,
                        pull_request,
                        checks,
                        should_change_repository: false,
                    }
                }
            }
        };
        Self::show_popup(popup, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_like_ghd() {
        assert_eq!(truncate_with_ellipsis("short", 50), "short");
        let long = "a".repeat(60);
        assert_eq!(
            truncate_with_ellipsis(&long, 50),
            format!("{}…", "a".repeat(50))
        );
        // variation selectors do not count
        let hearts = "❤\u{FE0F}".repeat(3);
        assert_eq!(truncate_with_ellipsis(&hearts, 3), hearts);
        assert_eq!(truncate_with_ellipsis(&hearts, 2), "❤\u{FE0F}❤\u{FE0F}…");
    }

    fn notification(kind: NotificationKind) -> PullRequestNotification {
        PullRequestNotification {
            repo: 1,
            owner: "wasi-master".into(),
            name: "corvane".into(),
            pull_request: crate::samples::sample_pull_request(),
            kind,
        }
    }

    #[test]
    fn titles_and_bodies() {
        let review = crate::samples::review(ApiPullRequestReviewState::ChangesRequested);
        let n = notification(NotificationKind::PullRequestReview { review });
        assert_eq!(n.title(), "@octocat requested changes on your pull request");
        assert!(
            n.body()
                .starts_with("Render pull request bodies as Markdown #42\nNice work")
        );
        assert!(n.body().ends_with('…'));

        let n = notification(NotificationKind::PullRequestComment {
            comment: crate::samples::comment(),
        });
        assert_eq!(n.title(), "@octocat commented on your pull request");

        let n = notification(NotificationKind::ChecksFailed {
            commit_sha: "4f1c2d3e4f5a6b7c8d9e".into(),
            checks: crate::samples::failed_checks(),
        });
        assert_eq!(n.title(), "Pull Request checks failed");
        assert_eq!(
            n.body(),
            "Render pull request bodies as Markdown #42 (4f1c2d3e4)\n2 checks were not successful."
        );
    }

    #[test]
    fn payload_round_trips() {
        let n = notification(NotificationKind::ChecksFailed {
            commit_sha: "abc".into(),
            checks: crate::samples::failed_checks(),
        });
        let json = serde_json::to_string(&n).unwrap();
        assert!(json.contains("\"type\":\"pr-checks-failed\""));
        assert_eq!(
            serde_json::from_str::<PullRequestNotification>(&json).unwrap(),
            n
        );
        let n = notification(NotificationKind::PullRequestReview {
            review: crate::samples::review(ApiPullRequestReviewState::Approved),
        });
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(
            serde_json::from_str::<PullRequestNotification>(&json).unwrap(),
            n
        );
    }
}
