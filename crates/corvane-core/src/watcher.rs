//! Filesystem watcher for the selected repository (Corvane addition; GHD only
//! refreshes on focus and after its own actions). FSEvents via `notify`,
//! debounced 300 ms on a helper thread, delivered as a coalesced "refresh"
//! signal (with the time of the burst's last event) over an async channel the
//! foreground awaits.
//!
//! Worktree paths git ignores (`target/`, `node_modules/`, …) do not count,
//! so a build writing into them does not refresh every debounce window. The
//! rules come from gitoxide's exclude stack ([`corvane_git::ignore::IgnoreMatcher`]:
//! the `.gitignore` files, `.git/info/exclude`, `core.excludesFile`), built
//! on the watcher thread and rebuilt after a `.gitignore`, `info/exclude`, the
//! config or the index changes. Inside `.git/` the name rules of
//! [`is_relevant`] apply as before.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use corvane_git::ignore::IgnoreMatcher;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{debug, warn};

/// The default debounce (`203-fs-watcher-debounce-ms` sets the real one).
pub const DEBOUNCE: Duration = Duration::from_millis(300);

pub struct RepoWatcher {
    _watcher: RecommendedWatcher,
}

/// Start watching `workdir`, coalescing bursts closer than `debounce`.
/// Each signal carries when the last event it covers arrived, so a refresh
/// that started later can be skipped. With `leading` (flag
/// `902-fs-watcher-leading-edge`) the first relevant event of a quiet period
/// is signalled at once and the rest of the burst after it settles.
/// Dropping the returned watcher stops everything.
pub fn watch(
    workdir: PathBuf,
    debounce: Duration,
    leading: bool,
) -> anyhow::Result<(RepoWatcher, async_channel::Receiver<Instant>)> {
    let (raw_tx, raw_rx) = mpsc::channel::<Vec<PathBuf>>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let _ = raw_tx.send(event.paths);
        }
    })?;
    watcher.watch(&workdir, RecursiveMode::Recursive)?;

    // unbounded: every burst's time reaches the dispatcher (a refresh that
    // started before it must not swallow it)
    let (tx, rx) = async_channel::unbounded::<Instant>();
    // FSEvents reports resolved paths (`/private/var/…` for `/var/…`)
    let root = workdir.canonicalize().unwrap_or_else(|_| workdir.clone());
    std::thread::Builder::new()
        .name("repo-watcher".into())
        .spawn(move || {
            let mut rules = Relevance::new(root.clone());
            // Each iteration: wait for one relevant event, then absorb the burst.
            while let Ok(paths) = raw_rx.recv() {
                let mut relevant = rules.any_relevant(&paths, false);
                let mut last = Instant::now();
                // leading edge: this event is signalled now; only later ones
                // make the burst's own signal necessary
                if relevant && leading {
                    debug!(path = ?paths.first(), "filesystem change, refresh requested (leading)");
                    let _ = tx.try_send(last);
                    relevant = false;
                }
                loop {
                    match raw_rx.recv_timeout(debounce) {
                        Ok(more) => {
                            if rules.any_relevant(&more, false) {
                                relevant = true;
                                last = Instant::now();
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                if relevant {
                    debug!(path = %root.display(), "filesystem change, refresh requested");
                    let _ = tx.try_send(last);
                }
            }
        })
        .map_err(|e| {
            warn!(?e, "could not spawn watcher thread");
            e
        })?;

    Ok((RepoWatcher { _watcher: watcher }, rx))
}

/// Relevance with the repository's ignore rules: [`is_relevant`] for `.git/`,
/// worktree paths unless git ignores them.
pub struct Relevance {
    root: PathBuf,
    /// `None` until first needed, after a rules file changed, or when the
    /// repository could not be opened (then every worktree path counts).
    matcher: Option<IgnoreMatcher>,
    stale: bool,
}

impl Relevance {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            matcher: None,
            stale: true,
        }
    }

    /// Whether a refresh is due after `paths` changed, given `already` from
    /// earlier events of the same burst. Every path is still looked at for
    /// rule changes, but ignore matching stops once the answer is known.
    pub fn any_relevant(&mut self, paths: &[PathBuf], already: bool) -> bool {
        let mut relevant = already;
        for path in paths {
            if self.changes_rules(path) {
                self.stale = true;
            }
            if !relevant {
                relevant = self.is_relevant(path);
            }
        }
        relevant
    }

    pub fn is_relevant(&mut self, path: &Path) -> bool {
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return true;
        };
        if rel.starts_with(".git") {
            return is_relevant(&self.root, path);
        }
        !self.is_ignored(rel)
    }

    fn is_ignored(&mut self, rel: &Path) -> bool {
        if self.stale {
            self.stale = false;
            self.matcher = IgnoreMatcher::open(&self.root)
                .map_err(|err| debug!(?err, "no ignore rules for the watcher"))
                .ok();
        }
        self.matcher
            .as_mut()
            .is_some_and(|matcher| matcher.is_ignored(rel))
    }

    /// A change to a file the ignore rules or the tracked set come from. A
    /// `.gitignore` in an ignored directory (`node_modules/pkg/.gitignore`)
    /// is not one: git never reads it.
    fn changes_rules(&mut self, path: &Path) -> bool {
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return false;
        };
        if rel.file_name().is_some_and(|name| name == ".gitignore") {
            return !rel.starts_with(".git") && (self.stale || !self.is_ignored(rel));
        }
        [
            ".git/index",
            ".git/config",
            ".git/info/exclude",
            ".git/info",
        ]
        .iter()
        .any(|p| rel == Path::new(p))
    }
}

/// Inside `.git/` only the refs/index/HEAD family matters; object writes and
/// lock files are noise. Paths outside `.git/` count here; [`Relevance`]
/// drops the ones git ignores.
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

    fn init_repo(root: &Path) {
        let git = std::sync::Arc::new(corvane_git::find_git().unwrap());
        corvane_git::init_repository(
            git,
            corvane_git::InitOptions {
                path: root.to_path_buf(),
                default_branch: None,
                description: None,
                readme: false,
                gitignore: None,
                license: None,
                git_attributes: None,
                keep_existing: false,
            },
        )
        .unwrap();
    }

    #[test]
    fn relevance_follows_ignore_rules() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        init_repo(&root);
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        let mut rules = Relevance::new(root.clone());
        assert!(!rules.is_relevant(&root.join("target/debug/deps/x.o")));
        assert!(rules.is_relevant(&root.join("src/main.rs")));
        assert!(rules.is_relevant(&root.join(".gitignore")));
        assert!(rules.is_relevant(&root.join(".git/HEAD")));
        assert!(!rules.is_relevant(&root.join(".git/objects/ab/cdef")));
        assert!(rules.is_relevant(Path::new("/elsewhere/a.txt")));
        // a burst of ignored writes and object writes stays quiet
        let burst = [
            root.join("target/debug/a.o"),
            root.join("target/debug/b.o"),
            root.join(".git/objects/aa/bb"),
        ];
        assert!(!rules.any_relevant(&burst, false));
        assert!(rules.any_relevant(&burst, true));

        // editing .gitignore counts and rebuilds the rules
        std::fs::write(root.join(".gitignore"), "node_modules/\n").unwrap();
        assert!(rules.any_relevant(&[root.join(".gitignore")], false));
        assert!(rules.is_relevant(&root.join("target/debug/deps/x.o")));
        assert!(!rules.is_relevant(&root.join("node_modules/pkg/index.js")));
        // a nested one too, even when a relevant path came first
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(root.join("web/.gitignore"), "dist\n").unwrap();
        assert!(rules.any_relevant(&[root.join("a.txt"), root.join("web/.gitignore")], false));
        assert!(!rules.is_relevant(&root.join("web/dist/app.js")));
        assert!(rules.is_relevant(&root.join("dist/app.js")));

        // .gitignore files git never reads change nothing
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("node_modules/pkg/.gitignore"), "*\n").unwrap();
        assert!(!rules.any_relevant(&[root.join("node_modules/pkg/.gitignore")], false));
        assert!(!rules.stale);

        // a new .git/info/exclude applies but does not refresh by itself
        std::fs::create_dir_all(root.join(".git/info")).unwrap();
        std::fs::write(root.join(".git/info/exclude"), "*.tmp\n").unwrap();
        assert!(!rules.any_relevant(&[root.join(".git/info/exclude")], false));
        assert!(!rules.is_relevant(&root.join("scratch.tmp")));
    }

    #[test]
    fn everything_counts_without_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        let mut rules = Relevance::new(root.clone());
        assert!(rules.is_relevant(&root.join("target/debug/x.o")));
        assert!(!rules.is_relevant(&root.join(".git/objects/ab/cdef")));
    }

    #[test]
    fn quiet_on_ignored_change() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let (_watcher, rx) =
            watch(dir.path().to_path_buf(), Duration::from_millis(100), false).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        for i in 0..20 {
            std::fs::write(dir.path().join(format!("target/{i}.o")), "x").unwrap();
        }
        let got = smol::block_on(async {
            smol::future::or(async { rx.recv().await.is_ok() }, async {
                smol::Timer::after(Duration::from_millis(1500)).await;
                false
            })
            .await
        });
        assert!(!got, "writes under an ignored directory must not refresh");
    }

    #[test]
    fn signals_on_worktree_change() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let (_watcher, rx) = watch(dir.path().to_path_buf(), DEBOUNCE, false).unwrap();
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
