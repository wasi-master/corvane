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
    /// Corvane plus every broadly useful extra. Extras that suit only some
    /// tastes or workflows stay at the Corvane preset's value.
    #[serde(alias = "everything")]
    Max,
}

impl Preset {
    pub const ALL: [Preset; 4] = [
        Preset::GitHubDesktop,
        Preset::Familiar,
        Preset::Corvane,
        Preset::Max,
    ];

    /// The `CORVANE_FLAGS` / JSON spelling.
    pub const fn slug(self) -> &'static str {
        match self {
            Preset::GitHubDesktop => "github-desktop",
            Preset::Familiar => "familiar",
            Preset::Corvane => "corvane",
            Preset::Max => "max",
        }
    }

    pub fn parse(s: &str) -> Option<Preset> {
        let s = s.trim().to_ascii_lowercase();
        // "everything": the Max preset's name before 2026-10-01
        if s == "everything" {
            return Some(Preset::Max);
        }
        Preset::ALL.into_iter().find(|p| p.slug() == s)
    }

    pub const fn title(self) -> &'static str {
        match self {
            Preset::GitHubDesktop => "GitHub Desktop",
            Preset::Familiar => "Familiar",
            Preset::Corvane => "Corvane",
            Preset::Max => "Max",
        }
    }

    /// One line for the dialog's preset picker.
    pub const fn description(self) -> &'static str {
        match self {
            Preset::GitHubDesktop => "Behaves exactly like GitHub Desktop 3.6.6.",
            Preset::Familiar => "Looks like GitHub Desktop, keeps the improvements you can't see.",
            Preset::Corvane => "Corvane as shipped: every built deviation on.",
            Preset::Max => "Corvane plus every extra most people would want.",
        }
    }

    /// A few sentences for the dialog's Presets page and the generated docs:
    /// who the preset is for and what it switches on.
    pub const fn details(self) -> &'static str {
        match self {
            Preset::GitHubDesktop => {
                "Every flag at GitHub Desktop 3.6.6's behaviour, its bugs included. Pick it to \
                 compare the two apps side by side or to keep your habits exactly as they are; \
                 the parity harness runs Corvane with it."
            }
            Preset::Familiar => {
                "Flags that change what you see stay at GitHub Desktop's behaviour; flags that \
                 only change how things work (most bug fixes, refreshes, smooth scrolling, commit \
                 templates and the like) are on. Nothing moves and nothing new appears."
            }
            Preset::Corvane => {
                "The default. Every deviation that is finished and broadly useful is on, bug \
                 fixes included. Extras that change GitHub Desktop's look or workflow more \
                 strongly, or that few people want, stay off until you switch them on."
            }
            Preset::Max => {
                "Corvane plus the extras it leaves off that most people would want: more \
                 buttons, menu items, shortcuts and background work. Extras that are a matter \
                 of taste or suit only some workflows (colour-blind diff colours, compact \
                 History rows, a different list order, extra confirmations, skipping the \
                 Trash…) stay as in the Corvane preset; switch those on one by one."
            }
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
        assert_eq!(Preset::parse("everything"), Some(Preset::Max));
        assert_eq!(
            serde_json::from_str::<Preset>("\"everything\"").unwrap(),
            Preset::Max
        );
        assert_eq!(Preset::parse("custom"), None);
    }
}
