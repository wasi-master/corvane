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

fn worktree_location(s: &str) -> Result<(), &'static str> {
    let s = s.trim();
    if s.is_empty() {
        Err("Enter a location, e.g. {clone-dir}")
    } else if s.contains(['\n', '\r']) {
        Err("One line only")
    } else if !s.starts_with(['/', '~', '{']) {
        Err("Start with /, ~ or {clone-dir}")
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

    /// Taller .gitignore and squash-message text areas.
    TALLER_TEXT_AREAS = 185 "taller-text-areas" {
        title: "Taller .gitignore and squash message boxes",
        summary: "Repository Settings › Ignored Files' .gitignore box is 260 px tall and the \
                  Squash dialog's description box shows 12 lines.",
        ghd_behaviour: "130 px for .gitignore, 6 lines for the squash description.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11715), Upstream::issue(13018)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs", "crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },

    /// The repository list's behind arrow in the success colour.
    REPOSITORY_LIST_BEHIND_ACCENT = 186 "repository-list-behind-accent" {
        title: "Repository list: green arrow for commits to pull",
        summary: "In the repository list, the down arrow (the branch is behind its upstream) is \
                  drawn in the success green instead of the badge text colour, except on the \
                  selected row.",
        ghd_behaviour: "Up and down arrows share the badge text colour.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15005)],
        code: &["crates/corvane-ui/src/repository_list.rs"],
    },

    /// The push / pull progress tooltip keeps one width.
    STEADY_PROGRESS_TOOLTIP = 187 "steady-progress-tooltip" {
        title: "Steady push / pull progress tooltip",
        summary: "While a push, pull or fetch runs, the push / pull button's progress tooltip is \
                  always 300 px wide instead of resizing with every progress line.",
        ghd_behaviour: "The tooltip fits its text, so it jumps in size as the progress text changes.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17429)],
        code: &["crates/corvane-ui/src/toolbar.rs", "crates/corvane-ui/src/widgets.rs"],
    },

    /// Blue / orange diff colours for red-green colour blindness.
    COLOUR_BLIND_DIFF = 188 "colour-blind-diff" {
        title: "Colour-blind friendly diff colours",
        summary: "Diffs show added lines in blue and deleted lines in orange (after Primer's \
                  protanopia / deuteranopia themes) in the Light and Dark themes.",
        ghd_behaviour: "Pale green and pale red, which are hard to tell apart with red-green \
                        colour blindness.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(6795)],
        code: &["crates/corvane-ui/src/theme/mod.rs", "crates/corvane/src/main.rs"],
    },

    /// A light title bar and toolbar in the Light theme.
    LIGHT_TOOLBAR = 189 "light-toolbar" {
        title: "Light title bar and toolbar in the Light theme",
        summary: "With the Light theme the title bar and the toolbar (repository, branch and \
                  push / pull buttons) use light greys and dark text.",
        ghd_behaviour: "The title bar and toolbar stay dark in every theme.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22123), Upstream::issue(22470)],
        code: &["crates/corvane-ui/src/theme/mod.rs", "crates/corvane-ui/src/title_bar.rs", "crates/corvane/src/main.rs"],
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

    /// "No local changes" offers Open in <Shell>.
    NO_CHANGES_OPEN_IN_SHELL = 285 "no-changes-open-in-shell" {
        title: "No local changes: Open in shell",
        summary: "The \"No local changes\" view adds an \"Open the repository in <Shell>\" \
                  suggestion after Show in Finder.",
        ghd_behaviour: "Editor, Finder and GitHub suggestions only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12453)],
        code: &["crates/corvane-ui/src/workspace.rs"],
    },

    /// Repository Settings › Remote shows the `upstream` remote.
    UPSTREAM_REMOTE_IN_SETTINGS = 286 "upstream-remote-in-settings" {
        title: "Repository Settings shows the upstream remote",
        summary: "Repository Settings › Remote shows the `upstream` remote's URL (read-only) \
                  under the primary remote's.",
        ghd_behaviour: "Only the primary remote.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(6877)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs"],
    },

    /// Repository Settings › Ignored Files offers the bundled templates.
    GITIGNORE_TEMPLATES = 287 "gitignore-templates" {
        title: "Repository Settings: .gitignore templates",
        summary: "Repository Settings › Ignored Files has an \"Add a template\" list (the \
                  Create a New Repository .gitignore templates) that fills an empty box or \
                  appends the template.",
        ghd_behaviour: "Templates only when creating a repository.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(2197)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs"],
    },

    /// Worktree rows show and match their path.
    WORKTREE_PATHS = 288 "worktree-paths" {
        title: "Worktree list shows and searches paths",
        summary: "Rows in the worktree list have a tooltip with the worktree's name and full \
                  path, and the filter also matches the path.",
        ghd_behaviour: "Only the folder name, truncated, and the filter matches only the name, \
                        so worktrees with the same folder name look alike.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22650), Upstream::issue(22946), Upstream::issue(22375)],
        code: &["crates/corvane-ui/src/worktree_list.rs"],
    },

    /// Default location for new worktrees.
    WORKTREE_LOCATION = 289 "worktree-location" {
        title: "Default worktree location",
        summary: "Where New Worktree puts worktrees by default: `{clone-dir}` is Settings' \
                  clone directory, `{repo}` the repository's name and a leading `~` the home \
                  folder (e.g. `~/code/worktrees/{repo}`).",
        ghd_behaviour: "Always the clone directory.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "{clone-dir}", validate: worktree_location },
        corvane: Value::text("{clone-dir}"), ghd: Value::text("{clone-dir}"),
        familiar: Value::text("{clone-dir}"), everything: Value::text("{clone-dir}"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22308)],
        code: &["crates/corvane-ui/src/worktree_list.rs", "crates/corvane-core/src/worktrees.rs"],
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
    /// Fork and pull-request remotes follow an SSH origin.
    FORK_REMOTES_KEEP_SSH = 385 "fork-remotes-keep-ssh" {
        title: "Fork remotes keep SSH",
        summary: "When the repository's remote is SSH, Create Fork's new origin, the upstream \
                  remote and the remote added to check out a pull request from a fork use SSH \
                  on the same host too.",
        ghd_behaviour: "Always uses the API's HTTPS clone URL, so SSH-only setups cannot fetch \
                        or push through those remotes.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(19074), Upstream::issue(9490)],
        code: &["crates/corvane-core/src/forks.rs", "crates/corvane-core/src/pull_requests.rs", "crates/corvane-ui/src/dialogs/fork_dialogs.rs"],
    },
    /// Every page of a commit's check runs.
    ALL_CHECK_RUN_PAGES = 386 "all-check-run-pages" {
        title: "Read every page of check runs",
        summary: "A commit's check runs are read page by page until all of them are in (up to \
                  1,000), so the status and the checks list count every run.",
        ghd_behaviour: "Reads the first 100 check runs only.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(18101)],
        code: &["crates/corvane-core/src/commit_status.rs", "crates/corvane-core/src/alive.rs"],
    },
    /// URL actions prefer the repository itself over a fork of it.
    EXACT_REPOSITORY_URL_FIRST = 387 "exact-repository-url-first" {
        title: "Open in Desktop prefers the repository over its forks",
        summary: "When an x-corvane://openRepo URL (Open with Desktop, a new branch from the web) \
                  names a repository that is added along with a fork of it, the repository \
                  itself is opened.",
        ghd_behaviour: "Opens whichever of them comes first in the list, often the fork.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21379)],
        code: &["crates/corvane-core/src/app_url.rs"],
    },
    /// Publish errors name the failed validation.
    API_ERROR_DETAILS = 388 "api-error-details" {
        title: "Publish errors say what GitHub rejected",
        summary: "When publishing a repository fails validation, the error adds GitHub's reasons \
                  (e.g. \"description is too long (maximum is 350 characters)\") to its message.",
        ghd_behaviour: "Shows only the top-level message (\"Repository creation failed.\"), or \
                        for an organization a hint to check its permissions.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19465)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-github/src/api.rs"],
    },
    /// No fork offers where the owner disabled forking.
    FORK_OFFER_RESPECTS_ALLOW_FORKING = 389 "fork-offer-respects-allow-forking" {
        title: "No fork offer when forking is disabled",
        summary: "A read-only repository whose owner disabled forking gets no \"create a fork\" \
                  suggestion in the commit form and no Create Fork dialog around a push.",
        ghd_behaviour: "Offers the fork anyway; creating it then fails.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22156)],
        code: &["crates/corvane-core/src/forks.rs", "crates/corvane-core/src/remote.rs", "crates/corvane-ui/src/changes.rs"],
    },
    /// No Re-run for read-only repositories.
    RERUN_NEEDS_PUSH_ACCESS = 390 "rerun-needs-push-access" {
        title: "Re-run checks needs push access",
        summary: "The check-run popover hides Re-run (and the per-job re-run) when the \
                  repository's permissions say the account can only read it.",
        ghd_behaviour: "Shows Re-run to everyone; for read-only accounts the request fails.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14061)],
        code: &["crates/corvane-ui/src/ci_check_popover.rs"],
    },
    /// Pull Requests tab without an account.
    PULL_REQUESTS_SIGNED_OUT = 391 "pull-requests-signed-out" {
        title: "Pull Requests tab asks to sign in",
        summary: "Without an account for the repository's host, the empty Pull Requests tab \
                  says \"Sign in to see pull requests\" with a sign-in link, and its refresh \
                  button is disabled.",
        ghd_behaviour: "Shows \"You're all set!\" and a refresh button that does nothing.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22365), Upstream::issue(5354)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-ui/src/pull_request_list.rs"],
    },
    /// Plain-HTTP Enterprise servers.
    ENTERPRISE_PLAIN_HTTP = 392 "enterprise-plain-http" {
        title: "Allow plain-HTTP Enterprise servers",
        summary: "An Enterprise address typed with http:// stays on plain HTTP, for servers \
                  without TLS. The token then crosses the network unencrypted; an address \
                  without a scheme still uses HTTPS.",
        ghd_behaviour: "Always connects over HTTPS (plain HTTP was removed in 3.4.7).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(20245)],
        code: &["crates/corvane-ui/src/dialogs/sign_in.rs", "crates/corvane-github/src/endpoint.rs"],
    },
    /// Pull requests whose fork was deleted.
    PULL_REQUESTS_FROM_DELETED_FORKS = 393 "pull-requests-from-deleted-forks" {
        title: "Pull requests from deleted forks",
        summary: "Open pull requests whose head repository was deleted stay in the Pull \
                  Requests list and check out from the base repository's pull/N/head into pr/N.",
        ghd_behaviour: "Leaves them out of the list.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14090)],
        code: &["crates/corvane-core/src/pull_requests.rs"],
    },
    /// Full refresh of the `#` issue cache.
    ISSUES_FULL_REFRESH_HOURS = 394 "issues-full-refresh-hours" {
        title: "Issue suggestions: full refresh interval",
        summary: "Every this many hours the # issue suggestions fetch all open issues again, so \
                  deleted and transferred issues drop out (0 never does).",
        ghd_behaviour: "Only fetches issues updated since the newest cached one, so deleted or \
                        transferred issues are suggested forever.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 720, unit: Some("h") },
        corvane: Value::Number(24), ghd: Value::Number(0),
        familiar: Value::Number(24), everything: Value::Number(24),
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(14124)],
        code: &["crates/corvane-core/src/autocomplete.rs"],
    },

    /// The check-run popover links to the pull request.
    CI_POPOVER_PULL_REQUEST_LINK = 395 "ci-popover-pull-request-link" {
        title: "Checks popover links to the pull request",
        summary: "The popover under the pull request badge ends its summary line with \
                  \"Open #N on GitHub\".",
        ghd_behaviour: "No way to open the pull request from the badge or its popover.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15418)],
        code: &["crates/corvane-ui/src/ci_check_popover.rs"],
    },

    /// Repository › View Upstream on GitHub.
    VIEW_UPSTREAM_ON_GITHUB = 396 "view-upstream-on-github" {
        title: "Repository › View Upstream on GitHub",
        summary: "The Repository menu adds \"View Upstream on GitHub\", which opens a fork's \
                  parent repository.",
        ghd_behaviour: "Only View on GitHub (the fork itself).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13533)],
        code: &["crates/corvane/src/menus.rs", "crates/corvane-core/src/integrations.rs"],
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
    /// The diff's "Open in <Editor> at Line N".
    DIFF_OPEN_IN_EDITOR_AT_LINE = 485 "diff-open-in-editor-at-line" {
        title: "Diff: Open in editor at a line",
        summary: "Right-clicking a line of a working-directory diff offers \"Open in <Editor> at \
                  Line N\" when the editor can jump to a line (VS Code and its forks, Sublime \
                  Text, Zed).",
        ghd_behaviour: "The diff's context menu has no editor item; Open in <Editor> opens the \
                        file at its top.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14476), Upstream::issue(20254)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-platform/src/editors.rs", "crates/corvane-core/src/integrations.rs"],
    },

    /// Window › Corvane shows the hidden main window.
    WINDOW_MENU_MAIN_WINDOW = 486 "window-menu-main-window" {
        title: "Window menu lists the main window",
        summary: "The Window menu ends with \"Corvane\", which shows the main window again after \
                  ⌘W or the close button hid it.",
        ghd_behaviour: "The closed window is not listed; only the Dock icon brings it back.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17647)],
        code: &["crates/corvane/src/menus.rs", "crates/corvane/src/main.rs"],
    },

    /// `--hidden` launches with the window hidden.
    LAUNCH_HIDDEN = 487 "launch-hidden" {
        title: "Launch hidden with --hidden",
        summary: "Started with `--hidden` (`open -a Corvane --args --hidden`, e.g. from a login \
                  script), Corvane keeps its window hidden until the Dock icon or Window menu \
                  brings it back.",
        ghd_behaviour: "Always shows the window at launch.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(18925)],
        code: &["crates/corvane/src/main.rs"],
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
    /// Editors GitHub Desktop does not detect.
    EXTRA_EDITORS = 585 "extra-editors" {
        title: "Detect more external editors",
        summary: "Settings › Integrations and Open in … also find editors GitHub Desktop 3.6.6 \
                  does not know: Antigravity.",
        ghd_behaviour: "Only its own editor table.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22922)],
        code: &["crates/corvane-platform/src/editors.rs", "crates/corvane-core/src/integrations.rs", "crates/corvane-core/src/flags/dispatch.rs"],
    },
    /// A name for the custom editor.
    CUSTOM_EDITOR_NAME = 586 "custom-editor-name" {
        title: "Custom editor name",
        summary: "Settings › Integrations › Configure Custom Editor… has a Name box; menus then \
                  say \"Open in <name>\" instead of \"Open in Custom Editor\".",
        ghd_behaviour: "Always \"Custom Editor\".",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21376)],
        code: &["crates/corvane-ui/src/dialogs/preferences.rs", "crates/corvane-core/src/state.rs"],
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
