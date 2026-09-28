//! Application state and the dispatcher that drives backends.
//! Mirrors GitHub Desktop's `AppStore` / `Dispatcher` / `RepositoryStateCache`.

pub mod dispatcher;
pub mod persistence;
pub mod state;

pub use corvane_models::*;
pub use dispatcher::Dispatcher;
pub use persistence::{Settings, StoreExt};
pub use state::{AppState, CloneState, Foldout, Popup, RepositoryState, SignInState, SignInStep};
