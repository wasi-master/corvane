//! GitHub API + OAuth device flow. Blocking `ureq` calls;
//! run them on a background thread.

pub mod api;
pub mod auth;
pub mod endpoint;
pub mod error;

pub use api::Client;
pub use endpoint::Endpoint;
pub use error::{GitHubError, Result};

/// OAuth App "Corvane" (public identifier; device flow needs no secret).
/// Override at build time with `CORVANE_GITHUB_CLIENT_ID`.
pub const CLIENT_ID: &str = match option_env!("CORVANE_GITHUB_CLIENT_ID") {
    Some(id) => id,
    None => "Ov23li8x4b9tpSBwDEOy",
};

/// Scopes GitHub Desktop requests, plus `read:user`/`user:email` for the account card.
pub const SCOPES: &str = "repo workflow read:user user:email";

pub const USER_AGENT: &str = concat!("Corvane/", env!("CARGO_PKG_VERSION"));
