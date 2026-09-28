//! `Dispatcher`: the only thing that mutates `AppState`. Every method is
//! callable from the UI with an `&mut App`; git and disk work runs on the
//! background executor and results are applied on the main thread.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use corvane_git::{GitError, find_git, open_repository};
use corvane_store::Store;
use gpui_kit::{App, AppContext, AsyncApp, Entity};
use tracing::{error, info, warn};

use crate::persistence::{Settings, StoreExt};
use crate::state::{AppState, Foldout, Popup, RepositoryState};
use corvane_models::{Repository, github_from_remote};

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

        let state = cx.new(|_| AppState {
            store,
            settings,
            git: None,
            git_error: None,
            repositories,
            recent,
            selected,
            repo_states: Default::default(),
            accounts,
            foldout: None,
            popup: None,
        });
        AppState::install(state.clone(), cx);

        Self::detect_git(cx);
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
