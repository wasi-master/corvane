//! Feature flags - a Corvane addition with no GitHub Desktop equivalent.
//!
//! Every deviation from GitHub Desktop 3.6.6 that can be switched off, and
//! every Corvane-only extra, is a flag in [`registry::REGISTRY`]: a numeric id
//! in a category block (`100` Appearance, `200` Repository, `300` GitHub,
//! `400` Window & menus, `500` Settings & updates, `600` Keyboard &
//! accessibility, `700` Changes & diffs, `800` History & branches, `900`
//! Experimental) plus a slug, shown as `201-commit-templates`. Flags are
//! toggles, selects, numbers or free text, and "on" always means Corvane's
//! deviation is active, so the **GitHub Desktop** preset turns every flag to
//! its GHD-exact value.
//!
//! Resolution: a [`Preset`] is the base layer, the user's per-flag overrides
//! sit on top (persisted under the store key `flags` as [`FlagOverrides`]),
//! and `CORVANE_FLAGS` (see [`env`]) overrides both for one session and locks
//! the flags it names. The resolved snapshot is [`Flags`] on `AppState`;
//! views read `state.flags` and re-render through the usual observe. Lower
//! crates (`corvane-git`, `corvane-platform`) never see flags: the layer that
//! has a `cx` passes them the value.
//!
//! Adding a flag: next id in its category block (ids are never reused, see
//! [`registry::RETIRED`]), its [`Nature`] (bug fix or feature), a value for
//! every preset, then
//! `UPDATE_FLAGS_DOC=1 cargo test -p corvane-core flags_doc` regenerates
//! `.docs/flags.md`.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

use corvane_models::{SyntaxHighlighter, ThemeSetting};
use serde::{Deserialize, Serialize};
use tracing::warn;

mod dispatch;
pub mod doc;
pub mod env;
pub mod presets;
pub mod registry;

pub use dispatch::ImportReport;
pub use env::EnvFlags;
pub use presets::Preset;
pub use registry::{REGISTRY, RETIRED, by_slug, def, find, ids, lookup};

/// A flag's number: the hundreds digit is its category block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FlagId(pub u16);

impl FlagId {
    pub fn category(self) -> Category {
        Category::of_block(self.0 / 100).unwrap_or(Category::Experimental)
    }

    /// `201-commit-templates`
    pub fn display(self) -> String {
        match find(self) {
            Some(def) => format!("{}-{}", self.0, def.slug),
            None => self.0.to_string(),
        }
    }
}

impl fmt::Display for FlagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

/// The category blocks, derived from the id (never stored separately).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Appearance,
    Repository,
    GitHub,
    WindowAndMenus,
    SettingsAndUpdates,
    Accessibility,
    ChangesAndDiffs,
    HistoryAndBranches,
    Experimental,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Appearance,
        Category::Repository,
        Category::GitHub,
        Category::WindowAndMenus,
        Category::SettingsAndUpdates,
        Category::Accessibility,
        Category::ChangesAndDiffs,
        Category::HistoryAndBranches,
        Category::Experimental,
    ];

    /// The first id of the block (`200` for Repository).
    pub const fn block(self) -> u16 {
        match self {
            Category::Appearance => 100,
            Category::Repository => 200,
            Category::GitHub => 300,
            Category::WindowAndMenus => 400,
            Category::SettingsAndUpdates => 500,
            Category::Accessibility => 600,
            Category::ChangesAndDiffs => 700,
            Category::HistoryAndBranches => 800,
            Category::Experimental => 900,
        }
    }

    /// `of_block(2)` is Repository.
    pub const fn of_block(hundreds: u16) -> Option<Category> {
        Some(match hundreds {
            1 => Category::Appearance,
            2 => Category::Repository,
            3 => Category::GitHub,
            4 => Category::WindowAndMenus,
            5 => Category::SettingsAndUpdates,
            6 => Category::Accessibility,
            7 => Category::ChangesAndDiffs,
            8 => Category::HistoryAndBranches,
            9 => Category::Experimental,
            _ => return None,
        })
    }

    pub const fn title(self) -> &'static str {
        match self {
            Category::Appearance => "Appearance",
            Category::Repository => "Repository",
            Category::GitHub => "GitHub",
            Category::WindowAndMenus => "Window & menus",
            Category::SettingsAndUpdates => "Settings & updates",
            Category::Accessibility => "Keyboard & accessibility",
            Category::ChangesAndDiffs => "Changes & diffs",
            Category::HistoryAndBranches => "History & branches",
            Category::Experimental => "Experimental",
        }
    }
}

/// A flag's value. Persisted untagged (`true`, `400`, `"github-desktop"`);
/// a Select stores its option's value as `Text`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Number(i64),
    Text(Cow<'static, str>),
}

impl Value {
    pub const fn text(s: &'static str) -> Value {
        Value::Text(Cow::Borrowed(s))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<i64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    /// The `CORVANE_FLAGS` spelling: `on` / `off`, the number, the text.
    pub fn render(&self) -> String {
        match self {
            Value::Bool(true) => "on".to_string(),
            Value::Bool(false) => "off".to_string(),
            Value::Number(n) => n.to_string(),
            Value::Text(s) => s.to_string(),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Number(n)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Text(Cow::Owned(s))
    }
}

/// One choice of a Select flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectOption {
    /// Stored value and `CORVANE_FLAGS` spelling (`min-400`).
    pub value: &'static str,
    /// Shown in the dialog.
    pub label: &'static str,
}

/// What kind of control a flag is, with its constraints.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Bool,
    Select {
        options: &'static [SelectOption],
    },
    Number {
        min: i64,
        max: i64,
        /// Shown after the field (`ms`).
        unit: Option<&'static str>,
    },
    Text {
        placeholder: &'static str,
        validate: fn(&str) -> Result<(), &'static str>,
    },
}

impl Kind {
    pub const fn name(&self) -> &'static str {
        match self {
            Kind::Bool => "toggle",
            Kind::Select { .. } => "select",
            Kind::Number { .. } => "number",
            Kind::Text { .. } => "text",
        }
    }

    /// Whether `value` fits this kind (type, range, option list, validator).
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        match (self, value) {
            (Kind::Bool, Value::Bool(_)) => Ok(()),
            (Kind::Select { options }, Value::Text(s)) => {
                if options.iter().any(|o| o.value == s) {
                    Ok(())
                } else {
                    Err(format!(
                        "`{s}` is not one of {}",
                        options
                            .iter()
                            .map(|o| o.value)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                }
            }
            (Kind::Number { min, max, unit }, Value::Number(n)) => {
                if (*min..=*max).contains(n) {
                    Ok(())
                } else {
                    Err(format!(
                        "{n} is outside {min}–{max}{}",
                        unit.map(|u| format!(" {u}")).unwrap_or_default()
                    ))
                }
            }
            (Kind::Text { validate, .. }, Value::Text(s)) => validate(s).map_err(str::to_string),
            (kind, value) => Err(format!(
                "expected a {} value, got {}",
                kind.name(),
                match value {
                    Value::Bool(_) => "a toggle",
                    Value::Number(_) => "a number",
                    Value::Text(_) => "text",
                }
            )),
        }
    }

    /// Parse the `CORVANE_FLAGS` spelling of a value: `on|off|true|false|1|0|
    /// yes|no` for toggles, an integer, an option's value, or free text.
    pub fn parse(&self, s: &str) -> Result<Value, String> {
        let s = s.trim();
        let value = match self {
            Kind::Bool => match s.to_ascii_lowercase().as_str() {
                "on" | "true" | "1" | "yes" => Value::Bool(true),
                "off" | "false" | "0" | "no" => Value::Bool(false),
                _ => return Err(format!("`{s}` is not on or off")),
            },
            Kind::Number { .. } => Value::Number(
                s.parse::<i64>()
                    .map_err(|_| format!("`{s}` is not a whole number"))?,
            ),
            Kind::Select { .. } | Kind::Text { .. } => Value::Text(Cow::Owned(s.to_string())),
        };
        self.validate(&value)?;
        Ok(value)
    }
}

/// Whether this build can honour the flag at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    Available,
    /// The build compiled the behaviour in; the flag is shown disabled with
    /// this reason and resolves to its GHD value.
    BuiltIn(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpstreamKind {
    Issue,
    Pull,
}

/// A `desktop/desktop` issue or pull request the flag relates to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Upstream {
    pub kind: UpstreamKind,
    pub number: u32,
}

impl Upstream {
    pub const fn issue(number: u32) -> Upstream {
        Upstream {
            kind: UpstreamKind::Issue,
            number,
        }
    }

    pub const fn pull(number: u32) -> Upstream {
        Upstream {
            kind: UpstreamKind::Pull,
            number,
        }
    }

    pub fn url(&self) -> String {
        let kind = match self.kind {
            UpstreamKind::Issue => "issues",
            UpstreamKind::Pull => "pull",
        };
        format!("https://github.com/desktop/desktop/{kind}/{}", self.number)
    }

    /// `#8698`
    pub fn label(&self) -> String {
        format!("#{}", self.number)
    }
}

/// Whether a flag fixes something GitHub Desktop plainly gets wrong or adds
/// something people may reasonably want either way. An attribute, not a
/// category: the flag stays in its numbered block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nature {
    /// A new capability, option or look.
    Feature,
    /// GitHub Desktop's behaviour is simply wrong and nearly everyone wants
    /// the fix. The Flags dialog hides these unless "Show bug fixes" is
    /// ticked (display only: presets and `CORVANE_FLAGS` still apply).
    BugFix,
}

impl Nature {
    pub const fn label(self) -> &'static str {
        match self {
            Nature::Feature => "Feature",
            Nature::BugFix => "Bug fix",
        }
    }
}

/// One entry of the registry.
#[derive(Debug)]
pub struct FlagDef {
    pub id: FlagId,
    pub slug: &'static str,
    pub title: &'static str,
    /// What the flag does when on (Corvane's behaviour).
    pub summary: &'static str,
    /// What GitHub Desktop does instead (shown as "GitHub Desktop: …").
    pub ghd_behaviour: &'static str,
    pub nature: Nature,
    pub kind: Kind,
    /// Today's behaviour: the Corvane preset and the "(default)" marker.
    pub corvane: Value,
    /// GHD-exact behaviour: the GitHub Desktop preset.
    pub ghd: Value,
    /// GHD look with the invisible improvements: the Familiar preset.
    pub familiar: Value,
    /// Every extra on: the Everything preset.
    pub everything: Value,
    /// Only takes effect at the next launch (the dialog offers Relaunch).
    pub restart: bool,
    /// Changes something the user sees (as opposed to timing or wording in
    /// logs); drives the Familiar preset's rule.
    pub visible: bool,
    pub availability: fn() -> Availability,
    pub upstream: &'static [Upstream],
    /// The gating sites, for the generated docs.
    pub code: &'static [&'static str],
}

impl FlagDef {
    pub fn category(&self) -> Category {
        self.id.category()
    }

    /// `201-commit-templates`
    pub fn ident(&self) -> String {
        format!("{}-{}", self.id.0, self.slug)
    }

    pub fn availability(&self) -> Availability {
        (self.availability)()
    }

    pub fn is_available(&self) -> bool {
        self.availability() == Availability::Available
    }

    pub fn is_bug_fix(&self) -> bool {
        self.nature == Nature::BugFix
    }

    /// Whether the Flags dialog lists this flag (and counts it) given its
    /// "Show bug fixes" checkbox.
    pub fn is_shown(&self, show_bug_fixes: bool) -> bool {
        show_bug_fixes || !self.is_bug_fix()
    }

    pub fn value_for(&self, preset: Preset) -> &Value {
        match preset {
            Preset::GitHubDesktop => &self.ghd,
            Preset::Familiar => &self.familiar,
            Preset::Corvane => &self.corvane,
            Preset::Everything => &self.everything,
        }
    }

    /// The label of a Select value (`Fixed 400 px`), or the value's spelling.
    pub fn label_for(&self, value: &Value) -> String {
        if let (Kind::Select { options }, Value::Text(s)) = (&self.kind, value)
            && let Some(o) = options.iter().find(|o| o.value == s)
        {
            return o.label.to_string();
        }
        value.render()
    }
}

/// The persisted layer (store key `flags`): the chosen preset and the
/// per-flag overrides on top of it, keyed by slug so ids can be renumbered.
/// Unknown slugs are kept so a newer build's flags survive a downgrade.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FlagOverrides {
    pub preset: Preset,
    pub overrides: BTreeMap<String, Value>,
}

/// Where a resolved value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Preset,
    Override,
    Env,
    BuiltIn,
}

/// The resolved snapshot every view and dispatcher reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flags {
    values: BTreeMap<FlagId, Value>,
    sources: BTreeMap<FlagId, Source>,
    preset: Preset,
    preset_from_env: bool,
}

static OFF: Value = Value::Bool(false);

impl Default for Flags {
    fn default() -> Self {
        Flags::resolve(&FlagOverrides::default(), &EnvFlags::default())
    }
}

impl Flags {
    /// Preset base, then valid stored overrides, then `CORVANE_FLAGS`
    /// (locking); a flag this build cannot honour resolves to its GHD value.
    pub fn resolve(stored: &FlagOverrides, env: &EnvFlags) -> Flags {
        let preset = env.preset.unwrap_or(stored.preset);
        let mut values = BTreeMap::new();
        let mut sources = BTreeMap::new();
        for def in REGISTRY {
            let mut value = def.value_for(preset).clone();
            let mut source = Source::Preset;
            if let Some(over) = stored.overrides.get(def.slug) {
                match def.kind.validate(over) {
                    Ok(()) => {
                        value = over.clone();
                        source = Source::Override;
                    }
                    Err(err) => {
                        warn!(flag = %def.ident(), %err, "ignoring invalid flag override")
                    }
                }
            }
            if let Some(from_env) = env.values.get(&def.id) {
                value = from_env.clone();
                source = Source::Env;
            }
            if let Availability::BuiltIn(_) = def.availability() {
                value = def.ghd.clone();
                source = Source::BuiltIn;
            }
            values.insert(def.id, value);
            sources.insert(def.id, source);
        }
        Flags {
            values,
            sources,
            preset,
            preset_from_env: env.preset.is_some(),
        }
    }

    pub fn value(&self, id: FlagId) -> &Value {
        self.values.get(&id).unwrap_or(&OFF)
    }

    pub fn bool(&self, id: FlagId) -> bool {
        self.value(id).as_bool().unwrap_or(false)
    }

    pub fn number(&self, id: FlagId) -> i64 {
        self.value(id).as_number().unwrap_or(0)
    }

    /// A Select's chosen option value, or a Text flag's text.
    pub fn text(&self, id: FlagId) -> &str {
        self.value(id).as_text().unwrap_or("")
    }

    pub fn source(&self, id: FlagId) -> Source {
        self.sources.get(&id).copied().unwrap_or(Source::Preset)
    }

    /// The base preset (from `CORVANE_FLAGS` when it names one).
    pub fn preset(&self) -> Preset {
        self.preset
    }

    pub fn preset_from_env(&self) -> bool {
        self.preset_from_env
    }

    /// Set by `CORVANE_FLAGS` for this session: shown locked.
    pub fn is_env_locked(&self, id: FlagId) -> bool {
        self.source(id) == Source::Env
    }

    /// Differs from the preset through a stored override.
    pub fn is_overridden(&self, id: FlagId) -> bool {
        self.source(id) == Source::Override
    }

    pub fn is_available(&self, id: FlagId) -> bool {
        self.source(id) != Source::BuiltIn
    }

    /// "Custom": at least one override applied on top of the preset.
    pub fn is_custom(&self) -> bool {
        self.modified_count() > 0
    }

    pub fn modified_count(&self) -> usize {
        self.sources
            .values()
            .filter(|s| **s == Source::Override)
            .count()
    }

    /// [`Flags::modified_count`] over the flags the dialog shows.
    pub fn modified_count_shown(&self, show_bug_fixes: bool) -> usize {
        REGISTRY
            .iter()
            .filter(|def| def.is_shown(show_bug_fixes) && self.is_overridden(def.id))
            .count()
    }

    /// Restart-required flags whose value differs from the one at launch.
    pub fn restart_pending(&self, at_launch: &Flags) -> Vec<FlagId> {
        REGISTRY
            .iter()
            .filter(|def| def.restart && self.value(def.id) != at_launch.value(def.id))
            .map(|def| def.id)
            .collect()
    }
}

/// With `101-high-contrast-theme` off a saved High Contrast theme shows as
/// Dark (the setting itself is left alone, so turning the flag back on
/// restores it).
pub fn effective_theme(setting: ThemeSetting, high_contrast_allowed: bool) -> ThemeSetting {
    match setting {
        ThemeSetting::HighContrast if !high_contrast_allowed => ThemeSetting::Dark,
        other => other,
    }
}

/// With `105-tree-sitter-highlighting` off every file uses GitHub Desktop's
/// highlighter (the saved choice is kept for when the flag comes back on).
pub fn effective_syntax_highlighter(
    setting: SyntaxHighlighter,
    tree_sitter_allowed: bool,
) -> SyntaxHighlighter {
    if tree_sitter_allowed {
        setting
    } else {
        SyntaxHighlighter::GitHubDesktop
    }
}

/// GHD `InitialReadmeContents` with the product name (`103-product-name`).
pub fn initial_readme(product_name: &str) -> String {
    format!(
        "# Welcome to {product_name}!\n\n\
         This is your README. READMEs are where you can communicate what your project is and how \
         to use it.\n\n\
         Write your name on line 6, save it, and then head back to {product_name}.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_serde_is_untagged() {
        assert_eq!(serde_json::to_string(&Value::Bool(true)).unwrap(), "true");
        assert_eq!(serde_json::to_string(&Value::Number(400)).unwrap(), "400");
        assert_eq!(serde_json::to_string(&Value::text("x")).unwrap(), "\"x\"");
        assert_eq!(
            serde_json::from_str::<Value>("false").unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            serde_json::from_str::<Value>("7").unwrap(),
            Value::Number(7)
        );
        assert_eq!(
            serde_json::from_str::<Value>("\"y\"").unwrap(),
            Value::text("y")
        );
    }

    #[test]
    fn overrides_serde_round_trip_keeps_unknown_slugs() {
        let json = r#"{"preset":"github-desktop","overrides":{"commit-templates":true,"from-the-future":"x"}}"#;
        let stored: FlagOverrides = serde_json::from_str(json).unwrap();
        assert_eq!(stored.preset, Preset::GitHubDesktop);
        assert_eq!(stored.overrides.len(), 2);
        let back = serde_json::to_string(&stored).unwrap();
        assert_eq!(
            serde_json::from_str::<FlagOverrides>(&back).unwrap(),
            stored
        );
        assert_eq!(
            serde_json::from_str::<FlagOverrides>("{}").unwrap(),
            FlagOverrides::default()
        );
    }

    #[test]
    fn resolve_precedence_preset_override_env() {
        let mut stored = FlagOverrides::default();
        let flags = Flags::resolve(&stored, &EnvFlags::default());
        assert!(flags.bool(ids::COMMIT_TEMPLATES));
        assert_eq!(flags.source(ids::COMMIT_TEMPLATES), Source::Preset);
        assert!(!flags.is_custom());

        stored.preset = Preset::GitHubDesktop;
        let flags = Flags::resolve(&stored, &EnvFlags::default());
        assert!(!flags.bool(ids::COMMIT_TEMPLATES));
        assert_eq!(flags.text(ids::PRODUCT_NAME), "GitHub Desktop");

        stored
            .overrides
            .insert("commit-templates".into(), Value::Bool(true));
        let flags = Flags::resolve(&stored, &EnvFlags::default());
        assert!(flags.bool(ids::COMMIT_TEMPLATES));
        assert!(flags.is_overridden(ids::COMMIT_TEMPLATES));
        assert!(flags.is_custom());
        assert_eq!(flags.modified_count(), 1);

        let mut env = EnvFlags::default();
        env.values.insert(ids::COMMIT_TEMPLATES, Value::Bool(false));
        let flags = Flags::resolve(&stored, &env);
        assert!(!flags.bool(ids::COMMIT_TEMPLATES));
        assert!(flags.is_env_locked(ids::COMMIT_TEMPLATES));
        assert!(!flags.is_overridden(ids::COMMIT_TEMPLATES));
        assert_eq!(flags.preset(), Preset::GitHubDesktop);
        assert!(!flags.preset_from_env());

        env.preset = Some(Preset::Everything);
        let flags = Flags::resolve(&stored, &env);
        assert_eq!(flags.preset(), Preset::Everything);
        assert!(flags.preset_from_env());
    }

    #[test]
    fn invalid_and_unknown_overrides_are_ignored() {
        let mut stored = FlagOverrides::default();
        stored
            .overrides
            .insert("commit-templates".into(), Value::Number(3));
        stored
            .overrides
            .insert("fs-watcher-debounce-ms".into(), Value::Number(99_999));
        stored
            .overrides
            .insert("no-such-flag".into(), Value::Bool(false));
        let flags = Flags::resolve(&stored, &EnvFlags::default());
        assert!(flags.bool(ids::COMMIT_TEMPLATES));
        assert_eq!(flags.number(ids::FS_WATCHER_DEBOUNCE_MS), 300);
        assert!(!flags.is_custom());
    }

    #[test]
    fn restart_pending_compares_restart_flags_only() {
        let launch = Flags::default();
        let mut stored = FlagOverrides::default();
        stored
            .overrides
            .insert("commit-templates".into(), Value::Bool(false));
        assert!(
            Flags::resolve(&stored, &EnvFlags::default())
                .restart_pending(&launch)
                .is_empty()
        );
        stored
            .overrides
            .insert("optional-components".into(), Value::Bool(false));
        let now = Flags::resolve(&stored, &EnvFlags::default());
        if now.is_available(ids::OPTIONAL_COMPONENTS) {
            assert_eq!(now.restart_pending(&launch), vec![ids::OPTIONAL_COMPONENTS]);
        } else {
            assert!(now.restart_pending(&launch).is_empty());
        }
    }

    #[test]
    fn kind_parse_and_validate() {
        for (s, expect) in [
            ("on", true),
            ("TRUE", true),
            ("1", true),
            ("yes", true),
            ("off", false),
            ("0", false),
        ] {
            assert_eq!(Kind::Bool.parse(s).unwrap(), Value::Bool(expect), "{s}");
        }
        assert!(Kind::Bool.parse("maybe").is_err());
        let number = Kind::Number {
            min: 50,
            max: 5000,
            unit: Some("ms"),
        };
        assert_eq!(number.parse(" 300 ").unwrap(), Value::Number(300));
        assert!(number.parse("10").unwrap_err().contains("50–5000 ms"));
        assert!(number.parse("x").is_err());
        assert!(number.validate(&Value::Bool(true)).is_err());
        let select = def(ids::PR_QUICK_VIEW_WIDTH).kind;
        assert_eq!(select.parse("min-400").unwrap(), Value::text("min-400"));
        assert!(select.parse("huge").is_err());
        let text = def(ids::PRODUCT_NAME).kind;
        assert_eq!(text.parse("Foo").unwrap(), Value::text("Foo"));
        assert!(text.parse("").is_err());
        assert!(text.parse("a\nb").is_err());
    }

    #[test]
    fn bug_fixes_are_hidden_from_the_dialog_counts() {
        let bug_fixes = REGISTRY.iter().filter(|d| d.is_bug_fix()).count();
        assert!(bug_fixes > 0);
        assert!(bug_fixes < REGISTRY.len());
        assert_eq!(
            REGISTRY.iter().filter(|d| d.is_shown(true)).count(),
            REGISTRY.len()
        );
        assert_eq!(
            REGISTRY.iter().filter(|d| d.is_shown(false)).count(),
            REGISTRY.len() - bug_fixes
        );
        let fix = def(ids::PUSH_BRANCH_COMMITS_ERROR_STOPS);
        assert_eq!(fix.nature, Nature::BugFix);
        assert!(!fix.is_shown(false) && fix.is_shown(true));
        assert!(def(ids::COMMIT_TEMPLATES).is_shown(false));

        let mut stored = FlagOverrides::default();
        stored
            .overrides
            .insert("push-branch-commits-error-stops".into(), Value::Bool(false));
        stored
            .overrides
            .insert("commit-templates".into(), Value::Bool(false));
        let flags = Flags::resolve(&stored, &EnvFlags::default());
        // hiding is display-only: the override still resolves
        assert!(!flags.bool(ids::PUSH_BRANCH_COMMITS_ERROR_STOPS));
        assert_eq!(flags.modified_count(), 2);
        assert_eq!(flags.modified_count_shown(true), 2);
        assert_eq!(flags.modified_count_shown(false), 1);
    }

    #[test]
    fn effective_theme_hides_high_contrast() {
        assert_eq!(
            effective_theme(ThemeSetting::HighContrast, false),
            ThemeSetting::Dark
        );
        assert_eq!(
            effective_theme(ThemeSetting::HighContrast, true),
            ThemeSetting::HighContrast
        );
        assert_eq!(
            effective_theme(ThemeSetting::System, false),
            ThemeSetting::System
        );
    }

    #[test]
    fn effective_syntax_highlighter_needs_the_flag() {
        assert_eq!(
            effective_syntax_highlighter(SyntaxHighlighter::TreeSitter, false),
            SyntaxHighlighter::GitHubDesktop
        );
        assert_eq!(
            effective_syntax_highlighter(SyntaxHighlighter::TreeSitterFallback, true),
            SyntaxHighlighter::TreeSitterFallback
        );
    }

    #[test]
    fn initial_readme_uses_the_name() {
        let readme = initial_readme("GitHub Desktop");
        assert!(readme.starts_with("# Welcome to GitHub Desktop!"));
        assert!(readme.contains("head back to GitHub Desktop."));
        assert!(!readme.contains("Corvane"));
    }

    #[test]
    fn upstream_urls() {
        assert_eq!(
            Upstream::issue(8698).url(),
            "https://github.com/desktop/desktop/issues/8698"
        );
        assert_eq!(
            Upstream::pull(12).url(),
            "https://github.com/desktop/desktop/pull/12"
        );
        assert_eq!(Upstream::issue(1).label(), "#1");
    }
}
