//! Local crash reports (Corvane addition, opt-in, never uploaded). GHD sends
//! uncaught errors to its crash reporter and shows `crash/crash-app.tsx`;
//! Corvane only keeps files on this machine:
//!
//! - a panic hook writes `<crashes>/<timestamp>.txt` (message, location,
//!   backtrace, version, OS) while Settings › Advanced › "Save crash reports
//!   locally" is on; `<crashes>` is `~/Library/Logs/Corvane/crashes` on
//!   macOS and `$XDG_STATE_HOME/corvane/crashes` on Linux;
//! - at the next launch the reports newer than the previous launch are
//!   found, together with the OS's own reports: macOS's `corvane*.ips` in
//!   `~/Library/Logs/DiagnosticReports`, apport's `*corvane*.crash` in
//!   `/var/crash` on Linux (only their names and dates are looked at, never
//!   their contents).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ENABLED: AtomicBool = AtomicBool::new(false);
static VERSION: OnceLock<String> = OnceLock::new();

/// `~/Library/Logs/Corvane/crashes` (Linux: `$XDG_STATE_HOME/corvane/crashes`)
pub fn crashes_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        crate::paths::logs_dir().join("crashes")
    }
    #[cfg(not(target_os = "macos"))]
    {
        if std::env::var_os("CORVANE_DATA_DIR").is_some() {
            return crate::paths::logs_dir().join("crashes");
        }
        crate::paths::state_dir().join("crashes")
    }
}

/// Where the OS keeps its own crash reports: `~/Library/Logs/DiagnosticReports`
/// on macOS, apport's `/var/crash` on Linux.
pub fn diagnostic_reports_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Logs/DiagnosticReports")
    }
    #[cfg(not(target_os = "macos"))]
    {
        PathBuf::from("/var/crash")
    }
}

/// An OS crash report of Corvane's: macOS `corvane-<date>.ips`, apport
/// `_usr_bin_corvane.<uid>.crash` / `…corvane….crash`.
fn is_os_report(name: &str) -> bool {
    (name.starts_with("corvane") && name.ends_with(".ips"))
        || (name.contains("corvane") && name.ends_with(".crash"))
}

/// Turn the panic hook's report writing on or off (the setting).
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Install the panic hook once at startup; it writes a report while
/// enabled, then hands over to the default hook.
pub fn install_panic_hook(version: &str) {
    let _ = VERSION.set(version.to_string());
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if ENABLED.load(Ordering::Relaxed) {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "(no message)".to_string());
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
            let thread = std::thread::current()
                .name()
                .unwrap_or("<unnamed>")
                .to_string();
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            let _ = write_report(
                &crashes_dir(),
                SystemTime::now(),
                &Report {
                    message: &message,
                    location: location.as_deref(),
                    thread: &thread,
                    backtrace: &backtrace,
                    version: VERSION.get().map(String::as_str).unwrap_or("unknown"),
                    os: &os_version(),
                },
            );
        }
        previous(info);
    }));
}

/// What a panic report holds.
pub struct Report<'a> {
    pub message: &'a str,
    pub location: Option<&'a str>,
    pub thread: &'a str,
    pub backtrace: &'a str,
    pub version: &'a str,
    pub os: &'a str,
}

/// Write `report` as `<dir>/<timestamp>.txt`.
pub fn write_report(dir: &Path, at: SystemTime, report: &Report<'_>) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.txt", timestamp(at)));
    let mut file = std::fs::File::create(&path)?;
    writeln!(file, "Corvane {} crashed", report.version)?;
    writeln!(file, "Date: {} UTC", timestamp(at))?;
    writeln!(file, "OS: {}", report.os)?;
    writeln!(file, "Thread: {}", report.thread)?;
    writeln!(file, "Message: {}", report.message)?;
    if let Some(location) = report.location {
        writeln!(file, "Location: {location}")?;
    }
    writeln!(file, "\nBacktrace:\n{}", report.backtrace)?;
    Ok(path)
}

/// `YYYY-MM-DD-HHMMSS` in UTC.
pub fn timestamp(at: SystemTime) -> String {
    let secs = at
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // civil-from-days (Howard Hinnant)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// `macOS 15.3 (arm64)` from `SystemVersion.plist`, without spawning.
#[cfg(target_os = "macos")]
pub fn os_version() -> String {
    let version = std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
        .ok()
        .and_then(|plist| {
            let key = plist.find("<key>ProductVersion</key>")?;
            let rest = &plist[key..];
            let start = rest.find("<string>")? + "<string>".len();
            let end = rest[start..].find("</string>")?;
            Some(rest[start..start + end].to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    format!("macOS {version} ({})", std::env::consts::ARCH)
}

/// `Ubuntu 24.04.1 LTS (x86_64)` from `/etc/os-release` (`PRETTY_NAME`).
#[cfg(not(target_os = "macos"))]
pub fn os_version() -> String {
    let name = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                let value = line.strip_prefix("PRETTY_NAME=")?;
                Some(value.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "Linux".to_string());
    format!("{name} ({})", std::env::consts::ARCH)
}

/// Reports modified after `since`: Corvane's `*.txt` in `crashes` and the
/// OS's reports of Corvane crashes in `diagnostic_reports`, newest first.
pub fn reports_since(crashes: &Path, diagnostic_reports: &Path, since: SystemTime) -> Vec<PathBuf> {
    let mut found: Vec<(SystemTime, PathBuf)> = Vec::new();
    let mut scan = |dir: &Path, wanted: &dyn Fn(&str) -> bool| {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !wanted(&name) {
                continue;
            }
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified())
                && modified > since
            {
                found.push((modified, entry.path()));
            }
        }
    };
    scan(crashes, &|name| name.ends_with(".txt"));
    scan(diagnostic_reports, &is_os_report);
    found.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    found.into_iter().map(|(_, path)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "corvane-crash-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn timestamps_are_utc_and_sortable() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01-000000");
        let t = UNIX_EPOCH + Duration::from_secs(1_790_689_728);
        assert_eq!(timestamp(t), "2026-09-29-134848");
    }

    #[test]
    fn reports_are_written_and_found_after_the_last_launch() {
        let crashes = temp_dir("own");
        let diagnostics = temp_dir("macos");
        let before = SystemTime::now() - Duration::from_secs(60);
        let path = write_report(
            &crashes,
            SystemTime::now(),
            &Report {
                message: "boom",
                location: Some("src/main.rs:1:1"),
                thread: "main",
                backtrace: "0: corvane::main",
                version: "0.1.0",
                os: "macOS 15.0 (arm64)",
            },
        )
        .expect("write");
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(text.starts_with("Corvane 0.1.0 crashed"));
        assert!(text.contains("Message: boom"));
        assert!(text.contains("Location: src/main.rs:1:1"));

        std::fs::write(diagnostics.join("corvane-2026-09-29-120000.ips"), "{}").expect("ips");
        std::fs::write(diagnostics.join("Safari-2026-09-29-120000.ips"), "{}").expect("other");
        std::fs::write(diagnostics.join("corvane.diag"), "{}").expect("other kind");

        let found = reports_since(&crashes, &diagnostics, before);
        let names: Vec<String> = found
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        assert_eq!(found.len(), 2, "{names:?}");
        assert!(names.iter().any(|n| n.ends_with(".txt")));
        assert!(names.contains(&"corvane-2026-09-29-120000.ips".to_string()));

        // nothing is newer than now
        assert!(
            reports_since(
                &crashes,
                &diagnostics,
                SystemTime::now() + Duration::from_secs(60)
            )
            .is_empty()
        );
        let _ = std::fs::remove_dir_all(&crashes);
        let _ = std::fs::remove_dir_all(&diagnostics);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn os_version_names_macos() {
        assert!(os_version().starts_with("macOS "));
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn os_version_names_the_architecture() {
        assert!(os_version().ends_with(&format!("({})", std::env::consts::ARCH)));
    }

    #[test]
    fn apport_reports_count() {
        assert!(is_os_report("_usr_bin_corvane.1000.crash"));
        assert!(!is_os_report("_usr_bin_firefox.1000.crash"));
    }
}
