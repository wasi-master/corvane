//! Native macOS menu bar, mirroring GitHub Desktop 3.6.6
//! (`app/src/main-process/menu/build-default-menu.ts`). Keyboard shortcuts are
//! shown from the keymap in `corvane_ui::keymap`. GPUI disables any item whose
//! action has no live handler, which matches GHD's context-dependent enabling.

use corvane_ui::actions::*;
use gpui_kit::*;

/// What the menu bar depends on; a change rebuilds it.
#[derive(Clone, PartialEq, Eq)]
pub struct MenuOptions {
    /// Labels for the dynamic "Open in …" items (GHD `editorLabel` /
    /// `shellLabel`).
    pub editor: String,
    pub shell: String,
    /// Flag `401-release-notes-menu-item`.
    pub show_release_notes: bool,
    /// Flag `206-import-from-github-desktop`.
    pub show_import: bool,
    /// Flag `321-view-upstream-on-github`.
    pub show_view_upstream: bool,
    /// Flag `405-window-menu-main-window`.
    pub show_main_window: bool,
    /// Flag `221-add-license`.
    pub show_add_license: bool,
    /// Flag `247-fetch-all-repositories`.
    pub fetch_all: bool,
    /// Flags that add key bindings and their View menu items
    /// (`612-navigation-shortcuts`, `801-history-review-mode`).
    pub keymap: corvane_ui::keymap::KeymapFlags,
}

impl MenuOptions {
    pub fn of(s: &corvane_core::AppState) -> Self {
        use corvane_core::flags::ids;
        Self {
            editor: s.editor_label(),
            shell: s.shell_label(),
            show_release_notes: s.flags.bool(ids::RELEASE_NOTES_MENU_ITEM),
            show_import: s.flags.bool(ids::IMPORT_FROM_GITHUB_DESKTOP),
            show_view_upstream: s.flags.bool(ids::VIEW_UPSTREAM_ON_GITHUB),
            show_main_window: s.flags.bool(ids::WINDOW_MENU_MAIN_WINDOW),
            show_add_license: s.flags.bool(ids::ADD_LICENSE),
            fetch_all: s.flags.bool(ids::FETCH_ALL_REPOSITORIES),
            keymap: corvane_ui::keymap::KeymapFlags::from_flags(&s.flags),
        }
    }
}

/// Build (or rebuild) the menu bar.
/// Corvane additions: "Flags…" (no GHD equivalent), File › Import
/// Repositories from GitHub Desktop…, Repository › Fetch All Repositories,
/// Repository › View Upstream on GitHub, Repository › Add License…,
/// Window › Corvane (shows the window hidden with ⌘W) and Help › Show
/// Release Notes.
pub fn install(cx: &mut App, options: &MenuOptions) {
    let keymap = options.keymap;
    let (editor, shell) = (&options.editor, &options.shell);
    let (show_release_notes, show_import) = (options.show_release_notes, options.show_import);
    let mut repository = vec![
        MenuItem::action("Push", Push),
        MenuItem::action("Pull", Pull),
        MenuItem::action("Fetch", Fetch),
    ];
    if options.fetch_all {
        repository.push(MenuItem::action(
            "Fetch All Repositories",
            FetchAllRepositories,
        ));
    }
    repository.extend([
        MenuItem::action("Remove…", RemoveRepository),
        MenuItem::separator(),
        MenuItem::action("View on GitHub", ViewOnGitHub),
    ]);
    if options.show_view_upstream {
        repository.push(MenuItem::action(
            "View Upstream on GitHub",
            ViewUpstreamOnGitHub,
        ));
    }
    repository.extend([
        MenuItem::action(format!("Open in {shell}"), OpenInShell),
        MenuItem::action("Show in Finder", ShowInFinder),
        MenuItem::action(format!("Open in {editor}"), OpenInEditor),
        MenuItem::action("Open With…", OpenWith),
        MenuItem::separator(),
        MenuItem::action("Create Issue on GitHub", CreateIssue),
        MenuItem::separator(),
        MenuItem::action("New Worktree…", NewWorktree),
        MenuItem::separator(),
    ]);
    if options.show_add_license {
        repository.push(MenuItem::action("Add License…", AddLicense));
    }
    repository.push(MenuItem::action("Repository Settings…", RepositorySettings));
    let mut view = vec![
        MenuItem::action("Show Changes", ShowChanges),
        MenuItem::action("Show History", ShowHistory),
        MenuItem::action("Show Repository List", ShowRepositoryList),
        MenuItem::action("Show Branches List", ShowBranchesList),
        MenuItem::action("Show Worktrees List", ShowWorktreesList),
    ];
    // Corvane (`612-navigation-shortcuts`)
    if keymap.navigation_shortcuts {
        view.push(MenuItem::action(
            "Show Pull Requests List",
            ShowPullRequestsList,
        ));
    }
    // Corvane (`801-history-review-mode`)
    if keymap.history_review_mode {
        view.push(MenuItem::action(
            "Toggle History Review Mode",
            ToggleHistoryReviewMode,
        ));
    }
    view.extend([
        MenuItem::separator(),
        MenuItem::action("Go to Summary", GoToSummary),
        MenuItem::action("Show Stashed Changes", ToggleStashedChanges),
        MenuItem::action("Hide Changes Filter", ToggleChangesFilter),
        MenuItem::separator(),
        MenuItem::action("Toggle Full Screen", ToggleFullScreen),
        MenuItem::separator(),
        MenuItem::action("Reset Zoom", ResetZoom),
        MenuItem::action("Zoom In", ZoomIn),
        MenuItem::action("Zoom Out", ZoomOut),
        MenuItem::action("Expand Active Resizable", ExpandActiveResizable),
        MenuItem::action("Contract Active Resizable", ContractActiveResizable),
    ]);
    cx.set_menus(vec![
        Menu::new("Corvane").items([
            MenuItem::action("About Corvane", About),
            MenuItem::separator(),
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::action("Flags…", OpenFlags),
            MenuItem::action("Install Command Line Tool…", InstallCli),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide Corvane", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit Corvane", Quit),
        ]),
        Menu::new("File").items(
            [
                MenuItem::action("New Repository…", NewRepository),
                MenuItem::separator(),
                MenuItem::action("Add Local Repository…", AddLocalRepository),
                MenuItem::action("Clone Repository…", CloneRepository),
            ]
            .into_iter()
            .chain(show_import.then(|| {
                MenuItem::action(
                    "Import Repositories from GitHub Desktop…",
                    ImportFromGitHubDesktop,
                )
            })),
        ),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", Undo, OsAction::Undo),
            MenuItem::os_action("Redo", Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action("Find", Find),
        ]),
        Menu::new("View").items(view),
        Menu::new("Repository").items(repository),
        Menu::new("Branch").items([
            MenuItem::action("New Branch…", NewBranch),
            MenuItem::action("Rename…", RenameBranch),
            MenuItem::action("Delete…", DeleteBranch),
            MenuItem::separator(),
            MenuItem::action("Discard All Changes…", DiscardAllChanges),
            MenuItem::action("Stash All Changes", StashAllChanges),
            MenuItem::separator(),
            MenuItem::action("Update from Default Branch", UpdateFromDefaultBranch),
            MenuItem::action("Compare to Branch", CompareToBranch),
            MenuItem::action("Merge into Current Branch…", MergeIntoCurrentBranch),
            MenuItem::action(
                "Squash and Merge into Current Branch…",
                SquashAndMergeIntoCurrentBranch,
            ),
            MenuItem::action("Rebase Current Branch…", RebaseCurrentBranch),
            MenuItem::separator(),
            MenuItem::action("Compare on GitHub", CompareOnGitHub),
            MenuItem::action("View Branch on GitHub", ViewBranchOnGitHub),
            MenuItem::separator(),
            MenuItem::action("Preview Pull Request", PreviewPullRequest),
            MenuItem::action("Create Pull Request", CreatePullRequest),
        ]),
        Menu::new("Window").items(window_items(options.show_main_window)),
        Menu::new("Help").items(help_items(show_release_notes)),
    ]);
}

/// Window menu; flag `405-window-menu-main-window` appends "Corvane", which
/// shows the main window again after ⌘W or the red close button.
fn window_items(show_main_window: bool) -> Vec<MenuItem> {
    let mut items = vec![
        MenuItem::action("Minimize", Minimize),
        MenuItem::action("Zoom", Zoom),
        MenuItem::action("Close Window", CloseWindow),
        MenuItem::separator(),
        MenuItem::action("Bring All to Front", BringAllToFront),
    ];
    if show_main_window {
        items.extend([
            MenuItem::separator(),
            MenuItem::action("Corvane", ShowMainWindow),
        ]);
    }
    items
}

/// Help menu; debug builds append GHD's test items (`buildTestMenu`, only
/// "Show notification" so far, as "Show Test Notifications").
fn help_items(show_release_notes: bool) -> Vec<MenuItem> {
    #[allow(unused_mut)]
    let mut items = vec![
        MenuItem::action("Report Issue…", ReportIssue),
        MenuItem::action("Contact GitHub Support…", ContactSupport),
        MenuItem::action("Show User Guides", ShowUserGuides),
        MenuItem::action("Show Keyboard Shortcuts", ShowKeyboardShortcuts),
        MenuItem::action("Show Logs in Finder", ShowLogs),
    ];
    if show_release_notes {
        items.extend([
            MenuItem::separator(),
            MenuItem::action("Show Release Notes", ShowReleaseNotes),
        ]);
    }
    #[cfg(debug_assertions)]
    items.extend([
        MenuItem::separator(),
        MenuItem::action("Show Test Notifications", ShowTestNotifications),
    ]);
    items
}
