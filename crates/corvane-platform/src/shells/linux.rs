//! Shell detection and launching - GHD `lib/shells/linux.ts` (paths and
//! arguments copied verbatim).

use std::path::Path;
use std::process::{Command, Stdio};

use super::FoundShell;

/// GHD `Shell` (Linux), in the enum's order; GNOME Terminal is the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Gnome,
    GnomeConsole,
    Ptyxis,
    Mate,
    Tilix,
    Terminator,
    Urxvt,
    Konsole,
    Xterm,
    Terminology,
    Deepin,
    Elementary,
    Xfce,
    Alacritty,
    Kitty,
    LxTerminal,
    Warp,
    Ghostty,
}

pub const DEFAULT_SHELL: Shell = Shell::Gnome;

const ALL: &[Shell] = &[
    Shell::Gnome,
    Shell::GnomeConsole,
    Shell::Ptyxis,
    Shell::Mate,
    Shell::Tilix,
    Shell::Terminator,
    Shell::Urxvt,
    Shell::Konsole,
    Shell::Xterm,
    Shell::Terminology,
    Shell::Deepin,
    Shell::Elementary,
    Shell::Xfce,
    Shell::Alacritty,
    Shell::Kitty,
    Shell::LxTerminal,
    Shell::Warp,
    Shell::Ghostty,
];

impl Shell {
    /// The label GHD persists and shows in Options › Integrations.
    pub fn label(self) -> &'static str {
        match self {
            Shell::Gnome => "GNOME Terminal",
            Shell::GnomeConsole => "GNOME Console",
            Shell::Ptyxis => "Ptyxis",
            Shell::Mate => "MATE Terminal",
            Shell::Tilix => "Tilix",
            Shell::Terminator => "Terminator",
            Shell::Urxvt => "URxvt",
            Shell::Konsole => "Konsole",
            Shell::Xterm => "XTerm",
            Shell::Terminology => "Terminology",
            Shell::Deepin => "Deepin Terminal",
            Shell::Elementary => "Elementary Terminal",
            Shell::Xfce => "XFCE Terminal",
            Shell::Alacritty => "Alacritty",
            Shell::Kitty => "Kitty",
            Shell::LxTerminal => "LXDE Terminal",
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

    /// GHD `getShellPath`.
    fn path(self) -> &'static str {
        match self {
            Shell::Gnome => "/usr/bin/gnome-terminal",
            Shell::GnomeConsole => "/usr/bin/kgx",
            Shell::Ptyxis => "/usr/bin/ptyxis",
            Shell::Mate => "/usr/bin/mate-terminal",
            Shell::Tilix => "/usr/bin/tilix",
            Shell::Terminator => "/usr/bin/terminator",
            Shell::Urxvt => "/usr/bin/urxvt",
            Shell::Konsole => "/usr/bin/konsole",
            Shell::Xterm => "/usr/bin/xterm",
            Shell::Terminology => "/usr/bin/terminology",
            Shell::Deepin => "/usr/bin/deepin-terminal",
            Shell::Elementary => "/usr/bin/io.elementary.terminal",
            Shell::Xfce => "/usr/bin/xfce4-terminal",
            Shell::Alacritty => "/usr/bin/alacritty",
            Shell::Kitty => "/usr/bin/kitty",
            Shell::LxTerminal => "/usr/bin/lxterminal",
            Shell::Warp => "/usr/bin/warp-terminal",
            Shell::Ghostty => "/usr/bin/ghostty",
        }
    }
}

/// Installed shells in GHD's order (`getAvailableShells`).
pub fn available_shells() -> Vec<FoundShell> {
    ALL.iter()
        .filter(|shell| Path::new(shell.path()).exists())
        .map(|&shell| FoundShell {
            shell,
            bundle_id: String::new(),
            path: shell.path().into(),
        })
        .collect()
}

/// The arguments and working directory GHD's `launch` spawns with.
fn launch_args(shell: Shell, dir: &str) -> (Vec<String>, Option<&str>) {
    let v = |args: &[&str]| args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    match shell {
        Shell::Gnome
        | Shell::GnomeConsole
        | Shell::Mate
        | Shell::Tilix
        | Shell::Terminator
        | Shell::Xfce
        | Shell::Alacritty => (v(&["--working-directory", dir]), None),
        Shell::Ptyxis => (v(&["--new-window", "--working-directory", dir]), None),
        Shell::Urxvt => (v(&["-cd", dir]), None),
        Shell::Konsole => (v(&["--workdir", dir]), None),
        Shell::Xterm => (v(&["-e", "/bin/bash"]), Some(dir)),
        Shell::Terminology => (v(&["-d", dir]), None),
        Shell::Deepin | Shell::Elementary => (v(&["-w", dir]), None),
        Shell::Kitty => (v(&["--single-instance", "--directory", dir]), None),
        Shell::LxTerminal | Shell::Ghostty => (vec![format!("--working-directory={dir}")], None),
        Shell::Warp => (Vec::new(), Some(dir)),
    }
}

/// GHD `launch`, detached so closing Corvane leaves the terminal open.
pub fn launch(found: &FoundShell, path: &Path) -> std::io::Result<()> {
    let dir = path.to_string_lossy();
    let (args, cwd) = launch_args(found.shell, &dir);
    let mut command = Command::new(&found.path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.spawn().map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_labels() {
        assert_eq!(Shell::parse("Konsole"), Shell::Konsole);
        assert_eq!(Shell::parse("LXDE Terminal"), Shell::LxTerminal);
        assert_eq!(Shell::parse("iTerm2"), Shell::Gnome);
    }

    #[test]
    fn launch_arguments_follow_ghd() {
        assert_eq!(
            launch_args(Shell::Gnome, "/r"),
            (vec!["--working-directory".into(), "/r".into()], None)
        );
        assert_eq!(
            launch_args(Shell::Ghostty, "/r"),
            (vec!["--working-directory=/r".to_string()], None)
        );
        assert_eq!(
            launch_args(Shell::Xterm, "/r"),
            (vec!["-e".into(), "/bin/bash".into()], Some("/r"))
        );
        assert_eq!(launch_args(Shell::Warp, "/r"), (Vec::new(), Some("/r")));
    }
}
