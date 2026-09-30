use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git was not found on this machine")]
    GitNotFound,
    #[error("git {found} is too old; Corvane needs {required} or newer")]
    GitTooOld { found: String, required: String },
    #[error("could not run git: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("git {args} failed with exit code {code:?}: {stderr}")]
    Failed {
        args: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("git output was not valid UTF-8")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0} is not a git repository")]
    NotARepository(PathBuf),
    #[error("could not open repository: {0}")]
    Open(#[from] Box<gix::Error>),
    #[error("{0}")]
    Gix(String),
}

impl From<gix::Error> for GitError {
    fn from(err: gix::Error) -> Self {
        GitError::Open(Box::new(err))
    }
}

pub type Result<T> = std::result::Result<T, GitError>;

/// GHD `getRepositoryType`'s `unsafe` case: the directory named by git's
/// "fatal: detected dubious ownership in repository at '<path>'".
pub fn dubious_ownership_path(stderr: &str) -> Option<PathBuf> {
    const MARKER: &str = "detected dubious ownership in repository at '";
    let start = stderr.find(MARKER)? + MARKER.len();
    let rest = &stderr[start..];
    // the path is quoted up to the end of the line
    let line = rest.lines().next()?;
    let end = line.rfind('\'')?;
    Some(PathBuf::from(&line[..end]))
}

/// The branch and worktree of git's refusals to use a branch that another
/// worktree has checked out: `'main' is already used by worktree at '<path>'`
/// (checkout, git 2.42+; older: `is already checked out at`) and `cannot
/// delete branch 'x' used by worktree at '<path>'` (older: `checked out at`).
pub fn branch_in_other_worktree(stderr: &str) -> Option<(String, PathBuf)> {
    const MARKERS: [&str; 4] = [
        "' is already used by worktree at '",
        "' is already checked out at '",
        "' used by worktree at '",
        "' checked out at '",
    ];
    stderr.lines().find_map(|line| {
        let (at, marker) = MARKERS
            .iter()
            .find_map(|m| line.find(m).map(|at| (at, *m)))?;
        let branch_start = line[..at].rfind('\'')? + 1;
        let rest = &line[at + marker.len()..];
        let path_end = rest.rfind('\'')?;
        Some((
            line[branch_start..at].to_string(),
            PathBuf::from(&rest[..path_end]),
        ))
    })
}

impl GitError {
    /// [`branch_in_other_worktree`] of a failed git command.
    pub fn branch_in_other_worktree(&self) -> Option<(String, PathBuf)> {
        match self {
            GitError::Failed { stderr, .. } => branch_in_other_worktree(stderr),
            _ => None,
        }
    }

    /// The unsafe directory when git refused to run because of its owner.
    pub fn unsafe_repository_path(&self) -> Option<PathBuf> {
        match self {
            GitError::Failed { stderr, .. } => dubious_ownership_path(stderr),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dubious_ownership() {
        let stderr = "fatal: detected dubious ownership in repository at '/Users/o'brien/repo'\n\
                      To add an exception for this directory, call:\n\n\
                      \tgit config --global --add safe.directory '/Users/o'brien/repo'\n";
        assert_eq!(
            dubious_ownership_path(stderr),
            Some(PathBuf::from("/Users/o'brien/repo"))
        );
        assert_eq!(dubious_ownership_path("fatal: not a git repository"), None);
    }

    #[test]
    fn parses_branch_in_other_worktree() {
        let cases = [
            "fatal: 'main' is already used by worktree at '/tmp/w 2'\n",
            "fatal: 'main' is already checked out at '/tmp/w 2'\n",
            "error: cannot delete branch 'main' used by worktree at '/tmp/w 2'\n",
            "error: Cannot delete branch 'main' checked out at '/tmp/w 2'\n",
        ];
        for stderr in cases {
            assert_eq!(
                branch_in_other_worktree(stderr),
                Some(("main".to_string(), PathBuf::from("/tmp/w 2"))),
                "{stderr}"
            );
        }
        assert_eq!(
            branch_in_other_worktree("error: branch 'x' not found."),
            None
        );
    }
}
