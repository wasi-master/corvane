//! Custom editor / shell (GHD `lib/custom-integration.ts`): an executable or
//! macOS app bundle plus arguments where `%TARGET_PATH%` stands for the
//! repository (or file) path.

use std::path::Path;

use crate::apps;

pub const TARGET_PATH_ARGUMENT: &str = "%TARGET_PATH%";

/// `stringArgv`: POSIX-ish word splitting with single/double quotes and
/// backslash escapes. `None` for unbalanced quotes.
pub fn parse_arguments(input: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some('"') if c == '\\' => {
                if let Some(next) = chars.next() {
                    if !matches!(next, '"' | '\\' | '$' | '`') {
                        current.push('\\');
                    }
                    current.push(next);
                }
            }
            Some(_) => current.push(c),
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    in_word = true;
                }
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                        in_word = true;
                    }
                }
                c if c.is_whitespace() => {
                    if in_word {
                        out.push(std::mem::take(&mut current));
                        in_word = false;
                    }
                }
                c => {
                    current.push(c);
                    in_word = true;
                }
            },
        }
    }
    if quote.is_some() {
        return None;
    }
    if in_word {
        out.push(current);
    }
    Some(out)
}

/// `expandTargetPathArgument`
pub fn expand_target_path(args: &[String], target: &str) -> Vec<String> {
    args.iter()
        .map(|arg| {
            if arg == &format!("'{TARGET_PATH_ARGUMENT}'")
                || arg == &format!("\"{TARGET_PATH_ARGUMENT}\"")
            {
                target.to_string()
            } else {
                arg.replace(TARGET_PATH_ARGUMENT, target)
            }
        })
        .collect()
}

/// `checkTargetPathArgument`
pub fn has_target_path(args: &[String]) -> bool {
    args.iter().any(|a| a.contains(TARGET_PATH_ARGUMENT))
}

/// Cheap synchronous check for the Settings form: an executable file, or a
/// `.app` bundle directory on macOS.
pub fn path_looks_valid(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let p = Path::new(path);
    let Ok(meta) = std::fs::metadata(p) else {
        return false;
    };
    if meta.is_dir() {
        return cfg!(target_os = "macos") && path.ends_with(".app");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

/// `getAppBundleID`: `mdls -name kMDItemCFBundleIdentifier -raw <app>`.
pub fn app_bundle_id(app: &Path) -> Option<String> {
    if !app.to_string_lossy().ends_with(".app") {
        return None;
    }
    let out = std::process::Command::new("/usr/bin/mdls")
        .args(["-name", "kMDItemCFBundleIdentifier", "-raw"])
        .arg(app)
        .output()
        .ok()?;
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!id.is_empty() && id != "(null)").then_some(id)
}

/// `launchCustomExternalEditor` / `launchCustomShell`: `open -b <bundle> args…`
/// for app bundles, a direct spawn otherwise.
pub fn launch(path: &str, arguments: &str, target: &Path) -> Result<(), String> {
    let args = parse_arguments(arguments).ok_or("These arguments are not valid.")?;
    let args = expand_target_path(&args, &target.to_string_lossy());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if path.ends_with(".app") {
        let bundle_id = app_bundle_id(Path::new(path))
            .ok_or_else(|| format!("Could not read the bundle identifier of '{path}'."))?;
        let mut open_args = vec!["-b", bundle_id.as_str()];
        open_args.extend(args.iter().copied());
        apps::spawn_detached("/usr/bin/open", &open_args)
    } else {
        apps::spawn_detached(path, &args)
    }
    .map_err(|err| format!("Could not start '{path}': {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_like_a_shell() {
        assert_eq!(
            parse_arguments(r#"-n "%TARGET_PATH%" 'a b' c\ d"#).unwrap(),
            vec!["-n", "%TARGET_PATH%", "a b", "c d"]
        );
        assert_eq!(parse_arguments(r#""unbalanced"#), None);
        let args = parse_arguments("--dir=%TARGET_PATH% '%TARGET_PATH%'").unwrap();
        assert!(has_target_path(&args));
        assert_eq!(expand_target_path(&args, "/r"), vec!["--dir=/r", "/r"]);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn validates_paths() {
        assert!(path_looks_valid("/bin/ls"));
        assert!(path_looks_valid("/System/Applications/TextEdit.app"));
        assert!(!path_looks_valid("/etc/hosts"));
        assert!(!path_looks_valid(""));
    }
}
