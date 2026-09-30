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
            Preset::Everything => {
                "Every flag on, including the extras the Corvane preset leaves off and the \
                 experimental ones. Good for trying out everything Corvane can do; expect parts \
                 of it to feel unlike GitHub Desktop."
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
        assert_eq!(Preset::parse("custom"), None);
    }
}
