//! GitHub Desktop's macOS keyboard shortcuts (`build-default-menu.ts`).
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

fn bindings(flags: KeymapFlags) -> Vec<KeyBinding> {
    let mut bindings = vec![
        KeyBinding::new("down", SelectNextFile, Some("ChangesList")),
        KeyBinding::new("up", SelectPreviousFile, Some("ChangesList")),
        KeyBinding::new("cmd-a", SelectAllFiles, Some("ChangesList")),
        // the diff's text selection (GHD `select-all` / the browser's copy)
        KeyBinding::new("cmd-a", SelectAll, Some("Diff")),
        KeyBinding::new("cmd-c", Copy, Some("Diff")),
        KeyBinding::new("shift-down", ExtendSelectionDown, Some("ChangesList")),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("ChangesList")),
        KeyBinding::new("cmd-up", SelectFirstFile, Some("ChangesList")),
        KeyBinding::new("cmd-down", SelectLastFile, Some("ChangesList")),
        KeyBinding::new("home", SelectFirstFile, Some("ChangesList")),
        KeyBinding::new("end", SelectLastFile, Some("ChangesList")),
        KeyBinding::new("space", ToggleIncludeSelected, Some("ChangesList")),
        KeyBinding::new("down", SelectNextFile, Some("HistoryList")),
        KeyBinding::new("up", SelectPreviousFile, Some("HistoryList")),
        KeyBinding::new("shift-down", ExtendSelectionDown, Some("HistoryList")),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("HistoryList")),
        KeyBinding::new("cmd-up", SelectFirstFile, Some("HistoryList")),
        KeyBinding::new("cmd-down", SelectLastFile, Some("HistoryList")),
        KeyBinding::new("home", SelectFirstFile, Some("HistoryList")),
        KeyBinding::new("end", SelectLastFile, Some("HistoryList")),
        KeyBinding::new("enter", ReorderConfirm, Some("HistoryList")),
        // GHD `List.handleKeyDown` in the commit, stash and pull request
        // file lists (`FileList`)
        KeyBinding::new("down", SelectNextFile, Some("CommitFileList")),
        KeyBinding::new("up", SelectPreviousFile, Some("CommitFileList")),
        KeyBinding::new("shift-down", ExtendSelectionDown, Some("CommitFileList")),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("CommitFileList")),
        KeyBinding::new("cmd-up", SelectFirstFile, Some("CommitFileList")),
        KeyBinding::new("cmd-down", SelectLastFile, Some("CommitFileList")),
        KeyBinding::new("home", SelectFirstFile, Some("CommitFileList")),
        KeyBinding::new("end", SelectLastFile, Some("CommitFileList")),
        KeyBinding::new("down", SelectNextFile, Some("StashFileList")),
        KeyBinding::new("up", SelectPreviousFile, Some("StashFileList")),
        KeyBinding::new("shift-down", ExtendSelectionDown, Some("StashFileList")),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("StashFileList")),
        KeyBinding::new("cmd-up", SelectFirstFile, Some("StashFileList")),
        KeyBinding::new("cmd-down", SelectLastFile, Some("StashFileList")),
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
        KeyBinding::new("cmd-up", SelectFirstFile, Some("PullRequestFileList")),
        KeyBinding::new("cmd-down", SelectLastFile, Some("PullRequestFileList")),
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
        KeyBinding::new("cmd-,", OpenSettings, MENU),
        // ⌘⇧, - macOS delivers the shifted character, so the chord is `cmd-<`
        KeyBinding::new("cmd-<", OpenFlags, MENU),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-q", Quit, None),
        // File
        KeyBinding::new("cmd-n", NewRepository, MENU),
        KeyBinding::new("cmd-o", AddLocalRepository, MENU),
        KeyBinding::new("shift-cmd-o", CloneRepository, MENU),
        // Edit
        KeyBinding::new("cmd-f", Find, None),
        // View
        KeyBinding::new("cmd-1", ShowChanges, MENU),
        KeyBinding::new("cmd-2", ShowHistory, MENU),
        KeyBinding::new("cmd-t", ShowRepositoryList, MENU),
        KeyBinding::new("cmd-b", ShowBranchesList, MENU),
        KeyBinding::new("alt-cmd-w", ShowWorktreesList, MENU),
        KeyBinding::new("cmd-g", GoToSummary, MENU),
        KeyBinding::new("ctrl-h", ToggleStashedChanges, MENU),
        KeyBinding::new("cmd-l", ToggleChangesFilter, MENU),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        KeyBinding::new("cmd-0", ResetZoom, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-9", ExpandActiveResizable, None),
        KeyBinding::new("cmd-8", ContractActiveResizable, None),
        KeyBinding::new("ctrl-tab", ToggleSection, MENU),
        // Repository
        KeyBinding::new("shift-cmd-p", Pull, MENU),
        KeyBinding::new("shift-cmd-t", Fetch, MENU),
        KeyBinding::new("cmd-backspace", RemoveRepository, MENU),
        KeyBinding::new("shift-cmd-g", ViewOnGitHub, MENU),
        KeyBinding::new("ctrl-`", OpenInShell, MENU),
        KeyBinding::new("shift-cmd-f", ShowInFinder, MENU),
        KeyBinding::new("shift-cmd-a", OpenInEditor, MENU),
        KeyBinding::new("shift-alt-cmd-a", OpenWith, MENU),
        KeyBinding::new("cmd-i", CreateIssue, MENU),
        // Branch
        KeyBinding::new("shift-cmd-n", NewBranch, MENU),
        KeyBinding::new("shift-cmd-w", NewWorktree, MENU),
        KeyBinding::new("shift-cmd-r", RenameBranch, MENU),
        KeyBinding::new("shift-cmd-d", DeleteBranch, MENU),
        KeyBinding::new("shift-cmd-backspace", DiscardAllChanges, MENU),
        KeyBinding::new("shift-cmd-s", StashAllChanges, MENU),
        KeyBinding::new("shift-cmd-u", UpdateFromDefaultBranch, MENU),
        KeyBinding::new("shift-cmd-b", CompareToBranch, MENU),
        KeyBinding::new("shift-cmd-m", MergeIntoCurrentBranch, MENU),
        KeyBinding::new("shift-cmd-h", SquashAndMergeIntoCurrentBranch, MENU),
        KeyBinding::new("shift-cmd-e", RebaseCurrentBranch, MENU),
        KeyBinding::new("shift-cmd-c", CompareOnGitHub, MENU),
        KeyBinding::new("alt-cmd-b", ViewBranchOnGitHub, MENU),
        KeyBinding::new("alt-cmd-p", PreviewPullRequest, MENU),
        KeyBinding::new("cmd-r", CreatePullRequest, MENU),
        // Window
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        // In-app
        KeyBinding::new("cmd-enter", Commit, Some("CommitMessage")),
        KeyBinding::new("escape", CloseFoldout, None),
    ];
    // Corvane flags. Added last: at equal depth a later binding wins, and a
    // context-free binding counts as the deepest context.
    if flags.discard_selected_files {
        bindings.push(KeyBinding::new(
            "cmd-backspace",
            DiscardSelectedFiles,
            Some("ChangesList"),
        ));
    }
    if !flags.no_push_shortcut {
        bindings.push(KeyBinding::new("cmd-p", Push, MENU));
    }
    if flags.open_in_shell_alt_shortcut {
        bindings.push(KeyBinding::new("alt-cmd-t", OpenInShell, MENU));
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
        bindings.push(KeyBinding::new("alt-cmd-s", ToggleDiffDisplayMode, MENU));
    }
    if flags.copy_path_shortcuts {
        for context in ["ChangesList", "CommitFileList"] {
            bindings.extend([
                KeyBinding::new("alt-cmd-c", CopySelectedFilePaths, Some(context)),
                KeyBinding::new(
                    "shift-alt-cmd-c",
                    CopySelectedRelativeFilePaths,
                    Some(context),
                ),
            ]);
        }
    }
    if flags.navigation_shortcuts {
        bindings.extend([
            KeyBinding::new("ctrl-cmd-p", ShowPullRequestsList, MENU),
            // ⇧⌘] / ⇧⌘[: macOS delivers the shifted character
            KeyBinding::new("cmd-}", NextRepository, MENU),
            KeyBinding::new("cmd-{", PreviousRepository, MENU),
            KeyBinding::new("cmd-3", FocusDiff, MENU),
            KeyBinding::new("alt-down", SelectNextFileFromDiff, Some("Diff")),
            KeyBinding::new("alt-up", SelectPreviousFileFromDiff, Some("Diff")),
        ]);
    }
    if flags.history_review_mode {
        bindings.push(KeyBinding::new("ctrl-cmd-s", ToggleHistoryReviewMode, MENU));
    }
    if flags.open_file_shortcuts {
        for context in ["ChangesList", "CommitFileList"] {
            bindings.extend([
                KeyBinding::new("shift-cmd-a", OpenSelectedFileInEditor, Some(context)),
                KeyBinding::new(
                    "alt-cmd-o",
                    OpenSelectedFileWithDefaultProgram,
                    Some(context),
                ),
            ]);
        }
    }
    bindings
}
