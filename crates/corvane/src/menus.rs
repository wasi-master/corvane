//! The menu bar, mirroring GitHub Desktop 3.6.6
//! (`app/src/main-process/menu/build-default-menu.ts`): native on macOS, the
//! model of the in-window menu bar elsewhere. Keyboard shortcuts are shown
//! from the keymap in `corvane_ui::keymap`. GPUI disables any item whose
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
    /// Flag `414-linux-install-cli` (the item is always there on macOS).
    pub install_cli: bool,
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
            install_cli: s.flags.bool(ids::LINUX_INSTALL_CLI),
            keymap: corvane_ui::keymap::KeymapFlags::from_flags(&s.flags),
        }
    }
}

/// GHD's label for this platform: `__DARWIN__ ? mac : other`. Off macOS
/// labels are sentence case and carry `&` mnemonics (Electron's classic
/// menu bar underlines the letter after `&`).
const fn l(mac: &'static str, other: &'static str) -> &'static str {
    if cfg!(target_os = "macos") {
        mac
    } else {
        other
    }
}

/// Build (or rebuild) the menu bar: the native macOS one, or on Linux the
/// model the in-window menu bar (`corvane_ui::menu_bar`, Electron's classic
/// menu bar) draws. Items are GHD's `build-default-menu.ts`, `__DARWIN__`
/// branch on macOS and the other one elsewhere.
/// Corvane additions: "Flags…" (no GHD equivalent), File › Import
/// Repositories from GitHub Desktop…, Repository › Fetch All Repositories,
/// Repository › View Upstream on GitHub, Repository › Add License…,
/// Window › Corvane (shows the window hidden with ⌘W) and Help › Show
/// Release Notes; on Linux "Flags…" and "Install Command Line Tool…" sit
/// under File after "Options…" (GHD's Linux menu has no app menu).
pub fn install(cx: &mut App, options: &MenuOptions) {
    let keymap = options.keymap;
    let (editor, shell) = (&options.editor, &options.shell);
    let (show_release_notes, show_import) = (options.show_release_notes, options.show_import);
    let mut repository = vec![
        MenuItem::action(l("Push", "P&ush"), Push),
        MenuItem::action(l("Pull", "Pu&ll"), Pull),
        MenuItem::action(l("Fetch", "&Fetch"), Fetch),
    ];
    if options.fetch_all {
        repository.push(MenuItem::action(
            l("Fetch All Repositories", "Fetch &all repositories"),
            FetchAllRepositories,
        ));
    }
    repository.extend([
        MenuItem::action(l("Remove…", "&Remove…"), RemoveRepository),
        MenuItem::separator(),
        MenuItem::action(l("View on GitHub", "&View on GitHub"), ViewOnGitHub),
    ]);
    if options.show_view_upstream {
        repository.push(MenuItem::action(
            l("View Upstream on GitHub", "View upstream on GitHub"),
            ViewUpstreamOnGitHub,
        ));
    }
    repository.extend([
        MenuItem::action(
            if cfg!(target_os = "macos") {
                format!("Open in {shell}")
            } else {
                format!("O&pen in {shell}")
            },
            OpenInShell,
        ),
        MenuItem::action(
            l("Show in Finder", "Show in your File Manager"),
            ShowInFinder,
        ),
        MenuItem::action(
            if cfg!(target_os = "macos") {
                format!("Open in {editor}")
            } else {
                format!("&Open in {editor}")
            },
            OpenInEditor,
        ),
        MenuItem::action(l("Open With…", "Open &with…"), OpenWith),
        MenuItem::separator(),
        MenuItem::action(
            l("Create Issue on GitHub", "Create &issue on GitHub"),
            CreateIssue,
        ),
        MenuItem::separator(),
        MenuItem::action(l("New Worktree…", "New work&tree…"), NewWorktree),
        MenuItem::separator(),
    ]);
    if options.show_add_license {
        repository.push(MenuItem::action(
            l("Add License…", "Add &license…"),
            AddLicense,
        ));
    }
    repository.push(MenuItem::action(
        l("Repository Settings…", "Repository &settings…"),
        RepositorySettings,
    ));
    let mut view = vec![
        MenuItem::action(l("Show Changes", "&Changes"), ShowChanges),
        MenuItem::action(l("Show History", "&History"), ShowHistory),
        MenuItem::action(
            l("Show Repository List", "Repository &list"),
            ShowRepositoryList,
        ),
        MenuItem::action(l("Show Branches List", "&Branches list"), ShowBranchesList),
        MenuItem::action(
            l("Show Worktrees List", "Wor&ktrees list"),
            ShowWorktreesList,
        ),
    ];
    // Corvane (`612-navigation-shortcuts`)
    if keymap.navigation_shortcuts {
        view.push(MenuItem::action(
            l("Show Pull Requests List", "&Pull requests list"),
            ShowPullRequestsList,
        ));
    }
    // Corvane (`801-history-review-mode`)
    if keymap.history_review_mode {
        view.push(MenuItem::action(
            l("Toggle History Review Mode", "Toggle history &review mode"),
            ToggleHistoryReviewMode,
        ));
    }
    view.extend([
        MenuItem::separator(),
        MenuItem::action(l("Go to Summary", "Go to &Summary"), GoToSummary),
        MenuItem::action(
            l("Show Stashed Changes", "Sho&w stashed changes"),
            ToggleStashedChanges,
        ),
        // GHD's non-macOS label really reads "Hide Toggle Changes Filter"
        MenuItem::action(
            l("Hide Changes Filter", "Hide Toggle Chan&ges Filter"),
            ToggleChangesFilter,
        ),
    ]);
    // Corvane's macOS menu sets Toggle Full Screen apart; GHD has no
    // separator before it (kept on macOS, GHD's layout elsewhere)
    if cfg!(target_os = "macos") {
        view.push(MenuItem::separator());
    }
    view.extend([
        MenuItem::action(
            l("Toggle Full Screen", "Toggle &full screen"),
            ToggleFullScreen,
        ),
        MenuItem::separator(),
        MenuItem::action(l("Reset Zoom", "Reset zoom"), ResetZoom),
        MenuItem::action(l("Zoom In", "Zoom in"), ZoomIn),
        MenuItem::action(l("Zoom Out", "Zoom out"), ZoomOut),
        MenuItem::action(
            l("Expand Active Resizable", "Expand active resizable"),
            ExpandActiveResizable,
        ),
        MenuItem::action(
            l("Contract Active Resizable", "Contract active resizable"),
            ContractActiveResizable,
        ),
    ]);
    let edit = Menu::new(l("Edit", "&Edit")).items([
        MenuItem::os_action(l("Undo", "&Undo"), Undo, OsAction::Undo),
        MenuItem::os_action(l("Redo", "&Redo"), Redo, OsAction::Redo),
        MenuItem::separator(),
        MenuItem::os_action(l("Cut", "Cu&t"), Cut, OsAction::Cut),
        MenuItem::os_action(l("Copy", "&Copy"), Copy, OsAction::Copy),
        MenuItem::os_action(l("Paste", "&Paste"), Paste, OsAction::Paste),
        MenuItem::os_action(
            l("Select All", "Select &all"),
            SelectAll,
            OsAction::SelectAll,
        ),
        MenuItem::separator(),
        MenuItem::action(l("Find", "&Find"), Find),
    ]);
    let branch = Menu::new(l("Branch", "&Branch")).items(
        [
            MenuItem::action(l("New Branch…", "New &branch…"), NewBranch),
            MenuItem::action(l("Rename…", "&Rename…"), RenameBranch),
            MenuItem::action(l("Delete…", "&Delete…"), DeleteBranch),
            MenuItem::separator(),
            MenuItem::action(
                l("Discard All Changes…", "Discard all changes…"),
                DiscardAllChanges,
            ),
            MenuItem::action(
                l("Stash All Changes", "&Stash all changes"),
                StashAllChanges,
            ),
            MenuItem::separator(),
            MenuItem::action(
                l("Update from Default Branch", "&Update from default branch"),
                UpdateFromDefaultBranch,
            ),
            MenuItem::action(
                l("Compare to Branch", "&Compare to branch"),
                CompareToBranch,
            ),
            MenuItem::action(
                l("Merge into Current Branch…", "&Merge into current branch…"),
                MergeIntoCurrentBranch,
            ),
            MenuItem::action(
                l(
                    "Squash and Merge into Current Branch…",
                    "Squas&h and merge into current branch…",
                ),
                SquashAndMergeIntoCurrentBranch,
            ),
            MenuItem::action(
                l("Rebase Current Branch…", "R&ebase current branch…"),
                RebaseCurrentBranch,
            ),
            MenuItem::separator(),
            MenuItem::action(
                l("Compare on GitHub", "Compare on &GitHub"),
                CompareOnGitHub,
            ),
            MenuItem::action(
                l("View Branch on GitHub", "View branch on GitHub"),
                ViewBranchOnGitHub,
            ),
        ]
        .into_iter()
        // same as Toggle Full Screen: a macOS-only separator
        .chain(cfg!(target_os = "macos").then(MenuItem::separator))
        .chain([
            MenuItem::action(
                l("Preview Pull Request", "Preview pull request"),
                PreviewPullRequest,
            ),
            MenuItem::action(
                l("Create Pull Request", "Create &pull request"),
                CreatePullRequest,
            ),
        ]),
    );
    let mut file_items = vec![
        MenuItem::action(l("New Repository…", "New &repository…"), NewRepository),
        MenuItem::separator(),
        MenuItem::action(
            l("Add Local Repository…", "Add &local repository…"),
            AddLocalRepository,
        ),
        MenuItem::action(
            l("Clone Repository…", "Clo&ne repository…"),
            CloneRepository,
        ),
    ];
    if show_import {
        file_items.push(MenuItem::action(
            l(
                "Import Repositories from GitHub Desktop…",
                "&Import repositories from GitHub Desktop…",
            ),
            ImportFromGitHubDesktop,
        ));
    }
    let mut menus = Vec::new();
    #[cfg(target_os = "macos")]
    menus.push(Menu::new("Corvane").items([
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
    ]));
    #[cfg(not(target_os = "macos"))]
    {
        file_items.extend([
            MenuItem::separator(),
            MenuItem::action("&Options…", OpenSettings),
            MenuItem::action("Fla&gs…", OpenFlags),
        ]);
        if options.install_cli {
            file_items.push(MenuItem::action("Install command line &tool…", InstallCli));
        }
        file_items.extend([MenuItem::separator(), MenuItem::action("E&xit", Quit)]);
    }
    menus.extend([
        Menu::new(l("File", "&File")).items(file_items),
        edit,
        Menu::new(l("View", "&View")).items(view),
        Menu::new(l("Repository", "&Repository")).items(repository),
        branch,
    ]);
    #[cfg(target_os = "macos")]
    menus.push(Menu::new("Window").items(window_items(options.show_main_window)));
    menus.push(Menu::new(l("Help", "&Help")).items(help_items(show_release_notes)));
    cx.set_menus(menus);
}

/// Window menu; flag `405-window-menu-main-window` appends "Corvane", which
/// shows the main window again after ⌘W or the red close button.
#[cfg(target_os = "macos")]
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
/// "Show notification" so far, as "Show Test Notifications"). Off macOS
/// About closes the menu, after a separator (GHD's non-darwin `&Help`).
fn help_items(show_release_notes: bool) -> Vec<MenuItem> {
    #[allow(unused_mut)]
    let mut items = vec![
        MenuItem::action(l("Report Issue…", "Report issue…"), ReportIssue),
        MenuItem::action(
            l("Contact GitHub Support…", "&Contact GitHub support…"),
            ContactSupport,
        ),
        MenuItem::action("Show User Guides", ShowUserGuides),
        MenuItem::action(
            l("Show Keyboard Shortcuts", "Show keyboard shortcuts"),
            ShowKeyboardShortcuts,
        ),
        MenuItem::action(
            l("Show Logs in Finder", "S&how logs in your File Manager"),
            ShowLogs,
        ),
    ];
    if show_release_notes {
        items.extend([
            MenuItem::separator(),
            MenuItem::action(
                l("Show Release Notes", "Show release &notes"),
                ShowReleaseNotes,
            ),
        ]);
    }
    #[cfg(debug_assertions)]
    items.extend([
        MenuItem::separator(),
        MenuItem::action(
            l("Show Test Notifications", "Show test notifications"),
            ShowTestNotifications,
        ),
    ]);
    #[cfg(not(target_os = "macos"))]
    items.extend([
        MenuItem::separator(),
        MenuItem::action("&About Corvane", About),
    ]);
    items
}
