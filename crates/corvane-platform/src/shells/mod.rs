//! Shell detection and launching: GHD `lib/shells/darwin.ts` on macOS,
//! `lib/shells/linux.ts` on Linux.

use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::*;
#[cfg(not(target_os = "macos"))]
mod linux;
#[cfg(not(target_os = "macos"))]
pub use linux::*;

/// GHD `FoundShell`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundShell {
    pub shell: Shell,
    /// macOS: the bundle identifier that matched (empty on Linux).
    pub bundle_id: String,
    /// The `.app` bundle, or the executable for shells launched directly.
    pub path: PathBuf,
}
