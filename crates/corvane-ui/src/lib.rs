//! Corvane UI: a GPUI recreation of GitHub Desktop's chrome.
//!
//! Geometry and colours come from `.docs/ghd-ui-inventory.md` and
//! `.docs/ghd-theme-tokens.md`. Views never touch git, network or disk;
//! they dispatch actions that `corvane-core` handles.

pub mod actions;
pub mod autocompletion;
pub mod banner;
pub mod branch_list;
pub mod changes;
pub mod ci_check_popover;
pub mod ci_status;
pub mod cloning_view;
pub mod context_menu;
pub mod dialog;
pub mod dialogs;
pub mod diff_expansion;
pub mod diff_view;
pub mod diff_view_rows;
pub mod foldout;
pub mod format;
pub mod history;
pub mod icons;
pub mod image_diff;
pub mod keymap;
#[cfg(target_os = "macos")]
pub mod native_menu;
pub mod no_changes;
pub mod no_repositories;
pub mod pull_request_list;
pub mod relative_time;
pub mod repository_list;
pub mod selected_commit;
pub mod stash_view;
pub mod tab_bar;
pub mod theme;
pub mod title_bar;
pub mod toolbar;
pub mod welcome;
pub mod widgets;
pub mod workspace;
pub mod worktree_list;

use gpui_kit::App;

/// Install the theme global and keymap. Call once after `gpui_kit::init`.
pub fn init(cx: &mut App, appearance: theme::Appearance) {
    theme::init(cx, appearance);
    keymap::install(cx);
}
