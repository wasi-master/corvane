//! Application state and the dispatcher that drives backends.
//! Mirrors GitHub Desktop's `AppStore` / `Dispatcher` / `RepositoryStateCache`.

pub mod autocomplete;
pub mod avatars;
pub mod commit_status;
pub mod compare;
pub mod dispatcher;
pub mod emoji;
pub mod filter;
pub mod forks;
pub mod integrations;
pub mod list_selection;
pub mod mco;
pub mod persistence;
pub mod pull_request_preview;
pub mod pull_requests;
pub mod push_errors;
pub mod remote;
pub mod repo_rules;
pub mod state;
pub mod templates;
pub mod watcher;
pub mod worktrees;

pub use autocomplete::{
    DEFAULT_MAX_HITS, Issue, IssueCache, IssueHit, MentionableCache, MentionableUser, Trigger,
    TriggerKind, find_trigger, issues_matching, users_matching,
};
pub use avatars::{AvatarEntry, avatar_for_email, avatar_for_url};
pub use commit_status::{CommitStatusStore, combined_status_summary, group_check_runs, status_key};
pub use compare::{CompareForm, CompareState, ComparisonMode};
pub use corvane_models::*;
pub use dispatcher::Dispatcher;
pub use emoji::CustomEmoji;
pub use forks::UPSTREAM_REMOTE_NAME;
pub use integrations::{PreferencesSave, RepositorySettingsSave};
pub use mco::{
    Banner, ConflictKind, ConflictState, McoConflicts, McoDetail, McoStep, McoUndo, MergePreview,
    MultiCommitOperation, RebasePreview, conflicted_files, resolved_files, unmerged_files,
};
pub use persistence::{
    CustomIntegration, DEFAULT_DATE_FORMAT, DEFAULT_NUMBER_FORMAT, DEFAULT_TIME_FORMAT, Settings,
    StoreExt, TAB_SIZE_DEFAULT, UncommittedChangesStrategy,
};
pub use pull_request_preview::{MergeStatus, PullRequestPreview};
pub use pull_requests::{
    BranchesTab, FORKED_REMOTE_PREFIX, PullRequestCache, PullRequestCaches, cache_key,
    find_associated_pull_request, fork_pull_request_remote_name,
};
pub use remote::{ForcePushState, PushPullKind, PushPullProgress, RepoIndicator, host_of};
pub use repo_rules::{failed_rules, rule_matches};
pub use state::{
    AppState, CloneState, DropTarget, FileListFilter, FilterOption, Foldout, GitConfigLocation,
    GlobalGitConfig, LastCommit, Popup, PreferencesTab, RepositorySettingsData,
    RepositorySettingsTab, RepositoryState, RetryAction, SignInState, SignInStep,
    UnreachableCommitsTab,
};
