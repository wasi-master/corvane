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
    /// File names without their directory in the changes list.
    CHANGES_FILE_NAMES_ONLY = 170 "changes-file-names-only" {
        title: "File names only in the changes list",
        summary: "Rows of the changes list show the file name alone, without its directory \
                  (the filter still matches the whole path).",
        ghd_behaviour: "Directory (dimmed) followed by the file name.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14268), Upstream::issue(19016)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },
    /// "Committing as" line above the commit summary.
    COMMIT_AUTHOR_LINE = 171 "commit-author-line" {
        title: "Show the commit author",
        summary: "A line above the commit summary names the identity git resolved for this \
                  repository (`user.name` / `user.email`, `includeIf` included): \
                  \"Committing as Name <email>\".",
        ghd_behaviour: "Only the avatar, whose tooltip names the author.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21883)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },
    /// ⇧-click keeps ⌘-clicked rows.
    SHIFT_CLICK_KEEPS_SELECTION = 172 "shift-click-keeps-selection" {
        title: "⇧-click keeps ⌘-clicked files",
        summary: "In the changes list, ⇧-click replaces only the range from the last clicked \
                  file; files ⌘-clicked outside it stay selected, as in Finder.",
        ghd_behaviour: "⇧-click selects the range alone and drops the other selected files.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16355)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/list_selection.rs"],
    },
    /// "The file mode changed" for a mode-only diff.
    FILE_MODE_CHANGE_MESSAGE = 173 "file-mode-change-message" {
        title: "Say when only the file mode changed",
        summary: "A diff whose only change is the file mode (e.g. the executable bit) says \
                  \"The file mode changed from 100644 to 100755\".",
        ghd_behaviour: "\"No content changes found\", or \"Only whitespace changes found\" while \
                        whitespace changes are hidden.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11685), Upstream::issue(557)],
        code: &["crates/corvane-git/src/diff.rs", "crates/corvane-ui/src/diff_view.rs"],
    },
    /// A renamed file's diff starts from HEAD.
    RENAMED_DIFF_AGAINST_HEAD = 174 "renamed-diff-against-head" {
        title: "Renamed files diff against the last commit",
        summary: "A renamed file's diff compares the old path in the last commit with the \
                  working copy, so edits staged outside Corvane show up too.",
        ghd_behaviour: "Compares the index with the working copy: a renamed file whose edits \
                        were staged (e.g. by `git add`) shows no changes.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19142), Upstream::issue(5575)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/diff.rs"],
    },
    /// Split mode shows added and deleted files unified.
    UNIFIED_DIFF_FOR_ADDED_FILES = 175 "unified-diff-for-added-files" {
        title: "Added and deleted files use the unified layout",
        summary: "With Diff Settings › Split selected, a new or deleted file is still shown \
                  unified, across the whole width, instead of beside an empty column.",
        ghd_behaviour: "Split mode draws a new file in the right half next to an empty left \
                        half (a deleted one the other way round).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13763), Upstream::issue(16610)],
        code: &["crates/corvane-ui/src/diff_view.rs"],
    },
    /// A symbolic link's contents are its target path.
    SYMLINK_CONTENTS = 176 "symlink-contents" {
        title: "Symbolic links are not followed",
        summary: "Loading a changed symbolic link reads the path it points to, as git records \
                  it, instead of the file behind it.",
        ghd_behaviour: "Reads the file the link points to for hunk expansion, so a link to a \
                        pipe or device keeps the diff loading forever and a link to a huge file \
                        loads it whole.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(18620)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/diff.rs"],
    },
    /// Intra-line highlights end on grapheme boundaries.
    INTRA_LINE_GRAPHEMES = 177 "intra-line-graphemes" {
        title: "Intra-line highlights keep accents with their letters",
        summary: "The changed part of a modified line is widened to whole characters as \
                  people see them (grapheme clusters), so a combining accent is highlighted \
                  together with its letter.",
        ghd_behaviour: "Compares UTF-16 code units, so the highlight can cut a combining mark off \
                        its base character and the mark renders apart or disappears.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(11492)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-ui/src/diff_view_rows.rs"],
    },
    /// Image diff borders go around the image.
    IMAGE_DIFF_BORDER_OUTSIDE = 178 "image-diff-border-outside" {
        title: "Image diff borders don't shrink the image",
        summary: "The coloured 1 px border of an image in the image diff is drawn around the \
                  image, which keeps its natural (or fitted) size.",
        ghd_behaviour: "The border is inside the image's box (`box-sizing: border-box`), so every \
                        image is drawn 2 px smaller than its size, blurring small images and \
                        pixel art.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14469)],
        code: &["crates/corvane-ui/src/image_diff.rs"],
    },
    /// The intra-line highlighting length cap.
    INTRA_LINE_MAX_LENGTH = 179 "intra-line-max-length" {
        title: "Longest line with intra-line highlighting",
        summary: "A modified line pair gets its changed characters highlighted only while both \
                  lines are shorter than this many bytes (0: no limit).",
        ghd_behaviour: "1024 (`MaxIntraLineDiffStringLength`), fixed; longer lines only show as \
                        wholly replaced.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 1_000_000, unit: Some("bytes") },
        corvane: Value::Number(1024), ghd: Value::Number(1024),
        familiar: Value::Number(1024), everything: Value::Number(0),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22556)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-ui/src/diff_view_rows.rs"],
    },
    /// Visible whitespace in diffs.
    DIFF_SHOW_WHITESPACE = 180 "diff-show-whitespace" {
        title: "Show whitespace in diffs",
        summary: "Diff lines mark every space with a faint dot and every tab with a faint line, \
                  so indentation and trailing whitespace changes can be told apart.",
        ghd_behaviour: "Whitespace is invisible.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12974)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-ui/src/diff_view_rows.rs"],
    },
    /// "Show the diff as text anyway" on binary files.
    BINARY_DIFF_AS_TEXT = 181 "binary-diff-as-text" {
        title: "Show binary files' diffs as text",
        summary: "A changed file git takes for binary (a stray NUL byte, an odd encoding) offers \
                  \"Show the diff as text anyway.\", which diffs it line by line with \
                  `git diff --text`. Its lines cannot be picked for a partial commit.",
        ghd_behaviour: "\"This binary file has changed.\" and a link to open it elsewhere.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16855)],
        code: &["crates/corvane-ui/src/diff_view.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/diff.rs"],
    },
    /// Diffs open with the whole file expanded.
    DIFF_EXPAND_WHOLE_FILE = 182 "diff-expand-whole-file" {
        title: "Expand the whole file in diffs",
        summary: "Every text diff opens as if \"Expand Whole File\" had been picked (files up \
                  to 20 000 lines; large diffs stay collapsed). \"Collapse Expanded Lines\" \
                  still collapses it.",
        ghd_behaviour: "Diffs open collapsed to their hunks; the expansion is per file and \
                        forgotten.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16140), Upstream::issue(20548)],
        code: &["crates/corvane-ui/src/diff_view.rs"],
    },
    /// The image diff's checkerboard.
    IMAGE_DIFF_BACKGROUND = 183 "image-diff-background" {
        title: "Image diff background",
        summary: "The checkerboard behind images in the image diff: light, dark, or dark while \
                  the app theme is dark, so light and translucent images stay visible.",
        ghd_behaviour: "Always the light checkerboard.",
        nature: Nature::Feature,
        kind: Kind::Select { options: IMAGE_DIFF_BACKGROUNDS },
        corvane: Value::text("light"), ghd: Value::text("light"),
        familiar: Value::text("light"), everything: Value::text("theme"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21092)],
        code: &["crates/corvane-ui/src/image_diff.rs"],
    },
    /// TGA images get an image diff.
    TGA_IMAGE_DIFF = 184 "tga-image-diff" {
        title: "Image diffs for TGA files",
        summary: "Changed `.tga` images (common in game assets) are shown in the image diff \
                  (2-up, Swipe, Onion Skin, Difference).",
        ghd_behaviour: "\"This binary file has changed.\"",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21970)],
        code: &["crates/corvane-ui/src/image_diff.rs", "crates/corvane-ui/src/diff_view.rs", "crates/corvane-models/src/lib.rs"],
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

    /// Commit form warning while HEAD is detached.
    DETACHED_HEAD_COMMIT_WARNING = 270 "detached-head-commit-warning" {
        title: "Warn when committing on a detached HEAD",
        summary: "While HEAD is detached the commit form shows a warning that the commit will not \
                  be on any branch, with a link to create one.",
        ghd_behaviour: "Commits on a detached HEAD without a word; the button reads \"Commit to\".",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(788)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },
    /// Open in editor / default program acts on every selected file.
    OPEN_MULTIPLE_FILES = 271 "open-multiple-files" {
        title: "Open several files at once",
        summary: "With several changed files selected, \"Open in <editor>\" and \"Open with Default \
                  Program\" open all of them; the changes list's context menu gains \"Open All in \
                  <editor>\" and a history file's menu \"Open All Files of Commit in <editor>\". \
                  At most 25 files at a time.",
        ghd_behaviour: "Opens only the right-clicked file.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16262), Upstream::issue(21374), Upstream::issue(15013)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-ui/src/selected_commit.rs"],
    },
    /// Glob patterns hidden from the changes list.
    CHANGES_HIDE_GLOBS = 272 "changes-hide-globs" {
        title: "Hide files from the changes list",
        summary: "Changed files matching these comma-separated glob patterns (gitignore-like: \
                  `*.lock`, `node_modules`, `/docs/**`) are left out of the changes list, which then \
                  reads \"N of M changed files\". View only: hidden files are still included in \
                  commits. Empty hides nothing.",
        ghd_behaviour: "Lists every changed file.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "*.lock, node_modules", validate: hide_globs },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), everything: Value::text(""),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(10093), Upstream::issue(20615), Upstream::issue(21242)],
        code: &["crates/corvane-core/src/filter.rs", "crates/corvane-ui/src/changes.rs"],
    },
    /// ↑ / ↓ in an empty commit summary recall recent commit messages.
    RECALL_COMMIT_MESSAGES = 273 "recall-commit-messages" {
        title: "Recall recent commit messages with ↑ / ↓",
        summary: "In an empty commit form, ↑ in the summary fills in the summary and description \
                  of the latest commit on the branch; more ↑ go further back (merges and repeated \
                  summaries skipped), ↓ comes forward and past the newest empties the form again. \
                  Editing the text keeps it.",
        ghd_behaviour: "↑ / ↓ only move the caret.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12927), Upstream::issue(20559), Upstream::issue(17525)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-models/src/lib.rs"],
    },
    /// "No local changes" links the branch's open pull request.
    NO_CHANGES_VIEW_PULL_REQUEST = 274 "no-changes-view-pull-request" {
        title: "\"View Pull Request\" when there are no local changes",
        summary: "While the current branch has an open pull request, the \"No local changes\" view \
                  leads with a \"View Pull Request\" card naming its number and title, opening it \
                  on GitHub.",
        ghd_behaviour: "Shows no pull request action while one is open (only Create / Preview \
                        Pull Request for a branch without one).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19329)],
        code: &["crates/corvane-ui/src/workspace.rs"],
    },
    /// Confirmation before committing on the default branch.
    CONFIRM_COMMIT_TO_DEFAULT_BRANCH = 275 "confirm-commit-to-default-branch" {
        title: "Confirm commits to the default branch",
        summary: "Committing (not amending) while the default branch is checked out asks \
                  \"Commit to Default Branch\" first.",
        ghd_behaviour: "Commits to the default branch without asking.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21857)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-ui/src/dialogs/confirm_commit_to_default_branch.rs"],
    },
    /// No Trash sentence when only submodules are discarded.
    DISCARD_SUBMODULE_NO_TRASH_HINT = 276 "discard-submodule-no-trash-hint" {
        title: "Discarding submodules does not mention the Trash",
        summary: "When every discarded entry is a submodule, the discard confirmation leaves out \
                  \"Changes can be restored by retrieving them from the Trash\": nothing is moved \
                  there.",
        ghd_behaviour: "Always promises the Trash.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(10402)],
        code: &["crates/corvane-ui/src/dialogs/discard_changes.rs"],
    },
    /// Hard 72-character limit on the commit summary.
    SUMMARY_MAX_LENGTH = 277 "summary-max-length" {
        title: "Limit the commit summary to 72 characters",
        summary: "The commit summary field takes at most 72 characters (GitHub truncates longer \
                  summaries); typing or pasting past the limit drops the excess, like an HTML \
                  maxlength.",
        ghd_behaviour: "No limit.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18290)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },
    /// Changed-file counts in the "Ignore All .x Files" items.
    IGNORE_MENU_COUNTS = 278 "ignore-menu-counts" {
        title: "Counts in \"Ignore All .x Files\"",
        summary: "The changes list's \"Ignore All .x Files\" context-menu items say how many \
                  changed files have that extension: \"Ignore All .png Files (170 Changed)\".",
        ghd_behaviour: "\"Ignore All .png Files (Add to .gitignore)\", no count.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(13789)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },
    /// "Copy Diff" in the changes list's file menu.
    COPY_DIFF = 279 "copy-diff" {
        title: "Copy Diff",
        summary: "The changes list's file context menu has \"Copy Diff\" (\"Copy Diff of Selected \
                  Files\" for a multi-selection): the working-directory changes of those files as \
                  a patch `git apply` takes, untracked files included.",
        ghd_behaviour: "No way to copy a diff.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17746)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/diff.rs"],
    },
    /// "Renamed files" in the changes list's Filter Options.
    RENAMED_FILES_FILTER = 280 "renamed-files-filter" {
        title: "\"Renamed files\" filter option",
        summary: "The changes list's Filter Options popover has a sixth option, \"Renamed files\".",
        ghd_behaviour: "Included / excluded, new, modified and deleted files only.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21147)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/filter.rs"],
    },
    /// "Copy" button on error dialogs.
    ERROR_DIALOG_COPY = 281 "error-dialog-copy" {
        title: "Copy button on error dialogs",
        summary: "Error dialogs (a failed commit, push, checkout…) have a \"Copy\" button that puts \
                  the title and message on the clipboard; the text itself cannot be selected.",
        ghd_behaviour: "No way to copy the message (⌘C does nothing).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19198), Upstream::issue(22591), Upstream::issue(22913)],
        code: &["crates/corvane-ui/src/dialogs/simple.rs"],
    },
    /// Order of the changes list.
    CHANGES_SORT_ORDER = 282 "changes-sort-order" {
        title: "Changes list order",
        summary: "How the changes list orders its files: by path, by status (conflicted, new, \
                  modified, renamed, deleted; path order within each), or by file name. A filter \
                  text still ranks its matches best first.",
        ghd_behaviour: "Path order (git's).",
        nature: Nature::Feature,
        kind: Kind::Select { options: CHANGES_SORT_ORDERS },
        corvane: Value::text("path"), ghd: Value::text("path"),
        familiar: Value::text("path"), everything: Value::text("status"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(4739)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/filter.rs"],
    },
    /// How the changes filter text matches.
    CHANGES_FILTER_MATCH = 283 "changes-filter-match" {
        title: "Changes filter matching",
        summary: "How the changes list's filter text matches a path: fuzzily (the letters in \
                  order), as a substring, as the end of the path (`.meta`), or as the exact path \
                  or file name. Case is ignored.",
        ghd_behaviour: "Fuzzy only.",
        nature: Nature::Feature,
        kind: Kind::Select { options: CHANGES_FILTER_MATCHES },
        corvane: Value::text("fuzzy"), ghd: Value::text("fuzzy"),
        familiar: Value::text("fuzzy"), everything: Value::text("substring"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20555)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/filter.rs"],
    },
    /// Warn about paths Windows cannot check out.
    WINDOWS_INVALID_NAMES_WARNING = 284 "windows-invalid-names-warning" {
        title: "Warn about names invalid on Windows",
        summary: "The commit form warns when an included file's path is invalid on Windows (a \
                  reserved name like `CON` or `nul.txt`, a character such as `:` or `?`, or a name \
                  ending in a space or a dot). Committing stays possible.",
        ghd_behaviour: "Commits them silently; Windows clones then fail to check them out.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(19292)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/portable_paths.rs"],
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

    /// "Assume Unchanged" in the changes list's menus.
    ASSUME_UNCHANGED = 470 "assume-unchanged" {
        title: "Assume Unchanged",
        summary: "The changes list's file menu has \"Assume Unchanged\" (`git update-index \
                  --assume-unchanged`) for modified or deleted tracked files, which then leave \
                  the list; the list's own menu has \"Stop Assuming Files Unchanged\" to bring \
                  them all back.",
        ghd_behaviour: "No such items; only ignoring (which does not affect tracked files).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22841)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/commit.rs"],
    },

    /// Clear the drafted message after a matching outside commit.
    CLEAR_MESSAGE_AFTER_OUTSIDE_COMMIT = 471 "clear-message-after-outside-commit" {
        title: "Clear the draft after an outside commit",
        summary: "When a new commit appears on the branch (made on the command line or in another \
                  app) whose summary is the one drafted in the commit form, the form is cleared \
                  as after committing in Corvane.",
        ghd_behaviour: "The drafted message stays.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5233)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// Context menu on the "Committed … Undo" bar.
    UNDO_BAR_MENU = 472 "undo-bar-menu" {
        title: "Context menu on the undo bar",
        summary: "Right-clicking the \"Committed just now … Undo\" bar under the commit button \
                  offers Amend Commit…, Undo Commit…, Create Tag…, Copy SHA and View on GitHub \
                  for that commit.",
        ghd_behaviour: "No context menu there; those items live in History.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(12561), Upstream::issue(19938)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// Optional tag field in the commit form.
    COMMIT_TAG_FIELD = 473 "commit-tag-field" {
        title: "Tag field in the commit form",
        summary: "The commit form has a \"Tag (optional)\" field under the description; a name \
                  there tags the new commit once it is made (not when amending).",
        ghd_behaviour: "Tags are created from History after committing.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(16256)],
        code: &["crates/corvane-ui/src/changes.rs"],
    },

    /// "Open With…" in the changes list's file menu.
    OPEN_FILE_WITH = 474 "open-file-with" {
        title: "Open a changed file with…",
        summary: "The changes list's file menu has \"Open With…\" after \"Open with Default \
                  Program\": pick any application to open the file in.",
        ghd_behaviour: "Only the configured editor or the default program.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20166)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/integrations.rs"],
    },

    /// VS Code opens the repository's workspace file.
    VSCODE_WORKSPACE_FILE = 475 "vscode-workspace-file" {
        title: "Open the VS Code workspace file",
        summary: "Opening the repository in Visual Studio Code (or VSCodium, Cursor, Windsurf) \
                  opens its `*.code-workspace` file when the repository's top folder has exactly \
                  one.",
        ghd_behaviour: "Always opens the folder.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(7007)],
        code: &["crates/corvane-core/src/integrations.rs", "crates/corvane-platform/src/editors.rs"],
    },

    /// Snooze the discard confirmation.
    DISCARD_CONFIRM_SNOOZE = 476 "discard-confirm-snooze" {
        title: "Snooze the discard confirmation",
        summary: "The Confirm Discard Changes dialog offers \"Do not show this message again for N \
                  minutes\": discarding in that repository then skips the confirmation for N \
                  minutes (this session; Discard All Changes still asks). 0 hides the option.",
        ghd_behaviour: "Only \"Do not show this message again\", for good.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 120, unit: Some("min") },
        corvane: Value::Number(0), ghd: Value::Number(0),
        familiar: Value::Number(0), everything: Value::Number(10),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20747)],
        code: &["crates/corvane-ui/src/dialogs/discard_changes.rs", "crates/corvane-core/src/dispatcher.rs"],
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

    /// Reveal in another file manager.
    FILE_MANAGER = 570 "file-manager" {
        title: "File manager",
        summary: "Application that Show in Finder and the Reveal in Finder items open the folder \
                  with (a file's parent folder), by name or path: `Path Finder`, \
                  `/Applications/ForkLift.app`. Empty uses Finder.",
        ghd_behaviour: "Always Finder.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "Path Finder", validate: app_name },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), everything: Value::text(""),
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(13812)],
        code: &["crates/corvane-core/src/integrations.rs"],
    },

    /// Open web links in a chosen browser.
    BROWSER = 571 "browser" {
        title: "Browser",
        summary: "Application that web links open in (GitHub pages, pull requests, sign-in, help \
                  links), by name or path: `Firefox`, `/Applications/Safari.app`. Empty uses the \
                  system's default browser.",
        ghd_behaviour: "Always the default browser.",
        nature: Nature::Feature,
        kind: Kind::Text { placeholder: "Firefox", validate: app_name },
        corvane: Value::text(""), ghd: Value::text(""),
        familiar: Value::text(""), everything: Value::text(""),
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(21762)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
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
    /// The diff's font size.
    DIFF_FONT_SIZE = 670 "diff-font-size" {
        title: "Diff font size",
        summary: "The size of the diff's monospace text, 9 to 16 pixels at 100 % zoom (0 \
                  keeps 11 px). Rows stay 20 px tall.",
        ghd_behaviour: "11 px, changed only by zooming the whole window.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 16, unit: Some("px") },
        corvane: Value::Number(0), ghd: Value::Number(0),
        familiar: Value::Number(0), everything: Value::Number(0),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22929)],
        code: &["crates/corvane-ui/src/diff_view.rs"],
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
