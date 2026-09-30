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
    /// Spinner while a working-directory diff loads.
    DIFF_LOADING_INDICATOR = 107 "diff-loading-indicator" {
        title: "Diff loading spinner",
        summary: "When a changed file's diff takes more than 300 ms to compute (large files, slow \
                  filters), a spinner covers the diff pane until it is ready.",
        ghd_behaviour: "The previous diff (or an empty pane) stays up with no sign of work.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(1913)],
        code: &["crates/corvane-ui/src/diff_view.rs"],
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
    /// Create Repository warns before replacing an existing README.md.
    README_OVERWRITE_WARNING = 207 "readme-overwrite-warning" {
        title: "Create Repository warns about an existing README",
        summary: "With \"Initialize this repository with a README\" ticked and a README.md already \
                  in the folder, the Create a New Repository dialog warns that its content will be \
                  replaced.",
        ghd_behaviour: "The warning only exists in beta builds; release builds silently overwrite \
                        the README.md.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22471)],
        code: &["crates/corvane-ui/src/dialogs/create_repository.rs"],
    },
    /// Changes list: lines added / deleted per file and in total.
    CHANGES_LINE_COUNTS = 208 "changes-line-counts" {
        title: "Line counts in the Changes list",
        summary: "Each changed file shows the lines it adds and removes against the last commit \
                  (+N -M), and the \"N changed files\" header shows the totals of the listed \
                  files. Runs git diff --numstat on every refresh; untracked files over 1 MiB and \
                  binary files get no count.",
        ghd_behaviour: "No line counts for uncommitted changes (only the commit summary in History \
                        has them).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
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
    /// Ignore File / Folder / Extension skip rules already in .gitignore.
    IGNORE_SKIPS_EXISTING_RULES = 209 "ignore-skips-existing-rules" {
        title: "Ignore menu items don't duplicate .gitignore rules",
        summary: "\"Ignore File\", \"Ignore Folder\" and \"Ignore All .ext Files\" leave out \
                  patterns the root .gitignore already has as a line.",
        ghd_behaviour: "Appends the pattern again, so repeated use fills .gitignore with duplicate \
                        lines.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(2537)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/ignore.rs"],
    },
    /// Fetch / pull / push give up on a stalled HTTP transfer.
    NETWORK_STALL_TIMEOUT = 210 "network-stall-timeout" {
        title: "Give up on stalled fetch, pull and push",
        summary: "Seconds an HTTPS fetch, pull, push or clone may transfer nothing before git \
                  aborts it with an error (GIT_HTTP_LOW_SPEED_LIMIT=1 and \
                  GIT_HTTP_LOW_SPEED_TIME). 0 waits forever. Does not apply to SSH remotes.",
        ghd_behaviour: "No limit: a stalled connection leaves the operation spinning until the app \
                        is restarted.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 3600, unit: Some("s") },
        corvane: Value::Number(0), ghd: Value::Number(0),
        familiar: Value::Number(0), everything: Value::Number(60),
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(22863)],
        code: &["crates/corvane-core/src/remote.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/process.rs"],
    },
    /// `status.showUntrackedFiles=no` hides untracked files.
    RESPECT_SHOW_UNTRACKED_FILES = 211 "respect-show-untracked-files" {
        title: "Respect status.showUntrackedFiles",
        summary: "When the repository's git config sets status.showUntrackedFiles to no, the \
                  Changes list leaves untracked files out, as git status does (useful for a home \
                  directory or dotfiles repository). Untracked files then cannot be committed \
                  from Corvane until they are added with git.",
        ghd_behaviour: "Always lists every untracked file (--untracked-files=all).",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(3734)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/status.rs"],
    },
    /// Discarding a dirty submodule cleans inside it.
    DISCARD_SUBMODULE_CHANGES = 212 "discard-submodule-changes" {
        title: "Discard cleans changes inside submodules",
        summary: "Discarding a submodule that has changes inside checks out its modified files \
                  and moves its untracked files to the Trash, so the submodule is clean \
                  afterwards.",
        ghd_behaviour: "The submodule stays in the list: untracked files and edits inside it are \
                        not discarded.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(10403)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/commit.rs"],
    },
    /// Wiki repositories are plain git repositories, not GitHub ones.
    WIKI_NOT_GITHUB = 213 "wiki-not-github" {
        title: "Wiki repositories are not treated as GitHub repositories",
        summary: "A repository whose origin is a GitHub wiki (owner/name.wiki) is handled as a \
                  plain git repository, so Corvane does not ask the API for its pull requests, \
                  issues, collaborators and checks, which do not exist. Applies at launch and \
                  when a repository is added.",
        ghd_behaviour: "Treats the wiki as a GitHub repository and every API request for it fails.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: true, visible: true, availability: available,
        upstream: &[Upstream::issue(2061)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-models/src/lib.rs"],
    },
    /// Discard deletes files instead of moving them to the Trash.
    DISCARD_SKIPS_TRASH = 214 "discard-skips-trash" {
        title: "Discard deletes instead of using the Trash",
        summary: "Discarding changes deletes new and untracked files (and untracked files inside \
                  a discarded submodule) permanently instead of moving them to the Trash; the \
                  confirmation says they cannot be restored.",
        ghd_behaviour: "Always moves discarded files to the Trash.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(10445)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-ui/src/dialogs/discard_changes.rs"],
    },
    /// `git status --ignore-submodules`.
    IGNORE_SUBMODULES = 215 "ignore-submodules" {
        title: "Hide submodule changes",
        summary: "What the Changes list leaves out about submodules: nothing beyond each \
                  submodule's own submodule.<name>.ignore setting, changes inside submodules (a \
                  new submodule commit is still listed), or submodules altogether (a new \
                  submodule commit can then not be committed from Corvane).",
        ghd_behaviour: "As configured: only submodule.<name>.ignore hides a submodule.",
        nature: Nature::Feature,
        kind: Kind::Select { options: IGNORE_SUBMODULE_MODES },
        corvane: Value::text("configured"), ghd: Value::text("configured"),
        familiar: Value::text("configured"), everything: Value::text("dirty"),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20484)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/status.rs"],
    },
    /// Remote names containing `/` are matched whole.
    REMOTE_NAMES_WITH_SLASHES = 216 "remote-names-with-slashes" {
        title: "Remote names with slashes",
        summary: "A remote branch's remote is found by matching the configured remote names, so \
                  a remote called team/fork gives team/fork/main the branch name main (checkout, \
                  push, pull requests and the branch list use it).",
        ghd_behaviour: "Takes everything before the first / as the remote name (team), so the \
                        branch becomes fork/main.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(3618)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/repo.rs", "crates/corvane-models/src/lib.rs"],
    },
    /// "Ignore File In" submenu.
    IGNORE_FILE_TARGETS = 217 "ignore-file-targets" {
        title: "Choose the ignore file",
        summary: "A changed file's context menu adds Ignore File In: the .gitignore of a folder \
                  above the file (anchored to that folder), .git/info/exclude (this clone only) or \
                  the global excludes file (core.excludesFile, else ~/.config/git/ignore; the file \
                  name is ignored in every repository).",
        ghd_behaviour: "Ignore items always write to the .gitignore at the repository root.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12171), Upstream::issue(16028)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/ignore.rs"],
    },
    /// Repository Settings › Ignored Files › Edit global ignore file.
    EDIT_GLOBAL_IGNORE_FILE = 218 "edit-global-ignore-file" {
        title: "Edit the global ignore file",
        summary: "Repository Settings › Ignored Files has an \"Edit global ignore file\" link that \
                  opens git's excludes file (core.excludesFile, else ~/.config/git/ignore, created \
                  when missing) in the external editor.",
        ghd_behaviour: "Only the repository's root .gitignore can be edited.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21951)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs", "crates/corvane-core/src/dispatcher.rs"],
    },
    /// Update from Default Branch fetches and merges the remote-tracking branch.
    UPDATE_FROM_DEFAULT_FETCHES = 219 "update-from-default-fetches" {
        title: "Update from the default branch's remote",
        summary: "Branch › Update from Default Branch fetches the default branch's remote first \
                  and merges its remote-tracking branch (origin/main), so the latest commits on \
                  the remote are brought in even when the local default branch is behind.",
        ghd_behaviour: "Merges the local default branch as it is, which may be behind its remote.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(13709), Upstream::issue(19559), Upstream::issue(21545)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/remote.rs"],
    },
    /// Commit form gear › Push After Committing.
    COMMIT_AND_PUSH = 220 "commit-and-push" {
        title: "Push after committing",
        summary: "The commit form's gear menu adds Push After Committing (kept per repository); \
                  while it is ticked the button reads \"Commit and push to main\" and the branch \
                  is pushed (or published) once the commit, hooks included, succeeds. Push errors \
                  show as for the toolbar button; amended commits are not pushed.",
        ghd_behaviour: "Committing and pushing are separate steps.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(21874), Upstream::issue(22742)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs"],
    },
    /// New untracked files start unticked in the Changes list.
    NEW_UNTRACKED_FILES_EXCLUDED = 221 "new-untracked-files-excluded" {
        title: "New untracked files start unticked",
        summary: "An untracked file that appears in the Changes list starts unticked, so it is \
                  only committed once it is ticked; tracked changes are still included.",
        ghd_behaviour: "Every new file is ticked and goes into the next commit.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18774), Upstream::issue(21427)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },
    /// Update from Default Branch rebases when pull.rebase is set.
    UPDATE_FROM_DEFAULT_REBASES = 222 "update-from-default-rebases" {
        title: "Update from Default Branch follows pull.rebase",
        summary: "When git config sets pull.rebase, Branch › Update from Default Branch rebases \
                  the current branch onto the default branch (with the usual force-push warning \
                  and conflict flow) instead of merging it in.",
        ghd_behaviour: "Always merges the default branch in.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(7956), Upstream::issue(16131)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/mco.rs"],
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
    /// Force push is recommended after an amend only when the amended commit was pushed.
    AMEND_FORCE_PUSH_IF_PUSHED = 309 "amend-force-push-if-pushed" {
        title: "Force push only after amending a pushed commit",
        summary: "After amending, the toolbar recommends Force push only when the amended commit \
                  is on the branch's upstream; amending a commit that was never pushed on a branch \
                  that is also behind offers Pull, as before the amend.",
        ghd_behaviour: "Every amend makes Force push the recommended action once the branch has \
                        diverged, even when the amended commit was never pushed.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20526)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },
    /// Submodules are updated (and new ones initialised) after a checkout or merge.
    SUBMODULES_FOLLOW_CHECKOUT = 310 "submodules-follow-checkout" {
        title: "Update submodules after checkout and merge",
        summary: "After switching branches or a merge (including Update from Default Branch), \
                  submodules are checked out at the commits the branch records and new ones are \
                  cloned (git submodule update --init --recursive). Submodules that showed \
                  changes beforehand are left alone.",
        ghd_behaviour: "Submodules stay at their old commits (and new ones uninitialised), so \
                        they show as changed and are easily committed back.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(18302), Upstream::issue(18673), Upstream::issue(9547)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/mco.rs", "crates/corvane-git/src/remote_ops.rs"],
    },
    /// Undo banner after deleting a local branch.
    UNDO_DELETE_BRANCH = 311 "undo-delete-branch" {
        title: "Undo deleting a branch",
        summary: "Deleting a local branch shows a \"Deleted branch\" banner for 15 seconds whose \
                  Undo recreates the branch at the commit it pointed at (without its upstream; a \
                  branch deleted on the remote too stays deleted there).",
        ghd_behaviour: "Deleted branches can only be recovered from the reflog on the command line.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20750)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/mco.rs", "crates/corvane-ui/src/banner.rs"],
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
    /// A settings file that cannot be written is reported.
    REPORT_SETTINGS_SAVE_ERRORS = 412 "report-settings-save-errors" {
        title: "Report settings that could not be saved",
        summary: "When a changed setting cannot be written to disk, an error dialog says so \
                  (with the reason) instead of the change silently being lost on the next launch.",
        ghd_behaviour: "A failed save goes unnoticed; the setting reverts after a restart.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5046)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },
    /// How many branches the branch list's Recent group shows.
    RECENT_BRANCHES_COUNT = 413 "recent-branches-count" {
        title: "Recent branches shown",
        summary: "How many recently checked-out branches the branch list shows in its Recent \
                  group (0 hides the group).",
        ghd_behaviour: "Always 5.",
        nature: Nature::Feature,
        kind: Kind::Number { min: 0, max: 50, unit: None },
        corvane: Value::Number(5), ghd: Value::Number(5),
        familiar: Value::Number(5), everything: Value::Number(10),
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(14311)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },
    /// Fetch after deleting the checked-out branch.
    FETCH_AFTER_DELETING_CURRENT_BRANCH = 414 "fetch-after-deleting-current-branch" {
        title: "Fetch after deleting the current branch",
        summary: "Deleting the checked-out branch switches to the default branch and then fetches \
                  its remote in the background, so the commits of a just-merged pull request \
                  show up without a manual Fetch.",
        ghd_behaviour: "Switches to the default branch without fetching.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15984)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },
    /// A clear error when a branch is checked out in another worktree.
    EXPLAIN_BRANCH_IN_OTHER_WORKTREE = 415 "explain-branch-in-other-worktree" {
        title: "Explain branches checked out in another worktree",
        summary: "When deleting a branch fails because it, or the default branch Corvane would \
                  switch to, is checked out in another worktree, the error names that worktree \
                  and says what to switch first.",
        ghd_behaviour: "Shows git's \"used by worktree at\" errors, one after another.",
        nature: Nature::BugFix,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: ON, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(22569)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/error.rs"],
    },
    /// A spinner in the Changes header while discarding or refreshing.
    CHANGES_BUSY_INDICATOR = 416 "changes-busy-indicator" {
        title: "Changes list busy indicator",
        summary: "The \"N changed files\" row ends in a spinner while Discard Changes runs and \
                  while a status refresh has been running for more than 300 ms, so a slow \
                  discard or git status is visibly in progress.",
        ghd_behaviour: "Nothing shows that a discard or status refresh is still running.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(15297), Upstream::issue(1914)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/dispatcher.rs"],
    },
    /// Show the newest command-line stash when the branch has no Desktop stash.
    SHOW_LATEST_OTHER_STASH = 417 "show-latest-other-stash" {
        title: "Show stashes made outside Corvane",
        summary: "When the current branch has no stash of its own, the Changes list's Stashed \
                  Changes row shows the newest stash that GitHub Desktop or Corvane did not make \
                  (git stash on the command line), so it can be viewed, restored or discarded. \
                  Stashing from Corvane never replaces such a stash.",
        ghd_behaviour: "Only stashes named !!GitHub_Desktop<branch> are shown; others are invisible.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17147)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-core/src/state.rs"],
    },
    /// A stash icon on branch rows that have a stash.
    BRANCH_LIST_STASH_ICON = 418 "branch-list-stash-icon" {
        title: "Stash icon in the branch list",
        summary: "Local branches with stashed changes (a GitHub Desktop or Corvane stash) show the \
                  stash icon after their name in the branch list.",
        ghd_behaviour: "A branch's stash is only visible after switching to it.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17198)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-core/src/dispatcher.rs"],
    },
    /// "No local changes" offers restoring the branch's stash.
    RESTORE_STASH_SUGGESTION = 419 "restore-stash-suggestion" {
        title: "Restore stash from No local changes",
        summary: "When the branch has stashed changes and nothing else is changed, the \"No local \
                  changes\" view starts with a \"Restore your stashed changes\" card whose Restore \
                  button brings them back in one click.",
        ghd_behaviour: "The stash has to be opened from the bottom of the Changes tab first.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(12864)],
        code: &["crates/corvane-ui/src/workspace.rs"],
    },
    /// Restore a branch's stash when switching back to it with no changes.
    POP_STASH_ON_RETURN = 420 "pop-stash-on-return" {
        title: "Restore a branch's stash when returning to it",
        summary: "Switching to a branch that has stashed changes restores them (git stash pop) \
                  when the working directory is clean after the switch, e.g. when the changes on \
                  the branch being left were stashed there. A pop that conflicts keeps the stash \
                  and shows the conflicts.",
        ghd_behaviour: "The stash stays until Stashed Changes › Restore is clicked.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(17682)],
        code: &["crates/corvane-core/src/dispatcher.rs"],
    },
    /// Repository Settings › Git Config › Line endings (core.autocrlf).
    LINE_ENDINGS_SETTING = 421 "line-endings-setting" {
        title: "Line endings setting per repository",
        summary: "Repository Settings › Git Config adds \"Line endings (core.autocrlf)\": use the \
                  global config, or store true, input or false in the repository's own config. \
                  It applies to later checkouts and commits; files already checked out keep \
                  their line endings.",
        ghd_behaviour: "No line ending option; core.autocrlf has to be set on the command line.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5230)],
        code: &["crates/corvane-ui/src/dialogs/repository_settings.rs", "crates/corvane-core/src/integrations.rs"],
    },
    /// "Name <email>" co-authors without a GitHub account.
    FREE_FORM_CO_AUTHORS = 422 "free-form-co-authors" {
        title: "Co-authors without a GitHub account",
        summary: "Typing \"Name <email>\" in the co-authors box adds that person as a co-author \
                  (a Co-Authored-By trailer) without looking them up on GitHub. To allow spaces \
                  in names, Space turns a typed word into a GitHub handle only when it starts \
                  with @; the suggestions list works as before.",
        ghd_behaviour: "Only GitHub users can be added; every word becomes a handle on Space.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(4308)],
        code: &["crates/corvane-ui/src/changes.rs", "crates/corvane-core/src/autocomplete.rs"],
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
    /// Remove a left-over index.lock from the error dialog.
    REMOVE_STALE_INDEX_LOCK = 512 "remove-stale-index-lock" {
        title: "Remove a left-over index.lock",
        summary: "When git fails because .git/index.lock exists, the error explains it and offers \
                  Remove Lock File, which deletes the lock only when no Git process is running in \
                  the repository.",
        ghd_behaviour: "Shows git's error; the lock has to be deleted by hand.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(908)],
        code: &["crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/index_lock.rs", "crates/corvane-ui/src/dialogs/simple.rs"],
    },
    /// Mark local branches whose upstream was deleted on the remote.
    BRANCH_UPSTREAM_GONE = 513 "branch-upstream-gone" {
        title: "Mark branches deleted on the remote",
        summary: "Local branches whose upstream branch was deleted on the remote (and pruned by \
                  a fetch) show a cloud icon after their name in the branch list, so merged \
                  branches are easy to spot and clean up.",
        ghd_behaviour: "Nothing tells such branches apart.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: ON, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(20897)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-core/src/dispatcher.rs", "crates/corvane-git/src/branch_ops.rs"],
    },
    /// Ahead/behind counts and "not published" in branch list rows.
    BRANCH_LIST_AHEAD_BEHIND = 514 "branch-list-ahead-behind" {
        title: "Push / pull state in the branch list",
        summary: "Local branch rows show how many commits they have to push and pull (\"2↑ 1↓\") \
                  against their upstream, or an upload icon when the branch was never published.",
        ghd_behaviour: "Only the current branch's state shows, on the toolbar's push / pull button.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: true, availability: available,
        upstream: &[Upstream::issue(5330)],
        code: &["crates/corvane-ui/src/branch_list.rs", "crates/corvane-core/src/dispatcher.rs"],
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
    /// ⌘⌫ in the changes list discards the highlighted files.
    CMD_BACKSPACE_DISCARDS_FILES = 607 "cmd-backspace-discards-files" {
        title: "⌘⌫ in the changes list discards the selected files",
        summary: "With the changes list focused, ⌘⌫ discards the highlighted files (confirming \
                  as the context menu's Discard Changes does); elsewhere it still removes the \
                  repository.",
        ghd_behaviour: "⌘⌫ is Repository › Remove… everywhere, also in the changes list.",
        nature: Nature::Feature,
        kind: Kind::Bool,
        corvane: OFF, ghd: OFF, familiar: OFF, everything: ON,
        restart: false, visible: false, availability: available,
        upstream: &[Upstream::issue(11924), Upstream::issue(17680)],
        code: &["crates/corvane-ui/src/keymap.rs", "crates/corvane-ui/src/changes.rs"],
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
