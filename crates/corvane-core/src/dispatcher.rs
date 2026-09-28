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
    AppState, CloneState, Foldout, LastCommit, Popup, RepositoryState, SignInState, SignInStep,
};
use corvane_models::{Account, DiffSelectionType, Repository, github_from_remote};
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
            watcher: None,
            watched_repo: None,
        });
        AppState::install(state.clone(), cx);

        if let Some(id) = state.read(cx).selected {
            Self::refresh_repository(id, cx);
            Self::start_watching(id, cx);
        }
        state
    }

    /// Watch the repository's worktree; each debounced change triggers a refresh.
    pub fn start_watching(id: u64, cx: &mut App) {
        let state = Self::state(cx);
        let path = {
            let s = state.read(cx);
            if s.watched_repo == Some(id) {
                return;
            }
            let Some(repo) = s.repository(id) else {
                return;
            };
            repo.path.clone()
        };
        match crate::watcher::watch(path.clone()) {
            Ok((watcher, rx)) => {
                state.update(cx, |s, _| {
                    s.watcher = Some(watcher);
                    s.watched_repo = Some(id);
                });
                let state = state.clone();
                cx.spawn(async move |cx: &mut AsyncApp| {
                    while rx.recv().await.is_ok() {
                        let still_watched = state.read_with(cx, |s, _| s.watched_repo == Some(id));
                        if !still_watched {
                            break;
                        }
                        cx.update(|cx| Self::refresh_repository(id, cx));
                    }
                })
                .detach();
            }
            Err(err) => warn!(?err, path = %path.display(), "could not watch repository"),
        }
    }

    /// GHD refreshes the selected repository when the window regains focus.
    pub fn refresh_selected(cx: &mut App) {
        if let Some(id) = Self::state(cx).read(cx).selected {
            Self::refresh_repository(id, cx);
        }
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
            Self::start_watching(id, cx);
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

    /// GHD `_refreshRepository`: re-read tip/branches/remotes, ahead/behind
    /// and working-directory status; then reload the selected diff.
    pub fn refresh_repository(id: u64, cx: &mut App) {
        let state = Self::state(cx);
        let (path, git, previous_status) = {
            let s = state.read(cx);
            let Some(repo) = s.repository(id) else {
                return;
            };
            (
                repo.path.clone(),
                s.git.clone(),
                s.repo_states.get(&id).and_then(|r| r.status.clone()),
            )
        };
        let already_running = state.update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.loading {
                rs.refresh_pending = true;
                return true;
            }
            rs.loading = true;
            cx.notify();
            false
        });
        if already_running {
            return;
        }
        let work = cx.background_executor().spawn(async move {
            let info = open_repository(&path)?;
            let (ahead_behind, status) = match &git {
                Some(git) => {
                    let ab = info.current_branch().and_then(|b| {
                        corvane_git::ahead_behind(git.clone(), &info.workdir, b)
                            .ok()
                            .flatten()
                    });
                    let status = corvane_git::get_status(
                        git.clone(),
                        &info.workdir,
                        previous_status.as_ref(),
                    )?;
                    (ab, Some(status))
                }
                None => (None, None),
            };
            Ok::<_, GitError>((info, ahead_behind, status))
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = work.await;
            cx.update(|cx| {
                let selected_file = Self::state(cx).update(cx, |s, cx| {
                    let repo_state: &mut RepositoryState = s.repo_state_mut(id);
                    repo_state.loading = false;
                    repo_state.last_refresh = Some(Instant::now());
                    let mut selected = None;
                    match result {
                        Ok((info, ahead_behind, status)) => {
                            repo_state.info = Some(info);
                            repo_state.ahead_behind = ahead_behind;
                            repo_state.error = None;
                            if let Some(status) = status {
                                // keep the selection if the file is still changed, else first file
                                let keep = repo_state
                                    .selected_file
                                    .as_ref()
                                    .filter(|p| status.files.iter().any(|f| &f.path == *p))
                                    .cloned();
                                repo_state.selected_file =
                                    keep.or_else(|| status.files.first().map(|f| f.path.clone()));
                                if repo_state.selected_file.is_none() {
                                    repo_state.diff = None;
                                }
                                selected = repo_state.selected_file.clone();
                                repo_state.status = Some(status);
                            }
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
                    selected
                });
                if selected_file.is_some() {
                    Self::load_diff(id, cx);
                }
                Self::load_commits(id, false, cx);
                let rerun = Self::state(cx).update(cx, |s, _| {
                    let rs = s.repo_state_mut(id);
                    std::mem::take(&mut rs.refresh_pending)
                });
                if rerun {
                    Self::refresh_repository(id, cx);
                }
            });
        })
        .detach();
    }

    // ---- changes list ----

    /// GHD `_changeChangesSelection`: select a file and load its diff.
    pub fn select_file(id: u64, path: String, cx: &mut App) {
        let changed = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.selected_file.as_deref() == Some(path.as_str()) {
                return false;
            }
            rs.selected_file = Some(path);
            rs.diff = None;
            cx.notify();
            true
        });
        if changed {
            Self::load_diff(id, cx);
        }
    }

    pub fn load_diff(id: u64, cx: &mut App) {
        let state = Self::state(cx);
        let (git, workdir, file) = {
            let s = state.read(cx);
            let Some(git) = s.git.clone() else { return };
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let Some(info) = rs.info.as_ref() else { return };
            let Some(path) = rs.selected_file.as_ref() else {
                return;
            };
            let Some(file) = rs
                .status
                .as_ref()
                .and_then(|st| st.files.iter().find(|f| &f.path == path))
                .cloned()
            else {
                return;
            };
            (git, info.workdir.clone(), file)
        };
        let path = file.path.clone();
        state.update(cx, |s, cx| {
            s.repo_state_mut(id).diff_loading = true;
            cx.notify();
        });
        let work = cx
            .background_executor()
            .spawn(async move { corvane_git::working_directory_diff(git, &workdir, &file) });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = work.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    // ignore stale results
                    if rs.selected_file.as_deref() != Some(path.as_str()) {
                        return;
                    }
                    rs.diff_loading = false;
                    match result {
                        Ok(diff) => rs.diff = Some(diff),
                        Err(err) => {
                            warn!(%err, "diff failed");
                            rs.diff = Some(corvane_models::Diff::Empty);
                        }
                    }
                    rs.diff_generation += 1;
                    // GHD `updateChangesWorkingDirectoryDiff`: bound the file's
                    // selection to the lines that exist in this diff.
                    let selectable: std::collections::BTreeSet<u32> = match rs.diff.as_ref() {
                        Some(corvane_models::Diff::Text { hunks, .. }) => hunks
                            .iter()
                            .flat_map(|h| {
                                h.lines.iter().enumerate().filter_map(move |(i, l)| {
                                    matches!(
                                        l.kind,
                                        corvane_models::DiffLineKind::Add
                                            | corvane_models::DiffLineKind::Delete
                                    )
                                    .then_some(h.unified_diff_start + i as u32)
                                })
                            })
                            .collect(),
                        _ => Default::default(),
                    };
                    if let Some(f) = rs
                        .status
                        .as_mut()
                        .and_then(|st| st.files.iter_mut().find(|f| f.path == path))
                    {
                        f.selection = f.selection.with_selectable_lines(selectable);
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    // ---- history (GHD `_loadHistory`, `_loadNextCommitBatch`, `_changeCommitSelection`) ----

    /// Load the first page of HEAD's history, or the next one when `more`.
    pub fn load_commits(id: u64, more: bool, cx: &mut App) {
        let state = Self::state(cx);
        let (workdir, skip) = {
            let s = state.read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            if rs.commits_loading || (more && rs.commits_exhausted) {
                return;
            }
            let Some(info) = rs.info.as_ref() else { return };
            (
                info.workdir.clone(),
                if more { rs.commits.len() } else { 0 },
            )
        };
        state.update(cx, |s, _| s.repo_state_mut(id).commits_loading = true);
        let task = cx.background_executor().spawn(async move {
            corvane_git::get_commits(&workdir, "HEAD", skip, corvane_git::COMMIT_BATCH_SIZE)
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                let reselect = Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    rs.commits_loading = false;
                    match result {
                        Ok(batch) => {
                            rs.commits_exhausted = batch.len() < corvane_git::COMMIT_BATCH_SIZE;
                            if more {
                                rs.commits.extend(batch);
                            } else {
                                rs.commits = batch;
                            }
                            if let Some(sha) = rs.selected_commit.clone()
                                && !rs.commits.iter().any(|c| c.sha == sha)
                            {
                                rs.selected_commit = None;
                                rs.changeset = None;
                                rs.commit_selected_file = None;
                                rs.commit_diff = None;
                            }
                        }
                        Err(err) => warn!(id, %err, "history failed"),
                    }
                    cx.notify();
                    if more {
                        None
                    } else {
                        rs.selected_commit.clone()
                    }
                });
                if let Some(sha) = reselect {
                    Self::load_changeset(id, sha, cx);
                }
            });
        })
        .detach();
    }

    pub fn select_commit(id: u64, sha: String, cx: &mut App) {
        let changed = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.selected_commit.as_deref() == Some(sha.as_str()) {
                return false;
            }
            rs.selected_commit = Some(sha.clone());
            rs.changeset = None;
            rs.commit_selected_file = None;
            rs.commit_diff = None;
            cx.notify();
            true
        });
        if changed {
            Self::load_changeset(id, sha, cx);
        }
    }

    fn load_changeset(id: u64, sha: String, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let sha_for_task = sha.clone();
        let task = cx
            .background_executor()
            .spawn(async move { corvane_git::get_changed_files(git, &workdir, &sha_for_task) });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                let load = Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    if rs.selected_commit.as_deref() != Some(sha.as_str()) {
                        return false;
                    }
                    match result {
                        Ok(data) => {
                            // keep the file selection when the same path is still there
                            let keep = rs
                                .commit_selected_file
                                .as_ref()
                                .filter(|p| data.files.iter().any(|f| &f.path == *p))
                                .cloned();
                            rs.commit_selected_file =
                                keep.or_else(|| data.files.first().map(|f| f.path.clone()));
                            rs.changeset = Some(data);
                        }
                        Err(err) => warn!(id, %err, "changed files failed"),
                    }
                    cx.notify();
                    true
                });
                if load {
                    Self::load_commit_diff(id, cx);
                }
            });
        })
        .detach();
    }

    pub fn select_commit_file(id: u64, path: String, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).commit_selected_file = Some(path);
            cx.notify();
        });
        Self::load_commit_diff(id, cx);
    }

    fn load_commit_diff(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some(file) = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| {
                let path = rs.commit_selected_file.as_ref()?;
                rs.changeset
                    .as_ref()?
                    .files
                    .iter()
                    .find(|f| &f.path == path)
                    .cloned()
            })
        else {
            return;
        };
        let key = (file.commitish.clone(), file.path.clone());
        let task = cx
            .background_executor()
            .spawn(async move { corvane_git::commit_file_diff(git, &workdir, &file) });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    if rs.selected_commit.as_deref() != Some(key.0.as_str())
                        || rs.commit_selected_file.as_deref() != Some(key.1.as_str())
                    {
                        return;
                    }
                    rs.commit_diff = Some(match result {
                        Ok(diff) => diff,
                        Err(err) => {
                            warn!(id, %err, "commit diff failed");
                            corvane_models::Diff::Empty
                        }
                    });
                    rs.commit_diff_generation += 1;
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub fn set_commit_summary_expanded(id: u64, expanded: bool, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).commit_summary_expanded = expanded;
            cx.notify();
        });
    }

    /// Toggle the include checkbox of one file (`_changeFileIncluded`).
    pub fn toggle_file_included(id: u64, path: String, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(status) = s.repo_state_mut(id).status.as_mut() {
                if let Some(f) = status.files.iter_mut().find(|f| f.path == path) {
                    // GHD: an indeterminate checkbox click checks it (Partial -> All)
                    f.selection = if f.selection.kind() == DiffSelectionType::All {
                        f.selection.select_none()
                    } else {
                        f.selection.select_all()
                    };
                }
                cx.notify();
            }
        });
    }

    /// Gutter click: toggle one diff line (`withToggleLineSelection`).
    pub fn toggle_diff_line(id: u64, path: String, line: u32, cx: &mut App) {
        Self::update_selection(id, &path, cx, |sel| sel.toggled(line));
    }

    /// Hunk handle / drag selection: mark `len` lines from `from`.
    pub fn set_diff_lines(
        id: u64,
        path: String,
        from: u32,
        len: u32,
        selected: bool,
        cx: &mut App,
    ) {
        Self::update_selection(id, &path, cx, |sel| sel.with_range(from, len, selected));
    }

    fn update_selection(
        id: u64,
        path: &str,
        cx: &mut App,
        edit: impl FnOnce(&corvane_models::DiffSelection) -> corvane_models::DiffSelection,
    ) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(f) = s
                .repo_state_mut(id)
                .status
                .as_mut()
                .and_then(|st| st.files.iter_mut().find(|f| f.path == path))
            {
                f.selection = edit(&f.selection);
                cx.notify();
            }
        });
    }

    /// GHD `onIncludeChanged(files, include)`: the header checkbox applies to
    /// the files currently visible through the filter.
    pub fn set_files_included(id: u64, paths: Vec<String>, include: bool, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(status) = s.repo_state_mut(id).status.as_mut() {
                for f in status.files.iter_mut().filter(|f| paths.contains(&f.path)) {
                    f.selection = if include {
                        f.selection.select_all()
                    } else {
                        f.selection.select_none()
                    };
                }
                cx.notify();
            }
        });
    }

    /// Filter Options popover checkbox.
    pub fn toggle_filter_option(id: u64, option: crate::state::FilterOption, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            let f = &mut s.repo_state_mut(id).file_list_filter;
            let on = !f.get(option);
            f.set(option, on);
            cx.notify();
        });
    }

    /// "Clear filters" (the text box is cleared by the view).
    pub fn clear_filter_options(id: u64, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).file_list_filter = Default::default();
            cx.notify();
        });
    }

    /// Header checkbox (`_changeIncludeAllFiles`).
    pub fn toggle_include_all(id: u64, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(status) = s.repo_state_mut(id).status.as_mut() {
                let select_all = status.include_all() != Some(true);
                for f in &mut status.files {
                    f.selection = if select_all {
                        f.selection.select_all()
                    } else {
                        f.selection.select_none()
                    };
                }
                cx.notify();
            }
        });
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

    // ---- commit / undo / discard (GHD `_commitIncludedChanges`, `_undoCommit`, `_discardChanges`) ----

    fn repo_context(
        id: u64,
        cx: &App,
    ) -> Option<(Arc<corvane_git::GitBinary>, std::path::PathBuf)> {
        let s = Self::state(cx).read(cx);
        let git = s.git.clone()?;
        let workdir = s.repo_states.get(&id)?.info.as_ref()?.workdir.clone();
        Some((git, workdir))
    }

    pub fn commit(id: u64, summary: String, description: String, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let files: Vec<_> = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.status.as_ref())
            .map(|st| {
                st.files
                    .iter()
                    .filter(|f| f.selection.kind() != DiffSelectionType::None)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let options = Self::state(cx)
            .read(cx)
            .repository(id)
            .map(|r| r.commit_options)
            .unwrap_or_default();
        if summary.trim().is_empty() || (files.is_empty() && !options.allow_empty_commit) {
            return;
        }
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).committing = true;
            cx.notify();
        });
        let message = corvane_git::format_message(&summary, &description);
        let summary_for_bar = summary.trim().to_string();
        let task = cx.background_executor().spawn(async move {
            corvane_git::unstage_all(git.clone(), &workdir)?;
            corvane_git::stage_files(git.clone(), &workdir, &files)?;
            corvane_git::stage_partial_files(git.clone(), &workdir, &files)?;
            corvane_git::commit(
                git,
                &workdir,
                &message,
                &corvane_git::CommitOptions {
                    amend: false,
                    no_verify: options.skip_commit_hooks,
                    signoff: options.sign_off_commits,
                    allow_empty: options.allow_empty_commit,
                },
            )
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    rs.committing = false;
                    if let Ok(sha) = &result {
                        rs.last_commit = Some(LastCommit {
                            sha: sha.clone(),
                            summary: summary_for_bar.clone(),
                            at: std::time::SystemTime::now(),
                        });
                        rs.commit_nonce += 1;
                    }
                    cx.notify();
                });
                if let Err(err) = result {
                    Self::show_error("Could not commit", err.to_string(), cx);
                }
                Self::refresh_repository(id, cx);
            });
        })
        .detach();
    }

    /// Commit form gear menu (`onUpdateCommitOptions`), persisted with the repository.
    pub fn update_commit_options(
        id: u64,
        edit: impl FnOnce(&mut corvane_models::RepoCommitOptions),
        cx: &mut App,
    ) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) {
                edit(&mut repo.commit_options);
                persist_repositories(s);
                cx.notify();
            }
        });
    }

    pub fn undo_commit(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let task = cx
            .background_executor()
            .spawn(async move { corvane_git::undo_last_commit(git, &workdir) });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.repo_state_mut(id).last_commit = None;
                    cx.notify();
                });
                if let Err(err) = result {
                    Self::show_error("Could not undo commit", err.to_string(), cx);
                }
                Self::refresh_repository(id, cx);
            });
        })
        .detach();
    }

    /// Discard the given paths' changes (after the confirmation prompt).
    pub fn discard_changes(id: u64, paths: Vec<String>, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let files: Vec<_> = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.status.as_ref())
            .map(|st| {
                st.files
                    .iter()
                    .filter(|f| paths.contains(&f.path))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if files.is_empty() {
            return;
        }
        let task = cx
            .background_executor()
            .spawn(async move { corvane_git::discard_changes(git, &workdir, &files, true) });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                if let Err(err) = result {
                    Self::show_error("Could not discard changes", err.to_string(), cx);
                }
                Self::refresh_repository(id, cx);
            });
        })
        .detach();
    }

    /// GHD `onDiscardChangesFromFiles`: confirm first unless the user opted out.
    pub fn request_discard_changes(id: u64, paths: Vec<String>, cx: &mut App) {
        if paths.is_empty() {
            return;
        }
        let (confirm, total) = {
            let s = Self::state(cx).read(cx);
            let total = s
                .repo_states
                .get(&id)
                .and_then(|r| r.status.as_ref())
                .map(|st| st.files.len())
                .unwrap_or(0);
            (s.settings.confirm_discard_changes, total)
        };
        if confirm {
            let all = paths.len() == total;
            Self::show_popup(
                Popup::DiscardChanges {
                    repo: id,
                    paths,
                    all,
                },
                cx,
            );
        } else {
            Self::discard_changes(id, paths, cx);
        }
    }

    /// "Ignore File / Folder" menu items: paths are escaped before writing.
    pub fn ignore_files(id: u64, paths: Vec<String>, cx: &mut App) {
        let patterns = paths
            .iter()
            .map(|p| corvane_git::escape_gitignore_pattern(p))
            .collect();
        Self::ignore_patterns(id, patterns, cx);
    }

    /// Append raw patterns (e.g. `*.log`) to the root `.gitignore`, then refresh.
    pub fn ignore_patterns(id: u64, patterns: Vec<String>, cx: &mut App) {
        let Some((_git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let task = cx
            .background_executor()
            .spawn(async move { corvane_git::append_ignore_rules(&workdir, &patterns) });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                if let Err(err) = result {
                    Self::show_error("Could not update .gitignore", err.to_string(), cx);
                }
                Self::refresh_repository(id, cx);
            });
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
