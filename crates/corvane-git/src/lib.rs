//! Git engine. Reads via `gix`, writes and network via the
//! `git` CLI so behaviour matches GitHub Desktop exactly.

pub mod detect;
pub mod error;
pub mod ops;
pub mod process;
pub mod repo;

pub use detect::{GitBinary, GitVersion, find_git};
pub use error::GitError;
pub use ops::{
    CloneProgress, InitOptions, PathStatus, clone, init_repository, normalize_clone_url,
    parse_clone_progress, path_status, repository_name_from_url,
};
pub use process::{GitCommand, GitOutput};
pub use repo::{ahead_behind, open_repository};
