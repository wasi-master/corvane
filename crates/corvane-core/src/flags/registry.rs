//! The flag table. One entry per switchable deviation from GitHub Desktop or
//! Corvane-only extra; `docs/reference/deviations.md` names each flag next to
//! the behaviour it controls and `docs/reference/flags.md` is generated from
//! this file.
//!
//! Rules: the id's hundreds digit is the category block, ids are never
//! reused (move a deleted flag's id and slug to [`RETIRED`]), "on" means the
//! Corvane deviation is active, and every preset gets an explicit value.
//!
//! Every entry also sets `nature`: [`Nature::BugFix`] when GitHub Desktop's
//! behaviour is plainly wrong and nearly everyone wants the fix,
//! [`Nature::Feature`] for a new capability, option or look (when in doubt,
//! Feature). It is an attribute, not a category: the flag keeps its numbered
//! block. The Flags dialog hides bug fixes unless "Show bug fixes" is ticked;
//! presets and `CORVANE_FLAGS` apply to them all the same.

use super::{Availability, FlagDef, FlagId, Kind, Nature, SelectOption, Upstream, Value};

/// Emits `ids::NAME` constants and `REGISTRY` from one table, so a const and
/// its definition cannot drift apart.
macro_rules! registry {
    ($( $(#[$m:meta])* $name:ident = $id:literal $slug:literal { $($field:ident : $value:expr),* $(,)? } ),* $(,)?) => {
        /// `FlagId` constants, one per flag (`ids::COMMIT_TEMPLATES`).
        pub mod ids {
            use super::FlagId;
            $( $(#[$m])* pub const $name: FlagId = FlagId($id); )*
        }

        pub static REGISTRY: &[FlagDef] = &[
            $( FlagDef { id: FlagId($id), slug: $slug, $($field: $value,)* } ),*
        ];
    };
}

fn available() -> Availability {
    Availability::Available
}

fn packs_availability() -> Availability {
    if corvane_highlight::syntaxes::extended_bundled() {
        Availability::BuiltIn("This build compiles every grammar in.")
    } else {
        Availability::Available
    }
}

fn product_name(s: &str) -> Result<(), &'static str> {
    let s = s.trim();
    if s.is_empty() {
        Err("Enter a name")
    } else if s.chars().count() > 40 {
        Err("At most 40 characters")
    } else if s.contains(['\n', '\r']) {
        Err("One line only")
    } else {
        Ok(())
    }
}

/// `264-branch-name-prefix`: empty (off) or a ref-name-safe prefix.
fn branch_name_prefix(s: &str) -> Result<(), &'static str> {
    if s.chars().count() > 60 {
        Err("At most 60 characters")
    } else if s
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\\"'".contains(c))
    {
        Err("No spaces or ~ ^ : ? * [ \\ quotes")
    } else if s.starts_with(['.', '/', '-']) || s.contains("..") || s.contains("//") {
        Err("Not a valid start of a branch name")
    } else {
        Ok(())
    }
}

/// `357-clone-default-account`: logins separated by commas or spaces.
fn account_logins(s: &str) -> Result<(), &'static str> {
    if s.chars().count() > 200 {
        Err("At most 200 characters")
    } else if s
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || "-_, ".contains(c)))
    {
        Err("Logins separated by commas or spaces")
    } else {
        Ok(())
    }
}

const ON: Value = Value::Bool(true);
const OFF: Value = Value::Bool(false);

const QUICK_VIEW_WIDTHS: &[SelectOption] = &[
    SelectOption {
        value: "fixed-400",
        label: "Fixed 400 px",
    },
    SelectOption {
        value: "min-400",
        label: "At least 400 px",
    },
];

const SIGN_IN_FLOWS: &[SelectOption] = &[
    SelectOption {
        value: "device",
        label: "One-time code (device flow)",
    },
    SelectOption {
        value: "browser",
        label: "Browser (web flow)",
    },
];

const WIDTH_SAVES: &[SelectOption] = &[
    SelectOption {
        value: "drag-end",
        label: "When the drag ends",
    },
    SelectOption {
        value: "every-move",
        label: "On every pointer move",
    },
];

registry! {
    // ---- 100 Appearance ----

    /// Settings › Appearance › High Contrast and the macOS "Increase contrast" switch.
    HIGH_CONTRAST_THEME = 101 "high-contrast-theme" {
        title: "High Contrast theme",
        summary: "Settings › Appearance offers a High Contrast theme (GitHub Desktop's dark tokens in \
                  Primer's high-contrast palette), and the System theme switches to it while macOS's \
                  \"Increase contrast\" is on.",
        ghd_behaviour: "Light, Dark and System only; \"Increase contrast\" is ignored.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(4544)],
        code: &["crates/corvane/src/main.rs", "crates/corvane-ui/src/dialogs/preferences.rs"],
    },
    /// Chromium's smooth-scroll curve for mouse-wheel ticks.
    SMOOTH_WHEEL_SCROLLING = 102 "smooth-wheel-scrolling" {
        title: "Smooth wheel scrolling",
        summary: "Mouse-wheel ticks animate with Chromium's smooth-scroll curve.",
        ghd_behaviour: "Jumps 40 px per tick (Electron only animates when NSScrollAnimationEnabled is set).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/scrollbar.rs"],
    },
    /// The name the Welcome flow and the tutorial README call the app.
    PRODUCT_NAME = 103 "product-name" {
        title: "Product name in Welcome, blank slate and tutorial copy",
        summary: "The name the Welcome flow, the no-repositories blank slate and the tutorial \
                  README use for the app.",
        ghd_behaviour: "\"GitHub Desktop\".",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "Corvane", validate: product_name },
        corvane: Value::text("Corvane"), ghd: Value::text("GitHub Desktop"),
        familiar: Value::text("Corvane"), everything: Value::text("Corvane"),
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/welcome.rs", "crates/corvane-ui/src/no_repositories.rs", "crates/corvane-ui/src/tutorial_panel.rs", "crates/corvane-core/src/tutorial.rs"],
    },

    /// A hovered selected list row keeps its selection colour.
    SELECTION_KEEPS_COLOUR_ON_HOVER = 104 "selection-keeps-colour-on-hover" {
        title: "Selected rows keep their colour under the pointer",
        summary: "Hovering a selected row in a list (changed files, commit files, branches, \
                  repositories, pull requests, stash files, branch pickers) keeps the selection \
                  colour instead of swapping in the hover colour.",
        ghd_behaviour: "`.list-item:hover` outranks `.list-item.selected` in specificity, so a \
                        selected row in an unfocused list shows the hover colour and looks \
                        unselected while the pointer is on it.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/widgets.rs", "crates/corvane-ui/src/changes.rs", "crates/corvane-ui/src/branch_list.rs"],
    },
    /// Settings › Appearance › Syntax highlighting (tree-sitter).
    TREE_SITTER_HIGHLIGHTING = 105 "tree-sitter-highlighting" {
        title: "Tree-sitter syntax highlighting",
        summary: "Settings › Appearance offers Syntax highlighting: GitHub Desktop's highlighter, \
                  tree-sitter for the languages it does not highlight, or tree-sitter wherever \
                  there is a grammar. The grammars download as an optional component.",
        ghd_behaviour: "CodeMirror 5 modes only; files in other languages get no colours.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[
            Upstream::issue(22015),
            Upstream::issue(19038),
            Upstream::issue(21385),
            Upstream::issue(22663),
            Upstream::issue(21106),
            Upstream::issue(19311),
        ],
        code: &[
            "crates/corvane-highlight/src/treesitter/mod.rs",
            "crates/corvane-ui/src/dialogs/preferences.rs",
            "crates/corvane-ui/src/diff_view.rs",
            "crates/corvane-core/src/packs.rs",
        ],
    },

    // ---- 200 Repository ----

    /// `commit.template` prefills the commit description.
    COMMIT_TEMPLATES = 201 "commit-templates" {
        title: "Commit message templates",
        summary: "The repository's commit.template (comment lines stripped) prefills the description \
                  while the summary is empty, and comes back after every commit.",
        ghd_behaviour: "Ignores commit.template.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(8698)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-git/src/commit_template.rs"],
    },
    /// FSEvents-driven refresh.
    FS_WATCHER = 202 "fs-watcher" {
        title: "Filesystem watcher",
        summary: "Refreshes the repository when files under the worktree or .git change, not only on \
                  window focus and after Corvane's own actions.",
        ghd_behaviour: "Refreshes on window focus and after its own actions only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(22600), Upstream::issue(2790)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/watcher.rs"],
    },
    /// The watcher's debounce.
    FS_WATCHER_DEBOUNCE_MS = 203 "fs-watcher-debounce-ms" {
        title: "Filesystem watcher debounce",
        summary: "How long the watcher waits after the last change before refreshing.",
        ghd_behaviour: "No watcher.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 50, max: 5000, unit: Some("ms") },
        corvane: Value::Number(300), ghd: Value::Number(300),
        familiar: Value::Number(300), everything: Value::Number(300),
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/watcher.rs"],
    },
    /// The clone dialog's `owner/name` 404.
    CLONE_SHORTHAND_NOT_FOUND = 204 "clone-shorthand-not-found" {
        title: "Clone: unknown owner/name shows an error",
        summary: "When every account answers 404 for an owner/name shorthand in the clone dialog, \
                  \"We couldn't find that repository\" is shown instead of starting the clone.",
        ghd_behaviour: "Hands the bare alias to git, which fails after a moment.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/clone_info.rs"],
    },

    /// Add Local Repository checks the path while typing.
    ADD_LOCAL_VALIDATES_WHILE_TYPING = 205 "add-local-validates-while-typing" {
        title: "Add Local Repository checks the path as you type",
        summary: "The \"does not appear to be a Git repository\" / bare-repository warning follows \
                  the Local Path field as it changes, and Add Repository is disabled until the \
                  path is a repository.",
        ghd_behaviour: "The path is only checked when Add Repository is pressed; the warning then \
                        stays, stale, while the path is edited.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/dialogs/add_existing.rs"],
    },
    /// File › Import Repositories from GitHub Desktop….
    IMPORT_FROM_GITHUB_DESKTOP = 206 "import-from-github-desktop" {
        title: "Import repositories from GitHub Desktop",
        summary: "File › Import Repositories from GitHub Desktop… (and a button on the \
                  \"Let's get started!\" page when GitHub Desktop's data is on this Mac) reads \
                  GitHub Desktop's repository list and adds the repositories you pick, aliases \
                  included.",
        ghd_behaviour: "Not applicable: GitHub Desktop has nothing to import from.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-platform/src/ghd_import.rs", "crates/corvane-core/src/ghd_import.rs", "crates/corvane-ui/src/dialogs/import_github_desktop.rs"],
    },

    /// Create a Branch can start from any branch.
    CREATE_BRANCH_FROM_ANY_BRANCH = 255 "create-branch-from-any-branch" {
        title: "Create a branch from any branch",
        summary: "Create a Branch offers \"Other branch…\" next to the default and current \
                  branches, with a filterable list of every local and remote branch to start from.",
        ghd_behaviour: "Only the default branch or the current branch (and no choice at all while \
                        the default branch is checked out).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12459), Upstream::issue(20083)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },
    /// Create a Branch starts from the current branch while there are changes.
    CREATE_BRANCH_WITH_CHANGES_FROM_CURRENT = 256 "create-branch-with-changes-from-current" {
        title: "New branch with uncommitted changes starts from the current branch",
        summary: "While the working directory has uncommitted changes, Create a Branch preselects \
                  the current branch as the starting point, so the changes brought along apply \
                  to the code they were written against.",
        ghd_behaviour: "Always preselects the default branch; bringing the changes onto it can \
                        conflict or appear to lose work.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(9670)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },

    /// Other Branches sorted by last update.
    BRANCH_LIST_SORT_BY_DATE = 257 "branch-list-sort-by-date" {
        title: "Branch lists sort other branches by date",
        summary: "Other Branches in the branch list and the branch pickers (merge, rebase, \
                  compare, pull request base, new branch) are ordered by the tip commit's date, \
                  most recently updated first.",
        ghd_behaviour: "Sorted by name only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19903), Upstream::issue(21358), Upstream::issue(5155)],
        code: &["crates/corvane-ui/src/branch_list.rs"],
    },

    /// Delete Branch warns about unmerged commits and a stash.
    DELETE_BRANCH_WARNINGS = 258 "delete-branch-warnings" {
        title: "Delete Branch warns about unmerged commits and stashes",
        summary: "Delete Branch warns when the branch has commits that neither the default branch \
                  nor the branch's upstream contain, and when changes are stashed on it.",
        ghd_behaviour: "Only \"This action cannot be undone.\"",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(4214), Upstream::issue(13714)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/branch_ops.rs"],
    },

    /// Create / Rename Branch refuse `head` in any case.
    REJECT_HEAD_BRANCH_NAME = 259 "reject-head-branch-name" {
        title: "Branches can't be named \"head\"",
        summary: "Create a Branch and Rename Branch refuse \"head\" in any letter case, which on \
                  a case-insensitive file system names HEAD itself.",
        ghd_behaviour: "Creates the branch; HEAD ends up detached and the branch can't be published.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13638)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },

    /// The branch filter ignores an `owner:` prefix.
    BRANCH_FILTER_STRIPS_OWNER = 260 "branch-filter-strips-owner" {
        title: "Branch filter understands owner:branch",
        summary: "Pasting GitHub's owner:branch form of a branch name into the branch list's filter \
                  finds the branch (the owner: part is ignored).",
        ghd_behaviour: "Filters for the whole text, so the branch is not found.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(7424)],
        code: &["crates/corvane-ui/src/branch_list.rs"],
    },
    /// Branch › New Branch… prefills the branch filter's text.
    NEW_BRANCH_FROM_FILTER = 261 "new-branch-from-filter" {
        title: "New Branch shortcut uses the branch filter",
        summary: "Branch › New Branch… (⌘⇧N) while the branch list is open prefills the name with \
                  its filter text, like the list's New Branch button.",
        ghd_behaviour: "The shortcut always opens Create a Branch with an empty name.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5199)],
        code: &["crates/corvane/src/main.rs", "crates/corvane-ui/src/workspace.rs", "crates/corvane-ui/src/branch_list.rs"],
    },

    /// Branch rows show where the branch lives.
    BRANCH_LIST_LOCAL_REMOTE_ICONS = 262 "branch-list-local-remote-icons" {
        title: "Branch list icons for local-only and remote branches",
        summary: "In the branch list a branch with no upstream (only on this computer) shows a \
                  desktop icon and a branch that exists only on the remote shows a server icon; \
                  tracked local branches keep the branch icon.",
        ghd_behaviour: "Every branch shows the same branch icon.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17012), Upstream::issue(22019)],
        code: &["crates/corvane-ui/src/branch_list.rs"],
    },

    /// Branch list toggle: remote branches only.
    BRANCH_LIST_REMOTE_ONLY = 263 "branch-list-remote-only" {
        title: "Branch list can show only remote branches",
        summary: "A server button beside the branch list's filter narrows the list to the remote \
                  branches (including those checked out locally), in one Remote Branches group.",
        ghd_behaviour: "Remote branches are only listed, under Other Branches, when there is no \
                        local branch of the same name.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14134)],
        code: &["crates/corvane-ui/src/branch_list.rs"],
    },

    /// Create a Branch prefills a prefix.
    BRANCH_NAME_PREFIX = 264 "branch-name-prefix" {
        title: "Branch name prefix",
        summary: "Text Create a Branch puts in front of the suggested name (for example \
                  \"feature/\" or \"yourname/\"); empty for none.",
        ghd_behaviour: "No prefix.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "feature/", validate: branch_name_prefix },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), everything: Value::text(""),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14004)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },

    /// The branch button shows a running merge.
    MERGE_PROGRESS_IN_BRANCH_BUTTON = 265 "merge-progress-in-branch-button" {
        title: "Branch button shows a running merge",
        summary: "While a merge runs (Merge into…, Update from Default Branch) the toolbar's \
                  branch button spins and reads \"Merging <branch>\", as it does while \
                  switching branches.",
        ghd_behaviour: "The merge dialog closes and nothing shows until the merge finishes.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(6120), Upstream::issue(15996)],
        code: &["crates/corvane-ui/src/toolbar.rs"],
    },

    /// Confirm before switching branch from the branch list.
    CONFIRM_BRANCH_SWITCH = 266 "confirm-branch-switch" {
        title: "Confirm before switching branches",
        summary: "Clicking a branch in the branch list asks \"Switch to <branch>?\" before \
                  checking it out.",
        ghd_behaviour: "Checks the branch out at once.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20410)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },

    /// Branch context menu: Rebase Current Branch onto <branch>….
    BRANCH_MENU_REBASE_ONTO = 267 "branch-menu-rebase-onto" {
        title: "Rebase onto a branch from the branch list",
        summary: "A branch's context menu in the branch list offers \"Rebase Current Branch onto \
                  <branch>…\", which opens the rebase dialog with that branch selected.",
        ghd_behaviour: "Rebasing starts from Branch › Rebase Current Branch… only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21657)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-core/src/mco.rs", "crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },

    /// Switch Branch › Discard my changes.
    SWITCH_BRANCH_DISCARD = 268 "switch-branch-discard" {
        title: "Switch Branch can discard changes",
        summary: "The Switch Branch dialog (shown for uncommitted changes) offers a third choice, \
                  \"Discard my changes\": its \"Discard Changes and Switch\" button discards \
                  every change (new files go to the Trash) and then switches.",
        ghd_behaviour: "Leave the changes in a stash or bring them along only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11491)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// Clone a Repository's "Shallow clone" checkbox.
    SHALLOW_CLONE = 269 "shallow-clone" {
        title: "Shallow clone option",
        summary: "Clone a Repository shows a \"Shallow clone\" checkbox under the local path; \
                  ticked, only the latest commit of the default branch is fetched \
                  (git clone --depth 1).",
        ghd_behaviour: "Always clones the full history.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21880)],
        code: &["crates/corvane-ui/src/dialogs/clone_repository.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/ops.rs"],
    },

    // ---- 300 GitHub ----

    /// The quick view's "opened … by author" line.
    PR_QUICK_VIEW_OPENED_BY = 301 "pr-quick-view-opened-by" {
        title: "Pull request quick view: \"opened by\" line",
        summary: "The pull request hover card shows the list item's \"opened N ago by author\" line \
                  next to the #N badge.",
        ghd_behaviour: "The card shows only the badge, the title and the body.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/pull_request_list.rs"],
    },
    /// The quick view's width rule.
    PR_QUICK_VIEW_WIDTH = 302 "pr-quick-view-width" {
        title: "Pull request quick view width",
        summary: "The hover card's width: fixed at 400 px, or at least 400 px and growing with its \
                  content.",
        ghd_behaviour: "min-width: 400px.",
        nature: Nature::Feature,
        kind: Kind::Select { options: QUICK_VIEW_WIDTHS },
        corvane: Value::text("fixed-400"), ghd: Value::text("min-400"),
        familiar: Value::text("min-400"), everything: Value::text("fixed-400"),
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/pull_request_list.rs"],
    },
    /// Create Fork before a push git would refuse.
    FORK_BEFORE_PUSH = 303 "fork-before-push" {
        title: "Fork before pushing to a read-only repository",
        summary: "A push to a repository the account can only read opens the Create Fork dialog \
                  before git runs.",
        ghd_behaviour: "Runs the push and offers the fork after the authentication failure.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/remote.rs"],
    },
    /// Preview Pull Request's base survives Push Branch Commits.
    PUSH_BRANCH_COMMITS_KEEPS_BASE = 304 "push-branch-commits-keeps-base" {
        title: "Push Branch Commits keeps the chosen base",
        summary: "The base branch picked in Preview Pull Request survives the push that precedes \
                  the pull request.",
        ghd_behaviour: "Drops the chosen base and opens the compare page against the default branch.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/integrations.rs", "crates/corvane-ui/src/dialogs/push_branch_commits.rs"],
    },
    /// A failed Push Branch Commits keeps its error.
    PUSH_BRANCH_COMMITS_ERROR_STOPS = 305 "push-branch-commits-error-stops" {
        title: "Push Branch Commits stops on a failed push",
        summary: "A failed push leaves its error on screen.",
        ghd_behaviour: "Opens the compare page on GitHub anyway.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/integrations.rs", "crates/corvane-core/src/remote.rs"],
    },
    /// Dropping commits on a pull request explains why it could not start.
    CHERRY_PICK_PR_BRANCH_ERROR = 306 "cherry-pick-pr-branch-error" {
        title: "Cherry-pick onto a pull request explains failures",
        summary: "When commits are dropped on a pull request whose branch cannot be determined, \
                  the reason is shown.",
        ghd_behaviour: "Logs the reason and ends the operation silently.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/pull_requests.rs"],
    },
    /// Device flow or browser flow first.
    SIGN_IN_FLOW = 307 "sign-in-flow" {
        title: "Sign-in flow",
        summary: "How Sign in to GitHub.com starts: with a one-time code (device flow) or in the \
                  browser (web flow with PKCE, which GitHub may refuse without a bundled client \
                  secret). The other flow stays one link away.",
        ghd_behaviour: "Browser flow only.",
        nature: Nature::Feature,
        kind: Kind::Select { options: SIGN_IN_FLOWS },
        corvane: Value::text("device"), ghd: Value::text("browser"),
        familiar: Value::text("device"), everything: Value::text("device"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18749)],
        code: &["crates/corvane-ui/src/dialogs/sign_in.rs", "crates/corvane-ui/src/welcome.rs", "crates/corvane-core/src/flags/dispatch.rs"],
    },
    /// Check-run subscriptions idle out.
    CI_STATUS_IDLE_MINUTES = 308 "ci-status-idle-minutes" {
        title: "Check-run refresh idle timeout",
        summary: "A commit's check status stops refreshing this many minutes after nothing rendered \
                  it (0 keeps every status refreshing).",
        ghd_behaviour: "Subscribes on mount and unsubscribes on unmount, so nothing idles out.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 1440, unit: Some("min") },
        corvane: Value::Number(5), ghd: Value::Number(0),
        familiar: Value::Number(5), everything: Value::Number(5),
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/commit_status.rs"],
    },

    /// Clone over SSH by default.
    CLONE_PREFERS_SSH = 355 "clone-prefers-ssh" {
        title: "Clone over SSH",
        summary: "Clone a Repository clones repositories picked from the list and owner/name \
                  shorthands with their SSH URL (git@host:owner/name.git). An https:// URL typed \
                  on the URL tab is still cloned over HTTPS.",
        ghd_behaviour: "Clones over HTTPS unless an SSH URL is typed.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(19824)],
        code: &["crates/corvane-ui/src/dialogs/clone_repository.rs", "crates/corvane-core/src/clone_info.rs"],
    },

    /// The clone list's filter accepts repository URLs.
    CLONE_FILTER_ACCEPTS_URLS = 356 "clone-filter-accepts-urls" {
        title: "Clone: repository URLs in the list filter",
        summary: "A repository URL pasted into the repository filter of Clone a Repository (or \
                  the \"Let's get started!\" page) filters by its owner/name, so \
                  https://github.com/owner/name finds owner/name.",
        ghd_behaviour: "Fuzzy-matches the whole URL and finds no repository.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20942)],
        code: &["crates/corvane-ui/src/cloneable_repositories.rs"],
    },

    /// The clone account picker's default account.
    CLONE_DEFAULT_ACCOUNT = 357 "clone-default-account" {
        title: "Default account for cloning",
        summary: "With several accounts signed in, Clone a Repository (and the \"Let's get \
                  started!\" page) start on the first account whose login is in this list \
                  (comma-separated) instead of the first account signed in; empty for GitHub \
                  Desktop's order.",
        ghd_behaviour: "The account signed in first, every time the dialog opens.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "your-login", validate: account_logins },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), everything: Value::text(""),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21743)],
        code: &["crates/corvane-ui/src/cloneable_repositories.rs", "crates/corvane-ui/src/dialogs/clone_repository.rs", "crates/corvane-ui/src/no_repositories.rs"],
    },

    /// Clone paths mirror owner/name.
    CLONE_PATH_INCLUDES_OWNER = 358 "clone-path-includes-owner" {
        title: "Clone into an owner folder",
        summary: "The local path Clone a Repository suggests is <clone folder>/<owner>/<name> \
                  (for example GitHub/desktop/desktop), so repositories with the same name from \
                  different owners don't collide.",
        ghd_behaviour: "Suggests <clone folder>/<name>.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21293), Upstream::issue(5449)],
        code: &["crates/corvane-ui/src/dialogs/clone_repository.rs"],
    },

    // ---- 400 Window & menus ----

    /// Help › Show Release Notes.
    RELEASE_NOTES_MENU_ITEM = 401 "release-notes-menu-item" {
        title: "Help › Show Release Notes",
        summary: "The Help menu can open the release notes of the running version at any time.",
        ghd_behaviour: "Shows release notes only right after an update.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane/src/menus.rs"],
    },
    /// About's architecture suffix and Source code link.
    ABOUT_EXTRAS = 402 "about-extras" {
        title: "About: architecture and source link",
        summary: "The About dialog shows the CPU architecture after the version and a Source code link.",
        ghd_behaviour: "The version only, and a Terms and Conditions link.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/dialogs/app_dialogs.rs"],
    },
    /// The Move to Applications prompt's backdrop.
    MOVE_TO_APPLICATIONS_BACKDROP_DISMISS = 403 "move-to-applications-backdrop-dismiss" {
        title: "Move to Applications prompt closes on backdrop click",
        summary: "Clicking outside the \"Move to the Applications folder?\" prompt dismisses it.",
        ghd_behaviour: "The prompt only closes through its buttons (backdropDismissable=false).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/dialog.rs", "crates/corvane-ui/src/dialogs/move_to_applications_folder.rs"],
    },
    /// When a resized toolbar button's width is persisted.
    TOOLBAR_WIDTH_SAVE = 404 "toolbar-width-save" {
        title: "Toolbar button width is saved",
        summary: "When a resized toolbar button's width is written to the store.",
        ghd_behaviour: "localStorage on every pointer move.",
        nature: Nature::BugFix,
        kind: Kind::Select { options: WIDTH_SAVES },
        corvane: Value::text("drag-end"), ghd: Value::text("every-move"),
        familiar: Value::text("drag-end"), everything: Value::text("drag-end"),
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/toolbar.rs"],
    },

    // ---- 500 Settings & updates ----

    /// Settings › Advanced › Save crash reports locally.
    CRASH_REPORTS = 501 "crash-reports" {
        title: "Save crash reports locally",
        summary: "Settings › Advanced offers \"Save crash reports locally\": a panic hook writes \
                  ~/Library/Logs/Corvane/crashes/ and the next launch lists new reports. Nothing \
                  is uploaded.",
        ghd_behaviour: "No local crash reports (GHD's crash reporter uploads to GitHub instead).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/dialogs/preferences.rs", "crates/corvane-core/src/crash_reports.rs"],
    },
    /// Settings › Advanced › Optional components and pack loading.
    OPTIONAL_COMPONENTS = 502 "optional-components" {
        title: "Optional components",
        summary: "Settings › Advanced offers downloadable packs (the syntax-extended grammar \
                  collection), and installed packs load at launch.",
        ghd_behaviour: "Ships every grammar; no packs section.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: true, visible: true, availability: packs_availability,
        upstream: &[],
        code: &["crates/corvane-ui/src/dialogs/preferences.rs", "crates/corvane-core/src/packs.rs"],
    },
    /// Background update-check errors stay in the log.
    QUIET_BACKGROUND_UPDATE_ERRORS = 503 "quiet-background-update-errors" {
        title: "Quiet background update checks",
        summary: "Errors from the automatic update checks are only logged; Check for Updates in \
                  About still shows them.",
        ghd_behaviour: "Posts every update error.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/updater.rs"],
    },
    /// Untagged release-note items keep their heading's kind.
    RELEASE_NOTES_HEADING_KINDS = 504 "release-notes-heading-kinds" {
        title: "Release notes: untagged items keep their heading's kind",
        summary: "Release-note items without a [Kind] tag are classified by their ## heading, and \
                  the notes' leading paragraph is shown.",
        ghd_behaviour: "Drops untagged items and the leading paragraph.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/release_notes.rs"],
    },

    // ---- 600 Accessibility ----

    /// ⌘9 / ⌘8 announce the width after the step.
    RESIZABLE_ANNOUNCES_NEW_WIDTH = 601 "resizable-announces-new-width" {
        title: "Expand / Contract Active Resizable announces the new width",
        summary: "⌘9 / ⌘8 announce the percentage of the width after the step.",
        ghd_behaviour: "Reads the width before applying the step, so the announced number lags one step.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/active_resizable.rs"],
    },
    /// Repository Settings › Git Config always labels its email box.
    GIT_CONFIG_EMAIL_LABEL = 602 "git-config-email-label" {
        title: "Git Config's email box keeps its label",
        summary: "Repository Settings › Git Config shows \"Email\" above the email text box \
                  whenever it stands alone.",
        ghd_behaviour: "The label disappears whenever the email isn't one of the signed-in \
                        accounts' addresses (always, when signed out), although the code means to \
                        hide it only under the account-email dropdown's \"Other\".",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs"],
    },
}

/// Ids and slugs that once existed; never reused.
pub const RETIRED: &[(u16, &str)] = &[];

pub fn find(id: FlagId) -> Option<&'static FlagDef> {
    REGISTRY.iter().find(|def| def.id == id)
}

/// The definition of a registry id (a `FlagId` constant); unknown ids are a
/// registry bug and fall back to the first entry so callers stay total.
pub fn def(id: FlagId) -> &'static FlagDef {
    match find(id) {
        Some(def) => def,
        None => {
            debug_assert!(false, "unknown flag id {}", id.0);
            &REGISTRY[0]
        }
    }
}

pub fn by_slug(slug: &str) -> Option<&'static FlagDef> {
    REGISTRY.iter().find(|def| def.slug == slug)
}

/// Accepts `201-commit-templates`, `commit-templates`, `201` or `#201`.
pub fn lookup(key: &str) -> Option<&'static FlagDef> {
    let key = key.trim().trim_start_matches('#');
    REGISTRY
        .iter()
        .find(|def| def.slug == key || def.ident() == key || def.id.0.to_string() == key)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::flags::{Category, Preset};

    #[test]
    fn ids_are_unique_in_their_block_and_never_retired() {
        let mut seen = HashSet::new();
        for def in REGISTRY {
            assert!(seen.insert(def.id), "duplicate id {}", def.id.0);
            let category = Category::of_block(def.id.0 / 100)
                .unwrap_or_else(|| panic!("{} is outside every category block", def.id.0));
            assert!(
                (category.block() + 1..category.block() + 100).contains(&def.id.0),
                "{} must be within its block",
                def.id.0
            );
            assert!(
                !RETIRED
                    .iter()
                    .any(|(id, slug)| *id == def.id.0 || *slug == def.slug),
                "{} reuses a retired id or slug",
                def.ident()
            );
        }
        assert!(
            REGISTRY.windows(2).all(|w| w[0].id < w[1].id),
            "registry must be sorted by id"
        );
    }

    #[test]
    fn slugs_are_unique_kebab_case() {
        let mut seen = HashSet::new();
        for def in REGISTRY {
            assert!(seen.insert(def.slug), "duplicate slug {}", def.slug);
            let ok = !def.slug.is_empty()
                && def.slug.split('-').all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                });
            assert!(ok, "slug `{}` is not kebab-case", def.slug);
            assert!(
                !def.title.is_empty() && !def.summary.is_empty() && !def.ghd_behaviour.is_empty()
            );
        }
    }

    #[test]
    fn every_preset_value_fits_its_kind() {
        for def in REGISTRY {
            for preset in Preset::ALL {
                let value = def.value_for(preset);
                def.kind
                    .validate(value)
                    .unwrap_or_else(|err| panic!("{} {preset:?}: {err}", def.ident()));
            }
            if let Kind::Bool = def.kind {
                assert_eq!(
                    def.everything,
                    Value::Bool(true),
                    "{}: Everything is on",
                    def.ident()
                );
                assert_eq!(def.ghd, Value::Bool(false), "{}: GHD is off", def.ident());
            }
        }
    }

    #[test]
    fn lookup_accepts_every_key_form() {
        for key in [
            "201-commit-templates",
            "commit-templates",
            "201",
            "#201",
            " 201 ",
        ] {
            assert_eq!(
                lookup(key).map(|d| d.id),
                Some(ids::COMMIT_TEMPLATES),
                "{key}"
            );
        }
        assert!(lookup("201-something-else").is_none());
        assert!(lookup("999").is_none());
        assert_eq!(by_slug("fs-watcher").map(|d| d.id), Some(ids::FS_WATCHER));
        assert_eq!(def(ids::PRODUCT_NAME).ident(), "103-product-name");
        assert_eq!(ids::PRODUCT_NAME.to_string(), "103-product-name");
        assert_eq!(ids::PRODUCT_NAME.category(), Category::Appearance);
    }

    #[test]
    fn labels_for_selects() {
        let def = def(ids::PR_QUICK_VIEW_WIDTH);
        assert_eq!(def.label_for(&Value::text("min-400")), "At least 400 px");
        assert_eq!(def.label_for(&Value::text("other")), "other");
        assert_eq!(def.label_for(&Value::Bool(true)), "on");
    }

    #[test]
    fn branch_name_prefix_accepts_ref_safe_text() {
        for ok in ["", "feature/", "wasi-", "team/wasi/"] {
            assert!(branch_name_prefix(ok).is_ok(), "{ok}");
        }
        for bad in ["my feature/", "a:b", "/x", ".x", "a..b", "a//b", "x~"] {
            assert!(branch_name_prefix(bad).is_err(), "{bad}");
        }
    }
}
