#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("network error: {0}")]
    Http(#[from] ureq::Error),
    #[error("unexpected response: {0}")]
    Json(#[from] serde_json::Error),
    #[error("GitHub returned {status}: {message}")]
    Api { status: u16, message: String },
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("the sign-in request expired; try again")]
    Expired,
    #[error("sign-in was denied")]
    Denied,
}

pub type Result<T> = std::result::Result<T, GitHubError>;
