//! A left-over `index.lock` (`512-remove-stale-index-lock`): git refuses to
//! touch the index while `<gitdir>/index.lock` exists, which a crashed or
//! killed git leaves behind. GitHub Desktop shows git's error only.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{GitError, Result};

/// The lock of git's `fatal: Unable to create '<path>/index.lock': File
/// exists.`
pub fn index_lock_path(stderr: &str) -> Option<PathBuf> {
    const MARKER: &str = "Unable to create '";
    stderr.lines().find_map(|line| {
        let start = line.find(MARKER)? + MARKER.len();
        let rest = &line[start..];
        let end = rest.find("': File exists")?;
        let path = &rest[..end];
        path.ends_with("index.lock").then(|| PathBuf::from(path))
    })
}

/// Whether a git process (other than a long-lived `fsmonitor--daemon`) has
/// its working directory in `dir`. `None` when that cannot be told.
fn git_running_in(dir: &Path) -> Option<bool> {
    // lsof reports resolved paths (/private/var/…)
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    // `lsof -c git` also lists git-remote-https, git-lfs, …
    let out = Command::new("/usr/sbin/lsof")
        .args(["-a", "-c", "git", "-d", "cwd", "-Fpn"])
        .output()
        .ok()?;
    // exit 1 with no output: no such process
    let text = String::from_utf8_lossy(&out.stdout);
    let mut pid = None;
    let mut pids = Vec::new();
    for line in text.lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = Some(p.to_string());
        } else if let Some(cwd) = line.strip_prefix('n')
            && Path::new(cwd).starts_with(&dir)
            && let Some(pid) = pid.clone()
        {
            pids.push(pid);
        }
    }
    if !out.status.success() && !text.is_empty() {
        return None;
    }
    for pid in pids {
        let args = Command::new("/bin/ps")
            .args(["-o", "args=", "-p", &pid])
            .output()
            .ok()?;
        let args = String::from_utf8_lossy(&args.stdout);
        if !args.trim().is_empty() && !args.contains("fsmonitor--daemon") {
            return Some(true);
        }
    }
    Some(false)
}

/// Remove `lock` unless a git process is running in `workdir` (or in the
/// lock's git directory). Errors say why the lock was kept.
pub fn remove_stale_index_lock(lock: &Path, workdir: &Path) -> Result<()> {
    let gitdir = lock.parent().unwrap_or(workdir);
    let running = [workdir, gitdir]
        .iter()
        .map(|dir| git_running_in(dir))
        .try_fold(false, |acc, r| r.map(|r| acc || r));
    match running {
        Some(false) => {}
        Some(true) => {
            return Err(GitError::Gix(
                "A Git process is still running in this repository. Wait for it to finish, \
                 then try again."
                    .into(),
            ));
        }
        None => {
            return Err(GitError::Gix(
                "Could not check whether a Git process is running, so the lock file was kept."
                    .into(),
            ));
        }
    }
    match std::fs::remove_file(lock) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_index_lock_path() {
        let stderr = "fatal: Unable to create '/tmp/o'brien/repo/.git/index.lock': File exists.\n\n\
                      Another git process seems to be running in this repository, …\n";
        assert_eq!(
            index_lock_path(stderr),
            Some(PathBuf::from("/tmp/o'brien/repo/.git/index.lock"))
        );
        assert_eq!(
            index_lock_path("fatal: Unable to create '/r/.git/HEAD.lock': File exists."),
            None
        );
        assert_eq!(index_lock_path("fatal: not a git repository"), None);
    }

    #[test]
    fn removes_a_stale_lock() {
        let dir = tempfile::tempdir().unwrap();
        let gitdir = dir.path().join(".git");
        std::fs::create_dir(&gitdir).unwrap();
        let lock = gitdir.join("index.lock");
        std::fs::write(&lock, b"").unwrap();
        remove_stale_index_lock(&lock, dir.path()).unwrap();
        assert!(!lock.exists());
        // already gone: fine
        remove_stale_index_lock(&lock, dir.path()).unwrap();
    }
}
