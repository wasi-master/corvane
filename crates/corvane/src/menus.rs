//! Native macOS menu bar, mirroring GitHub Desktop 3.6.6
//! (`app/src/main-process/menu/build-default-menu.ts`). Keyboard shortcuts are
//! shown from the keymap in `corvane_ui::keymap`. GPUI disables any item whose
//! action has no live handler, which matches GHD's context-dependent enabling.

use corvane_ui::actions::*;
use gpui_kit::*;

/// Build (or rebuild) the menu bar. `editor` / `shell` are the labels for
/// the dynamic "Open in …" items (GHD `editorLabel` / `shellLabel`);
/// `show_release_notes` is the `401-release-notes-menu-item` flag and
/// `show_import` the `206-import-from-github-desktop` one.
/// Corvane additions: "Flags…" (no GHD equivalent), File › Import
/// Repositories from GitHub Desktop… and Help › Show Release Notes.
pub fn install(
    cx: &mut App,
    editor: &str,
    shell: &str,
    show_release_notes: bool,
    show_import: bool,
) {
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
        Menu::new("View").items([
            MenuItem::action("Show Changes", ShowChanges),
            MenuItem::action("Show History", ShowHistory),
            MenuItem::action("Show Repository List", ShowRepositoryList),
            MenuItem::action("Show Branches List", ShowBranchesList),
            MenuItem::action("Show Worktrees List", ShowWorktreesList),
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
        ]),
        Menu::new("Repository").items([
            MenuItem::action("Push", Push),
            MenuItem::action("Pull", Pull),
            MenuItem::action("Fetch", Fetch),
            MenuItem::action("Remove…", RemoveRepository),
            MenuItem::separator(),
            MenuItem::action("View on GitHub", ViewOnGitHub),
            MenuItem::action(format!("Open in {shell}"), OpenInShell),
            MenuItem::action("Show in Finder", ShowInFinder),
            MenuItem::action(format!("Open in {editor}"), OpenInEditor),
            MenuItem::action("Open With…", OpenWith),
            MenuItem::separator(),
            MenuItem::action("Create Issue on GitHub", CreateIssue),
            MenuItem::separator(),
            MenuItem::action("New Worktree…", NewWorktree),
            MenuItem::separator(),
            MenuItem::action("Repository Settings…", RepositorySettings),
        ]),
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
        Menu::new("Window").items([
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
            MenuItem::action("Close Window", CloseWindow),
            MenuItem::separator(),
            MenuItem::action("Bring All to Front", BringAllToFront),
        ]),
        Menu::new("Help").items(help_items(show_release_notes)),
    ]);
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
