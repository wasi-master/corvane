//! Git engine. Reads via `gix`, writes and network via the
//! `git` CLI so behaviour matches GitHub Desktop exactly.

pub mod branch_ops;
pub mod commit;
pub mod commit_template;
pub mod config;
pub mod detect;
pub mod diff;
pub mod error;
pub mod history_ops;
pub mod hook_env;
pub mod ignore;
pub mod log;
pub mod ops;
pub mod patch;
pub mod paths;
pub mod process;
pub mod rebase_ops;
pub mod remote_ops;
pub mod repo;
pub mod status;
pub mod worktree;

pub use branch_ops::{
    DESKTOP_STASH_MARKER, MergeOutcome, abort_merge, checkout_branch, checkout_new_branch,
    commits_ahead, configured_default_branch, create_branch, create_desktop_stash,
    delete_local_branch, delete_remote_branch, desktop_stash_message, drop_stash,
    find_default_branch, get_stashes, is_local_changes_overwritten, merge_branch,
    parse_recent_branches, pop_stash, recent_branches, remote_head, rename_branch, stashed_files,
};
pub use commit::{
    CommitOptions, add_paths, commit, discard_changes, format_message, head_sha, merge_trailers,
    stage_files, undo_last_commit, unstage_all,
};
pub use config::{
    add_safe_directory, global_config_value, local_config_value, remove_local_config_value,
    set_default_branch, set_global_config_value, set_local_config_value,
};
pub use detect::{GitBinary, GitVersion, find_git};
pub use diff::{
    blob_bytes, blob_lines, file_lines, has_hidden_bidi_chars, image_diff,
    parse_line_endings_warning, parse_raw_diff, parse_raw_diff_with_warnings, parse_unified,
    submodule_diff, working_directory_diff, working_file_lines,
};
pub use error::{GitError, dubious_ownership_path};
pub use history_ops::{
    ResetMode, checkout_commit, create_tag, delete_tag, reset_to, revert_commit,
};
pub use ignore::{
    append_ignore_files, append_ignore_rules, escape_gitignore_pattern, read_gitignore,
    save_gitignore,
};
pub use log::{
    COMMIT_BATCH_SIZE, NULL_TREE_SHA, commit_file_diff, commit_range_file_diff, get_changed_files,
    get_commit_range_changed_files, get_commits, get_commits_in_range, merge_base,
    merge_base_changed_files, merge_base_file_diff, most_recent_local_commit,
    parse_raw_log_with_numstat,
};
pub use ops::{
    CloneProgress, InitOptions, PathStatus, clone, global_identity, init_repository,
    normalize_clone_url, parse_clone_progress, path_status, readme_exists,
    repository_name_from_url, set_global_identity,
};
pub use patch::{
    apply_patch_to_index, discard_changes_from_selection, format_patch,
    format_patch_to_discard_changes, stage_partial_files,
};
pub use paths::git_dir;
pub use process::{GitCommand, GitOutput, set_credential_helper};
pub use rebase_ops::{
    CherryPickResult, CherryPickSnapshot, RebaseResult, RebaseSnapshot, abort_cherry_pick,
    abort_rebase, abort_squash_merge, binary_paths, cherry_pick, cherry_pick_head_found,
    cherry_pick_snapshot, commits_between, commits_in_range, conflict_marker_counts,
    continue_cherry_pick, continue_rebase, create_merge_commit, determine_mergeability,
    merge_commits_exist_after, merge_head_set, rebase, rebase_head_set, rebase_internal_state,
    rebase_snapshot, reorder, squash, squash_msg_set, stage_manual_conflict_resolution,
};
pub use remote_ops::{
    AskpassEnv, ProgressParser, RemoteFailure, add_remote, classify_remote_failure, config_value,
    fast_forward_branches, fetch, fetch_refspec, find_default_remote, get_remotes,
    install_lfs_hooks, is_using_lfs, last_fetched, lfs_available, lfs_hooks_installed,
    parse_progress_line, pull, pull_with_rebase, push, remote_failure, remove_remote,
    set_remote_url, update_remote_head,
};
pub use repo::{
    ahead_behind, main_worktree_path, open_repository, symmetric_ahead_behind,
    top_level_working_directory,
};
pub use status::{
    LineStats, get_status, map_status, parse_porcelain_v2, working_directory_line_stats,
};
pub use worktree::{
    add_worktree, list_worktrees, move_worktree, parse_worktree_porcelain, remove_worktree,
};
