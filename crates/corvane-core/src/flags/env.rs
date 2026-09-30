//! `CORVANE_FLAGS`: flag values for one session, on top of the stored preset
//! and overrides, never persisted. The flags it names are shown locked in
//! the dialog. The parity harness launches Corvane with
//! `CORVANE_FLAGS=preset=github-desktop`.
//!
//! Grammar (entries separated by commas, whitespace around them ignored):
//!
//! ```text
//! preset=github-desktop|familiar|corvane|everything
//! 201-commit-templates=off      key = `NNN-slug`, `slug` or `NNN`
//! fs-watcher                    a bare toggle key means `on`
//! 203=250                       numbers, `select` option values and text as is
//! ```
//!
//! Toggle values: `on|off|true|false|1|0|yes|no`. Text values cannot contain
//! a comma. Every bad entry yields one message and the rest still apply.

use std::collections::BTreeMap;

use super::{FlagId, FlagOverrides, Kind, Preset, Value, by_slug, lookup};

pub const VAR: &str = "CORVANE_FLAGS";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnvFlags {
    pub preset: Option<Preset>,
    pub values: BTreeMap<FlagId, Value>,
}

impl EnvFlags {
    pub fn is_empty(&self) -> bool {
        self.preset.is_none() && self.values.is_empty()
    }
}

/// Parse a spec; the messages describe the entries that were skipped.
pub fn parse(spec: &str) -> (EnvFlags, Vec<String>) {
    let mut env = EnvFlags::default();
    let mut errors = Vec::new();
    for entry in spec.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (key, value) = match entry.split_once('=') {
            Some((k, v)) => (k.trim(), Some(v.trim())),
            None => (entry, None),
        };
        if key.eq_ignore_ascii_case("preset") {
            match value.and_then(Preset::parse) {
                Some(preset) => env.preset = Some(preset),
                None => errors.push(format!(
                    "{VAR}: `{entry}`: preset must be one of {}",
                    Preset::ALL
                        .iter()
                        .map(|p| p.slug())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
            continue;
        }
        let Some(def) = lookup(key) else {
            errors.push(format!("{VAR}: unknown flag `{key}`"));
            continue;
        };
        let parsed = match (value, &def.kind) {
            (None, Kind::Bool) => Ok(Value::Bool(true)),
            (None, _) => Err("needs a value".to_string()),
            (Some(v), kind) => kind.parse(v),
        };
        match parsed {
            Ok(value) => {
                env.values.insert(def.id, value);
            }
            Err(err) => errors.push(format!("{VAR}: {}: {err}", def.ident())),
        }
    }
    (env, errors)
}

/// Read and parse `CORVANE_FLAGS` (empty when unset).
pub fn from_env() -> (EnvFlags, Vec<String>) {
    match std::env::var(VAR) {
        Ok(spec) => parse(&spec),
        Err(_) => (EnvFlags::default(), Vec::new()),
    }
}

/// The inverse: a spec that reproduces the stored preset and overrides
/// (unknown slugs are skipped). `preset=corvane,201-commit-templates=off`.
pub fn render(overrides: &FlagOverrides) -> String {
    let mut entries = vec![format!("preset={}", overrides.preset.slug())];
    let mut known: Vec<_> = overrides
        .overrides
        .iter()
        .filter_map(|(slug, value)| by_slug(slug).map(|def| (def, value)))
        .collect();
    known.sort_by_key(|(def, _)| def.id);
    entries.extend(
        known
            .into_iter()
            .map(|(def, value)| format!("{}={}", def.ident(), value.render())),
    );
    entries.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::ids;

    #[test]
    fn parses_every_key_form_and_value_spelling() {
        let (env, errors) = parse(
            " preset=github-desktop , 201-commit-templates=off, fs-watcher ,203=250,\
             302-pr-quick-view-width=min-400, product-name=Foo Bar,,",
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(env.preset, Some(Preset::GitHubDesktop));
        assert_eq!(env.values[&ids::COMMIT_TEMPLATES], Value::Bool(false));
        assert_eq!(env.values[&ids::FS_WATCHER], Value::Bool(true));
        assert_eq!(env.values[&ids::FS_WATCHER_DEBOUNCE_MS], Value::Number(250));
        assert_eq!(
            env.values[&ids::PR_QUICK_VIEW_WIDTH],
            Value::text("min-400")
        );
        assert_eq!(env.values[&ids::PRODUCT_NAME], Value::text("Foo Bar"));
    }

    #[test]
    fn bad_entries_are_reported_and_skipped() {
        let (env, errors) = parse(
            "preset=custom,bogus=1,203=9000,201-commit-templates=maybe,203,commit-templates=on",
        );
        assert_eq!(env.preset, None);
        assert_eq!(env.values.len(), 1);
        assert_eq!(env.values[&ids::COMMIT_TEMPLATES], Value::Bool(true));
        assert_eq!(errors.len(), 5, "{errors:?}");
        assert!(errors[0].contains("preset must be one of"));
        assert!(errors[1].contains("unknown flag `bogus`"));
        assert!(errors[2].contains("203-fs-watcher-debounce-ms: 9000 is outside"));
        assert!(errors[3].contains("is not on or off"));
        assert!(errors[4].contains("needs a value"));
    }

    #[test]
    fn empty_spec_is_empty() {
        let (env, errors) = parse("  ");
        assert!(env.is_empty());
        assert!(errors.is_empty());
    }

    #[test]
    fn render_round_trips() {
        let mut stored = FlagOverrides {
            preset: Preset::Familiar,
            ..Default::default()
        };
        stored
            .overrides
            .insert("pr-quick-view-width".into(), Value::text("min-400"));
        stored
            .overrides
            .insert("commit-templates".into(), Value::Bool(false));
        stored
            .overrides
            .insert("from-the-future".into(), Value::Bool(true));
        let spec = render(&stored);
        assert_eq!(
            spec,
            "preset=familiar,201-commit-templates=off,302-pr-quick-view-width=min-400"
        );
        let (env, errors) = parse(&spec);
        assert!(errors.is_empty());
        assert_eq!(env.preset, Some(Preset::Familiar));
        assert_eq!(env.values.len(), 2);
    }
}
