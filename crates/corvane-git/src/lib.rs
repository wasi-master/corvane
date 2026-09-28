//! Git engine (PLAN.md §3.3). Reads via `gix`, writes and network via the
//! `git` CLI so behaviour matches GitHub Desktop exactly.

pub mod detect;
pub mod error;
pub mod process;
pub mod repo;

pub use detect::{GitBinary, GitVersion, find_git};
pub use error::GitError;
pub use process::{GitCommand, GitOutput};
pub use repo::{ahead_behind, open_repository};
