//! Dev-hook setups for the `CORVANE_POPUP` hook; the sample records
//! themselves live in `corvane_core::samples` (shared with the Test
//! Notifications dialog).

use corvane_core::samples::{REVIEW_BODY, github_repository, iso_ago};
use corvane_core::{AppState, CheckConclusion};
use gpui_kit::App;

pub use corvane_core::samples::{comment, failed_checks, pull_request, review};

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
