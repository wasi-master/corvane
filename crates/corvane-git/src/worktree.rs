//! Worktrees (GHD `lib/git/worktree.ts`): `git worktree list/add/remove/move`.

use std::path::Path;
use std::sync::Arc;

use corvane_models::{WorktreeEntry, WorktreeType};

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// GHD `parseWorktreePorcelainOutput` for `--porcelain -z`: NUL-separated
/// lines, blank line (double NUL) between entries; the first entry is the
/// main worktree.
pub fn parse_worktree_porcelain(stdout: &[u8]) -> Vec<WorktreeEntry> {
    let text = String::from_utf8_lossy(stdout);
    let text = text.trim_end_matches('\0');
    if text.trim().is_empty() {
        return Vec::new();
    }
    text.split("\0\0")
        .enumerate()
        .map(|(index, block)| {
            let mut entry = WorktreeEntry {
                path: Path::new("").to_path_buf(),
                head: String::new(),
                branch: None,
                is_detached: false,
                kind: if index == 0 {
                    WorktreeType::Main
                } else {
                    WorktreeType::Linked
                },
                is_locked: false,
                is_prunable: false,
            };
            for line in block.split('\0') {
                if let Some(path) = line.strip_prefix("worktree ") {
                    entry.path = Path::new(path).to_path_buf();
                } else if let Some(head) = line.strip_prefix("HEAD ") {
                    entry.head = head.to_string();
                } else if let Some(branch) = line.strip_prefix("branch ") {
                    entry.branch = Some(branch.to_string());
                } else if line == "detached" {
                    entry.is_detached = true;
                } else if line == "locked" || line.starts_with("locked ") {
                    entry.is_locked = true;
                } else if line == "prunable" || line.starts_with("prunable ") {
                    entry.is_prunable = true;
                }
            }
            entry
        })
        .filter(|e| !e.path.as_os_str().is_empty())
        .collect()
}

/// GHD `listWorktrees`.
pub fn list_worktrees(git: Arc<GitBinary>, workdir: &Path) -> Result<Vec<WorktreeEntry>> {
    let out = GitCommand::new(git)
        .args(["worktree", "list", "--porcelain", "-z"])
        .current_dir(workdir)
        .run()?;
    Ok(parse_worktree_porcelain(&out.stdout))
}

/// GHD `addWorktree`: `git worktree add [-b <branch>] <path> [<commitish>]`.
pub fn add_worktree(
    git: Arc<GitBinary>,
    workdir: &Path,
    path: &Path,
    create_branch: Option<&str>,
    commitish: Option<&str>,
) -> Result<()> {
    let mut cmd = GitCommand::new(git)
        .args(["worktree", "add"])
        .current_dir(workdir);
    if let Some(branch) = create_branch {
        cmd = cmd.args(["-b", branch]);
    }
    cmd = cmd.arg(path);
    if let Some(commitish) = commitish {
        cmd = cmd.arg(commitish);
    }
    cmd.run()?;
    Ok(())
}

/// GHD `removeWorktree`: `git worktree remove [--force] <path>`.
pub fn remove_worktree(
    git: Arc<GitBinary>,
    workdir: &Path,
    path: &Path,
    force: bool,
) -> Result<()> {
    let mut cmd = GitCommand::new(git)
        .args(["worktree", "remove"])
        .current_dir(workdir);
    if force {
        cmd = cmd.arg("--force");
    }
    cmd.arg(path).run()?;
    Ok(())
}

/// GHD `moveWorktree`: `git worktree move <old> <new>`.
pub fn move_worktree(git: Arc<GitBinary>, workdir: &Path, old: &Path, new: &Path) -> Result<()> {
    GitCommand::new(git)
        .args(["worktree", "move"])
        .arg(old)
        .arg(new)
        .current_dir(workdir)
        .run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_blocks() {
        let out = b"worktree /repo\0HEAD abcdef1234567\0branch refs/heads/main\0\0worktree /repo-feat\0HEAD 1234567abcdef\0branch refs/heads/feat\0locked\0\0worktree /repo-old\0HEAD 7654321abcdef\0detached\0prunable gitdir file points to non-existent location\0";
        let entries = parse_worktree_porcelain(out);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].kind, WorktreeType::Main);
        assert_eq!(entries[0].description(), "main");
        assert_eq!(entries[1].kind, WorktreeType::Linked);
        assert!(entries[1].is_locked);
        assert_eq!(entries[1].display_name(), "repo-feat");
        assert!(entries[2].is_detached && entries[2].is_prunable);
        assert_eq!(entries[2].description(), "7654321");
        assert!(parse_worktree_porcelain(b"").is_empty());
    }
}
