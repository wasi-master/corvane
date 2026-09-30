//! Sample data for API-backed UI that cannot be reached without a signed-in
//! GitHub account: the `CORVANE_POPUP` dev hook and the Test Notifications
//! dialog (GHD's "Test UI components" / `TestNotifications` play the same
//! role with real API data).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use corvane_github::api::{
    ApiIdentity, ApiIssueComment, ApiPullRequestReview, ApiPullRequestReviewState,
};
use corvane_models::{
    CheckConclusion, CheckStatus, GitHubRepository, JobStep, PullRequest, PullRequestRef, RefCheck,
    WorkflowRun,
};
use gpui_kit::App;

use crate::AppState;

/// `seconds` ago as the API's ISO-8601 UTC timestamp.
pub fn iso_ago(seconds: u64) -> String {
    let t = SystemTime::now()
        .checked_sub(Duration::from_secs(seconds))
        .unwrap_or(UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (t / 86_400, t % 86_400);
    // civil-from-days (Howard Hinnant)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn octocat() -> ApiIdentity {
    ApiIdentity {
        id: 583_231,
        login: "octocat".into(),
        html_url: Some("https://github.com/octocat".into()),
        name: Some("The Octocat".into()),
        email: None,
        avatar_url: Some("https://avatars.githubusercontent.com/u/583231?v=4".into()),
        kind: Some("User".into()),
    }
}

/// A GitHub repository for repositories that have none.
fn stand_in_github_repository() -> GitHubRepository {
    GitHubRepository {
        endpoint: "https://api.github.com".into(),
        owner: "wasi-master".into(),
        name: "corvane-demo".into(),
        html_url: "https://github.com/wasi-master/corvane-demo".into(),
        clone_url: "https://github.com/wasi-master/corvane-demo.git".into(),
        default_branch: Some("main".into()),
        private: false,
        fork: false,
        parent: None,
        archived: false,
        permissions: None,
        allow_forking: None,
    }
}

/// The selected repository's GitHub repository, or a stand-in.
pub fn github_repository(repo: u64, cx: &App) -> GitHubRepository {
    AppState::global(cx)
        .read(cx)
        .repository(repo)
        .and_then(|r| r.github.clone())
        .unwrap_or_else(stand_in_github_repository)
}

pub fn pull_request(repo: u64, cx: &App) -> PullRequest {
    pull_request_in(github_repository(repo, cx))
}

/// The sample pull request of the stand-in repository.
pub fn sample_pull_request() -> PullRequest {
    pull_request_in(stand_in_github_repository())
}

fn pull_request_in(github: GitHubRepository) -> PullRequest {
    PullRequest {
        number: 42,
        title: "Render pull request bodies as Markdown".into(),
        created_at: iso_ago(3 * 86_400),
        updated_at: iso_ago(2 * 3600),
        head: PullRequestRef {
            ref_name: "feature/markdown".into(),
            sha: "4f1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c".into(),
            repository: Some(github.clone()),
        },
        base: PullRequestRef {
            ref_name: "main".into(),
            sha: "0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b".into(),
            repository: Some(github),
        },
        author: "wasi-master".into(),
        draft: false,
        body: "Renders **Markdown** natively.".into(),
    }
}

/// A body that exercises every block `corvane_ui::markdown` lays out.
pub const REVIEW_BODY: &str = "Nice work on the **renderer**! A few things before this lands:\n\n\
### Blocking\n\n\
1. The `Walk` should flush *inline* text before a nested list.\n\
2. Links like [the CommonMark spec](https://spec.commonmark.org) must open in the browser.\n\n\
- Bare URLs: https://github.com/wasi-master/corvane\n\
- ~~Tables~~ degrade to plain text\n\
\x20 - nested item\n\n\
> Quote from the style guide: keep the vertical rhythm at 16 px.\n\n\
```rust\nfn render(blocks: &[Block]) -> Div {\n    todo!()\n}\n```\n\n\
---\n\n\
Thanks!";

pub fn review(state: ApiPullRequestReviewState) -> ApiPullRequestReview {
    ApiPullRequestReview {
        id: 80,
        user: octocat(),
        body: if state == ApiPullRequestReviewState::Approved {
            String::new()
        } else {
            REVIEW_BODY.into()
        },
        html_url: "https://github.com/wasi-master/corvane-demo/pull/42#pullrequestreview-80".into(),
        submitted_at: iso_ago(2 * 3600),
        state,
    }
}

pub fn comment() -> ApiIssueComment {
    ApiIssueComment {
        id: 1,
        body: "Could you add a screenshot of the **dark theme** too? :eyes:\n\n\
               See `.docs/ghd-theme-tokens.md` for the colours."
            .into(),
        html_url: "https://github.com/wasi-master/corvane-demo/pull/42#issuecomment-1".into(),
        user: octocat(),
        created_at: iso_ago(25 * 60),
    }
}

fn step(number: u64, name: &str, conclusion: CheckConclusion, secs: u64) -> JobStep {
    JobStep {
        name: name.into(),
        number,
        status: CheckStatus::Completed,
        conclusion: Some(conclusion),
        started_at: Some(iso_ago(600 + secs)),
        completed_at: Some(iso_ago(600)),
    }
}

pub fn failed_checks() -> Vec<RefCheck> {
    let workflow = WorkflowRun {
        id: 7,
        workflow_id: 3,
        name: "CI".into(),
        event: "pull_request".into(),
        check_suite_id: Some(11),
        created_at: iso_ago(900),
    };
    let check = |id: u64,
                 name: &str,
                 description: &str,
                 conclusion: CheckConclusion,
                 steps: Option<Vec<JobStep>>| RefCheck {
        id,
        name: name.into(),
        description: description.into(),
        status: CheckStatus::Completed,
        conclusion: Some(conclusion),
        app_name: "GitHub Actions".into(),
        html_url: Some(format!(
            "https://github.com/wasi-master/corvane-demo/actions/runs/7/job/{id}"
        )),
        check_suite_id: Some(11),
        actions_workflow: steps.as_ref().map(|_| workflow.clone()),
        job_steps: steps,
    };
    vec![
        check(
            101,
            "test (macos-15)",
            "Process completed with exit code 101.",
            CheckConclusion::Failure,
            Some(vec![
                step(1, "Set up job", CheckConclusion::Success, 2),
                step(2, "Checkout", CheckConclusion::Success, 3),
                step(3, "cargo clippy", CheckConclusion::Success, 94),
                step(4, "cargo test --workspace", CheckConclusion::Failure, 211),
                step(5, "Post Checkout", CheckConclusion::Skipped, 0),
            ]),
        ),
        check(
            102,
            "fmt",
            "Successful in 21s",
            CheckConclusion::Success,
            Some(vec![
                step(1, "Set up job", CheckConclusion::Success, 1),
                step(2, "cargo fmt --check", CheckConclusion::Success, 20),
            ]),
        ),
        check(
            103,
            "Vercel",
            "Deployment has failed",
            CheckConclusion::Failure,
            None,
        ),
    ]
}

/// GHD `TestNotificationType`: the sample notification of each kind for
/// `repo` (the Test Notifications dialog).
pub fn notification(
    kind: crate::notifications::TestNotificationType,
    repo: u64,
    cx: &App,
) -> crate::notifications::PullRequestNotification {
    use crate::notifications::{NotificationKind, TestNotificationType};
    let github = github_repository(repo, cx);
    let pull_request = pull_request(repo, cx);
    let kind = match kind {
        TestNotificationType::PullRequestReview => NotificationKind::PullRequestReview {
            review: review(ApiPullRequestReviewState::ChangesRequested),
        },
        TestNotificationType::PullRequestComment => {
            NotificationKind::PullRequestComment { comment: comment() }
        }
        TestNotificationType::ChecksFailed => NotificationKind::ChecksFailed {
            commit_sha: pull_request.head.sha.clone(),
            checks: failed_checks(),
        },
    };
    crate::notifications::PullRequestNotification {
        repo,
        owner: github.owner,
        name: github.name,
        pull_request,
        kind,
    }
}
