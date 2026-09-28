//! OAuth device flow (RFC 8628) against GitHub. No client secret involved.
//!
//! 1. `request_device_code` → show `user_code`, open `verification_uri`.
//! 2. Poll `poll_token` every `interval` seconds until `Token`/`Denied`/`Expired`.
//! 3. `Client::new(endpoint, token).current_user()` → `Account`.

use std::time::Duration;

use serde::Deserialize;
use tracing::{debug, info};

use crate::endpoint::Endpoint;
use crate::error::{GitHubError, Result};
use crate::{CLIENT_ID, SCOPES, USER_AGENT};

#[derive(Clone, Debug, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

impl DeviceCode {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.interval.max(1))
    }
}

#[derive(Debug)]
pub enum PollOutcome {
    /// User has not finished yet; poll again after the interval.
    Pending,
    /// GitHub asked us to add 5 s to the interval.
    SlowDown,
    /// Success: the access token and the granted scopes.
    Token { token: String, scopes: Vec<String> },
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .new_agent()
}

/// POST `/login/device/code`.
pub fn request_device_code(endpoint: &Endpoint, client_id: &str) -> Result<DeviceCode> {
    let url = endpoint.web("login/device/code");
    debug!(%url, "requesting device code");
    let mut response = agent()
        .post(&url)
        .header("Accept", "application/json")
        .send_form([("client_id", client_id), ("scope", SCOPES)])?;
    let status = response.status().as_u16();
    if status != 200 {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        return Err(GitHubError::Api {
            status,
            message: body,
        });
    }
    let code: DeviceCode = response.body_mut().read_json()?;
    info!(user_code = %code.user_code, expires_in = code.expires_in, "device code issued");
    Ok(code)
}

/// One POST to `/login/oauth/access_token`. Map GitHub's `error` field to an outcome.
pub fn poll_token(endpoint: &Endpoint, client_id: &str, device_code: &str) -> Result<PollOutcome> {
    let url = endpoint.web("login/oauth/access_token");
    let mut response = agent()
        .post(&url)
        .header("Accept", "application/json")
        .send_form([
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])?;
    let status = response.status().as_u16();
    let body: TokenResponse = response.body_mut().read_json()?;
    match (body.access_token, body.error.as_deref()) {
        (Some(token), _) => {
            let scopes = body
                .scope
                .unwrap_or_default()
                .split([',', ' '])
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            info!("device flow completed");
            Ok(PollOutcome::Token { token, scopes })
        }
        (None, Some("authorization_pending")) => Ok(PollOutcome::Pending),
        (None, Some("slow_down")) => Ok(PollOutcome::SlowDown),
        (None, Some("expired_token")) => Err(GitHubError::Expired),
        (None, Some("access_denied")) => Err(GitHubError::Denied),
        (None, Some(other)) => Err(GitHubError::Auth(
            body.error_description.unwrap_or_else(|| other.to_string()),
        )),
        (None, None) => Err(GitHubError::Api {
            status,
            message: "no token in response".into(),
        }),
    }
}

/// Convenience: the default OAuth App.
pub fn request_device_code_default(endpoint: &Endpoint) -> Result<DeviceCode> {
    request_device_code(endpoint, CLIENT_ID)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_token_response_shapes() {
        let ok: TokenResponse = serde_json::from_str(
            r#"{"access_token":"gho_x","token_type":"bearer","scope":"repo,workflow"}"#,
        )
        .unwrap();
        assert_eq!(ok.access_token.as_deref(), Some("gho_x"));
        let pending: TokenResponse = serde_json::from_str(
            r#"{"error":"authorization_pending","error_description":"...","error_uri":"..."}"#,
        )
        .unwrap();
        assert_eq!(pending.error.as_deref(), Some("authorization_pending"));
        assert!(pending.access_token.is_none());
    }

    #[test]
    fn client_id_is_set() {
        assert_eq!(CLIENT_ID.len(), 20);
        assert!(CLIENT_ID.starts_with("Ov23li"));
    }
}
