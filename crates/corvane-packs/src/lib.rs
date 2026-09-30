//! On-demand packs: a minisign-signed manifest on GitHub
//! Releases lists archives (`syntax-extended`, `tree-sitter-all`,
//! `tree-sitter-rest`, later `git-portable` and `git-lfs`) with their sha256; a pack is downloaded to
//! `~/Library/Caches/Corvane/packs/`, checked against the manifest's sha256,
//! unpacked into `~/Library/Application Support/Corvane/packs/<name>/<version>/`
//! and marked installed. The `default` build fetches packs on demand; the
//! `full` build (`bundled-*` cargo features in the crates that use them)
//! compiles the same data in and never needs a download.
//!
//! GitHub Desktop has no equivalent: Electron ships every grammar and a
//! private git; Corvane keeps the default bundle lean instead.
//!
//! Entries of native packs (the tree-sitter grammar libraries) carry a
//! `target` ([`pack_target`]); entries of a kind or target this build does
//! not know are skipped, so a manifest can grow without breaking older apps.
//!
//! Testing hook: `CORVANE_PACKS_MANIFEST=<url or file path>` replaces the
//! manifest location.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Where releases publish the manifest (`packs-manifest.json` + `.minisig`
/// next to the app assets, see `packaging/release.md`).
pub const MANIFEST_URL: &str =
    "https://github.com/wasi-master/corvane/releases/latest/download/packs-manifest.json";

/// The manifest schema this build reads.
pub const MANIFEST_SCHEMA: u32 = 1;

const USER_AGENT: &str = concat!("Corvane/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Error)]
pub enum PackError {
    #[error("the packs manifest could not be reached: {0}")]
    Network(String),
    #[error("the server answered with status {0}")]
    Status(u16),
    #[error("the packs manifest could not be read: {0}")]
    Manifest(String),
    #[error("the packs manifest's signature does not match the key compiled into Corvane ({0})")]
    Signature(String),
    #[error("the packs manifest is for a newer Corvane (schema {0})")]
    Schema(u32),
    #[error("pack {0} is not in the manifest")]
    NotListed(String),
    #[error("pack {name} {version} needs Corvane {min_app} or newer")]
    NeedsNewerApp {
        name: String,
        version: String,
        min_app: String,
    },
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("the downloaded pack is corrupt (sha256 mismatch)")]
    Checksum,
    #[error("{0}")]
    Install(String),
}

/// What a pack contains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackKind {
    /// two-face's full grammar set as a syntect dump (`syntaxes.packdump`).
    SyntaxExtended,
    /// dugite-native git + git-lfs.
    GitPortable,
    /// git-lfs alone.
    GitLfs,
    /// every tree-sitter grammar (`corvane-grammars` as a dynamic library).
    TreeSitterAll,
    /// the tree-sitter grammars for languages no CodeMirror port covers.
    TreeSitterRest,
}

/// The index of a tree-sitter pack: its grammars and the gzipped library
/// (`grammars/<unit>.dylib.gz`) each one is in.
const GRAMMAR_INDEX: &str = "index.json";

/// The `target` of manifest entries for native packs this build can load:
/// `<os>-<arch>` (`macos-aarch64`, `linux-x86_64`). Native packs are per
/// architecture rather than universal: the grammar tables are large.
pub fn pack_target() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos-aarch64"
    } else if cfg!(target_os = "macos") {
        "macos-x86_64"
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        "windows-aarch64"
    } else if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "linux-aarch64"
    } else {
        "linux-x86_64"
    }
}

impl PackKind {
    /// The manifest name of the pack.
    pub fn name(self) -> &'static str {
        match self {
            PackKind::SyntaxExtended => "syntax-extended",
            PackKind::GitPortable => "git-portable",
            PackKind::GitLfs => "git-lfs",
            PackKind::TreeSitterAll => "tree-sitter-all",
            PackKind::TreeSitterRest => "tree-sitter-rest",
        }
    }

    /// What Settings › Advanced calls it.
    pub fn title(self) -> &'static str {
        match self {
            PackKind::SyntaxExtended => "Extended syntax highlighting",
            PackKind::GitPortable => "Portable Git",
            PackKind::GitLfs => "Git LFS",
            PackKind::TreeSitterAll => "Tree-sitter grammars",
            PackKind::TreeSitterRest => "Tree-sitter grammars for other languages",
        }
    }

    /// The file the pack's consumer opens, relative to the install directory.
    pub fn entry_file(self) -> &'static str {
        match self {
            PackKind::SyntaxExtended => "syntaxes.packdump",
            PackKind::GitPortable => "bin/git",
            PackKind::GitLfs => "bin/git-lfs",
            PackKind::TreeSitterAll | PackKind::TreeSitterRest => GRAMMAR_INDEX,
        }
    }
}

/// One manifest row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackEntry {
    pub name: String,
    pub version: String,
    /// Oldest Corvane that can use this pack version.
    pub min_app: String,
    pub url: String,
    /// Hex sha256 of the archive at `url`.
    pub sha256: String,
    pub size: u64,
    pub kind: PackKind,
    /// The platform of a native pack ([`pack_target`]); `None` for data
    /// packs every platform reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema: u32,
    pub packs: Vec<PackEntry>,
}

impl PackManifest {
    /// The newest entry of `kind` for this platform that this app version
    /// can use.
    pub fn entry_for(&self, kind: PackKind, app_version: &str) -> Option<&PackEntry> {
        self.packs
            .iter()
            .filter(|p| p.kind == kind)
            .filter(|p| p.target.as_deref().is_none_or(|t| t == pack_target()))
            .filter(|p| !version_is_newer(&p.min_app, app_version))
            .max_by(|a, b| compare_versions(&a.version, &b.version))
    }
}

/// An installed pack (`<packs>/<name>/<version>/`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPack {
    pub kind: PackKind,
    pub version: String,
    pub path: PathBuf,
}

impl InstalledPack {
    /// The file the consumer opens.
    pub fn entry_path(&self) -> PathBuf {
        self.path.join(self.kind.entry_file())
    }
}

/// The marker written after a successful install.
#[derive(Debug, Serialize, Deserialize)]
struct InstalledMarker {
    kind: PackKind,
    version: String,
    sha256: String,
}

const MARKER: &str = "installed.json";

/// `~/Library/Application Support/Corvane/packs` (`CORVANE_PACKS_DIR`
/// overrides it for tests).
pub fn packs_dir() -> PathBuf {
    std::env::var_os("CORVANE_PACKS_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| corvane_platform::paths::app_support_dir().join("packs"))
}

/// `~/Library/Caches/Corvane/packs`
fn download_dir() -> PathBuf {
    corvane_platform::paths::cache_dir().join("packs")
}

/// The manifest location: `CORVANE_PACKS_MANIFEST` or [`MANIFEST_URL`].
pub fn manifest_url() -> String {
    std::env::var("CORVANE_PACKS_MANIFEST")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| MANIFEST_URL.to_string())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .new_agent()
}

fn fetch_bytes(url: &str, limit: u64) -> Result<Vec<u8>, PackError> {
    if let Some(path) = url.strip_prefix("file://") {
        return Ok(std::fs::read(path)?);
    }
    if !url.contains("://") {
        return Ok(std::fs::read(url)?);
    }
    let mut response = agent()
        .get(url)
        .call()
        .map_err(|err| PackError::Network(err.to_string()))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(PackError::Status(status));
    }
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|err| PackError::Network(err.to_string()))
}

/// Fetch and verify the manifest (`<url>` + `<url>.minisig`, signed with the
/// release key compiled into `corvane_platform::updater::PUBLIC_KEY`).
pub fn fetch_manifest() -> Result<PackManifest, PackError> {
    let url = manifest_url();
    debug!(%url, "fetching the packs manifest");
    let body = fetch_bytes(&url, 4 * 1024 * 1024)?;
    let signature = fetch_bytes(&format!("{url}.minisig"), 64 * 1024)?;
    corvane_platform::updater::verify_bytes(&body, &signature)
        .map_err(|err| PackError::Signature(err.to_string()))?;
    parse_manifest(&body)
}

/// Parse a manifest body (already verified). Entries this build cannot read
/// (a pack kind added later) are skipped.
pub fn parse_manifest(body: &[u8]) -> Result<PackManifest, PackError> {
    #[derive(Deserialize)]
    struct Raw {
        schema: u32,
        packs: Vec<serde_json::Value>,
    }
    let raw: Raw =
        serde_json::from_slice(body).map_err(|err| PackError::Manifest(err.to_string()))?;
    if raw.schema > MANIFEST_SCHEMA {
        return Err(PackError::Schema(raw.schema));
    }
    let packs = raw
        .packs
        .into_iter()
        .filter_map(|value| match serde_json::from_value::<PackEntry>(value) {
            Ok(entry) => Some(entry),
            Err(err) => {
                debug!("skipping a packs manifest entry: {err}");
                None
            }
        })
        .collect();
    Ok(PackManifest {
        schema: raw.schema,
        packs,
    })
}

/// The installed version of `kind`, newest first when several are present.
pub fn installed(kind: PackKind) -> Option<InstalledPack> {
    let dir = packs_dir().join(kind.name());
    let mut versions: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join(MARKER).is_file() && p.join(kind.entry_file()).exists())
        .filter_map(|p| Some((p.file_name()?.to_str()?.to_string(), p)))
        .collect();
    versions.sort_by(|a, b| compare_versions(&b.0, &a.0));
    versions
        .into_iter()
        .next()
        .map(|(version, path)| InstalledPack {
            kind,
            version,
            path,
        })
}

/// Download, check and unpack `entry`; `progress(received, total)` reports
/// the download. Returns the installed pack.
pub fn install(
    entry: &PackEntry,
    app_version: &str,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<InstalledPack, PackError> {
    if version_is_newer(&entry.min_app, app_version) {
        return Err(PackError::NeedsNewerApp {
            name: entry.name.clone(),
            version: entry.version.clone(),
            min_app: entry.min_app.clone(),
        });
    }
    let archive = download_archive(entry, progress)?;
    let target = packs_dir().join(&entry.name).join(&entry.version);
    let staging = packs_dir()
        .join(&entry.name)
        .join(format!(".{}.partial", entry.version));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    unpack(&archive, &staging)?;
    if !staging.join(entry.kind.entry_file()).exists() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(PackError::Install(format!(
            "the archive has no {}",
            entry.kind.entry_file()
        )));
    }
    let marker = InstalledMarker {
        kind: entry.kind,
        version: entry.version.clone(),
        sha256: entry.sha256.clone(),
    };
    std::fs::write(
        staging.join(MARKER),
        serde_json::to_vec_pretty(&marker).map_err(|err| PackError::Install(err.to_string()))?,
    )?;
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::rename(&staging, &target)?;
    let _ = std::fs::remove_file(&archive);
    info!(pack = %entry.name, version = %entry.version, path = %target.display(), "pack installed");
    Ok(InstalledPack {
        kind: entry.kind,
        version: entry.version.clone(),
        path: target,
    })
}

/// Remove every installed version of `kind`.
pub fn uninstall(kind: PackKind) -> Result<(), PackError> {
    let dir = packs_dir().join(kind.name());
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
        info!(pack = kind.name(), "pack removed");
    }
    Ok(())
}

fn download_archive(
    entry: &PackEntry,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<PathBuf, PackError> {
    let dir = download_dir();
    std::fs::create_dir_all(&dir)?;
    let file_name = entry
        .url
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("pack.zip");
    let archive = dir.join(format!("{}-{}-{file_name}", entry.name, entry.version));
    let part = dir.join(format!("{}.part", archive.display()));
    let mut hasher = Sha256::new();
    if entry.url.contains("://") && !entry.url.starts_with("file://") {
        info!(url = %entry.url, "downloading pack");
        let mut response = agent()
            .get(&entry.url)
            .call()
            .map_err(|err| PackError::Network(err.to_string()))?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(PackError::Status(status));
        }
        let total = response
            .body_mut()
            .content_length()
            .or(Some(entry.size).filter(|s| *s > 0));
        let mut file = std::fs::File::create(&part)?;
        let mut reader = response.body_mut().as_reader();
        let mut buf = vec![0u8; 256 * 1024];
        let mut received = 0u64;
        progress(0, total);
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n])?;
            received += n as u64;
            progress(received, total);
        }
        file.flush()?;
    } else {
        // a local archive (tests, `CORVANE_PACKS_MANIFEST` pointing at a folder)
        let source = entry.url.strip_prefix("file://").unwrap_or(&entry.url);
        let bytes = std::fs::read(source)?;
        hasher.update(&bytes);
        std::fs::write(&part, &bytes)?;
        progress(bytes.len() as u64, Some(bytes.len() as u64));
    }
    let digest = format!("{:x}", hasher.finalize());
    if !digest.eq_ignore_ascii_case(entry.sha256.trim()) {
        warn!(expected = %entry.sha256, actual = %digest, "pack checksum mismatch");
        let _ = std::fs::remove_file(&part);
        return Err(PackError::Checksum);
    }
    std::fs::rename(&part, &archive)?;
    Ok(archive)
}

/// `.zip` through `ditto` (keeps executable bits and signatures),
/// `.tar.gz` / `.tgz` through `tar`.
fn unpack(archive: &Path, into: &Path) -> Result<(), PackError> {
    let name = archive.to_string_lossy();
    let status = if name.ends_with(".zip") {
        std::process::Command::new("/usr/bin/ditto")
            .args(["-x", "-k"])
            .arg(archive)
            .arg(into)
            .status()
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        std::process::Command::new("/usr/bin/tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(into)
            .status()
    } else {
        return Err(PackError::Install(format!(
            "unknown archive type: {}",
            archive.display()
        )));
    };
    let status = status.map_err(|err| PackError::Install(format!("could not unpack: {err}")))?;
    if !status.success() {
        return Err(PackError::Install(format!(
            "unpacking {} failed ({status})",
            archive.display()
        )));
    }
    Ok(())
}

/// Hex sha256 of a file (used by the release script's test and callers
/// that build manifests).
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn parse_version(text: &str) -> Option<[u64; 3]> {
    let core = text.trim().trim_start_matches('v');
    let core = core.split_once('-').map(|(c, _)| c).unwrap_or(core);
    let mut parts = [0u64; 3];
    for (ix, piece) in core.split('.').enumerate() {
        if ix >= 3 {
            return None;
        }
        parts[ix] = piece.parse().ok()?;
    }
    Some(parts)
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    parse_version(a).cmp(&parse_version(b))
}

/// Is `candidate` newer than `current`? (Pre-release tags are ignored.)
fn version_is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(c), Some(r)) => c > r,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: PackKind, version: &str, min_app: &str) -> PackEntry {
        PackEntry {
            name: kind.name().to_string(),
            version: version.to_string(),
            min_app: min_app.to_string(),
            url: format!("https://example.invalid/{}-{version}.zip", kind.name()),
            sha256: String::new(),
            size: 0,
            kind,
            target: None,
        }
    }

    #[test]
    fn native_packs_match_this_platform() {
        let mut here = entry(PackKind::TreeSitterAll, "1.0.0", "0.1.0");
        here.target = Some(pack_target().to_string());
        let mut elsewhere = entry(PackKind::TreeSitterAll, "2.0.0", "0.1.0");
        elsewhere.target = Some("plan9-mips".to_string());
        let manifest = PackManifest {
            schema: 1,
            packs: vec![here, elsewhere],
        };
        let pick = manifest
            .entry_for(PackKind::TreeSitterAll, "0.1.0")
            .unwrap();
        assert_eq!(pick.version, "1.0.0");
        assert!(
            manifest
                .entry_for(PackKind::TreeSitterRest, "0.1.0")
                .is_none()
        );
    }

    #[test]
    fn unknown_kinds_are_skipped() {
        let json = r#"{"schema":1,"packs":[
            {"name":"future","version":"1.0.0","min_app":"0.1.0","url":"u","sha256":"ab","size":1,"kind":"future-kind"},
            {"name":"tree-sitter-all","version":"1.0.0","min_app":"0.1.0","url":"u","sha256":"ab","size":1,"kind":"tree-sitter-all","target":"macos-aarch64"}
        ]}"#;
        let manifest = parse_manifest(json.as_bytes()).unwrap();
        assert_eq!(manifest.packs.len(), 1);
        assert_eq!(manifest.packs[0].kind, PackKind::TreeSitterAll);
        assert_eq!(manifest.packs[0].target.as_deref(), Some("macos-aarch64"));
    }

    #[test]
    fn manifest_picks_the_newest_compatible_entry() {
        let manifest = PackManifest {
            schema: 1,
            packs: vec![
                entry(PackKind::SyntaxExtended, "1.0.0", "0.1.0"),
                entry(PackKind::SyntaxExtended, "1.2.0", "0.1.0"),
                entry(PackKind::SyntaxExtended, "2.0.0", "0.5.0"),
                entry(PackKind::GitLfs, "3.5.0", "0.1.0"),
            ],
        };
        let pick = manifest
            .entry_for(PackKind::SyntaxExtended, "0.1.0")
            .unwrap();
        assert_eq!(pick.version, "1.2.0");
        let pick = manifest
            .entry_for(PackKind::SyntaxExtended, "0.5.0")
            .unwrap();
        assert_eq!(pick.version, "2.0.0");
        assert!(manifest.entry_for(PackKind::GitPortable, "0.1.0").is_none());
    }

    #[test]
    fn manifest_json_round_trips_and_newer_schemas_are_refused() {
        let json = r#"{"schema":1,"packs":[{"name":"syntax-extended","version":"1.0.0","min_app":"0.1.0","url":"https://example.invalid/x.zip","sha256":"ab","size":12,"kind":"syntax-extended"}]}"#;
        let manifest = parse_manifest(json.as_bytes()).unwrap();
        assert_eq!(manifest.packs[0].kind, PackKind::SyntaxExtended);
        assert!(matches!(
            parse_manifest(br#"{"schema":9,"packs":[]}"#),
            Err(PackError::Schema(9))
        ));
    }

    #[test]
    fn installs_a_local_zip_and_checks_its_sha256() {
        let dir = std::env::temp_dir().join(format!("corvane-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/syntaxes.packdump"), b"not really a dump").unwrap();
        let zip = dir.join("syntax-extended-1.0.0.zip");
        let status = std::process::Command::new("/usr/bin/ditto")
            .args(["-c", "-k"])
            .arg(dir.join("src"))
            .arg(&zip)
            .status()
            .unwrap();
        assert!(status.success());
        let sha = sha256_file(&zip).unwrap();
        let mut entry = PackEntry {
            name: "syntax-extended".into(),
            version: "1.0.0".into(),
            min_app: "0.1.0".into(),
            url: zip.to_string_lossy().into_owned(),
            sha256: sha,
            size: 0,
            kind: PackKind::SyntaxExtended,
            target: None,
        };
        // SAFETY: the only test touching CORVANE_PACKS_DIR
        unsafe { std::env::set_var("CORVANE_PACKS_DIR", dir.join("packs")) };
        let real_packs = packs_dir();
        assert!(real_packs.starts_with(&dir));
        let mut progress = |_: u64, _: Option<u64>| {};
        let installed = install(&entry, "0.1.0", &mut progress).unwrap();
        assert!(installed.entry_path().is_file());
        assert_eq!(installed.version, "1.0.0");
        assert_eq!(
            super::installed(PackKind::SyntaxExtended).map(|p| p.version),
            Some("1.0.0".to_string())
        );
        // a wrong checksum is refused before anything is unpacked
        entry.version = "1.0.1".into();
        entry.sha256 = "00".into();
        assert!(matches!(
            install(&entry, "0.1.0", &mut progress),
            Err(PackError::Checksum)
        ));
        // an app too old for the pack is told so
        entry.min_app = "99.0.0".into();
        assert!(matches!(
            install(&entry, "0.1.0", &mut progress),
            Err(PackError::NeedsNewerApp { .. })
        ));
        uninstall(PackKind::SyntaxExtended).unwrap();
        assert!(super::installed(PackKind::SyntaxExtended).is_none());
        assert!(!real_packs.join("syntax-extended").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
