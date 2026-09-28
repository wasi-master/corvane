//! Git engine. Reads via `gix`, writes and network via the
//! `git` CLI so behaviour matches GitHub Desktop exactly.

pub mod branch_ops;
pub mod commit;
pub mod detect;
pub mod diff;
pub mod error;
pub mod history_ops;
pub mod ignore;
pub mod log;
pub mod ops;
pub mod patch;
pub mod process;
pub mod repo;
pub mod status;

pub use branch_ops::{
    DESKTOP_STASH_MARKER, MergeOutcome, abort_merge, checkout_branch, checkout_new_branch,
    commits_ahead, configured_default_branch, create_branch, create_desktop_stash,
    delete_local_branch, delete_remote_branch, desktop_stash_message, drop_stash,
    find_default_branch, get_stashes, is_local_changes_overwritten, merge_branch,
    parse_recent_branches, pop_stash, recent_branches, remote_head, rename_branch, stashed_files,
};
pub use commit::{
    CommitOptions, commit, discard_changes, format_message, head_sha, stage_files,
    undo_last_commit, unstage_all,
};
pub use detect::{GitBinary, GitVersion, find_git};
pub use diff::{parse_raw_diff, parse_unified, working_directory_diff};
pub use error::GitError;
pub use history_ops::{
    ResetMode, checkout_commit, create_tag, delete_tag, reset_to, revert_commit,
};
pub use ignore::{append_ignore_files, append_ignore_rules, escape_gitignore_pattern};
pub use log::{
    COMMIT_BATCH_SIZE, commit_file_diff, get_changed_files, get_commits, parse_raw_log_with_numstat,
};
pub use ops::{
    CloneProgress, InitOptions, PathStatus, clone, global_identity, init_repository,
    normalize_clone_url, parse_clone_progress, path_status, repository_name_from_url,
    set_global_identity,
};
pub use patch::{apply_patch_to_index, format_patch, stage_partial_files};
pub use process::{GitCommand, GitOutput};
pub use repo::{ahead_behind, open_repository};
pub use status::{get_status, map_status, parse_porcelain_v2};
