//! Self-updater - the place of Electron's `autoUpdater` + Squirrel in GHD
//! (`main-process/main.ts` `autoUpdater.*`, `ui/lib/update-store.ts`),
//! where the GitHub Releases API is the feed
//! (`GET /repos/wasi-master/corvane/releases/latest`).
//!
//! macOS: the release's `.zip` is downloaded to
//! `~/Library/Caches/Corvane/updates/`, verified with its minisign signature
//! against the public key compiled into this binary, unpacked with `ditto`
//! (keeps the code signature intact), swapped in for the running bundle
//! (`Corvane.app` → `Corvane.app.old`, new bundle moved in) and opened again
//! once this process has exited. The `.old` bundle is removed at the next
//! launch. Files this app writes carry no quarantine attribute, so the
//! self-signed update launches without a Gatekeeper prompt. A bundle
//! installed by Homebrew is never swapped: `brew upgrade corvane` owns it
//! (see [`is_homebrew_install`]).
//!
//! Linux (GHD ships no Linux build; Squirrel has no Linux backend): only an
//! AppImage updates itself. `$APPIMAGE` names the running image
//! ([`running_appimage`]); the release's `Corvane-<v>-<arch>.AppImage` and
//! its `.minisig` are downloaded to
//! `$XDG_CACHE_HOME/corvane/updates/` and verified like the zip, then
//! [`install`] copies the image next to `$APPIMAGE` under a temporary name,
//! verifies that copy again, makes it executable, fsyncs it and renames it
//! over `$APPIMAGE` (atomic: a crash leaves the old or the new image, never
//! half of one). The relaunch runs the new image once this process has
//! exited (`app_location::relaunch_after_exit`). Any other install (the
//! `.deb` in `/usr/lib/corvane`, a bare binary) belongs to the package
//! manager, like a Homebrew cask ([`is_package_managed`]).
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

/// What the updater installs on this OS, for error messages.
#[cfg(target_os = "macos")]
const ASSET_KIND: &str = "macOS .zip";
#[cfg(not(target_os = "macos"))]
const ASSET_KIND: &str = "AppImage";
#[cfg(target_os = "macos")]
const ASSET_EXTENSION: &str = ".zip";
#[cfg(not(target_os = "macos"))]
const ASSET_EXTENSION: &str = ".AppImage";

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("the release feed could not be reached: {0}")]
    Network(String),
    #[error("the release feed answered with status {0}")]
    Status(u16),
    #[error("the release feed could not be read: {0}")]
    Feed(String),
    #[error("release {0} has no {ASSET_KIND} asset")]
    NoAsset(String),
    #[error("release {0} has no minisign signature (.minisig) next to its {ASSET_EXTENSION}")]
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
    /// The asset to install: the `.zip` on macOS, the AppImage on Linux
    /// (the field names predate Linux).
    pub zip_name: String,
    pub zip_url: String,
    pub zip_size: u64,
    /// `<zip_name>.minisig`
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
    release_info_for(
        release,
        current_version,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// [`release_info`] for an `os` / `arch` pair (`std::env::consts` values).
fn release_info_for(
    release: ApiRelease,
    current_version: &str,
    os: &str,
    arch: &str,
) -> Result<Option<ReleaseInfo>, UpdateError> {
    if release.draft {
        return Ok(None);
    }
    let version = release.tag_name.trim_start_matches('v').to_string();
    if !version_is_newer(&version, current_version) {
        debug!(latest = %version, running = %current_version, "no update available");
        return Ok(None);
    }
    let Some(zip) = pick_asset(&release.assets, os, arch) else {
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

/// The asset an `os` / `arch` machine installs: the macOS `.zip`
/// ([`pick_zip_asset`]) or, anywhere else, the AppImage
/// ([`pick_appimage_asset`]; the `.deb` is never picked).
fn pick_asset<'a>(assets: &'a [ApiAsset], os: &str, arch: &str) -> Option<&'a ApiAsset> {
    if os == "macos" {
        pick_zip_asset(assets, arch)
    } else {
        pick_appimage_asset(assets, arch)
    }
}

/// The `.zip` for this machine: a universal / macOS zip, else one naming
/// this architecture; the packs manifest and `.minisig` files are skipped.
fn pick_zip_asset<'a>(assets: &'a [ApiAsset], arch: &str) -> Option<&'a ApiAsset> {
    let zips: Vec<&ApiAsset> = assets
        .iter()
        .filter(|a| a.name.ends_with(".zip"))
        .filter(|a| !a.name.contains("pack") && !a.name.contains("Full"))
        .collect();
    let arch = match arch {
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

/// `Corvane-<version>-<arch>.AppImage` (`packaging/linux/package.sh`),
/// `<arch>` being `x86_64` or `aarch64` as `std::env::consts::ARCH` spells
/// them; a `Corvane-Full-…` image (should there ever be one) and `.minisig`
/// files are skipped.
fn pick_appimage_asset<'a>(assets: &'a [ApiAsset], arch: &str) -> Option<&'a ApiAsset> {
    let suffix = format!("-{arch}.AppImage");
    assets.iter().find(|a| {
        a.name.starts_with("Corvane-") && !a.name.contains("Full") && a.name.ends_with(&suffix)
    })
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

/// The file or bundle an update replaces: the running `.app` on macOS
/// (`None` for a bare binary), the AppImage on Linux (`None` for a `.deb` or
/// a bare binary).
pub fn install_target() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        crate::app_location::running_bundle()
    }
    #[cfg(not(target_os = "macos"))]
    {
        running_appimage()
    }
}

/// A package manager owns this install, so the updater only announces a
/// release: a Homebrew cask bundle on macOS ([`is_homebrew_install`]); on
/// Linux anything that is not an AppImage (the `.deb` under
/// `/usr/lib/corvane`, a binary built from source).
pub fn is_package_managed() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::app_location::running_bundle().is_some_and(|b| is_homebrew_install(&b))
    }
    #[cfg(not(target_os = "macos"))]
    {
        running_appimage().is_none()
    }
}

/// The AppImage this process runs from: `$APPIMAGE` (set by the AppImage
/// runtime) when it names a regular file this user may replace.
#[cfg(not(target_os = "macos"))]
pub fn running_appimage() -> Option<PathBuf> {
    appimage_target(std::env::var_os("APPIMAGE"))
}

/// [`running_appimage`] for a given `$APPIMAGE` value, symlinks resolved so
/// the rename replaces the image itself. "May replace" means the file is
/// writable and so is its folder (the rename needs the latter); a running
/// image can answer `ETXTBSY` for the former, which counts as writable
/// since the image is replaced, never written to.
#[cfg(not(target_os = "macos"))]
fn appimage_target(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    let path = PathBuf::from(value.filter(|v| !v.is_empty())?);
    if !path.is_absolute() {
        return None;
    }
    let path = std::fs::canonicalize(path).ok()?;
    if !std::fs::metadata(&path).ok()?.is_file() {
        return None;
    }
    let dir = path.parent()?;
    let file_ok = match writable(&path) {
        Ok(()) => true,
        Err(err) => err.raw_os_error() == Some(libc::ETXTBSY),
    };
    (file_ok && writable(dir).is_ok()).then_some(path)
}

/// `access(path, W_OK)`
#[cfg(not(target_os = "macos"))]
fn writable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: `c_path` is a valid NUL-terminated string that outlives the call.
    if unsafe { libc::access(c_path.as_ptr(), libc::W_OK) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// `~/Library/Caches/Corvane/updates` (Linux: `$XDG_CACHE_HOME/corvane/updates`)
pub fn updates_dir() -> PathBuf {
    crate::paths::cache_dir().join("updates")
}

/// Download the release's asset (`.zip` / AppImage) and `.minisig` into a
/// fresh updates directory; `progress(received, total)` is called as bytes
/// arrive. Returns the asset's path.
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

/// Install the verified download over [`install_target`]; the caller
/// relaunches (`app_location::relaunch_after_exit`) and quits. macOS: unpack
/// the zip and swap bundles ([`install_bundle`]). Linux: rename the AppImage
/// over the running one ([`install_appimage`]).
pub fn install(file: &Path, target: &Path) -> Result<(), UpdateError> {
    #[cfg(target_os = "macos")]
    {
        install_bundle(file, target)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let signature = PathBuf::from(format!("{}.minisig", file.display()));
        install_appimage(file, &signature, target, PUBLIC_KEY)
    }
}

/// Copy `new_image` next to `running` under a temporary name, verify the
/// copy against `signature` with `public_key` (what gets renamed is what
/// was verified, whatever happened to the cache since the download), make it
/// `0755`, fsync it and rename it over `running`, then fsync the folder. On
/// any failure the temporary file is removed and `running` is untouched.
#[cfg(not(target_os = "macos"))]
fn install_appimage(
    new_image: &Path,
    signature: &Path,
    running: &Path,
    public_key: &str,
) -> Result<(), UpdateError> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let dir = running
        .parent()
        .ok_or_else(|| UpdateError::Install(format!("{} has no folder", running.display())))?;
    let name = running
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Corvane.AppImage".into());
    let temp = dir.join(format!(".{name}.update-{}", std::process::id()));
    let _ = std::fs::remove_file(&temp);
    let result = (|| -> Result<(), UpdateError> {
        let mut source = std::fs::File::open(new_image)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o755)
            .open(&temp)?;
        std::io::copy(&mut source, &mut file)?;
        // `mode` above is subject to the umask
        file.set_permissions(std::fs::Permissions::from_mode(0o755))?;
        file.sync_all()?;
        drop(file);
        verify_with_key(&temp, signature, public_key)?;
        std::fs::rename(&temp, running).map_err(|err| {
            UpdateError::Install(format!("could not replace {}: {err}", running.display()))
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
        return result;
    }
    // the rename is durable once the folder is
    if let Ok(dir) = std::fs::File::open(dir) {
        let _ = dir.sync_all();
    }
    info!(image = %running.display(), "update installed");
    Ok(())
}

/// Unpack the verified `zip` and swap it in for `running_bundle`. The old
/// bundle is left as `<bundle>.old` for [`remove_old_bundle`].
#[cfg(target_os = "macos")]
fn install_bundle(zip: &Path, running_bundle: &Path) -> Result<(), UpdateError> {
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
#[cfg(target_os = "macos")]
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
#[cfg(target_os = "macos")]
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
#[cfg(target_os = "macos")]
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
        let info = release_info_for(release, "0.1.0", "macos", "aarch64")
            .unwrap()
            .unwrap();
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
        assert_eq!(
            release_info_for(release, "0.1.0", "macos", "x86_64").unwrap(),
            None
        );
        release = ApiRelease {
            tag_name: "v9.0.0".into(),
            name: None,
            body: None,
            published_at: None,
            html_url: String::new(),
            draft: true,
            assets: vec![],
        };
        assert_eq!(
            release_info_for(release, "0.1.0", "macos", "x86_64").unwrap(),
            None
        );
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
            release_info_for(release, "0.1.0", "macos", "x86_64"),
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

    fn release_with(assets: &[&str]) -> ApiRelease {
        ApiRelease {
            tag_name: "v0.2.0".into(),
            name: None,
            body: None,
            published_at: None,
            html_url: String::new(),
            draft: false,
            assets: assets.iter().map(|name| asset(name)).collect(),
        }
    }

    const LINUX_ASSETS: [&str; 10] = [
        "Corvane-0.2.0-macos-universal.zip",
        "Corvane-0.2.0-macos-universal.zip.minisig",
        "corvane_0.2.0_amd64.deb",
        "corvane_0.2.0_amd64.deb.minisig",
        "Corvane-0.2.0-x86_64.AppImage.minisig",
        "Corvane-0.2.0-x86_64.AppImage",
        "Corvane-0.2.0-aarch64.AppImage",
        "Corvane-0.2.0-aarch64.AppImage.minisig",
        "packs-manifest.json",
        "packs-manifest.json.minisig",
    ];

    #[test]
    fn linux_picks_the_appimage_for_its_architecture() {
        let info = release_info_for(release_with(&LINUX_ASSETS), "0.1.0", "linux", "x86_64")
            .unwrap()
            .unwrap();
        assert_eq!(info.zip_name, "Corvane-0.2.0-x86_64.AppImage");
        assert!(
            info.signature_url
                .ends_with("/Corvane-0.2.0-x86_64.AppImage.minisig")
        );
        let info = release_info_for(release_with(&LINUX_ASSETS), "0.1.0", "linux", "aarch64")
            .unwrap()
            .unwrap();
        assert_eq!(info.zip_name, "Corvane-0.2.0-aarch64.AppImage");
        // macOS still takes the zip from the same release
        let info = release_info_for(release_with(&LINUX_ASSETS), "0.1.0", "macos", "aarch64")
            .unwrap()
            .unwrap();
        assert_eq!(info.zip_name, "Corvane-0.2.0-macos-universal.zip");
    }

    #[test]
    fn linux_ignores_the_deb_and_other_architectures() {
        let only_deb = release_with(&[
            "corvane_0.2.0_amd64.deb",
            "corvane_0.2.0_amd64.deb.minisig",
            "Corvane-0.2.0-macos-universal.zip",
        ]);
        assert!(matches!(
            release_info_for(only_deb, "0.1.0", "linux", "x86_64"),
            Err(UpdateError::NoAsset(_))
        ));
        let other_arch = release_with(&[
            "Corvane-0.2.0-aarch64.AppImage",
            "Corvane-0.2.0-aarch64.AppImage.minisig",
        ]);
        assert!(matches!(
            release_info_for(other_arch, "0.1.0", "linux", "x86_64"),
            Err(UpdateError::NoAsset(_))
        ));
        let unsigned = release_with(&["Corvane-0.2.0-x86_64.AppImage"]);
        assert!(matches!(
            release_info_for(unsigned, "0.1.0", "linux", "x86_64"),
            Err(UpdateError::NoSignature(_))
        ));
        let full = [asset("Corvane-Full-0.2.0-x86_64.AppImage")];
        assert!(pick_appimage_asset(&full, "x86_64").is_none());
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod linux_tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// The rsign key pair and signature of `verifies_a_signed_file_…`:
    /// "corvane\n" signed.
    const PK: &str = "RWQWQQA4BcbC0arsHabh/pvzTzJMt/cgR143jQlKG/hdxJRgz0QvST8y";
    const SIG: &str = "untrusted comment: signature from rsign secret key\n\
RUQWQQA4BcbC0ZcEEcxolelI9m4z1OEr3spEy1ILi+R6nNin5bF/cq/b/1vQDABxCC7S2nMoeR/MznbRnNOaHoj+08ODrWq5bwg=\n\
trusted comment: file:corvane.txt hashed\n\
dtHnoc7Q3DoWMwpvaTIB3VqmmNHAKafDMTDw/fUPRD2ShSxmbDbCFrnR+eJ/MTpKtzTl7yeie0bqlbc3RflvCQ==\n";

    /// `<tmp>/apps/Corvane.AppImage` (old, `0644`) and a downloaded
    /// `<tmp>/updates/Corvane-9.9.9-x86_64.AppImage` with its signature.
    fn setup(new_contents: &[u8]) -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let apps = tmp.path().join("apps");
        let updates = tmp.path().join("updates");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::create_dir_all(&updates).unwrap();
        let running = apps.join("Corvane.AppImage");
        std::fs::write(&running, b"old image").unwrap();
        std::fs::set_permissions(&running, std::fs::Permissions::from_mode(0o644)).unwrap();
        let new_image = updates.join("Corvane-9.9.9-x86_64.AppImage");
        std::fs::write(&new_image, new_contents).unwrap();
        let signature = updates.join("Corvane-9.9.9-x86_64.AppImage.minisig");
        std::fs::write(&signature, SIG).unwrap();
        (tmp, running, new_image, signature)
    }

    fn leftovers(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n != "Corvane.AppImage")
            .collect()
    }

    #[test]
    fn installs_a_verified_appimage_by_rename() {
        let (_tmp, running, new_image, signature) = setup(b"corvane\n");
        install_appimage(&new_image, &signature, &running, PK).unwrap();
        assert_eq!(std::fs::read(&running).unwrap(), b"corvane\n");
        let mode = std::fs::metadata(&running).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
        assert!(leftovers(running.parent().unwrap()).is_empty());
        // the download stays where it was (the cache is cleared next time)
        assert!(new_image.exists());
    }

    #[test]
    fn a_failed_verification_leaves_the_running_image_alone() {
        let (_tmp, running, new_image, signature) = setup(b"corvane!\n");
        assert!(matches!(
            install_appimage(&new_image, &signature, &running, PK),
            Err(UpdateError::Signature(_))
        ));
        assert_eq!(std::fs::read(&running).unwrap(), b"old image");
        let mode = std::fs::metadata(&running).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o644);
        assert!(leftovers(running.parent().unwrap()).is_empty());
        // the right bytes under the wrong key fail the same way
        std::fs::write(&new_image, b"corvane\n").unwrap();
        assert!(matches!(
            install_appimage(&new_image, &signature, &running, PLACEHOLDER_PUBLIC_KEY),
            Err(UpdateError::Signature(_))
        ));
        assert_eq!(std::fs::read(&running).unwrap(), b"old image");
    }

    #[test]
    fn appimage_target_needs_a_replaceable_regular_file() {
        let (tmp, running, _, _) = setup(b"x");
        assert_eq!(
            appimage_target(Some(running.clone().into_os_string())),
            Some(std::fs::canonicalize(&running).unwrap())
        );
        // a link to the image resolves to the image
        let link = tmp.path().join("corvane");
        std::os::unix::fs::symlink(&running, &link).unwrap();
        assert_eq!(
            appimage_target(Some(link.into_os_string())),
            Some(std::fs::canonicalize(&running).unwrap())
        );
        assert_eq!(appimage_target(None), None);
        assert_eq!(appimage_target(Some("".into())), None);
        assert_eq!(appimage_target(Some("Corvane.AppImage".into())), None);
        assert_eq!(appimage_target(Some(tmp.path().join("apps").into())), None);
        assert_eq!(
            appimage_target(Some(tmp.path().join("missing").into())),
            None
        );
    }
}
