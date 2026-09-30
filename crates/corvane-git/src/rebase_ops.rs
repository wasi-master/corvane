//! Multi-commit operations: rebase, interactive rebase (squash / reorder),
//! cherry-pick, and the conflict helpers they share - GHD
//! `lib/git/{rebase,squash,reorder,cherry-pick,merge-tree,diff-check,stage,rev-list}.ts`.
//!
//! Outcomes are classified from the repository state after git exits
//! (`REBASE_HEAD`, `CHERRY_PICK_HEAD`, `MERGE_HEAD`) rather than by matching
//! dugite's stderr regexes, which is more robust across git versions.
//!
//! Deviation: with `keep_messages` (flag `448`) rebases use
//! `commit.cleanup=scissors` so `#` message lines survive a conflict
//! (GHD `lib/git/rebase.ts` keeps git's `strip`); cherry-picks likewise
//! (flag `449`, GHD `lib/git/cherry-pick.ts`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use corvane_models::{
    Commit, CommitOneLine, FileStatusKind, GitStatusEntry, ManualConflictResolution, McoProgress,
    Mergeability, RebaseInternalState, WorkingDirectoryFileChange,
};
use tracing::warn;

use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::paths::git_dir;
use crate::process::GitCommand;

/// GHD `RebaseResult`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebaseResult {
    CompletedWithoutError,
    AlreadyUpToDate,
    /// Stopped on conflicts; `REBASE_HEAD` is set.
    ConflictsEncountered,
    /// `--continue` refused because conflicted files were not staged.
    OutstandingFilesNotStaged,
    /// The repository was not in a state where the rebase could go on.
    Aborted,
    Error(String),
}

/// GHD `CherryPickResult`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CherryPickResult {
    CompletedWithoutError,
    ConflictsEncountered,
    OutstandingFilesNotStaged,
    UnableToStart,
    Error(String),
}

/// GHD `ICherryPickSnapshot`: reconstructed from `.git/sequencer`.
#[derive(Clone, Debug, PartialEq)]
pub struct CherryPickSnapshot {
    pub progress: McoProgress,
    pub remaining: Vec<CommitOneLine>,
    pub commits: Vec<CommitOneLine>,
    /// The target branch tip before the cherry-pick (`sequencer/head`).
    pub target_branch_undo_sha: String,
    pub cherry_picked_count: usize,
}

/// GHD `GitRebaseSnapshot`
#[derive(Clone, Debug, PartialEq)]
pub struct RebaseSnapshot {
    pub commits: Vec<CommitOneLine>,
    pub progress: McoProgress,
}

// ---------------------------------------------------------------------------
// repository state files
// ---------------------------------------------------------------------------

/// A rebase is in progress. Git ≥ 2.4x leaves `REBASE_HEAD` behind after a
/// finished rebase, so the `rebase-merge` / `rebase-apply` directories are
/// the reliable signal (GHD checks `REBASE_HEAD` and then reads those files).
pub fn rebase_head_set(workdir: &Path) -> bool {
    let dir = git_dir(workdir);
    dir.join("rebase-merge").exists() || dir.join("rebase-apply").exists()
}

pub fn cherry_pick_head_found(workdir: &Path) -> bool {
    git_dir(workdir).join("CHERRY_PICK_HEAD").exists()
}

pub fn merge_head_set(workdir: &Path) -> bool {
    git_dir(workdir).join("MERGE_HEAD").exists()
}

pub fn squash_msg_set(workdir: &Path) -> bool {
    git_dir(workdir).join("SQUASH_MSG").exists()
}

fn read_trimmed(path: PathBuf) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// GHD `getRebaseInternalState`
pub fn rebase_internal_state(workdir: &Path) -> Option<RebaseInternalState> {
    if !rebase_head_set(workdir) {
        return None;
    }
    let dir = git_dir(workdir).join("rebase-merge");
    let original_branch_tip = read_trimmed(dir.join("orig-head"))?;
    let head_name = read_trimmed(dir.join("head-name"))?;
    let base_branch_tip = read_trimmed(dir.join("onto"))?;
    let target_branch = head_name
        .strip_prefix("refs/heads/")
        .unwrap_or(&head_name)
        .to_string();
    Some(RebaseInternalState {
        target_branch,
        base_branch_tip,
        original_branch_tip,
    })
}

/// GHD `formatRebaseValue`: clamp to 0..=1 with two decimals.
pub fn format_rebase_value(value: f32) -> f32 {
    (value.clamp(0., 1.) * 100.).round() / 100.
}

/// GHD `getRebaseSnapshot`: progress of a rebase started outside Corvane
/// (`.git/rebase-merge/{msgnum,end}`).
pub fn rebase_snapshot(git: Arc<GitBinary>, workdir: &Path) -> Option<RebaseSnapshot> {
    if !rebase_head_set(workdir) {
        return None;
    }
    let dir = git_dir(workdir).join("rebase-merge");
    let next: usize = read_trimmed(dir.join("msgnum"))?.parse().ok()?;
    let last: usize = read_trimmed(dir.join("end"))?.parse().ok()?;
    let original_branch_tip = read_trimmed(dir.join("orig-head"))?;
    let base_branch_tip = read_trimmed(dir.join("onto"))?;
    if next == 0 || last == 0 {
        return None;
    }
    let commits = commits_between(git, workdir, &base_branch_tip, &original_branch_tip)
        .ok()
        .flatten()?;
    if commits.is_empty() {
        return None;
    }
    let current_summary = commits
        .get(next - 1)
        .map(|c| c.summary.clone())
        .unwrap_or_default();
    Some(RebaseSnapshot {
        progress: McoProgress {
            value: format_rebase_value(next as f32 / last as f32),
            position: next,
            total: last,
            current_summary,
        },
        commits,
    })
}

// ---------------------------------------------------------------------------
// rev-list helpers
// ---------------------------------------------------------------------------

fn is_bad_revision(err: &GitError) -> bool {
    matches!(err, GitError::Failed { stderr, .. }
        if stderr.contains("bad revision") || stderr.contains("unknown revision"))
}

/// GHD `getCommitsInRange`: oldest first. `None` for a bad revision range.
pub fn commits_in_range(
    git: Arc<GitBinary>,
    workdir: &Path,
    range: &str,
) -> Result<Option<Vec<CommitOneLine>>> {
    let out = GitCommand::new(git)
        .args([
            "rev-list",
            range,
            "--reverse",
            "--oneline",
            "--no-abbrev-commit",
            "--",
        ])
        .current_dir(workdir)
        .run();
    let out = match out {
        Ok(out) => out,
        Err(err) if is_bad_revision(&err) => return Ok(None),
        Err(err) => return Err(err),
    };
    Ok(Some(parse_one_line_commits(&out.stdout_string()?)))
}

pub fn parse_one_line_commits(text: &str) -> Vec<CommitOneLine> {
    text.lines()
        .filter_map(|line| {
            let (sha, summary) = line.split_once(' ').unwrap_or((line, ""));
            (sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())).then(|| CommitOneLine {
                sha: sha.to_string(),
                summary: summary.to_string(),
            })
        })
        .collect()
}

/// GHD `getCommitsBetweenCommits`: commits reachable from `target` but not
/// from `base`, in the order a rebase would apply them.
pub fn commits_between(
    git: Arc<GitBinary>,
    workdir: &Path,
    base: &str,
    target: &str,
) -> Result<Option<Vec<CommitOneLine>>> {
    commits_in_range(git, workdir, &format!("{base}..{target}"))
}

/// GHD `doMergeCommitsExistAfterCommit`
pub fn merge_commits_exist_after(
    git: Arc<GitBinary>,
    workdir: &Path,
    commit_ref: Option<&str>,
) -> Result<bool> {
    let revision = match commit_ref {
        Some(r) => format!("{r}..HEAD"),
        None => "HEAD".to_string(),
    };
    let out = GitCommand::new(git)
        .args(["rev-list", "-1", "--merges", &revision, "--"])
        .current_dir(workdir)
        .allow_exit_code(128)
        .run()?;
    Ok(!out.stdout.is_empty())
}

/// GHD `determineMergeability`: `merge-tree --write-tree` without touching
/// the working directory.
pub fn determine_mergeability(
    git: Arc<GitBinary>,
    workdir: &Path,
    ours: &str,
    theirs: &str,
) -> Result<Mergeability> {
    let out = GitCommand::new(git)
        .args([
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            "-z",
            ours,
            theirs,
        ])
        .current_dir(workdir)
        .allow_exit_code(1)
        .run();
    match out {
        Ok(out) => {
            // "<tree-id>\0[<filename>\0]*"
            let nuls = out.stdout.iter().filter(|b| **b == 0).count();
            let conflicted = nuls.saturating_sub(1) as u32;
            Ok(if conflicted > 0 {
                Mergeability::Conflicts(conflicted)
            } else {
                Mergeability::Clean
            })
        }
        Err(GitError::Failed { stderr, .. }) if stderr.contains("unrelated histories") => {
            Ok(Mergeability::Invalid)
        }
        Err(err) => Err(err),
    }
}

// ---------------------------------------------------------------------------
// conflicts
// ---------------------------------------------------------------------------

/// GHD `getFilesWithConflictMarkers`: `diff --check` leftover markers per path.
pub fn conflict_marker_counts(git: Arc<GitBinary>, workdir: &Path) -> Result<HashMap<String, u32>> {
    let out = GitCommand::new(git)
        .args(["diff", "--check"])
        .current_dir(workdir)
        .allow_exit_code(2)
        .run()?;
    Ok(parse_conflict_markers(
        &out.stdout_string().unwrap_or_default(),
    ))
}

pub fn parse_conflict_markers(stdout: &str) -> HashMap<String, u32> {
    let mut counts = HashMap::new();
    for line in stdout.lines() {
        let Some(rest) = line.strip_suffix(" leftover conflict marker") else {
            continue;
        };
        // "<path>:<line>:"
        let Some(rest) = rest.strip_suffix(':') else {
            continue;
        };
        let Some((path, line_no)) = rest.rsplit_once(':') else {
            continue;
        };
        if line_no.parse::<u32>().is_ok() {
            *counts.entry(path.to_string()).or_insert(0) += 1;
        }
    }
    counts
}

/// GHD `getBinaryPaths`: paths whose numstat is `-\t-`.
pub fn binary_paths(git: Arc<GitBinary>, workdir: &Path, paths: &[String]) -> Result<Vec<String>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let out = GitCommand::new(git)
        .args(["diff", "--numstat", "-z", "--"])
        .args(paths)
        .current_dir(workdir)
        .run()?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text
        .split('\0')
        .filter_map(|record| {
            let mut parts = record.splitn(3, '\t');
            let added = parts.next()?;
            let deleted = parts.next()?;
            let path = parts.next()?;
            (added == "-" && deleted == "-").then(|| path.to_string())
        })
        .collect())
}

/// GHD `stageManualConflictResolution`: take one side of a conflicted file.
pub fn stage_manual_conflict_resolution(
    git: Arc<GitBinary>,
    workdir: &Path,
    file: &WorkingDirectoryFileChange,
    resolution: ManualConflictResolution,
) -> Result<()> {
    let status = &file.status;
    if status.kind != FileStatusKind::Conflicted {
        warn!(path = %file.path, "tried to manually resolve an unconflicted file");
        return Ok(());
    }
    if status.conflict_markers == Some(0) {
        // resolved in an editor after all; keep the file as is
        return Ok(());
    }
    let chosen = match resolution {
        ManualConflictResolution::Ours => status.us(),
        ManualConflictResolution::Theirs => status.them(),
    };
    let added_in_both =
        status.us() == GitStatusEntry::Added && status.them() == GitStatusEntry::Added;
    if chosen == GitStatusEntry::Unmerged || added_in_both {
        let side = match resolution {
            ManualConflictResolution::Ours => "--ours",
            ManualConflictResolution::Theirs => "--theirs",
        };
        GitCommand::new(git.clone())
            .args(["checkout", side, "--", &file.path])
            .current_dir(workdir)
            .run()?;
    }
    match chosen {
        GitStatusEntry::Deleted => {
            GitCommand::new(git)
                .args(["rm", "--", &file.path])
                .current_dir(workdir)
                .run()?;
        }
        _ => {
            GitCommand::new(git)
                .args(["add", "--", &file.path])
                .current_dir(workdir)
                .run()?;
        }
    }
    Ok(())
}

/// Stage tracked files before continuing an operation: manual resolutions
/// first, then `add` for everything else (GHD `continueRebase` /
/// `continueCherryPick` / `createMergeCommit` share this).
fn stage_for_continue(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
    resolutions: &BTreeMap<String, ManualConflictResolution>,
) -> Result<()> {
    let tracked: Vec<&WorkingDirectoryFileChange> = files
        .iter()
        .filter(|f| f.status.kind != FileStatusKind::Untracked)
        .collect();
    for (path, resolution) in resolutions {
        match tracked.iter().find(|f| &f.path == path) {
            Some(file) => {
                stage_manual_conflict_resolution(git.clone(), workdir, file, *resolution)?
            }
            None => warn!(%path, "manual resolution for a file that is not in the status"),
        }
    }
    let others: Vec<WorkingDirectoryFileChange> = tracked
        .iter()
        .filter(|f| !resolutions.contains_key(&f.path))
        .map(|f| (*f).clone())
        .collect();
    stage_all(git, workdir, &others)
}

/// `update-index --add --remove --replace` for every given file (ignores the
/// partial-selection state, unlike `commit::stage_files`).
fn stage_all(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
) -> Result<()> {
    let mut paths: Vec<u8> = Vec::new();
    for file in files {
        if let Some(old) = &file.old_path {
            paths.extend_from_slice(old.as_bytes());
            paths.push(0);
        }
        paths.extend_from_slice(file.path.as_bytes());
        paths.push(0);
    }
    if paths.is_empty() {
        return Ok(());
    }
    GitCommand::new(git)
        .args([
            "update-index",
            "--add",
            "--remove",
            "--replace",
            "-z",
            "--stdin",
        ])
        .current_dir(workdir)
        .stdin(paths)
        .run()?;
    Ok(())
}

/// GHD `createMergeCommit`: stage the conflicted files (with resolutions) and
/// commit with the prepared `MERGE_MSG` / `SQUASH_MSG`. Returns the new HEAD.
pub fn create_merge_commit(
    git: Arc<GitBinary>,
    workdir: &Path,
    conflicted: &[WorkingDirectoryFileChange],
    resolutions: &BTreeMap<String, ManualConflictResolution>,
) -> Result<String> {
    stage_for_continue(git.clone(), workdir, conflicted, resolutions)?;
    GitCommand::new(git.clone())
        .args(["commit", "--no-edit"])
        .env("GIT_EDITOR", ":")
        .current_dir(workdir)
        .run()?;
    crate::commit::head_sha(git, workdir)
}

/// GHD `_abortSquashMerge`: a `merge --squash` has no `MERGE_HEAD` to abort,
/// so go back to the tip and drop the prepared message.
pub fn abort_squash_merge(git: Arc<GitBinary>, workdir: &Path, tip: &str) -> Result<()> {
    GitCommand::new(git)
        .args(["reset", "--hard", tip])
        .current_dir(workdir)
        .run()?;
    let dir = git_dir(workdir);
    for name in ["SQUASH_MSG", "MERGE_MSG"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    Ok(())
}

fn stderr_says_unresolved(stderr: &str) -> bool {
    stderr.contains("unresolved conflict")
        || stderr.contains("You must edit all merge conflicts")
        || stderr.contains("mark them as resolved")
}

// ---------------------------------------------------------------------------
// rebase
// ---------------------------------------------------------------------------

/// The cut line git writes above the conflict hint in `scissors` cleanup mode.
const SCISSORS: &str = "------------------------ >8 ------------------------";

/// `-c commit.cleanup=scissors` when `keep`: commit messages are kept as
/// written (lines starting with `#` included) and only the conflict hint git
/// appends below a cut line is dropped. Git's default, `strip`, drops every
/// `#` line, so a message whose summary starts with `#` ends up empty after
/// a conflict ("Aborting commit due to empty commit message").
fn cleanup_config(keep: bool) -> &'static [&'static str] {
    if keep {
        &["-c", "commit.cleanup=scissors"]
    } else {
        &[]
    }
}

/// The stopped pick's message has the cut line, i.e. it was started with
/// [`cleanup_config`]; a rebase started elsewhere keeps git's default.
fn message_has_scissors(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|m| m.lines().any(|l| l.contains(SCISSORS)))
}

/// GHD `GitRebaseParser`: `Rebasing (n/m)` on stderr.
pub fn parse_rebase_progress(line: &str, commits: &[CommitOneLine]) -> Option<McoProgress> {
    let rest = line.trim().strip_prefix("Rebasing (")?;
    let rest = rest.strip_suffix(')')?;
    let (done, total) = rest.split_once('/')?;
    let done: usize = done.parse().ok()?;
    let total: usize = total.parse().ok()?;
    if total == 0 {
        return None;
    }
    Some(McoProgress {
        value: format_rebase_value(done as f32 / total as f32),
        position: done,
        total,
        current_summary: commits
            .get(done.saturating_sub(1))
            .map(|c| c.summary.clone())
            .unwrap_or_default(),
    })
}

fn classify_rebase(workdir: &Path, result: Result<crate::process::GitOutput>) -> RebaseResult {
    match result {
        Ok(out) => {
            let stdout = out.stdout_string().unwrap_or_default();
            let up_to_date = stdout.lines().any(|l| {
                l.starts_with("Current branch ") && l.trim_end().ends_with("is up to date.")
            });
            if up_to_date {
                RebaseResult::AlreadyUpToDate
            } else {
                RebaseResult::CompletedWithoutError
            }
        }
        Err(GitError::Failed { stderr, .. }) => {
            let unresolved = stderr_says_unresolved(&stderr);
            if rebase_head_set(workdir) && (!unresolved || stderr.contains("could not apply")) {
                RebaseResult::ConflictsEncountered
            } else if unresolved {
                RebaseResult::OutstandingFilesNotStaged
            } else {
                RebaseResult::Error(stderr)
            }
        }
        Err(err) => RebaseResult::Error(err.to_string()),
    }
}

/// GHD `rebase`: `git rebase <base> <target>` with progress from stderr.
/// `keep_messages`: see [`cleanup_config`].
pub fn rebase(
    git: Arc<GitBinary>,
    workdir: &Path,
    base_branch: &str,
    target_branch: &str,
    commits: &[CommitOneLine],
    keep_messages: bool,
    mut on_progress: impl FnMut(McoProgress),
) -> RebaseResult {
    let result = GitCommand::new(git)
        .args(cleanup_config(keep_messages))
        .args([
            "-c",
            "rebase.backend=merge",
            "rebase",
            base_branch,
            target_branch,
        ])
        .env("GIT_EDITOR", ":")
        .current_dir(workdir)
        .run_streaming(|line| {
            if let Some(p) = parse_rebase_progress(line, commits) {
                on_progress(p);
            }
        });
    classify_rebase(workdir, result)
}

/// GHD `abortRebase`
pub fn abort_rebase(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    GitCommand::new(git)
        .args(["rebase", "--abort"])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// GHD `continueRebase`: stage the (resolved) tracked files, then
/// `rebase --continue`, or `--skip` when nothing is left to commit.
pub fn continue_rebase(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
    resolutions: &BTreeMap<String, ManualConflictResolution>,
    commits: &[CommitOneLine],
    keep_messages: bool,
    mut on_progress: impl FnMut(McoProgress),
) -> Result<RebaseResult> {
    stage_for_continue(git.clone(), workdir, files, resolutions)?;
    if !rebase_head_set(workdir) {
        return Ok(RebaseResult::Aborted);
    }
    let keep_messages =
        keep_messages && message_has_scissors(&git_dir(workdir).join("rebase-merge/message"));
    let status = crate::status::get_status(git.clone(), workdir, None)?;
    let tracked_after = status
        .files
        .iter()
        .filter(|f| f.status.kind != FileStatusKind::Untracked)
        .count();
    let action = if tracked_after == 0 {
        warn!("no tracked changes to commit; skipping this commit");
        "--skip"
    } else {
        "--continue"
    };
    let result = GitCommand::new(git)
        .args(cleanup_config(keep_messages))
        .args(["rebase", action])
        .env("GIT_EDITOR", ":")
        .current_dir(workdir)
        .run_streaming(|line| {
            if let Some(p) = parse_rebase_progress(line, commits) {
                on_progress(p);
            }
        });
    Ok(classify_rebase(workdir, result))
}

/// GHD `rebaseInteractive`: replay `todo` with `sequence.editor=cat todo >`.
/// `last_retained_ref` is the commit before the first rewritten one, or
/// `None` to rebase from the root.
#[allow(clippy::too_many_arguments)]
pub fn rebase_interactive(
    git: Arc<GitBinary>,
    workdir: &Path,
    todo: &Path,
    last_retained_ref: Option<&str>,
    git_editor: Option<&str>,
    commits: &[CommitOneLine],
    keep_messages: bool,
    mut on_progress: impl FnMut(McoProgress),
) -> RebaseResult {
    let todo_path = todo.to_string_lossy();
    if todo_path.contains('"') {
        return RebaseResult::Error("temporary todo path contains a quote".into());
    }
    let sequence_editor = format!("sequence.editor=cat \"{todo_path}\" >");
    let base = last_retained_ref.unwrap_or("--root");
    let result = GitCommand::new(git)
        .args(cleanup_config(keep_messages))
        .args([
            "-c",
            &sequence_editor,
            "-c",
            "rebase.backend=merge",
            "rebase",
            "-i",
            base,
        ])
        .env_remove("GIT_SEQUENCE_EDITOR")
        .env("GIT_EDITOR", git_editor.unwrap_or(":"))
        .current_dir(workdir)
        .run_streaming(|line| {
            if let Some(p) = parse_rebase_progress(line, commits) {
                on_progress(p);
            }
        });
    classify_rebase(workdir, result)
}

/// Commits from `last_retained_ref` (exclusive) to HEAD, oldest first; the
/// whole branch when rebasing from the root.
fn commits_to_replay(
    git: Arc<GitBinary>,
    workdir: &Path,
    last_retained_ref: Option<&str>,
) -> Result<Vec<CommitOneLine>> {
    let range = match last_retained_ref {
        Some(r) => format!("{r}..HEAD"),
        None => "HEAD".to_string(),
    };
    commits_in_range(git, workdir, &range)?
        .ok_or_else(|| GitError::Gix(format!("could not list commits for {range}")))
}

/// GHD `squash`'s todo list. `commits` are oldest first; `to_squash` never
/// contains `squash_onto`. Returns `None` when `squash_onto` is not in the log.
pub fn squash_todo(
    commits: &[CommitOneLine],
    to_squash: &HashSet<String>,
    squash_onto: &str,
) -> Option<String> {
    let mut todo = String::new();
    let mut found_onto = false;
    let mut replay_at_squash: Vec<&CommitOneLine> = Vec::new();
    let mut replay_after: Vec<&CommitOneLine> = Vec::new();
    for commit in commits {
        if to_squash.contains(&commit.sha) {
            if found_onto {
                todo.push_str(&format!("squash {} {}\n", commit.sha, commit.summary));
            } else {
                replay_at_squash.push(commit);
            }
            continue;
        }
        if commit.sha == squash_onto {
            found_onto = true;
            replay_at_squash.push(commit);
            for (i, c) in replay_at_squash.iter().enumerate() {
                let action = if i == 0 { "pick" } else { "squash" };
                todo.push_str(&format!("{action} {} {}\n", c.sha, c.summary));
            }
            continue;
        }
        if found_onto {
            replay_after.push(commit);
            continue;
        }
        todo.push_str(&format!("pick {} {}\n", commit.sha, commit.summary));
    }
    for c in replay_after {
        todo.push_str(&format!("pick {} {}\n", c.sha, c.summary));
    }
    found_onto.then_some(todo)
}

/// GHD `reorder`'s todo list. `to_move` commits end up right before
/// `before` (or at the newest end when `before` is `None`).
pub fn reorder_todo(
    commits: &[CommitOneLine],
    to_move: &HashSet<String>,
    before: Option<&str>,
) -> Option<String> {
    let mut todo = String::new();
    let mut found_base = before.is_none() && false;
    let mut replay_before_base: Vec<&CommitOneLine> = Vec::new();
    let mut replay_after: Vec<&CommitOneLine> = Vec::new();
    for commit in commits {
        if to_move.contains(&commit.sha) {
            if found_base {
                todo.push_str(&format!("pick {} {}\n", commit.sha, commit.summary));
            } else {
                replay_before_base.push(commit);
            }
            continue;
        }
        if before == Some(commit.sha.as_str()) {
            found_base = true;
            replay_after.push(commit);
            for c in &replay_before_base {
                todo.push_str(&format!("pick {} {}\n", c.sha, c.summary));
            }
            continue;
        }
        if found_base {
            replay_after.push(commit);
            continue;
        }
        todo.push_str(&format!("pick {} {}\n", commit.sha, commit.summary));
    }
    for c in replay_after {
        todo.push_str(&format!("pick {} {}\n", c.sha, c.summary));
    }
    if before.is_none() {
        for c in &replay_before_base {
            todo.push_str(&format!("pick {} {}\n", c.sha, c.summary));
        }
    } else if !found_base {
        return None;
    }
    Some(todo)
}

fn temp_file(prefix: &str, contents: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "corvane-{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(&path, contents)?;
    Ok(path)
}

/// GHD `squash`: squash `to_squash` onto `squash_onto` with `message`
/// (summary line + body) via interactive rebase.
#[allow(clippy::too_many_arguments)]
pub fn squash(
    git: Arc<GitBinary>,
    workdir: &Path,
    to_squash: &[Commit],
    squash_onto: &Commit,
    last_retained_ref: Option<&str>,
    message: &str,
    keep_messages: bool,
    on_progress: impl FnMut(McoProgress),
) -> RebaseResult {
    if to_squash.is_empty() {
        return RebaseResult::Error("no commits to squash".into());
    }
    let shas: HashSet<String> = to_squash.iter().map(|c| c.sha.clone()).collect();
    if shas.contains(&squash_onto.sha) {
        return RebaseResult::Error("cannot squash a commit onto itself".into());
    }
    let commits = match commits_to_replay(git.clone(), workdir, last_retained_ref) {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => return RebaseResult::Error("could not find commits to replay".into()),
        Err(err) => return RebaseResult::Error(err.to_string()),
    };
    let Some(todo) = squash_todo(&commits, &shas, &squash_onto.sha) else {
        return RebaseResult::Error(
            "the commit to squash onto is not in the log; continuing would drop commits".into(),
        );
    };
    let todo_path = match temp_file("squash-todo", &todo) {
        Ok(p) => p,
        Err(err) => return RebaseResult::Error(err.to_string()),
    };
    let message_path = if message.trim().is_empty() {
        None
    } else {
        match temp_file("squash-message", message) {
            Ok(p) => Some(p),
            Err(err) => return RebaseResult::Error(err.to_string()),
        }
    };
    let editor = message_path
        .as_ref()
        .map(|p| format!("cat \"{}\" >", p.to_string_lossy()));
    let involved: Vec<CommitOneLine> = to_squash
        .iter()
        .chain(std::iter::once(squash_onto))
        .map(|c| CommitOneLine {
            sha: c.sha.clone(),
            summary: c.summary.clone(),
        })
        .collect();
    // without a message git's own (with its `#` notes) would be kept
    let keep_messages = keep_messages && editor.is_some();
    let result = rebase_interactive(
        git,
        workdir,
        &todo_path,
        last_retained_ref,
        editor.as_deref(),
        &involved,
        keep_messages,
        on_progress,
    );
    let _ = std::fs::remove_file(&todo_path);
    if let Some(p) = message_path {
        let _ = std::fs::remove_file(p);
    }
    result
}

/// GHD `reorder`: move `to_move` right before `before` (or to the tip).
pub fn reorder(
    git: Arc<GitBinary>,
    workdir: &Path,
    to_move: &[Commit],
    before: Option<&Commit>,
    last_retained_ref: Option<&str>,
    keep_messages: bool,
    on_progress: impl FnMut(McoProgress),
) -> RebaseResult {
    if to_move.is_empty() {
        return RebaseResult::Error("no commits to reorder".into());
    }
    let shas: HashSet<String> = to_move.iter().map(|c| c.sha.clone()).collect();
    let commits = match commits_to_replay(git.clone(), workdir, last_retained_ref) {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => return RebaseResult::Error("could not find commits to replay".into()),
        Err(err) => return RebaseResult::Error(err.to_string()),
    };
    let Some(todo) = reorder_todo(&commits, &shas, before.map(|c| c.sha.as_str())) else {
        return RebaseResult::Error(
            "the base commit is not in the log; continuing would drop commits".into(),
        );
    };
    let todo_path = match temp_file("reorder-todo", &todo) {
        Ok(p) => p,
        Err(err) => return RebaseResult::Error(err.to_string()),
    };
    let result = rebase_interactive(
        git,
        workdir,
        &todo_path,
        last_retained_ref,
        None,
        &commits,
        keep_messages,
        on_progress,
    );
    let _ = std::fs::remove_file(&todo_path);
    result
}

// ---------------------------------------------------------------------------
// cherry-pick
// ---------------------------------------------------------------------------

/// GHD `GitCherryPickParser`: `[branch sha] summary` on stdout per commit.
pub fn parse_cherry_pick_progress(
    line: &str,
    commits: &[CommitOneLine],
    count: &mut usize,
) -> Option<McoProgress> {
    let rest = line.strip_prefix('[')?;
    let (inside, _) = rest.split_once(']')?;
    if !inside.contains(' ') {
        return None;
    }
    *count += 1;
    let total = commits.len().max(1);
    Some(McoProgress {
        value: format_rebase_value(*count as f32 / total as f32),
        position: *count,
        total: commits.len(),
        current_summary: commits
            .get(*count - 1)
            .map(|c| c.summary.clone())
            .unwrap_or_default(),
    })
}

fn classify_cherry_pick(
    workdir: &Path,
    result: Result<crate::process::GitOutput>,
) -> CherryPickResult {
    match result {
        Ok(_) => CherryPickResult::CompletedWithoutError,
        Err(GitError::Failed { stderr, .. }) => {
            if stderr_says_unresolved(&stderr) && !stderr.contains("could not apply") {
                CherryPickResult::OutstandingFilesNotStaged
            } else if cherry_pick_head_found(workdir) {
                CherryPickResult::ConflictsEncountered
            } else {
                CherryPickResult::Error(stderr)
            }
        }
        Err(err) => CherryPickResult::Error(err.to_string()),
    }
}

/// GHD `cherryPick`: `cherry-pick <shas> --empty=keep -m 1` (oldest first).
/// `keep_messages` adds `--cleanup=scissors` (see [`cleanup_config`]; the
/// sequencer remembers it for the later picks).
pub fn cherry_pick(
    git: Arc<GitBinary>,
    workdir: &Path,
    commits: &[CommitOneLine],
    keep_messages: bool,
    mut on_progress: impl FnMut(McoProgress),
) -> CherryPickResult {
    if commits.is_empty() {
        return CherryPickResult::UnableToStart;
    }
    let mut args: Vec<String> = vec!["cherry-pick".into()];
    args.extend(commits.iter().map(|c| c.sha.clone()));
    // `--empty=keep` (git ≥ 2.45) keeps empty picks in the history like GHD;
    // older gits get the equivalent pair of flags.
    if git.version.major > 2 || (git.version.major == 2 && git.version.minor >= 45) {
        args.push("--empty=keep".into());
    } else {
        args.push("--keep-redundant-commits".into());
        args.push("--allow-empty".into());
    }
    args.push("-m".into());
    args.push("1".into());
    if keep_messages {
        args.push("--cleanup=scissors".into());
    }
    let mut count = 0;
    let result = GitCommand::new(git)
        .args(&args)
        .env("GIT_EDITOR", ":")
        .current_dir(workdir)
        .run_streaming_stdout(|line| {
            if let Some(p) = parse_cherry_pick_progress(line, commits, &mut count) {
                on_progress(p);
            }
        });
    classify_cherry_pick(workdir, result)
}

/// GHD `abortCherryPick`
pub fn abort_cherry_pick(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    GitCommand::new(git)
        .args(["cherry-pick", "--abort"])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// GHD `getCherryPickSnapshot`: reconstruct progress from `.git/sequencer`.
pub fn cherry_pick_snapshot(git: Arc<GitBinary>, workdir: &Path) -> Option<CherryPickSnapshot> {
    if !cherry_pick_head_found(workdir) {
        return None;
    }
    let dir = git_dir(workdir);
    let sequencer = dir.join("sequencer");
    let parsed = (|| {
        let abort_safety = read_trimmed(sequencer.join("abort-safety"))?;
        let head = read_trimmed(sequencer.join("head"))?;
        let todo = read_trimmed(sequencer.join("todo"))?;
        let remaining: Vec<CommitOneLine> = todo
            .lines()
            .filter_map(|line| {
                let line = line.strip_prefix("pick ").unwrap_or(line);
                let (sha, summary) = line.trim().split_once(' ')?;
                Some(CommitOneLine {
                    sha: sha.to_string(),
                    summary: summary.to_string(),
                })
            })
            .collect();
        (!remaining.is_empty()).then_some((abort_safety, head, remaining))
    })();
    match parsed {
        Some((abort_safety, head, remaining)) => {
            let picked = if abort_safety != head {
                commits_between(git, workdir, &head, &abort_safety)
                    .ok()
                    .flatten()?
            } else {
                Vec::new()
            };
            let mut commits = picked.clone();
            commits.extend(remaining.iter().cloned());
            let position = picked.len() + 1;
            Some(CherryPickSnapshot {
                progress: McoProgress {
                    value: format_rebase_value(position as f32 / commits.len().max(1) as f32),
                    position,
                    total: commits.len(),
                    current_summary: remaining[0].summary.clone(),
                },
                remaining,
                commits,
                target_branch_undo_sha: head,
                cherry_picked_count: picked.len(),
            })
        }
        None => {
            // a single-commit cherry-pick does not use the sequencer files
            let sha = read_trimmed(dir.join("CHERRY_PICK_HEAD"))?;
            let summary = GitCommand::new(git)
                .args(["log", "-1", "--format=%s", &sha, "--"])
                .current_dir(workdir)
                .run()
                .ok()
                .and_then(|o| o.stdout_string().ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let commit = CommitOneLine {
                sha,
                summary: summary.clone(),
            };
            Some(CherryPickSnapshot {
                progress: McoProgress {
                    value: 1.,
                    position: 1,
                    total: 1,
                    current_summary: summary,
                },
                remaining: Vec::new(),
                commits: vec![commit],
                target_branch_undo_sha: String::new(),
                cherry_picked_count: 0,
            })
        }
    }
}

/// GHD `continueCherryPick`. With `keep_messages` and a stopped pick started
/// that way (its `MERGE_MSG` has the cut line), the pick is committed here
/// with `--cleanup=scissors` and the sequencer, if any, continued after it:
/// `cherry-pick --continue` itself commits a single pick with `strip`,
/// dropping `#` lines, and `commit --no-edit` keeps git's conflict note.
pub fn continue_cherry_pick(
    git: Arc<GitBinary>,
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
    resolutions: &BTreeMap<String, ManualConflictResolution>,
    keep_messages: bool,
    mut on_progress: impl FnMut(McoProgress),
) -> Result<CherryPickResult> {
    stage_for_continue(git.clone(), workdir, files, resolutions)?;
    if !cherry_pick_head_found(workdir) {
        return Ok(CherryPickResult::UnableToStart);
    }
    let status = crate::status::get_status(git.clone(), workdir, None)?;
    let (commits, mut count) = match cherry_pick_snapshot(git.clone(), workdir) {
        Some(s) => (s.commits, s.cherry_picked_count),
        None => return Ok(CherryPickResult::UnableToStart),
    };
    let tracked_after = status
        .files
        .iter()
        .filter(|f| f.status.kind != FileStatusKind::Untracked)
        .count();
    let dir = git_dir(workdir);
    if keep_messages && message_has_scissors(&dir.join("MERGE_MSG")) {
        // the editor "runs" (`:`), so `scissors` cuts at the line
        let committed = GitCommand::new(git.clone())
            .args(["commit", "--allow-empty", "--cleanup=scissors"])
            .env("GIT_EDITOR", ":")
            .current_dir(workdir)
            .run();
        match committed {
            Ok(out) => {
                for line in out.stdout_string().unwrap_or_default().lines() {
                    if let Some(p) = parse_cherry_pick_progress(line, &commits, &mut count) {
                        on_progress(p);
                    }
                }
            }
            Err(GitError::Failed { stderr, .. }) => return Ok(CherryPickResult::Error(stderr)),
            Err(err) => return Err(err),
        }
        if !dir.join("sequencer").join("todo").exists() {
            return Ok(CherryPickResult::CompletedWithoutError);
        }
        let result = GitCommand::new(git)
            .args(["cherry-pick", "--continue"])
            .env("GIT_EDITOR", ":")
            .current_dir(workdir)
            .run_streaming_stdout(|line| {
                if let Some(p) = parse_cherry_pick_progress(line, &commits, &mut count) {
                    on_progress(p);
                }
            });
        return Ok(classify_cherry_pick(workdir, result));
    }
    let result = if tracked_after == 0 {
        warn!("no tracked changes to commit; continuing cherry-pick with an empty commit");
        GitCommand::new(git)
            .args(["commit", "--allow-empty", "--no-edit"])
            .env("GIT_EDITOR", ":")
            .current_dir(workdir)
            .run()
    } else {
        GitCommand::new(git)
            .args(["cherry-pick", "--continue"])
            .env("GIT_EDITOR", ":")
            .current_dir(workdir)
            .run_streaming_stdout(|line| {
                if let Some(p) = parse_cherry_pick_progress(line, &commits, &mut count) {
                    on_progress(p);
                }
            })
    };
    Ok(classify_cherry_pick(workdir, result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn c(sha: &str, summary: &str) -> CommitOneLine {
        CommitOneLine {
            sha: sha.into(),
            summary: summary.into(),
        }
    }

    #[test]
    fn squash_todo_replays_in_log_order() {
        // oldest → newest: A B C D E; squash A and E onto C → B, A-C-E, D
        let commits = [
            c("A", "a"),
            c("B", "b"),
            c("C", "c"),
            c("D", "d"),
            c("E", "e"),
        ];
        let to_squash: HashSet<String> = ["A".to_string(), "E".to_string()].into();
        let todo = squash_todo(&commits, &to_squash, "C").unwrap();
        assert_eq!(
            todo,
            "pick B b\npick A a\nsquash C c\nsquash E e\npick D d\n"
        );
        assert!(squash_todo(&commits, &to_squash, "Z").is_none());
    }

    #[test]
    fn reorder_todo_moves_before_base_or_to_tip() {
        let commits = [
            c("A", "a"),
            c("B", "b"),
            c("C", "c"),
            c("D", "d"),
            c("E", "e"),
        ];
        let to_move: HashSet<String> = ["A".to_string(), "E".to_string()].into();
        assert_eq!(
            reorder_todo(&commits, &to_move, Some("C")).unwrap(),
            "pick B b\npick A a\npick E e\npick C c\npick D d\n"
        );
        assert_eq!(
            reorder_todo(&commits, &to_move, None).unwrap(),
            "pick B b\npick C c\npick D d\npick A a\npick E e\n"
        );
        assert!(reorder_todo(&commits, &to_move, Some("Z")).is_none());
    }

    #[test]
    fn parses_progress_lines() {
        let commits = [c("A", "first"), c("B", "second")];
        let p = parse_rebase_progress("Rebasing (2/2)", &commits).unwrap();
        assert_eq!((p.position, p.total, p.value), (2, 2, 1.0));
        assert_eq!(p.current_summary, "second");
        assert!(parse_rebase_progress("Successfully rebased", &commits).is_none());
        let mut count = 0;
        let p = parse_cherry_pick_progress("[main 1234abc] first", &commits, &mut count).unwrap();
        assert_eq!((p.position, p.total), (1, 2));
        assert!(parse_cherry_pick_progress(" Date: x", &commits, &mut count).is_none());
    }

    #[test]
    fn parses_conflict_markers() {
        let out = "a.txt:3: leftover conflict marker\na.txt:5: leftover conflict marker\nb.txt:1: trailing whitespace.\n";
        let counts = parse_conflict_markers(out);
        assert_eq!(counts.get("a.txt"), Some(&2));
        assert_eq!(counts.get("b.txt"), None);
    }

    fn repo() -> (tempfile::TempDir, Arc<GitBinary>) {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), &["init", "-q", "-b", "main"]);
        run(dir.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-q", "-m", "first"]);
        (dir, Arc::new(crate::find_git().unwrap()))
    }

    fn run(path: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .status()
                .unwrap()
                .success(),
            "git {args:?}"
        );
    }

    fn commit_file(path: &Path, name: &str, contents: &str, message: &str) {
        std::fs::write(path.join(name), contents).unwrap();
        run(path, &["add", "."]);
        run(path, &["commit", "-q", "-m", message]);
    }

    #[test]
    fn rebase_completes_and_reports_up_to_date() {
        let (dir, git) = repo();
        let path = dir.path();
        run(path, &["checkout", "-q", "-b", "feature"]);
        commit_file(path, "f.txt", "f\n", "feature work");
        run(path, &["checkout", "-q", "main"]);
        commit_file(path, "m.txt", "m\n", "main work");
        run(path, &["checkout", "-q", "feature"]);
        let commits = commits_between(git.clone(), path, "main", "feature")
            .unwrap()
            .unwrap();
        assert_eq!(commits.len(), 1);
        let mut seen = Vec::new();
        let result = rebase(git.clone(), path, "main", "feature", &commits, false, |p| {
            seen.push(p)
        });
        assert_eq!(result, RebaseResult::CompletedWithoutError);
        assert!(!seen.is_empty(), "progress lines were parsed");
        assert!(path.join("m.txt").exists());
        assert_eq!(
            rebase(git, path, "main", "feature", &commits, false, |_| {}),
            RebaseResult::AlreadyUpToDate
        );
    }

    #[test]
    fn rebase_conflict_state_continue_and_abort() {
        let (dir, git) = repo();
        let path = dir.path();
        run(path, &["checkout", "-q", "-b", "feature"]);
        commit_file(path, "a.txt", "feature\n", "feature change");
        run(path, &["checkout", "-q", "main"]);
        commit_file(path, "a.txt", "main\n", "main change");
        run(path, &["checkout", "-q", "feature"]);
        let result = rebase(git.clone(), path, "main", "feature", &[], false, |_| {});
        assert_eq!(result, RebaseResult::ConflictsEncountered);
        let state = rebase_internal_state(path).unwrap();
        assert_eq!(state.target_branch, "feature");
        let status = crate::status::get_status(git.clone(), path, None).unwrap();
        let conflicted = status
            .files
            .iter()
            .find(|f| f.path == "a.txt")
            .expect("conflicted file");
        assert_eq!(conflicted.status.kind, FileStatusKind::Conflicted);
        assert!(conflicted.status.conflict_markers.unwrap_or(0) > 0);
        assert!(status.rebase_internal_state.is_some());
        // resolve by taking our side (the base branch during a rebase)
        let mut resolutions = BTreeMap::new();
        resolutions.insert("a.txt".to_string(), ManualConflictResolution::Theirs);
        let result = continue_rebase(
            git.clone(),
            path,
            &status.files,
            &resolutions,
            &[],
            false,
            |_| {},
        )
        .unwrap();
        assert_eq!(result, RebaseResult::CompletedWithoutError);
        assert!(!rebase_head_set(path));
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "feature\n"
        );
        // abort path
        run(path, &["checkout", "-q", "main"]);
        run(path, &["checkout", "-q", "-b", "other", "HEAD~1"]);
        commit_file(path, "a.txt", "other\n", "other change");
        assert_eq!(
            rebase(git.clone(), path, "main", "other", &[], false, |_| {}),
            RebaseResult::ConflictsEncountered
        );
        abort_rebase(git, path).unwrap();
        assert!(!rebase_head_set(path));
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "other\n"
        );
    }

    #[test]
    fn rebase_keeps_hash_messages_across_a_conflict() {
        let (dir, git) = repo();
        let path = dir.path();
        run(path, &["checkout", "-q", "-b", "feature"]);
        std::fs::write(path.join("a.txt"), "feature\n").unwrap();
        run(path, &["add", "."]);
        run(
            path,
            &["commit", "-q", "-m", "#12 feature", "-m", "# kept too"],
        );
        run(path, &["checkout", "-q", "main"]);
        commit_file(path, "a.txt", "main\n", "main change");
        run(path, &["checkout", "-q", "feature"]);
        assert_eq!(
            rebase(git.clone(), path, "main", "feature", &[], true, |_| {}),
            RebaseResult::ConflictsEncountered
        );
        let status = crate::status::get_status(git.clone(), path, None).unwrap();
        let mut resolutions = BTreeMap::new();
        resolutions.insert("a.txt".to_string(), ManualConflictResolution::Theirs);
        let result = continue_rebase(
            git.clone(),
            path,
            &status.files,
            &resolutions,
            &[],
            true,
            |_| {},
        )
        .unwrap();
        assert_eq!(result, RebaseResult::CompletedWithoutError);
        let out = GitCommand::new(git)
            .args(["log", "-1", "--format=%B"])
            .current_dir(path)
            .run()
            .unwrap()
            .stdout_string()
            .unwrap();
        assert_eq!(out.trim_end(), "#12 feature\n\n# kept too");
    }

    #[test]
    fn squash_and_reorder_rewrite_history() {
        let (dir, git) = repo();
        let path = dir.path();
        commit_file(path, "b.txt", "b\n", "second");
        commit_file(path, "c.txt", "c\n", "third");
        let all = commits_in_range(git.clone(), path, "HEAD")
            .unwrap()
            .unwrap();
        assert_eq!(all.len(), 3);
        let identity = corvane_models::CommitIdentity {
            name: "T".into(),
            email: "t@example.com".into(),
            seconds: 0,
            offset: 0,
        };
        let full = |one: &CommitOneLine| Commit {
            sha: one.sha.clone(),
            summary: one.summary.clone(),
            body: String::new(),
            author: identity.clone(),
            committer: identity.clone(),
            parents: Vec::new(),
            tags: Vec::new(),
        };
        // squash "third" onto "second", keeping "first"
        let result = squash(
            git.clone(),
            path,
            &[full(&all[2])],
            &full(&all[1]),
            Some(&all[0].sha),
            "combined\n\nsecond + third",
            false,
            |_| {},
        );
        assert_eq!(result, RebaseResult::CompletedWithoutError);
        let after = commits_in_range(git.clone(), path, "HEAD")
            .unwrap()
            .unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(after[1].summary, "combined");
        assert!(path.join("c.txt").exists());
        // reorder: move "first" after "combined" is impossible (root); move combined before first → same order
        commit_file(path, "d.txt", "d\n", "fourth");
        let all = commits_in_range(git.clone(), path, "HEAD")
            .unwrap()
            .unwrap();
        let result = reorder(
            git.clone(),
            path,
            &[full(&all[2])],
            Some(&full(&all[1])),
            Some(&all[0].sha),
            false,
            |_| {},
        );
        assert_eq!(result, RebaseResult::CompletedWithoutError);
        let after = commits_in_range(git, path, "HEAD").unwrap().unwrap();
        let summaries: Vec<&str> = after.iter().map(|c| c.summary.as_str()).collect();
        assert_eq!(summaries, ["first", "fourth", "combined"]);
    }

    #[test]
    fn cherry_pick_copies_and_conflicts() {
        let (dir, git) = repo();
        let path = dir.path();
        run(path, &["checkout", "-q", "-b", "feature"]);
        commit_file(path, "f.txt", "f\n", "feature commit");
        let feature = commits_in_range(git.clone(), path, "main..feature")
            .unwrap()
            .unwrap();
        run(path, &["checkout", "-q", "main"]);
        let mut progress = Vec::new();
        assert_eq!(
            cherry_pick(git.clone(), path, &feature, false, |p| progress.push(p)),
            CherryPickResult::CompletedWithoutError
        );
        assert!(path.join("f.txt").exists());
        assert_eq!(progress.len(), 1);
        // conflicting pick
        run(path, &["checkout", "-q", "feature"]);
        commit_file(path, "a.txt", "feature\n", "feature edits a");
        let picks = commits_in_range(git.clone(), path, "HEAD~1..HEAD")
            .unwrap()
            .unwrap();
        run(path, &["checkout", "-q", "main"]);
        commit_file(path, "a.txt", "main\n", "main edits a");
        assert_eq!(
            cherry_pick(git.clone(), path, &picks, false, |_| {}),
            CherryPickResult::ConflictsEncountered
        );
        assert!(cherry_pick_head_found(path));
        let snapshot = cherry_pick_snapshot(git.clone(), path).unwrap();
        assert_eq!(snapshot.commits.len(), 1);
        assert_eq!(
            determine_mergeability(git.clone(), path, "main", "feature").unwrap(),
            Mergeability::Conflicts(1)
        );
        abort_cherry_pick(git, path).unwrap();
        assert!(!cherry_pick_head_found(path));
    }

    #[test]
    fn cherry_pick_keeps_hash_messages_across_a_conflict() {
        let (dir, git) = repo();
        let path = dir.path();
        run(path, &["checkout", "-q", "-b", "feature"]);
        std::fs::write(path.join("a.txt"), "feature\n").unwrap();
        run(path, &["add", "."]);
        run(path, &["commit", "-q", "-m", "#7 edits a", "-m", "# body"]);
        commit_file(path, "b.txt", "b\n", "#8 adds b");
        let picks = commits_in_range(git.clone(), path, "main..feature")
            .unwrap()
            .unwrap();
        run(path, &["checkout", "-q", "main"]);
        commit_file(path, "a.txt", "main\n", "main edits a");
        let log = |n: &str| {
            GitCommand::new(git.clone())
                .args(["log", n, "--format=%B"])
                .current_dir(path)
                .run()
                .unwrap()
                .stdout_string()
                .unwrap()
        };
        let resolve = |side: ManualConflictResolution| {
            let status = crate::status::get_status(git.clone(), path, None).unwrap();
            let mut resolutions = BTreeMap::new();
            resolutions.insert("a.txt".to_string(), side);
            continue_cherry_pick(git.clone(), path, &status.files, &resolutions, true, |_| {})
                .unwrap()
        };
        // two picks, the first conflicts: the sequencer goes on afterwards
        assert_eq!(
            cherry_pick(git.clone(), path, &picks, true, |_| {}),
            CherryPickResult::ConflictsEncountered
        );
        assert_eq!(
            resolve(ManualConflictResolution::Theirs),
            CherryPickResult::CompletedWithoutError
        );
        assert!(!cherry_pick_head_found(path));
        assert_eq!(log("-2"), "#8 adds b\n\n#7 edits a\n\n# body\n\n");
        // one pick resolved to our side: an empty commit, same message
        run(path, &["reset", "-q", "--hard", "HEAD~2"]);
        assert_eq!(
            cherry_pick(git.clone(), path, &picks[..1], true, |_| {}),
            CherryPickResult::ConflictsEncountered
        );
        assert_eq!(
            resolve(ManualConflictResolution::Ours),
            CherryPickResult::CompletedWithoutError
        );
        assert_eq!(log("-1"), "#7 edits a\n\n# body\n\n");
    }

    #[test]
    fn squash_merge_uses_the_given_message() {
        let (dir, git) = repo();
        let path = dir.path();
        run(path, &["checkout", "-q", "-b", "feature"]);
        commit_file(path, "f.txt", "f\n", "feature one");
        commit_file(path, "a.txt", "feature\n", "feature two");
        run(path, &["checkout", "-q", "main"]);
        let log = || {
            GitCommand::new(git.clone())
                .args(["log", "-1", "--format=%B"])
                .current_dir(path)
                .run()
                .unwrap()
                .stdout_string()
                .unwrap()
        };
        assert_eq!(
            crate::merge_branch_with_message(
                git.clone(),
                path,
                "feature",
                true,
                Some("Feature\n\nall of it")
            )
            .unwrap(),
            crate::MergeOutcome::Success
        );
        assert_eq!(log().trim_end(), "Feature\n\nall of it");
        // conflicting: the message waits in SQUASH_MSG for the commit
        run(path, &["reset", "-q", "--hard", "HEAD~1"]);
        commit_file(path, "a.txt", "main\n", "main edits a");
        assert_eq!(
            crate::merge_branch_with_message(git.clone(), path, "feature", true, Some("Squashed"))
                .unwrap(),
            crate::MergeOutcome::Conflicts
        );
        let status = crate::status::get_status(git.clone(), path, None).unwrap();
        let conflicted: Vec<_> = status
            .files
            .iter()
            .filter(|f| f.status.kind == FileStatusKind::Conflicted)
            .cloned()
            .collect();
        let mut resolutions = BTreeMap::new();
        resolutions.insert("a.txt".to_string(), ManualConflictResolution::Theirs);
        create_merge_commit(git.clone(), path, &conflicted, &resolutions).unwrap();
        assert_eq!(log().trim_end(), "Squashed");
    }

    #[test]
    fn merge_conflict_commit_and_squash_abort() {
        let (dir, git) = repo();
        let path = dir.path();
        run(path, &["checkout", "-q", "-b", "feature"]);
        commit_file(path, "a.txt", "feature\n", "feature edits a");
        run(path, &["checkout", "-q", "main"]);
        commit_file(path, "a.txt", "main\n", "main edits a");
        assert_eq!(
            crate::merge_branch(git.clone(), path, "feature", false).unwrap(),
            crate::MergeOutcome::Conflicts
        );
        assert!(merge_head_set(path));
        let status = crate::status::get_status(git.clone(), path, None).unwrap();
        let conflicted: Vec<_> = status
            .files
            .iter()
            .filter(|f| f.status.kind == FileStatusKind::Conflicted)
            .cloned()
            .collect();
        assert_eq!(conflicted.len(), 1);
        let mut resolutions = BTreeMap::new();
        resolutions.insert("a.txt".to_string(), ManualConflictResolution::Ours);
        let sha = create_merge_commit(git.clone(), path, &conflicted, &resolutions).unwrap();
        assert_eq!(sha.len(), 40);
        assert!(!merge_head_set(path));
        assert_eq!(
            std::fs::read_to_string(path.join("a.txt")).unwrap(),
            "main\n"
        );
        // squash merge that conflicts, then abort it
        run(path, &["checkout", "-q", "-b", "other", "HEAD~1"]);
        commit_file(path, "a.txt", "other\n", "other edits a");
        let tip = crate::head_sha(git.clone(), path).unwrap();
        assert_eq!(
            crate::merge_branch(git.clone(), path, "feature", true).unwrap(),
            crate::MergeOutcome::Conflicts
        );
        assert!(squash_msg_set(path));
        abort_squash_merge(git.clone(), path, &tip).unwrap();
        assert!(!squash_msg_set(path));
        assert_eq!(crate::head_sha(git, path).unwrap(), tip);
    }
}
