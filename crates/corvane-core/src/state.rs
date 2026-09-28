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
    InstallGit {
        reason: String,
    },
    Error {
        title: String,
        message: String,
    },
    AddExistingRepository {
        path: Option<PathBuf>,
    },
    CreateRepository {
        path: Option<PathBuf>,
    },
    CloneRepository {
        url: Option<String>,
    },
    SignIn {
        enterprise: bool,
    },
    /// `all` selects the "Discard All Changes" wording.
    DiscardChanges {
        repo: u64,
        paths: Vec<String>,
        all: bool,
    },
    /// `WarningBeforeReset`: dirty working directory before `reset --mixed`.
    ResetToCommit {
        repo: u64,
        sha: String,
    },
    /// `ConfirmCheckoutCommit`: detached HEAD warning.
    CheckoutCommit {
        repo: u64,
        sha: String,
    },
    /// `CreateTag`
    CreateTag {
        repo: u64,
        sha: String,
    },
    /// `WarnLocalChangesBeforeUndo`
    WarnLocalChangesBeforeUndo {
        repo: u64,
    },
    /// `CreateBranch`; `target_sha` when created from a commit in History.
    CreateBranch {
        repo: u64,
        target_sha: Option<String>,
        /// The branch filter text, prefilled as the name (`onCreateNewBranch`).
        initial_name: String,
    },
    RenameBranch {
        repo: u64,
        name: String,
    },
    DeleteBranch {
        repo: u64,
        name: String,
    },
    /// `StashAndSwitchBranch`: ask what to do with local changes.
    StashAndSwitchBranch {
        repo: u64,
        branch: String,
    },
    /// `ConfirmOverwriteStash`
    ConfirmOverwriteStash {
        repo: u64,
        branch: String,
    },
    /// `MultiCommitOperation` ChooseBranch step for merge (`squash` = Squash and Merge).
    MergeBranch {
        repo: u64,
        squash: bool,
    },
    /// `ConfirmDiscardStash`
    ConfirmDiscardStash {
        repo: u64,
    },
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
    /// Bumped whenever `diff` is replaced, so views can cache derived rows.
    pub diff_generation: u64,
    /// Most recent commit made from Corvane in this session (`UndoCommit` bar).
    pub last_commit: Option<LastCommit>,
    /// Incremented after every successful commit so the form can clear itself.
    pub commit_nonce: u64,
    pub committing: bool,
    /// A refresh was requested while one was running; run again when done.
    pub refresh_pending: bool,
    /// Filter Options popover state (`IFileListFilterState` minus the text).
    pub file_list_filter: FileListFilter,

    // ---- history (`ICompareState` / `ICommitSelection`) ----
    /// Commits of HEAD, newest first, loaded in `COMMIT_BATCH_SIZE` pages.
    pub commits: Vec<corvane_models::Commit>,
    pub commits_loading: bool,
    /// The last page was shorter than a batch: nothing more to load.
    pub commits_exhausted: bool,
    /// `commitSelection.shas[0]`
    pub selected_commit: Option<String>,
    /// Files + line counts of the selected commit (`changesetData`).
    pub changeset: Option<corvane_models::ChangesetData>,
    /// Path selected in the commit's file list.
    pub commit_selected_file: Option<String>,
    pub commit_diff: Option<Diff>,
    pub commit_diff_generation: u64,
    /// `isExpanded` of the expandable commit summary.
    pub commit_summary_expanded: bool,
    /// `commitToAmend`: the commit form rewrites HEAD instead of adding a commit.
    pub commit_to_amend: Option<corvane_models::Commit>,
    /// Bumped when amending starts so the form loads the commit's message.
    pub amend_nonce: u64,

    // ---- branches (`IBranchesState`) ----
    /// `recentBranches` (reflog checkouts, newest first).
    pub recent_branches: Vec<String>,
    /// `defaultBranch` name (`findDefaultBranch`).
    pub default_branch: Option<String>,
    /// Branch a checkout is switching to (`checkoutProgress.target`).
    pub checkout_target: Option<String>,
    /// Corvane/GHD stash entry for the current branch (`changesState.stashEntry`).
    pub stash: Option<corvane_models::StashEntry>,
    /// Total stash entries (`stashEntryCount`).
    pub stash_count: usize,
    /// Merge dialog preview: (branch, commits that would be merged).
    pub merge_preview: Option<(String, u32)>,

    // ---- stash viewer (`isShowingStashEntry`, `selectedStashedFile`) ----
    pub showing_stash: bool,
    pub stash_files: Option<Vec<corvane_models::CommittedFileChange>>,
    pub stash_selected_file: Option<String>,
    pub stash_diff: Option<Diff>,
    pub stash_diff_generation: u64,
}

/// GHD `IFileListFilterState` option flags; the text lives in the text box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileListFilter {
    pub included: bool,
    pub excluded: bool,
    pub new_files: bool,
    pub modified: bool,
    pub deleted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterOption {
    IncludedInCommit,
    ExcludedFromCommit,
    NewFiles,
    ModifiedFiles,
    DeletedFiles,
}

impl FileListFilter {
    pub fn get(&self, option: FilterOption) -> bool {
        match option {
            FilterOption::IncludedInCommit => self.included,
            FilterOption::ExcludedFromCommit => self.excluded,
            FilterOption::NewFiles => self.new_files,
            FilterOption::ModifiedFiles => self.modified,
            FilterOption::DeletedFiles => self.deleted,
        }
    }

    pub fn set(&mut self, option: FilterOption, on: bool) {
        match option {
            FilterOption::IncludedInCommit => self.included = on,
            FilterOption::ExcludedFromCommit => self.excluded = on,
            FilterOption::NewFiles => self.new_files = on,
            FilterOption::ModifiedFiles => self.modified = on,
            FilterOption::DeletedFiles => self.deleted = on,
        }
    }

    /// `countActiveFilterOptions`
    pub fn count_active(&self) -> usize {
        [
            self.included,
            self.excluded,
            self.new_files,
            self.modified,
            self.deleted,
        ]
        .iter()
        .filter(|b| **b)
        .count()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastCommit {
    pub sha: String,
    pub summary: String,
    pub at: std::time::SystemTime,
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
    /// Watcher for the selected repository's worktree.
    pub watcher: Option<crate::watcher::RepoWatcher>,
    pub watched_repo: Option<u64>,
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
