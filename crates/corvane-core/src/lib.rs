//! Application state and the dispatcher that drives backends.
//! Mirrors GitHub Desktop's `AppStore` / `Dispatcher` / `RepositoryStateCache`.

pub mod autocomplete;
pub mod avatars;
pub mod compare;
pub mod dispatcher;
pub mod emoji;
pub mod filter;
pub mod integrations;
pub mod mco;
pub mod persistence;
pub mod remote;
pub mod state;
pub mod templates;
pub mod watcher;
pub mod worktrees;

pub use autocomplete::{
    DEFAULT_MAX_HITS, Issue, IssueCache, IssueHit, MentionableCache, MentionableUser, Trigger,
    TriggerKind, find_trigger, issues_matching, users_matching,
};
pub use avatars::{AvatarEntry, avatar_for_email, avatar_for_url};
pub use compare::{CompareForm, CompareState, ComparisonMode};
pub use corvane_models::*;
pub use dispatcher::Dispatcher;
pub use emoji::CustomEmoji;
pub use integrations::{PreferencesSave, RepositorySettingsSave};
pub use mco::{
    Banner, ConflictKind, ConflictState, McoConflicts, McoDetail, McoStep, McoUndo, MergePreview,
    MultiCommitOperation, RebasePreview, conflicted_files, resolved_files, unmerged_files,
};
pub use persistence::{
    CustomIntegration, DEFAULT_DATE_FORMAT, DEFAULT_NUMBER_FORMAT, DEFAULT_TIME_FORMAT, Settings,
    StoreExt, TAB_SIZE_DEFAULT, UncommittedChangesStrategy,
};
pub use remote::{ForcePushState, PushPullKind, PushPullProgress, RepoIndicator, host_of};
pub use state::{
    AppState, CloneState, DropTarget, FileListFilter, FilterOption, Foldout, GitConfigLocation,
    GlobalGitConfig, LastCommit, Popup, PreferencesTab, RepositorySettingsData,
    RepositorySettingsTab, RepositoryState, RetryAction, SignInState, SignInStep,
    UnreachableCommitsTab,
};
