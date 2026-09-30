//! Bundled `.gitignore` and license templates for "Create a New Repository"
//! (GHD `ui/add-repository/gitignores.ts` + `licenses.ts`, data from
//! `assets/templates/`, see the README there).
//!
//! Corvane addition (`221-add-license`): Repository › Add License… writes
//! one of the license templates into an existing repository
//! ([`write_license`]); GHD offers licenses only when creating one.

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../assets/templates"]
#[include = "gitignore/*.gitignore"]
#[include = "licenses/*.txt"]
struct Embedded;

/// GHD `ILicense`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct License {
    /// Human-readable name (`title` in the front matter).
    pub name: String,
    /// Featured licenses are listed first (GHD shows a separator after them).
    pub featured: bool,
    pub hidden: bool,
    /// Template body with `[year]`/`{fullname}`-style tokens.
    pub body: String,
}

/// GHD `ILicenseFields`.
#[derive(Clone, Debug, Default)]
pub struct LicenseFields {
    pub fullname: String,
    pub email: String,
    pub project: String,
    pub description: String,
    pub year: String,
}

/// `getGitIgnoreNames`: template names, sorted.
pub fn gitignore_names() -> Vec<String> {
    let mut names: Vec<String> = Embedded::iter()
        .filter_map(|p| {
            p.strip_prefix("gitignore/")
                .and_then(|n| n.strip_suffix(".gitignore"))
                .map(str::to_string)
        })
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// The text of a named template.
pub fn gitignore_text(name: &str) -> Option<String> {
    Embedded::get(&format!("gitignore/{name}.gitignore"))
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
}

/// `getLicenses`: featured first, then by name; hidden ones excluded.
pub fn licenses() -> Vec<License> {
    let mut out: Vec<License> = Embedded::iter()
        .filter(|p| p.starts_with("licenses/"))
        .filter_map(|p| {
            let file = Embedded::get(&p)?;
            parse_license(&String::from_utf8_lossy(&file.data))
        })
        .filter(|l| !l.hidden)
        .collect();
    out.sort_by(|a, b| {
        b.featured
            .cmp(&a.featured)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// Split the choosealicense front matter from the body.
fn parse_license(text: &str) -> Option<License> {
    let rest = text.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let (front, body) = rest.split_at(end);
    let body = body.trim_start_matches("\n---").trim_start_matches('\n');
    let mut name = None;
    let mut featured = false;
    let mut hidden = false;
    for line in front.lines() {
        if let Some(v) = line.strip_prefix("title:") {
            name = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("featured:") {
            featured = v.trim() == "true";
        } else if let Some(v) = line.strip_prefix("hidden:") {
            hidden = v.trim() == "true";
        }
    }
    Some(License {
        name: name?,
        featured,
        hidden,
        body: body.to_string(),
    })
}

/// `replaceTokens`: the templates use both `[token]` and `{token}`.
pub fn render_license(body: &str, fields: &LicenseFields) -> String {
    let mut out = body.to_string();
    for (token, value) in [
        ("fullname", fields.fullname.as_str()),
        ("email", fields.email.as_str()),
        ("project", fields.project.as_str()),
        ("description", fields.description.as_str()),
        ("year", fields.year.as_str()),
    ] {
        out = out
            .replace(&format!("[{token}]"), value)
            .replace(&format!("{{{token}}}"), value);
    }
    out
}

/// An existing license file in `dir` (`LICENSE`, `LICENSE.md`, `COPYING`,
/// `LICENCE.txt`, …, any case), `None` when there is none.
pub fn existing_license_file(dir: &std::path::Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stem = name
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let is_license = matches!(stem.as_str(), "license" | "licence" | "copying");
        is_license.then_some(name)
    })
}

/// `221-add-license`: write `text` to `<dir>/LICENSE`. Never replaces a
/// file: an existing license file (see [`existing_license_file`]) is an
/// error, and the file is created with `create_new`.
pub fn write_license(dir: &std::path::Path, text: &str) -> Result<std::path::PathBuf, String> {
    use std::io::Write;
    if let Some(name) = existing_license_file(dir) {
        return Err(format!(
            "This repository already has a license file ({name}). Corvane won't replace it."
        ));
    }
    let path = dir.join("LICENSE");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|err| format!("Couldn't create {}: {err}", path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|err| format!("Couldn't write {}: {err}", path.display()))?;
    Ok(path)
}

/// `writeGitAttributes` contents.
pub const GIT_ATTRIBUTES: &str =
    "# Auto detect text files and perform LF normalization\n* text=auto\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_license_never_replaces_a_license() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(existing_license_file(dir.path()), None);
        let path = write_license(dir.path(), "MIT").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "MIT");
        assert_eq!(
            existing_license_file(dir.path()).as_deref(),
            Some("LICENSE")
        );
        assert!(write_license(dir.path(), "other").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "MIT");

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Copying.txt"), "GPL").unwrap();
        std::fs::write(dir.path().join("licenses.json"), "{}").unwrap();
        assert!(write_license(dir.path(), "MIT").is_err());
        assert!(!dir.path().join("LICENSE").exists());
    }

    #[test]
    fn templates_are_bundled() {
        let names = gitignore_names();
        assert!(names.iter().any(|n| n == "Rust"));
        assert!(gitignore_text("Rust").unwrap().contains("target"));
        let licenses = licenses();
        assert!(licenses.first().is_some_and(|l| l.featured));
        let mit = licenses.iter().find(|l| l.name == "MIT License").unwrap();
        assert!(mit.body.starts_with("MIT License"));
        let text = render_license(
            mit.body.as_str(),
            &LicenseFields {
                fullname: "Ada".into(),
                year: "2026".into(),
                ..Default::default()
            },
        );
        assert!(text.contains("Copyright (c) 2026 Ada"));
    }
}
