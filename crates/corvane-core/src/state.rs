//! `AppState`: everything the UI renders from. Lives in one GPUI entity that
//! views observe; only the `Dispatcher` mutates it.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use corvane_git::GitBinary;
use corvane_store::Store;
use gpui_kit::{App, Entity, Global};

use crate::persistence::Settings;
use corvane_models::{Account, AheadBehind, Repository, RepositoryInfo, Section};

/// Which toolbar foldout is open (`FoldoutType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foldout {
    Repository,
    Branch,
    PushPull,
}

/// Modal dialogs (`PopupType`, the subset Corvane has so far).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Popup {
    InstallGit { reason: String },
    Error { title: String, message: String },
}

/// Per-repository cache (`IRepositoryState`, trimmed).
#[derive(Clone, Debug, Default)]
pub struct RepositoryState {
    pub info: Option<RepositoryInfo>,
    pub ahead_behind: Option<AheadBehind>,
    pub loading: bool,
    pub error: Option<String>,
    pub last_refresh: Option<Instant>,
    pub section: Section,
}

pub struct AppState {
    pub store: Arc<Store>,
    pub settings: Settings,
    pub git: Option<Arc<GitBinary>>,
    pub git_error: Option<String>,
    pub repositories: Vec<Repository>,
    /// Most recent first, max 3 (GHD `RecentRepositoriesLength`).
    pub recent: Vec<u64>,
    pub selected: Option<u64>,
    pub repo_states: HashMap<u64, RepositoryState>,
    pub accounts: Vec<Account>,
    pub foldout: Option<Foldout>,
    pub popup: Option<Popup>,
}

struct AppStateHandle(Entity<AppState>);
impl Global for AppStateHandle {}

impl AppState {
    pub(crate) fn install(entity: Entity<AppState>, cx: &mut App) {
        cx.set_global(AppStateHandle(entity));
    }

    /// The single app-state entity. Panics if `Dispatcher::init` has not run.
    pub fn global(cx: &App) -> Entity<AppState> {
        cx.global::<AppStateHandle>().0.clone()
    }

    pub fn repository(&self, id: u64) -> Option<&Repository> {
        self.repositories.iter().find(|r| r.id == id)
    }

    pub fn selected_repository(&self) -> Option<&Repository> {
        self.selected.and_then(|id| self.repository(id))
    }

    pub fn selected_state(&self) -> Option<&RepositoryState> {
        self.selected.and_then(|id| self.repo_states.get(&id))
    }

    pub fn repo_state_mut(&mut self, id: u64) -> &mut RepositoryState {
        self.repo_states.entry(id).or_default()
    }

    /// Repositories ordered for the foldout: alphabetical by display name.
    pub fn sorted_repositories(&self) -> Vec<&Repository> {
        let mut v: Vec<&Repository> = self.repositories.iter().collect();
        v.sort_by_key(|r| r.name().to_lowercase());
        v
    }
}
