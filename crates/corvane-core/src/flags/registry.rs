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
    /// Summary-only History rows.
    COMPACT_COMMIT_ROWS = 140 "compact-commit-rows" {
        title: "Compact History rows",
        summary: "History rows are 30 px tall and show the commit summary only, without the avatar, \
                  author and time line (the commit's details pane still has them).",
        ghd_behaviour: "50 px rows with an avatar + \"author • time\" line under the summary.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15956)],
        code: &["crates/corvane-ui/src/history.rs"],
    },
    /// Inline code and autolinks in commit messages.
    COMMIT_MESSAGE_RICH_TEXT = 141 "commit-message-rich-text" {
        title: "Inline code and links in commit messages",
        summary: "The selected commit's title and description show `backtick` spans as inline code \
                  and link bare URLs and, in a GitHub repository, commit SHAs.",
        ghd_behaviour: "Backticks show literally; SHAs are plain text.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18104), Upstream::issue(7723)],
        code: &["crates/corvane-ui/src/selected_commit.rs", "crates/corvane-core/src/markdown.rs"],
    },
    /// History's first-parent toggle.
    HISTORY_FIRST_PARENT = 142 "history-first-parent" {
        title: "First-parent History",
        summary: "A filter button before \"Select Branch to Compare…\" switches the History list to \
                  first parents only (git log --first-parent), hiding the commits that came in \
                  through merges. The choice is remembered.",
        ghd_behaviour: "History always lists every commit reachable from HEAD.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21414)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/log.rs"],
    },

    /// The Rebase dialog starts on the default branch.
    REBASE_PRESELECTS_DEFAULT_BRANCH = 143 "rebase-preselects-default-branch" {
        title: "Rebase dialog preselects the default branch",
        summary: "Branch › Rebase Current Branch… opens with the default branch selected and its \
                  preview shown, unless the default branch is the current one.",
        ghd_behaviour: "The current branch shows as selected, and a branch has to be picked first.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17731)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },
    /// Squash message: keep only the target's.
    SQUASH_KEEP_TARGET_MESSAGE = 144 "squash-keep-target-message" {
        title: "Squash: use only the target commit's message",
        summary: "The squash message dialog has a \"Use only the target commit's message\" link \
                  that replaces the combined description with the summary and description of the \
                  commit squashed onto.",
        ghd_behaviour: "The combined message (target description plus every squashed commit's \
                        message) has to be trimmed by hand.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20507)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs", "crates/corvane-ui/src/dialogs/mod.rs"],
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
    /// History context menus: Copy Commit Title / Message / URL, Copy SHAs.
    HISTORY_COPY_ITEMS = 240 "history-copy-items" {
        title: "History: copy commit title, message, URL and SHAs",
        summary: "A commit's context menu adds Copy Commit Title, Copy Commit Message and (for \
                  GitHub repositories) Copy Commit URL next to Copy SHA; a multi-commit selection's \
                  menu adds Copy SHAs (newest first, one per line).",
        ghd_behaviour: "Copy SHA and Copy Tag only; nothing to copy for a multi-commit selection.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12547), Upstream::issue(20853), Upstream::issue(7518), Upstream::issue(9791), Upstream::issue(21061)],
        code: &["crates/corvane-ui/src/history.rs"],
    },
    /// The history list scrolls to the top when the branch changes.
    HISTORY_SCROLLS_TO_TOP_ON_BRANCH_CHANGE = 241 "history-scrolls-to-top-on-branch-change" {
        title: "History scrolls to the top on branch change",
        summary: "Switching branch (or repository) scrolls the History list back to the newest \
                  commit.",
        ghd_behaviour: "The list keeps its scroll offset, so another branch's history opens \
                        somewhere in the middle.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20849), Upstream::issue(6706)],
        code: &["crates/corvane-ui/src/history.rs"],
    },
    /// Revert without committing, for one commit or a multi-selection.
    REVERT_WITHOUT_COMMITTING = 242 "revert-without-committing" {
        title: "Revert without committing",
        summary: "A commit's context menu adds Revert Changes in Commit Without Committing, and a \
                  multi-commit selection's menu Revert Changes in N Commits Without Committing: \
                  git revert --no-commit, newest first, leaves the combined inverse staged in \
                  Changes. Needs a clean working directory; a conflict rolls everything back.",
        ghd_behaviour: "Reverts one commit at a time, each as its own commit.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17278), Upstream::issue(9967)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/history_ops.rs"],
    },
    /// History "Push Up to This Commit".
    PUSH_UP_TO_COMMIT = 243 "push-up-to-commit" {
        title: "Push up to a commit",
        summary: "A commit's context menu adds Push Up to This Commit, enabled on the current \
                  branch's unpushed commits: it pushes that commit (and the ones before it) to the \
                  upstream branch and keeps the newer ones local. Unpushed tags stay behind.",
        ghd_behaviour: "Push always pushes the whole branch.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19238), Upstream::issue(20670)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/remote.rs"],
    },
    /// Create a Tag's Message field.
    TAG_MESSAGE = 244 "tag-message" {
        title: "Tag message",
        summary: "Create a Tag has an optional Message field; the annotated tag carries it as \
                  typed.",
        ghd_behaviour: "Annotated tags always get an empty message.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12995), Upstream::issue(22890)],
        code: &["crates/corvane-ui/src/dialogs/history_dialogs.rs", "crates/corvane-git/src/history_ops.rs"],
    },
    /// Multi-select in a commit's file list.
    COMMIT_FILES_MULTI_SELECT = 245 "commit-files-multi-select" {
        title: "Multi-select a commit's files",
        summary: "The History file list selects several files with ⌘-click and ⇧-click; right-clicking \
                  the selection offers Copy File Paths and Copy Relative File Paths (one per line). \
                  The diff shows the last clicked file.",
        ghd_behaviour: "One file at a time.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15525), Upstream::issue(20467)],
        code: &["crates/corvane-ui/src/selected_commit.rs"],
    },
    /// The Pull button's tooltip lists the incoming commits.
    PULL_TOOLTIP_LISTS_COMMITS = 246 "pull-tooltip-lists-commits" {
        title: "Pull button lists incoming commits",
        summary: "Hovering Pull shows the summaries of the commits it would bring in (up to ten, \
                  newest first, then how many more).",
        ghd_behaviour: "No tooltip; only the behind count.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(6753)],
        code: &["crates/corvane-ui/src/toolbar.rs", "crates/corvane-core/src/dispatcher.rs"],
    },
    /// Undo Commit warns only when local changes touch the commit's files.
    UNDO_WARNS_ONLY_ON_OVERLAP = 247 "undo-warns-only-on-overlap" {
        title: "Undo Commit warns only about overlapping changes",
        summary: "The \"changes in progress\" warning before Undo Commit appears only when a file \
                  with local changes is one the commit touched.",
        ghd_behaviour: "Warns whenever there are any local changes, although undoing never loses \
                        changes to files the commit did not touch.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18388)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },
    /// History › Open with Default Program opens the file as of the commit.
    OPEN_HISTORICAL_FILE = 248 "open-historical-file" {
        title: "History opens the commit's version of a file",
        summary: "Open with Default Program in a commit's file list opens the file as it is in \
                  that commit (a read-only copy in the temporary directory), not the working copy.",
        ghd_behaviour: "Opens the file in the working directory, whatever its current state.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21117)],
        code: &["crates/corvane-ui/src/selected_commit.rs", "crates/corvane-core/src/integrations.rs"],
    },
    /// Copy path items for a commit's file that is gone from disk.
    COPY_PATH_OF_MISSING_FILE = 249 "copy-path-of-missing-file" {
        title: "Copy the path of a file missing on disk",
        summary: "In a commit's file list, a file that no longer exists on disk still offers Copy \
                  File Path and Copy Relative File Path under \"File Does Not Exist on Disk\".",
        ghd_behaviour: "Only the disabled \"File Does Not Exist on Disk\" item.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18349)],
        code: &["crates/corvane-ui/src/selected_commit.rs"],
    },
    /// Cherry-pick / squash / reorder without a branch explain themselves.
    NO_BRANCH_EXPLAINED = 250 "no-branch-explained" {
        title: "Explain why cherry-pick, squash and reorder cannot start",
        summary: "Cherry-pick, squash and reorder on a detached HEAD or during a rebase show an \
                  error saying so.",
        ghd_behaviour: "Nothing happens.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18715), Upstream::issue(20982)],
        code: &["crates/corvane-core/src/mco.rs"],
    },
    /// Line totals for a multi-commit selection.
    MULTI_COMMIT_LINE_TOTALS = 251 "multi-commit-line-totals" {
        title: "Line totals for a multi-commit selection",
        summary: "Selecting several commits shows the range's added and removed line totals next \
                  to \"Showing changes from N commits\".",
        ghd_behaviour: "Only the commit count.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17869)],
        code: &["crates/corvane-ui/src/selected_commit.rs"],
    },
    /// A mark on history rows whose commit has a description.
    COMMIT_BODY_INDICATOR = 252 "commit-body-indicator" {
        title: "Mark commits that have a description",
        summary: "A History row whose commit message has a description (extended body) shows a \
                  ⋯ mark after the summary.",
        ghd_behaviour: "Only the summary; the description is seen by selecting the commit.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20401)],
        code: &["crates/corvane-ui/src/history.rs"],
    },
    /// The commit details show the date and link the SHA.
    COMMIT_DETAILS_EXTRAS = 253 "commit-details-extras" {
        title: "Commit date and SHA link in the commit details",
        summary: "The selected commit's details show the author date and time (the relative time on \
                  hover), and in a GitHub repository the SHA opens the commit on GitHub.",
        ghd_behaviour: "Author, SHA and line counts only; the SHA is plain text.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20715), Upstream::issue(3785)],
        code: &["crates/corvane-ui/src/selected_commit.rs"],
    },
    /// Tag tooltips in History.
    TAGS_TOOLTIP = 254 "tags-tooltip" {
        title: "Tag tooltips in History",
        summary: "Hovering a commit's tag pill in the History list, or the tag list in the commit's \
                  details, shows every tag, one per line.",
        ghd_behaviour: "The pill shows the first tag and a sliver for the rest; a truncated tag list \
                        cannot be read.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(9687)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-ui/src/selected_commit.rs"],
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
    /// Checkout Commit on the branch tip.
    CHECKOUT_HEAD_COMMIT = 440 "checkout-head-commit" {
        title: "Checkout Commit on the latest commit",
        summary: "A commit's Checkout Commit is also enabled on the current branch's latest commit, \
                  detaching HEAD there.",
        ghd_behaviour: "Disabled on the latest commit, so HEAD cannot be detached at the branch tip.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22495)],
        code: &["crates/corvane-ui/src/history.rs"],
    },
    /// Undo Commit warns about the commit's tags.
    WARN_UNDO_TAGGED_COMMIT = 441 "warn-undo-tagged-commit" {
        title: "Warn before undoing a tagged commit",
        summary: "Undo Commit on a commit that has tags asks first: the tags would stay on a commit \
                  that is no longer on any branch.",
        ghd_behaviour: "Undoes silently; the tags keep pointing at the orphaned commit.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19844)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-ui/src/dialogs/history_dialogs.rs", "crates/corvane-ui/src/changes.rs"],
    },
    /// ⌘⏎ submits Create a Tag.
    CMD_ENTER_SUBMITS_CREATE_TAG = 442 "cmd-enter-submits-create-tag" {
        title: "⌘⏎ creates the tag",
        summary: "In Create a Tag, ⌘⏎ creates the tag from the Message field as well as from Name \
                  (where ⏎ does too).",
        ghd_behaviour: "Only ⏎ in the Name field submits; ⌘⏎ does nothing.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(22740)],
        code: &["crates/corvane-ui/src/dialogs/history_dialogs.rs"],
    },
    /// Revert one file of a commit.
    REVERT_FILE_IN_COMMIT = 443 "revert-file-in-commit" {
        title: "Revert one file of a commit",
        summary: "A commit's file menu adds Revert Changes to This File: that file's changes from the \
                  commit are undone in the working directory (nothing is committed). A working file \
                  that has changed since in the same places is left alone and the error says so.",
        ghd_behaviour: "Only whole commits can be reverted.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19207)],
        code: &["crates/corvane-ui/src/selected_commit.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/history_ops.rs"],
    },
    /// Tags in the compare list.
    COMPARE_TAGS = 444 "compare-tags" {
        title: "Compare to a tag",
        summary: "Typing in \"Select Branch to Compare…\" also lists the matching tags, under Tags \
                  after the branches; picking one compares the current branch with it as with a \
                  branch.",
        ghd_behaviour: "Branches only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15702)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/compare.rs", "crates/corvane-git/src/log.rs"],
    },
    /// Delete pushed tags, optionally from the remote.
    DELETE_PUSHED_TAGS = 445 "delete-pushed-tags" {
        title: "Delete pushed tags",
        summary: "A commit's Delete tag items are enabled for every tag, not only ones created here \
                  and not pushed yet. Those others ask first, with an unticked option to delete the \
                  tag from the remote too (git push <remote> --delete).",
        ghd_behaviour: "Only tags created in the app and not yet pushed can be deleted.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15858)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-ui/src/dialogs/history_dialogs.rs", "crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },
    /// Conflicts dialog › Resolve All ▾.
    RESOLVE_ALL_CONFLICTS = 446 "resolve-all-conflicts" {
        title: "Resolve all conflicts using one side",
        summary: "With two or more conflicted files, the conflicts dialog has a Resolve All menu \
                  that picks one branch's version for every file still in conflict. Like the \
                  per-file choice it is applied on Continue, and each file keeps its Undo.",
        ghd_behaviour: "One side is picked file by file.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12377), Upstream::issue(15829), Upstream::issue(22516)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs", "crates/corvane-core/src/mco.rs"],
    },
    /// Rebase with local changes: stash, then rebase.
    REBASE_STASH_AND_CONTINUE = 447 "rebase-stash-and-continue" {
        title: "Rebase: Stash Changes and Continue rebases",
        summary: "Rebasing with uncommitted changes first offers to stash them; Stash Changes and \
                  Continue stashes and then runs the rebase.",
        ghd_behaviour: "Stash Changes and Continue stashes the changes and stops; the rebase has to \
                        be started again.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21904)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-core/src/state.rs"],
    },
    /// Rebase / squash / reorder keep `#` message lines.
    REBASE_KEEPS_HASH_MESSAGES = 448 "rebase-keeps-hash-messages" {
        title: "Rebase keeps commit messages that start with #",
        summary: "Rebase, squash and reorder keep commit message lines starting with # (a \
                  \"#123 Fix\" summary, say) when a commit stopped on conflicts is continued, and \
                  git's conflict notes stay out of the message.",
        ghd_behaviour: "Continuing after a conflict drops every line starting with #; a summary \
                        starting with # leaves the message empty and the rebase fails.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(16444)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-git/src/rebase_ops.rs"],
    },
    /// Cherry-pick keeps the picked message after a conflict.
    CHERRY_PICK_KEEPS_MESSAGES = 449 "cherry-pick-keeps-messages" {
        title: "Cherry-pick keeps the commit message after a conflict",
        summary: "A cherry-picked commit that stopped on conflicts keeps its message as written \
                  when continued: lines starting with # stay, and git's \"Conflicts:\" note is \
                  not added.",
        ghd_behaviour: "Continuing can fail with \"Aborting commit due to empty commit message\"; \
                        lines starting with # are dropped.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21685)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-git/src/rebase_ops.rs"],
    },
    /// Squash and merge asks for the commit message.
    SQUASH_MERGE_MESSAGE = 450 "squash-merge-message" {
        title: "Squash and merge: commit message",
        summary: "The Squash and Merge dialog has summary and description fields above its \
                  button. With a summary, the squashed commit gets that message; left empty, \
                  git's \"Squashed commit of the following\" list as before.",
        ghd_behaviour: "The squashed commit always gets git's \"Squashed commit of the following\" \
                        list of the merged commits.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21718)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs", "crates/corvane-core/src/mco.rs", "crates/corvane-git/src/branch_ops.rs"],
    },
    /// Rebase onto `origin/main`.
    REBASE_ONTO_REMOTE_BRANCH = 451 "rebase-onto-remote-branch" {
        title: "Rebase onto a remote branch",
        summary: "The Rebase dialog's list ends with Remote Branches: the remote-tracking \
                  branches of local branches (origin/main next to main), so a branch can be \
                  rebased onto what was last fetched without checking out and pulling the local \
                  branch first.",
        ghd_behaviour: "A remote branch that has a local branch is not listed; only the local one \
                        can be picked.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13994)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs", "crates/corvane-ui/src/branch_list.rs"],
    },
    /// Conflicted file menu › Copy File Path.
    CONFLICT_MENU_COPY_PATHS = 452 "conflict-menu-copy-paths" {
        title: "Conflicts dialog: copy file paths",
        summary: "A conflicted file's ▾ menu in the conflicts dialog adds Copy File Path and Copy \
                  Relative File Path, as in the changes list.",
        ghd_behaviour: "Open with Default Program, Reveal in Finder and the resolution choices only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22399)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },
    /// Conflicts dialog names the stopped commit.
    CONFLICTS_SHOW_CURRENT_COMMIT = 453 "conflicts-show-current-commit" {
        title: "Conflicts dialog shows the stopped commit",
        summary: "While a rebase, squash or reorder waits on conflicts, the conflicts \
                  dialog starts with \"Commit N of M:\" and that commit's summary, as the \
                  progress dialog showed it.",
        ghd_behaviour: "The conflicts dialog does not say which commit is being applied.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18796)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },
    /// A rebase found in progress names its base branch.
    REBASE_BASE_NAME_RESOLVED = 454 "rebase-base-name-resolved" {
        title: "Rebase found in progress names its base branch",
        summary: "For a rebase that stopped on conflicts outside Corvane (or before a restart), the \
                  branch at the commit being rebased onto is looked up, so the conflicts dialog's \
                  choices read \"from main\" and the success banner names the base.",
        ghd_behaviour: "The base side is unnamed (\"Use the modified file\").",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(8113)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-git/src/rebase_ops.rs"],
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
}
