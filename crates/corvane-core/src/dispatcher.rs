//! `Dispatcher`: the only thing that mutates `AppState`. Every method is
//! callable from the UI with an `&mut App`; git and disk work runs on the
//! background executor and results are applied on the main thread.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use corvane_git::{GitError, InitOptions, find_git, open_repository};
use corvane_store::Store;
use gpui_kit::{App, AppContext, AsyncApp, Entity};
use tracing::{error, info, warn};

use crate::persistence::{Settings, StoreExt};
use crate::state::{
    AppState, CloneState, Foldout, Popup, RepositoryState, SignInState, SignInStep,
};
use corvane_models::{Account, Repository, github_from_remote};
use std::sync::atomic::{AtomicBool, Ordering};

const RECENT_REPOSITORIES_LENGTH: usize = 3;

pub struct Dispatcher;

impl Dispatcher {
    /// Load persisted state, create the global entity, kick off git detection
    /// and a refresh of the selected repository.
    pub fn init(store: Arc<Store>, settings: Settings, cx: &mut App) -> Entity<AppState> {
        let repositories = store.repositories().unwrap_or_else(|err| {
            error!(?err, "could not load repositories");
            Vec::new()
        });
        let recent = store.recent_repositories().unwrap_or_default();
        let selected = store
            .selected_repository()
            .ok()
            .flatten()
            .filter(|id| repositories.iter().any(|r| r.id == *id))
            .or_else(|| recent.first().copied())
            .or_else(|| repositories.first().map(|r| r.id));
        let accounts = store.accounts().unwrap_or_default();

        // Synchronous: a few `git --version` probes (~10 ms). Avoids racing
        // launch-time operations against an async detection.
        let (git, git_error, popup) = match find_git() {
            Ok(bin) => (Some(Arc::new(bin)), None, None),
            Err(err) => {
                warn!(%err, "git not usable");
                (
                    None,
                    Some(err.to_string()),
                    Some(Popup::InstallGit {
                        reason: err.to_string(),
                    }),
                )
            }
        };
        let state = cx.new(|_| AppState {
            store,
            settings,
            git,
            git_error,
            repositories,
            recent,
            selected,
            repo_states: Default::default(),
            accounts,
            foldout: None,
            popup,
            cloning: None,
            sign_in: None,
        });
        AppState::install(state.clone(), cx);

        if let Some(id) = state.read(cx).selected {
            Self::refresh_repository(id, cx);
        }
        state
    }

    fn state(cx: &App) -> Entity<AppState> {
        AppState::global(cx)
    }

    /// Find git on a background thread, then refresh the selected repository.
    pub fn detect_git(cx: &mut App) {
        let task = cx.background_executor().spawn(async move { find_git() });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                let state = Self::state(cx);
                state.update(cx, |s, cx| {
                    match result {
                        Ok(bin) => {
                            s.git = Some(Arc::new(bin));
                            s.git_error = None;
                        }
                        Err(err) => {
                            warn!(%err, "git not usable");
                            s.git = None;
                            s.git_error = Some(err.to_string());
                            s.popup = Some(Popup::InstallGit {
                                reason: err.to_string(),
                            });
                        }
                    }
                    cx.notify();
                });
                if let Some(id) = state.read(cx).selected {
                    Self::refresh_repository(id, cx);
                }
            });
        })
        .detach();
    }

    // ---- foldouts / popups ----

    pub fn toggle_foldout(foldout: Foldout, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.foldout = if s.foldout == Some(foldout) {
                None
            } else {
                Some(foldout)
            };
            cx.notify();
        });
    }

    pub fn close_foldout(cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if s.foldout.take().is_some() {
                cx.notify();
            }
        });
    }

    pub fn show_popup(popup: Popup, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.popup = Some(popup);
            cx.notify();
        });
    }

    pub fn close_popup(cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if s.popup.take().is_some() {
                cx.notify();
            }
        });
    }

    pub fn show_error(title: impl Into<String>, message: impl Into<String>, cx: &mut App) {
        Self::show_popup(
            Popup::Error {
                title: title.into(),
                message: message.into(),
            },
            cx,
        );
    }

    // ---- repositories ----

    /// Validate `path` is a git repository (background), then add + select it.
    /// Existing entries for the same path are selected instead of duplicated.
    pub fn add_repository(path: PathBuf, cx: &mut App) {
        let state = Self::state(cx);
        if let Some(existing) = state
            .read(cx)
            .repositories
            .iter()
            .find(|r| same_path(&r.path, &path))
        {
            let id = existing.id;
            Self::select_repository(id, cx);
            return;
        }
        let probe = cx
            .background_executor()
            .spawn(async move { open_repository(&path).map(|info| (path, info)) });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = probe.await;
            cx.update(|cx| match result {
                Ok((path, info)) => {
                    let state = Self::state(cx);
                    let id = state.update(cx, |s, cx| {
                        let id = s.store.next_repository_id().unwrap_or_else(|_| {
                            s.repositories.iter().map(|r| r.id).max().unwrap_or(0) + 1
                        });
                        let mut repo = Repository::new(id, info.workdir.clone());
                        repo.github = info
                            .remote("origin")
                            .and_then(|r| github_from_remote(&r.url, &[]));
                        s.repositories.push(repo);
                        let repo_state = s.repo_state_mut(id);
                        repo_state.info = Some(info);
                        repo_state.last_refresh = Some(Instant::now());
                        persist_repositories(s);
                        info!(id, path = %path.display(), "added repository");
                        cx.notify();
                        id
                    });
                    Self::select_repository(id, cx);
                }
                Err(GitError::NotARepository(path)) => Self::show_error(
                    "Not a git repository",
                    format!(
                        "The directory {} does not appear to be a Git repository.",
                        path.display()
                    ),
                    cx,
                ),
                Err(err) => Self::show_error("Could not add repository", err.to_string(), cx),
            });
        })
        .detach();
    }

    pub fn select_repository(id: u64, cx: &mut App) {
        let state = Self::state(cx);
        let changed = state.update(cx, |s, cx| {
            if s.repository(id).is_none() {
                return false;
            }
            s.selected = Some(id);
            s.recent.retain(|r| *r != id);
            s.recent.insert(0, id);
            s.recent.truncate(RECENT_REPOSITORIES_LENGTH);
            s.foldout = None;
            let _ = s.store.save_selected_repository(Some(id));
            let _ = s.store.save_recent_repositories(&s.recent);
            cx.notify();
            true
        });
        if changed {
            Self::refresh_repository(id, cx);
        }
    }

    pub fn remove_repository(id: u64, cx: &mut App) {
        let state = Self::state(cx);
        let next = state.update(cx, |s, cx| {
            s.repositories.retain(|r| r.id != id);
            s.recent.retain(|r| *r != id);
            s.repo_states.remove(&id);
            let next = if s.selected == Some(id) {
                s.recent
                    .first()
                    .copied()
                    .or_else(|| s.repositories.first().map(|r| r.id))
            } else {
                s.selected
            };
            s.selected = next;
            persist_repositories(s);
            let _ = s.store.save_recent_repositories(&s.recent);
            let _ = s.store.save_selected_repository(next);
            cx.notify();
            next
        });
        if let Some(next) = next {
            Self::refresh_repository(next, cx);
        }
    }

    /// GHD `_refreshRepository`: re-read tip/branches/remotes and ahead/behind.
    pub fn refresh_repository(id: u64, cx: &mut App) {
        let state = Self::state(cx);
        let (path, git) = {
            let s = state.read(cx);
            let Some(repo) = s.repository(id) else {
                return;
            };
            (repo.path.clone(), s.git.clone())
        };
        state.update(cx, |s, cx| {
            s.repo_state_mut(id).loading = true;
            cx.notify();
        });
        let work = cx.background_executor().spawn(async move {
            let info = open_repository(&path)?;
            let ahead_behind = match (&git, info.current_branch()) {
                (Some(git), Some(branch)) => {
                    corvane_git::ahead_behind(git.clone(), &info.workdir, branch)
                        .unwrap_or_default()
                }
                _ => None,
            };
            Ok::<_, GitError>((info, ahead_behind))
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = work.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let repo_state: &mut RepositoryState = s.repo_state_mut(id);
                    repo_state.loading = false;
                    repo_state.last_refresh = Some(Instant::now());
                    match result {
                        Ok((info, ahead_behind)) => {
                            repo_state.info = Some(info);
                            repo_state.ahead_behind = ahead_behind;
                            repo_state.error = None;
                            if let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) {
                                repo.missing = false;
                            }
                        }
                        Err(GitError::NotARepository(_)) => {
                            repo_state.error = Some("repository is missing".into());
                            if let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) {
                                repo.missing = true;
                            }
                        }
                        Err(err) => {
                            warn!(id, %err, "refresh failed");
                            repo_state.error = Some(err.to_string());
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// Native folder picker → `add_repository` (GHD `AddRepository` shortcut).
    pub fn prompt_add_repository(cx: &mut App) {
        let receiver = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add Repository".into()),
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let picked = match receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            if let Some(path) = picked {
                cx.update(|cx| Self::add_repository(path, cx));
            }
        })
        .detach();
    }

    // ---- create / clone ----

    /// GHD `CreateRepository` dialog submit: `git init` (+ README commit), then add.
    pub fn create_repository(
        path: PathBuf,
        description: Option<String>,
        readme: bool,
        cx: &mut App,
    ) {
        let state = Self::state(cx);
        let Some(git) = state.read(cx).git.clone() else {
            Self::show_error("Git is not available", "Install git and retry.", cx);
            return;
        };
        Self::close_popup(cx);
        let task = cx.background_executor().spawn(async move {
            corvane_git::init_repository(
                git,
                InitOptions {
                    path,
                    default_branch: Some("main".into()),
                    description,
                    readme,
                },
            )
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| match result {
                Ok(path) => Self::add_repository(path, cx),
                Err(err) => Self::show_error("Could not create repository", err.to_string(), cx),
            });
        })
        .detach();
    }

    /// GHD `cloneRepository`: streams progress into `AppState::cloning`,
    /// adds the repository when done.
    pub fn clone_repository(url: String, path: PathBuf, cx: &mut App) {
        let state = Self::state(cx);
        let Some(git) = state.read(cx).git.clone() else {
            Self::show_error("Git is not available", "Install git and retry.", cx);
            return;
        };
        Self::close_popup(cx);
        state.update(cx, |s, cx| {
            s.cloning = Some(CloneState {
                url: url.clone(),
                path: path.clone(),
                description: "Cloning…".into(),
                value: None,
            });
            cx.notify();
        });

        let (tx, rx) = std::sync::mpsc::channel::<corvane_git::CloneProgress>();
        let clone_path = path.clone();
        let clone_url = url.clone();
        let task = cx.background_executor().spawn(async move {
            corvane_git::clone(git, &clone_url, &clone_path, |p| {
                let _ = tx.send(p);
            })
        });
        // Progress pump: poll the channel on the foreground at ~30 Hz while cloning.
        let pump_state = state.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            loop {
                let mut latest = None;
                while let Ok(p) = rx.try_recv() {
                    latest = Some(p);
                }
                if let Some(p) = latest {
                    let done = pump_state.update(cx, |s, cx| {
                        if let Some(c) = s.cloning.as_mut() {
                            c.description = p.description;
                            c.value = p.value;
                            cx.notify();
                            false
                        } else {
                            true
                        }
                    });
                    if done {
                        break;
                    }
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(33))
                    .await;
                if pump_state.read_with(cx, |s, _| s.cloning.is_none()) {
                    break;
                }
            }
        })
        .detach();

        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.cloning = None;
                    cx.notify();
                });
                match result {
                    Ok(()) => Self::add_repository(path, cx),
                    Err(err) => Self::show_error("Clone failed", err.to_string(), cx),
                }
            });
        })
        .detach();
    }

    pub fn open_url(url: &str, cx: &mut App) {
        cx.open_url(url);
    }

    /// Native folder picker → `Some(path)` on the foreground.
    pub fn pick_directory(
        prompt: &str,
        cx: &mut App,
        on_pick: impl FnOnce(Option<PathBuf>, &mut App) + 'static,
    ) {
        let receiver = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(prompt.to_string().into()),
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let picked = match receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            cx.update(|cx| on_pick(picked, cx));
        })
        .detach();
    }

    // ---- sign-in (GHD `SignInStore`) ----

    fn set_sign_in_step(step: SignInStep, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(si) = s.sign_in.as_mut() {
                si.step = step;
                cx.notify();
            }
        });
    }

    /// OAuth device flow against GitHub.com (or a GHES host with the same
    /// OAuth App registered). Runs on its own thread; progress is pumped to
    /// the foreground.
    pub fn sign_in_device_flow(endpoint: corvane_github::Endpoint, cx: &mut App) {
        let cancel = Arc::new(AtomicBool::new(false));
        Self::state(cx).update(cx, |s, cx| {
            if let Some(existing) = s.sign_in.as_ref() {
                existing.cancel.store(true, Ordering::SeqCst);
            }
            s.sign_in = Some(SignInState {
                endpoint: endpoint.api_base.clone(),
                step: SignInStep::Requesting,
                cancel: cancel.clone(),
            });
            cx.notify();
        });

        enum Msg {
            Code(corvane_github::auth::DeviceCode),
            Token { token: String, scopes: Vec<String> },
            Failed(String),
        }
        let (tx, rx) = std::sync::mpsc::channel::<Msg>();
        let worker_endpoint = endpoint.clone();
        let worker_cancel = cancel.clone();
        std::thread::Builder::new()
            .name("device-flow".into())
            .spawn(move || {
                let code = match corvane_github::auth::request_device_code_default(&worker_endpoint)
                {
                    Ok(code) => code,
                    Err(err) => {
                        let _ = tx.send(Msg::Failed(err.to_string()));
                        return;
                    }
                };
                let mut interval = code.poll_interval();
                let deadline = std::time::Instant::now()
                    + std::time::Duration::from_secs(code.expires_in.max(60));
                let device_code = code.device_code.clone();
                let _ = tx.send(Msg::Code(code));
                loop {
                    if worker_cancel.load(Ordering::SeqCst) {
                        return;
                    }
                    if std::time::Instant::now() > deadline {
                        let _ =
                            tx.send(Msg::Failed("the sign-in request expired; try again".into()));
                        return;
                    }
                    std::thread::sleep(interval);
                    if worker_cancel.load(Ordering::SeqCst) {
                        return;
                    }
                    match corvane_github::auth::poll_token(
                        &worker_endpoint,
                        corvane_github::CLIENT_ID,
                        &device_code,
                    ) {
                        Ok(corvane_github::auth::PollOutcome::Pending) => {}
                        Ok(corvane_github::auth::PollOutcome::SlowDown) => {
                            interval += std::time::Duration::from_secs(5);
                        }
                        Ok(corvane_github::auth::PollOutcome::Token { token, scopes }) => {
                            let _ = tx.send(Msg::Token { token, scopes });
                            return;
                        }
                        Err(err) => {
                            let _ = tx.send(Msg::Failed(err.to_string()));
                            return;
                        }
                    }
                }
            })
            .ok();

        cx.spawn(async move |cx: &mut AsyncApp| {
            loop {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                let mut finished = false;
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        Msg::Code(code) => {
                            let uri = code.verification_uri.clone();
                            cx.update(|cx| {
                                Self::set_sign_in_step(
                                    SignInStep::DeviceCode {
                                        user_code: code.user_code.clone(),
                                        verification_uri: uri.clone(),
                                    },
                                    cx,
                                );
                                cx.open_url(&uri);
                            });
                        }
                        Msg::Token { token, scopes } => {
                            let endpoint = endpoint.clone();
                            cx.update(|cx| Self::finish_sign_in(endpoint, token, scopes, cx));
                            finished = true;
                        }
                        Msg::Failed(err) => {
                            cx.update(|cx| Self::set_sign_in_step(SignInStep::Error(err), cx));
                            finished = true;
                        }
                    }
                }
                if finished {
                    break;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
            }
        })
        .detach();
    }

    /// Personal access token (GHES, or the fallback link on GitHub.com).
    pub fn sign_in_with_token(endpoint: corvane_github::Endpoint, token: String, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.sign_in = Some(SignInState {
                endpoint: endpoint.api_base.clone(),
                step: SignInStep::Verifying,
                cancel: Arc::new(AtomicBool::new(false)),
            });
            cx.notify();
        });
        Self::finish_sign_in(endpoint, token, Vec::new(), cx);
    }

    /// Token → account (background), keychain + store, then close the dialog.
    fn finish_sign_in(
        endpoint: corvane_github::Endpoint,
        token: String,
        scopes: Vec<String>,
        cx: &mut App,
    ) {
        Self::set_sign_in_step(SignInStep::Verifying, cx);
        let task = cx.background_executor().spawn({
            let endpoint = endpoint.clone();
            let token = token.clone();
            async move {
                let client = corvane_github::Client::new(endpoint, token.clone());
                let account = client.current_user(scopes)?;
                corvane_platform::keychain::store_token(&account.host(), &account.login, &token)
                    .map_err(|e| corvane_github::GitHubError::Auth(e.to_string()))?;
                Ok::<Account, corvane_github::GitHubError>(account)
            }
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| match result {
                Ok(account) => {
                    info!(login = %account.login, endpoint = %account.endpoint, "signed in");
                    Self::state(cx).update(cx, |s, cx| {
                        s.accounts.retain(|a| a.endpoint != account.endpoint);
                        s.accounts.push(account);
                        if let Err(err) = s.store.save_accounts(&s.accounts) {
                            error!(?err, "could not save accounts");
                        }
                        s.sign_in = None;
                        if matches!(s.popup, Some(Popup::SignIn { .. })) {
                            s.popup = None;
                        }
                        cx.notify();
                    });
                }
                Err(err) => Self::set_sign_in_step(SignInStep::Error(err.to_string()), cx),
            });
        })
        .detach();
    }

    pub fn cancel_sign_in(cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(si) = s.sign_in.take() {
                si.cancel.store(true, Ordering::SeqCst);
                cx.notify();
            }
        });
    }

    pub fn sign_out(endpoint: String, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(account) = s.accounts.iter().find(|a| a.endpoint == endpoint).cloned() {
                let _ = corvane_platform::keychain::delete_token(&account.host(), &account.login);
            }
            s.accounts.retain(|a| a.endpoint != endpoint);
            let _ = s.store.save_accounts(&s.accounts);
            cx.notify();
        });
    }

    // ---- welcome / identity ----

    /// `git config --global user.name/user.email` (GHD `ConfigureGitUser` save).
    pub fn set_global_identity(name: String, email: String, cx: &mut App) {
        let Some(git) = Self::state(cx).read(cx).git.clone() else {
            return;
        };
        let task = cx
            .background_executor()
            .spawn(async move { corvane_git::set_global_identity(git, &name, &email) });
        cx.spawn(async move |cx: &mut AsyncApp| {
            if let Err(err) = task.await {
                cx.update(|cx| {
                    Self::show_error("Could not save Git identity", err.to_string(), cx)
                });
            }
        })
        .detach();
    }

    pub fn complete_welcome(cx: &mut App) {
        Self::update_settings(cx, |s| s.welcome_completed = true);
    }

    // ---- settings ----

    pub fn update_settings(cx: &mut App, edit: impl FnOnce(&mut Settings)) {
        Self::state(cx).update(cx, |s, cx| {
            edit(&mut s.settings);
            if let Err(err) = s.store.save_settings(&s.settings) {
                error!(?err, "could not save settings");
            }
            cx.notify();
        });
    }
}

fn persist_repositories(s: &mut AppState) {
    if let Err(err) = s.store.save_repositories(&s.repositories) {
        error!(?err, "could not save repositories");
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
