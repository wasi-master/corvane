//! Install Command Line Tool… - GHD `ui/lib/install-cli.ts`: symlink the
//! packaged shell script (`Contents/Resources/corvane`, from
//! `packaging/corvane.sh`) to `/usr/local/bin/corvane`, retrying with
//! administrator rights (`osascript … with administrator privileges`, GHD
//! uses `fs-admin`) when the plain attempt fails.

use std::path::{Path, PathBuf};

/// GHD `InstalledCLIPath` (`/usr/local/bin/github` there).
pub const INSTALLED_CLI_PATH: &str = "/usr/local/bin/corvane";

/// Where the symlink goes: [`INSTALLED_CLI_PATH`], or
/// `CORVANE_CLI_INSTALL_PATH` (dev/testing convenience).
pub fn install_path() -> PathBuf {
    std::env::var_os("CORVANE_CLI_INSTALL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(INSTALLED_CLI_PATH))
}

/// GHD `PackagedPath`: the script inside the running bundle.
pub fn packaged_path() -> Option<PathBuf> {
    let bundle = crate::app_location::running_bundle()?;
    let script = bundle.join("Contents/Resources/corvane");
    script.is_file().then_some(script)
}

/// `installCLI`: nothing to do when the link already points at `packaged`.
pub fn install(packaged: &Path, installed: &Path) -> Result<(), String> {
    if std::fs::read_link(installed).is_ok_and(|target| target == packaged) {
        return Ok(());
    }
    if symlink(packaged, installed).is_ok() {
        return Ok(());
    }
    symlink_as_admin(packaged, installed)
}

/// `symlinkCLI(false)`: remove the old file, create the folder, link.
fn symlink(packaged: &Path, installed: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(installed) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    if let Some(dir) = installed.parent() {
        std::fs::create_dir_all(dir)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(packaged, installed)
    }
    #[cfg(not(unix))]
    {
        let _ = packaged;
        Err(std::io::Error::other("not supported on this platform"))
    }
}

/// `symlinkCLI(true)`: the same through an authorization prompt.
fn symlink_as_admin(packaged: &Path, installed: &Path) -> Result<(), String> {
    let quote = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', r"'\''"));
    let dir = installed.parent().unwrap_or(Path::new("/"));
    let command = format!(
        "rm -f {installed} && mkdir -p {dir} && ln -s {packaged} {installed}",
        installed = quote(installed),
        dir = quote(dir),
        packaged = quote(packaged),
    );
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        command.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let output = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to symlink {} to {}. {}",
            packaged.display(),
            installed.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_replaces_and_keeps_the_symlink() {
        let dir = std::env::temp_dir().join(format!("corvane-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let packaged = dir.join("corvane.sh");
        std::fs::write(&packaged, "#!/bin/sh\n").expect("script");
        let installed = dir.join("bin/corvane");

        install(&packaged, &installed).expect("fresh install");
        assert_eq!(std::fs::read_link(&installed).expect("link"), packaged);
        // already installed: untouched
        install(&packaged, &installed).expect("reinstall");
        // an older link or file is replaced
        std::fs::remove_file(&installed).expect("remove");
        std::fs::write(&installed, "old").expect("old file");
        install(&packaged, &installed).expect("replace");
        assert_eq!(std::fs::read_link(&installed).expect("link"), packaged);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
