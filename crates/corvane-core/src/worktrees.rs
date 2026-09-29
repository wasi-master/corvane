//! Worktrees (GHD `app-store.ts` `_switchWorktree`, `_deleteWorktree`,
//! `_moveWorktree`, `_requestDeleteWorktree`, `recoverMissingWorktree`): a
//! repository entry keeps its id and follows the worktree path it is switched
//! to, and remembers its main worktree (`mainWorktreePath`) so that when the
//! linked worktree it points at is deleted outside Corvane it falls back to
//! the main worktree instead of showing a stale repository.
//!
//! Deviation: GHD records `mainWorktreePath` when it switches worktrees and
//! otherwise asks the worktree's git dir; Corvane also records it on every
//! refresh (from `git worktree list`), since `git worktree remove` deletes the
//! git dir metadata GHD's fallback reads.

use std::path::{Path, PathBuf};

use corvane_git::open_repository;
use corvane_models::{BranchKind, WorktreeType};
use gpui_kit::App;
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::persistence::StoreExt;
use crate::remote::spawn_bg;
use crate::state::Popup;

impl Dispatcher {
    /// GHD `_switchWorktree`: point the repository at `path` and reload it.
    pub fn switch_worktree(id: u64, path: PathBuf, cx: &mut App) {
        let probe = path.clone();
        spawn_bg(
            cx,
            move || {
                open_repository(&probe)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            },
            move |result, cx| {
                if let Err(err) = result {
                    Self::show_error(
                        "Could not switch worktree",
                        format!(
                            "The worktree path '{}' does not appear to be a valid Git repository.\n{err}",
                            path.display()
                        ),
                        cx,
                    );
                    return;
                }
                Self::apply_worktree_path(id, path, cx);
            },
        );
    }

    /// Repoint the repository entry (persisted) and refresh + rewatch it.
    fn apply_worktree_path(id: u64, path: PathBuf, cx: &mut App) {
        let changed = Self::state(cx).update(cx, |s, cx| {
            let main = s
                .repo_states
                .get(&id)
                .and_then(|rs| rs.worktrees.iter().find(|w| w.kind == WorktreeType::Main))
                .map(|w| w.path.clone());
            let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) else {
                return false;
            };
            if repo.path == path {
                return false;
            }
            info!(id, path = %path.display(), "switching worktree");
            if let Some(main) = main {
                repo.main_worktree_path = Some(main);
            }
            repo.missing = false;
            repo.path = path;
            if let Err(err) = s.store.save_repositories(&s.repositories) {
                warn!(%err, "could not persist repository path");
            }
            // force the file watcher onto the new directory
            if s.watched_repo == Some(id) {
                s.watched_repo = None;
                s.watcher = None;
            }
            cx.notify();
            true
        });
        if changed {
            Self::refresh_repository(id, cx);
            Self::start_watching(id, cx);
        }
    }

    /// GHD `AddWorktreeDialog.onSubmit`: create the worktree for `branch`
    /// (checked out if it exists locally or on a remote, created otherwise)
    /// and switch to it.
    pub fn add_worktree(id: u64, path: PathBuf, branch: String, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let branches = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.info.as_ref())
            .map(|i| i.branches.clone())
            .unwrap_or_default();
        let existing = branches.iter().find(|b| b.name == branch).cloned();
        let target = path.clone();
        spawn_bg(
            cx,
            move || {
                let (create, commitish) = match &existing {
                    Some(b) if b.kind == BranchKind::Remote => (
                        Some(b.name_without_remote().to_string()),
                        Some(b.full_name.clone()),
                    ),
                    Some(b) => (None, Some(b.name.clone())),
                    None => (Some(branch.clone()), None),
                };
                corvane_git::add_worktree(
                    git.clone(),
                    &workdir,
                    &target,
                    create.as_deref(),
                    commitish.as_deref(),
                )
                .map_err(|e| e.to_string())?;
                let worktrees =
                    corvane_git::list_worktrees(git, &workdir).map_err(|e| e.to_string())?;
                if !worktrees.iter().any(|w| same_path(&w.path, &target)) {
                    return Err("Failed to find the newly created worktree".to_string());
                }
                Ok(target)
            },
            move |result, cx| match result {
                Ok(path) => {
                    Self::close_popup(cx);
                    Self::apply_worktree_path(id, path, cx);
                }
                Err(err) => Self::show_error("Could not create worktree", err, cx),
            },
        );
    }

    /// GHD `_requestDeleteWorktree`: confirm unless the user opted out.
    pub fn request_delete_worktree(id: u64, path: PathBuf, cx: &mut App) {
        if Self::state(cx).read(cx).settings.confirm_worktree_removal {
            Self::show_popup(Popup::DeleteWorktree { repo: id, path }, cx);
        } else {
            Self::delete_worktree(id, path, false, cx);
        }
    }

    /// GHD `_deleteWorktree`: switch to the main worktree first when the
    /// current one is being removed; a failure offers `--force`.
    pub fn delete_worktree(id: u64, path: PathBuf, force: bool, cx: &mut App) {
        let (current, main) = {
            let s = Self::state(cx).read(cx);
            let Some(repo) = s.repository(id) else { return };
            let main = s
                .repo_states
                .get(&id)
                .and_then(|rs| rs.worktrees.iter().find(|w| w.kind == WorktreeType::Main))
                .map(|w| w.path.clone());
            (repo.path.clone(), main)
        };
        let deleting_current = same_path(&current, &path);
        let original = deleting_current.then(|| current.clone());
        let base = if deleting_current {
            match main {
                Some(main) => {
                    Self::apply_worktree_path(id, main.clone(), cx);
                    main
                }
                None => {
                    Self::show_error(
                        "Could not delete worktree",
                        "Could not find main worktree",
                        cx,
                    );
                    return;
                }
            }
        } else {
            current
        };
        let Some(git) = Self::state(cx).read(cx).git.clone() else {
            return;
        };
        let target = path.clone();
        spawn_bg(
            cx,
            move || {
                corvane_git::remove_worktree(git, &base, &target, force).map_err(|e| e.to_string())
            },
            move |result, cx| {
                match result {
                    Ok(()) => info!(id, path = %path.display(), "worktree removed"),
                    Err(error) => {
                        warn!(%error, "worktree removal failed");
                        Self::show_popup(
                            Popup::DeleteWorktreeFailed {
                                repo: id,
                                path,
                                error,
                                original,
                            },
                            cx,
                        );
                    }
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// GHD `_moveWorktree` (Rename…): the current worktree follows its new path.
    pub fn move_worktree(id: u64, old: PathBuf, new: PathBuf, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (old_for_task, new_for_task) = (old.clone(), new.clone());
        spawn_bg(
            cx,
            move || {
                corvane_git::move_worktree(git, &workdir, &old_for_task, &new_for_task)
                    .map_err(|e| e.to_string())
            },
            move |result, cx| match result {
                Ok(()) => {
                    Self::close_popup(cx);
                    let current = Self::state(cx)
                        .read(cx)
                        .repository(id)
                        .map(|r| r.path.clone());
                    if current.is_some_and(|c| same_path(&c, &old)) {
                        Self::apply_worktree_path(id, new, cx);
                    } else {
                        Self::refresh_repository(id, cx);
                    }
                }
                Err(err) => Self::show_error("Could not rename worktree", err, cx),
            },
        );
    }
}

impl Dispatcher {
    /// GHD `recoverMissingWorktree`, run by `refresh_repository` when the
    /// repository's path no longer exists: switch the entry to its main
    /// worktree (the recorded `main_worktree_path`, else the main worktree of
    /// the last `git worktree list`), or select the repository entry that
    /// already points there. With no main worktree to go to, the repository
    /// is marked missing as before.
    pub(crate) fn recover_missing_worktree(id: u64, missing_path: PathBuf, cx: &mut App) {
        let (main, existing) = {
            let s = Self::state(cx).read(cx);
            let listed = s
                .repo_states
                .get(&id)
                .and_then(|rs| rs.worktrees.iter().find(|w| w.kind == WorktreeType::Main))
                .map(|w| w.path.clone());
            let main = s
                .repository(id)
                .and_then(|r| r.main_worktree_path.clone())
                .into_iter()
                .chain(listed)
                .find(|p| *p != missing_path && p.exists());
            let existing = main.as_ref().and_then(|main| {
                s.repositories
                    .iter()
                    .find(|r| r.id != id && same_path(&r.path, main))
                    .map(|r| r.id)
            });
            (main, existing)
        };
        let Some(main) = main else {
            Self::mark_missing(id, cx);
            return;
        };
        let probe = main.clone();
        spawn_bg(
            cx,
            move || open_repository(&probe).map(|_| ()),
            move |result, cx| {
                if result.is_err() {
                    Self::mark_missing(id, cx);
                    return;
                }
                info!(
                    id,
                    from = %missing_path.display(),
                    to = %main.display(),
                    "worktree is gone, falling back to the main worktree"
                );
                match existing {
                    // `switchWorktree` found the main worktree already listed
                    Some(other) => {
                        if Self::state(cx).read(cx).selected == Some(id) {
                            Self::select_repository(other, cx);
                        }
                    }
                    None => Self::apply_worktree_path(id, main, cx),
                }
            },
        );
    }

    /// `_updateRepositoryMissing(repository, true)`.
    fn mark_missing(id: u64, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).error = Some("repository is missing".into());
            if let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) {
                repo.missing = true;
            }
            cx.notify();
        });
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
