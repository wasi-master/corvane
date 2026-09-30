//! `.gitignore` edits — GHD `lib/git/gitignore.ts` (`appendIgnoreRule`,
//! `appendIgnoreFile`, `escapeGitSpecialCharacters`) — and [`IgnoreMatcher`],
//! gitoxide's exclude stack for the filesystem watcher (a Corvane addition).

use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;

use crate::error::{GitError, Result};

/// Escape the characters git treats specially in a pattern: `[ ] ! * # ?`.
pub fn escape_gitignore_pattern(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if matches!(c, '[' | ']' | '!' | '*' | '#' | '?') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Append raw patterns to the root `.gitignore`, creating it if needed. Keeps
/// the file's existing line endings (GHD consults `core.autocrlf`).
pub fn append_ignore_rules(workdir: &Path, patterns: &[String]) -> Result<()> {
    let path = workdir.join(".gitignore");
    let mut text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if !text.is_empty() && !text.ends_with('\n') {
        text.push_str(eol);
    }
    for pattern in patterns {
        text.push_str(pattern);
        text.push_str(eol);
    }
    std::fs::write(&path, text)?;
    Ok(())
}

/// The root `.gitignore` text, `None` when the file does not exist
/// (GHD `readGitIgnoreAtRoot`).
pub fn read_gitignore(workdir: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(workdir.join(".gitignore")) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// GHD `saveGitIgnore`: empty text deletes the file; otherwise the text is
/// written with a trailing newline, using CRLF when `core.autocrlf` is on.
pub fn save_gitignore(workdir: &Path, text: &str, autocrlf: bool) -> Result<()> {
    let path = workdir.join(".gitignore");
    if text.is_empty() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        };
    }
    let eol = if autocrlf { "\r\n" } else { "\n" };
    let mut out = String::with_capacity(text.len() + 2);
    for line in text.replace("\r\n", "\n").split('\n') {
        out.push_str(line);
        out.push_str(eol);
    }
    // `split` yields a trailing empty piece when the text ends with a newline.
    while out.ends_with(&format!("{eol}{eol}")) {
        out.truncate(out.len() - eol.len());
    }
    std::fs::write(&path, out)?;
    Ok(())
}

/// Ignore file paths (escaped first), as the "Ignore File" menu items do.
pub fn append_ignore_files(workdir: &Path, paths: &[String]) -> Result<()> {
    let patterns: Vec<String> = paths.iter().map(|p| escape_gitignore_pattern(p)).collect();
    append_ignore_rules(workdir, &patterns)
}

/// Answers "would git ignore this worktree path?" the way `git status` does:
/// the `.gitignore` files on the way to the path (read lazily as paths are
/// asked about), `$GIT_DIR/info/exclude` and `core.excludesFile` (or the XDG
/// default), with an ignored directory ignoring everything below it. Paths
/// the index tracks never count as ignored, since git still reports their
/// changes. The index and the global files are read once: build a new matcher
/// after a `.gitignore`, `info/exclude`, the config or the index changes.
pub struct IgnoreMatcher {
    repo: gix::Repository,
    index: gix::worktree::Index,
    stack: gix::worktree::Stack,
    workdir: PathBuf,
}

impl IgnoreMatcher {
    /// Build the matcher for the repository whose worktree is `workdir`.
    pub fn open(workdir: &Path) -> Result<Self> {
        let repo = gix::open(workdir)?;
        let workdir = repo
            .workdir()
            .map(Path::to_path_buf)
            .ok_or_else(|| GitError::NotARepository(workdir.to_path_buf()))?;
        let index = repo
            .index_or_empty()
            .map_err(|err| GitError::Gix(err.to_string()))?;
        let stack = repo
            .excludes(
                &index,
                None,
                gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
            )
            .map_err(|err| GitError::Gix(err.to_string()))?
            .detach();
        Ok(Self {
            repo,
            index,
            stack,
            workdir,
        })
    }

    /// Whether git ignores `rel`, a path relative to the worktree root. A path
    /// that no longer exists is matched as a file. Errors reading a
    /// `.gitignore` count as "not ignored".
    pub fn is_ignored(&mut self, rel: &Path) -> bool {
        if rel.as_os_str().is_empty() || self.is_tracked(rel) {
            return false;
        }
        // Checked as a file first: an ignored ancestor directory, the common
        // case (`target/…`), answers without a stat.
        if self.excluded(rel, None) {
            return true;
        }
        let is_dir = std::fs::symlink_metadata(self.workdir.join(rel))
            .map(|m| m.is_dir())
            .unwrap_or(false);
        is_dir && self.excluded(rel, Some(gix::index::entry::Mode::DIR))
    }

    fn excluded(&mut self, rel: &Path, mode: Option<gix::index::entry::Mode>) -> bool {
        self.stack
            .at_path(rel, mode, &self.repo.objects)
            .map(|platform| platform.is_excluded())
            .unwrap_or(false)
    }

    /// The index has `rel` itself or, for a directory, something below it.
    fn is_tracked(&self, rel: &Path) -> bool {
        let path = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(rel));
        let path = path.as_ref();
        if self.index.entry_by_path(path).is_some() {
            return true;
        }
        let mut prefix = path.to_owned();
        prefix.push(b'/');
        self.index
            .prefixed_entries_range(prefix.as_bstr())
            .is_some_and(|range| !range.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_special_characters() {
        assert_eq!(
            escape_gitignore_pattern("a[1]!*#?.txt"),
            "a\\[1\\]\\!\\*\\#\\?.txt"
        );
        assert_eq!(escape_gitignore_pattern("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn appends_with_trailing_newline() {
        let dir = tempfile::tempdir().unwrap();
        append_ignore_rules(dir.path(), &["*.log".into()]).unwrap();
        std::fs::write(dir.path().join(".gitignore"), "*.log\nbuild").unwrap();
        append_ignore_files(dir.path(), &["dist".into(), "we!rd".into()]).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text, "*.log\nbuild\ndist\nwe\\!rd\n");
    }

    #[test]
    fn save_normalises_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_gitignore(dir.path()).unwrap(), None);
        save_gitignore(dir.path(), "a\nb", false).unwrap();
        assert_eq!(
            read_gitignore(dir.path()).unwrap().as_deref(),
            Some("a\nb\n")
        );
        save_gitignore(dir.path(), "a\r\nb\n", true).unwrap();
        assert_eq!(
            read_gitignore(dir.path()).unwrap().as_deref(),
            Some("a\r\nb\r\n")
        );
        save_gitignore(dir.path(), "", false).unwrap();
        assert_eq!(read_gitignore(dir.path()).unwrap(), None);
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn matcher_follows_git_ignore_rules() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        write(
            root,
            ".gitignore",
            "target/\n*.log\n/build\nlogs/*\n!logs/keep\n",
        );
        write(root, "sub/.gitignore", "gen/\n");
        write(root, ".git/info/exclude", "secret\n");
        let global = root.join(".git/global-excludes");
        std::fs::write(&global, "*.tmp\n").unwrap();
        git(
            root,
            &["config", "core.excludesFile", global.to_str().unwrap()],
        );
        write(root, "target/debug/deps/x.o", "");
        write(root, "target/keep.txt", "");
        write(root, "sub/gen/out.rs", "");
        write(root, "src/main.rs", "");
        git(root, &["add", "-f", "target/keep.txt"]);

        let mut m = IgnoreMatcher::open(root).unwrap();
        let ignored = |m: &mut IgnoreMatcher, p: &str| m.is_ignored(Path::new(p));
        // an ignored directory ignores everything below it
        assert!(ignored(&mut m, "target/debug/deps/x.o"));
        assert!(ignored(&mut m, "target/debug"));
        assert!(ignored(&mut m, "target/gone/never-existed.rs"));
        assert!(ignored(&mut m, "a.log"));
        assert!(ignored(&mut m, "src/deep/b.log"));
        assert!(ignored(&mut m, "build"));
        assert!(!ignored(&mut m, "src/build"));
        assert!(ignored(&mut m, "logs/today"));
        assert!(!ignored(&mut m, "logs/keep"));
        // nested .gitignore, info/exclude, core.excludesFile
        assert!(ignored(&mut m, "sub/gen/out.rs"));
        assert!(!ignored(&mut m, "gen/out.rs"));
        assert!(ignored(&mut m, "secret"));
        assert!(ignored(&mut m, "notes.tmp"));
        // tracked paths still count, as does a directory holding one
        assert!(!ignored(&mut m, "target/keep.txt"));
        assert!(!ignored(&mut m, "target"));
        assert!(!ignored(&mut m, "src/main.rs"));
        assert!(!ignored(&mut m, ".gitignore"));
        assert!(!ignored(&mut m, ""));
    }

    #[test]
    fn keeps_crlf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "a\r\n").unwrap();
        append_ignore_rules(dir.path(), &["b".into()]).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text, "a\r\nb\r\n");
    }
}
