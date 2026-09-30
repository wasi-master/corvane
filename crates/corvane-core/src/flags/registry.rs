//! The flag table. One entry per switchable deviation from GitHub Desktop or
//! Corvane-only extra; `.docs/deviations.md` names each flag next to
//! the behaviour it controls and `.docs/flags.md` is generated from
//! this file.
//!
//! Rules: the id's hundreds digit is the category block, ids are never
//! reused (move a deleted flag's id and slug to [`RETIRED`]), "on" means the
//! Corvane deviation is active, and every preset gets an explicit value.
//! `max` is `corvane` plus the extras most people would want; an extra that
//! is a matter of taste or suits only some workflows (colour-blind colours,
//! compact rows, another list order, an extra confirmation) keeps the
//! Corvane value there too.
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

/// `845-branch-name-prefix`: empty (off) or a ref-name-safe prefix.
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

/// `228-clone-default-account`: logins separated by commas or spaces.
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

fn hide_globs(s: &str) -> Result<(), &'static str> {
    if s.chars().count() > 1000 {
        Err("At most 1000 characters")
    } else {
        Ok(())
    }
}

fn app_name(s: &str) -> Result<(), &'static str> {
    if s.chars().count() > 500 {
        Err("At most 500 characters")
    } else if s.contains(['\n', '\r']) {
        Err("One line only")
    } else {
        Ok(())
    }
}

/// A comma-separated list of GitHub logins (`229-hidden-clone-owners`).
fn owner_list(s: &str) -> Result<(), &'static str> {
    if s.contains(['\n', '\r']) {
        Err("One line only")
    } else if s.chars().count() > 500 {
        Err("At most 500 characters")
    } else if s
        .split(',')
        .map(str::trim)
        .any(|o| o.contains(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')))
    {
        Err("Logins separated by commas")
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

const BACKGROUND_FETCHES: &[SelectOption] = &[
    SelectOption {
        value: "off",
        label: "Off",
    },
    SelectOption {
        value: "github",
        label: "GitHub repositories",
    },
    SelectOption {
        value: "any",
        label: "Any repository with a remote",
    },
];

const SIGN_IN_FLOWS: &[SelectOption] = &[
    SelectOption {
        value: "auto",
        label: "Browser when this build has a client secret",
    },
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

const CHANGES_SORT_ORDERS: &[SelectOption] = &[
    SelectOption {
        value: "path",
        label: "Path",
    },
    SelectOption {
        value: "status",
        label: "Status, then path",
    },
    SelectOption {
        value: "name",
        label: "File name",
    },
];
const CHANGES_FILTER_MATCHES: &[SelectOption] = &[
    SelectOption {
        value: "fuzzy",
        label: "Fuzzy",
    },
    SelectOption {
        value: "substring",
        label: "Contains the text",
    },
    SelectOption {
        value: "suffix",
        label: "Ends with the text",
    },
    SelectOption {
        value: "exact",
        label: "Exact path or file name",
    },
];
const IMAGE_DIFF_BACKGROUNDS: &[SelectOption] = &[
    SelectOption {
        value: "light",
        label: "Light checkerboard",
    },
    SelectOption {
        value: "dark",
        label: "Dark checkerboard",
    },
    SelectOption {
        value: "theme",
        label: "Follow the app theme",
    },
];
const IMAGE_DIFF_ALIGNMENTS: &[SelectOption] = &[
    SelectOption {
        value: "centre",
        label: "Centred",
    },
    SelectOption {
        value: "top-left",
        label: "Top left corners together",
    },
];

const IGNORE_SUBMODULE_MODES: &[SelectOption] = &[
    SelectOption {
        value: "configured",
        label: "As configured (submodule.<name>.ignore)",
    },
    SelectOption {
        value: "dirty",
        label: "Changes inside submodules",
    },
    SelectOption {
        value: "all",
        label: "All submodule changes",
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
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/scrollbar.rs"],
    },

    /// The name the Welcome flow and the tutorial README call the app.
    PRODUCT_NAME = 103 "product-name" {
        title: "Product name in Welcome, blank slate, tutorial, Move to Applications and submodule copy",
        summary: "The name the Welcome flow, the no-repositories blank slate, the tutorial \
                  README and the tutorial repository's GitHub description, the Move to \
                  Applications dialog and the submodule diff's \"Open this submodule\" card \
                  use for the app.",
        ghd_behaviour: "\"GitHub Desktop\".",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "Corvane", validate: product_name },
        corvane: Value::text("Corvane"), ghd: Value::text("GitHub Desktop"),
        familiar: Value::text("Corvane"), max: Value::text("Corvane"),
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/welcome.rs", "crates/corvane-ui/src/no_repositories.rs", "crates/corvane-ui/src/tutorial_panel.rs", "crates/corvane-core/src/tutorial.rs", "crates/corvane-ui/src/dialogs/move_to_applications_folder.rs", "crates/corvane-ui/src/diff_view.rs"],
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
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
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
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
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

    /// Relative dates in weeks and calendar months.
    CALENDAR_RELATIVE_DATES = 106 "calendar-relative-dates" {
        title: "Relative dates in weeks and calendar months",
        summary: "Past a week, relative dates count weeks (\"4 weeks ago\") until two calendar \
                  months have passed, then calendar months and years.",
        ghd_behaviour: "Days until 30, then days ÷ 30 rounded as months: a commit on the 1st is \
                        \"last month\" on the 31st.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20830), Upstream::issue(21903)],
        code: &["crates/corvane-ui/src/relative_time.rs", "crates/corvane/src/main.rs"],
    },

    /// Taller .gitignore and squash-message text areas.
    TALLER_TEXT_AREAS = 107 "taller-text-areas" {
        title: "Taller .gitignore and squash message boxes",
        summary: "Repository Settings › Ignored Files' .gitignore box is 260 px tall and the \
                  Squash dialog's description box shows 12 lines.",
        ghd_behaviour: "130 px for .gitignore, 6 lines for the squash description.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11715), Upstream::issue(13018)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs", "crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },

    /// Blue / orange diff colours for red-green colour blindness.
    COLOUR_BLIND_DIFF = 108 "colour-blind-diff" {
        title: "Colour-blind friendly diff colours",
        summary: "Diffs show added lines in blue and deleted lines in orange (after Primer's \
                  protanopia / deuteranopia themes) in the Light and Dark themes.",
        ghd_behaviour: "Pale green and pale red, which are hard to tell apart with red-green \
                        colour blindness.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(6795)],
        code: &["crates/corvane-ui/src/theme/mod.rs", "crates/corvane/src/main.rs"],
    },

    /// A light title bar and toolbar in the Light theme.
    LIGHT_TOOLBAR = 109 "light-toolbar" {
        title: "Light title bar and toolbar in the Light theme",
        summary: "With the Light theme the title bar and the toolbar (repository, branch and \
                  push / pull buttons) use light greys and dark text.",
        ghd_behaviour: "The title bar and toolbar stay dark in every theme.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22123), Upstream::issue(22470)],
        code: &["crates/corvane-ui/src/theme/mod.rs", "crates/corvane-ui/src/title_bar.rs", "crates/corvane/src/main.rs"],
    },
    /// Typing hides hover highlights and tooltips until the pointer moves.
    KEYBOARD_HIDES_HOVER = 110 "keyboard-hides-hover" {
        title: "Typing hides hover highlights",
        summary: "A key press clears the hover highlight and tooltip under a resting pointer \
                  until the pointer moves, so keyboard navigation isn't shadowed by the row \
                  under the mouse.",
        ghd_behaviour: "`:hover` and open tooltips stay on the element the pointer last moved \
                        over while typing; an element that appears under the resting pointer \
                        isn't hovered until it moves.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/widgets.rs", "vendor/gpui-pre/src/window.rs", "vendor/gpui-pre/src/elements/div.rs"],
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(22600), Upstream::issue(2790)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/watcher.rs"],
    },


    /// The clone dialog's `owner/name` 404.
    CLONE_SHORTHAND_NOT_FOUND = 204 "clone-shorthand-not-found" {
        title: "Clone: unknown owner/name shows an error",
        summary: "When every account answers 404 for an owner/name shorthand in the clone dialog, \
                  \"We couldn't find that repository\" is shown instead of starting the clone.",
        ghd_behaviour: "Hands the bare alias to git, which fails after a moment.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
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
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-platform/src/ghd_import.rs", "crates/corvane-core/src/ghd_import.rs", "crates/corvane-ui/src/dialogs/import_github_desktop.rs"],
    },

    /// The repository list filters to repositories with changes or commits to push / pull.
    REPOSITORY_STATUS_FILTER = 207 "repository-status-filter" {
        title: "Repository list status filters",
        summary: "A filter button next to the repository list's filter box shows only \
                  repositories with uncommitted changes and / or commits to push or pull (the \
                  rows' indicators); the Recent group is left out while one is on.",
        ghd_behaviour: "The list filters by name only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18322), Upstream::issue(22693)],
        code: &["crates/corvane-ui/src/repository_list.rs"],
    },

    /// The repository list filters to forks or to the rest.
    REPOSITORY_FORK_FILTER = 208 "repository-fork-filter" {
        title: "Repository list fork filter",
        summary: "The repository list's filter button (added by this flag if 110 is off) offers \
                  Forks and Not forks, from the GitHub repository's fork flag.",
        ghd_behaviour: "The list filters by name only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15655)],
        code: &["crates/corvane-ui/src/repository_list.rs"],
    },

    /// How many recent repositories the repository list shows.
    RECENT_REPOSITORIES_COUNT = 209 "recent-repositories-count" {
        title: "Recent repositories shown",
        summary: "How many recently opened repositories the repository list shows in its Recent \
                  group (0 hides the group).",
        ghd_behaviour: "Always 3.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 20, unit: None },
        corvane: Value::Number(3), ghd: Value::Number(3),
        familiar: Value::Number(3), max: Value::Number(5),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15244), Upstream::issue(19828)],
        code: &["crates/corvane-ui/src/repository_list.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// Opening the repository list selects its remembered filter text.
    REPOSITORY_FILTER_SELECTS_TEXT = 210 "repository-filter-selects-text" {
        title: "Repository filter text is selected on open",
        summary: "Opening the repository list (⌘T or the toolbar button) selects the filter text \
                  it remembers, so typing replaces it.",
        ghd_behaviour: "The caret lands after the old text, which has to be deleted first.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(2652)],
        code: &["crates/corvane-ui/src/repository_list.rs"],
    },

    /// `/pattern/` in the repository filter is a regular expression.
    REGEX_REPOSITORY_FILTER = 211 "regex-repository-filter" {
        title: "Regular expressions in the repository filter",
        summary: "Typing /pattern/ in the repository list's filter matches names against a \
                  case-insensitive regular expression; any other text (or an invalid pattern) \
                  filters as before.",
        ghd_behaviour: "Plain text matching only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(20745)],
        code: &["crates/corvane-ui/src/repository_list.rs", "crates/corvane-core/src/filter.rs"],
    },

    /// Filtering the repository list shows one ranked list.
    FLAT_REPOSITORY_RESULTS = 212 "flat-repository-results" {
        title: "Flat repository filter results",
        summary: "While text is typed in the repository list's filter, the matches form one list \
                  without owner groups, the best match (closest to the start of a word, fewest \
                  extra characters) first.",
        ghd_behaviour: "Matches stay in their owner groups, so the best match can sit far down.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(4860)],
        code: &["crates/corvane-ui/src/repository_list.rs"],
    },

    /// Same-named repositories in a group show the parent folders that differ.
    DUPLICATE_NAMES_SHOW_PATH = 213 "duplicate-names-show-path" {
        title: "Same-named repositories show their folder",
        summary: "When repositories in one group of the repository list share a name, each row \
                  adds, dimmed, the parent folders that tell them apart (fork-a beside fork-b).",
        ghd_behaviour: "Identical rows; only the tooltip's path tells them apart.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15937)],
        code: &["crates/corvane-ui/src/repository_list.rs"],
    },

    /// Repository list rows name the checked-out branch.
    REPOSITORY_LIST_BRANCH = 214 "repository-list-branch" {
        title: "Branch names in the repository list",
        summary: "Each repository list row shows, dimmed after the name, the branch checked out \
                  in that repository (from the background indicator refresh).",
        ghd_behaviour: "Only the name and the change / ahead-behind indicators.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(8158)],
        code: &["crates/corvane-ui/src/repository_list.rs", "crates/corvane-core/src/remote.rs"],
    },

    /// The repository list's behind arrow in the success colour.
    REPOSITORY_LIST_BEHIND_ACCENT = 215 "repository-list-behind-accent" {
        title: "Repository list: green arrow for commits to pull",
        summary: "In the repository list, the down arrow (the branch is behind its upstream) is \
                  drawn in the success green instead of the badge text colour, except on the \
                  selected row.",
        ghd_behaviour: "Up and down arrows share the badge text colour.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15005)],
        code: &["crates/corvane-ui/src/repository_list.rs"],
    },

    /// A missing repository's menu removes every missing repository.
    REMOVE_ALL_MISSING_REPOSITORIES = 216 "remove-all-missing-repositories" {
        title: "Remove all missing repositories",
        summary: "The context menu of a repository Corvane cannot find offers \"Remove All N \
                  Missing Repositories\" when several are missing; like removing one missing \
                  repository it only takes them off the list, without confirmation.",
        ghd_behaviour: "Missing repositories are removed one by one.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21151)],
        code: &["crates/corvane-ui/src/repository_list.rs"],
    },

    /// Repository list indicators refresh promptly.
    PROMPT_INDICATOR_REFRESH = 217 "prompt-indicator-refresh" {
        title: "Prompt repository indicator refresh",
        summary: "The repository list's changes and ahead/behind indicators are refreshed right \
                  after launch and whenever the repository list opens (at most once a minute), \
                  not only every 15 minutes.",
        ghd_behaviour: "The indicator updater starts on its delayed 15-minute cadence, so the list \
                        can show stale indicators after launch.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(22154)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// Switching repositories closes dialogs bound to the previous one.
    CLOSE_DIALOGS_ON_REPOSITORY_SWITCH = 218 "close-dialogs-on-repository-switch" {
        title: "Switching repositories closes the previous repository's dialog",
        summary: "When another repository is selected (e.g. from a link or the command line \
                  tool) a dialog for the previous repository closes, except conflict and \
                  credential prompts of an operation in progress.",
        ghd_behaviour: "The dialog stays open over the newly selected repository.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(9847)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/state.rs"],
    },

    /// Create Repository warns before replacing an existing README.md.
    README_OVERWRITE_WARNING = 219 "readme-overwrite-warning" {
        title: "Create Repository warns about an existing README",
        summary: "With \"Initialize this repository with a README\" ticked and a README.md already \
                  in the folder, the Create a New Repository dialog warns that its content will be \
                  replaced.",
        ghd_behaviour: "The warning only exists in beta builds; release builds silently overwrite \
                        the README.md.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22471)],
        code: &["crates/corvane-ui/src/dialogs/create_repository.rs"],
    },

    /// Create a New Repository in the chosen folder itself.
    CREATE_REPOSITORY_IN_FOLDER = 220 "create-repository-in-folder" {
        title: "Create a repository in an existing folder",
        summary: "Create a New Repository shows \"Create the repository in this folder (no \
                  subfolder)\" under the local path: ticked, the Local Path folder itself becomes \
                  the repository, and a README.md, .gitignore or LICENSE already there is kept.",
        ghd_behaviour: "Always creates a <name> subfolder of the local path.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11413)],
        code: &["crates/corvane-ui/src/dialogs/create_repository.rs", "crates/corvane-git/src/ops.rs"],
    },

    /// Repository › Add License….
    ADD_LICENSE = 221 "add-license" {
        title: "Add a license to a repository",
        summary: "Repository › Add License… writes one of the license templates Create a New \
                  Repository offers to LICENSE in the current repository (filled in with your \
                  Git name and the year). An existing license file is never replaced.",
        ghd_behaviour: "Licenses can only be added when creating a repository.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12222)],
        code: &["crates/corvane/src/menus.rs", "crates/corvane-ui/src/dialogs/add_license.rs", "crates/corvane-core/src/templates.rs"],
    },

    /// Add Local Repository › Choose… picks several folders.
    ADD_LOCAL_MULTIPLE = 222 "add-local-multiple" {
        title: "Add several local repositories at once",
        summary: "Add Local Repository's Choose… can select several folders; picking more than \
                  one adds every one that is a Git repository and lists the others.",
        ghd_behaviour: "One folder at a time.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(2978)],
        code: &["crates/corvane-ui/src/dialogs/add_existing.rs"],
    },

    /// Add Local Repository autocompletes folders.
    ADD_LOCAL_PATH_COMPLETION = 223 "add-local-path-completion" {
        title: "Folder completion in Add Local Repository",
        summary: "Typing a path (/… or ~/…) in Add Local Repository's Local Path lists the \
                  matching folders; ↑/↓ pick one, Enter or Tab completes it, Esc closes the list.",
        ghd_behaviour: "A plain text box.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18303)],
        code: &["crates/corvane-ui/src/dialogs/add_existing.rs", "crates/corvane-ui/src/autocompletion.rs", "crates/corvane-core/src/autocomplete.rs"],
    },

    /// An Alias field in New / Add / Clone.
    ALIAS_WHEN_ADDING = 224 "alias-when-adding" {
        title: "Alias field when adding a repository",
        summary: "Create a New Repository, Add Local Repository and Clone a Repository have an \
                  optional Alias field; the repository shows under that name in the list once \
                  it is added.",
        ghd_behaviour: "An alias can only be set afterwards (Create Alias in the repository \
                        list).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22505)],
        code: &["crates/corvane-ui/src/dialogs/add_existing.rs", "crates/corvane-ui/src/dialogs/create_repository.rs", "crates/corvane-ui/src/dialogs/clone_repository.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// Add › Clone Repository… carries the repository list's filter text.
    CLONE_PREFILLS_FILTER = 225 "clone-prefills-filter" {
        title: "Clone dialog takes the repository filter",
        summary: "Add › Clone Repository… in the repository list opens the clone dialog with the \
                  list's filter text in its GitHub tabs' filter box, so a repository that is not \
                  cloned yet can be found without typing its name again.",
        ghd_behaviour: "The clone dialog's filter starts empty.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20685)],
        code: &[
            "crates/corvane-ui/src/repository_list.rs",
            "crates/corvane-ui/src/dialogs/clone_repository.rs",
        ],
    },

    /// Clone over SSH by default.
    CLONE_PREFERS_SSH = 226 "clone-prefers-ssh" {
        title: "Clone over SSH",
        summary: "Clone a Repository clones repositories picked from the list and owner/name \
                  shorthands with their SSH URL (git@host:owner/name.git). An https:// URL typed \
                  on the URL tab is still cloned over HTTPS.",
        ghd_behaviour: "Clones over HTTPS unless an SSH URL is typed.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(19824)],
        code: &["crates/corvane-ui/src/dialogs/clone_repository.rs", "crates/corvane-core/src/clone_info.rs"],
    },

    /// The clone list's filter accepts repository URLs.
    CLONE_FILTER_ACCEPTS_URLS = 227 "clone-filter-accepts-urls" {
        title: "Clone: repository URLs in the list filter",
        summary: "A repository URL pasted into the repository filter of Clone a Repository (or \
                  the \"Let's get started!\" page) filters by its owner/name, so \
                  https://github.com/owner/name finds owner/name.",
        ghd_behaviour: "Fuzzy-matches the whole URL and finds no repository.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20942)],
        code: &["crates/corvane-ui/src/cloneable_repositories.rs"],
    },

    /// The clone account picker's default account.
    CLONE_DEFAULT_ACCOUNT = 228 "clone-default-account" {
        title: "Default account for cloning",
        summary: "With several accounts signed in, Clone a Repository (and the \"Let's get \
                  started!\" page) start on the first account whose login is in this list \
                  (comma-separated) instead of the first account signed in; empty for GitHub \
                  Desktop's order.",
        ghd_behaviour: "The account signed in first, every time the dialog opens.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "your-login", validate: account_logins },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), max: Value::text(""),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21743)],
        code: &["crates/corvane-ui/src/cloneable_repositories.rs", "crates/corvane-ui/src/dialogs/clone_repository.rs", "crates/corvane-ui/src/no_repositories.rs"],
    },

    /// Owners whose repositories the clone lists leave out.
    HIDDEN_CLONE_OWNERS = 229 "hidden-clone-owners" {
        title: "Owners hidden from the clone list",
        summary: "GitHub users or organizations (comma-separated logins) whose repositories the \
                  clone dialog and the blank slate's repository list leave out.",
        ghd_behaviour: "Every repository the account can access is listed.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "org-a, org-b", validate: owner_list },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), max: Value::text(""),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11908)],
        code: &[
            "crates/corvane-ui/src/cloneable_repositories.rs",
            "crates/corvane-ui/src/dialogs/clone_repository.rs",
            "crates/corvane-ui/src/no_repositories.rs",
        ],
    },

    /// Clone paths mirror owner/name.
    CLONE_PATH_INCLUDES_OWNER = 230 "clone-path-includes-owner" {
        title: "Clone into an owner folder",
        summary: "The local path Clone a Repository suggests is <clone folder>/<owner>/<name> \
                  (for example GitHub/desktop/desktop), so repositories with the same name from \
                  different owners don't collide.",
        ghd_behaviour: "Suggests <clone folder>/<name>.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21293), Upstream::issue(5449)],
        code: &["crates/corvane-ui/src/dialogs/clone_repository.rs"],
    },

    /// Clone offers to add a repository already at the local path.
    CLONE_OFFER_ADD_EXISTING = 231 "clone-offer-add-existing" {
        title: "Clone: add a repository already at the destination",
        summary: "When Clone a Repository's local path is already a Git repository (usually an \
                  earlier clone), \"Add this repository instead?\" under the path adds it.",
        ghd_behaviour: "Only says the folder contains files; the repository has to be added \
                        through Add Local Repository.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(2956), Upstream::issue(3540)],
        code: &["crates/corvane-ui/src/dialogs/clone_repository.rs"],
    },

    /// Clone from a local folder.
    CLONE_LOCAL_SOURCES = 232 "clone-local-sources" {
        title: "Clone from a local folder",
        summary: "Clone a Repository's URL tab takes a local repository (/path, ~/path or a \
                  file:// URL): the local path is named after the folder, and a path without a \
                  Git repository is reported before cloning.",
        ghd_behaviour: "A path like /a/b is taken for the GitHub repository a/b; longer paths \
                        can't be cloned.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(2995)],
        code: &["crates/corvane-core/src/clone_info.rs", "crates/corvane-ui/src/dialogs/clone_repository.rs"],
    },

    /// Clone a Repository's "Shallow clone" checkbox.
    SHALLOW_CLONE = 233 "shallow-clone" {
        title: "Shallow clone option",
        summary: "Clone a Repository shows a \"Shallow clone\" checkbox under the local path; \
                  ticked, only the latest commit of the default branch is fetched \
                  (git clone --depth 1).",
        ghd_behaviour: "Always clones the full history.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21880)],
        code: &["crates/corvane-ui/src/dialogs/clone_repository.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/ops.rs"],
    },

    /// The cloning view's Cancel button.
    CLONE_CANCEL = 234 "clone-cancel" {
        title: "Cancel a running clone",
        summary: "The cloning view has a Cancel button that stops `git clone`; git removes the \
                  directory it created.",
        ghd_behaviour: "No way to stop a clone: removing the cloning repository leaves the download \
                        running in the background.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21866), Upstream::issue(22478)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-ui/src/cloning_view.rs", "crates/corvane-git/src/process.rs"],
    },

    /// A failed clone reopens the clone dialog.
    CLONE_FAILURE_KEEPS_INPUT = 235 "clone-failure-keeps-input" {
        title: "Failed clone keeps the dialog's input",
        summary: "When a clone fails, Clone a Repository opens again with the same URL and \
                  local path and git's error at the top, ready to fix and retry.",
        ghd_behaviour: "Shows a \"Clone failed\" error dialog; the URL and path have to be \
                        entered again.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(8720)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-ui/src/dialogs/clone_repository.rs"],
    },

    /// Repository Settings › Remote shows the `upstream` remote.
    UPSTREAM_REMOTE_IN_SETTINGS = 236 "upstream-remote-in-settings" {
        title: "Repository Settings shows the upstream remote",
        summary: "Repository Settings › Remote shows the `upstream` remote's URL (read-only) \
                  under the primary remote's.",
        ghd_behaviour: "Only the primary remote.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(6877)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs"],
    },

    /// Repository Settings › Ignored Files offers the bundled templates.
    GITIGNORE_TEMPLATES = 237 "gitignore-templates" {
        title: "Repository Settings: .gitignore templates",
        summary: "Repository Settings › Ignored Files has an \"Add a template\" list (the \
                  Create a New Repository .gitignore templates) that fills an empty box or \
                  appends the template.",
        ghd_behaviour: "Templates only when creating a repository.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(2197)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs"],
    },

    /// Repository Settings › Ignored Files › Edit global ignore file.
    EDIT_GLOBAL_IGNORE_FILE = 238 "edit-global-ignore-file" {
        title: "Edit the global ignore file",
        summary: "Repository Settings › Ignored Files has an \"Edit global ignore file\" link that \
                  opens git's excludes file (core.excludesFile, else ~/.config/git/ignore, created \
                  when missing) in the external editor.",
        ghd_behaviour: "Only the repository's root .gitignore can be edited.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21951)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// Repository Settings › Git Config › Line endings (core.autocrlf).
    LINE_ENDINGS_SETTING = 239 "line-endings-setting" {
        title: "Line endings setting per repository",
        summary: "Repository Settings › Git Config adds \"Line endings (core.autocrlf)\": use the \
                  global config, or store true, input or false in the repository's own config. \
                  It applies to later checkouts and commits; files already checked out keep \
                  their line endings.",
        ghd_behaviour: "No line ending option; core.autocrlf has to be set on the command line.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5230)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs", "crates/corvane-core/src/integrations.rs"],
    },

    /// Worktree rows show and match their path.
    WORKTREE_PATHS = 240 "worktree-paths" {
        title: "Worktree list shows and searches paths",
        summary: "Rows in the worktree list have a tooltip with the worktree's name and full \
                  path, and the filter also matches the path.",
        ghd_behaviour: "Only the folder name, truncated, and the filter matches only the name, \
                        so worktrees with the same folder name look alike.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22650), Upstream::issue(22946), Upstream::issue(22375)],
        code: &["crates/corvane-ui/src/worktree_list.rs"],
    },

    /// Default location for new worktrees.
    WORKTREE_LOCATION = 241 "worktree-location" {
        title: "Default worktree location",
        summary: "Where New Worktree puts worktrees by default: `{clone-dir}` is Settings' \
                  clone directory, `{repo}` the repository's name and a leading `~` the home \
                  folder (e.g. `~/code/worktrees/{repo}`).",
        ghd_behaviour: "Always the clone directory.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "{clone-dir}", validate: worktree_location },
        corvane: Value::text("{clone-dir}"), ghd: Value::text("{clone-dir}"),
        familiar: Value::text("{clone-dir}"), max: Value::text("{clone-dir}"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22308)],
        code: &["crates/corvane-ui/src/worktree_list.rs", "crates/corvane-core/src/worktrees.rs"],
    },

    /// New worktrees default to the repository's own folder.
    WORKTREE_DIR_BESIDE_REPOSITORY = 242 "worktree-dir-beside-repository" {
        title: "New worktrees beside the repository",
        summary: "Add Worktree's path starts in the folder that holds the repository's main \
                  worktree, so worktrees become its siblings.",
        ghd_behaviour: "Starts in the last clone folder, whatever repository the dialog is for.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22565)],
        code: &["crates/corvane-ui/src/worktree_list.rs"],
    },

    /// The worktree list leaves out prunable worktrees.
    HIDE_PRUNABLE_WORKTREES = 243 "hide-prunable-worktrees" {
        title: "Hide prunable worktrees",
        summary: "The worktree list leaves out worktrees git reports as prunable (their directory \
                  was deleted outside git), so they cannot be selected into an error.",
        ghd_behaviour: "Lists them until `git worktree prune` runs; selecting one shows \"does not \
                        appear to be a valid Git repository\".",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22605)],
        code: &["crates/corvane-ui/src/worktree_list.rs", "crates/corvane-ui/src/toolbar.rs"],
    },

    /// Which repositories the hourly background fetch covers.
    BACKGROUND_FETCH = 244 "background-fetch" {
        title: "Background fetch",
        summary: "Which selected repositories are fetched in the background every hour: none, \
                  GitHub repositories only, or any repository with a remote.",
        ghd_behaviour: "GitHub repositories only, with no way to turn it off.",
        nature: Nature::Feature,
        kind: Kind::Select { options: BACKGROUND_FETCHES },
        corvane: Value::text("github"), ghd: Value::text("github"),
        familiar: Value::text("github"), max: Value::text("any"),
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(10687), Upstream::issue(12474)],
        code: &["crates/corvane-core/src/remote.rs"],
    },

    /// Push, pull and fetch stay available during a background fetch.
    PUSH_DURING_BACKGROUND_FETCH = 245 "push-during-background-fetch" {
        title: "Push during a background fetch",
        summary: "The hourly background fetch runs without taking over the push/pull button, and a \
                  push, pull or fetch asked for meanwhile starts as soon as it finishes.",
        ghd_behaviour: "The button shows the background fetch's progress and is disabled; a push \
                        from the menu is ignored until the fetch is done.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(1011)],
        code: &["crates/corvane-core/src/remote.rs"],
    },

    /// The background fetch fast-forwards the current branch.
    BACKGROUND_FETCH_FAST_FORWARDS = 246 "background-fetch-fast-forwards" {
        title: "Background fetch pulls when safe",
        summary: "After a background fetch, the checked-out branch is fast-forwarded to its \
                  upstream when it is only behind, the working directory has no changes and no \
                  merge, rebase or cherry-pick is in progress. Anything else is left for Pull.",
        ghd_behaviour: "Only fetches; the branch stays behind until you pull.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16586)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// Repository › Fetch All Repositories.
    FETCH_ALL_REPOSITORIES = 247 "fetch-all-repositories" {
        title: "Repository › Fetch All Repositories",
        summary: "The Repository menu can fetch every repository in the list that has a remote, \
                  one after another; failures are listed in one error at the end.",
        ghd_behaviour: "Fetches the selected repository only; others wait for their background \
                        fetch after being selected.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13700)],
        code: &["crates/corvane/src/menus.rs", "crates/corvane-core/src/remote.rs"],
    },

    /// Fetch deletes local tags the remote no longer has.
    FETCH_PRUNE_TAGS = 248 "fetch-prune-tags" {
        title: "Fetch prunes deleted tags",
        summary: "Fetch (and the background fetch) passes --prune-tags, so tags deleted on the \
                  remote disappear locally. Local tags that were never pushed are deleted too, \
                  except while tags created in Corvane are waiting to be pushed.",
        ghd_behaviour: "Prunes branches only; a tag deleted on the remote stays forever.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21022), Upstream::issue(22776)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// Fetch passes `--write-commit-graph`.
    FETCH_WRITES_COMMIT_GRAPH = 249 "fetch-writes-commit-graph" {
        title: "Fetch updates the commit-graph",
        summary: "Fetch passes --write-commit-graph, so git extends the commit-graph file that \
                  speeds up history, ahead/behind and merge-base computations in big repositories.",
        ghd_behaviour: "Plain fetch; the commit-graph is only written by git's own maintenance.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(22045)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// Fetch and pull leave submodules alone.
    SYNC_SKIPS_SUBMODULES = 250 "sync-skips-submodules" {
        title: "Fetch and pull skip submodules",
        summary: "Fetch and pull pass --no-recurse-submodules, so submodules are neither fetched \
                  nor updated; syncing them is left to you.",
        ghd_behaviour: "Fetch recurses into submodules on demand and pull always updates them, \
                        which fails the whole pull when a submodule has conflicts.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(15758)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// Pull skips `remote set-head -a` when the remote HEAD is known.
    REMOTE_HEAD_ONCE = 251 "remote-head-once" {
        title: "Skip updating the remote HEAD",
        summary: "After a pull, `git remote set-head -a` (which lists every ref on the server) \
                  only runs when refs/remotes/<remote>/HEAD is missing or points at a branch that \
                  no longer exists.",
        ghd_behaviour: "Runs it after every pull, which takes minutes on repositories with \
                        hundreds of thousands of refs.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(22039)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// Prune stale remote refs and retry a failed fetch or pull.
    PRUNE_STALE_REFS_AND_RETRY = 252 "prune-stale-refs-and-retry" {
        title: "Prune stale remote refs and retry",
        summary: "A fetch or pull that fails with \"cannot lock ref\" / \"unable to update local \
                  ref\" runs `git remote prune` and tries once more before showing an error.",
        ghd_behaviour: "Shows the error; the user has to run `git remote prune origin` in a \
                        terminal.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(11391)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// "Last fetched" counts the clone.
    CLONE_COUNTS_AS_FETCH = 253 "clone-counts-as-fetch" {
        title: "Last fetched counts the clone",
        summary: "A repository that was cloned and not fetched since shows the clone's time as \
                  \"Last fetched\" (from HEAD's first reflog entry) instead of \"Never fetched\".",
        ghd_behaviour: "Reads FETCH_HEAD only, which a clone does not write, so a fresh clone \
                        says \"Never fetched\".",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13401)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// Fetch / pull / push give up on a stalled HTTP transfer.
    NETWORK_STALL_TIMEOUT = 254 "network-stall-timeout" {
        title: "Give up on stalled fetch, pull and push",
        summary: "Seconds an HTTPS fetch, pull, push or clone may transfer nothing before git \
                  aborts it with an error (GIT_HTTP_LOW_SPEED_LIMIT=1 and \
                  GIT_HTTP_LOW_SPEED_TIME). 0 waits forever. Does not apply to SSH remotes.",
        ghd_behaviour: "No limit: a stalled connection leaves the operation spinning until the app \
                        is restarted.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 3600, unit: Some("s") },
        corvane: Value::Number(0), ghd: Value::Number(0),
        familiar: Value::Number(0), max: Value::Number(60),
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(22863)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/process.rs"],
    },

    /// Plain-language text for two confusing git errors.
    PLAIN_LANGUAGE_REMOTE_ERRORS = 255 "plain-language-remote-errors" {
        title: "Plain-language remote errors",
        summary: "A pull whose upstream branch was deleted on the remote, and a clone into a folder \
                  you may not write to, explain what happened in a sentence before git's message.",
        ghd_behaviour: "Shows git's text only (\"Your configuration specifies to merge with the \
                        ref …\", \"Permission denied\").",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(1325), Upstream::issue(13187)],
        code: &["crates/corvane-core/src/push_errors.rs", "crates/corvane-core/src/remote.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// Remote names containing `/` are matched whole.
    REMOTE_NAMES_WITH_SLASHES = 256 "remote-names-with-slashes" {
        title: "Remote names with slashes",
        summary: "A remote branch's remote is found by matching the configured remote names, so \
                  a remote called team/fork gives team/fork/main the branch name main (checkout, \
                  push, pull requests and the branch list use it).",
        ghd_behaviour: "Takes everything before the first / as the remote name (team), so the \
                        branch becomes fork/main.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(3618)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/repo.rs", "crates/corvane-models/src/lib.rs"],
    },

    /// The Pull button's tooltip lists the incoming commits.
    PULL_TOOLTIP_LISTS_COMMITS = 257 "pull-tooltip-lists-commits" {
        title: "Pull button lists incoming commits",
        summary: "Hovering Pull shows the summaries of the commits it would bring in (up to ten, \
                  newest first, then how many more).",
        ghd_behaviour: "No tooltip; only the behind count.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(6753)],
        code: &["crates/corvane-ui/src/toolbar.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// A failed force push keeps Force Push recommended.
    FORCE_PUSH_KEPT_ON_FAILURE = 258 "force-push-kept-on-failure" {
        title: "Failed force push keeps Force Push",
        summary: "After a rebase, the branch's \"Force push\" recommendation is only cleared once a \
                  force push succeeds, so the button still offers it after a failed attempt.",
        ghd_behaviour: "Clears the recommendation before the push runs; after a failure the button \
                        offers a plain Push that git rejects.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(16352)],
        code: &["crates/corvane-core/src/remote.rs"],
    },

    /// Force push is recommended after an amend only when the amended commit was pushed.
    AMEND_FORCE_PUSH_IF_PUSHED = 259 "amend-force-push-if-pushed" {
        title: "Force push only after amending a pushed commit",
        summary: "After amending, the toolbar recommends Force push only when the amended commit \
                  is on the branch's upstream; amending a commit that was never pushed on a branch \
                  that is also behind offers Pull, as before the amend.",
        ghd_behaviour: "Every amend makes Force push the recommended action once the branch has \
                        diverged, even when the amended commit was never pushed.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20526)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },

    /// Force push is recommended after a rewrite outside Corvane.
    FORCE_PUSH_AFTER_OUTSIDE_REWRITE = 260 "force-push-after-outside-rewrite" {
        title: "Suggest force push after an outside rewrite",
        summary: "When the branch is ahead of and behind its upstream and the upstream's tip is in \
                  the branch's reflog (pushed commits were amended, rebased or reset in another \
                  tool), the push/pull button recommends Force push instead of Pull.",
        ghd_behaviour: "Recommends a force push only after its own amend or rebase; otherwise it \
                        offers Pull, which merges the old commits back in or conflicts.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(9739)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// The push/pull foldout's "Reset to <upstream>".
    RESET_TO_REMOTE = 261 "reset-to-remote" {
        title: "Reset to remote",
        summary: "While the current branch has commits its upstream lacks, the push/pull dropdown \
                  offers \"Reset to origin/…\": after a confirmation that names what is discarded, \
                  the branch and working directory are reset hard to the upstream.",
        ghd_behaviour: "No such command; resetting to the remote needs a terminal.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16673)],
        code: &["crates/corvane-ui/src/foldout.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-ui/src/dialogs/history_dialogs.rs"],
    },

    /// View on GitHub opens other hosts' remotes too.
    VIEW_ON_REMOTE = 262 "view-on-remote" {
        title: "View on Remote for other hosts",
        summary: "Repository › View on GitHub opens the default remote's web page (as https://host/path) \
                  for a repository that is not on GitHub, and the repository list's context menu offers \
                  it as \"View on Remote\".",
        ghd_behaviour: "View on GitHub does nothing / is disabled for repositories not on GitHub.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17846), Upstream::issue(20840)],
        code: &["crates/corvane-core/src/integrations.rs", "crates/corvane-ui/src/repository_list.rs"],
    },

    /// Submodules are updated (and new ones initialised) after a checkout or merge.
    SUBMODULES_FOLLOW_CHECKOUT = 263 "submodules-follow-checkout" {
        title: "Update submodules after checkout and merge",
        summary: "After switching branches or a merge (including Update from Default Branch), \
                  submodules are checked out at the commits the branch records and new ones are \
                  cloned (git submodule update --init --recursive). Submodules that showed \
                  changes beforehand are left alone.",
        ghd_behaviour: "Submodules stay at their old commits (and new ones uninitialised), so \
                        they show as changed and are easily committed back.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18302), Upstream::issue(18673), Upstream::issue(9547)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/mco.rs", "crates/corvane-git/src/remote_ops.rs"],
    },


    /// Remove a left-over index.lock from the error dialog.
    REMOVE_STALE_INDEX_LOCK = 265 "remove-stale-index-lock" {
        title: "Remove a left-over index.lock",
        summary: "When git fails because .git/index.lock exists, the error explains it and offers \
                  Remove Lock File, which deletes the lock only when no Git process is running in \
                  the repository.",
        ghd_behaviour: "Shows git's error; the lock has to be deleted by hand.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(908)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/index_lock.rs", "crates/corvane-ui/src/dialogs/simple.rs"],
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
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
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
        familiar: Value::text("min-400"), max: Value::text("fixed-400"),
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/pull_requests.rs"],
    },

    /// Device flow or browser flow first.
    SIGN_IN_FLOW = 307 "sign-in-flow" {
        title: "Sign-in flow",
        summary: "How Sign in to GitHub.com starts: with a one-time code (device flow) or in the \
                  browser (web flow with PKCE, which GitHub refuses without a bundled client \
                  secret). Auto uses the browser on GitHub.com when the build has a client \
                  secret and the one-time code otherwise. The other flow stays one link away.",
        ghd_behaviour: "Browser flow only.",
        nature: Nature::Feature,
        kind: Kind::Select { options: SIGN_IN_FLOWS },
        corvane: Value::text("auto"), ghd: Value::text("browser"),
        familiar: Value::text("auto"), max: Value::text("auto"),
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
        familiar: Value::Number(5), max: Value::Number(5),
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/commit_status.rs"],
    },

    /// Fork and pull-request remotes follow an SSH origin.
    FORK_REMOTES_KEEP_SSH = 309 "fork-remotes-keep-ssh" {
        title: "Fork remotes keep SSH",
        summary: "When the repository's remote is SSH, Create Fork's new origin, the upstream \
                  remote and the remote added to check out a pull request from a fork use SSH \
                  on the same host too.",
        ghd_behaviour: "Always uses the API's HTTPS clone URL, so SSH-only setups cannot fetch \
                        or push through those remotes.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(19074), Upstream::issue(9490)],
        code: &["crates/corvane-core/src/forks.rs", "crates/corvane-core/src/pull_requests.rs", "crates/corvane-ui/src/dialogs/fork_dialogs.rs"],
    },

    /// No fork offers where the owner disabled forking.
    FORK_OFFER_RESPECTS_ALLOW_FORKING = 310 "fork-offer-respects-allow-forking" {
        title: "No fork offer when forking is disabled",
        summary: "A read-only repository whose owner disabled forking gets no \"create a fork\" \
                  suggestion in the commit form and no Create Fork dialog around a push.",
        ghd_behaviour: "Offers the fork anyway; creating it then fails.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22156)],
        code: &["crates/corvane-core/src/forks.rs", "crates/corvane-core/src/remote.rs", "crates/corvane-ui/src/changes.rs"],
    },

    /// Publish errors name the failed validation.
    API_ERROR_DETAILS = 311 "api-error-details" {
        title: "Publish errors say what GitHub rejected",
        summary: "When publishing a repository fails validation, the error adds GitHub's reasons \
                  (e.g. \"description is too long (maximum is 350 characters)\") to its message.",
        ghd_behaviour: "Shows only the top-level message (\"Repository creation failed.\"), or \
                        for an organization a hint to check its permissions.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19465)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-github/src/api.rs"],
    },

    /// URL actions prefer the repository itself over a fork of it.
    EXACT_REPOSITORY_URL_FIRST = 312 "exact-repository-url-first" {
        title: "Open in Desktop prefers the repository over its forks",
        summary: "When an x-corvane://openRepo URL (Open with Desktop, a new branch from the web) \
                  names a repository that is added along with a fork of it, the repository \
                  itself is opened.",
        ghd_behaviour: "Opens whichever of them comes first in the list, often the fork.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21379)],
        code: &["crates/corvane-core/src/app_url.rs"],
    },

    /// Wiki repositories are plain git repositories, not GitHub ones.
    WIKI_NOT_GITHUB = 313 "wiki-not-github" {
        title: "Wiki repositories are not treated as GitHub repositories",
        summary: "A repository whose origin is a GitHub wiki (owner/name.wiki) is handled as a \
                  plain git repository, so Corvane does not ask the API for its pull requests, \
                  issues, collaborators and checks, which do not exist. Applies at launch and \
                  when a repository is added.",
        ghd_behaviour: "Treats the wiki as a GitHub repository and every API request for it fails.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: true, visible: true, availability: available,
        upstream: &[Upstream::issue(2061)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-models/src/lib.rs"],
    },

    /// Plain-HTTP Enterprise servers.
    ENTERPRISE_PLAIN_HTTP = 314 "enterprise-plain-http" {
        title: "Allow plain-HTTP Enterprise servers",
        summary: "An Enterprise address typed with http:// stays on plain HTTP, for servers \
                  without TLS. The token then crosses the network unencrypted; an address \
                  without a scheme still uses HTTPS.",
        ghd_behaviour: "Always connects over HTTPS (plain HTTP was removed in 3.4.7).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(20245)],
        code: &["crates/corvane-ui/src/dialogs/sign_in.rs", "crates/corvane-github/src/endpoint.rs"],
    },

    /// Pull Requests tab without an account.
    PULL_REQUESTS_SIGNED_OUT = 315 "pull-requests-signed-out" {
        title: "Pull Requests tab asks to sign in",
        summary: "Without an account for the repository's host, the empty Pull Requests tab \
                  says \"Sign in to see pull requests\" with a sign-in link, and its refresh \
                  button is disabled.",
        ghd_behaviour: "Shows \"You're all set!\" and a refresh button that does nothing.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22365), Upstream::issue(5354)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-ui/src/pull_request_list.rs"],
    },

    /// Pull requests whose fork was deleted.
    PULL_REQUESTS_FROM_DELETED_FORKS = 316 "pull-requests-from-deleted-forks" {
        title: "Pull requests from deleted forks",
        summary: "Open pull requests whose head repository was deleted stay in the Pull \
                  Requests list and check out from the base repository's pull/N/head into pr/N.",
        ghd_behaviour: "Leaves them out of the list.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14090)],
        code: &["crates/corvane-core/src/pull_requests.rs"],
    },

    /// Every page of a commit's check runs.
    ALL_CHECK_RUN_PAGES = 317 "all-check-run-pages" {
        title: "Read every page of check runs",
        summary: "A commit's check runs are read page by page until all of them are in (up to \
                  1,000), so the status and the checks list count every run.",
        ghd_behaviour: "Reads the first 100 check runs only.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(18101)],
        code: &["crates/corvane-core/src/commit_status.rs", "crates/corvane-core/src/alive.rs"],
    },

    /// No Re-run for read-only repositories.
    RERUN_NEEDS_PUSH_ACCESS = 318 "rerun-needs-push-access" {
        title: "Re-run checks needs push access",
        summary: "The check-run popover hides Re-run (and the per-job re-run) when the \
                  repository's permissions say the account can only read it.",
        ghd_behaviour: "Shows Re-run to everyone; for read-only accounts the request fails.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14061)],
        code: &["crates/corvane-ui/src/ci_check_popover.rs"],
    },

    /// The check-run popover links to the pull request.
    CI_POPOVER_PULL_REQUEST_LINK = 319 "ci-popover-pull-request-link" {
        title: "Checks popover links to the pull request",
        summary: "The popover under the pull request badge ends its summary line with \
                  \"Open #N on GitHub\".",
        ghd_behaviour: "No way to open the pull request from the badge or its popover.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15418)],
        code: &["crates/corvane-ui/src/ci_check_popover.rs"],
    },

    /// Full refresh of the `#` issue cache.
    ISSUES_FULL_REFRESH_HOURS = 320 "issues-full-refresh-hours" {
        title: "Issue suggestions: full refresh interval",
        summary: "Every this many hours the # issue suggestions fetch all open issues again, so \
                  deleted and transferred issues drop out (0 never does).",
        ghd_behaviour: "Only fetches issues updated since the newest cached one, so deleted or \
                        transferred issues are suggested forever.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 720, unit: Some("h") },
        corvane: Value::Number(24), ghd: Value::Number(0),
        familiar: Value::Number(24), max: Value::Number(24),
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(14124)],
        code: &["crates/corvane-core/src/autocomplete.rs"],
    },

    /// Repository › View Upstream on GitHub.
    VIEW_UPSTREAM_ON_GITHUB = 321 "view-upstream-on-github" {
        title: "Repository › View Upstream on GitHub",
        summary: "The Repository menu adds \"View Upstream on GitHub\", which opens a fork's \
                  parent repository.",
        ghd_behaviour: "Only View on GitHub (the fork itself).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
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
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
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
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
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
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
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
        familiar: Value::text("drag-end"), max: Value::text("drag-end"),
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/toolbar.rs"],
    },

    /// Window › Corvane shows the hidden main window.
    WINDOW_MENU_MAIN_WINDOW = 405 "window-menu-main-window" {
        title: "Window menu lists the main window",
        summary: "The Window menu ends with \"Corvane\", which shows the main window again after \
                  ⌘W or the close button hid it.",
        ghd_behaviour: "The closed window is not listed; only the Dock icon brings it back.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17647)],
        code: &["crates/corvane/src/menus.rs", "crates/corvane/src/main.rs"],
    },

    /// `--hidden` launches with the window hidden.
    LAUNCH_HIDDEN = 406 "launch-hidden" {
        title: "Launch hidden with --hidden",
        summary: "Started with `--hidden` (`open -a Corvane --args --hidden`, e.g. from a login \
                  script), Corvane keeps its window hidden until the Dock icon or Window menu \
                  brings it back.",
        ghd_behaviour: "Always shows the window at launch.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(18925)],
        code: &["crates/corvane/src/main.rs"],
    },

    /// Smaller minimum window and sidebar sizes.
    SMALLER_MINIMUM_SIZES = 407 "smaller-minimum-sizes" {
        title: "Smaller minimum window and sidebar",
        summary: "The window can shrink to 600 × 400 and the repository sidebar to 120 px, for tiled \
                  and side-by-side layouts (the toolbar and lists clip below GitHub Desktop's sizes).",
        ghd_behaviour: "At least 960 × 660 for the window and 220 px for the sidebar.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: true, visible: true, availability: available,
        upstream: &[Upstream::issue(14286), Upstream::issue(22492), Upstream::issue(21368)],
        code: &["crates/corvane/src/main.rs", "crates/corvane-ui/src/workspace.rs"],
    },

    /// Open in editor / shell buttons in the toolbar.
    TOOLBAR_OPEN_BUTTONS = 408 "toolbar-open-buttons" {
        title: "Toolbar: Open in editor and shell buttons",
        summary: "Two icon buttons after Push / Pull open the repository in the external editor \
                  and in the shell.",
        ghd_behaviour: "Only the Repository menu, its shortcuts and the no-changes suggestions.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21171)],
        code: &["crates/corvane-ui/src/toolbar.rs"],
    },

    /// The repository button names the GitHub owner.
    OWNER_IN_REPOSITORY_BUTTON = 409 "owner-in-repository-button" {
        title: "Owner in the repository button",
        summary: "For a GitHub repository the Current Repository toolbar button's small line \
                  shows the owner (user or organization) instead of \"Current Repository\", so \
                  forks with the same name are told apart.",
        ghd_behaviour: "Always \"Current Repository\".",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17252)],
        code: &["crates/corvane-ui/src/toolbar.rs"],
    },

    /// An aliased repository's name is italic in the toolbar too.
    ALIAS_ITALIC_IN_TOOLBAR = 410 "alias-italic-in-toolbar" {
        title: "Aliases are italic in the toolbar",
        summary: "The Current Repository button shows an aliased repository's name in italics, \
                  as the repository list does.",
        ghd_behaviour: "Italic in the repository list, upright in the toolbar.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17770)],
        code: &["crates/corvane-ui/src/toolbar.rs"],
    },

    /// The push / pull progress tooltip keeps one width.
    STEADY_PROGRESS_TOOLTIP = 411 "steady-progress-tooltip" {
        title: "Steady push / pull progress tooltip",
        summary: "While a push, pull or fetch runs, the push / pull button's progress tooltip is \
                  always 300 px wide instead of resizing with every progress line.",
        ghd_behaviour: "The tooltip fits its text, so it jumps in size as the progress text changes.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17429)],
        code: &["crates/corvane-ui/src/toolbar.rs", "crates/corvane-ui/src/widgets.rs"],
    },

    /// The branch button shows a running merge.
    MERGE_PROGRESS_IN_BRANCH_BUTTON = 412 "merge-progress-in-branch-button" {
        title: "Branch button shows a running merge",
        summary: "While a merge runs (Merge into…, Update from Default Branch) the toolbar's \
                  branch button spins and reads \"Merging <branch>\", as it does while \
                  switching branches.",
        ghd_behaviour: "The merge dialog closes and nothing shows until the merge finishes.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(6120), Upstream::issue(15996)],
        code: &["crates/corvane-ui/src/toolbar.rs"],
    },

    /// "Copy" button on error dialogs.
    ERROR_DIALOG_COPY = 413 "error-dialog-copy" {
        title: "Copy button on error dialogs",
        summary: "Error dialogs (a failed commit, push, checkout…) have a \"Copy\" button that puts \
                  the title and message on the clipboard; the text itself cannot be selected.",
        ghd_behaviour: "No way to copy the message (⌘C does nothing).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19198), Upstream::issue(22591), Upstream::issue(22913)],
        code: &["crates/corvane-ui/src/dialogs/simple.rs"],
    },

    // ---- 500 Settings & updates ----

    /// Settings › Advanced › Save crash reports locally.
    CRASH_REPORTS = 501 "crash-reports" {
        title: "Save crash reports locally",
        summary: "Settings › Advanced offers \"Save crash reports locally\": a panic hook writes \
                  ~/Library/Logs/Corvane/crashes/ (Linux: ~/.local/state/corvane/crashes/) and \
                  the next launch lists new reports. Nothing is uploaded.",
        ghd_behaviour: "No local crash reports (GHD's crash reporter uploads to GitHub instead).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
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
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
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
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/release_notes.rs"],
    },

    /// A settings file that cannot be written is reported.
    REPORT_SETTINGS_SAVE_ERRORS = 505 "report-settings-save-errors" {
        title: "Report settings that could not be saved",
        summary: "When a changed setting cannot be written to disk, an error dialog says so \
                  (with the reason) instead of the change silently being lost on the next launch.",
        ghd_behaviour: "A failed save goes unnoticed; the setting reverts after a restart.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5046)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },

    /// No automatic update checks.
    NO_AUTOMATIC_UPDATE_CHECKS = 506 "no-automatic-update-checks" {
        title: "No automatic update checks",
        summary: "Corvane does not check for updates at launch or every four hours; Check for \
                  Updates in About still checks, downloads and installs on request.",
        ghd_behaviour: "Always checks at launch and every four hours and downloads what it finds.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(3410), Upstream::issue(22468)],
        code: &["crates/corvane-core/src/updater.rs"],
    },

    /// Editors GitHub Desktop does not detect.
    EXTRA_EDITORS = 507 "extra-editors" {
        title: "Detect more external editors",
        summary: "Settings › Integrations and Open in … also find editors GitHub Desktop 3.6.6 \
                  does not know: Antigravity.",
        ghd_behaviour: "Only its own editor table.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22922)],
        code: &["crates/corvane-platform/src/editors.rs", "crates/corvane-core/src/integrations.rs", "crates/corvane-core/src/flags/dispatch.rs"],
    },

    /// A name for the custom editor.
    CUSTOM_EDITOR_NAME = 508 "custom-editor-name" {
        title: "Custom editor name",
        summary: "Settings › Integrations › Configure Custom Editor… has a Name box; menus then \
                  say \"Open in <name>\" instead of \"Open in Custom Editor\".",
        ghd_behaviour: "Always \"Custom Editor\".",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21376)],
        code: &["crates/corvane-ui/src/dialogs/preferences.rs", "crates/corvane-core/src/state.rs"],
    },

    /// VS Code opens the repository's workspace file.
    VSCODE_WORKSPACE_FILE = 509 "vscode-workspace-file" {
        title: "Open the VS Code workspace file",
        summary: "Opening the repository in Visual Studio Code (or VSCodium, Cursor, Windsurf) \
                  opens its `*.code-workspace` file when the repository's top folder has exactly \
                  one.",
        ghd_behaviour: "Always opens the folder.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(7007)],
        code: &["crates/corvane-core/src/integrations.rs", "crates/corvane-platform/src/editors.rs"],
    },

    /// Reveal in another file manager.
    FILE_MANAGER = 510 "file-manager" {
        title: "File manager",
        summary: "Application that Show in Finder and the Reveal in Finder items open the folder \
                  with (a file's parent folder), by name or path: `Path Finder`, \
                  `/Applications/ForkLift.app`. Empty uses Finder.",
        ghd_behaviour: "Always Finder.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "Path Finder", validate: app_name },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), max: Value::text(""),
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(13812)],
        code: &["crates/corvane-core/src/integrations.rs"],
    },

    /// Open web links in a chosen browser.
    BROWSER = 511 "browser" {
        title: "Browser",
        summary: "Application that web links open in (GitHub pages, pull requests, sign-in, help \
                  links), by name or path: `Firefox`, `/Applications/Safari.app`. Empty uses the \
                  system's default browser.",
        ghd_behaviour: "Always the default browser.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "Firefox", validate: app_name },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), max: Value::text(""),
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21762)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },
    /// Settings › Prompts leaves out the Copilot prompt.
    COPILOT_PROMPT_OMITTED = 512 "copilot-prompt-omitted" {
        title: "Settings › Prompts: no Copilot prompt",
        summary: "Settings › Prompts leaves out \"Overriding commit message with generated \
                  message\": Corvane has no Copilot commit message generation, so the \
                  checkbox would do nothing.",
        ghd_behaviour: "Lists the checkbox between \"Undo commit\" and \"Removing worktrees\".",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/dialogs/preferences.rs"],
    },

    // ---- 600 Keyboard & accessibility ----

    /// ⌘9 / ⌘8 announce the width after the step.
    RESIZABLE_ANNOUNCES_NEW_WIDTH = 601 "resizable-announces-new-width" {
        title: "Expand / Contract Active Resizable announces the new width",
        summary: "⌘9 / ⌘8 announce the percentage of the width after the step.",
        ghd_behaviour: "Reads the width before applying the step, so the announced number lags one step.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/active_resizable.rs"],
    },

    /// Repository Settings › Git Config always labels its email box.
    GIT_CONFIG_EMAIL_LABEL = 602 "git-config-email-label" {
        title: "Git Config's email box keeps its label",
        summary: "Settings › Git › Author and Repository Settings › Git Config show \"Email\" \
                  above the email text box whenever it stands alone.",
        ghd_behaviour: "The label disappears whenever the email isn't one of the signed-in \
                        accounts' addresses (always, when signed out), although the code means to \
                        hide it only under the account-email dropdown's \"Other\".",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs", "crates/corvane-ui/src/dialogs/preferences.rs"],
    },

    /// Switching to Changes or History focuses that section's list.
    FOCUS_LIST_ON_SECTION_SWITCH = 603 "focus-list-on-section-switch" {
        title: "Switching tabs focuses the list",
        summary: "Clicking the Changes or History tab, ⌘1 / ⌘2 and ⌃Tab put keyboard focus on \
                  the section's list, so the arrow keys work straight away.",
        ghd_behaviour: "Focus stays where it was (often the page body), and the list has to be \
                        tabbed to.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(535)],
        code: &["crates/corvane-ui/src/workspace.rs", "crates/corvane/src/main.rs"],
    },

    /// At launch the commit summary takes focus when there are changes.
    LAUNCH_FOCUSES_COMMIT_SUMMARY = 604 "launch-focuses-commit-summary" {
        title: "Launch focuses the commit summary",
        summary: "When Corvane opens on a repository with uncommitted changes, the caret starts \
                  in the commit summary, ready to type.",
        ghd_behaviour: "Nothing useful has focus at launch; the summary has to be clicked (or \
                        reached with ⌘G).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20417)],
        code: &["crates/corvane-ui/src/workspace.rs"],
    },

    /// ⌘⌫ in the changes list discards the highlighted files.
    CMD_BACKSPACE_DISCARDS_FILES = 605 "cmd-backspace-discards-files" {
        title: "⌘⌫ in the changes list discards the selected files",
        summary: "With the changes list focused, ⌘⌫ discards the highlighted files (confirming \
                  as the context menu's Discard Changes does); elsewhere it still removes the \
                  repository.",
        ghd_behaviour: "⌘⌫ is Repository › Remove… everywhere, also in the changes list.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(11924), Upstream::issue(17680)],
        code: &["crates/corvane-ui/src/keymap.rs", "crates/corvane-ui/src/changes.rs"],
    },

    /// ⇧⌘A / ⌥⌘O open the file selected in a file list.
    OPEN_FILE_SHORTCUTS = 606 "open-file-shortcuts" {
        title: "Shortcuts that open the selected file",
        summary: "With the changes list or a commit's file list focused, ⇧⌘A opens the selected \
                  file in the external editor (instead of the repository) and ⌥⌘O opens it with \
                  its default program.",
        ghd_behaviour: "Only the file context menu opens a file; ⇧⌘A always opens the repository.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(13691), Upstream::issue(4655), Upstream::issue(20773)],
        code: &[
            "crates/corvane-ui/src/keymap.rs",
            "crates/corvane-ui/src/changes.rs",
            "crates/corvane-ui/src/selected_commit.rs",
        ],
    },

    /// Repository › Push has no ⌘P shortcut.
    NO_PUSH_SHORTCUT = 607 "no-push-shortcut" {
        title: "No ⌘P shortcut for Push",
        summary: "⌘P does nothing and Repository › Push shows no shortcut, so a stray ⌘P (print, \
                  quick open in an editor) cannot push.",
        ghd_behaviour: "⌘P pushes (or opens Force Push… after a rebase or amend).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14604)],
        code: &["crates/corvane-ui/src/keymap.rs"],
    },

    /// ⌥⌘T opens the repository in the shell, next to ⌃`.
    OPEN_IN_SHELL_ALT_SHORTCUT = 608 "open-in-shell-alt-shortcut" {
        title: "⌥⌘T also opens the shell",
        summary: "Repository › Open in <shell> also answers to ⌥⌘T, which every keyboard layout \
                  can type (⌃` is a dead key on German and other layouts).",
        ghd_behaviour: "Only ⌃`.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(3240)],
        code: &["crates/corvane-ui/src/keymap.rs"],
    },

    /// ⌃N / ⌃P move the selection in the changes and history lists.
    EMACS_LIST_KEYS = 609 "emacs-list-keys" {
        title: "⌃N / ⌃P move through lists",
        summary: "⌃N and ⌃P select the next and previous row of the changes and history lists, \
                  like ↓ and ↑ (the macOS text-system keys).",
        ghd_behaviour: "Only the arrow keys move the selection.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(7266)],
        code: &["crates/corvane-ui/src/keymap.rs"],
    },

    /// ⌥⌘S switches the diff between unified and split.
    DIFF_MODE_SHORTCUT = 610 "diff-mode-shortcut" {
        title: "⌥⌘S switches the diff display",
        summary: "⌥⌘S toggles Diff Settings between Unified and Split.",
        ghd_behaviour: "Only the diff settings popover changes it (three clicks).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(15284)],
        code: &["crates/corvane-ui/src/keymap.rs", "crates/corvane/src/main.rs"],
    },

    /// ⌥⌘C / ⇧⌥⌘C copy the selected files' paths.
    COPY_PATH_SHORTCUTS = 611 "copy-path-shortcuts" {
        title: "Shortcuts that copy file paths",
        summary: "With the changes list or a commit's file list focused, ⌥⌘C copies the selected \
                  files' full paths and ⇧⌥⌘C their paths relative to the repository (VS Code's \
                  keys), one per line.",
        ghd_behaviour: "Only the file context menu copies paths.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21810)],
        code: &[
            "crates/corvane-ui/src/keymap.rs",
            "crates/corvane-ui/src/changes.rs",
            "crates/corvane-ui/src/selected_commit.rs",
        ],
    },

    /// ⌃⌘P, ⇧⌘] / ⇧⌘[, ⌘3 and ⌥↓ / ⌥↑ in the diff.
    NAVIGATION_SHORTCUTS = 612 "navigation-shortcuts" {
        title: "More navigation shortcuts",
        summary: "View › Show Pull Requests List (⌃⌘P) opens the branch list on its Pull Requests \
                  tab; ⇧⌘] / ⇧⌘[ switch to the next / previous repository in the list's order; \
                  ⌘3 focuses the diff; ⌥↓ / ⌥↑ in the diff select the next / previous file.",
        ghd_behaviour: "None of these shortcuts; the pull request list, other repositories and the \
                        diff are reached with the mouse or by tabbing.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[
            Upstream::issue(16854),
            Upstream::issue(20115),
            Upstream::issue(20677),
            Upstream::issue(19935),
        ],
        code: &[
            "crates/corvane-ui/src/keymap.rs",
            "crates/corvane-ui/src/workspace.rs",
            "crates/corvane/src/main.rs",
            "crates/corvane/src/menus.rs",
        ],
    },

    // ---- 700 Changes & diffs ----

    /// Changes list: lines added / deleted per file and in total.
    CHANGES_LINE_COUNTS = 701 "changes-line-counts" {
        title: "Line counts in the Changes list",
        summary: "Each changed file shows the lines it adds and removes against the last commit \
                  (+N -M), and the \"N changed files\" header shows the totals of the listed \
                  files. Runs git diff --numstat on every refresh; untracked files over 1 MiB and \
                  binary files get no count.",
        ghd_behaviour: "No line counts for uncommitted changes (only the commit summary in History \
                        has them).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[
            Upstream::issue(12914),
            Upstream::issue(14916),
            Upstream::issue(16024),
            Upstream::issue(16930),
            Upstream::issue(22403),
        ],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-ui/src/changes.rs", "crates/corvane-git/src/status.rs"],
    },

    /// File names without their directory in the changes list.
    CHANGES_FILE_NAMES_ONLY = 702 "changes-file-names-only" {
        title: "File names only in the changes list",
        summary: "Rows of the changes list show the file name alone, without its directory \
                  (the filter still matches the whole path).",
        ghd_behaviour: "Directory (dimmed) followed by the file name.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14268), Upstream::issue(19016)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// Order of the changes list.
    CHANGES_SORT_ORDER = 703 "changes-sort-order" {
        title: "Changes list order",
        summary: "How the changes list orders its files: by path, by status (conflicted, new, \
                  modified, renamed, deleted; path order within each), or by file name. A filter \
                  text still ranks its matches best first.",
        ghd_behaviour: "Path order (git's).",
        nature: Nature::Feature,
        kind: Kind::Select { options: CHANGES_SORT_ORDERS },
        corvane: Value::text("path"), ghd: Value::text("path"),
        familiar: Value::text("path"), max: Value::text("path"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(4739)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/filter.rs"],
    },

    /// How the changes filter text matches.
    CHANGES_FILTER_MATCH = 704 "changes-filter-match" {
        title: "Changes filter matching",
        summary: "How the changes list's filter text matches a path: fuzzily (the letters in \
                  order), as a substring, as the end of the path (`.meta`), or as the exact path \
                  or file name. Case is ignored.",
        ghd_behaviour: "Fuzzy only.",
        nature: Nature::Feature,
        kind: Kind::Select { options: CHANGES_FILTER_MATCHES },
        corvane: Value::text("fuzzy"), ghd: Value::text("fuzzy"),
        familiar: Value::text("fuzzy"), max: Value::text("fuzzy"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20555)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/filter.rs"],
    },

    /// "Renamed files" in the changes list's Filter Options.
    RENAMED_FILES_FILTER = 705 "renamed-files-filter" {
        title: "\"Renamed files\" filter option",
        summary: "The changes list's Filter Options popover has a sixth option, \"Renamed files\".",
        ghd_behaviour: "Included / excluded, new, modified and deleted files only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21147)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/filter.rs"],
    },

    /// Glob patterns hidden from the changes list.
    CHANGES_HIDE_GLOBS = 706 "changes-hide-globs" {
        title: "Hide files from the changes list",
        summary: "Changed files matching these comma-separated glob patterns (gitignore-like: \
                  `*.lock`, `node_modules`, `/docs/**`) are left out of the changes list, which then \
                  reads \"N of M changed files\". View only: hidden files are still included in \
                  commits. Empty hides nothing.",
        ghd_behaviour: "Lists every changed file.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "*.lock, node_modules", validate: hide_globs },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), max: Value::text(""),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(10093), Upstream::issue(20615), Upstream::issue(21242)],
        code: &["crates/corvane-core/src/filter.rs", "crates/corvane-ui/src/changes.rs"],
    },

    /// ⇧-click keeps ⌘-clicked rows.
    SHIFT_CLICK_KEEPS_SELECTION = 707 "shift-click-keeps-selection" {
        title: "⇧-click keeps ⌘-clicked files",
        summary: "In the changes list, ⇧-click replaces only the range from the last clicked \
                  file; files ⌘-clicked outside it stay selected, as in Finder.",
        ghd_behaviour: "⇧-click selects the range alone and drops the other selected files.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16355)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/list_selection.rs"],
    },

    /// A spinner in the Changes header while discarding or refreshing.
    CHANGES_BUSY_INDICATOR = 708 "changes-busy-indicator" {
        title: "Changes list busy indicator",
        summary: "The \"N changed files\" row ends in a spinner while Discard Changes runs and \
                  while a status refresh has been running for more than 300 ms, so a slow \
                  discard or git status is visibly in progress.",
        ghd_behaviour: "Nothing shows that a discard or status refresh is still running.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15297), Upstream::issue(1914)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// New untracked files start unticked in the Changes list.
    NEW_UNTRACKED_FILES_EXCLUDED = 709 "new-untracked-files-excluded" {
        title: "New untracked files start unticked",
        summary: "An untracked file that appears in the Changes list starts unticked, so it is \
                  only committed once it is ticked; tracked changes are still included.",
        ghd_behaviour: "Every new file is ticked and goes into the next commit.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18774), Upstream::issue(21427)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },

    /// `status.showUntrackedFiles=no` hides untracked files.
    RESPECT_SHOW_UNTRACKED_FILES = 710 "respect-show-untracked-files" {
        title: "Respect status.showUntrackedFiles",
        summary: "When the repository's git config sets status.showUntrackedFiles to no, the \
                  Changes list leaves untracked files out, as git status does (useful for a home \
                  directory or dotfiles repository). Untracked files then cannot be committed \
                  from Corvane until they are added with git.",
        ghd_behaviour: "Always lists every untracked file (--untracked-files=all).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(3734)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/status.rs"],
    },

    /// `git status --ignore-submodules`.
    IGNORE_SUBMODULES = 711 "ignore-submodules" {
        title: "Hide submodule changes",
        summary: "What the Changes list leaves out about submodules: nothing beyond each \
                  submodule's own submodule.<name>.ignore setting, changes inside submodules (a \
                  new submodule commit is still listed), or submodules altogether (a new \
                  submodule commit can then not be committed from Corvane).",
        ghd_behaviour: "As configured: only submodule.<name>.ignore hides a submodule.",
        nature: Nature::Feature,
        kind: Kind::Select { options: IGNORE_SUBMODULE_MODES },
        corvane: Value::text("configured"), ghd: Value::text("configured"),
        familiar: Value::text("configured"), max: Value::text("configured"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20484)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/status.rs"],
    },

    /// Open in editor / default program acts on every selected file.
    OPEN_MULTIPLE_FILES = 712 "open-multiple-files" {
        title: "Open several files at once",
        summary: "With several changed files selected, \"Open in <editor>\" and \"Open with Default \
                  Program\" open all of them; the changes list's context menu gains \"Open All in \
                  <editor>\" and a history file's menu \"Open All Files of Commit in <editor>\". \
                  At most 25 files at a time.",
        ghd_behaviour: "Opens only the right-clicked file.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16262), Upstream::issue(21374), Upstream::issue(15013)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-ui/src/selected_commit.rs"],
    },

    /// "Open With…" in the changes list's file menu.
    OPEN_FILE_WITH = 713 "open-file-with" {
        title: "Open a changed file with…",
        summary: "The changes list's file menu has \"Open With…\" after \"Open with Default \
                  Program\": pick any application to open the file in.",
        ghd_behaviour: "Only the configured editor or the default program.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20166)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/integrations.rs"],
    },

    /// "Copy Diff" in the changes list's file menu.
    COPY_DIFF = 714 "copy-diff" {
        title: "Copy Diff",
        summary: "The changes list's file context menu has \"Copy Diff\" (\"Copy Diff of Selected \
                  Files\" for a multi-selection): the working-directory changes of those files as \
                  a patch `git apply` takes, untracked files included.",
        ghd_behaviour: "No way to copy a diff.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17746)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/diff.rs"],
    },

    /// "Assume Unchanged" in the changes list's menus.
    ASSUME_UNCHANGED = 715 "assume-unchanged" {
        title: "Assume Unchanged",
        summary: "The changes list's file menu has \"Assume Unchanged\" (`git update-index \
                  --assume-unchanged`) for modified or deleted tracked files, which then leave \
                  the list; the list's own menu has \"Stop Assuming Files Unchanged\" to bring \
                  them all back.",
        ghd_behaviour: "No such items; only ignoring (which does not affect tracked files).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22841)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/commit.rs"],
    },

    /// Warn about paths Windows cannot check out.
    WINDOWS_INVALID_NAMES_WARNING = 716 "windows-invalid-names-warning" {
        title: "Warn about names invalid on Windows",
        summary: "The commit form warns when an included file's path is invalid on Windows (a \
                  reserved name like `CON` or `nul.txt`, a character such as `:` or `?`, or a name \
                  ending in a space or a dot). Committing stays possible.",
        ghd_behaviour: "Commits them silently; Windows clones then fail to check them out.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19292)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/portable_paths.rs"],
    },

    /// Ignore File / Folder / Extension skip rules already in .gitignore.
    IGNORE_SKIPS_EXISTING_RULES = 717 "ignore-skips-existing-rules" {
        title: "Ignore menu items don't duplicate .gitignore rules",
        summary: "\"Ignore File\", \"Ignore Folder\" and \"Ignore All .ext Files\" leave out \
                  patterns the root .gitignore already has as a line.",
        ghd_behaviour: "Appends the pattern again, so repeated use fills .gitignore with duplicate \
                        lines.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(2537)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/ignore.rs"],
    },

    /// Changed-file counts in the "Ignore All .x Files" items.
    IGNORE_MENU_COUNTS = 718 "ignore-menu-counts" {
        title: "Counts in \"Ignore All .x Files\"",
        summary: "The changes list's \"Ignore All .x Files\" context-menu items say how many \
                  changed files have that extension: \"Ignore All .png Files (170 Changed)\".",
        ghd_behaviour: "\"Ignore All .png Files (Add to .gitignore)\", no count.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13789)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// "Ignore File In" submenu.
    IGNORE_FILE_TARGETS = 719 "ignore-file-targets" {
        title: "Choose the ignore file",
        summary: "A changed file's context menu adds Ignore File In: the .gitignore of a folder \
                  above the file (anchored to that folder), .git/info/exclude (this clone only) or \
                  the global excludes file (core.excludesFile, else ~/.config/git/ignore; the file \
                  name is ignored in every repository).",
        ghd_behaviour: "Ignore items always write to the .gitignore at the repository root.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12171), Upstream::issue(16028)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/ignore.rs"],
    },

    /// Discarding a dirty submodule cleans inside it.
    DISCARD_SUBMODULE_CHANGES = 720 "discard-submodule-changes" {
        title: "Discard cleans changes inside submodules",
        summary: "Discarding a submodule that has changes inside checks out its modified files \
                  and moves its untracked files to the Trash, so the submodule is clean \
                  afterwards.",
        ghd_behaviour: "The submodule stays in the list: untracked files and edits inside it are \
                        not discarded.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(10403)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/commit.rs"],
    },

    /// No Trash sentence when only submodules are discarded.
    DISCARD_SUBMODULE_NO_TRASH_HINT = 721 "discard-submodule-no-trash-hint" {
        title: "Discarding submodules does not mention the Trash",
        summary: "When every discarded entry is a submodule, the discard confirmation leaves out \
                  \"Changes can be restored by retrieving them from the Trash\": nothing is moved \
                  there.",
        ghd_behaviour: "Always promises the Trash.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(10402)],
        code: &["crates/corvane-ui/src/dialogs/discard_changes.rs"],
    },

    /// Discard deletes files instead of moving them to the Trash.
    DISCARD_SKIPS_TRASH = 722 "discard-skips-trash" {
        title: "Discard deletes instead of using the Trash",
        summary: "Discarding changes deletes new and untracked files (and untracked files inside \
                  a discarded submodule) permanently instead of moving them to the Trash; the \
                  confirmation says they cannot be restored.",
        ghd_behaviour: "Always moves discarded files to the Trash.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(10445)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-ui/src/dialogs/discard_changes.rs"],
    },

    /// Snooze the discard confirmation.
    DISCARD_CONFIRM_SNOOZE = 723 "discard-confirm-snooze" {
        title: "Snooze the discard confirmation",
        summary: "The Confirm Discard Changes dialog offers \"Do not show this message again for N \
                  minutes\": discarding in that repository then skips the confirmation for N \
                  minutes (this session; Discard All Changes still asks). 0 hides the option.",
        ghd_behaviour: "Only \"Do not show this message again\", for good.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 120, unit: Some("min") },
        corvane: Value::Number(0), ghd: Value::Number(0),
        familiar: Value::Number(0), max: Value::Number(10),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20747)],
        code: &["crates/corvane-ui/src/dialogs/discard_changes.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// "No local changes" offers Open in <Shell>.
    NO_CHANGES_OPEN_IN_SHELL = 724 "no-changes-open-in-shell" {
        title: "No local changes: Open in shell",
        summary: "The \"No local changes\" view adds an \"Open the repository in <Shell>\" \
                  suggestion after Show in Finder.",
        ghd_behaviour: "Editor, Finder and GitHub suggestions only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12453)],
        code: &["crates/corvane-ui/src/workspace.rs"],
    },

    /// "No local changes" links the branch's open pull request.
    NO_CHANGES_VIEW_PULL_REQUEST = 725 "no-changes-view-pull-request" {
        title: "\"View Pull Request\" when there are no local changes",
        summary: "While the current branch has an open pull request, the \"No local changes\" view \
                  leads with a \"View Pull Request\" card naming its number and title, opening it \
                  on GitHub.",
        ghd_behaviour: "Shows no pull request action while one is open (only Create / Preview \
                        Pull Request for a branch without one).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19329)],
        code: &["crates/corvane-ui/src/workspace.rs"],
    },

    /// "No local changes" offers restoring the branch's stash.
    RESTORE_STASH_SUGGESTION = 726 "restore-stash-suggestion" {
        title: "Restore stash from No local changes",
        summary: "When the branch has stashed changes and nothing else is changed, the \"No local \
                  changes\" view starts with a \"Restore your stashed changes\" card whose Restore \
                  button brings them back in one click.",
        ghd_behaviour: "The stash has to be opened from the bottom of the Changes tab first.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12864)],
        code: &["crates/corvane-ui/src/workspace.rs"],
    },

    /// A dot on the Changes tab while History shows and the branch has a stash.
    STASH_DOT_ON_CHANGES_TAB = 727 "stash-dot-on-changes-tab" {
        title: "Stash dot on the Changes tab",
        summary: "While the History tab is showing, the Changes tab has a blue dot when the \
                  current branch has stashed changes.",
        ghd_behaviour: "The stash is only visible at the bottom of the Changes tab.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(8589)],
        code: &["crates/corvane-ui/src/workspace.rs", "crates/corvane-ui/src/tab_bar.rs"],
    },

    /// Show the newest command-line stash when the branch has no Desktop stash.
    SHOW_LATEST_OTHER_STASH = 728 "show-latest-other-stash" {
        title: "Show stashes made outside Corvane",
        summary: "When the current branch has no stash of its own, the Changes list's Stashed \
                  Changes row shows the newest stash that GitHub Desktop or Corvane did not make \
                  (git stash on the command line), so it can be viewed, restored or discarded. \
                  Stashing from Corvane never replaces such a stash.",
        ghd_behaviour: "Only stashes named !!GitHub_Desktop<branch> are shown; others are invisible.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17147)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/state.rs"],
    },

    /// Restore a branch's stash when switching back to it with no changes.
    POP_STASH_ON_RETURN = 729 "pop-stash-on-return" {
        title: "Restore a branch's stash when returning to it",
        summary: "Switching to a branch that has stashed changes restores them (git stash pop) \
                  when the working directory is clean after the switch, e.g. when the changes on \
                  the branch being left were stashed there. A pop that conflicts keeps the stash \
                  and shows the conflicts.",
        ghd_behaviour: "The stash stays until Stashed Changes › Restore is clicked.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17682)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },

    /// Commit form warning while HEAD is detached.
    DETACHED_HEAD_COMMIT_WARNING = 730 "detached-head-commit-warning" {
        title: "Warn when committing on a detached HEAD",
        summary: "While HEAD is detached the commit form shows a warning that the commit will not \
                  be on any branch, with a link to create one.",
        ghd_behaviour: "Commits on a detached HEAD without a word; the button reads \"Commit to\".",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(788)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// ↑ / ↓ in an empty commit summary recall recent commit messages.
    RECALL_COMMIT_MESSAGES = 731 "recall-commit-messages" {
        title: "Recall recent commit messages with ↑ / ↓",
        summary: "In an empty commit form, ↑ in the summary fills in the summary and description \
                  of the latest commit on the branch; more ↑ go further back (merges and repeated \
                  summaries skipped), ↓ comes forward and past the newest empties the form again. \
                  Editing the text keeps it.",
        ghd_behaviour: "↑ / ↓ only move the caret.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12927), Upstream::issue(20559), Upstream::issue(17525)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-models/src/lib.rs"],
    },

    /// Confirmation before committing on the default branch.
    CONFIRM_COMMIT_TO_DEFAULT_BRANCH = 732 "confirm-commit-to-default-branch" {
        title: "Confirm commits to the default branch",
        summary: "Committing (not amending) while the default branch is checked out asks \
                  \"Commit to Default Branch\" first.",
        ghd_behaviour: "Commits to the default branch without asking.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21857)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-ui/src/dialogs/confirm_commit_to_default_branch.rs"],
    },

    /// Hard 72-character limit on the commit summary.
    SUMMARY_MAX_LENGTH = 733 "summary-max-length" {
        title: "Limit the commit summary to 72 characters",
        summary: "The commit summary field takes at most 72 characters (GitHub truncates longer \
                  summaries); typing or pasting past the limit drops the excess, like an HTML \
                  maxlength.",
        ghd_behaviour: "No limit.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18290)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// "Committing as" line above the commit summary.
    COMMIT_AUTHOR_LINE = 734 "commit-author-line" {
        title: "Show the commit author",
        summary: "A line above the commit summary names the identity git resolved for this \
                  repository (`user.name` / `user.email`, `includeIf` included): \
                  \"Committing as Name <email>\".",
        ghd_behaviour: "Only the avatar, whose tooltip names the author.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21883)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// "Name <email>" co-authors without a GitHub account.
    FREE_FORM_CO_AUTHORS = 735 "free-form-co-authors" {
        title: "Co-authors without a GitHub account",
        summary: "Typing \"Name <email>\" in the co-authors box adds that person as a co-author \
                  (a Co-Authored-By trailer) without looking them up on GitHub. To allow spaces \
                  in names, Space turns a typed word into a GitHub handle only when it starts \
                  with @; the suggestions list works as before.",
        ghd_behaviour: "Only GitHub users can be added; every word becomes a handle on Space.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(4308)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/autocomplete.rs"],
    },

    /// Commit form gear › Push After Committing.
    COMMIT_AND_PUSH = 736 "commit-and-push" {
        title: "Push after committing",
        summary: "The commit form's gear menu adds Push After Committing (kept per repository); \
                  while it is ticked the button reads \"Commit and push to main\" and the branch \
                  is pushed (or published) once the commit, hooks included, succeeds. Push errors \
                  show as for the toolbar button; amended commits are not pushed.",
        ghd_behaviour: "Committing and pushing are separate steps.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21874), Upstream::issue(22742)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// Optional tag field in the commit form.
    COMMIT_TAG_FIELD = 737 "commit-tag-field" {
        title: "Tag field in the commit form",
        summary: "The commit form has a \"Tag (optional)\" field under the description; a name \
                  there tags the new commit once it is made (not when amending).",
        ghd_behaviour: "Tags are created from History after committing.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16256)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// Clear the drafted message after a matching outside commit.
    CLEAR_MESSAGE_AFTER_OUTSIDE_COMMIT = 738 "clear-message-after-outside-commit" {
        title: "Clear the draft after an outside commit",
        summary: "When a new commit appears on the branch (made on the command line or in another \
                  app) whose summary is the one drafted in the commit form, the form is cleared \
                  as after committing in Corvane.",
        ghd_behaviour: "The drafted message stays.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5233)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// Context menu on the "Committed … Undo" bar.
    UNDO_BAR_MENU = 739 "undo-bar-menu" {
        title: "Context menu on the undo bar",
        summary: "Right-clicking the \"Committed just now … Undo\" bar under the commit button \
                  offers Amend Commit…, Undo Commit…, Create Tag…, Copy SHA and View on GitHub \
                  for that commit.",
        ghd_behaviour: "No context menu there; those items live in History.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(12561), Upstream::issue(19938)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// Spinner while a working-directory diff loads.
    DIFF_LOADING_INDICATOR = 740 "diff-loading-indicator" {
        title: "Diff loading spinner",
        summary: "When a changed file's diff takes more than 300 ms to compute (large files, slow \
                  filters), a spinner covers the diff pane until it is ready.",
        ghd_behaviour: "The previous diff (or an empty pane) stays up with no sign of work.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(1913)],
        code: &["crates/corvane-ui/src/diff_view.rs"],
    },

    /// The diff's "Open in <Editor> at Line N".
    DIFF_OPEN_IN_EDITOR_AT_LINE = 741 "diff-open-in-editor-at-line" {
        title: "Diff: Open in editor at a line",
        summary: "Right-clicking a line of a working-directory diff offers \"Open in <Editor> at \
                  Line N\" when the editor can jump to a line (VS Code and its forks, Sublime \
                  Text, Zed).",
        ghd_behaviour: "The diff's context menu has no editor item; Open in <Editor> opens the \
                        file at its top.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14476), Upstream::issue(20254)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-platform/src/editors.rs", "crates/corvane-core/src/integrations.rs"],
    },

    /// "The file mode changed" for a mode-only diff.
    FILE_MODE_CHANGE_MESSAGE = 742 "file-mode-change-message" {
        title: "Say when only the file mode changed",
        summary: "A diff whose only change is the file mode (e.g. the executable bit) says \
                  \"The file mode changed from 100644 to 100755\".",
        ghd_behaviour: "\"No content changes found\", or \"Only whitespace changes found\" while \
                        whitespace changes are hidden.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11685), Upstream::issue(557)],
        code: &["crates/corvane-git/src/diff.rs", "crates/corvane-ui/src/diff_view.rs"],
    },

    /// A renamed file's diff starts from HEAD.
    RENAMED_DIFF_AGAINST_HEAD = 743 "renamed-diff-against-head" {
        title: "Renamed files diff against the last commit",
        summary: "A renamed file's diff compares the old path in the last commit with the \
                  working copy, so edits staged outside Corvane show up too.",
        ghd_behaviour: "Compares the index with the working copy: a renamed file whose edits \
                        were staged (e.g. by `git add`) shows no changes.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19142), Upstream::issue(5575)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/diff.rs"],
    },

    /// Split mode shows added and deleted files unified.
    UNIFIED_DIFF_FOR_ADDED_FILES = 744 "unified-diff-for-added-files" {
        title: "Added and deleted files use the unified layout",
        summary: "With Diff Settings › Split selected, a new or deleted file is still shown \
                  unified, across the whole width, instead of beside an empty column.",
        ghd_behaviour: "Split mode draws a new file in the right half next to an empty left \
                        half (a deleted one the other way round).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13763), Upstream::issue(16610)],
        code: &["crates/corvane-ui/src/diff_view.rs"],
    },

    /// A symbolic link's contents are its target path.
    SYMLINK_CONTENTS = 745 "symlink-contents" {
        title: "Symbolic links are not followed",
        summary: "Loading a changed symbolic link reads the path it points to, as git records \
                  it, instead of the file behind it.",
        ghd_behaviour: "Reads the file the link points to for hunk expansion, so a link to a \
                        pipe or device keeps the diff loading forever and a link to a huge file \
                        loads it whole.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(18620)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/diff.rs"],
    },

    /// Intra-line highlights end on grapheme boundaries.
    INTRA_LINE_GRAPHEMES = 746 "intra-line-graphemes" {
        title: "Intra-line highlights keep accents with their letters",
        summary: "The changed part of a modified line is widened to whole characters as \
                  people see them (grapheme clusters), so a combining accent is highlighted \
                  together with its letter.",
        ghd_behaviour: "Compares UTF-16 code units, so the highlight can cut a combining mark off \
                        its base character and the mark renders apart or disappears.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11492)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-ui/src/diff_view_rows.rs"],
    },

    /// The intra-line highlighting length cap.
    INTRA_LINE_MAX_LENGTH = 747 "intra-line-max-length" {
        title: "Longest line with intra-line highlighting",
        summary: "A modified line pair gets its changed characters highlighted only while both \
                  lines are shorter than this many bytes (0: no limit).",
        ghd_behaviour: "1024 (`MaxIntraLineDiffStringLength`), fixed; longer lines only show as \
                        wholly replaced.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 1_000_000, unit: Some("bytes") },
        corvane: Value::Number(1024), ghd: Value::Number(1024),
        familiar: Value::Number(1024), max: Value::Number(1024),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22556)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-ui/src/diff_view_rows.rs"],
    },

    /// Visible whitespace in diffs.
    DIFF_SHOW_WHITESPACE = 748 "diff-show-whitespace" {
        title: "Show whitespace in diffs",
        summary: "Diff lines mark every space with a faint dot and every tab with a faint line, \
                  so indentation and trailing whitespace changes can be told apart.",
        ghd_behaviour: "Whitespace is invisible.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12974)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-ui/src/diff_view_rows.rs"],
    },

    /// "Show the diff as text anyway" on binary files.
    BINARY_DIFF_AS_TEXT = 749 "binary-diff-as-text" {
        title: "Show binary files' diffs as text",
        summary: "A changed file git takes for binary (a stray NUL byte, an odd encoding) offers \
                  \"Show the diff as text anyway.\", which diffs it line by line with \
                  `git diff --text`. Its lines cannot be picked for a partial commit.",
        ghd_behaviour: "\"This binary file has changed.\" and a link to open it elsewhere.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16855)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/diff.rs"],
    },

    /// Diffs open with the whole file expanded.
    DIFF_EXPAND_WHOLE_FILE = 750 "diff-expand-whole-file" {
        title: "Expand the whole file in diffs",
        summary: "Every text diff opens as if \"Expand Whole File\" had been picked (files up \
                  to 20 000 lines; large diffs stay collapsed). \"Collapse Expanded Lines\" \
                  still collapses it.",
        ghd_behaviour: "Diffs open collapsed to their hunks; the expansion is per file and \
                        forgotten.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16140), Upstream::issue(20548)],
        code: &["crates/corvane-ui/src/diff_view.rs"],
    },

    /// The diff's font size.
    DIFF_FONT_SIZE = 751 "diff-font-size" {
        title: "Diff font size",
        summary: "The size of the diff's monospace text, 9 to 16 pixels at 100 % zoom (0 \
                  keeps 11 px). Rows stay 20 px tall.",
        ghd_behaviour: "11 px, changed only by zooming the whole window.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 16, unit: Some("px") },
        corvane: Value::Number(0), ghd: Value::Number(0),
        familiar: Value::Number(0), max: Value::Number(0),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22929)],
        code: &["crates/corvane-ui/src/diff_view.rs"],
    },

    /// Image diff borders go around the image.
    IMAGE_DIFF_BORDER_OUTSIDE = 752 "image-diff-border-outside" {
        title: "Image diff borders don't shrink the image",
        summary: "The coloured 1 px border of an image in the image diff is drawn around the \
                  image, which keeps its natural (or fitted) size.",
        ghd_behaviour: "The border is inside the image's box (`box-sizing: border-box`), so every \
                        image is drawn 2 px smaller than its size, blurring small images and \
                        pixel art.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14469)],
        code: &["crates/corvane-ui/src/image_diff.rs"],
    },

    /// The image diff's checkerboard.
    IMAGE_DIFF_BACKGROUND = 753 "image-diff-background" {
        title: "Image diff background",
        summary: "The checkerboard behind images in the image diff: light, dark, or dark while \
                  the app theme is dark, so light and translucent images stay visible.",
        ghd_behaviour: "Always the light checkerboard.",
        nature: Nature::Feature,
        kind: Kind::Select { options: IMAGE_DIFF_BACKGROUNDS },
        corvane: Value::text("light"), ghd: Value::text("light"),
        familiar: Value::text("light"), max: Value::text("theme"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21092)],
        code: &["crates/corvane-ui/src/image_diff.rs"],
    },

    /// How Swipe / Onion Skin / Difference align images of different sizes.
    IMAGE_DIFF_ALIGNMENT = 754 "image-diff-alignment" {
        title: "Image diff alignment",
        summary: "Where Swipe, Onion Skin and Difference put two images of different sizes: \
                  centred on each other, or with their top left corners together (for layouts \
                  that grow to the right and down, such as UI snapshots).",
        ghd_behaviour: "Always centred.",
        nature: Nature::Feature,
        kind: Kind::Select { options: IMAGE_DIFF_ALIGNMENTS },
        corvane: Value::text("centre"), ghd: Value::text("centre"),
        familiar: Value::text("centre"), max: Value::text("centre"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19385)],
        code: &["crates/corvane-ui/src/image_diff.rs"],
    },

    /// TGA images get an image diff.
    TGA_IMAGE_DIFF = 755 "tga-image-diff" {
        title: "Image diffs for TGA files",
        summary: "Changed `.tga` images (common in game assets) are shown in the image diff \
                  (2-up, Swipe, Onion Skin, Difference).",
        ghd_behaviour: "\"This binary file has changed.\"",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21970)],
        code: &["crates/corvane-ui/src/image_diff.rs", "crates/corvane-ui/src/diff_view.rs", "crates/corvane-models/src/lib.rs"],
    },

    // ---- 800 History & branches ----

    /// History review mode: the diff alone, full width.
    HISTORY_REVIEW_MODE = 801 "history-review-mode" {
        title: "History review mode",
        summary: "View › Toggle History Review Mode (⌃⌘S) hides the repository sidebar and the \
                  commit's file list in History so the diff gets the whole width; ⌥↓ / ⌥↑ \
                  (flag 614) still step through the files.",
        ghd_behaviour: "The sidebar and file list always take their width.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(8456)],
        code: &[
            "crates/corvane-ui/src/workspace.rs",
            "crates/corvane-ui/src/selected_commit.rs",
            "crates/corvane/src/menus.rs",
        ],
    },

    /// Summary-only History rows.
    COMPACT_COMMIT_ROWS = 802 "compact-commit-rows" {
        title: "Compact History rows",
        summary: "History rows are 30 px tall and show the commit summary only, without the avatar, \
                  author and time line (the commit's details pane still has them).",
        ghd_behaviour: "50 px rows with an avatar + \"author • time\" line under the summary.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15956)],
        code: &["crates/corvane-ui/src/history.rs"],
    },

    /// A mark on history rows whose commit has a description.
    COMMIT_BODY_INDICATOR = 803 "commit-body-indicator" {
        title: "Mark commits that have a description",
        summary: "A History row whose commit message has a description (extended body) shows a \
                  ⋯ mark after the summary.",
        ghd_behaviour: "Only the summary; the description is seen by selecting the commit.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20401)],
        code: &["crates/corvane-ui/src/history.rs"],
    },

    /// Inline code and autolinks in commit messages.
    COMMIT_MESSAGE_RICH_TEXT = 804 "commit-message-rich-text" {
        title: "Inline code and links in commit messages",
        summary: "The selected commit's title and description show `backtick` spans as inline code, \
                  link URLs inside punctuation and, in a GitHub repository, commit SHAs.",
        ghd_behaviour: "Backticks show literally; SHAs are plain text; a URL is linked only as a \
                        whole word.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18104), Upstream::issue(7723)],
        code: &["crates/corvane-ui/src/selected_commit.rs", "crates/corvane-core/src/markdown.rs"],
    },

    /// The commit details show the date and link the SHA.
    COMMIT_DETAILS_EXTRAS = 805 "commit-details-extras" {
        title: "Commit date and SHA link in the commit details",
        summary: "The selected commit's details show the author date and time (the relative time on \
                  hover), and in a GitHub repository the SHA opens the commit on GitHub.",
        ghd_behaviour: "Author, SHA and line counts only; the SHA is plain text.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20715), Upstream::issue(3785)],
        code: &["crates/corvane-ui/src/selected_commit.rs"],
    },

    /// Tag tooltips in History.
    TAGS_TOOLTIP = 806 "tags-tooltip" {
        title: "Tag tooltips in History",
        summary: "Hovering a commit's tag pill in the History list, or the tag list in the commit's \
                  details, shows every tag, one per line.",
        ghd_behaviour: "The pill shows the first tag and a sliver for the rest; a truncated tag list \
                        cannot be read.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(9687)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-ui/src/selected_commit.rs"],
    },

    /// History's first-parent toggle.
    HISTORY_FIRST_PARENT = 807 "history-first-parent" {
        title: "First-parent History",
        summary: "A filter button before \"Select Branch to Compare…\" switches the History list to \
                  first parents only (git log --first-parent), hiding the commits that came in \
                  through merges. The choice is remembered.",
        ghd_behaviour: "History always lists every commit reachable from HEAD.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21414)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/log.rs"],
    },

    /// The history list scrolls to the top when the branch changes.
    HISTORY_SCROLLS_TO_TOP_ON_BRANCH_CHANGE = 808 "history-scrolls-to-top-on-branch-change" {
        title: "History scrolls to the top on branch change",
        summary: "Switching branch (or repository) scrolls the History list back to the newest \
                  commit.",
        ghd_behaviour: "The list keeps its scroll offset, so another branch's history opens \
                        somewhere in the middle.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20849), Upstream::issue(6706)],
        code: &["crates/corvane-ui/src/history.rs"],
    },

    /// History context menus: Copy Commit Title / Message / URL, Copy SHAs.
    HISTORY_COPY_ITEMS = 809 "history-copy-items" {
        title: "History: copy commit title, message, URL and SHAs",
        summary: "A commit's context menu adds Copy Commit Title, Copy Commit Message and (for \
                  GitHub repositories) Copy Commit URL next to Copy SHA; a multi-commit selection's \
                  menu adds Copy SHAs (newest first, one per line).",
        ghd_behaviour: "Copy SHA and Copy Tag only; nothing to copy for a multi-commit selection.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12547), Upstream::issue(20853), Upstream::issue(7518), Upstream::issue(9791), Upstream::issue(21061)],
        code: &["crates/corvane-ui/src/history.rs"],
    },

    /// Multi-select in a commit's file list.
    COMMIT_FILES_MULTI_SELECT = 810 "commit-files-multi-select" {
        title: "Multi-select a commit's files",
        summary: "The History file list selects several files with ⌘-click and ⇧-click; right-clicking \
                  the selection offers Copy File Paths and Copy Relative File Paths (one per line). \
                  The diff shows the last clicked file.",
        ghd_behaviour: "One file at a time.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15525), Upstream::issue(20467)],
        code: &["crates/corvane-ui/src/selected_commit.rs"],
    },

    /// History › Open with Default Program opens the file as of the commit.
    OPEN_HISTORICAL_FILE = 811 "open-historical-file" {
        title: "History opens the commit's version of a file",
        summary: "Open with Default Program in a commit's file list opens the file as it is in \
                  that commit (a read-only copy in the temporary directory), not the working copy.",
        ghd_behaviour: "Opens the file in the working directory, whatever its current state.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21117)],
        code: &["crates/corvane-ui/src/selected_commit.rs", "crates/corvane-core/src/integrations.rs"],
    },

    /// Copy path items for a commit's file that is gone from disk.
    COPY_PATH_OF_MISSING_FILE = 812 "copy-path-of-missing-file" {
        title: "Copy the path of a file missing on disk",
        summary: "In a commit's file list, a file that no longer exists on disk still offers Copy \
                  File Path and Copy Relative File Path under \"File Does Not Exist on Disk\".",
        ghd_behaviour: "Only the disabled \"File Does Not Exist on Disk\" item.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18349)],
        code: &["crates/corvane-ui/src/selected_commit.rs"],
    },

    /// Line totals for a multi-commit selection.
    MULTI_COMMIT_LINE_TOTALS = 813 "multi-commit-line-totals" {
        title: "Line totals for a multi-commit selection",
        summary: "Selecting several commits shows the range's added and removed line totals next \
                  to \"Showing changes from N commits\".",
        ghd_behaviour: "Only the commit count.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17869)],
        code: &["crates/corvane-ui/src/selected_commit.rs"],
    },

    /// Revert one file of a commit.
    REVERT_FILE_IN_COMMIT = 814 "revert-file-in-commit" {
        title: "Revert one file of a commit",
        summary: "A commit's file menu adds Revert Changes to This File: that file's changes from the \
                  commit are undone in the working directory (nothing is committed). A working file \
                  that has changed since in the same places is left alone and the error says so.",
        ghd_behaviour: "Only whole commits can be reverted.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19207)],
        code: &["crates/corvane-ui/src/selected_commit.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/history_ops.rs"],
    },

    /// Revert without committing, for one commit or a multi-selection.
    REVERT_WITHOUT_COMMITTING = 815 "revert-without-committing" {
        title: "Revert without committing",
        summary: "A commit's context menu adds Revert Changes in Commit Without Committing, and a \
                  multi-commit selection's menu Revert Changes in N Commits Without Committing: \
                  git revert --no-commit, newest first, leaves the combined inverse staged in \
                  Changes. Needs a clean working directory; a conflict rolls everything back.",
        ghd_behaviour: "Reverts one commit at a time, each as its own commit.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17278), Upstream::issue(9967)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/history_ops.rs"],
    },

    /// History "Push Up to This Commit".
    PUSH_UP_TO_COMMIT = 816 "push-up-to-commit" {
        title: "Push up to a commit",
        summary: "A commit's context menu adds Push Up to This Commit, enabled on the current \
                  branch's unpushed commits: it pushes that commit (and the ones before it) to the \
                  upstream branch and keeps the newer ones local. Unpushed tags stay behind.",
        ghd_behaviour: "Push always pushes the whole branch.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19238), Upstream::issue(20670)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/remote.rs"],
    },

    /// Checkout Commit on the branch tip.
    CHECKOUT_HEAD_COMMIT = 817 "checkout-head-commit" {
        title: "Checkout Commit on the latest commit",
        summary: "A commit's Checkout Commit is also enabled on the current branch's latest commit, \
                  detaching HEAD there.",
        ghd_behaviour: "Disabled on the latest commit, so HEAD cannot be detached at the branch tip.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22495)],
        code: &["crates/corvane-ui/src/history.rs"],
    },

    /// Undo Commit warns only when local changes touch the commit's files.
    UNDO_WARNS_ONLY_ON_OVERLAP = 818 "undo-warns-only-on-overlap" {
        title: "Undo Commit warns only about overlapping changes",
        summary: "The \"changes in progress\" warning before Undo Commit appears only when a file \
                  with local changes is one the commit touched.",
        ghd_behaviour: "Warns whenever there are any local changes, although undoing never loses \
                        changes to files the commit did not touch.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18388)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },

    /// Undo Commit warns about the commit's tags.
    WARN_UNDO_TAGGED_COMMIT = 819 "warn-undo-tagged-commit" {
        title: "Warn before undoing a tagged commit",
        summary: "Undo Commit on a commit that has tags asks first: the tags would stay on a commit \
                  that is no longer on any branch.",
        ghd_behaviour: "Undoes silently; the tags keep pointing at the orphaned commit.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19844)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-ui/src/dialogs/history_dialogs.rs", "crates/corvane-ui/src/changes.rs"],
    },

    /// History › Cherry-pick Without Committing.
    CHERRY_PICK_WITHOUT_COMMITTING = 820 "cherry-pick-without-committing" {
        title: "Cherry-pick without committing",
        summary: "A commit's menu (and a multi-commit selection's) adds Cherry-pick Commit Without \
                  Committing: the changes are applied to the current branch and left staged in \
                  Changes. Needs a clean working directory; a conflict undoes everything.",
        ghd_behaviour: "Cherry-picking always commits, onto a branch picked in a dialog.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21383)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/history_ops.rs"],
    },

    /// History › Create Patch File….
    CREATE_PATCH_FILES = 821 "create-patch-files" {
        title: "Create patch files from commits",
        summary: "A commit's menu (and a multi-commit selection's) adds Create Patch File…: after \
                  a folder is picked, git format-patch writes one numbered .patch file per commit \
                  there and the first is shown in Finder.",
        ghd_behaviour: "No way to export commits as patches.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20935)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/history_ops.rs"],
    },

    /// Cherry-pick / squash / reorder without a branch explain themselves.
    NO_BRANCH_EXPLAINED = 822 "no-branch-explained" {
        title: "Explain why cherry-pick, squash and reorder cannot start",
        summary: "Cherry-pick, squash and reorder on a detached HEAD or during a rebase show an \
                  error saying so.",
        ghd_behaviour: "Nothing happens.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18715), Upstream::issue(20982)],
        code: &["crates/corvane-core/src/mco.rs"],
    },

    /// Create a Tag's Message field.
    TAG_MESSAGE = 823 "tag-message" {
        title: "Tag message",
        summary: "Create a Tag has an optional Message field; the annotated tag carries it as \
                  typed.",
        ghd_behaviour: "Annotated tags always get an empty message.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12995), Upstream::issue(22890)],
        code: &["crates/corvane-ui/src/dialogs/history_dialogs.rs", "crates/corvane-git/src/history_ops.rs"],
    },

    /// ⌘⏎ submits Create a Tag.
    CMD_ENTER_SUBMITS_CREATE_TAG = 824 "cmd-enter-submits-create-tag" {
        title: "⌘⏎ creates the tag",
        summary: "In Create a Tag, ⌘⏎ creates the tag from the Message field as well as from Name \
                  (where ⏎ does too).",
        ghd_behaviour: "Only ⏎ in the Name field submits; ⌘⏎ does nothing.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(22740)],
        code: &["crates/corvane-ui/src/dialogs/history_dialogs.rs"],
    },

    /// Tags in the compare list.
    COMPARE_TAGS = 825 "compare-tags" {
        title: "Compare to a tag",
        summary: "Typing in \"Select Branch to Compare…\" also lists the matching tags, under Tags \
                  after the branches; picking one compares the current branch with it as with a \
                  branch.",
        ghd_behaviour: "Branches only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15702)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-core/src/compare.rs", "crates/corvane-git/src/log.rs"],
    },

    /// Delete pushed tags, optionally from the remote.
    DELETE_PUSHED_TAGS = 826 "delete-pushed-tags" {
        title: "Delete pushed tags",
        summary: "A commit's Delete tag items are enabled for every tag, not only ones created here \
                  and not pushed yet. Those others ask first, with an unticked option to delete the \
                  tag from the remote too (git push <remote> --delete).",
        ghd_behaviour: "Only tags created in the app and not yet pushed can be deleted.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15858)],
        code: &["crates/corvane-ui/src/history.rs", "crates/corvane-ui/src/dialogs/history_dialogs.rs", "crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// Squash message: keep only the target's.
    SQUASH_KEEP_TARGET_MESSAGE = 827 "squash-keep-target-message" {
        title: "Squash: use only the target commit's message",
        summary: "The squash message dialog has a \"Use only the target commit's message\" link \
                  that replaces the combined description with the summary and description of the \
                  commit squashed onto.",
        ghd_behaviour: "The combined message (target description plus every squashed commit's \
                        message) has to be trimmed by hand.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20507)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs", "crates/corvane-ui/src/dialogs/mod.rs"],
    },

    /// A failed squash keeps its message.
    SQUASH_KEEPS_DRAFT = 828 "squash-keeps-draft" {
        title: "Squash remembers its message after an error",
        summary: "When a squash fails, the message typed for it is kept: squashing the same \
                  commits again opens the dialog with that message instead of the combined one.",
        ghd_behaviour: "The message is lost; the next try starts from the combined messages again.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(20764)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-core/src/state.rs"],
    },

    /// Squash with local changes: git stashes them around it.
    SQUASH_AUTOSTASH = 829 "squash-autostash" {
        title: "Squash with uncommitted changes",
        summary: "Squashing with uncommitted changes runs the rebase with --autostash: git \
                  stashes the changes first and puts them back afterwards. If they no longer \
                  apply, they stay in git's stash and an error says so.",
        ghd_behaviour: "Asks to stash the changes first (Stash Changes and Continue) and leaves \
                        them stashed.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(12759)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-git/src/rebase_ops.rs"],
    },

    /// Squash / reorder keep the History selection.
    SELECT_REWRITTEN_COMMITS = 830 "select-rewritten-commits" {
        title: "History keeps the selection through squash and reorder",
        summary: "After a squash or reorder that rewrote the selected commits, History selects \
                  the squashed commit (or the moved commits) under their new SHAs.",
        ghd_behaviour: "The selection jumps to the newest commit.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12549)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// The Rebase dialog starts on the default branch.
    REBASE_PRESELECTS_DEFAULT_BRANCH = 831 "rebase-preselects-default-branch" {
        title: "Rebase dialog preselects the default branch",
        summary: "Branch › Rebase Current Branch… opens with the default branch selected and its \
                  preview shown, unless the default branch is the current one.",
        ghd_behaviour: "The current branch shows as selected, and a branch has to be picked first.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17731)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },

    /// Rebase onto `origin/main`.
    REBASE_ONTO_REMOTE_BRANCH = 832 "rebase-onto-remote-branch" {
        title: "Rebase onto a remote branch",
        summary: "The Rebase dialog's list ends with Remote Branches: the remote-tracking \
                  branches of local branches (origin/main next to main), so a branch can be \
                  rebased onto what was last fetched without checking out and pulling the local \
                  branch first.",
        ghd_behaviour: "A remote branch that has a local branch is not listed; only the local one \
                        can be picked.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13994)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs", "crates/corvane-ui/src/branch_list.rs"],
    },

    /// Rebase with local changes: stash, then rebase.
    REBASE_STASH_AND_CONTINUE = 833 "rebase-stash-and-continue" {
        title: "Rebase: Stash Changes and Continue rebases",
        summary: "Rebasing with uncommitted changes first offers to stash them; Stash Changes and \
                  Continue stashes and then runs the rebase.",
        ghd_behaviour: "Stash Changes and Continue stashes the changes and stops; the rebase has to \
                        be started again.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21904)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-core/src/state.rs"],
    },

    /// Rebase / squash / reorder keep `#` message lines.
    REBASE_KEEPS_HASH_MESSAGES = 834 "rebase-keeps-hash-messages" {
        title: "Rebase keeps commit messages that start with #",
        summary: "Rebase, squash and reorder keep commit message lines starting with # (a \
                  \"#123 Fix\" summary, say) when a commit stopped on conflicts is continued, and \
                  git's conflict notes stay out of the message.",
        ghd_behaviour: "Continuing after a conflict drops every line starting with #; a summary \
                        starting with # leaves the message empty and the rebase fails.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(16444)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-git/src/rebase_ops.rs"],
    },

    /// A rebase found in progress names its base branch.
    REBASE_BASE_NAME_RESOLVED = 835 "rebase-base-name-resolved" {
        title: "Rebase found in progress names its base branch",
        summary: "For a rebase that stopped on conflicts outside Corvane (or before a restart), the \
                  branch at the commit being rebased onto is looked up, so the conflicts dialog's \
                  choices read \"from main\" and the success banner names the base.",
        ghd_behaviour: "The base side is unnamed (\"Use the modified file\").",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(8113)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-git/src/rebase_ops.rs"],
    },

    /// Cherry-pick keeps the picked message after a conflict.
    CHERRY_PICK_KEEPS_MESSAGES = 836 "cherry-pick-keeps-messages" {
        title: "Cherry-pick keeps the commit message after a conflict",
        summary: "A cherry-picked commit that stopped on conflicts keeps its message as written \
                  when continued: lines starting with # stay, and git's \"Conflicts:\" note is \
                  not added.",
        ghd_behaviour: "Continuing can fail with \"Aborting commit due to empty commit message\"; \
                        lines starting with # are dropped.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21685)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-git/src/rebase_ops.rs"],
    },

    /// Squash and merge asks for the commit message.
    SQUASH_MERGE_MESSAGE = 837 "squash-merge-message" {
        title: "Squash and merge: commit message",
        summary: "The Squash and Merge dialog has summary and description fields above its \
                  button. With a summary, the squashed commit gets that message; left empty, \
                  git's \"Squashed commit of the following\" list as before.",
        ghd_behaviour: "The squashed commit always gets git's \"Squashed commit of the following\" \
                        list of the merged commits.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21718)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs", "crates/corvane-core/src/mco.rs", "crates/corvane-git/src/branch_ops.rs"],
    },

    /// No new merge or rebase while the repository is conflicted.
    NO_MERGE_WHILE_CONFLICTED = 838 "no-merge-while-conflicted" {
        title: "No merge while conflicted",
        summary: "While a merge, rebase or cherry-pick still has conflicts, the History tab's merge \
                  button is disabled and Branch › Merge, Squash and Merge, Rebase and Update from \
                  Default Branch explain that it must be finished or aborted first.",
        ghd_behaviour: "Starts the new operation, which git refuses with an error.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(6429), Upstream::issue(6584)],
        code: &["crates/corvane-core/src/mco.rs", "crates/corvane-core/src/compare.rs", "crates/corvane/src/main.rs", "crates/corvane-ui/src/history.rs"],
    },

    /// Conflicts dialog › Resolve All ▾.
    RESOLVE_ALL_CONFLICTS = 839 "resolve-all-conflicts" {
        title: "Resolve all conflicts using one side",
        summary: "With two or more conflicted files, the conflicts dialog has a Resolve All menu \
                  that picks one branch's version for every file still in conflict. Like the \
                  per-file choice it is applied on Continue, and each file keeps its Undo.",
        ghd_behaviour: "One side is picked file by file.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12377), Upstream::issue(15829), Upstream::issue(22516)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs", "crates/corvane-core/src/mco.rs"],
    },

    /// Conflicted file menu › Copy File Path.
    CONFLICT_MENU_COPY_PATHS = 840 "conflict-menu-copy-paths" {
        title: "Conflicts dialog: copy file paths",
        summary: "A conflicted file's ▾ menu in the conflicts dialog adds Copy File Path and Copy \
                  Relative File Path, as in the changes list.",
        ghd_behaviour: "Open with Default Program, Reveal in Finder and the resolution choices only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22399)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },

    /// Conflicts dialog names the stopped commit.
    CONFLICTS_SHOW_CURRENT_COMMIT = 841 "conflicts-show-current-commit" {
        title: "Conflicts dialog shows the stopped commit",
        summary: "While a rebase, squash or reorder waits on conflicts, the conflicts \
                  dialog starts with \"Commit N of M:\" and that commit's summary, as the \
                  progress dialog showed it.",
        ghd_behaviour: "The conflicts dialog does not say which commit is being applied.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18796)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },

    /// Conflicted file › Open in Merge Tool.
    OPEN_IN_MERGE_TOOL = 842 "open-in-merge-tool" {
        title: "Open conflicts in your merge tool",
        summary: "A conflicted file's ▾ menu in the conflicts dialog starts with Open in Merge \
                  Tool: git mergetool runs the tool set in git's merge.tool (Beyond Compare, \
                  kdiff3, …) on the file, and the list refreshes when it closes.",
        ghd_behaviour: "Only the editor, the default program or Finder.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(9609)],
        code: &["crates/corvane-ui/src/dialogs/mco_dialogs.rs", "crates/corvane-core/src/mco.rs", "crates/corvane-git/src/rebase_ops.rs"],
    },

    /// Create a Branch can start from any branch.
    CREATE_BRANCH_FROM_ANY_BRANCH = 843 "create-branch-from-any-branch" {
        title: "Create a branch from any branch",
        summary: "Create a Branch offers \"Other branch…\" next to the default and current \
                  branches, with a filterable list of every local and remote branch to start from.",
        ghd_behaviour: "Only the default branch or the current branch (and no choice at all while \
                        the default branch is checked out).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12459), Upstream::issue(20083)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },

    /// Create a Branch starts from the current branch while there are changes.
    CREATE_BRANCH_WITH_CHANGES_FROM_CURRENT = 844 "create-branch-with-changes-from-current" {
        title: "New branch with uncommitted changes starts from the current branch",
        summary: "While the working directory has uncommitted changes, Create a Branch preselects \
                  the current branch as the starting point, so the changes brought along apply \
                  to the code they were written against.",
        ghd_behaviour: "Always preselects the default branch; bringing the changes onto it can \
                        conflict or appear to lose work.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(9670)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },

    /// Create a Branch prefills a prefix.
    BRANCH_NAME_PREFIX = 845 "branch-name-prefix" {
        title: "Branch name prefix",
        summary: "Text Create a Branch puts in front of the suggested name (for example \
                  \"feature/\" or \"yourname/\"); empty for none.",
        ghd_behaviour: "No prefix.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "feature/", validate: branch_name_prefix },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), max: Value::text(""),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14004)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },

    /// Create / Rename Branch refuse `head` in any case.
    REJECT_HEAD_BRANCH_NAME = 846 "reject-head-branch-name" {
        title: "Branches can't be named \"head\"",
        summary: "Create a Branch and Rename Branch refuse \"head\" in any letter case, which on \
                  a case-insensitive file system names HEAD itself.",
        ghd_behaviour: "Creates the branch; HEAD ends up detached and the branch can't be published.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13638)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },

    /// Branch › New Branch… prefills the branch filter's text.
    NEW_BRANCH_FROM_FILTER = 847 "new-branch-from-filter" {
        title: "New Branch shortcut uses the branch filter",
        summary: "Branch › New Branch… (⌘⇧N) while the branch list is open prefills the name with \
                  its filter text, like the list's New Branch button.",
        ghd_behaviour: "The shortcut always opens Create a Branch with an empty name.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5199)],
        code: &["crates/corvane/src/main.rs", "crates/corvane-ui/src/workspace.rs", "crates/corvane-ui/src/branch_list.rs"],
    },

    /// Other Branches sorted by last update.
    BRANCH_LIST_SORT_BY_DATE = 848 "branch-list-sort-by-date" {
        title: "Branch lists sort other branches by date",
        summary: "Other Branches in the branch list and the branch pickers (merge, rebase, \
                  compare, pull request base, new branch) are ordered by the tip commit's date, \
                  most recently updated first.",
        ghd_behaviour: "Sorted by name only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19903), Upstream::issue(21358), Upstream::issue(5155)],
        code: &["crates/corvane-ui/src/branch_list.rs"],
    },

    /// The branch filter ignores an `owner:` prefix.
    BRANCH_FILTER_STRIPS_OWNER = 849 "branch-filter-strips-owner" {
        title: "Branch filter understands owner:branch",
        summary: "Pasting GitHub's owner:branch form of a branch name into the branch list's filter \
                  finds the branch (the owner: part is ignored).",
        ghd_behaviour: "Filters for the whole text, so the branch is not found.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(7424)],
        code: &["crates/corvane-ui/src/branch_list.rs"],
    },

    /// Branch rows show where the branch lives.
    BRANCH_LIST_LOCAL_REMOTE_ICONS = 850 "branch-list-local-remote-icons" {
        title: "Branch list icons for local-only and remote branches",
        summary: "In the branch list a branch with no upstream (only on this computer) shows a \
                  desktop icon and a branch that exists only on the remote shows a server icon; \
                  tracked local branches keep the branch icon.",
        ghd_behaviour: "Every branch shows the same branch icon.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17012), Upstream::issue(22019)],
        code: &["crates/corvane-ui/src/branch_list.rs"],
    },

    /// Branch list toggle: remote branches only.
    BRANCH_LIST_REMOTE_ONLY = 851 "branch-list-remote-only" {
        title: "Branch list can show only remote branches",
        summary: "A server button beside the branch list's filter narrows the list to the remote \
                  branches (including those checked out locally), in one Remote Branches group.",
        ghd_behaviour: "Remote branches are only listed, under Other Branches, when there is no \
                        local branch of the same name.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14134)],
        code: &["crates/corvane-ui/src/branch_list.rs"],
    },

    /// Mark local branches whose upstream was deleted on the remote.
    BRANCH_UPSTREAM_GONE = 852 "branch-upstream-gone" {
        title: "Mark branches deleted on the remote",
        summary: "Local branches whose upstream branch was deleted on the remote (and pruned by \
                  a fetch) show a cloud icon after their name in the branch list, so merged \
                  branches are easy to spot and clean up.",
        ghd_behaviour: "Nothing tells such branches apart.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20897)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/branch_ops.rs"],
    },

    /// Ahead/behind counts and "not published" in branch list rows.
    BRANCH_LIST_AHEAD_BEHIND = 853 "branch-list-ahead-behind" {
        title: "Push / pull state in the branch list",
        summary: "Local branch rows show how many commits they have to push and pull (\"2↑ 1↓\") \
                  against their upstream, or an upload icon when the branch was never published.",
        ghd_behaviour: "Only the current branch's state shows, on the toolbar's push / pull button.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5330)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// A stash icon on branch rows that have a stash.
    BRANCH_LIST_STASH_ICON = 854 "branch-list-stash-icon" {
        title: "Stash icon in the branch list",
        summary: "Local branches with stashed changes (a GitHub Desktop or Corvane stash) show the \
                  stash icon after their name in the branch list.",
        ghd_behaviour: "A branch's stash is only visible after switching to it.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17198)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// How many branches the branch list's Recent group shows.
    RECENT_BRANCHES_COUNT = 855 "recent-branches-count" {
        title: "Recent branches shown",
        summary: "How many recently checked-out branches the branch list shows in its Recent \
                  group (0 hides the group).",
        ghd_behaviour: "Always 5.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 50, unit: None },
        corvane: Value::Number(5), ghd: Value::Number(5),
        familiar: Value::Number(5), max: Value::Number(10),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14311)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },

    /// Branch context menu: Rebase Current Branch onto <branch>….
    BRANCH_MENU_REBASE_ONTO = 856 "branch-menu-rebase-onto" {
        title: "Rebase onto a branch from the branch list",
        summary: "A branch's context menu in the branch list offers \"Rebase Current Branch onto \
                  <branch>…\", which opens the rebase dialog with that branch selected.",
        ghd_behaviour: "Rebasing starts from Branch › Rebase Current Branch… only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21657)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-core/src/mco.rs", "crates/corvane-ui/src/dialogs/mco_dialogs.rs"],
    },

    /// Branch context menu: fast-forward a branch that is not checked out.
    UPDATE_BRANCH_FROM_UPSTREAM = 857 "update-branch-from-upstream" {
        title: "Update a branch from its upstream",
        summary: "The branch list's context menu offers \"Update from origin/…\" on local branches \
                  that are not checked out: the branch is fast-forwarded to its upstream without \
                  switching to it (a diverged branch is left alone with an explanation).",
        ghd_behaviour: "No such command; the branch has to be checked out and pulled.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19837)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    /// Update from Default Branch fetches and merges the remote-tracking branch.
    UPDATE_FROM_DEFAULT_FETCHES = 858 "update-from-default-fetches" {
        title: "Update from the default branch's remote",
        summary: "Branch › Update from Default Branch fetches the default branch's remote first \
                  and merges its remote-tracking branch (origin/main), so the latest commits on \
                  the remote are brought in even when the local default branch is behind.",
        ghd_behaviour: "Merges the local default branch as it is, which may be behind its remote.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(13709), Upstream::issue(19559), Upstream::issue(21545)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/remote.rs"],
    },

    /// Update from Default Branch rebases when pull.rebase is set.
    UPDATE_FROM_DEFAULT_REBASES = 859 "update-from-default-rebases" {
        title: "Update from Default Branch follows pull.rebase",
        summary: "When git config sets pull.rebase, Branch › Update from Default Branch rebases \
                  the current branch onto the default branch (with the usual force-push warning \
                  and conflict flow) instead of merging it in.",
        ghd_behaviour: "Always merges the default branch in.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(7956), Upstream::issue(16131)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/mco.rs"],
    },

    /// Delete Branch warns about unmerged commits and a stash.
    DELETE_BRANCH_WARNINGS = 860 "delete-branch-warnings" {
        title: "Delete Branch warns about unmerged commits and stashes",
        summary: "Delete Branch warns when the branch has commits that neither the default branch \
                  nor the branch's upstream contain, and when changes are stashed on it.",
        ghd_behaviour: "Only \"This action cannot be undone.\"",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(4214), Upstream::issue(13714)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/branch_ops.rs"],
    },

    /// Undo banner after deleting a local branch.
    UNDO_DELETE_BRANCH = 861 "undo-delete-branch" {
        title: "Undo deleting a branch",
        summary: "Deleting a local branch shows a \"Deleted branch\" banner for 15 seconds whose \
                  Undo recreates the branch at the commit it pointed at (without its upstream; a \
                  branch deleted on the remote too stays deleted there).",
        ghd_behaviour: "Deleted branches can only be recovered from the reflog on the command line.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20750)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/mco.rs", "crates/corvane-ui/src/banner.rs"],
    },

    /// Fetch after deleting the checked-out branch.
    FETCH_AFTER_DELETING_CURRENT_BRANCH = 862 "fetch-after-deleting-current-branch" {
        title: "Fetch after deleting the current branch",
        summary: "Deleting the checked-out branch switches to the default branch and then fetches \
                  its remote in the background, so the commits of a just-merged pull request \
                  show up without a manual Fetch.",
        ghd_behaviour: "Switches to the default branch without fetching.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15984)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },

    /// A clear error when a branch is checked out in another worktree.
    EXPLAIN_BRANCH_IN_OTHER_WORKTREE = 863 "explain-branch-in-other-worktree" {
        title: "Explain branches checked out in another worktree",
        summary: "When deleting a branch fails because it, or the default branch Corvane would \
                  switch to, is checked out in another worktree, the error names that worktree \
                  and says what to switch first.",
        ghd_behaviour: "Shows git's \"used by worktree at\" errors, one after another.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22569)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/error.rs"],
    },

    /// Confirm before switching branch from the branch list.
    CONFIRM_BRANCH_SWITCH = 864 "confirm-branch-switch" {
        title: "Confirm before switching branches",
        summary: "Clicking a branch in the branch list asks \"Switch to <branch>?\" before \
                  checking it out.",
        ghd_behaviour: "Checks the branch out at once.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: OFF,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20410)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-ui/src/dialogs/branch_dialogs.rs"],
    },

    /// Switch Branch › Discard my changes.
    SWITCH_BRANCH_DISCARD = 865 "switch-branch-discard" {
        title: "Switch Branch can discard changes",
        summary: "The Switch Branch dialog (shown for uncommitted changes) offers a third choice, \
                  \"Discard my changes\": its \"Discard Changes and Switch\" button discards \
                  every change (new files go to the Trash) and then switches.",
        ghd_behaviour: "Leave the changes in a stash or bring them along only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11491)],
        code: &["crates/corvane-ui/src/dialogs/branch_dialogs.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    // ---- 900 Performance ----

    /// Diffs of neighbouring files and commits computed ahead of time.
    PREFETCH_DIFFS = 901 "prefetch-diffs" {
        title: "Prefetch diffs",
        summary: "While a file or commit is shown, the diffs of the files and commits next to it \
                  are computed in the background, so moving the selection with the arrow keys \
                  shows the next diff at once. Costs a few extra git processes per selection.",
        ghd_behaviour: "Runs git for a diff when its file or commit is selected.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/diff_cache.rs"],
    },

    /// The watcher refreshes on the first change of a burst.
    FS_WATCHER_LEADING_EDGE = 902 "fs-watcher-leading-edge" {
        title: "Filesystem watcher: refresh at the first change",
        summary: "A change made outside Corvane (a save in the editor) refreshes at once instead of \
                  after the watcher's debounce; a burst of changes still ends with one refresh \
                  after it settles.",
        ghd_behaviour: "No watcher.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/watcher.rs"],
    },

    /// Save refreshed stat data after a slow status.
    REFRESH_STALE_INDEX = 903 "refresh-stale-index" {
        title: "Refresh the index after a slow status",
        summary: "When git status took over half a second, Corvane runs git update-index --refresh \
                  once (at most once a minute), so git stops re-reading files whose timestamps \
                  changed without their contents (a copied or restored checkout, a formatter): \
                  status on a 50,000-file tree went from 6 s to 0.2 s. It briefly holds \
                  index.lock, like any git command that writes the index.",
        ghd_behaviour: "Runs status without ever writing the index, so such a tree stays slow \
                        until a git command run elsewhere refreshes it.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: true, availability: available,
        upstream: &[],
        code: &["crates/corvane-git/src/status.rs", "crates/corvane-core/src/dispatcher.rs"],
    },

    /// The watcher's debounce.
    FS_WATCHER_DEBOUNCE_MS = 904 "fs-watcher-debounce-ms" {
        title: "Filesystem watcher debounce",
        summary: "How long the watcher waits after the last change before refreshing.",
        ghd_behaviour: "No watcher.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 50, max: 5000, unit: Some("ms") },
        corvane: Value::Number(300), ghd: Value::Number(300),
        familiar: Value::Number(300), max: Value::Number(300),
        restart: false, visible: false, availability: available,
        upstream: &[],
        code: &["crates/corvane-core/src/watcher.rs"],
    },

    /// The Git LFS check reads .gitattributes instead of `git lfs track`.
    LFS_DETECT_BY_ATTRIBUTES = 905 "lfs-detect-by-attributes" {
        title: "Fast Git LFS detection",
        summary: "Whether a newly added repository uses Git LFS is decided from its committed \
                  .gitattributes files (plus the root one and info/attributes) instead of \
                  `git lfs track`, which walks every directory, untracked ones included.",
        ghd_behaviour: "Runs `git lfs track --json`, which can take minutes in a worktree with \
                        many untracked files.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, max: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(5198)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-git/src/remote_ops.rs"],
    },

    // ---- 1000 Experimental ----
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
                assert!(
                    def.corvane != Value::Bool(true) || def.max == Value::Bool(true),
                    "{}: on in Corvane, so on in Max",
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
