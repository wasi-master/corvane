//! External editor detection - GHD `lib/editors/darwin.ts` (the bundle
//! identifier table is copied verbatim), `lib/editors/linux.ts` (the path
//! table, likewise) and `lib/editors/launch.ts`.
//!
//! Linux: GHD's relative paths (`.local/share/flatpak/…`, JetBrains
//! Toolbox scripts) and `~/.local/bin/zed` resolve against the home folder;
//! GHD passes them to `pathExists` as they are, which resolves against the
//! process's working directory and only finds them when that is home.
//!
//! `launch_at_line` is Corvane's (flag `diff-open-in-editor-at-line`): VS Code
//! and its forks, Sublime Text and Zed open a file at a line through the
//! command line tool in their bundle; other editors just open the file.
//! `EXTRA_EDITORS` (flag `extra-editors`) adds editors GHD does not list.
//! Deviation: [`code_workspace_file`] lets VS Code and its forks open a
//! repository's only `*.code-workspace` file (`509-vscode-workspace-file`).

use std::path::{Path, PathBuf};

use crate::apps;

/// Friendly name + bundle identifiers, in GHD's order (the first installed
/// editor is the default when none is selected in Settings).
#[cfg(target_os = "macos")]
const EDITORS: &[(&str, &[&str])] = &[
    ("Atom", &["com.github.atom"]),
    ("Aptana Studio", &["aptana.studio"]),
    ("Eclipse IDE for Java Developers", &["epp.package.java"]),
    (
        "Eclipse IDE for Enterprise Java and Web Developers",
        &["epp.package.jee"],
    ),
    ("Eclipse IDE for C/C++ Developers", &["epp.package.cpp"]),
    (
        "Eclipse IDE for Eclipse Committers",
        &["epp.package.committers"],
    ),
    (
        "Eclipse IDE for Embedded C/C++ Developers",
        &["epp.package.embedcpp"],
    ),
    ("Eclipse IDE for PHP Developers", &["epp.package.php"]),
    (
        "Eclipse IDE for Java and DSL Developers",
        &["epp.package.dsl"],
    ),
    (
        "Eclipse IDE for RCP and RAP Developers",
        &["epp.package.rcp"],
    ),
    ("Eclipse Modeling Tools", &["epp.package.modeling"]),
    (
        "Eclipse IDE for Scientific Computing",
        &["epp.package.parallel"],
    ),
    ("Eclipse IDE for Scout Developers", &["epp.package.scout"]),
    ("MacVim", &["org.vim.MacVim"]),
    ("Neovide", &["com.neovide.neovide"]),
    ("VimR", &["com.qvacua.VimR"]),
    ("Visual Studio Code", &["com.microsoft.VSCode"]),
    (
        "Visual Studio Code (Insiders)",
        &["com.microsoft.VSCodeInsiders"],
    ),
    ("VSCodium", &["com.visualstudio.code.oss", "com.vscodium"]),
    (
        "Sublime Text",
        &[
            "com.sublimetext.4",
            "com.sublimetext.3",
            "com.sublimetext.2",
        ],
    ),
    ("BBEdit", &["com.barebones.bbedit"]),
    ("PhpStorm", &["com.jetbrains.PhpStorm"]),
    ("PyCharm", &["com.jetbrains.PyCharm"]),
    ("PyCharm Community Edition", &["com.jetbrains.pycharm.ce"]),
    ("DataSpell", &["com.jetbrains.DataSpell"]),
    ("RubyMine", &["com.jetbrains.RubyMine"]),
    ("RustRover", &["com.jetbrains.RustRover"]),
    ("RStudio", &["org.rstudio.RStudio", "com.rstudio.desktop"]),
    ("TextMate", &["com.macromates.TextMate"]),
    ("Brackets", &["io.brackets.appshell"]),
    ("WebStorm", &["com.jetbrains.WebStorm"]),
    ("CLion", &["com.jetbrains.CLion"]),
    ("Typora", &["abnerworks.Typora"]),
    ("CodeRunner", &["com.krill.CodeRunner"]),
    (
        "SlickEdit",
        &[
            "com.slickedit.SlickEditPro2018",
            "com.slickedit.SlickEditPro2017",
            "com.slickedit.SlickEditPro2016",
            "com.slickedit.SlickEditPro2015",
        ],
    ),
    ("IntelliJ", &["com.jetbrains.intellij"]),
    ("IntelliJ Community Edition", &["com.jetbrains.intellij.ce"]),
    ("Xcode", &["com.apple.dt.Xcode"]),
    ("GoLand", &["com.jetbrains.goland"]),
    ("Android Studio", &["com.google.android.studio"]),
    ("Rider", &["com.jetbrains.rider"]),
    ("Nova", &["com.panic.Nova"]),
    ("Emacs", &["org.gnu.Emacs"]),
    ("Lite XL", &["com.lite-xl"]),
    ("Fleet", &["Fleet.app"]),
    ("Pulsar", &["dev.pulsar-edit.pulsar"]),
    ("Zed", &["dev.zed.Zed"]),
    ("Zed (Preview)", &["dev.zed.Zed-Preview"]),
    ("Cursor", &["com.todesktop.230313mzl4w4u92"]),
    ("Windsurf", &["com.exafunction.windsurf"]),
];

/// Friendly name + executable paths (GHD `lib/editors/linux.ts`), in GHD's
/// order; the first existing path wins.
#[cfg(not(target_os = "macos"))]
const EDITORS: &[(&str, &[&str])] = &[
    ("Atom", &["/snap/bin/atom", "/usr/bin/atom"]),
    ("Neovim", &["/usr/bin/nvim"]),
    ("Neovim-Qt", &["/usr/bin/nvim-qt"]),
    ("Neovide", &["/usr/bin/neovide"]),
    ("gVim", &["/usr/bin/gvim"]),
    (
        "Visual Studio Code",
        &[
            "/usr/share/code/bin/code",
            "/snap/bin/code",
            "/usr/bin/code",
            "/mnt/c/Program Files/Microsoft VS Code/bin/code",
            "/var/lib/flatpak/app/com.visualstudio.code/current/active/export/bin/com.visualstudio.code",
            ".local/share/flatpak/app/com.visualstudio.code/current/active/export/bin/com.visualstudio.code",
        ],
    ),
    (
        "Visual Studio Code (Insiders)",
        &[
            "/snap/bin/code-insiders",
            "/usr/bin/code-insiders",
            "/var/lib/flatpak/app/com.visualstudio.code.insiders/current/active/export/bin/com.visualstudio.code.insiders",
            ".local/share/flatpak/app/com.visualstudio.code.insiders/current/active/export/bin/com.visualstudio.code.insiders",
        ],
    ),
    (
        "VSCodium",
        &[
            "/usr/bin/codium",
            "/var/lib/flatpak/app/com.vscodium.codium/current/active/export/bin/com.vscodium.codium",
            "/usr/share/vscodium-bin/bin/codium",
            ".local/share/flatpak/app/com.vscodium.codium/current/active/export/bin/com.vscodium.codium",
            "/snap/bin/codium",
        ],
    ),
    ("VSCodium (Insiders)", &["/usr/bin/codium-insiders"]),
    ("Sublime Text", &["/usr/bin/subl"]),
    ("Typora", &["/usr/bin/typora"]),
    (
        "SlickEdit",
        &[
            "/opt/slickedit-pro2018/bin/vs",
            "/opt/slickedit-pro2017/bin/vs",
            "/opt/slickedit-pro2016/bin/vs",
            "/opt/slickedit-pro2015/bin/vs",
        ],
    ),
    // elementary OS's code editor
    ("Code", &["/usr/bin/io.elementary.code"]),
    ("Lite XL", &["/usr/bin/lite-xl"]),
    (
        "JetBrains PhpStorm",
        &[
            "/snap/bin/phpstorm",
            ".local/share/JetBrains/Toolbox/scripts/PhpStorm",
        ],
    ),
    (
        "JetBrains WebStorm",
        &[
            "/snap/bin/webstorm",
            ".local/share/JetBrains/Toolbox/scripts/webstorm",
        ],
    ),
    (
        "IntelliJ IDEA",
        &[
            "/snap/bin/idea",
            ".local/share/JetBrains/Toolbox/scripts/idea",
        ],
    ),
    (
        "IntelliJ IDEA Ultimate Edition",
        &[
            "/snap/bin/intellij-idea-ultimate",
            ".local/share/JetBrains/Toolbox/scripts/intellij-idea-ultimate",
        ],
    ),
    (
        "JetBrains Goland",
        &[
            "/snap/bin/goland",
            ".local/share/JetBrains/Toolbox/scripts/goland",
        ],
    ),
    (
        "JetBrains CLion",
        &[
            "/snap/bin/clion",
            ".local/share/JetBrains/Toolbox/scripts/clion1",
        ],
    ),
    (
        "JetBrains Rider",
        &[
            "/snap/bin/rider",
            ".local/share/JetBrains/Toolbox/scripts/rider",
        ],
    ),
    (
        "JetBrains RubyMine",
        &[
            "/snap/bin/rubymine",
            ".local/share/JetBrains/Toolbox/scripts/rubymine",
        ],
    ),
    (
        "JetBrains PyCharm",
        &[
            "/snap/bin/pycharm",
            "/snap/bin/pycharm-professional",
            ".local/share/JetBrains/Toolbox/scripts/pycharm",
        ],
    ),
    (
        "JetBrains RustRover",
        &[
            "/snap/bin/rustrover",
            ".local/share/JetBrains/Toolbox/scripts/rustrover",
        ],
    ),
    (
        "Android Studio",
        &[
            "/snap/bin/studio",
            ".local/share/JetBrains/Toolbox/scripts/studio",
        ],
    ),
    (
        "Emacs",
        &["/snap/bin/emacs", "/usr/local/bin/emacs", "/usr/bin/emacs"],
    ),
    ("Kate", &["/usr/bin/kate"]),
    ("GEdit", &["/usr/bin/gedit"]),
    ("GNOME Text Editor", &["/usr/bin/gnome-text-editor"]),
    ("GNOME Builder", &["/usr/bin/gnome-builder"]),
    ("Notepadqq", &["/usr/bin/notepadqq"]),
    ("Mousepad", &["/usr/bin/mousepad"]),
    ("Pulsar", &["/usr/bin/pulsar"]),
    ("Pluma", &["/usr/bin/pluma"]),
    (
        "Zed",
        &[
            "/usr/bin/zedit",
            "/usr/bin/zeditor",
            "/usr/bin/zed-editor",
            "~/.local/bin/zed",
            "/usr/bin/zed",
        ],
    ),
];

/// Editors GHD 3.6.6 does not know, in the same shape; detected only with
/// flag `extra-editors` and listed after GHD's.
#[cfg(target_os = "macos")]
const EXTRA_EDITORS: &[(&str, &[&str])] = &[
    // desktop/desktop#22922, bundle id from desktop/desktop#21417
    ("Antigravity", &["com.google.antigravity"]),
];
#[cfg(not(target_os = "macos"))]
const EXTRA_EDITORS: &[(&str, &[&str])] = &[
    // desktop/desktop#22922
    ("Antigravity", &["/usr/bin/antigravity"]),
    ("Cursor", &["/usr/bin/cursor", "/opt/cursor/cursor"]),
    ("Windsurf", &["/usr/bin/windsurf"]),
];

/// GHD's Linux paths: absolute, `~/…`, or relative to the home folder.
#[cfg(not(target_os = "macos"))]
fn resolve_linux_path(path: &str) -> PathBuf {
    let home = || dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    if let Some(rest) = path.strip_prefix("~/") {
        home().join(rest)
    } else if path.starts_with('/') {
        PathBuf::from(path)
    } else {
        home().join(path)
    }
}

/// The first of `paths` that exists (GHD `getAvailablePath`).
#[cfg(not(target_os = "macos"))]
fn first_existing(paths: &[&str]) -> Option<(String, PathBuf)> {
    paths.iter().find_map(|p| {
        let path = resolve_linux_path(p);
        path.exists().then(|| ((*p).to_string(), path))
    })
}

/// Editors that open VS Code `.code-workspace` files (`509-vscode-workspace-file`).
const CODE_WORKSPACE_EDITORS: &[&str] = &[
    "Visual Studio Code",
    "Visual Studio Code (Insiders)",
    "VSCodium",
    "Cursor",
    "Windsurf",
];

/// Corvane `509-vscode-workspace-file`: the one `*.code-workspace` file at
/// the top of `dir` when `editor` is VS Code or a fork of it; `None` when
/// there is none or several.
pub fn code_workspace_file(editor: &FoundEditor, dir: &Path) -> Option<PathBuf> {
    if !CODE_WORKSPACE_EDITORS.contains(&editor.name.as_str()) {
        return None;
    }
    let mut found = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "code-workspace"));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// GHD `suggestedExternalEditor`.
pub const SUGGESTED_EDITOR_NAME: &str = "Visual Studio Code";
pub const SUGGESTED_EDITOR_URL: &str = "https://code.visualstudio.com";

/// GHD `FoundEditor`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundEditor {
    pub name: String,
    /// macOS: the bundle identifier that matched. Linux: the table entry
    /// that matched (GHD's spelling of the path).
    pub bundle_id: String,
    /// macOS: the `.app` bundle. Linux: the executable.
    pub path: PathBuf,
}

/// Every known editor installed on this machine, in table order (then
/// [`EXTRA_EDITORS`] when `extras`). Costs one LaunchServices lookup (or
/// `stat`) per candidate; run it off the main thread.
pub fn available_editors(extras: bool) -> Vec<FoundEditor> {
    let extra: &[(&str, &[&str])] = if extras { EXTRA_EDITORS } else { &[] };
    #[cfg(target_os = "macos")]
    let find = apps::first_installed;
    #[cfg(not(target_os = "macos"))]
    let find = first_existing;
    EDITORS
        .iter()
        .chain(extra)
        .filter_map(|(name, ids)| {
            find(ids).map(|(bundle_id, path)| FoundEditor {
                name: (*name).to_string(),
                bundle_id,
                path,
            })
        })
        .collect()
}

/// Why launching failed (GHD `ExternalEditorError` + its metadata).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorError {
    pub message: String,
    /// Offer a link to install the suggested editor.
    pub suggest_default_editor: bool,
    /// Offer to open Settings › Integrations.
    pub open_preferences: bool,
}

/// GHD `findEditorOrDefault`: the named editor, else the first installed one.
pub fn find_editor_or_default<'a>(
    editors: &'a [FoundEditor],
    name: Option<&str>,
) -> Result<Option<&'a FoundEditor>, EditorError> {
    if editors.is_empty() {
        return Ok(None);
    }
    match name {
        Some(name) => editors
            .iter()
            .find(|e| e.name == name)
            .map(Some)
            .ok_or_else(|| EditorError {
                message: format!(
                    "The editor '{name}' could not be found. Please open {SETTINGS_LABEL} and choose an available editor."
                ),
                suggest_default_editor: false,
                open_preferences: true,
            }),
        None => Ok(editors.first()),
    }
}

/// GHD's name for its settings dialog in messages: "Settings" on macOS,
/// "Options" elsewhere (`__DARWIN__ ? 'Settings' : 'Options'`).
pub const SETTINGS_LABEL: &str = if cfg!(target_os = "macos") {
    "Settings"
} else {
    "Options"
};

/// GHD `launchExternalEditor`: `open -a <bundle> <path>` on macOS, the
/// executable with the path elsewhere, detached.
pub fn launch(editor: &FoundEditor, target: &Path) -> Result<(), EditorError> {
    if !editor.path.exists() {
        return Err(EditorError {
            message: format!(
                "Could not find executable for '{}' at path '{}'. Please open {SETTINGS_LABEL} and select an available editor.",
                editor.name,
                editor.path.display()
            ),
            suggest_default_editor: false,
            open_preferences: true,
        });
    }
    #[cfg(target_os = "macos")]
    let launched = apps::open_with_app(&editor.path, target);
    #[cfg(not(target_os = "macos"))]
    let launched = apps::spawn_detached(&editor.path, &[&target.to_string_lossy()]);
    launched.map_err(|err| EditorError {
        message: if err.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "Corvane doesn't have the proper permissions to start '{}'. Please open {SETTINGS_LABEL} and try another editor.",
                editor.name
            )
        } else {
            format!(
                "Something went wrong while trying to start '{}'. Please open {SETTINGS_LABEL} and try another editor.",
                editor.name
            )
        },
        suggest_default_editor: false,
        open_preferences: true,
    })
}

/// How an editor's bundled command line tool opens a file at a line (not in
/// GHD, which only opens files; desktop/desktop#14476).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineArgs {
    /// `-g <file>:<line>` (VS Code and its forks)
    Goto,
    /// `<file>:<line>` (Sublime Text's `subl`, Zed's `cli`)
    Suffix,
}

/// The command line tool inside an editor's bundle (relative candidates,
/// first existing wins) and its line syntax, for the editors that ship one.
#[cfg(target_os = "macos")]
fn line_tool(bundle_id: &str) -> Option<(&'static [&'static str], LineArgs)> {
    Some(match bundle_id {
        "com.microsoft.VSCode" => (&["Contents/Resources/app/bin/code"], LineArgs::Goto),
        "com.microsoft.VSCodeInsiders" => (
            &[
                "Contents/Resources/app/bin/code-insiders",
                "Contents/Resources/app/bin/code",
            ],
            LineArgs::Goto,
        ),
        "com.visualstudio.code.oss" | "com.vscodium" => {
            (&["Contents/Resources/app/bin/codium"], LineArgs::Goto)
        }
        "com.todesktop.230313mzl4w4u92" => (&["Contents/Resources/app/bin/cursor"], LineArgs::Goto),
        "com.exafunction.windsurf" => (&["Contents/Resources/app/bin/windsurf"], LineArgs::Goto),
        "com.sublimetext.4" | "com.sublimetext.3" | "com.sublimetext.2" => {
            (&["Contents/SharedSupport/bin/subl"], LineArgs::Suffix)
        }
        "dev.zed.Zed" | "dev.zed.Zed-Preview" => (&["Contents/MacOS/cli"], LineArgs::Suffix),
        _ => return None,
    })
}

/// Linux: the found executable is the command line tool itself; its line
/// syntax by editor name (the candidates are relative to the executable,
/// i.e. empty).
#[cfg(not(target_os = "macos"))]
fn line_tool(name: &str) -> Option<(&'static [&'static str], LineArgs)> {
    Some(match name {
        "Visual Studio Code"
        | "Visual Studio Code (Insiders)"
        | "VSCodium"
        | "VSCodium (Insiders)"
        | "Cursor"
        | "Windsurf" => (&[""], LineArgs::Goto),
        "Sublime Text" | "Zed" => (&[""], LineArgs::Suffix),
        _ => return None,
    })
}

/// What [`line_tool`] is keyed by: the bundle identifier on macOS, the
/// editor's name elsewhere.
fn line_tool_key(editor: &FoundEditor) -> &str {
    if cfg!(target_os = "macos") {
        &editor.bundle_id
    } else {
        &editor.name
    }
}

/// The program and arguments that open `target` at `line` (1-based) in
/// `editor`, when its bundle has a command line tool that can.
fn line_command(editor: &FoundEditor, target: &Path, line: u32) -> Option<(PathBuf, Vec<String>)> {
    let (candidates, syntax) = line_tool(line_tool_key(editor))?;
    let program = candidates
        .iter()
        .map(|rel| {
            if rel.is_empty() {
                editor.path.clone()
            } else {
                editor.path.join(rel)
            }
        })
        .find(|p| p.is_file())?;
    let at = format!("{}:{line}", target.display());
    let args = match syntax {
        LineArgs::Goto => vec!["-g".to_string(), at],
        LineArgs::Suffix => vec![at],
    };
    Some((program, args))
}

/// Whether `editor` can open a file at a line (see [`launch_at_line`]).
pub fn supports_line(editor: &FoundEditor) -> bool {
    line_tool(line_tool_key(editor)).is_some()
}

/// Open `target` at `line` through the editor's command line tool; editors
/// without one (or a missing tool) open the file as [`launch`] does.
pub fn launch_at_line(editor: &FoundEditor, target: &Path, line: u32) -> Result<(), EditorError> {
    match line_command(editor, target, line) {
        Some((program, args)) => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            apps::spawn_detached(&program, &args).or_else(|_| launch(editor, target))
        }
        None => launch(editor, target),
    }
}

/// GHD `openInExternalEditor` when nothing is installed.
pub fn no_editor_error() -> EditorError {
    EditorError {
        message: format!(
            "No suitable editors installed for Corvane to launch. Install {SUGGESTED_EDITOR_NAME} for your default editor."
        ),
        suggest_default_editor: true,
        open_preferences: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editors() -> Vec<FoundEditor> {
        vec![
            FoundEditor {
                name: "Zed".into(),
                bundle_id: "dev.zed.Zed".into(),
                path: "/Applications/Zed.app".into(),
            },
            FoundEditor {
                name: "Cursor".into(),
                bundle_id: "com.todesktop.230313mzl4w4u92".into(),
                path: "/Applications/Cursor.app".into(),
            },
        ]
    }

    #[test]
    fn code_workspace_only_when_single() {
        let dir = tempfile::tempdir().unwrap();
        let code = FoundEditor {
            name: "Visual Studio Code".into(),
            bundle_id: "com.microsoft.VSCode".into(),
            path: "/Applications/Visual Studio Code.app".into(),
        };
        assert_eq!(code_workspace_file(&code, dir.path()), None);
        std::fs::write(dir.path().join("app.code-workspace"), "{}").unwrap();
        assert_eq!(
            code_workspace_file(&code, dir.path()),
            Some(dir.path().join("app.code-workspace"))
        );
        assert_eq!(code_workspace_file(&editors()[0], dir.path()), None);
        std::fs::write(dir.path().join("other.code-workspace"), "{}").unwrap();
        assert_eq!(code_workspace_file(&code, dir.path()), None);
    }

    #[test]
    fn picks_named_or_first() {
        let e = editors();
        assert_eq!(
            find_editor_or_default(&e, None).unwrap().unwrap().name,
            "Zed"
        );
        assert_eq!(
            find_editor_or_default(&e, Some("Cursor"))
                .unwrap()
                .unwrap()
                .name,
            "Cursor"
        );
        let err = find_editor_or_default(&e, Some("Nope")).unwrap_err();
        assert!(err.open_preferences && err.message.contains("'Nope'"));
        assert!(find_editor_or_default(&[], Some("Zed")).unwrap().is_none());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn line_commands_per_editor() {
        let dir = std::env::temp_dir().join(format!("corvane-editors-{}", std::process::id()));
        let app = dir.join("Code.app");
        let bin = app.join("Contents/Resources/app/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("code"), "").unwrap();
        let code = FoundEditor {
            name: "Visual Studio Code".into(),
            bundle_id: "com.microsoft.VSCode".into(),
            path: app.clone(),
        };
        let (program, args) = line_command(&code, Path::new("/r/src/a.rs"), 12).unwrap();
        assert_eq!(program, bin.join("code"));
        assert_eq!(args, ["-g", "/r/src/a.rs:12"]);
        // the bundle lacks the tool: no line command
        let zed = FoundEditor {
            name: "Zed".into(),
            bundle_id: "dev.zed.Zed".into(),
            path: app,
        };
        assert!(supports_line(&zed));
        assert!(line_command(&zed, Path::new("/r/a"), 1).is_none());
        let bbedit = FoundEditor {
            name: "BBEdit".into(),
            bundle_id: "com.barebones.bbedit".into(),
            path: "/Applications/BBEdit.app".into(),
        };
        assert!(!supports_line(&bbedit));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn linux_line_commands_use_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let code_bin = dir.path().join("code");
        std::fs::write(&code_bin, "").unwrap();
        let code = FoundEditor {
            name: "Visual Studio Code".into(),
            bundle_id: "/usr/bin/code".into(),
            path: code_bin.clone(),
        };
        let (program, args) = line_command(&code, Path::new("/r/src/a.rs"), 12).unwrap();
        assert_eq!(program, code_bin);
        assert_eq!(args, ["-g", "/r/src/a.rs:12"]);
        let zed = FoundEditor {
            name: "Zed".into(),
            bundle_id: "/usr/bin/zeditor".into(),
            path: code_bin,
        };
        assert_eq!(
            line_command(&zed, Path::new("/r/a"), 3).unwrap().1,
            ["/r/a:3"]
        );
        let kate = FoundEditor {
            name: "Kate".into(),
            bundle_id: "/usr/bin/kate".into(),
            path: "/usr/bin/kate".into(),
        };
        assert!(!supports_line(&kate));
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn linux_paths_resolve_against_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(
            resolve_linux_path("/usr/bin/code"),
            PathBuf::from("/usr/bin/code")
        );
        assert_eq!(
            resolve_linux_path("~/.local/bin/zed"),
            home.join(".local/bin/zed")
        );
        assert_eq!(
            resolve_linux_path(".local/share/JetBrains/Toolbox/scripts/idea"),
            home.join(".local/share/JetBrains/Toolbox/scripts/idea")
        );
    }

    #[test]
    fn table_has_no_duplicate_names() {
        let mut names: Vec<&str> = EDITORS
            .iter()
            .chain(EXTRA_EDITORS)
            .map(|(n, _)| *n)
            .collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(before, names.len());
    }
}
