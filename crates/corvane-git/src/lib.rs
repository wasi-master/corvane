//! Git engine. Reads via `gix`, writes and network via the
//! `git` CLI so behaviour matches GitHub Desktop exactly.

pub mod commit;
pub mod detect;
pub mod diff;
pub mod error;
pub mod ignore;
pub mod ops;
pub mod process;
pub mod repo;
pub mod status;

pub use commit::{
    CommitOptions, commit, discard_changes, format_message, head_sha, stage_files,
    undo_last_commit, unstage_all,
};
pub use detect::{GitBinary, GitVersion, find_git};
pub use diff::{parse_raw_diff, parse_unified, working_directory_diff};
pub use error::GitError;
pub use ignore::{append_ignore_files, append_ignore_rules, escape_gitignore_pattern};
pub use ops::{
    CloneProgress, InitOptions, PathStatus, clone, global_identity, init_repository,
    normalize_clone_url, parse_clone_progress, path_status, repository_name_from_url,
    set_global_identity,
};
pub use process::{GitCommand, GitOutput};
pub use repo::{ahead_behind, open_repository};
pub use status::{get_status, map_status, parse_porcelain_v2};
