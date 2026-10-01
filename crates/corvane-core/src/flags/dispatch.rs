//! `Dispatcher` half of the flags: the single write path, the dialog's
//! actions, and the live side effects core owns (the watcher, the crash
//! reports hook). `main.rs` owns the rest (menus, theme) through its
//! observer.

use gpui_kit::App;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use super::{Availability, FlagId, FlagOverrides, Flags, Preset, Value, def, ids};
use crate::dispatcher::Dispatcher;
use crate::persistence::StoreExt;
use crate::state::Popup;

/// What `import_flags_json` dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    /// Slugs this build does not know (kept out of the store).
    pub unknown: Vec<String>,
}

impl Dispatcher {
    /// The single write path (mirrors `update_settings`): edit the stored
    /// layer, re-resolve, persist, notify, then apply live side effects.
    pub fn update_flags(cx: &mut App, edit: impl FnOnce(&mut FlagOverrides)) {
        let previous = Self::state(cx).update(cx, |s, cx| {
            let previous = s.flags.clone();
            edit(&mut s.flag_overrides);
            s.flags = Flags::resolve(&s.flag_overrides, &s.flags_env);
            if let Err(err) = s.store.save_flags(&s.flag_overrides) {
                error!(?err, "could not save flags");
            }
            cx.notify();
            previous
        });
        Self::after_flags_changed(&previous, cx);
    }

    /// Set one flag. A value equal to the preset's own drops the override,
    /// so the accent bar means "differs from the preset".
    pub fn set_flag(id: FlagId, value: Value, cx: &mut App) -> Result<(), String> {
        let def = def(id);
        def.kind.validate(&value)?;
        if let Availability::BuiltIn(reason) = def.availability() {
            return Err(reason.to_string());
        }
        if Self::state(cx).read(cx).flags.is_env_locked(id) {
            return Err(format!(
                "{} is set by CORVANE_FLAGS for this session",
                def.ident()
            ));
        }
        info!(flag = %def.ident(), value = %value.render(), "flag set");
        Self::update_flags(cx, |o| {
            if def.value_for(o.preset) == &value {
                o.overrides.remove(def.slug);
            } else {
                o.overrides.insert(def.slug.to_string(), value);
            }
        });
        Ok(())
    }

    /// Back to the preset's value.
    pub fn reset_flag(id: FlagId, cx: &mut App) {
        let slug = def(id).slug;
        Self::update_flags(cx, |o| {
            o.overrides.remove(slug);
        });
    }

    /// Switch the base layer; the overrides go with it (picking "GitHub
    /// Desktop" has to give GitHub Desktop).
    pub fn apply_preset(preset: Preset, cx: &mut App) {
        info!(preset = preset.slug(), "flags preset applied");
        Self::update_flags(cx, |o| {
            o.preset = preset;
            o.overrides.clear();
        });
    }

    /// Drop every override, keep the preset.
    pub fn reset_all_flags(cx: &mut App) {
        Self::update_flags(cx, |o| o.overrides.clear());
    }

    /// The stored layer as pretty JSON (the dialog's "Copy as JSON").
    pub fn export_flags_json(cx: &App) -> String {
        serde_json::to_string_pretty(&Self::state(cx).read(cx).flag_overrides)
            .unwrap_or_else(|_| "{}".to_string())
    }

    /// The stored layer as a `CORVANE_FLAGS` spec.
    pub fn export_env_string(cx: &App) -> String {
        super::env::render(&Self::state(cx).read(cx).flag_overrides)
    }

    /// Replace the stored layer with `text` (the dialog's "Paste JSON"):
    /// unknown slugs are dropped and reported, an invalid value aborts.
    pub fn import_flags_json(text: &str, cx: &mut App) -> Result<ImportReport, String> {
        let mut parsed: FlagOverrides =
            serde_json::from_str(text.trim()).map_err(|err| format!("Not a flags JSON: {err}"))?;
        let mut report = ImportReport::default();
        let mut keep = std::collections::BTreeMap::new();
        for (slug, value) in std::mem::take(&mut parsed.overrides) {
            match super::by_slug(&slug) {
                Some(def) => {
                    def.kind
                        .validate(&value)
                        .map_err(|err| format!("{}: {err}", def.ident()))?;
                    keep.insert(slug, value);
                }
                None => report.unknown.push(slug),
            }
        }
        parsed.overrides = keep;
        Self::update_flags(cx, |o| *o = parsed);
        Ok(report)
    }

    /// Corvane › Flags… (also `CORVANE_POPUP=flags[:query]` and
    /// `x-corvane://flags?q=`).
    pub fn open_flags(query: Option<String>, cx: &mut App) {
        Self::show_popup(Popup::Flags { query }, cx);
    }

    /// The Flags dialog's Relaunch button: start Corvane again once this
    /// process has exited (the updater's path: `open -n` the bundle on
    /// macOS, the AppImage or executable on Linux), then quit.
    pub fn relaunch(cx: &mut App) {
        let Some(bundle) = corvane_platform::app_location::relaunch_target() else {
            Self::show_error(
                "Could not relaunch Corvane",
                "Corvane is not running from an app bundle; quit and start it again.",
                cx,
            );
            return;
        };
        match corvane_platform::app_location::relaunch_after_exit(&bundle, std::process::id()) {
            Ok(()) => {
                info!(bundle = %bundle.display(), "relaunching for flags");
                cx.quit();
            }
            Err(err) => Self::show_error("Could not relaunch Corvane", err, cx),
        }
    }

    /// Sign in the way `307-sign-in-flow` says (the dialog's primary button,
    /// Welcome, re-authorization prompts).
    pub fn begin_sign_in(endpoint: corvane_github::Endpoint, cx: &mut App) {
        if Self::browser_sign_in_first(&endpoint, cx) {
            Self::sign_in_web_flow(endpoint, cx)
        } else {
            Self::sign_in_device_flow(endpoint, cx)
        }
    }

    /// Whether `307-sign-in-flow` starts `endpoint`'s sign-in in the browser.
    /// "auto" does on GitHub.com when the build has a client secret (GitHub
    /// refuses the web flow's token exchange without one); GitHub Enterprise
    /// keeps the device flow, as its secret lives in the keychain.
    pub fn browser_sign_in_first(endpoint: &corvane_github::Endpoint, cx: &App) -> bool {
        match Self::state(cx).read(cx).flags.text(ids::SIGN_IN_FLOW) {
            "browser" => true,
            "auto" => endpoint.is_dotcom() && corvane_github::CLIENT_SECRET.is_some(),
            _ => false,
        }
    }

    /// Live side effects core owns.
    fn after_flags_changed(previous: &Flags, cx: &mut App) {
        let now = Self::state(cx).read(cx).flags.clone();
        if now == *previous {
            return;
        }
        if now.bool(ids::FS_WATCHER) != previous.bool(ids::FS_WATCHER)
            || now.number(ids::FS_WATCHER_DEBOUNCE_MS)
                != previous.number(ids::FS_WATCHER_DEBOUNCE_MS)
        {
            Self::restart_watcher(cx);
        }
        Self::sync_crash_reports_setting(cx);
        if now.bool(ids::EXTRA_EDITORS) != previous.bool(ids::EXTRA_EDITORS) {
            Self::detect_integrations(cx);
        }
        if now.bool(ids::TREE_SITTER_HIGHLIGHTING) && !previous.bool(ids::TREE_SITTER_HIGHLIGHTING)
        {
            Self::load_tree_sitter_packs(cx);
        }
        for id in now.restart_pending(&Self::state(cx).read(cx).flags_at_launch) {
            warn!(flag = %id, "flag takes effect at the next launch");
        }
    }

    /// Drop the watcher and start it again for the selected repository (or
    /// not, when `202-fs-watcher` is off).
    pub fn restart_watcher(cx: &mut App) {
        let selected = Self::state(cx).update(cx, |s, _| {
            s.watcher = None;
            s.watched_repo = None;
            s.selected
        });
        if let Some(id) = selected {
            Self::start_watching(id, cx);
        }
    }
}
