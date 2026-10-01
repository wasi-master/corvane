//! GitHub Desktop's keyboard shortcuts (`build-default-menu.ts`). Its
//! `CmdOrCtrl` is GPUI's `secondary` (⌘ on macOS, Ctrl elsewhere); the
//! macOS-only app and Window menu shortcuts are bound on macOS only.
//!
//! Corvane: some bindings depend on flags ([`KeymapFlags`]); [`sync`]
//! rebuilds the keymap when one changes, so the menu bar (which reads its
//! shortcuts from the keymap) must be rebuilt after it.

use corvane_core::flags::{Flags, ids};
use gpui_kit::{App, Global, KeyBinding};

use crate::actions::*;

/// The flags that add or remove key bindings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeymapFlags {
    /// `605-cmd-backspace-discards-files`: ⌘⌫ in the changes list discards
    /// the selected files instead of removing the repository.
    pub discard_selected_files: bool,
    /// `606-open-file-shortcuts`: ⇧⌘A / ⌥⌘O in the changes and commit file
    /// lists open the selected file in the editor / default program.
    pub open_file_shortcuts: bool,
    /// `607-no-push-shortcut`: ⌘P does not push.
    pub no_push_shortcut: bool,
    /// `608-open-in-shell-alt-shortcut`: ⌥⌘T also opens the shell.
    pub open_in_shell_alt_shortcut: bool,
    /// `609-emacs-list-keys`: ⌃N / ⌃P move through the changes and history lists.
    pub emacs_list_keys: bool,
    /// `610-diff-mode-shortcut`: ⌥⌘S switches between unified and split diffs.
    pub diff_mode_shortcut: bool,
    /// `611-copy-path-shortcuts`: ⌥⌘C / ⇧⌥⌘C copy the selected files' paths.
    pub copy_path_shortcuts: bool,
    /// `612-navigation-shortcuts`: ⌃⌘P pull requests, ⇧⌘] / ⇧⌘[ next /
    /// previous repository, ⌘3 the diff, ⌥↓ / ⌥↑ files from the diff.
    pub navigation_shortcuts: bool,
    /// `801-history-review-mode`: ⌃⌘S hides History's lists.
    pub history_review_mode: bool,
}

impl KeymapFlags {
    pub fn from_flags(flags: &Flags) -> Self {
        Self {
            discard_selected_files: flags.bool(ids::CMD_BACKSPACE_DISCARDS_FILES),
            open_file_shortcuts: flags.bool(ids::OPEN_FILE_SHORTCUTS),
            no_push_shortcut: flags.bool(ids::NO_PUSH_SHORTCUT),
            open_in_shell_alt_shortcut: flags.bool(ids::OPEN_IN_SHELL_ALT_SHORTCUT),
            emacs_list_keys: flags.bool(ids::EMACS_LIST_KEYS),
            diff_mode_shortcut: flags.bool(ids::DIFF_MODE_SHORTCUT),
            copy_path_shortcuts: flags.bool(ids::COPY_PATH_SHORTCUTS),
            navigation_shortcuts: flags.bool(ids::NAVIGATION_SHORTCUTS),
            history_review_mode: flags.bool(ids::HISTORY_REVIEW_MODE),
        }
    }
}

/// Electron's `togglefullscreen` role: ⌃⌘F on macOS, F11 elsewhere.
const FULL_SCREEN: &str = if cfg!(target_os = "macos") {
    "ctrl-cmd-f"
} else {
    "f11"
};

/// Corvane's ⌃⌘ chords (`612-navigation-shortcuts`, `801-history-review-mode`):
/// off macOS Ctrl is already the command key, so they become Ctrl+Alt.
const CTRL_CMD_P: &str = if cfg!(target_os = "macos") {
    "ctrl-cmd-p"
} else {
    "ctrl-alt-p"
};
const CTRL_CMD_S: &str = if cfg!(target_os = "macos") {
    "ctrl-cmd-s"
} else {
    "ctrl-alt-s"
};

/// Context of the shortcuts for menu items GitHub Desktop disables while a
/// popup is open (`menu-update.ts` `getMenuState`). `Workspace` adds
/// `Popup` to its key context while one is; the key then goes to the dialog
/// (a text field's ⌘⌫, ⌃H…) instead of acting on the window behind it. The
/// handlers check too, for the menu bar's own key equivalents and clicks.
const MENU: Option<&str> = Some("!Popup");

/// The bindings other crates (gpui-kit's components) installed before ours,
/// kept so [`sync`] can rebuild the whole keymap.
struct InstalledKeymap {
    base: Vec<KeyBinding>,
    flags: KeymapFlags,
}

impl Global for InstalledKeymap {}

pub fn install(cx: &mut App) {
    let base = cx.key_bindings().borrow().bindings().cloned().collect();
    let flags = KeymapFlags::default();
    cx.bind_keys(bindings(flags));
    cx.set_global(InstalledKeymap { base, flags });
}

/// Rebinds the keymap when `flags` differ from the installed ones; true when
/// it did (rebuild the menu bar so its shortcuts follow).
pub fn sync(flags: KeymapFlags, cx: &mut App) -> bool {
    let Some(installed) = cx.try_global::<InstalledKeymap>() else {
        return false;
    };
    if installed.flags == flags {
        return false;
    }
    let base = installed.base.clone();
    cx.clear_key_bindings();
    cx.bind_keys(base);
    cx.bind_keys(bindings(flags));
    cx.global_mut::<InstalledKeymap>().flags = flags;
    true
}

/// GHD `List.onKeyDown` `isHomeKey` / `isEndKey`: ⌘↑ / ⌘↓ go to the first
/// / last row on macOS; elsewhere only Home / End do, and Ctrl+↑ / Ctrl+↓
/// are plain arrows (the `ArrowUp` / `ArrowDown` branch ignores Ctrl).
fn list_end_binding(up: bool, context: &'static str) -> KeyBinding {
    let key = if up { "secondary-up" } else { "secondary-down" };
    match (cfg!(target_os = "macos"), up) {
        (true, true) => KeyBinding::new(key, SelectFirstFile, Some(context)),
        (true, false) => KeyBinding::new(key, SelectLastFile, Some(context)),
        (false, true) => KeyBinding::new(key, SelectPreviousFile, Some(context)),
        (false, false) => KeyBinding::new(key, SelectNextFile, Some(context)),
    }
}

fn bindings(flags: KeymapFlags) -> Vec<KeyBinding> {
    let mut bindings = vec![
        KeyBinding::new("down", SelectNextFile, Some("ChangesList")),
        KeyBinding::new("up", SelectPreviousFile, Some("ChangesList")),
        KeyBinding::new("secondary-a", SelectAllFiles, Some("ChangesList")),
        // the diff's text selection (GHD `select-all` / the browser's copy)
        KeyBinding::new("secondary-a", SelectAll, Some("Diff")),
        KeyBinding::new("secondary-c", Copy, Some("Diff")),
        KeyBinding::new("shift-down", ExtendSelectionDown, Some("ChangesList")),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("ChangesList")),
        list_end_binding(true, "ChangesList"),
        list_end_binding(false, "ChangesList"),
        KeyBinding::new("home", SelectFirstFile, Some("ChangesList")),
        KeyBinding::new("end", SelectLastFile, Some("ChangesList")),
        KeyBinding::new("space", ToggleIncludeSelected, Some("ChangesList")),
        KeyBinding::new("down", SelectNextFile, Some("HistoryList")),
        KeyBinding::new("up", SelectPreviousFile, Some("HistoryList")),
        KeyBinding::new("shift-down", ExtendSelectionDown, Some("HistoryList")),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("HistoryList")),
        list_end_binding(true, "HistoryList"),
        list_end_binding(false, "HistoryList"),
        KeyBinding::new("home", SelectFirstFile, Some("HistoryList")),
        KeyBinding::new("end", SelectLastFile, Some("HistoryList")),
        KeyBinding::new("enter", ReorderConfirm, Some("HistoryList")),
        // GHD `List.handleKeyDown` in the commit, stash and pull request
        // file lists (`FileList`)
        KeyBinding::new("down", SelectNextFile, Some("CommitFileList")),
        KeyBinding::new("up", SelectPreviousFile, Some("CommitFileList")),
        KeyBinding::new("shift-down", ExtendSelectionDown, Some("CommitFileList")),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("CommitFileList")),
        list_end_binding(true, "CommitFileList"),
        list_end_binding(false, "CommitFileList"),
        KeyBinding::new("home", SelectFirstFile, Some("CommitFileList")),
        KeyBinding::new("end", SelectLastFile, Some("CommitFileList")),
        KeyBinding::new("down", SelectNextFile, Some("StashFileList")),
        KeyBinding::new("up", SelectPreviousFile, Some("StashFileList")),
        KeyBinding::new("shift-down", ExtendSelectionDown, Some("StashFileList")),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("StashFileList")),
        list_end_binding(true, "StashFileList"),
        list_end_binding(false, "StashFileList"),
        KeyBinding::new("home", SelectFirstFile, Some("StashFileList")),
        KeyBinding::new("end", SelectLastFile, Some("StashFileList")),
        KeyBinding::new("down", SelectNextFile, Some("PullRequestFileList")),
        KeyBinding::new("up", SelectPreviousFile, Some("PullRequestFileList")),
        KeyBinding::new(
            "shift-down",
            ExtendSelectionDown,
            Some("PullRequestFileList"),
        ),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("PullRequestFileList")),
        list_end_binding(true, "PullRequestFileList"),
        list_end_binding(false, "PullRequestFileList"),
        KeyBinding::new("home", SelectFirstFile, Some("PullRequestFileList")),
        KeyBinding::new("end", SelectLastFile, Some("PullRequestFileList")),
        KeyBinding::new("enter", CompareSelect, Some("CompareFilter")),
        KeyBinding::new("escape", CompareClear, Some("CompareFilter")),
        KeyBinding::new("down", SelectNextFile, Some("CompareFilter")),
        KeyBinding::new("up", SelectPreviousFile, Some("CompareFilter")),
        KeyBinding::new("escape", ReorderCancel, Some("HistoryList")),
        // GHD `FilterList.onFilterKeyDown`: ↓ / ↑ / Enter from the filter box
        KeyBinding::new("down", SelectNextFile, Some("RepositoryFilter")),
        KeyBinding::new("up", SelectPreviousFile, Some("RepositoryFilter")),
        KeyBinding::new("enter", FilterListPick, Some("RepositoryFilter")),
        KeyBinding::new("down", SelectNextFile, Some("BranchFilter")),
        KeyBinding::new("up", SelectPreviousFile, Some("BranchFilter")),
        KeyBinding::new("enter", FilterListPick, Some("BranchFilter")),
        KeyBinding::new("down", SelectNextFile, Some("PullRequestFilter")),
        KeyBinding::new("up", SelectPreviousFile, Some("PullRequestFilter")),
        KeyBinding::new("enter", FilterListPick, Some("PullRequestFilter")),
        KeyBinding::new("down", SelectNextFile, Some("WorktreeFilter")),
        KeyBinding::new("up", SelectPreviousFile, Some("WorktreeFilter")),
        KeyBinding::new("enter", FilterListPick, Some("WorktreeFilter")),
        KeyBinding::new("secondary-,", OpenSettings, MENU),
        // ⌘⇧, - macOS delivers the shifted character, so the chord is `cmd-<`
        KeyBinding::new("secondary-<", OpenFlags, MENU),
        KeyBinding::new("secondary-q", Quit, None),
        // File
        KeyBinding::new("secondary-n", NewRepository, MENU),
        KeyBinding::new("secondary-o", AddLocalRepository, MENU),
        KeyBinding::new("shift-secondary-o", CloneRepository, MENU),
        // Edit
        KeyBinding::new("secondary-f", Find, None),
        // View
        KeyBinding::new("secondary-1", ShowChanges, MENU),
        KeyBinding::new("secondary-2", ShowHistory, MENU),
        KeyBinding::new("secondary-t", ShowRepositoryList, MENU),
        KeyBinding::new("secondary-b", ShowBranchesList, MENU),
        KeyBinding::new("alt-secondary-w", ShowWorktreesList, MENU),
        KeyBinding::new("secondary-g", GoToSummary, MENU),
        KeyBinding::new("ctrl-h", ToggleStashedChanges, MENU),
        KeyBinding::new("secondary-l", ToggleChangesFilter, MENU),
        KeyBinding::new(FULL_SCREEN, ToggleFullScreen, None),
        KeyBinding::new("secondary-0", ResetZoom, None),
        KeyBinding::new("secondary-=", ZoomIn, None),
        KeyBinding::new("secondary--", ZoomOut, None),
        KeyBinding::new("secondary-9", ExpandActiveResizable, None),
        KeyBinding::new("secondary-8", ContractActiveResizable, None),
        KeyBinding::new("ctrl-tab", ToggleSection, MENU),
        // Repository
        KeyBinding::new("shift-secondary-p", Pull, MENU),
        KeyBinding::new("shift-secondary-t", Fetch, MENU),
        KeyBinding::new("secondary-backspace", RemoveRepository, MENU),
        KeyBinding::new("shift-secondary-g", ViewOnGitHub, MENU),
        KeyBinding::new("ctrl-`", OpenInShell, MENU),
        KeyBinding::new("shift-secondary-f", ShowInFinder, MENU),
        KeyBinding::new("shift-secondary-a", OpenInEditor, MENU),
        KeyBinding::new("shift-alt-secondary-a", OpenWith, MENU),
        KeyBinding::new("secondary-i", CreateIssue, MENU),
        // Branch
        KeyBinding::new("shift-secondary-n", NewBranch, MENU),
        KeyBinding::new("shift-secondary-w", NewWorktree, MENU),
        KeyBinding::new("shift-secondary-r", RenameBranch, MENU),
        KeyBinding::new("shift-secondary-d", DeleteBranch, MENU),
        KeyBinding::new("shift-secondary-backspace", DiscardAllChanges, MENU),
        KeyBinding::new("shift-secondary-s", StashAllChanges, MENU),
        KeyBinding::new("shift-secondary-u", UpdateFromDefaultBranch, MENU),
        KeyBinding::new("shift-secondary-b", CompareToBranch, MENU),
        KeyBinding::new("shift-secondary-m", MergeIntoCurrentBranch, MENU),
        KeyBinding::new("shift-secondary-h", SquashAndMergeIntoCurrentBranch, MENU),
        KeyBinding::new("shift-secondary-e", RebaseCurrentBranch, MENU),
        KeyBinding::new("shift-secondary-c", CompareOnGitHub, MENU),
        KeyBinding::new("alt-secondary-b", ViewBranchOnGitHub, MENU),
        KeyBinding::new("alt-secondary-p", PreviewPullRequest, MENU),
        KeyBinding::new("secondary-r", CreatePullRequest, MENU),
        // In-app
        KeyBinding::new("secondary-enter", Commit, Some("CommitMessage")),
        KeyBinding::new("escape", CloseFoldout, None),
    ];
    // Corvane flags. Added last: at equal depth a later binding wins, and a
    // context-free binding counts as the deepest context.
    if flags.discard_selected_files {
        bindings.push(KeyBinding::new(
            "secondary-backspace",
            DiscardSelectedFiles,
            Some("ChangesList"),
        ));
    }
    if !flags.no_push_shortcut {
        bindings.push(KeyBinding::new("secondary-p", Push, MENU));
    }
    if flags.open_in_shell_alt_shortcut {
        bindings.push(KeyBinding::new("alt-secondary-t", OpenInShell, MENU));
    }
    if flags.emacs_list_keys {
        for context in ["ChangesList", "HistoryList"] {
            bindings.extend([
                KeyBinding::new("ctrl-n", SelectNextFile, Some(context)),
                KeyBinding::new("ctrl-p", SelectPreviousFile, Some(context)),
            ]);
        }
    }
    if flags.diff_mode_shortcut {
        bindings.push(KeyBinding::new(
            "alt-secondary-s",
            ToggleDiffDisplayMode,
            MENU,
        ));
    }
    if flags.copy_path_shortcuts {
        for context in ["ChangesList", "CommitFileList"] {
            bindings.extend([
                KeyBinding::new("alt-secondary-c", CopySelectedFilePaths, Some(context)),
                KeyBinding::new(
                    "shift-alt-secondary-c",
                    CopySelectedRelativeFilePaths,
                    Some(context),
                ),
            ]);
        }
    }
    if flags.navigation_shortcuts {
        bindings.extend([
            KeyBinding::new(CTRL_CMD_P, ShowPullRequestsList, MENU),
            // ⇧⌘] / ⇧⌘[: macOS delivers the shifted character
            KeyBinding::new("secondary-}", NextRepository, MENU),
            KeyBinding::new("secondary-{", PreviousRepository, MENU),
            KeyBinding::new("secondary-3", FocusDiff, MENU),
            KeyBinding::new("alt-down", SelectNextFileFromDiff, Some("Diff")),
            KeyBinding::new("alt-up", SelectPreviousFileFromDiff, Some("Diff")),
        ]);
    }
    if flags.history_review_mode {
        bindings.push(KeyBinding::new(CTRL_CMD_S, ToggleHistoryReviewMode, MENU));
    }
    if flags.open_file_shortcuts {
        for context in ["ChangesList", "CommitFileList"] {
            bindings.extend([
                KeyBinding::new("shift-secondary-a", OpenSelectedFileInEditor, Some(context)),
                KeyBinding::new(
                    "alt-secondary-o",
                    OpenSelectedFileWithDefaultProgram,
                    Some(context),
                ),
            ]);
        }
    }
    // GHD `Dialog.onKeyDown`: CmdOrCtrl+W dismisses the dialog (on macOS
    // ⌘W is the Window menu's Close Window below)
    #[cfg(not(target_os = "macos"))]
    bindings.push(KeyBinding::new("secondary-w", CloseFoldout, Some("Popup")));
    #[cfg(target_os = "macos")]
    bindings.extend([
        // the app menu and the Window menu (macOS only in GHD)
        KeyBinding::new("secondary-h", Hide, None),
        KeyBinding::new("alt-secondary-h", HideOthers, None),
        KeyBinding::new("secondary-m", Minimize, None),
        KeyBinding::new("secondary-w", CloseWindow, None),
    ]);
    bindings
}
