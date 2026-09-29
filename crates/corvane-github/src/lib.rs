//! GitHub API + OAuth device flow. Blocking `ureq` calls;
//! run them on a background thread.

pub mod api;
pub mod auth;
pub mod endpoint;
pub mod error;

pub use api::{ApiIssue, ApiMentionableUser, Client, IssueState};
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

/// Plain GET of a small binary resource (avatars); 5 s per phase, no auth.
pub fn download(url: &str) -> Result<Vec<u8>> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .user_agent(USER_AGENT)
        .build()
        .new_agent();
    let mut response = agent.get(url).call()?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(GitHubError::Api {
            status,
            message: url.to_string(),
        });
    }
    let bytes = response.body_mut().read_to_vec()?;
    Ok(bytes)
}
