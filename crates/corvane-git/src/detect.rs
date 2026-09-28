//! Locate a usable `git` binary. GitHub Desktop bundles git; Corvane uses the
//! system one and shows the InstallGit dialog when it is missing.

use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info};

use crate::error::{GitError, Result};

/// Oldest git Corvane supports (`--porcelain=v2`, `--force-with-lease`, sparse index fixes).
pub const MIN_VERSION: GitVersion = GitVersion {
    major: 2,
    minor: 40,
    patch: 0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for GitVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl GitVersion {
    /// Parse `git version 2.54.0` / `git version 2.39.5 (Apple Git-154)`.
    pub fn parse(output: &str) -> Option<Self> {
        let rest = output.trim().strip_prefix("git version ")?;
        let token = rest.split_whitespace().next()?;
        let mut parts = token.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts
            .next()
            .and_then(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .unwrap_or(0);
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitBinary {
    pub path: PathBuf,
    pub version: GitVersion,
}

/// Find git: `$CORVANE_GIT`, then `$PATH`, then well-known locations.
/// `/usr/bin/git` is only tried when the Xcode Command Line Tools are present,
/// because Apple's shim otherwise pops an install dialog.
pub fn find_git() -> Result<GitBinary> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("CORVANE_GIT") {
        candidates.push(PathBuf::from(p));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("git");
            if candidate == Path::new("/usr/bin/git") && !command_line_tools_present() {
                continue;
            }
            candidates.push(candidate);
        }
    }
    for p in ["/opt/homebrew/bin/git", "/usr/local/bin/git"] {
        candidates.push(PathBuf::from(p));
    }
    if command_line_tools_present() {
        candidates.push(PathBuf::from("/usr/bin/git"));
    }

    let mut too_old: Option<GitVersion> = None;
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        match probe(&candidate) {
            Some(version) if version >= MIN_VERSION => {
                info!(path = %candidate.display(), %version, "using git");
                return Ok(GitBinary {
                    path: candidate,
                    version,
                });
            }
            Some(version) => {
                debug!(path = %candidate.display(), %version, "git too old, skipping");
                too_old = too_old.max(Some(version));
            }
            None => debug!(path = %candidate.display(), "could not probe git"),
        }
    }
    match too_old {
        Some(found) => Err(GitError::GitTooOld {
            found: found.to_string(),
            required: MIN_VERSION.to_string(),
        }),
        None => Err(GitError::GitNotFound),
    }
}

fn command_line_tools_present() -> bool {
    Path::new("/Library/Developer/CommandLineTools/usr/bin/git").exists()
        || Path::new("/Applications/Xcode.app/Contents/Developer/usr/bin/git").exists()
}

fn probe(path: &Path) -> Option<GitVersion> {
    let output = Command::new(path)
        .arg("--version")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    GitVersion::parse(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        let v = GitVersion::parse("git version 2.54.0\n").unwrap();
        assert_eq!(
            v,
            GitVersion {
                major: 2,
                minor: 54,
                patch: 0
            }
        );
        let apple = GitVersion::parse("git version 2.39.5 (Apple Git-154)").unwrap();
        assert_eq!(apple.minor, 39);
        assert!(apple < MIN_VERSION);
        assert!(GitVersion::parse("nope").is_none());
        let rc = GitVersion::parse("git version 2.50.0-rc1").unwrap();
        assert_eq!(rc.patch, 0);
    }

    #[test]
    fn finds_system_git() {
        let git = find_git().expect("git on this machine");
        assert!(git.version >= MIN_VERSION);
    }
}
