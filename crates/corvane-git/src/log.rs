//! Commit history - GHD `lib/git/log.ts` (`getCommits`, `getChangedFiles`,
//! `getCommitDiff`). The walk is done in-process with gitoxide; changed files
//! and per-file diffs come from the git CLI so rename/copy detection matches
//! GitHub Desktop exactly.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use corvane_models::{
    ChangesetData, Commit, CommitIdentity, CommittedFileChange, Diff, FileStatus, FileStatusKind,
    GitStatusEntry,
};

use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::process::GitCommand;

/// GHD `CommitBatchSize`
pub const COMMIT_BATCH_SIZE: usize = 100;

fn identity(sig: gix::actor::SignatureRef<'_>) -> CommitIdentity {
    let time = sig.time().unwrap_or_default();
    CommitIdentity {
        name: sig.name.to_string(),
        email: sig.email.to_string(),
        seconds: time.seconds,
        offset: time.offset,
    }
}

/// Commits reachable from `revision` (a ref name or sha), newest first,
/// `skip` then at most `limit` of them.
pub fn get_commits(
    workdir: &Path,
    revision: &str,
    skip: usize,
    limit: usize,
) -> Result<Vec<Commit>> {
    let repo = gix::open(workdir)?;
    let Some(tip) = repo.rev_parse_single(revision).ok() else {
        return Ok(Vec::new());
    };
    let mut tags: HashMap<gix::ObjectId, Vec<String>> = HashMap::new();
    if let Ok(refs) = repo.references()
        && let Ok(iter) = refs.tags()
    {
        for r in iter.flatten() {
            let name = r.name().shorten().to_string();
            if let Ok(id) = r.into_fully_peeled_id() {
                tags.entry(id.detach()).or_default().push(name);
            }
        }
    }
    let walk = repo
        .rev_walk([tip.detach()])
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
        .map_err(|e| GitError::Gix(e.to_string()))?;
    let mut out = Vec::with_capacity(limit.min(COMMIT_BATCH_SIZE));
    for info in walk.skip(skip).take(limit) {
        let info = info.map_err(|e| GitError::Gix(e.to_string()))?;
        let commit = info.object().map_err(|e| GitError::Gix(e.to_string()))?;
        let decoded = commit.decode().map_err(|e| GitError::Gix(e.to_string()))?;
        let message = decoded.message();
        let summary = message.summary().to_string();
        let body = message
            .body()
            .map(|b| b.to_string().trim_end().to_string())
            .unwrap_or_default();
        out.push(Commit {
            sha: info.id.to_string(),
            summary,
            body,
            author: identity(commit.author().map_err(|e| GitError::Gix(e.to_string()))?),
            committer: identity(
                commit
                    .committer()
                    .map_err(|e| GitError::Gix(e.to_string()))?,
            ),
            parents: info.parent_ids.iter().map(|p| p.to_string()).collect(),
            tags: tags.get(&info.id).cloned().unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Commits reachable from `to` but not from `from` (`from..to`), newest
/// first, at most `limit` (GHD `getCommits(repository, revRange(from, to))`).
pub fn get_commits_in_range(
    workdir: &Path,
    from: &str,
    to: &str,
    limit: usize,
) -> Result<Vec<Commit>> {
    let repo = gix::open(workdir)?;
    let (Ok(from_id), Ok(to_id)) = (repo.rev_parse_single(from), repo.rev_parse_single(to)) else {
        return Ok(Vec::new());
    };
    let mut tags: HashMap<gix::ObjectId, Vec<String>> = HashMap::new();
    if let Ok(refs) = repo.references()
        && let Ok(iter) = refs.tags()
    {
        for r in iter.flatten() {
            let name = r.name().shorten().to_string();
            if let Ok(id) = r.into_fully_peeled_id() {
                tags.entry(id.detach()).or_default().push(name);
            }
        }
    }
    let walk = repo
        .rev_walk([to_id.detach()])
        .with_hidden([from_id.detach()])
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
        .map_err(|e| GitError::Gix(e.to_string()))?;
    let mut out = Vec::new();
    for info in walk.take(limit) {
        let info = info.map_err(|e| GitError::Gix(e.to_string()))?;
        let commit = info.object().map_err(|e| GitError::Gix(e.to_string()))?;
        let decoded = commit.decode().map_err(|e| GitError::Gix(e.to_string()))?;
        let message = decoded.message();
        let summary = message.summary().to_string();
        let body = message
            .body()
            .map(|b| b.to_string().trim_end().to_string())
            .unwrap_or_default();
        out.push(Commit {
            sha: info.id.to_string(),
            summary,
            body,
            author: identity(commit.author().map_err(|e| GitError::Gix(e.to_string()))?),
            committer: identity(
                commit
                    .committer()
                    .map_err(|e| GitError::Gix(e.to_string()))?,
            ),
            parents: info.parent_ids.iter().map(|p| p.to_string()).collect(),
            tags: tags.get(&info.id).cloned().unwrap_or_default(),
        });
    }
    Ok(out)
}

/// `getChangedFiles`: `log <sha> -C -M -m -1 --first-parent --raw --numstat -z`.
pub fn get_changed_files(git: Arc<GitBinary>, workdir: &Path, sha: &str) -> Result<ChangesetData> {
    let out = GitCommand::new(git)
        .args([
            "log",
            sha,
            "-C",
            "-M",
            "-m",
            "-1",
            "--no-show-signature",
            "--first-parent",
            "--raw",
            "--format=format:",
            "--numstat",
            "-z",
            "--",
        ])
        .current_dir(workdir)
        .run()?;
    Ok(parse_raw_log_with_numstat(&out.stdout, sha))
}

/// Parse `--raw --numstat -z` output (GHD `parseRawLogWithNumstat`).
///
/// Raw entries: `:<old mode> <new mode> <old sha> <new sha> <status>[score]` NUL
/// path [NUL path2 for renames/copies]; numstat entries: `added TAB deleted TAB`
/// path (renames: NUL old NUL new).
pub fn parse_raw_log_with_numstat(stdout: &[u8], sha: &str) -> ChangesetData {
    let text = String::from_utf8_lossy(stdout);
    let mut fields = text.split('\0').peekable();
    let mut data = ChangesetData::default();
    while let Some(field) = fields.next() {
        let field = field.trim_start_matches('\n');
        if field.is_empty() {
            continue;
        }
        if let Some(raw) = field.strip_prefix(':') {
            // ":100644 100644 5716ca5 db3c77d M" or "R100"
            let status = raw.split(' ').nth(4).unwrap_or("");
            let (letter, score) = status.split_at(1);
            let score = score.parse::<u8>().ok();
            let first = fields.next().unwrap_or("").to_string();
            let (path, old_path) = if matches!(letter, "R" | "C") {
                let second = fields.next().unwrap_or("").to_string();
                (second, Some(first))
            } else {
                (first, None)
            };
            let kind = match letter {
                "A" => FileStatusKind::New,
                "D" => FileStatusKind::Deleted,
                "R" => FileStatusKind::Renamed,
                "C" => FileStatusKind::Copied,
                "U" => FileStatusKind::Conflicted,
                _ => FileStatusKind::Modified,
            };
            let entry = match kind {
                FileStatusKind::New => GitStatusEntry::Added,
                FileStatusKind::Deleted => GitStatusEntry::Deleted,
                FileStatusKind::Renamed => GitStatusEntry::Renamed,
                FileStatusKind::Copied => GitStatusEntry::Copied,
                FileStatusKind::Conflicted => GitStatusEntry::Unmerged,
                _ => GitStatusEntry::Modified,
            };
            data.files.push(CommittedFileChange {
                path,
                old_path,
                status: FileStatus {
                    kind,
                    index: entry,
                    working_tree: GitStatusEntry::Unchanged,
                    score,
                    code: letter.to_string(),
                    submodule: raw.starts_with("160000") || raw.split(' ').nth(1) == Some("160000"),
                    conflict_markers: None,
                },
                commitish: sha.to_string(),
            });
        } else {
            // numstat: "added\tdeleted\tpath" - "-" for binary files
            let mut parts = field.splitn(3, '\t');
            let added = parts.next().and_then(|n| n.parse::<u64>().ok());
            let deleted = parts.next().and_then(|n| n.parse::<u64>().ok());
            let path = parts.next().unwrap_or("");
            if path.is_empty() {
                // rename numstat: "added\tdeleted\t" NUL old NUL new
                fields.next();
                fields.next();
            }
            data.lines_added += added.unwrap_or(0);
            data.lines_deleted += deleted.unwrap_or(0);
        }
    }
    data
}

/// The empty tree, used as the parent of a root commit (GHD `NullTreeSHA`).
pub const NULL_TREE_SHA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

fn is_bad_revision(err: &crate::error::GitError) -> bool {
    matches!(err, crate::error::GitError::Failed { stderr, .. }
        if stderr.contains("bad revision") || stderr.contains("unknown revision"))
}

/// `getCommitRangeChangedFiles`: files changed between `shas[0]^` and the
/// newest sha (`shas` oldest first). Falls back to the empty tree when the
/// oldest commit is a root commit.
pub fn get_commit_range_changed_files(
    git: Arc<GitBinary>,
    workdir: &Path,
    shas: &[String],
) -> Result<ChangesetData> {
    let (Some(oldest), Some(newest)) = (shas.first(), shas.last()) else {
        return Ok(ChangesetData::default());
    };
    let run = |base: &str| {
        GitCommand::new(git.clone())
            .args([
                "diff",
                base,
                newest,
                "-C",
                "-M",
                "-z",
                "--raw",
                "--numstat",
                "--",
            ])
            .current_dir(workdir)
            .run()
    };
    let out = match run(&format!("{oldest}^")) {
        Ok(out) => out,
        Err(err) if is_bad_revision(&err) => run(NULL_TREE_SHA)?,
        Err(err) => return Err(err),
    };
    Ok(parse_raw_log_with_numstat(&out.stdout, newest))
}

/// `getCommitRangeDiff`: one file's patch across a range of commits.
pub fn commit_range_file_diff(
    git: Arc<GitBinary>,
    workdir: &Path,
    file: &CommittedFileChange,
    oldest: &str,
    newest: &str,
) -> Result<Diff> {
    if file.status.submodule {
        return Ok(Diff::Submodule);
    }
    let run = |base: &str| {
        let mut cmd = GitCommand::new(git.clone())
            .args([
                "diff",
                base,
                newest,
                "--patch-with-raw",
                "--format=",
                "-z",
                "--no-color",
                "--",
            ])
            .current_dir(workdir)
            .arg(&file.path);
        if let Some(old) = &file.old_path {
            cmd = cmd.arg(old);
        }
        cmd.run()
    };
    let out = match run(&format!("{oldest}^")) {
        Ok(out) => out,
        Err(err) if is_bad_revision(&err) => run(NULL_TREE_SHA)?,
        Err(err) => return Err(err),
    };
    Ok(crate::diff::parse_raw_diff(&out.stdout))
}

/// `getCommitDiff`: the patch for one file of a commit.
pub fn commit_file_diff(
    git: Arc<GitBinary>,
    workdir: &Path,
    file: &CommittedFileChange,
) -> Result<Diff> {
    if file.status.submodule {
        return Ok(Diff::Submodule);
    }
    let mut cmd = GitCommand::new(git)
        .args([
            "log",
            &file.commitish,
            "-m",
            "-1",
            "--first-parent",
            "--no-show-signature",
            "--patch-with-raw",
            "-z",
            "--no-color",
            "--format=format:",
            "--",
        ])
        .current_dir(workdir)
        .arg(&file.path);
    if let Some(old) = &file.old_path {
        cmd = cmd.arg(old);
    }
    let out = cmd.run()?;
    Ok(crate::diff::parse_raw_diff(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn repo() -> (tempfile::TempDir, Arc<GitBinary>) {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(dir.path())
                    // explicit identity: another test sets GIT_AUTHOR_NAME process-wide
                    .env("GIT_AUTHOR_NAME", "Ada")
                    .env("GIT_AUTHOR_EMAIL", "ada@example.com")
                    .env("GIT_COMMITTER_NAME", "Ada")
                    .env("GIT_COMMITTER_EMAIL", "ada@example.com")
                    .env("GIT_AUTHOR_DATE", "2024-01-02T03:04:05+0000")
                    .env("GIT_COMMITTER_DATE", "2024-01-02T03:04:05+0000")
                    .status()
                    .unwrap()
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "commit.gpgsign", "false"]);
        run(&["config", "user.name", "Ada"]);
        run(&["config", "user.email", "ada@example.com"]);
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "first\n\nbody line"]);
        std::fs::write(dir.path().join("a.txt"), "one\nTWO\nthree\n").unwrap();
        std::fs::write(dir.path().join("b.txt"), "b\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "second"]);
        run(&["tag", "v1"]);
        (dir, Arc::new(crate::find_git().unwrap()))
    }

    #[test]
    fn walks_commits_newest_first_with_tags() {
        let (dir, _) = repo();
        let commits = get_commits(dir.path(), "HEAD", 0, 10).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].summary, "second");
        assert_eq!(commits[0].tags, vec!["v1".to_string()]);
        assert_eq!(commits[1].summary, "first");
        assert_eq!(commits[1].body, "body line");
        assert_eq!(commits[1].author.name, "Ada");
        assert_eq!(commits[1].author.seconds, 1704164645);
        assert_eq!(commits[0].parents, vec![commits[1].sha.clone()]);
        let page = get_commits(dir.path(), "HEAD", 1, 10).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].summary, "first");
    }

    #[test]
    fn changed_files_and_diff() {
        let (dir, git) = repo();
        let head = get_commits(dir.path(), "HEAD", 0, 1).unwrap().remove(0);
        let data = get_changed_files(git.clone(), dir.path(), &head.sha).unwrap();
        let paths: Vec<_> = data.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["a.txt", "b.txt"]);
        assert_eq!(data.files[1].status.kind, FileStatusKind::New);
        assert_eq!((data.lines_added, data.lines_deleted), (3, 1));
        let diff = commit_file_diff(git, dir.path(), &data.files[0]).unwrap();
        let Diff::Text { hunks, .. } = diff else {
            panic!("text diff expected")
        };
        assert_eq!(hunks.len(), 1);
    }

    #[test]
    fn parses_rename_raw_numstat() {
        let out = b":100644 100644 abc def R100\0old.txt\0new.txt\0\n0\t0\t\0old.txt\0new.txt\0";
        let data = parse_raw_log_with_numstat(out, "sha");
        assert_eq!(data.files.len(), 1);
        assert_eq!(data.files[0].path, "new.txt");
        assert_eq!(data.files[0].old_path.as_deref(), Some("old.txt"));
        assert_eq!(data.files[0].status.kind, FileStatusKind::Renamed);
        assert_eq!(data.files[0].status.score, Some(100));
    }
}
