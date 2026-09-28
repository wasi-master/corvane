//! Application state and the dispatcher that drives backends.
//! Mirrors GitHub Desktop's `AppStore` / `Dispatcher` / `RepositoryStateCache`.

pub mod dispatcher;
pub mod filter;
pub mod persistence;
pub mod state;
pub mod watcher;

pub use corvane_models::*;
pub use dispatcher::Dispatcher;
pub use persistence::{Settings, StoreExt};
pub use state::{
    AppState, CloneState, FileListFilter, FilterOption, Foldout, LastCommit, Popup,
    RepositoryState, SignInState, SignInStep,
};
