//! Locate installed applications by bundle identifier and launch them -
//! GHD's `app-path` dependency (`LSCopyApplicationURLsForBundleIdentifier`)
//! and its `spawn('open', …)` calls.
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where the app bundle with this identifier lives, per LaunchServices.
/// `None` when nothing registered on this machine claims the identifier.
#[cfg(target_os = "macos")]
pub fn app_path_for_bundle_id(bundle_id: &str) -> Option<PathBuf> {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    // Force the AppKit framework to be linked so `NSWorkspace` resolves even
    // in processes (tests) that never touch the UI.
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    let c_id = CString::new(bundle_id).ok()?;
    // SAFETY: plain Objective-C message sends on well-known AppKit classes;
    // every returned object is autoreleased and drained with the pool below.
    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let ns_id: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c_id.as_ptr()];
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let url: *mut Object = msg_send![workspace, URLForApplicationWithBundleIdentifier: ns_id];
        let result = if url.is_null() {
            None
        } else {
            let path: *mut Object = msg_send![url, path];
            let cstr: *const c_char = msg_send![path, UTF8String];
            if cstr.is_null() {
                None
            } else {
                Some(PathBuf::from(
                    CStr::from_ptr(cstr).to_string_lossy().into_owned(),
                ))
            }
        };
        let _: () = msg_send![pool, drain];
        result
    }
}

#[cfg(not(target_os = "macos"))]
pub fn app_path_for_bundle_id(_bundle_id: &str) -> Option<PathBuf> {
    None
}

/// First installed bundle out of the candidates, with the identifier that matched.
pub fn first_installed(bundle_ids: &[&str]) -> Option<(String, PathBuf)> {
    bundle_ids
        .iter()
        .find_map(|id| app_path_for_bundle_id(id).map(|p| (id.to_string(), p)))
}

/// Spawn a detached process (stdio ignored) so closing Corvane never takes
/// the launched editor or shell with it.
pub fn spawn_detached(program: impl AsRef<Path>, args: &[&str]) -> std::io::Result<()> {
    Command::new(program.as_ref())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

/// `open -a <app bundle> <target>` (Linux: `app` is the executable, run
/// with `target`).
pub fn open_with_app(app: &Path, target: &Path) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        spawn_detached(
            "/usr/bin/open",
            &["-a", &app.to_string_lossy(), &target.to_string_lossy()],
        )
    } else {
        spawn_detached(app, &[&target.to_string_lossy()])
    }
}

/// `open -b <bundle id> <target>`
#[cfg(target_os = "macos")]
pub fn open_with_bundle(bundle_id: &str, target: &Path) -> std::io::Result<()> {
    spawn_detached(
        "/usr/bin/open",
        &["-b", bundle_id, &target.to_string_lossy()],
    )
}

/// Electron's `shell.showItemInFolder` on Linux (GHD `revealInFileManager`,
/// `platform_util_linux.cc`): `org.freedesktop.FileManager1.ShowItems` with
/// the item's URI so the file manager opens its folder with it selected;
/// without a file manager service, `xdg-open` on the folder.
#[cfg(not(target_os = "macos"))]
pub fn show_item_in_folder(path: &Path) -> std::io::Result<()> {
    let shown = zbus::blocking::Connection::session().and_then(|bus| {
        bus.call_method(
            Some("org.freedesktop.FileManager1"),
            "/org/freedesktop/FileManager1",
            Some("org.freedesktop.FileManager1"),
            "ShowItems",
            &(vec![file_uri(path)], ""),
        )
        .map(drop)
    });
    match shown {
        Ok(()) => Ok(()),
        Err(err) => {
            tracing::debug!(%err, "no FileManager1 service; opening the folder");
            let dir = if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(path)
            };
            spawn_detached("xdg-open", &[&dir.to_string_lossy()])
        }
    }
}

/// `file://` URI with every byte outside RFC 3986's unreserved set and `/`
/// percent-encoded (what GLib's `g_filename_to_uri` produces).
#[cfg(not(target_os = "macos"))]
pub fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            uri.push(b as char);
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    uri
}

#[cfg(all(test, not(target_os = "macos")))]
mod linux_tests {
    #[test]
    fn file_uris_are_escaped() {
        assert_eq!(
            super::file_uri(std::path::Path::new("/home/a b/ü#.txt")),
            "file:///home/a%20b/%C3%BC%23.txt"
        );
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn finds_finder_and_misses_nonsense() {
        let finder = app_path_for_bundle_id("com.apple.finder");
        assert!(finder.is_some_and(|p| p.ends_with("Finder.app")));
        assert_eq!(app_path_for_bundle_id("com.example.does-not-exist"), None);
    }
}
