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
    Account, AheadBehind, Diff, GitHubRepository, Identity, Remote, Repository, RepositoryInfo,
    Section, WorkingDirectoryStatus,
};
use corvane_platform::editors::FoundEditor;
use corvane_platform::shells::FoundShell;

/// Which toolbar foldout is open (`FoldoutType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foldout {
    Repository,
    Branch,
    PushPull,
    Worktree,
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
    /// `InvalidatedToken`: an API call answered 401; the account was
    /// signed out and can sign in again.
    InvalidatedToken {
        account: Account,
    },
    /// `StartPullRequest`: the Preview Pull Request dialog (state in
    /// `RepositoryState::pull_request_preview`).
    StartPullRequest {
        repo: u64,
    },
    /// `CreateFork`: "Do you want to fork this repository?"
    CreateFork {
        repo: u64,
    },
    /// `CLIInstalled`: the command line tool was linked at `path`.
    CLIInstalled {
        path: PathBuf,
    },
    /// `MoveToApplicationsFolder`: offered at launch outside /Applications.
    MoveToApplicationsFolder,
    /// `Acknowledgements`: License and Open Source Notices.
    Acknowledgements,
    /// Corvane addition: crash reports left by the previous session (newest
    /// first), with "Save crash reports locally" on.
    CrashReportFound {
        reports: Vec<PathBuf>,
    },
    /// `ReleaseNotes`: what's new in the running version.
    ReleaseNotes {
        summary: crate::release_notes::ReleaseSummary,
    },
    /// `UpstreamAlreadyExists`: the fork's `upstream` remote points elsewhere.
    UpstreamAlreadyExists {
        repo: u64,
        existing_url: String,
    },
    /// `ChooseForkSettings`: "How are you planning to use this fork?"
    ChooseForkSettings {
        repo: u64,
    },
    /// `PushProtectionError`: secrets the server refused; `bypassed` lists
    /// the placeholder ids already allowed through.
    PushProtectionError {
        repo: u64,
        secrets: Vec<corvane_models::SecretScanResult>,
        bypassed: Vec<String>,
    },
    /// `BypassPushProtection`: why a secret gets pushed anyway.
    BypassPushProtection {
        repo: u64,
        secret: corvane_models::SecretScanResult,
        secrets: Vec<corvane_models::SecretScanResult>,
        bypassed: Vec<String>,
    },
    /// `PushRejectedDueToMissingWorkflowScope`
    PushRejectedDueToMissingWorkflowScope {
        repo: u64,
        rejected_path: String,
    },
    /// `SAMLReauthRequired`
    SAMLReauthRequired {
        repo: u64,
        organization: String,
        endpoint: String,
        retry: Option<RetryAction>,
    },
    /// `CreateTutorialRepository`: "Start tutorial" for `account`, with the
    /// creation progress (title, percent, detail) once it runs.
    CreateTutorialRepository {
        account: Account,
        progress: Option<(String, u8, Option<String>)>,
    },
    /// `ConfirmExitTutorial`
    ConfirmExitTutorial,
    /// `TestNotifications`: post sample pull request notifications for
    /// `repo` (debug builds).
    TestNotifications {
        repo: u64,
    },
    /// `CICheckRunRerun`: re-run (failed) checks of the PR head ref.
    CICheckRunRerun {
        repo: u64,
        github: GitHubRepository,
        checks: Vec<corvane_models::RefCheck>,
        git_ref: String,
        failed_only: bool,
    },
    /// `PullRequestReview`: a review on one of the user's pull requests
    /// (GHD shows it from a notification). `should_*` pick the OK button:
    /// switch repository and/or check out the PR branch.
    PullRequestReview {
        repo: u64,
        pull_request: corvane_models::PullRequest,
        review: corvane_github::api::ApiPullRequestReview,
        should_checkout_branch: bool,
        should_change_repository: bool,
    },
    /// `PullRequestComment`: a comment on one of the user's pull requests.
    PullRequestComment {
        repo: u64,
        pull_request: corvane_models::PullRequest,
        comment: corvane_github::api::ApiIssueComment,
        should_checkout_branch: bool,
        should_change_repository: bool,
    },
    /// `PullRequestChecksFailed`: checks failed on one of the user's pull
    /// requests.
    PullRequestChecksFailed {
        repo: u64,
        pull_request: corvane_models::PullRequest,
        checks: Vec<corvane_models::RefCheck>,
        should_change_repository: bool,
    },
    /// `UnknownAuthors`: co-author handles that could not be resolved;
    /// "Commit Anyway" commits with the known ones only.
    UnknownAuthors {
        repo: u64,
        usernames: Vec<String>,
        summary: String,
        description: String,
    },
    /// `ConfirmDiscardSelection`: lines picked from the diff gutter menu.
    ConfirmDiscardSelection {
        repo: u64,
        path: String,
        selection: corvane_models::DiffSelection,
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
    /// Worktrees (GHD 3.6 `enableWorktreeSupport`).
    AddWorktree {
        repo: u64,
        /// `initialBranchName` / `initialWorktreeName` (checkout in a new
        /// worktree from the pull request list).
        initial_branch_name: Option<String>,
        initial_worktree_name: Option<String>,
    },
    RenameWorktree {
        repo: u64,
        path: PathBuf,
    },
    DeleteWorktree {
        repo: u64,
        path: PathBuf,
    },
    /// `git worktree remove` failed: offer `--force`; dismissing switches
    /// back to `original` when the current worktree was the one deleted.
    DeleteWorktreeFailed {
        repo: u64,
        path: PathBuf,
        error: String,
        original: Option<PathBuf>,
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
    /// `MultiCommitOperation`: the dialog for the current `RepositoryState::mco` step.
    /// `flow` changes per operation so the dialog view is rebuilt (no stale
    /// branch selection from an earlier flow).
    MultiCommitOperation {
        repo: u64,
        flow: u64,
    },
    /// `LocalChangesOverwritten`: the operation needs a clean working directory.
    LocalChangesOverwritten {
        repo: u64,
        retry: RetryAction,
        files: Vec<String>,
    },
    /// `PushBranchCommits`: the branch must be published (`unpushed: None`)
    /// or has local commits to push before its pull request is created.
    PushBranchCommits {
        repo: u64,
        branch: String,
        unpushed: Option<u32>,
        base: Option<String>,
    },
    /// `PublishRepository`
    PublishRepository {
        repo: u64,
    },
    /// `PushNeedsPull`
    PushNeedsPull {
        repo: u64,
    },
    /// `ConfirmForcePush`
    ConfirmForcePush {
        repo: u64,
        upstream_branch: String,
    },
    /// `GenericGitAuthentication`
    GenericGitAuthentication {
        repo: u64,
        remote_url: String,
        host: String,
        username: Option<String>,
        retry: RetryAction,
    },
    /// `InitializeLFS`
    InitializeLFS {
        repos: Vec<u64>,
    },
    /// `CommitMessage` popup used for the squashed commit's message.
    SquashCommitMessage {
        repo: u64,
        to_squash: Vec<String>,
        onto: String,
        summary: String,
        description: String,
        count: usize,
    },
    /// `Preferences` (Settings…), opened on a tab.
    Preferences {
        tab: PreferencesTab,
    },
    /// `RepositorySettings`
    RepositorySettings {
        repo: u64,
        tab: RepositorySettingsTab,
    },
    /// `RemoveRepository` confirmation.
    ConfirmRemoveRepository {
        repo: u64,
    },
    /// `About`
    About {
        version: String,
    },
    /// `ExternalEditorError`
    ExternalEditorError {
        message: String,
        suggest_default_editor: bool,
        open_preferences: bool,
    },
    /// `OpenShellFailed`
    ShellError {
        message: String,
    },
    /// `UnreachableCommits`: which selected commits the range diff covers.
    UnreachableCommits {
        repo: u64,
        tab: UnreachableCommitsTab,
    },
}

/// GHD `UnreachableCommitsTab`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnreachableCommitsTab {
    #[default]
    Unreachable,
    Reachable,
}

/// GHD `DropTarget`: what the dragged commits currently hover, for the
/// drag element's tooltip ("Copy to <branch>", "Squash N commits", …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropTarget {
    Branch(String),
    Commit,
    /// Reorder insertion line; `count` = commits being dragged.
    InsertionPoint {
        count: usize,
    },
}

/// GHD `PreferencesTab` (Copilot omitted).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PreferencesTab {
    #[default]
    Accounts,
    Integrations,
    Git,
    Appearance,
    Notifications,
    Prompts,
    Advanced,
    Accessibility,
}

/// GHD `RepositorySettingsTab` (Fork Behavior omitted).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RepositorySettingsTab {
    #[default]
    Remote,
    IgnoredFiles,
    GitConfig,
    /// "Fork Behavior" (forks with a known parent only).
    ForkSettings,
}

/// GHD `GitConfigLocation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GitConfigLocation {
    #[default]
    Global,
    Local,
}

/// What the Settings › Git tab edits: read from the global git config when
/// the dialog opens (`isLoadingGitConfig` until then).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GlobalGitConfig {
    pub name: Option<String>,
    pub email: Option<String>,
    pub default_branch: String,
}

/// Everything the Repository Settings dialog needs, loaded when it opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositorySettingsData {
    pub repo: u64,
    /// `defaultRemote` (origin, else the first remote).
    pub remote: Option<Remote>,
    /// Root `.gitignore` text, `None` when the file does not exist.
    pub gitignore: Option<String>,
    /// `--local` `user.name` / `user.email`.
    pub local_name: Option<String>,
    pub local_email: Option<String>,
    pub global: Identity,
    /// `core.autocrlf` (line endings written to `.gitignore`).
    pub autocrlf: bool,
}

/// GHD `RetryAction` (the subset behind `LocalChangesOverwritten`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetryAction {
    CherryPick {
        target: String,
    },
    CherryPickNewBranch {
        name: String,
        start_point: Option<String>,
    },
    Squash {
        to_squash: Vec<String>,
        onto: String,
        message: String,
    },
    Reorder {
        to_move: Vec<String>,
        before: Option<String>,
    },
    Push {
        force_with_lease: bool,
        branch: Option<String>,
    },
    Pull,
    Fetch,
}

impl RetryAction {
    /// `getRetryActionName`
    pub fn name(&self) -> &'static str {
        match self {
            RetryAction::CherryPick { .. } | RetryAction::CherryPickNewBranch { .. } => {
                "cherry-pick"
            }
            RetryAction::Squash { .. } => "squash",
            RetryAction::Reorder { .. } => "reorder",
            RetryAction::Push { .. } => "push",
            RetryAction::Pull => "pull",
            RetryAction::Fetch => "fetch",
        }
    }
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
    /// GHD `RepositoryType` `unsafe`: the directory git named as having
    /// dubious ownership ("Trust Repository" view).
    pub unsafe_path: Option<PathBuf>,
    /// `isTrustingPath`: `safe.directory` is being added.
    pub trusting_path: bool,
    pub last_refresh: Option<Instant>,
    pub section: Section,
    /// `git status` result (`IChangesState.workingDirectory`).
    pub status: Option<WorkingDirectoryStatus>,
    /// Path of the file whose diff is shown (`selectedFileIDs[0]`).
    pub selected_file: Option<String>,
    /// Every selected path (`selectedFileIDs`), click order; ⌘/⇧-click extend it.
    pub selected_files: Vec<String>,
    pub diff: Option<Diff>,
    pub diff_loading: bool,
    /// Bumped whenever `diff` is replaced, so views can cache derived rows.
    pub diff_generation: u64,
    /// The new side of the selected file as lines, for hunk expansion
    /// (GHD `fileContents.newContents`); `None` when it cannot be expanded.
    pub diff_contents: Option<Arc<Vec<String>>>,
    /// Most recent commit made from Corvane in this session (`UndoCommit` bar).
    pub last_commit: Option<LastCommit>,
    /// Incremented after every successful commit so the form can clear itself.
    pub commit_nonce: u64,
    /// GHD `showCoAuthoredBy` / `coAuthors` (per repository, this session).
    pub show_co_authored_by: bool,
    pub co_authors: Vec<corvane_models::Author>,
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
    /// `commitSelection.shas[0]`: the anchor of the selection.
    pub selected_commit: Option<String>,
    /// `commitSelection.shas` in click order (⌘/⇧-click multi-select).
    pub selected_commits: Vec<String>,
    /// `commitSelection.isContiguous`
    pub commits_contiguous: bool,
    /// `commitSelection.shasInDiff`: the selected commits reachable from the newest one.
    pub shas_in_diff: Vec<String>,
    /// Files + line counts of the selected commit (`changesetData`).
    pub changeset: Option<corvane_models::ChangesetData>,
    /// Path selected in the commit's file list.
    pub commit_selected_file: Option<String>,
    pub commit_diff: Option<Diff>,
    pub commit_diff_generation: u64,
    pub commit_diff_contents: Option<Arc<Vec<String>>>,
    /// `isExpanded` of the expandable commit summary.
    pub commit_summary_expanded: bool,
    /// `commitToAmend`: the commit form rewrites HEAD instead of adding a commit.
    pub commit_to_amend: Option<corvane_models::Commit>,
    /// Bumped when amending starts so the form loads the commit's message.
    pub amend_nonce: u64,

    // ---- branches (`IBranchesState`) ----
    /// `recentBranches` (reflog checkouts, newest first).
    pub recent_branches: Vec<String>,
    /// `git worktree list` (main first); the toolbar button shows once
    /// there is more than one.
    pub worktrees: Vec<corvane_models::WorktreeEntry>,
    /// `defaultBranch` name (`findDefaultBranch`).
    pub default_branch: Option<String>,
    /// Branch a checkout is switching to (`checkoutProgress.target`).
    pub checkout_target: Option<String>,
    /// Corvane/GHD stash entry for the current branch (`changesState.stashEntry`).
    pub stash: Option<corvane_models::StashEntry>,
    /// Total stash entries (`stashEntryCount`).
    pub stash_count: usize,
    /// Merge dialog preview.
    pub merge_preview: Option<crate::mco::MergePreview>,
    /// `pullRequestState`: the Preview Pull Request dialog's data.
    pub pull_request_preview: Option<crate::pull_request_preview::PullRequestPreview>,
    /// `addUpstreamRemoteIfNeeded` ran for this repository this session.
    pub upstream_checked: bool,
    /// `changesState.currentBranchProtected`
    pub current_branch_protected: bool,
    /// `changesState.currentRepoRulesInfo`
    pub repo_rules: corvane_models::RepoRulesInfo,
    /// Which branch the rules were fetched for, and when.
    pub repo_rules_branch: Option<String>,
    pub repo_rules_fetched_at: Option<Instant>,
    /// Rebase dialog preview.
    pub rebase_preview: Option<crate::mco::RebasePreview>,

    /// `compareState`
    pub compare: crate::compare::CompareState,

    // ---- multi-commit operations ----
    pub mco: Option<crate::mco::MultiCommitOperation>,
    /// Bumped by every new multi-commit operation (see `Popup::MultiCommitOperation`).
    pub mco_flow: u64,
    /// `shasToHighlight`: rows kept opaque while hovering the multi-commit
    /// summary's counts; everything else dims.
    pub highlighted_shas: Vec<String>,
    pub mco_undo: Option<crate::mco::McoUndo>,
    /// `changesState.conflictState`
    pub conflict_state: Option<crate::mco::ConflictState>,
    /// `forcePushBranches`: branch → tip after a rewrite that needs a force push.
    pub force_push_branches: HashMap<String, String>,

    // ---- remote (`isPushPullFetchInProgress`, `pushPullFetchProgress`, `lastFetched`) ----
    pub push_pull_in_progress: bool,
    pub push_pull_progress: Option<crate::remote::PushPullProgress>,
    pub last_fetched: Option<std::time::SystemTime>,
    pub pull_with_rebase: bool,
    pub publishing: bool,
    /// The LFS initialisation prompt was already considered for this repository.
    pub lfs_checked: bool,

    // ---- stash viewer (`isShowingStashEntry`, `selectedStashedFile`) ----
    pub showing_stash: bool,
    pub stash_files: Option<Vec<corvane_models::CommittedFileChange>>,
    pub stash_selected_file: Option<String>,
    pub stash_diff: Option<Diff>,
    pub stash_diff_generation: u64,
    pub stash_diff_contents: Option<Arc<Vec<String>>>,
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
    /// The commit list the History tab shows: the comparison while comparing
    /// to a branch, else the branch's own history.
    pub fn visible_commits(&self) -> &Vec<corvane_models::Commit> {
        if self.compare.is_comparing() {
            &self.compare.commits
        } else {
            &self.commits
        }
    }

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
    /// What to retry once the sign-in dialog opened by a re-authorization
    /// prompt succeeds (`beginBrowserBasedSignIn` → `performRetry`).
    pub retry_after_sign_in: Option<(u64, RetryAction)>,
    /// Watcher for the selected repository's worktree.
    pub watcher: Option<crate::watcher::RepoWatcher>,
    pub watched_repo: Option<u64>,
    /// `currentBanner`
    pub banner: Option<crate::mco::Banner>,
    pub banner_nonce: u64,
    /// Sidebar indicators per repository (`localRepositoryStateLookup`).
    pub indicators: HashMap<u64, crate::remote::RepoIndicator>,
    /// Generic git server logins (host → username) for the askpass helper.
    pub generic_logins: HashMap<String, String>,
    /// Avatar cache (`crate::avatars`).
    pub avatars: crate::avatars::Avatars,
    /// `dragAndDropManager` drop target during a commit drag.
    pub drag_target: Option<DropTarget>,
    /// Clone dialog: `GET /user/repos` per account endpoint (`ApiRepositoriesStore`).
    pub api_repositories: HashMap<String, Vec<corvane_models::GitHubRepository>>,
    pub api_repositories_loading: std::collections::HashSet<String>,
    /// `#issue` / `@user` autocompletion caches (`IssuesStore`, `GitHubUserStore`).
    pub issues: crate::autocomplete::IssueCaches,
    pub mentionables: crate::autocomplete::MentionableCaches,
    /// Open pull requests per GitHub repository (`PullRequestCoordinator`).
    pub pull_requests: crate::pull_requests::PullRequestCaches,
    /// `selectedBranchesTab`
    pub branches_tab: crate::pull_requests::BranchesTab,
    /// `OnboardingTutorialAssessor.tutorialAnnounced` (per session).
    pub tutorial_announced: bool,
    /// `CORVANE_POPUP=tutorial:<step>`: the step the tutorial repository is
    /// shown at, whatever its state (dev/testing convenience).
    pub tutorial_step_override: Option<crate::tutorial::TutorialStep>,
    /// `showCIStatusPopover`: the check-run popover under the PR badge.
    pub show_ci_status_popover: bool,
    /// `CommitStatusStore`: CI statuses of refs.
    pub commit_statuses: crate::commit_status::CommitStatusStore,
    /// `cachedRepoRulesets`: ruleset id → how it applies to the user.
    pub repo_rulesets: HashMap<u64, corvane_models::RepoRuleEnforced>,
    /// Installed editors / shells (`getAvailableEditors` / `getAvailableShells`).
    pub editors: Vec<FoundEditor>,
    pub shells: Vec<FoundShell>,
    /// Loaded when the Settings dialog opens (`None` while loading).
    pub global_git: Option<GlobalGitConfig>,
    /// Loaded when the Repository Settings dialog opens.
    pub repo_settings: Option<RepositorySettingsData>,
    /// `resolveOpenInDesktop`: an `x-corvane://openRepo` action waiting for
    /// the clone it opened.
    pub pending_open_in_desktop: Option<crate::app_url::PendingOpenInDesktop>,
    /// `UpdateStore` state + `isUpdateAvailableBannerVisible`.
    pub update: crate::updater::UpdateState,
    /// On-demand packs.
    pub packs: crate::packs::PacksState,
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

    /// `None` before `Dispatcher::init` (widgets rendered in isolation).
    pub fn try_global(cx: &App) -> Option<Entity<AppState>> {
        cx.try_global::<AppStateHandle>().map(|h| h.0.clone())
    }

    /// The editor "Open in …" menu items name: the selected editor, else the
    /// first installed one, else GHD's generic "External Editor".
    pub fn editor_label(&self) -> String {
        if self.settings.use_custom_editor && self.settings.custom_editor.is_some() {
            return "Custom Editor".to_string();
        }
        self.settings
            .external_editor
            .clone()
            .or_else(|| self.editors.first().map(|e| e.name.clone()))
            .unwrap_or_else(|| "External Editor".to_string())
    }

    /// The shell "Open in …" menu items name (`Terminal` by default).
    pub fn shell_label(&self) -> String {
        if self.settings.use_custom_shell && self.settings.custom_shell.is_some() {
            return "Custom Shell".to_string();
        }
        self.settings
            .shell
            .clone()
            .unwrap_or_else(|| corvane_platform::shells::DEFAULT_SHELL.label().to_string())
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
