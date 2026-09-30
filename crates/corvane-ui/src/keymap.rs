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
    /// `607-cmd-backspace-discards-files`: ⌘⌫ in the changes list discards
    /// the selected files instead of removing the repository.
    pub discard_selected_files: bool,
    /// `608-open-file-shortcuts`: ⇧⌘A / ⌥⌘O in the changes and commit file
    /// lists open the selected file in the editor / default program.
    pub open_file_shortcuts: bool,
    /// `609-no-push-shortcut`: ⌘P does not push.
    pub no_push_shortcut: bool,
    /// `610-open-in-shell-alt-shortcut`: ⌥⌘T also opens the shell.
    pub open_in_shell_alt_shortcut: bool,
    /// `611-emacs-list-keys`: ⌃N / ⌃P move through the changes and history lists.
    pub emacs_list_keys: bool,
    /// `612-diff-mode-shortcut`: ⌥⌘S switches between unified and split diffs.
    pub diff_mode_shortcut: bool,
    /// `613-copy-path-shortcuts`: ⌥⌘C / ⇧⌥⌘C copy the selected files' paths.
    pub copy_path_shortcuts: bool,
    /// `614-navigation-shortcuts`: ⌃⌘P pull requests, ⇧⌘] / ⇧⌘[ next /
    /// previous repository, ⌘3 the diff, ⌥↓ / ⌥↑ files from the diff.
    pub navigation_shortcuts: bool,
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
        }
    }
}

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
        KeyBinding::new("enter", CompareSelect, Some("CompareFilter")),
        KeyBinding::new("escape", CompareClear, Some("CompareFilter")),
        KeyBinding::new("down", SelectNextFile, Some("CompareFilter")),
        KeyBinding::new("up", SelectPreviousFile, Some("CompareFilter")),
        KeyBinding::new("escape", ReorderCancel, Some("HistoryList")),
        KeyBinding::new("cmd-,", OpenSettings, None),
        // ⌘⇧, - macOS delivers the shifted character, so the chord is `cmd-<`
        KeyBinding::new("cmd-<", OpenFlags, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-q", Quit, None),
        // File
        KeyBinding::new("cmd-n", NewRepository, None),
        KeyBinding::new("cmd-o", AddLocalRepository, None),
        KeyBinding::new("shift-cmd-o", CloneRepository, None),
        // Edit
        KeyBinding::new("cmd-f", Find, None),
        // View
        KeyBinding::new("cmd-1", ShowChanges, None),
        KeyBinding::new("cmd-2", ShowHistory, None),
        KeyBinding::new("cmd-t", ShowRepositoryList, None),
        KeyBinding::new("cmd-b", ShowBranchesList, None),
        KeyBinding::new("alt-cmd-w", ShowWorktreesList, None),
        KeyBinding::new("cmd-g", GoToSummary, None),
        KeyBinding::new("ctrl-h", ToggleStashedChanges, None),
        KeyBinding::new("cmd-l", ToggleChangesFilter, None),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        KeyBinding::new("cmd-0", ResetZoom, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-9", ExpandActiveResizable, None),
        KeyBinding::new("cmd-8", ContractActiveResizable, None),
        KeyBinding::new("ctrl-tab", ToggleSection, None),
        // Repository
        KeyBinding::new("shift-cmd-p", Pull, None),
        KeyBinding::new("shift-cmd-t", Fetch, None),
        KeyBinding::new("cmd-backspace", RemoveRepository, None),
        KeyBinding::new("shift-cmd-g", ViewOnGitHub, None),
        KeyBinding::new("ctrl-`", OpenInShell, None),
        KeyBinding::new("shift-cmd-f", ShowInFinder, None),
        KeyBinding::new("shift-cmd-a", OpenInEditor, None),
        KeyBinding::new("shift-alt-cmd-a", OpenWith, None),
        KeyBinding::new("cmd-i", CreateIssue, None),
        // Branch
        KeyBinding::new("shift-cmd-n", NewBranch, None),
        KeyBinding::new("shift-cmd-w", NewWorktree, None),
        KeyBinding::new("shift-cmd-r", RenameBranch, None),
        KeyBinding::new("shift-cmd-d", DeleteBranch, None),
        KeyBinding::new("shift-cmd-backspace", DiscardAllChanges, None),
        KeyBinding::new("shift-cmd-s", StashAllChanges, None),
        KeyBinding::new("shift-cmd-u", UpdateFromDefaultBranch, None),
        KeyBinding::new("shift-cmd-b", CompareToBranch, None),
        KeyBinding::new("shift-cmd-m", MergeIntoCurrentBranch, None),
        KeyBinding::new("shift-cmd-h", SquashAndMergeIntoCurrentBranch, None),
        KeyBinding::new("shift-cmd-e", RebaseCurrentBranch, None),
        KeyBinding::new("shift-cmd-c", CompareOnGitHub, None),
        KeyBinding::new("alt-cmd-b", ViewBranchOnGitHub, None),
        KeyBinding::new("alt-cmd-p", PreviewPullRequest, None),
        KeyBinding::new("cmd-r", CreatePullRequest, None),
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
        bindings.push(KeyBinding::new("cmd-p", Push, None));
    }
    if flags.open_in_shell_alt_shortcut {
        bindings.push(KeyBinding::new("alt-cmd-t", OpenInShell, None));
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
        bindings.push(KeyBinding::new("alt-cmd-s", ToggleDiffDisplayMode, None));
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
            KeyBinding::new("ctrl-cmd-p", ShowPullRequestsList, None),
            // ⇧⌘] / ⇧⌘[: macOS delivers the shifted character
            KeyBinding::new("cmd-}", NextRepository, None),
            KeyBinding::new("cmd-{", PreviousRepository, None),
            KeyBinding::new("cmd-3", FocusDiff, None),
            KeyBinding::new("alt-down", SelectNextFileFromDiff, Some("Diff")),
            KeyBinding::new("alt-up", SelectPreviousFileFromDiff, Some("Diff")),
        ]);
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
