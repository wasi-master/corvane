//! Where the running `.app` lives - Electron's `app.isInApplicationsFolder()`
//! and `app.moveToApplicationsFolder()` as GHD uses them
//! (`main-process/main.ts`, `is-in-application-folder` /
//! `move-to-applications-folder`).
//!
//! Like Electron, a bundle Gatekeeper translocated (launched straight from a
//! quarantined download) is resolved to its original location first
//! (`SecTranslocateCreateOriginalPathForURL`), and an existing
//! `/Applications/<name>.app` is moved to the Trash before the new one takes
//! its place. No administrator rights are requested: when `/Applications` is
//! not writable the move fails with that error.

use std::path::{Path, PathBuf};

/// The `.app` bundle the current executable runs from (`None` for a bare
/// binary such as `cargo run`).
pub fn running_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bundle = exe
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))?
        .to_path_buf();
    Some(original_path(&bundle).unwrap_or(bundle))
}

/// The folders Electron counts as "Applications".
fn applications_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join("Applications"));
    }
    dirs
}

/// `app.isInApplicationsFolder()`: the bundle sits somewhere under
/// `/Applications` or `~/Applications`.
pub fn is_in_applications_folder(bundle: &Path) -> bool {
    applications_dirs()
        .iter()
        .any(|dir| bundle.starts_with(dir))
}

/// Move `bundle` into `dest_dir`, replacing (trashing) an app of the same
/// name. Returns the new path. A move across volumes copies with `ditto`
/// (keeps signatures, extended attributes and symlinks) and removes the
/// source.
pub fn move_bundle(bundle: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    let name = bundle
        .file_name()
        .ok_or_else(|| format!("{} is not an app bundle", bundle.display()))?;
    let dest = dest_dir.join(name);
    if dest == bundle {
        return Ok(dest);
    }
    if dest.exists() {
        crate::trash::move_to_trash(&dest).map_err(|err| {
            format!(
                "Could not move the existing {} to the Trash: {err}",
                dest.display()
            )
        })?;
    }
    match std::fs::rename(bundle, &dest) {
        Ok(()) => Ok(dest),
        // EXDEV: another volume (a mounted disk image, an external drive)
        Err(err) if err.raw_os_error() == Some(18) => {
            let status = std::process::Command::new("/usr/bin/ditto")
                .arg(bundle)
                .arg(&dest)
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!("Copying {} failed", bundle.display()));
            }
            // a read-only source (disk image) stays where it is
            let _ = std::fs::remove_dir_all(bundle);
            Ok(dest)
        }
        Err(err) => Err(format!(
            "Could not move {} to {}: {err}",
            bundle.display(),
            dest_dir.display()
        )),
    }
}

/// `app.moveToApplicationsFolder()` for the running bundle.
/// `CORVANE_APPLICATIONS_DIR` replaces `/Applications` (testing convenience).
pub fn move_to_applications_folder(bundle: &Path) -> Result<PathBuf, String> {
    let dest = std::env::var_os("CORVANE_APPLICATIONS_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Applications"));
    move_bundle(bundle, &dest)
}

/// Open `bundle` once process `pid` has exited (the store lock is released
/// by then), from a detached shell so it outlives this process. `open`
/// hands its environment to the new app, so Corvane's own `CORVANE_*`
/// variables (dev hooks, a `CORVANE_UPDATE_INSTALL=1` test run) are dropped:
/// the relaunch starts as clean as a Dock launch.
pub fn relaunch_after_exit(bundle: &Path, pid: u32) -> Result<(), String> {
    let script =
        "while kill -0 \"$1\" 2>/dev/null; do sleep 0.1; done; exec /usr/bin/open -n \"$2\"";
    let mut command = std::process::Command::new("/bin/sh");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("CORVANE_") {
            command.env_remove(&key);
        }
    }
    command
        .arg("-c")
        .arg(script)
        .arg("sh")
        .arg(pid.to_string())
        .arg(bundle)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}

/// The pre-translocation path of a translocated bundle, `None` when it is
/// not translocated.
#[cfg(target_os = "macos")]
fn original_path(bundle: &Path) -> Option<PathBuf> {
    use std::ffi::{CStr, c_void};
    use std::os::raw::c_char;
    use std::os::unix::ffi::OsStrExt;

    type CFTypeRef = *const c_void;
    type CFURLRef = *const c_void;
    type CFErrorRef = *const c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFURLCreateFromFileSystemRepresentation(
            allocator: CFTypeRef,
            buffer: *const u8,
            len: isize,
            is_directory: u8,
        ) -> CFURLRef;
        fn CFURLGetFileSystemRepresentation(
            url: CFURLRef,
            resolve_against_base: u8,
            buffer: *mut u8,
            max_len: isize,
        ) -> u8;
        fn CFRelease(cf: CFTypeRef);
    }
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecTranslocateIsTranslocatedURL(
            path: CFURLRef,
            is_translocated: *mut bool,
            error: *mut CFErrorRef,
        ) -> u8;
        fn SecTranslocateCreateOriginalPathForURL(
            translocated_path: CFURLRef,
            error: *mut CFErrorRef,
        ) -> CFURLRef;
    }

    if !bundle.to_string_lossy().contains("/AppTranslocation/") {
        return None;
    }
    let bytes = bundle.as_os_str().as_bytes();
    // SAFETY: CoreFoundation/Security calls on a URL we create and release;
    // the output buffer is sized and NUL-terminated by the callee.
    unsafe {
        let url = CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            1,
        );
        if url.is_null() {
            return None;
        }
        let mut translocated = false;
        let mut error: CFErrorRef = std::ptr::null();
        let ok = SecTranslocateIsTranslocatedURL(url, &mut translocated, &mut error);
        let mut result = None;
        if ok != 0 && translocated {
            let original = SecTranslocateCreateOriginalPathForURL(url, &mut error);
            if !original.is_null() {
                let mut buf = vec![0u8; 4096];
                if CFURLGetFileSystemRepresentation(
                    original,
                    1,
                    buf.as_mut_ptr(),
                    buf.len() as isize,
                ) != 0
                {
                    let c = CStr::from_ptr(buf.as_ptr() as *const c_char);
                    result = Some(PathBuf::from(std::ffi::OsStr::from_bytes(c.to_bytes())));
                }
                CFRelease(original);
            }
        }
        CFRelease(url);
        result
    }
}

#[cfg(not(target_os = "macos"))]
fn original_path(_bundle: &Path) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applications_folders() {
        assert!(is_in_applications_folder(Path::new(
            "/Applications/Corvane.app"
        )));
        assert!(is_in_applications_folder(Path::new(
            "/Applications/Dev/Corvane.app"
        )));
        if let Some(home) = dirs::home_dir() {
            assert!(is_in_applications_folder(
                &home.join("Applications/Corvane.app")
            ));
        }
        assert!(!is_in_applications_folder(Path::new(
            "/Users/someone/Downloads/Corvane.app"
        )));
    }

    #[test]
    fn moves_a_bundle_into_a_folder() {
        let dir = std::env::temp_dir().join(format!("corvane-move-{}", std::process::id()));
        let src = dir.join("from/Test.app/Contents");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("Info.plist"), "x").unwrap();
        let dest_dir = dir.join("to");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let moved = move_bundle(&dir.join("from/Test.app"), &dest_dir).unwrap();
        assert_eq!(moved, dest_dir.join("Test.app"));
        assert!(moved.join("Contents/Info.plist").exists());
        assert!(!dir.join("from/Test.app").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn untranslocated_path_is_left_alone() {
        assert_eq!(original_path(Path::new("/Applications/Corvane.app")), None);
    }
}
