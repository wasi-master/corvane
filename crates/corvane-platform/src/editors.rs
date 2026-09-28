//! External editor detection - GHD `lib/editors/darwin.ts` (the bundle
//! identifier table is copied verbatim) and `lib/editors/launch.ts`.

use std::path::{Path, PathBuf};

use crate::apps;

/// Friendly name + bundle identifiers, in GHD's order (the first installed
/// editor is the default when none is selected in Settings).
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

/// GHD `suggestedExternalEditor`.
pub const SUGGESTED_EDITOR_NAME: &str = "Visual Studio Code";
pub const SUGGESTED_EDITOR_URL: &str = "https://code.visualstudio.com";

/// GHD `FoundEditor`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundEditor {
    pub name: String,
    pub bundle_id: String,
    pub path: PathBuf,
}

/// Every known editor installed on this machine, in table order. Costs one
/// LaunchServices lookup per identifier; run it off the main thread.
pub fn available_editors() -> Vec<FoundEditor> {
    EDITORS
        .iter()
        .filter_map(|(name, ids)| {
            apps::first_installed(ids).map(|(bundle_id, path)| FoundEditor {
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
                    "The editor '{name}' could not be found. Please open Settings and choose an available editor."
                ),
                suggest_default_editor: false,
                open_preferences: true,
            }),
        None => Ok(editors.first()),
    }
}

/// GHD `launchExternalEditor`: `open -a <bundle> <path>`, detached.
pub fn launch(editor: &FoundEditor, target: &Path) -> Result<(), EditorError> {
    if !editor.path.exists() {
        return Err(EditorError {
            message: format!(
                "Could not find executable for '{}' at path '{}'. Please open Settings and select an available editor.",
                editor.name,
                editor.path.display()
            ),
            suggest_default_editor: false,
            open_preferences: true,
        });
    }
    apps::open_with_app(&editor.path, target).map_err(|err| EditorError {
        message: if err.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "Corvane doesn't have the proper permissions to start '{}'. Please open Settings and try another editor.",
                editor.name
            )
        } else {
            format!(
                "Something went wrong while trying to start '{}'. Please open Settings and try another editor.",
                editor.name
            )
        },
        suggest_default_editor: false,
        open_preferences: true,
    })
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
    fn table_has_no_duplicate_names() {
        let mut names: Vec<&str> = EDITORS.iter().map(|(n, _)| *n).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(before, names.len());
    }
}
