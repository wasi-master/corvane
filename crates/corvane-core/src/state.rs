//! `AppState`: everything the UI renders from. Lives in one GPUI entity that
//! views observe; only the `Dispatcher` mutates it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use corvane_git::GitBinary;
use corvane_store::Store;
use gpui_kit::{App, Entity, Global};

use crate::persistence::Settings;
use corvane_models::{
    Account, AheadBehind, Diff, Repository, RepositoryInfo, Section, WorkingDirectoryStatus,
};

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
    AddExistingRepository { path: Option<PathBuf> },
    CreateRepository { path: Option<PathBuf> },
    CloneRepository { url: Option<String> },
    SignIn { enterprise: bool },
}

/// Where a sign-in is (GHD `SignInState`), driven by `Dispatcher::sign_in_*`.
#[derive(Clone, Debug, PartialEq)]
pub enum SignInStep {
    /// Asking GitHub for a device code.
    Requesting,
    /// Show the code; poll until the user authorises in the browser.
    DeviceCode {
        user_code: String,
        verification_uri: String,
    },
    /// Token in hand; fetching the account.
    Verifying,
    Error(String),
}

#[derive(Clone, Debug)]
pub struct SignInState {
    /// API base, e.g. `https://api.github.com`.
    pub endpoint: String,
    pub step: SignInStep,
    pub cancel: Arc<std::sync::atomic::AtomicBool>,
}

impl PartialEq for SignInState {
    fn eq(&self, other: &Self) -> bool {
        self.endpoint == other.endpoint && self.step == other.step
    }
}

/// An in-flight `git clone` shown in the content area (`CloningRepository`).
#[derive(Clone, Debug, PartialEq)]
pub struct CloneState {
    pub url: String,
    pub path: PathBuf,
    pub description: String,
    /// 0..1, `None` = indeterminate.
    pub value: Option<f32>,
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
    /// `git status` result (`IChangesState.workingDirectory`).
    pub status: Option<WorkingDirectoryStatus>,
    /// Path of the file whose diff is shown (`selectedFileIDs[0]`).
    pub selected_file: Option<String>,
    pub diff: Option<Diff>,
    pub diff_loading: bool,
}

impl RepositoryState {
    pub fn changed_files(&self) -> usize {
        self.status.as_ref().map(|s| s.files.len()).unwrap_or(0)
    }
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
    pub cloning: Option<CloneState>,
    pub sign_in: Option<SignInState>,
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

    pub fn account_for(&self, endpoint: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.endpoint == endpoint)
    }

    pub fn dotcom_account(&self) -> Option<&Account> {
        self.accounts.iter().find(|a| a.is_dotcom())
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
