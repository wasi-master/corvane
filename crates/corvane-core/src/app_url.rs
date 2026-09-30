//! `x-corvane://` URL actions - GHD `lib/parse-app-url.ts` and the
//! dispatcher side of `dispatchURLAction` / `dispatchCLIAction`
//! (`ui/dispatcher/dispatcher.ts`: `openRepositoryFromUrl`,
//! `openBranchNameFromUrl`, `openPullRequestFromUrl`, `openOrCloneRepository`,
//! `checkoutLocalBranch`, and `app-store` `_startOpenInDesktop` /
//! `_completeOpenInDesktop`).
//!
//! - `x-corvane://openRepo/<remote url>?branch=&pr=&filepath=`: an added
//!   repository whose GitHub repository (or its parent) matches is
//!   selected, fetched and switched to the branch; an unknown one opens the
//!   clone dialog prefilled, and the branch / pull request / file follow once
//!   that clone finishes. `pr=` fetches the pull request and checks it out.
//! - `x-corvane://openLocalRepo/<percent-encoded path>`: GHD's CLI
//!   `open-repository` action - the repository root containing the path is
//!   selected, or the worktree it belongs to switched to, or the Add Local
//!   Repository dialog opens prefilled.
//! - `oauth` completes the browser sign-in (`Dispatcher::complete_web_flow`); it is
//!   built (device flow only), so it is logged and ignored.
//!
//! Deviations: GHD 3.6.6 has no `openLocalRepo` action (its CLI passes
//! `--cli-open` to a second Electron instance); Corvane's `corvane` tool
//! sends that URL so an already running Corvane receives it. With flag
//! `exact-repository-url-first`, `openRepo` prefers the repository that is
//! the URL over a fork matching through its parent (`doesRepositoryMatchUrl`
//! callers take the first match).

use std::path::{Path, PathBuf};

use gpui_kit::App;
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::Popup;
use corvane_models::{PullRequest, url_matches_remote};

/// The URL scheme Corvane registers (`CFBundleURLSchemes`).
pub const URL_SCHEME: &str = "x-corvane";

/// GHD `URLActionType`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UrlAction {
    /// `IOAuthAction`
    OAuth { code: String, state: String },
    /// `IOpenRepositoryFromURLAction`
    OpenRepositoryFromUrl {
        url: String,
        branch: Option<String>,
        pr: Option<String>,
        filepath: Option<String>,
    },
    /// Corvane: open a local path (the CLI's `open`).
    OpenLocalRepository { path: PathBuf },
    /// Corvane: the Flags dialog (`x-corvane://flags?q=<search>`).
    Flags { query: Option<String> },
    /// `IUnknownAction`
    Unknown { url: String },
}

/// `%XX` decoding (invalid escapes stay as they are); `plus` also turns
/// `+` into a space, as `querystring.parse` does.
fn percent_decode(s: &str, plus: bool) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                        continue;
                    }
                    None => out.push(b'%'),
                }
            }
            b'+' if plus => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encode everything but unreserved characters and `/`.
pub fn percent_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// GHD `testForInvalidChars` (`lib/sanitize-ref-name.ts`).
pub fn has_invalid_ref_chars(name: &str) -> bool {
    static RE: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r#"[\x00-\x20\x7F~^:?*\[\\|"<>]+|@\{|\.\.+|^\.|\.$|\.lock$|/$"#).ok()
    })
    .as_ref()
    .is_some_and(|re| re.is_match(name))
}

/// GHD `getQueryStringValue`: the first value of `key`.
fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (percent_decode(k, true) == key).then(|| percent_decode(v, true))
    })
}

/// GHD `parseAppURL`
pub fn parse_app_url(url: &str) -> UrlAction {
    let unknown = || UrlAction::Unknown {
        url: url.to_string(),
    };
    let Some((_, rest)) = url.split_once("://") else {
        return unknown();
    };
    // `url.parse`: the fragment goes, the query follows the first `?`
    let rest = rest.split('#').next().unwrap_or(rest);
    let (location, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (host, path) = match location.find('/') {
        Some(ix) => (&location[..ix], &location[ix..]),
        None => (location, ""),
    };
    if host.is_empty() {
        return unknown();
    }
    let action = host.to_lowercase();
    if action == "oauth" {
        return match (query_value(query, "code"), query_value(query, "state")) {
            (Some(code), Some(state)) => UrlAction::OAuth { code, state },
            _ => unknown(),
        };
    }
    if action == "flags" {
        return UrlAction::Flags {
            query: query_value(query, "q").filter(|q| !q.is_empty()),
        };
    }
    // something resembling a URL (or path) must follow the action
    if path.len() <= 1 {
        return unknown();
    }
    let parsed_path = &path[1..];
    match action.as_str() {
        "openrepo" => {
            let pr = query_value(query, "pr");
            let branch = query_value(query, "branch");
            let filepath = query_value(query, "filepath");
            if let Some(pr) = &pr {
                if pr.is_empty() || !pr.bytes().all(|b| b.is_ascii_digit()) {
                    return unknown();
                }
                // a forked PR's branch is `pr/<number>`
                if let Some(branch) = &branch {
                    let valid = branch
                        .strip_prefix("pr/")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
                    if !valid {
                        return unknown();
                    }
                }
            }
            if branch.as_deref().is_some_and(has_invalid_ref_chars) {
                return unknown();
            }
            UrlAction::OpenRepositoryFromUrl {
                url: parsed_path.to_string(),
                branch,
                pr,
                filepath,
            }
        }
        // the path's own leading `/` is the one after the action
        "openlocalrepo" => {
            let decoded = percent_decode(parsed_path, false);
            UrlAction::OpenLocalRepository {
                path: if decoded.starts_with('/') {
                    PathBuf::from(decoded)
                } else {
                    Path::new("/").join(decoded)
                },
            }
        }
        _ => unknown(),
    }
}

/// `x-corvane://openLocalRepo/<path>` (the CLI builds the same string).
pub fn open_local_repo_url(path: &Path) -> String {
    let path = path.to_string_lossy();
    format!(
        "{URL_SCHEME}://openLocalRepo/{}",
        percent_encode_path(path.trim_start_matches('/'))
    )
}

/// GHD `resolveWithin`: `relative` inside `root`, symlinks resolved; `None`
/// when it escapes the root or does not exist.
fn resolve_within(root: &Path, relative: &str) -> Option<PathBuf> {
    let root = root.canonicalize().ok()?;
    let resolved = root.join(relative).canonicalize().ok()?;
    resolved.starts_with(&root).then_some(resolved)
}

/// What an `openRepo` action still has to do once its repository is known
/// (GHD awaits `_startOpenInDesktop` across the clone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingOpenInDesktop {
    pub branch: Option<String>,
    pub pull_request: Option<PullRequest>,
    pub filepath: Option<String>,
}

/// Where URLs arrive before the app can take them: GPUI's `on_open_urls`
/// callback (no `App` there, possibly before launch finished) and the
/// Finder service send here; [`Dispatcher::listen_for_app_urls`] drains it.
pub struct AppUrlInbox {
    tx: async_channel::Sender<String>,
    rx: async_channel::Receiver<String>,
}

/// A cloneable handle that queues URLs into an [`AppUrlInbox`].
#[derive(Clone)]
pub struct AppUrlSender(async_channel::Sender<String>);

impl AppUrlSender {
    pub fn send(&self, url: String) {
        let _ = self.0.try_send(url);
    }

    /// Only bring the window forward (a second launch without URLs, GHD's
    /// `second-instance` handler).
    pub fn focus(&self) {
        let _ = self.0.try_send(String::new());
    }
}

impl Default for AppUrlInbox {
    fn default() -> Self {
        let (tx, rx) = async_channel::unbounded();
        Self { tx, rx }
    }
}

impl AppUrlInbox {
    pub fn sender(&self) -> AppUrlSender {
        AppUrlSender(self.tx.clone())
    }
}

impl Dispatcher {
    /// Handle every URL queued in `inbox` (`app.on('open-url')`);
    /// `focus_window` shows the (possibly hidden) window first.
    pub fn listen_for_app_urls(
        inbox: AppUrlInbox,
        focus_window: impl Fn(&mut App) + 'static,
        cx: &mut App,
    ) {
        let rx = inbox.rx;
        cx.spawn(async move |cx| {
            while let Ok(url) = rx.recv().await {
                cx.update(|cx| {
                    focus_window(cx);
                    if !url.is_empty() {
                        Self::handle_app_url(&url, cx);
                    }
                });
            }
        })
        .detach();
    }

    /// GHD `dispatchURLAction` for a URL handed to Corvane by macOS.
    pub fn handle_app_url(url: &str, cx: &mut App) {
        let action = parse_app_url(url);
        info!(?action, "URL action");
        match action {
            UrlAction::OAuth { code, state } => Self::complete_web_flow(code, state, cx),
            UrlAction::OpenRepositoryFromUrl {
                url,
                branch,
                pr,
                filepath,
            } => Self::open_repository_from_url(url, branch, pr, filepath, cx),
            UrlAction::OpenLocalRepository { path } => Self::open_local_repository(path, cx),
            UrlAction::Flags { query } => Self::open_flags(query, cx),
            UrlAction::Unknown { url } => warn!(%url, "unknown URL action"),
        }
    }

    /// GHD `doesRepositoryMatchUrl`: the repository's GitHub repository or
    /// its parent is `url`. With `exact-repository-url-first` a repository
    /// that is `url` itself wins over a fork matching through its parent
    /// (GHD takes the first match in list order).
    fn repository_matching_url(url: &str, cx: &App) -> Option<u64> {
        let s = Self::state(cx).read(cx);
        if s.flags.bool(crate::flags::ids::EXACT_REPOSITORY_URL_FIRST)
            && let Some(exact) = s.repositories.iter().find(|r| {
                r.github
                    .as_ref()
                    .is_some_and(|g| url_matches_remote(&g.html_url, url))
            })
        {
            return Some(exact.id);
        }
        s.repositories
            .iter()
            .find(|r| {
                r.github.as_ref().is_some_and(|g| {
                    url_matches_remote(&g.html_url, url)
                        || g.parent
                            .as_ref()
                            .is_some_and(|p| url_matches_remote(&p.html_url, url))
                })
            })
            .map(|r| r.id)
    }

    /// GHD `openRepositoryFromUrl`
    fn open_repository_from_url(
        url: String,
        branch: Option<String>,
        pr: Option<String>,
        filepath: Option<String>,
        cx: &mut App,
    ) {
        if let Some(number) = pr.and_then(|pr| pr.parse::<u64>().ok()) {
            Self::open_pull_request_from_url(url, number, filepath, cx);
            return;
        }
        Self::open_or_clone_repository(
            url,
            PendingOpenInDesktop {
                branch,
                pull_request: None,
                filepath,
            },
            cx,
        );
    }

    /// GHD `openOrCloneRepository`: select the matching repository, else
    /// the clone dialog prefilled; `then` runs once the repository is known.
    fn open_or_clone_repository(url: String, then: PendingOpenInDesktop, cx: &mut App) {
        if let Some(id) = Self::repository_matching_url(&url, cx) {
            Self::select_repository(id, cx);
            Self::complete_open_in_desktop(id, then, cx);
            return;
        }
        Self::state(cx).update(cx, |s, _| s.pending_open_in_desktop = Some(then));
        Self::show_popup(Popup::CloneRepository { url: Some(url) }, cx);
    }

    /// GHD `_completeOpenInDesktop`: the clone of an `openRepo` action
    /// finished (called with the added repository).
    pub(crate) fn resume_open_in_desktop(id: u64, cx: &mut App) {
        let pending = Self::state(cx).update(cx, |s, _| s.pending_open_in_desktop.take());
        if let Some(pending) = pending {
            Self::complete_open_in_desktop(id, pending, cx);
        }
    }

    /// The rest of `openBranchNameFromUrl` / `openPullRequestFromUrl` /
    /// `openRepositoryFromUrl` for the selected repository `id`.
    fn complete_open_in_desktop(id: u64, pending: PendingOpenInDesktop, cx: &mut App) {
        if let Some(pr) = pending.pull_request {
            Self::switch_to_pull_request(id, pr, cx);
        } else if let Some(branch) = pending.branch {
            Self::open_branch_when_loaded(id, branch, cx);
        }
        if let Some(filepath) = pending.filepath {
            if Path::new(&filepath).is_absolute() {
                warn!(%filepath, "refusing to open absolute path");
                return;
            }
            let Some(root) = Self::state(cx)
                .read(cx)
                .repository(id)
                .map(|r| r.path.clone())
            else {
                return;
            };
            match resolve_within(&root, &filepath) {
                Some(resolved) => Self::show_in_finder(&resolved, cx),
                None => warn!(
                    %filepath,
                    "prevented attempt to open path outside of the repository root"
                ),
            }
        }
    }

    /// `openBranchNameFromUrl` after `openOrCloneRepository`: fetch, then
    /// `checkoutLocalBranch` (a local or remote branch of that name, unless
    /// it is already checked out).
    fn open_branch_when_loaded(id: u64, branch: String, cx: &mut App) {
        let loaded = move |cx: &App| {
            Self::state(cx)
                .read(cx)
                .repo_states
                .get(&id)
                .is_some_and(|rs| rs.info.is_some())
        };
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
                if !loaded(cx) {
                    return;
                }
                Self::fetch_then(id, cx, move |cx| {
                    Self::checkout_local_branch(id, &branch, cx);
                });
            });
        })
        .detach();
    }

    /// GHD `await this.appStore._fetch(…)`: fetch, then `then` once the
    /// fetch and the refresh after it are done (at once without a remote).
    fn fetch_then(id: u64, cx: &mut App, then: impl FnOnce(&mut App) + 'static) {
        Self::fetch(id, false, cx);
        let busy = move |cx: &App| {
            Self::state(cx)
                .read(cx)
                .repo_states
                .get(&id)
                .is_some_and(|rs| rs.push_pull_in_progress || rs.loading)
        };
        cx.spawn(async move |cx| {
            for _ in 0..1200 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                if !cx.update(|cx| busy(cx)) {
                    break;
                }
            }
            cx.update(then);
        })
        .detach();
    }

    /// GHD `checkoutLocalBranch`: only a branch Corvane knows (local, or a
    /// remote branch with that name), and only when it is not current.
    fn checkout_local_branch(id: u64, branch: &str, cx: &mut App) {
        let target = {
            let s = Self::state(cx).read(cx);
            let Some(info) = s.repo_states.get(&id).and_then(|rs| rs.info.as_ref()) else {
                return;
            };
            if info
                .current_branch()
                .is_some_and(|b| b.name_without_remote() == branch)
            {
                return;
            }
            info.branches
                .iter()
                .find(|b| b.kind == corvane_models::BranchKind::Local && b.name == branch)
                .or_else(|| {
                    info.branches
                        .iter()
                        .find(|b| b.name_without_remote() == branch)
                })
                .map(|b| b.name.clone())
        };
        if let Some(name) = target {
            Self::checkout_branch(id, name, None, cx);
        } else {
            info!(%branch, "branch from the URL does not exist; staying on the current one");
        }
    }

    /// GHD `openPullRequestFromUrl`: fetch the pull request, find the
    /// repository it lives in (head, then base), else clone `url`; then
    /// check it out.
    fn open_pull_request_from_url(
        url: String,
        number: u64,
        filepath: Option<String>,
        cx: &mut App,
    ) {
        let Some(github) = corvane_models::github_from_remote(&url, &[]) else {
            warn!(%url, "not a GitHub repository URL");
            return;
        };
        let Some((endpoint, token, _)) = Self::api_for(&github, cx) else {
            warn!(%url, "no account to fetch the pull request with");
            return;
        };
        spawn_bg(
            cx,
            move || {
                let client = corvane_github::Client::new(endpoint, token);
                client
                    .pull_request(&github.owner, &github.name, number)
                    .map(|pr| crate::pull_requests::convert_pull_request(&client, pr).0)
            },
            move |result, cx| {
                let pr = match result {
                    Ok(pr) => pr,
                    Err(err) => {
                        warn!(%err, number, "could not fetch the pull request");
                        return;
                    }
                };
                let pending = PendingOpenInDesktop {
                    branch: None,
                    pull_request: Some(pr.clone()),
                    filepath,
                };
                let found = [&pr.head, &pr.base].into_iter().find_map(|r| {
                    r.repository
                        .as_ref()
                        .and_then(|g| Self::repository_matching_url(&g.clone_url, cx))
                });
                match found {
                    Some(id) => {
                        Self::select_repository(id, cx);
                        Self::complete_open_in_desktop(id, pending, cx);
                    }
                    None => Self::open_or_clone_repository(url, pending, cx),
                }
            },
        );
    }

    /// GHD `dispatchCLIAction` `open-repository` (and `openLocalRepo` URLs):
    /// the repository root containing `path` is selected when added, the
    /// worktree it is switched to when it belongs to an added repository,
    /// else Add Local Repository opens prefilled.
    pub fn open_local_repository(path: PathBuf, cx: &mut App) {
        let git = Self::state(cx).read(cx).git.clone();
        spawn_bg(
            cx,
            move || {
                let root = corvane_git::top_level_working_directory(&path).unwrap_or(path);
                let worktrees = git
                    .and_then(|git| corvane_git::list_worktrees(git, &root).ok())
                    .unwrap_or_default();
                (root, worktrees)
            },
            |(root, worktrees), cx| {
                let s = Self::state(cx).read(cx);
                if let Some(id) = s
                    .repositories
                    .iter()
                    .find(|r| crate::dispatcher::same_path(&r.path, &root))
                    .map(|r| r.id)
                {
                    Self::select_repository(id, cx);
                    return;
                }
                // a repository sharing the main worktree: switch to this one
                let shared = s
                    .repositories
                    .iter()
                    .find(|r| {
                        worktrees
                            .iter()
                            .any(|w| crate::dispatcher::same_path(&w.path, &r.path))
                    })
                    .map(|r| r.id);
                if let Some(id) = shared {
                    Self::select_repository(id, cx);
                    Self::switch_worktree(id, root, cx);
                    return;
                }
                Self::show_popup(Popup::AddExistingRepository { path: Some(root) }, cx);
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_repo(url: &str) -> (String, Option<String>, Option<String>, Option<String>) {
        match parse_app_url(url) {
            UrlAction::OpenRepositoryFromUrl {
                url,
                branch,
                pr,
                filepath,
            } => (url, branch, pr, filepath),
            other => panic!("expected openRepo, got {other:?}"),
        }
    }

    fn is_unknown(url: &str) -> bool {
        matches!(parse_app_url(url), UrlAction::Unknown { .. })
    }

    // ported from GHD app/test/unit/parse-app-url-test.ts
    #[test]
    fn unknown_by_default() {
        assert!(is_unknown(""));
        assert!(is_unknown("x-corvane://"));
        assert!(is_unknown("x-corvane://whatever/thing"));
    }

    #[test]
    fn oauth() {
        assert_eq!(
            parse_app_url(
                "x-github-client://oauth?code=18142422&state=e4cd2dea-1567-46aa-8eb2-c7f56e943187"
            ),
            UrlAction::OAuth {
                code: "18142422".into(),
                state: "e4cd2dea-1567-46aa-8eb2-c7f56e943187".into()
            }
        );
    }

    #[test]
    fn flags_dialog_url() {
        assert_eq!(
            parse_app_url("x-corvane://flags"),
            UrlAction::Flags { query: None }
        );
        assert_eq!(
            parse_app_url("x-corvane://flags?q="),
            UrlAction::Flags { query: None }
        );
        assert_eq!(
            parse_app_url("x-corvane://flags?q=commit%20tpl"),
            UrlAction::Flags {
                query: Some("commit tpl".into())
            }
        );
        assert_eq!(
            parse_app_url("x-corvane://Flags/?other=1&q=%23201"),
            UrlAction::Flags {
                query: Some("#201".into())
            }
        );
    }

    #[test]
    fn open_repo_via_https() {
        assert_eq!(
            open_repo("github-mac://openRepo/https://github.com/desktop/desktop").0,
            "https://github.com/desktop/desktop"
        );
        assert!(is_unknown("github-mac://openRepo/"));
        let (url, branch, _, _) = open_repo(
            "github-mac://openRepo/https://github.com/desktop/desktop?branch=cancel-2fa-flow",
        );
        assert_eq!(url, "https://github.com/desktop/desktop");
        assert_eq!(branch.as_deref(), Some("cancel-2fa-flow"));
        let (url, branch, pr, _) = open_repo(
            "github-mac://openRepo/https://github.com/octokit/octokit.net?branch=pr%2F1569&pr=1569",
        );
        assert_eq!(url, "https://github.com/octokit/octokit.net");
        assert_eq!(branch.as_deref(), Some("pr/1569"));
        assert_eq!(pr.as_deref(), Some("1569"));
        assert!(is_unknown(
            "github-mac://openRepo/https://github.com/octokit/octokit.net?branch=bar&pr=foo"
        ));
        assert!(is_unknown(
            "github-mac://openRepo/https://github.com/octokit/octokit.net?branch=%3C%3E"
        ));
        let (_, branch, _, filepath) = open_repo(
            "github-mac://openRepo/https://github.com/octokit/octokit.net?branch=master&filepath=Octokit.Reactive%2FOctokit.Reactive.csproj",
        );
        assert_eq!(branch.as_deref(), Some("master"));
        assert_eq!(
            filepath.as_deref(),
            Some("Octokit.Reactive/Octokit.Reactive.csproj")
        );
    }

    #[test]
    fn open_repo_via_ssh() {
        assert_eq!(
            open_repo("github-mac://openRepo/git@github.com/desktop/desktop").0,
            "git@github.com/desktop/desktop"
        );
        let (url, branch, pr, _) = open_repo(
            "github-mac://openRepo/git@github.com/octokit/octokit.net?branch=pr%2F1569&pr=1569",
        );
        assert_eq!(url, "git@github.com/octokit/octokit.net");
        assert_eq!(branch.as_deref(), Some("pr/1569"));
        assert_eq!(pr.as_deref(), Some("1569"));
        assert!(is_unknown(
            "github-mac://openRepo/git@github.com/octokit/octokit.net?branch=bar&pr=foo"
        ));
    }

    #[test]
    fn open_local_repo_round_trips() {
        let path = Path::new("/Users/me/Code/my repo/ünïcode+plus");
        let url = open_local_repo_url(path);
        assert_eq!(
            url,
            "x-corvane://openLocalRepo/Users/me/Code/my%20repo/%C3%BCn%C3%AFcode%2Bplus"
        );
        assert_eq!(
            parse_app_url(&url),
            UrlAction::OpenLocalRepository {
                path: path.to_path_buf()
            }
        );
        assert!(is_unknown("x-corvane://openLocalRepo/"));
    }

    #[test]
    fn invalid_ref_chars() {
        assert!(has_invalid_ref_chars("<>"));
        assert!(has_invalid_ref_chars("a..b"));
        assert!(has_invalid_ref_chars("topic.lock"));
        assert!(has_invalid_ref_chars("trailing/"));
        assert!(!has_invalid_ref_chars("feature/markdown"));
    }

    #[test]
    fn resolves_only_inside_the_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "").unwrap();
        assert!(resolve_within(dir.path(), "src/lib.rs").is_some());
        assert!(resolve_within(dir.path(), "../").is_none());
        assert!(resolve_within(dir.path(), "missing").is_none());
    }
}
