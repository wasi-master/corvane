//! License and open source notices - GHD `ui/acknowledgements/acknowledgements.tsx`
//! reads `static/licenses.json`; Corvane reads `assets/acknowledgements.json`,
//! generated from `cargo about` by `packaging/acknowledgements.sh` and embedded
//! in the binary (the app never runs cargo-about).

use std::sync::Arc;

use serde::Deserialize;

#[derive(Deserialize)]
struct Raw {
    app_license: String,
    texts: Vec<String>,
    crates: Vec<RawCrate>,
}

#[derive(Deserialize)]
struct RawCrate {
    name: String,
    version: String,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    texts: Vec<usize>,
}

/// One distributed library (GHD `ILicense` keyed by `name@version`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Acknowledgement {
    pub name: String,
    pub version: String,
    /// GHD `normalizedGitHubURL(repository)`.
    pub repository: Option<String>,
    /// SPDX expression (`License: …` when there is no text).
    pub license: Option<String>,
    /// The license texts that apply, shared between crates.
    pub texts: Vec<Arc<str>>,
}

/// Everything the dialog shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Acknowledgements {
    /// Corvane's own `LICENSE`.
    pub app_license: String,
    pub libraries: Vec<Acknowledgement>,
}

/// GHD `normalizedGitHubURL`.
fn normalized_url(url: &str) -> String {
    let url = url
        .replace("git+https://github.com", "https://github.com")
        .replace("git+ssh://git@github.com", "https://github.com");
    url.strip_suffix(".git").map(str::to_string).unwrap_or(url)
}

/// Parse `acknowledgements.json`; `None` when it is missing or malformed.
pub fn parse(bytes: &[u8]) -> Option<Acknowledgements> {
    let raw: Raw = serde_json::from_slice(bytes).ok()?;
    let texts: Vec<Arc<str>> = raw.texts.into_iter().map(Arc::from).collect();
    let libraries = raw
        .crates
        .into_iter()
        .map(|c| Acknowledgement {
            name: c.name,
            version: c.version,
            repository: c.repository.as_deref().map(normalized_url),
            license: c.license,
            texts: c
                .texts
                .into_iter()
                .filter_map(|ix| texts.get(ix).cloned())
                .collect(),
        })
        .collect();
    Some(Acknowledgements {
        app_license: raw.app_license,
        libraries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shared_texts_and_normalizes_urls() {
        let json = br#"{"app_license":"MIT License","texts":["MIT text","Apache text"],
            "crates":[
              {"name":"a","version":"1.0.0","repository":"git+https://github.com/o/a.git","license":"MIT OR Apache-2.0","texts":[0,1]},
              {"name":"b","version":"0.2.0","license":"MIT","texts":[0,7]}]}"#;
        let ack = parse(json).unwrap();
        assert_eq!(ack.app_license, "MIT License");
        assert_eq!(ack.libraries.len(), 2);
        assert_eq!(
            ack.libraries[0].repository.as_deref(),
            Some("https://github.com/o/a")
        );
        assert_eq!(ack.libraries[0].texts.len(), 2);
        // out-of-range indices are dropped; texts are shared
        assert_eq!(ack.libraries[1].texts.len(), 1);
        assert!(Arc::ptr_eq(
            &ack.libraries[0].texts[0],
            &ack.libraries[1].texts[0]
        ));
    }

    #[test]
    fn malformed_is_none() {
        assert!(parse(b"{").is_none());
    }

    #[test]
    fn bundled_file_parses() {
        let bytes = include_bytes!("../../../assets/acknowledgements.json");
        let ack = parse(bytes).expect("assets/acknowledgements.json");
        assert!(!ack.libraries.is_empty());
        assert!(
            ack.libraries
                .iter()
                .all(|l| !l.texts.is_empty() || l.license.is_some())
        );
    }
}
