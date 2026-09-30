//! OS integration: macOS and Linux (freedesktop: XDG base directories,
//! D-Bus services, `xdg-open`). Every function here is the seam for Windows
//! later.

/// `CFBundleIdentifier` of the app bundle (keychain service, notification
/// settings deep link, GPUI `app_id`).
pub const BUNDLE_ID: &str = "com.wasimaster.corvane";

pub mod accessibility;
pub mod app_location;
pub mod apps;
pub mod cli;
pub mod crash_reports;
pub mod custom_integration;
pub mod editors;
pub mod ghd_import;
pub mod keychain;
pub mod locale;
pub mod notifications;
pub mod services;
pub mod shells;
#[cfg(not(target_os = "macos"))]
pub mod single_instance;
pub mod spell;
pub mod trash;
pub mod updater;
pub mod url_schemes;

pub mod paths {
    use std::path::PathBuf;

    pub const APP_NAME: &str = "Corvane";

    /// The per-app folder name inside the OS's data, cache and log
    /// directories: `Corvane` on macOS, `corvane` under the XDG base
    /// directories on Linux (freedesktop convention).
    #[cfg(target_os = "macos")]
    const DIR_NAME: &str = APP_NAME;
    #[cfg(not(target_os = "macos"))]
    const DIR_NAME: &str = "corvane";

    fn home() -> PathBuf {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    }

    /// `~/Library/Application Support/Corvane` (Linux: `$XDG_DATA_HOME/corvane`,
    /// `~/.local/share/corvane`), or `CORVANE_DATA_DIR` (an isolated store for
    /// test harnesses such as `tools/parity`).
    pub fn app_support_dir() -> PathBuf {
        if let Some(dir) = std::env::var_os("CORVANE_DATA_DIR") {
            return PathBuf::from(dir);
        }
        dirs::data_dir()
            .unwrap_or_else(|| home().join(".local/share"))
            .join(DIR_NAME)
    }

    /// `~/Library/Logs/Corvane` (Linux: `$XDG_STATE_HOME/corvane/logs`,
    /// `~/.local/state/corvane/logs`; under `CORVANE_DATA_DIR` when set so a
    /// harness instance keeps its logs to itself).
    pub fn logs_dir() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            home().join("Library/Logs").join(DIR_NAME)
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Some(dir) = std::env::var_os("CORVANE_DATA_DIR") {
                return PathBuf::from(dir).join("logs");
            }
            state_dir().join("logs")
        }
    }

    /// `$XDG_STATE_HOME/corvane` (`~/.local/state/corvane`): logs and crash
    /// reports, which the XDG spec files under state rather than data.
    #[cfg(not(target_os = "macos"))]
    pub fn state_dir() -> PathBuf {
        dirs::state_dir()
            .unwrap_or_else(|| home().join(".local/state"))
            .join(DIR_NAME)
    }

    /// `~/Library/Caches/Corvane` (Linux: `$XDG_CACHE_HOME/corvane`,
    /// `~/.cache/corvane`)
    pub fn cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| home().join(".cache"))
            .join(DIR_NAME)
    }

    /// GitHub Desktop's default clone location: `~/Documents/GitHub`.
    pub fn default_clone_dir() -> PathBuf {
        dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("GitHub")
    }

    /// Candidate clone locations offered during onboarding, existing ones only
    /// except the GHD default which is always offered.
    pub fn clone_dir_candidates() -> Vec<PathBuf> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut out = vec![default_clone_dir()];
        for candidate in [home.join("Developer"), home.join("Work")] {
            if candidate.is_dir() {
                out.push(candidate);
            }
        }
        out
    }
}

pub mod fonts {
    /// The family GitHub Desktop's `--font-family-monospace` stack
    /// (`SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace`)
    /// resolves to: SF Mono only when it is installed as a regular font
    /// (Apple's download puts `SF-Mono-*.otf` in a Fonts folder; the copy
    /// inside Terminal.app is not visible to Chromium), otherwise Menlo.
    #[cfg(target_os = "macos")]
    pub fn ghd_monospace_family() -> String {
        let dirs = [
            Some(std::path::PathBuf::from("/Library/Fonts")),
            dirs::home_dir().map(|h| h.join("Library/Fonts")),
        ];
        let installed = dirs.iter().flatten().any(|dir| {
            std::fs::read_dir(dir).is_ok_and(|entries| {
                entries.flatten().any(|e| {
                    let name = e.file_name();
                    let name = name.to_string_lossy();
                    name.starts_with("SF-Mono") || name.starts_with("SFMono")
                })
            })
        });
        if installed { "SF Mono" } else { "Menlo" }.to_string()
    }

    /// Linux Chromium resolves the same stack through fontconfig: the first
    /// named family that is installed as itself (fontconfig substitutes only
    /// for the generic `monospace`), else what `monospace` matches
    /// (DejaVu Sans Mono on a stock Ubuntu).
    #[cfg(not(target_os = "macos"))]
    pub fn ghd_monospace_family() -> String {
        [
            "SFMono-Regular",
            "SF Mono",
            "Consolas",
            "Liberation Mono",
            "Menlo",
        ]
        .into_iter()
        .find(|family| fc_match(family).is_some_and(|m| m.eq_ignore_ascii_case(family)))
        .map(str::to_string)
        .or_else(|| fc_match("monospace"))
        .unwrap_or_else(|| "DejaVu Sans Mono".to_string())
    }

    /// The family GHD's `--font-family-sans-serif` stack (`system-ui,
    /// -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", Helvetica,
    /// Arial, sans-serif, …`) resolves to in Chromium on Linux: `system-ui`
    /// is the desktop's UI font (GTK's `gtk-font-name`, fontconfig's
    /// `sans-serif` without a desktop), and the named families after it only
    /// count when installed as themselves.
    #[cfg(not(target_os = "macos"))]
    pub fn ghd_ui_family() -> String {
        fc_match("sans-serif")
            .or_else(|| {
                ["Noto Sans", "Helvetica", "Arial"]
                    .into_iter()
                    .find(|family| fc_match(family).is_some_and(|m| m.eq_ignore_ascii_case(family)))
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "DejaVu Sans".to_string())
    }

    /// `fc-match -f '%{family[0]}' <pattern>`: the family fontconfig picks.
    #[cfg(not(target_os = "macos"))]
    fn fc_match(pattern: &str) -> Option<String> {
        let out = std::process::Command::new("fc-match")
            .args(["-f", "%{family[0]}", pattern])
            .output()
            .ok()?;
        let family = String::from_utf8(out.stdout).ok()?.trim().to_string();
        (out.status.success() && !family.is_empty()).then_some(family)
    }
}
