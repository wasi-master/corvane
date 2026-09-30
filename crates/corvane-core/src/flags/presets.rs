//! The built-in presets: the base layer every flag resolves from before the
//! user's overrides. "Custom" is not a preset, it is derived from having
//! overrides.

use serde::{Deserialize, Serialize};

use super::{FlagDef, Value};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Preset {
    /// Every flag at its GHD-exact value: a 1:1 GitHub Desktop.
    #[serde(rename = "github-desktop")]
    GitHubDesktop,
    /// GHD's look with the improvements nobody can see (smooth scrolling,
    /// commit templates, fork-before-push…).
    Familiar,
    /// Today's Corvane: every built deviation on.
    #[default]
    Corvane,
    /// Everything on, including extras that are off by default.
    Everything,
}

impl Preset {
    pub const ALL: [Preset; 4] = [
        Preset::GitHubDesktop,
        Preset::Familiar,
        Preset::Corvane,
        Preset::Everything,
    ];

    /// The `CORVANE_FLAGS` / JSON spelling.
    pub const fn slug(self) -> &'static str {
        match self {
            Preset::GitHubDesktop => "github-desktop",
            Preset::Familiar => "familiar",
            Preset::Corvane => "corvane",
            Preset::Everything => "everything",
        }
    }

    pub fn parse(s: &str) -> Option<Preset> {
        let s = s.trim().to_ascii_lowercase();
        Preset::ALL.into_iter().find(|p| p.slug() == s)
    }

    pub const fn title(self) -> &'static str {
        match self {
            Preset::GitHubDesktop => "GitHub Desktop",
            Preset::Familiar => "Familiar",
            Preset::Corvane => "Corvane",
            Preset::Everything => "Everything",
        }
    }

    /// One line for the dialog's preset picker.
    pub const fn description(self) -> &'static str {
        match self {
            Preset::GitHubDesktop => "Behaves exactly like GitHub Desktop 3.6.6.",
            Preset::Familiar => "Looks like GitHub Desktop, keeps the improvements you can't see.",
            Preset::Corvane => "Corvane as shipped: every built deviation on.",
            Preset::Everything => "Every extra on, including the experimental ones.",
        }
    }

    pub fn value_of(self, def: &FlagDef) -> &Value {
        def.value_for(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_round_trip() {
        for preset in Preset::ALL {
            assert_eq!(Preset::parse(preset.slug()), Some(preset));
            assert_eq!(
                serde_json::to_string(&preset).unwrap(),
                format!("\"{}\"", preset.slug())
            );
        }
        assert_eq!(
            Preset::parse(" GitHub-Desktop "),
            Some(Preset::GitHubDesktop)
        );
        assert_eq!(Preset::parse("custom"), None);
    }
}
