//! Filesystem watcher for the selected repository (Corvane addition; GHD only
//! refreshes on focus and after its own actions). FSEvents via `notify`,
//! debounced 300 ms on a helper thread, delivered as a coalesced "refresh"
//! signal over an async channel the foreground awaits.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{debug, warn};

/// The default debounce (`203-fs-watcher-debounce-ms` sets the real one).
pub const DEBOUNCE: Duration = Duration::from_millis(300);

pub struct RepoWatcher {
    _watcher: RecommendedWatcher,
}

/// Start watching `workdir`, coalescing bursts closer than `debounce`.
/// Dropping the returned watcher stops everything.
pub fn watch(
    workdir: PathBuf,
    debounce: Duration,
) -> anyhow::Result<(RepoWatcher, async_channel::Receiver<()>)> {
    let (raw_tx, raw_rx) = mpsc::channel::<Vec<PathBuf>>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let _ = raw_tx.send(event.paths);
        }
    })?;
    watcher.watch(&workdir, RecursiveMode::Recursive)?;

    let (tx, rx) = async_channel::bounded::<()>(1);
    let root = workdir.clone();
    std::thread::Builder::new()
        .name("repo-watcher".into())
        .spawn(move || {
            // Each iteration: wait for one relevant event, then absorb the burst.
            while let Ok(paths) = raw_rx.recv() {
                let mut relevant = paths.iter().any(|p| is_relevant(&root, p));
                loop {
                    match raw_rx.recv_timeout(debounce) {
                        Ok(more) => relevant |= more.iter().any(|p| is_relevant(&root, p)),
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                if relevant {
                    debug!(path = %root.display(), "filesystem change, refresh requested");
                    // try_send: a pending signal already covers this burst
                    let _ = tx.try_send(());
                }
            }
        })
        .map_err(|e| {
            warn!(?e, "could not spawn watcher thread");
            e
        })?;

    Ok((RepoWatcher { _watcher: watcher }, rx))
}

/// Worktree files always count. Inside `.git/` only the refs/index/HEAD family
/// matters; object writes and lock files are noise.
pub fn is_relevant(root: &Path, path: &Path) -> bool {
    let rel = match path.strip_prefix(root) {
        Ok(rel) => rel,
        Err(_) => return true,
    };
    let mut comps = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    let first = comps.next().unwrap_or_default();
    if first != ".git" {
        return true;
    }
    let rest: Vec<String> = comps.collect();
    let Some(second) = rest.first() else {
        return false;
    };
    let name = rest.last().cloned().unwrap_or_default();
    if name.ends_with(".lock") {
        return false;
    }
    match second.as_str() {
        "objects" | "modules" | "lfs" | "hooks" | "info" | "worktrees" => false,
        "HEAD" | "index" | "packed-refs" | "MERGE_HEAD" | "REBASE_HEAD" | "CHERRY_PICK_HEAD"
        | "ORIG_HEAD" | "FETCH_HEAD" | "config" => true,
        "refs" | "logs" | "rebase-merge" | "rebase-apply" => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevance_rules() {
        let root = Path::new("/r");
        assert!(is_relevant(root, Path::new("/r/src/main.rs")));
        assert!(is_relevant(root, Path::new("/r/.git/HEAD")));
        assert!(is_relevant(root, Path::new("/r/.git/refs/heads/main")));
        assert!(is_relevant(root, Path::new("/r/.git/index")));
        assert!(!is_relevant(root, Path::new("/r/.git/index.lock")));
        assert!(!is_relevant(root, Path::new("/r/.git/objects/ab/cdef")));
        assert!(!is_relevant(root, Path::new("/r/.git/hooks/pre-commit")));
        assert!(!is_relevant(root, Path::new("/r/.git")));
    }

    #[test]
    fn signals_on_worktree_change() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let (_watcher, rx) = watch(dir.path().to_path_buf(), DEBOUNCE).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        let got = smol::block_on(async {
            smol::future::or(async { rx.recv().await.is_ok() }, async {
                smol::Timer::after(Duration::from_secs(5)).await;
                false
            })
            .await
        });
        assert!(got, "expected a refresh signal");
    }
}
