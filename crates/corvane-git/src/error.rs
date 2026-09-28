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
