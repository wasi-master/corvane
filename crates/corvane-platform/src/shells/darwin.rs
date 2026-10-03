//! Shell detection and launching - GHD `lib/shells/darwin.ts`.

use std::path::Path;

use super::FoundShell;
use crate::apps;

/// GHD `Shell` (macOS), in the enum's order; `Terminal` is the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Terminal,
    Hyper,
    ITerm2,
    PowerShellCore,
    Kitty,
    Alacritty,
    Tabby,
    WezTerm,
    Warp,
    Ghostty,
}

pub const DEFAULT_SHELL: Shell = Shell::Terminal;

const ALL: &[Shell] = &[
    Shell::Terminal,
    Shell::Hyper,
    Shell::ITerm2,
    Shell::PowerShellCore,
    Shell::Kitty,
    Shell::Alacritty,
    Shell::Tabby,
    Shell::WezTerm,
    Shell::Warp,
    Shell::Ghostty,
];

impl Shell {
    /// The label GHD persists and shows in Settings › Integrations.
    pub fn label(self) -> &'static str {
        match self {
            Shell::Terminal => "Terminal",
            Shell::Hyper => "Hyper",
            Shell::ITerm2 => "iTerm2",
            Shell::PowerShellCore => "PowerShell Core",
            Shell::Kitty => "Kitty",
            Shell::Alacritty => "Alacritty",
            Shell::Tabby => "Tabby",
            Shell::WezTerm => "WezTerm",
            Shell::Warp => "Warp",
            Shell::Ghostty => "Ghostty",
        }
    }

    /// GHD `parse`: unknown labels fall back to the default.
    pub fn parse(label: &str) -> Shell {
        ALL.iter()
            .copied()
            .find(|s| s.label() == label)
            .unwrap_or(DEFAULT_SHELL)
    }

    fn bundle_ids(self) -> &'static [&'static str] {
        match self {
            Shell::Terminal => &["com.apple.Terminal"],
            Shell::ITerm2 => &["com.googlecode.iterm2"],
            Shell::Hyper => &["co.zeit.hyper"],
            Shell::PowerShellCore => &["com.microsoft.powershell"],
            Shell::Kitty => &["net.kovidgoyal.kitty"],
            Shell::Alacritty => &["org.alacritty", "io.alacritty"],
            Shell::Tabby => &["org.tabby"],
            Shell::WezTerm => &["com.github.wez.wezterm"],
            Shell::Warp => &["dev.warp.Warp-Stable"],
            Shell::Ghostty => &["com.mitchellh.ghostty"],
        }
    }
}

/// Installed shells in GHD's display order (Terminal, Hyper, iTerm2,
/// PowerShell Core, Ghostty, Kitty, Alacritty, Tabby, WezTerm, Warp).
pub fn available_shells() -> Vec<FoundShell> {
    let order = [
        Shell::Terminal,
        Shell::Hyper,
        Shell::ITerm2,
        Shell::PowerShellCore,
        Shell::Ghostty,
        Shell::Kitty,
        Shell::Alacritty,
        Shell::Tabby,
        Shell::WezTerm,
        Shell::Warp,
    ];
    order
        .iter()
        .filter_map(|&shell| {
            let (bundle_id, app) = apps::first_installed(shell.bundle_ids())?;
            let path = match shell {
                Shell::Kitty => app.join("Contents/MacOS/kitty"),
                Shell::Alacritty => app.join("Contents/MacOS/alacritty"),
                Shell::Tabby => app.join("Contents/MacOS/Tabby"),
                Shell::WezTerm => app.join("Contents/MacOS/wezterm"),
                Shell::Warp => app.join("Contents/MacOS/stable"),
                _ => app,
            };
            Some(FoundShell {
                shell,
                bundle_id,
                path,
            })
        })
        .collect()
}

/// GHD `launch`: shells that cannot take a folder through `open` get their
/// own working-directory flag.
pub fn launch(found: &FoundShell, path: &Path) -> std::io::Result<()> {
    let dir = path.to_string_lossy();
    match found.shell {
        Shell::Kitty => {
            apps::spawn_detached(&found.path, &["--single-instance", "--directory", &dir])
        }
        Shell::Alacritty => apps::spawn_detached(&found.path, &["--working-directory", &dir]),
        Shell::Tabby => apps::spawn_detached(&found.path, &["open", &dir]),
        Shell::WezTerm => apps::spawn_detached(&found.path, &["start", "--cwd", &dir]),
        _ => apps::open_with_bundle(&found.bundle_id, path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_labels() {
        assert_eq!(Shell::parse("iTerm2"), Shell::ITerm2);
        assert_eq!(Shell::parse("Ghostty"), Shell::Ghostty);
        assert_eq!(Shell::parse("nonsense"), Shell::Terminal);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn terminal_is_always_available() {
        let shells = available_shells();
        assert_eq!(shells.first().map(|s| s.shell), Some(Shell::Terminal));
    }
}
