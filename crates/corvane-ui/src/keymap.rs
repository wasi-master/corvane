//! GitHub Desktop's macOS keyboard shortcuts (`build-default-menu.ts`).

use gpui_kit::{App, KeyBinding};

use crate::actions::*;

pub fn install(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("down", SelectNextFile, Some("ChangesList")),
        KeyBinding::new("up", SelectPreviousFile, Some("ChangesList")),
        KeyBinding::new("cmd-a", SelectAllFiles, Some("ChangesList")),
        KeyBinding::new("shift-down", ExtendSelectionDown, Some("ChangesList")),
        KeyBinding::new("shift-up", ExtendSelectionUp, Some("ChangesList")),
        KeyBinding::new("down", SelectNextFile, Some("HistoryList")),
        KeyBinding::new("up", SelectPreviousFile, Some("HistoryList")),
        KeyBinding::new("enter", ReorderConfirm, Some("HistoryList")),
        KeyBinding::new("enter", CompareSelect, Some("CompareFilter")),
        KeyBinding::new("escape", CompareClear, Some("CompareFilter")),
        KeyBinding::new("down", SelectNextFile, Some("CompareFilter")),
        KeyBinding::new("up", SelectPreviousFile, Some("CompareFilter")),
        KeyBinding::new("escape", ReorderCancel, Some("HistoryList")),
        KeyBinding::new("cmd-,", OpenSettings, None),
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
        KeyBinding::new("cmd-p", Push, None),
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
    ]);
}
