//! Application state and the dispatcher that drives backends.
//! Mirrors GitHub Desktop's `AppStore` / `Dispatcher` / `RepositoryStateCache`.

pub mod compare;
pub mod dispatcher;
pub mod filter;
pub mod mco;
pub mod persistence;
pub mod state;
pub mod watcher;

pub use compare::{CompareForm, CompareState, ComparisonMode};
pub use corvane_models::*;
pub use dispatcher::Dispatcher;
pub use mco::{
    Banner, ConflictKind, ConflictState, McoConflicts, McoDetail, McoStep, McoUndo, MergePreview,
    MultiCommitOperation, RebasePreview, conflicted_files, resolved_files, unmerged_files,
};
pub use persistence::{Settings, StoreExt, UncommittedChangesStrategy};
pub use state::{
    AppState, CloneState, FileListFilter, FilterOption, Foldout, LastCommit, Popup,
    RepositoryState, RetryAction, SignInState, SignInStep,
};
