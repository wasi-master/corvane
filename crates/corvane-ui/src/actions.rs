//! Every menu / keyboard action, named after GitHub Desktop's menu items.

gpui_kit::actions!(
    corvane,
    [
        // Lists (arrow keys while the changes list has focus)
        SelectNextFile,
        SelectPreviousFile,
        SelectAllFiles,
        // ⇧↑ / ⇧↓ range selection in multi-select lists
        ExtendSelectionUp,
        ExtendSelectionDown,
        // ⌘↑ / ⌘↓ (Home / End): the first / last row (GHD `List` isHomeKey / isEndKey)
        SelectFirstFile,
        SelectLastFile,
        // Space in the changes list: include / exclude the highlighted files
        ToggleIncludeSelected,
        // ⌘⌫ in the changes list (`607-cmd-backspace-discards-files`)
        DiscardSelectedFiles,
        // ⇧⌘A / ⌥⌘O in a file list (`608-open-file-shortcuts`)
        OpenSelectedFileInEditor,
        OpenSelectedFileWithDefaultProgram,
        // ⌥⌘S: unified ⇄ split diff (`612-diff-mode-shortcut`)
        ToggleDiffDisplayMode,
        // ⌥⌘C / ⇧⌥⌘C in a file list (`613-copy-path-shortcuts`)
        CopySelectedFilePaths,
        CopySelectedRelativeFilePaths,
        // `614-navigation-shortcuts`
        ShowPullRequestsList,
        NextRepository,
        PreviousRepository,
        FocusDiff,
        SelectNextFileFromDiff,
        SelectPreviousFileFromDiff,
        // Worktrees
        NewWorktree,
        ShowWorktreesList,
        // Commit form: "Add Co-Authors" / "Remove Co-Authors"
        ToggleCoAuthors,
        // Commit form context menu (spelling suggestions are picked by index)
        SpellSuggestion0,
        SpellSuggestion1,
        SpellSuggestion2,
        SpellSuggestion3,
        SpellSuggestion4,
        SpellAddToDictionary,
        ToggleCommitSpellcheck,
        // Compare-to-branch filter box
        CompareSelect,
        CompareClear,
        // History keyboard reorder mode
        ReorderMoveUp,
        ReorderMoveDown,
        ReorderConfirm,
        ReorderCancel,
        // App menu
        About,
        OpenSettings,
        OpenFlags,
        InstallCli,
        Hide,
        HideOthers,
        ShowAll,
        Quit,
        // File
        NewRepository,
        AddLocalRepository,
        CloneRepository,
        ImportFromGitHubDesktop,
        // Edit
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll,
        Find,
        // View
        ShowChanges,
        ShowHistory,
        ShowRepositoryList,
        ShowBranchesList,
        GoToSummary,
        ToggleStashedChanges,
        ToggleChangesFilter,
        ToggleFullScreen,
        ResetZoom,
        ZoomIn,
        ZoomOut,
        ExpandActiveResizable,
        ContractActiveResizable,
        // Repository
        Push,
        Pull,
        Fetch,
        RemoveRepository,
        ViewOnGitHub,
        OpenInShell,
        ShowInFinder,
        OpenInEditor,
        OpenWith,
        CreateIssue,
        RepositorySettings,
        // Branch
        NewBranch,
        RenameBranch,
        DeleteBranch,
        DiscardAllChanges,
        StashAllChanges,
        UpdateFromDefaultBranch,
        CompareToBranch,
        MergeIntoCurrentBranch,
        SquashAndMergeIntoCurrentBranch,
        RebaseCurrentBranch,
        CompareOnGitHub,
        ViewBranchOnGitHub,
        PreviewPullRequest,
        CreatePullRequest,
        // Window
        Minimize,
        Zoom,
        CloseWindow,
        BringAllToFront,
        // Help
        ReportIssue,
        ContactSupport,
        ShowUserGuides,
        ShowReleaseNotes,
        // Help › Show Test Notifications (debug builds; GHD test menu "Show notification")
        ShowTestNotifications,
        ShowKeyboardShortcuts,
        ShowLogs,
        // In-app
        Commit,
        ToggleSection,
        CloseFoldout,
    ]
);
