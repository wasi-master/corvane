//! Sample data for the `CORVANE_POPUP` dev hook: API-backed dialogs that
//! cannot be reached without a signed-in GitHub account (GHD's "Test UI
//! components" / `TestNotifications` popups play the same role).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use corvane_core::{
    AppState, CheckConclusion, CheckStatus, GitHubRepository, JobStep, PullRequest, PullRequestRef,
    RefCheck, WorkflowRun,
};
use corvane_github::api::{
    ApiIdentity, ApiIssueComment, ApiPullRequestReview, ApiPullRequestReviewState,
};
use gpui_kit::App;

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

/// The selected repository's GitHub repository, or a stand-in.
fn github_repository(repo: u64, cx: &App) -> GitHubRepository {
    AppState::global(cx)
        .read(cx)
        .repository(repo)
        .and_then(|r| r.github.clone())
        .unwrap_or_else(|| GitHubRepository {
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
        })
}

/// Marks `repo` as a GitHub repository and fills its pull request cache and
/// their CI statuses, so the branch foldout's Pull Requests tab and quick
/// view can be exercised without an account.
pub fn install_pull_requests(repo: u64, cx: &mut App) {
    let github = github_repository(repo, cx);
    let mut first = pull_request(repo, cx);
    first.body = REVIEW_BODY.into();
    let mut draft = pull_request(repo, cx);
    draft.number = 41;
    draft.title = "Resizable toolbar buttons".into();
    draft.draft = true;
    draft.author = "octocat".into();
    draft.created_at = iso_ago(9 * 86_400);
    draft.head.ref_name = "resizable-toolbar".into();
    draft.body = String::new();
    let mut third = pull_request(repo, cx);
    third.number = 37;
    third.title = "Crash reports saved locally".into();
    third.author = "hubot".into();
    third.created_at = iso_ago(40 * 86_400);
    third.head.ref_name = "crash-reports".into();
    third.body =
        "Adds an opt-in panic hook.\n\n- writes `~/Library/Logs/Corvane/crashes`\n- never uploads"
            .into();
    let prs = vec![first, draft, third];
    let statuses = [
        CheckConclusion::Failure,
        CheckConclusion::Success,
        CheckConclusion::Success,
    ];
    AppState::global(cx).update(cx, |s, cx| {
        if let Some(r) = s.repositories.iter_mut().find(|r| r.id == repo) {
            r.github = Some(github.clone());
        }
        for (pr, conclusion) in prs.iter().zip(statuses) {
            let checks = failed_checks()
                .into_iter()
                .map(|mut c| {
                    if conclusion == CheckConclusion::Success {
                        c.conclusion = Some(CheckConclusion::Success);
                    }
                    c
                })
                .collect();
            s.commit_statuses.entries.insert(
                corvane_core::status_key(&github, &pr.commit_ref()),
                corvane_core::commit_status::CommitStatusEntry {
                    check: corvane_core::CombinedRefCheck::from_checks(checks),
                    fetched_at: std::time::Instant::now(),
                },
            );
        }
        let mut cache = corvane_core::PullRequestCache::default();
        cache.pull_requests = prs;
        s.pull_requests
            .insert(corvane_core::cache_key(&github), cache);
        cx.notify();
    });
}

/// `repo` becomes a GitHub repository the user can only read.
pub fn make_read_only(repo: u64, cx: &mut App) {
    let mut github = github_repository(repo, cx);
    github.permissions = Some(corvane_core::RepositoryPermission::Read);
    AppState::global(cx).update(cx, |s, cx| {
        if let Some(r) = s.repositories.iter_mut().find(|r| r.id == repo) {
            r.github = Some(github);
        }
        cx.notify();
    });
}

pub fn pull_request(repo: u64, cx: &App) -> PullRequest {
    let github = github_repository(repo, cx);
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
