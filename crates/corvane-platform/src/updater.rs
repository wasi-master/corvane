//! Self-updater - the place of Electron's `autoUpdater` + Squirrel in GHD
//! (`main-process/main.ts` `autoUpdater.*`, `ui/lib/update-store.ts`),
//! where the GitHub Releases API is the feed
//! (`GET /repos/wasi-master/corvane/releases/latest`), the release's macOS
//! `.zip` is downloaded to `~/Library/Caches/Corvane/updates/`, verified with
//! its minisign signature against the public key compiled into this binary,
//! unpacked with `ditto` (keeps the ad-hoc signature intact), swapped in for
//! the running bundle (`Corvane.app` → `Corvane.app.old`, new bundle moved
//! in) and opened again once this process has exited. The `.old` bundle is
//! removed at the next launch.
//!
//! Files this app writes carry no quarantine attribute, so the ad-hoc signed
//! update launches without a Gatekeeper prompt.
//!
//! A bundle installed by Homebrew is never swapped: `brew upgrade corvane`
//! owns it (see [`is_homebrew_install`]).
//!
//! Testing hooks: `CORVANE_UPDATE_FEED=<url>` replaces the feed URL,
//! `CORVANE_UPDATE_PUBLIC_KEY` (build time) replaces the public key.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use tracing::{debug, info, warn};

/// GHD `__UPDATES_URL__`: the release feed.
pub const RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/wasi-master/corvane/releases/latest";

/// A syntactically valid minisign key (all-zero key id and key) that signs
/// nothing; `build.rs` warns when it is what gets compiled in.
const PLACEHOLDER_PUBLIC_KEY: &str = "RWQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// The minisign public key releases are verified against, from
/// `CORVANE_UPDATE_PUBLIC_KEY` at build time (`packaging/release.md`).
pub const PUBLIC_KEY: &str = match option_env!("CORVANE_UPDATE_PUBLIC_KEY") {
    Some(key) => key,
    None => PLACEHOLDER_PUBLIC_KEY,
};

/// True when this binary was built without a real signing key: every
/// signature check will fail.
pub fn public_key_is_placeholder() -> bool {
    let key = PUBLIC_KEY.trim();
    key.is_empty() || key == PLACEHOLDER_PUBLIC_KEY
}

const USER_AGENT: &str = concat!("Corvane/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("the release feed could not be reached: {0}")]
    Network(String),
    #[error("the release feed answered with status {0}")]
    Status(u16),
    #[error("the release feed could not be read: {0}")]
    Feed(String),
    #[error("release {0} has no macOS .zip asset")]
    NoAsset(String),
    #[error("release {0} has no minisign signature (.minisig) next to its .zip")]
    NoSignature(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("the download could not be verified: {0}")]
    Signature(String),
    #[error("{0}")]
    Install(String),
}

/// A newer release than the running version, with the asset to install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// `tag_name` without its `v`.
    pub version: String,
    pub tag: String,
    pub name: Option<String>,
    /// The Markdown release notes.
    pub body: Option<String>,
    /// ISO-8601.
    pub published_at: Option<String>,
    pub html_url: String,
    pub zip_name: String,
    pub zip_url: String,
    pub zip_size: u64,
    pub signature_url: String,
}

#[derive(Debug, Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Debug, Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// The feed URL: `CORVANE_UPDATE_FEED` or the GitHub Releases API.
pub fn feed_url() -> String {
    std::env::var("CORVANE_UPDATE_FEED")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| RELEASES_LATEST_URL.to_string())
}

fn agent(global_timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_global(global_timeout)
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .new_agent()
}

/// Ask the feed for the latest release; `Ok(None)` when it is not newer
/// than `current_version` (or is a draft).
pub fn check_latest(current_version: &str) -> Result<Option<ReleaseInfo>, UpdateError> {
    let url = feed_url();
    debug!(%url, "checking for updates");
    let mut response = agent(Some(Duration::from_secs(30)))
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|err| UpdateError::Network(err.to_string()))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(UpdateError::Status(status));
    }
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|err| UpdateError::Feed(err.to_string()))?;
    let release: ApiRelease =
        serde_json::from_str(&body).map_err(|err| UpdateError::Feed(err.to_string()))?;
    release_info(release, current_version)
}

fn release_info(
    release: ApiRelease,
    current_version: &str,
) -> Result<Option<ReleaseInfo>, UpdateError> {
    if release.draft {
        return Ok(None);
    }
    let version = release.tag_name.trim_start_matches('v').to_string();
    if !version_is_newer(&version, current_version) {
        debug!(latest = %version, running = %current_version, "no update available");
        return Ok(None);
    }
    let Some(zip) = pick_zip_asset(&release.assets) else {
        return Err(UpdateError::NoAsset(release.tag_name));
    };
    let signature_name = format!("{}.minisig", zip.name);
    let Some(signature) = release.assets.iter().find(|a| a.name == signature_name) else {
        return Err(UpdateError::NoSignature(release.tag_name));
    };
    Ok(Some(ReleaseInfo {
        version,
        tag: release.tag_name,
        name: release.name,
        body: release.body,
        published_at: release.published_at,
        html_url: release.html_url,
        zip_name: zip.name.clone(),
        zip_url: zip.browser_download_url.clone(),
        zip_size: zip.size,
        signature_url: signature.browser_download_url.clone(),
    }))
}

/// The `.zip` for this machine: a universal / macOS zip, else one naming
/// this architecture; the packs manifest and `.minisig` files are skipped.
fn pick_zip_asset(assets: &[ApiAsset]) -> Option<&ApiAsset> {
    let zips: Vec<&ApiAsset> = assets
        .iter()
        .filter(|a| a.name.ends_with(".zip"))
        .filter(|a| !a.name.contains("pack") && !a.name.contains("Full"))
        .collect();
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    };
    let lower = |a: &&ApiAsset| a.name.to_ascii_lowercase();
    zips.iter()
        .find(|a| lower(a).contains("universal"))
        .or_else(|| zips.iter().find(|a| lower(a).contains(arch)))
        .or_else(|| zips.iter().find(|a| lower(a).contains("macos")))
        .or_else(|| zips.first())
        .copied()
}

/// `MAJOR.MINOR.PATCH[-pre]`; a pre-release sorts before its release.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    parts: [u64; 3],
    /// `None` > `Some(_)` in the ordering below, so `release` is a flag first.
    release: bool,
    pre: String,
}

fn parse_version(text: &str) -> Option<Version> {
    let text = text.trim().trim_start_matches('v');
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (core, pre.to_string()),
        None => (text, String::new()),
    };
    let core = core.split_once('+').map(|(c, _)| c).unwrap_or(core);
    let mut parts = [0u64; 3];
    for (ix, piece) in core.split('.').enumerate() {
        if ix >= 3 {
            return None;
        }
        parts[ix] = piece.parse().ok()?;
    }
    Some(Version {
        parts,
        release: pre.is_empty(),
        pre,
    })
}

/// Is `candidate` a newer version than `current`? Unparsable versions never are.
pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(c), Some(r)) => c > r,
        _ => false,
    }
}

/// The Homebrew cask owns bundles it installed: under a Caskroom, or the
/// `/Applications/Corvane.app` a `corvane` cask has put there.
pub fn is_homebrew_install(bundle: &Path) -> bool {
    const CASKROOMS: [&str; 2] = ["/opt/homebrew/Caskroom", "/usr/local/Caskroom"];
    if CASKROOMS.iter().any(|room| bundle.starts_with(room)) {
        return true;
    }
    let in_applications = bundle.starts_with("/Applications");
    in_applications
        && CASKROOMS
            .iter()
            .any(|room| Path::new(room).join("corvane").is_dir())
}

/// `~/Library/Caches/Corvane/updates`
pub fn updates_dir() -> PathBuf {
    crate::paths::cache_dir().join("updates")
}

/// Download the release's `.zip` and `.minisig` into a fresh updates
/// directory; `progress(received, total)` is called as bytes arrive. Returns
/// the zip path.
pub fn download(
    release: &ReleaseInfo,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<PathBuf, UpdateError> {
    let dir = updates_dir();
    // older downloads and extracted bundles are never reused
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let zip = dir.join(&release.zip_name);
    let signature = dir.join(format!("{}.minisig", release.zip_name));

    let agent = agent(None);
    info!(url = %release.signature_url, "downloading update signature");
    let mut sig_response = agent
        .get(&release.signature_url)
        .call()
        .map_err(|err| UpdateError::Network(err.to_string()))?;
    let status = sig_response.status().as_u16();
    if status != 200 {
        return Err(UpdateError::Status(status));
    }
    let sig_bytes = sig_response
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_to_vec()
        .map_err(|err| UpdateError::Network(err.to_string()))?;
    std::fs::write(&signature, sig_bytes)?;

    info!(url = %release.zip_url, "downloading update");
    let mut response = agent
        .get(&release.zip_url)
        .call()
        .map_err(|err| UpdateError::Network(err.to_string()))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(UpdateError::Status(status));
    }
    let total = response
        .body_mut()
        .content_length()
        .or(Some(release.zip_size).filter(|s| *s > 0));
    let part = dir.join(format!("{}.part", release.zip_name));
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
        file.write_all(&buf[..n])?;
        received += n as u64;
        progress(received, total);
    }
    file.flush()?;
    drop(file);
    std::fs::rename(&part, &zip)?;
    Ok(zip)
}

/// Verify `zip` against `<zip>.minisig` with [`PUBLIC_KEY`].
pub fn verify(zip: &Path) -> Result<(), UpdateError> {
    let signature_path = PathBuf::from(format!("{}.minisig", zip.display()));
    verify_with_key(zip, &signature_path, PUBLIC_KEY)
}

fn verify_with_key(zip: &Path, signature_path: &Path, public_key: &str) -> Result<(), UpdateError> {
    let key = minisign_verify::PublicKey::from_base64(public_key.trim())
        .map_err(|err| UpdateError::Signature(format!("bad public key: {err}")))?;
    let signature = minisign_verify::Signature::from_file(signature_path)
        .map_err(|err| UpdateError::Signature(format!("bad signature file: {err}")))?;
    let mut verifier = key
        .verify_stream(&signature)
        .map_err(|err| UpdateError::Signature(err.to_string()))?;
    let mut file = std::fs::File::open(zip)?;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        verifier.update(&buf[..n]);
    }
    verifier
        .finalize()
        .map_err(|err| UpdateError::Signature(err.to_string()))
}

/// Verify in-memory `data` against a minisign signature file's contents
/// with [`PUBLIC_KEY`] (the packs manifest).
pub fn verify_bytes(data: &[u8], signature: &[u8]) -> Result<(), UpdateError> {
    let key = minisign_verify::PublicKey::from_base64(PUBLIC_KEY.trim())
        .map_err(|err| UpdateError::Signature(format!("bad public key: {err}")))?;
    let signature = std::str::from_utf8(signature)
        .map_err(|_| UpdateError::Signature("signature is not text".into()))?;
    let signature = minisign_verify::Signature::decode(signature)
        .map_err(|err| UpdateError::Signature(format!("bad signature: {err}")))?;
    let mut verifier = key
        .verify_stream(&signature)
        .map_err(|err| UpdateError::Signature(err.to_string()))?;
    verifier.update(data);
    verifier
        .finalize()
        .map_err(|err| UpdateError::Signature(err.to_string()))
}

/// `<bundle>.old` next to the bundle (`Corvane.app.old`).
pub fn old_bundle_path(bundle: &Path) -> PathBuf {
    let mut name = bundle
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".old");
    bundle.with_file_name(name)
}

/// Unpack the verified `zip` and swap it in for `running_bundle`. The old
/// bundle is left as `<bundle>.old` for [`remove_old_bundle`]; the caller
/// relaunches (`app_location::relaunch_after_exit`) and quits.
pub fn install(zip: &Path, running_bundle: &Path) -> Result<(), UpdateError> {
    let extracted = updates_dir().join("extracted");
    let _ = std::fs::remove_dir_all(&extracted);
    std::fs::create_dir_all(&extracted)?;
    let status = std::process::Command::new("/usr/bin/ditto")
        .args(["-x", "-k", "--sequesterRsrc"])
        .arg(zip)
        .arg(&extracted)
        .status()
        .map_err(|err| UpdateError::Install(format!("could not run ditto: {err}")))?;
    if !status.success() {
        return Err(UpdateError::Install(format!(
            "unpacking {} failed ({status})",
            zip.display()
        )));
    }
    let new_bundle = find_app_bundle(&extracted).ok_or_else(|| {
        UpdateError::Install(format!("{} contains no .app bundle", zip.display()))
    })?;
    if !new_bundle.join("Contents/MacOS/corvane").is_file() {
        return Err(UpdateError::Install(format!(
            "{} is not a Corvane bundle",
            new_bundle.display()
        )));
    }
    swap_bundles(&new_bundle, running_bundle)
}

/// The first `.app` at the top of `dir` or one level down (zips made with
/// `ditto --keepParent` wrap the bundle in a folder).
fn find_app_bundle(dir: &Path) -> Option<PathBuf> {
    let is_app = |p: &Path| p.extension().is_some_and(|e| e == "app") && p.is_dir();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    if let Some(app) = entries.iter().find(|p| is_app(p)) {
        return Some(app.clone());
    }
    for entry in entries.iter().filter(|p| p.is_dir()) {
        let mut inner: Vec<PathBuf> = std::fs::read_dir(entry)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        inner.sort();
        if let Some(app) = inner.into_iter().find(|p| is_app(p)) {
            return Some(app);
        }
    }
    None
}

/// `running` → `running.old`, `new_bundle` → `running`; the first move is
/// undone when the second fails.
fn swap_bundles(new_bundle: &Path, running: &Path) -> Result<(), UpdateError> {
    let old = old_bundle_path(running);
    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    std::fs::rename(running, &old).map_err(|err| {
        UpdateError::Install(format!("could not move {} aside: {err}", running.display()))
    })?;
    if let Err(err) = move_dir(new_bundle, running) {
        let restored = std::fs::rename(&old, running);
        return Err(UpdateError::Install(format!(
            "could not move the new bundle into {}: {err}{}",
            running.display(),
            if restored.is_ok() {
                ""
            } else {
                " (and the old bundle could not be restored)"
            }
        )));
    }
    info!(bundle = %running.display(), "update installed");
    Ok(())
}

/// Rename, or `ditto` + remove when the source is on another volume.
fn move_dir(from: &Path, to: &Path) -> Result<(), String> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) if err.raw_os_error() == Some(18) => {
            let status = std::process::Command::new("/usr/bin/ditto")
                .arg(from)
                .arg(to)
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!("copying {} failed", from.display()));
            }
            let _ = std::fs::remove_dir_all(from);
            Ok(())
        }
        Err(err) => Err(err.to_string()),
    }
}

/// Delete the `<bundle>.old` a previous update left behind. `Ok(true)` when
/// there was one.
pub fn remove_old_bundle(bundle: &Path) -> std::io::Result<bool> {
    let old = old_bundle_path(bundle);
    if !old.exists() {
        return Ok(false);
    }
    match std::fs::remove_dir_all(&old) {
        Ok(()) => {
            info!(path = %old.display(), "removed the previous bundle");
            Ok(true)
        }
        Err(err) => {
            warn!(path = %old.display(), %err, "could not remove the previous bundle");
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ordering() {
        assert!(version_is_newer("0.1.1", "0.1.0"));
        assert!(version_is_newer("v0.2.0", "0.1.9"));
        assert!(version_is_newer("1.0.0", "0.99.99"));
        assert!(!version_is_newer("0.1.0", "0.1.0"));
        assert!(!version_is_newer("0.0.9", "0.1.0"));
        // pre-releases sort before their release
        assert!(version_is_newer("0.2.0", "0.2.0-beta.1"));
        assert!(!version_is_newer("0.2.0-beta.1", "0.2.0"));
        assert!(version_is_newer("0.2.0-beta.2", "0.2.0-beta.1"));
        assert!(version_is_newer("0.2.0-rc.1+build", "0.2.0-beta.9"));
        assert!(!version_is_newer("garbage", "0.1.0"));
        assert!(!version_is_newer("0.1.0.1", "0.1.0"));
    }

    fn asset(name: &str) -> ApiAsset {
        ApiAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.invalid/{name}"),
            size: 1,
        }
    }

    #[test]
    fn picks_the_universal_zip_and_its_signature() {
        let release = ApiRelease {
            tag_name: "v0.2.0".into(),
            name: None,
            body: Some("- [New] Things".into()),
            published_at: None,
            html_url: "https://example.invalid/r".into(),
            draft: false,
            assets: vec![
                asset("packs-manifest.zip"),
                asset("Corvane-0.2.0-macos-universal.zip.minisig"),
                asset("Corvane-0.2.0-macos-universal.zip"),
                asset("Corvane-0.2.0-macos-universal.dmg"),
            ],
        };
        let info = release_info(release, "0.1.0").unwrap().unwrap();
        assert_eq!(info.version, "0.2.0");
        assert_eq!(info.zip_name, "Corvane-0.2.0-macos-universal.zip");
        assert!(info.signature_url.ends_with(".zip.minisig"));
    }

    #[test]
    fn drafts_and_older_releases_are_ignored_and_missing_assets_are_errors() {
        let mut release = ApiRelease {
            tag_name: "v0.0.1".into(),
            name: None,
            body: None,
            published_at: None,
            html_url: String::new(),
            draft: false,
            assets: vec![asset("Corvane-0.0.1-macos-universal.zip")],
        };
        assert_eq!(release_info(release, "0.1.0").unwrap(), None);
        release = ApiRelease {
            tag_name: "v9.0.0".into(),
            name: None,
            body: None,
            published_at: None,
            html_url: String::new(),
            draft: true,
            assets: vec![],
        };
        assert_eq!(release_info(release, "0.1.0").unwrap(), None);
        release = ApiRelease {
            tag_name: "v9.0.0".into(),
            name: None,
            body: None,
            published_at: None,
            html_url: String::new(),
            draft: false,
            assets: vec![asset("Corvane-9.0.0-macos-universal.zip")],
        };
        assert!(matches!(
            release_info(release, "0.1.0"),
            Err(UpdateError::NoSignature(_))
        ));
    }

    #[test]
    fn old_bundle_sits_next_to_the_bundle() {
        assert_eq!(
            old_bundle_path(Path::new("/Applications/Corvane.app")),
            PathBuf::from("/Applications/Corvane.app.old")
        );
    }

    #[test]
    fn homebrew_detection() {
        assert!(is_homebrew_install(Path::new(
            "/opt/homebrew/Caskroom/corvane/0.1.0/Corvane.app"
        )));
        assert!(!is_homebrew_install(Path::new(
            "/Users/someone/Downloads/Corvane.app"
        )));
    }

    #[test]
    fn placeholder_key_parses_but_is_flagged() {
        assert!(minisign_verify::PublicKey::from_base64(PLACEHOLDER_PUBLIC_KEY).is_ok());
        assert!(PUBLIC_KEY != PLACEHOLDER_PUBLIC_KEY || public_key_is_placeholder());
    }

    #[test]
    fn verifies_a_signed_file_and_rejects_the_wrong_key_or_tampering() {
        // key pair + signature made with rsign2 (minisign-compatible) for "corvane\n"
        const PK: &str = "RWQWQQA4BcbC0arsHabh/pvzTzJMt/cgR143jQlKG/hdxJRgz0QvST8y";
        const SIG: &str = "untrusted comment: signature from rsign secret key\n\
RUQWQQA4BcbC0ZcEEcxolelI9m4z1OEr3spEy1ILi+R6nNin5bF/cq/b/1vQDABxCC7S2nMoeR/MznbRnNOaHoj+08ODrWq5bwg=\n\
trusted comment: file:corvane.txt hashed\n\
dtHnoc7Q3DoWMwpvaTIB3VqmmNHAKafDMTDw/fUPRD2ShSxmbDbCFrnR+eJ/MTpKtzTl7yeie0bqlbc3RflvCQ==\n";
        let dir = std::env::temp_dir().join(format!("corvane-updater-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("corvane.txt");
        std::fs::write(&file, b"corvane\n").unwrap();
        let sig = dir.join("corvane.txt.minisig");
        std::fs::write(&sig, SIG).unwrap();
        verify_with_key(&file, &sig, PK).unwrap();
        assert!(matches!(
            verify_with_key(&file, &sig, PLACEHOLDER_PUBLIC_KEY),
            Err(UpdateError::Signature(_))
        ));
        std::fs::write(&file, b"corvane!\n").unwrap();
        assert!(matches!(
            verify_with_key(&file, &sig, PK),
            Err(UpdateError::Signature(_))
        ));
        std::fs::remove_dir_all(&dir).ok();
    }
}
