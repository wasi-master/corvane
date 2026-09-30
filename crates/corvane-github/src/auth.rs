//! OAuth device flow (RFC 8628) against GitHub. No client secret involved.
//! The client ID is the endpoint's `OAuthApp` (Corvane's on GitHub.com, an
//! administrator-registered one on GitHub Enterprise).
//!
//! 1. `request_device_code` → show `user_code`, open `verification_uri`.
//! 2. Poll `poll_token` every `interval` seconds until `Token`/`Denied`/`Expired`.
//! 3. `Client::new(endpoint, token).current_user()` → `Account`.

use std::time::Duration;

use serde::Deserialize;
use tracing::{debug, info};

use crate::endpoint::Endpoint;
use crate::error::{GitHubError, Result};
use crate::{SCOPES, USER_AGENT};

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
        use crate::CLIENT_ID;
        assert_eq!(CLIENT_ID.len(), 20);
        assert!(CLIENT_ID.starts_with("Ov23li"));
    }
}

// ---- browser (web application) flow -------------------------------------
//
// GHD `SignInStore.authenticateWithBrowser` + `resolveOAuthRequest` with
// `getOAuthAuthorizationURL` / `requestOAuthToken` (`lib/api.ts`): the
// browser opens `/login/oauth/authorize`, GitHub redirects to the app's
// callback with `code` and the CSRF `state`, the app exchanges the code for
// a token. Corvane adds PKCE (`code_challenge` S256) and, when no client
// secret was compiled in (`CORVANE_GITHUB_CLIENT_SECRET`), exchanges the
// code with the verifier alone - which needs the OAuth app to allow it;
// otherwise GitHub answers that `client_secret` is required and the device
// flow stays the way in.

/// A pending browser sign-in: the CSRF state and PKCE verifier the callback
/// must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebFlow {
    pub state: String,
    pub code_verifier: String,
    pub redirect_uri: String,
}

/// The callback the OAuth app must list: the `x-corvane-auth` URL scheme.
pub const SCHEME_REDIRECT_URI: &str = "x-corvane-auth://oauth";

/// Cryptographically random bytes as base64url (no padding).
fn random_token(bytes: usize) -> Result<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|err| GitHubError::Auth(err.to_string()))?;
    Ok(data_encoding::BASE64URL_NOPAD.encode(&buf))
}

/// RFC 7636 `code_challenge` (S256) for `verifier`.
pub fn code_challenge(verifier: &str) -> String {
    use sha2::{Digest, Sha256};
    data_encoding::BASE64URL_NOPAD.encode(&Sha256::digest(verifier.as_bytes()))
}

impl WebFlow {
    /// A fresh flow redirecting to `redirect_uri`.
    pub fn new(redirect_uri: impl Into<String>) -> Result<Self> {
        Ok(Self {
            state: random_token(24)?,
            code_verifier: random_token(48)?,
            redirect_uri: redirect_uri.into(),
        })
    }

    /// `getOAuthAuthorizationURL` + PKCE: where the browser goes.
    pub fn authorize_url(&self, endpoint: &Endpoint, client_id: &str) -> String {
        format!(
            "{}?client_id={}&scope={}&state={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256",
            endpoint.web("login/oauth/authorize"),
            url_encode(client_id),
            url_encode(SCOPES),
            url_encode(&self.state),
            url_encode(&self.redirect_uri),
            code_challenge(&self.code_verifier)
        )
    }

    /// `requestOAuthToken`: the code from the callback → token (+ scopes).
    /// `client_secret` is what GHD sends; without one the verifier must do.
    pub fn exchange_code(
        &self,
        endpoint: &Endpoint,
        client_id: &str,
        client_secret: Option<&str>,
        code: &str,
    ) -> Result<(String, Vec<String>)> {
        let url = endpoint.web("login/oauth/access_token");
        let mut form: Vec<(&str, &str)> = vec![
            ("client_id", client_id),
            ("code", code),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("code_verifier", self.code_verifier.as_str()),
        ];
        if let Some(secret) = client_secret.filter(|s| !s.is_empty()) {
            form.push(("client_secret", secret));
        }
        let mut response = agent()
            .post(&url)
            .header("Accept", "application/json")
            .send_form(form)?;
        let status = response.status().as_u16();
        let body: TokenResponse = response.body_mut().read_json()?;
        match (body.access_token, body.error.as_deref()) {
            (Some(token), _) => Ok((
                token,
                body.scope
                    .unwrap_or_default()
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim().to_string())
                    .collect(),
            )),
            (None, Some(error)) => Err(GitHubError::Auth(format!(
                "{error}: {}",
                body.error_description.unwrap_or_default()
            ))),
            (None, None) => Err(GitHubError::Auth(format!(
                "the token exchange answered with status {status}"
            ))),
        }
    }
}

/// Percent-encode a query value (RFC 3986 unreserved characters stay).
fn url_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The `code` and `state` of a callback URL's query
/// (`x-corvane-auth://oauth?code=…&state=…`, `http://127.0.0.1:1234/callback?…`).
pub fn parse_callback_query(query: &str) -> Option<(String, String)> {
    let mut code = None;
    let mut state = None;
    for pair in query.trim_start_matches('?').split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = url_decode(value);
        match key {
            "code" => code = Some(value),
            "state" => state = Some(value),
            _ => {}
        }
    }
    Some((code?, state?))
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len() + 1
            && let Some(hex) = value.get(i + 1..i + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The loopback fallback (`http://127.0.0.1:<port>/callback`): a one-shot
/// HTTP listener on an ephemeral port for browsers that cannot hand the
/// custom URL scheme to the app. `on_callback` runs on the listener thread
/// with the request's `code` and `state`.
#[derive(Debug)]
pub struct LoopbackListener {
    pub redirect_uri: String,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    port: u16,
}

impl LoopbackListener {
    pub fn start(
        on_callback: impl FnOnce(Option<(String, String)>) + Send + 'static,
    ) -> Result<Self> {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|err| GitHubError::Auth(format!("loopback listener: {err}")))?;
        let port = listener
            .local_addr()
            .map_err(|err| GitHubError::Auth(err.to_string()))?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|err| GitHubError::Auth(err.to_string()))?;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_stop = stop.clone();
        std::thread::Builder::new()
            .name("oauth-loopback".into())
            .spawn(move || {
                let mut on_callback = Some(on_callback);
                loop {
                    if worker_stop.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    let (mut stream, _) = match listener.accept() {
                        Ok(pair) => pair,
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(100));
                            continue;
                        }
                        Err(_) => return,
                    };
                    // the accepted socket must block (a non-blocking listener's
                    // children may not on every platform), and the request
                    // line may arrive in pieces
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut raw = Vec::with_capacity(1024);
                    let mut buf = [0u8; 1024];
                    loop {
                        match stream.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                raw.extend_from_slice(&buf[..n]);
                                if raw.windows(2).any(|w| w == b"\r\n") || raw.len() > 8192 {
                                    break;
                                }
                            }
                            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock
                                || err.kind() == std::io::ErrorKind::Interrupted =>
                            {
                                continue;
                            }
                            Err(_) => break,
                        }
                    }
                    let request = String::from_utf8_lossy(&raw);
                    // `GET /callback?code=…&state=… HTTP/1.1`
                    let query = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .and_then(|path| path.split_once('?').map(|(_, q)| q.to_string()));
                    let result = query.as_deref().and_then(parse_callback_query);
                    let body = if result.is_some() {
                        "<!doctype html><title>Corvane</title><p>Signed in. You can close this window and return to Corvane.</p>"
                    } else {
                        "<!doctype html><title>Corvane</title><p>Corvane did not receive a sign-in code from this request.</p>"
                    };
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.flush();
                    if result.is_some()
                        && let Some(callback) = on_callback.take()
                    {
                        callback(result);
                        return;
                    }
                }
            })
            .map_err(|err| GitHubError::Auth(err.to_string()))?;
        Ok(Self {
            redirect_uri: format!("http://127.0.0.1:{port}/callback"),
            stop,
            port,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for LoopbackListener {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod web_flow_tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc_7636() {
        // RFC 7636 appendix B
        assert_eq!(
            code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorize_url_carries_state_scope_and_challenge() {
        let flow = WebFlow {
            state: "st ate".into(),
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
            redirect_uri: SCHEME_REDIRECT_URI.into(),
        };
        let url = flow.authorize_url(&Endpoint::github_com(), "abc");
        assert_eq!(
            url,
            "https://github.com/login/oauth/authorize?client_id=abc&scope=repo%20workflow%20read%3Auser%20user%3Aemail&state=st%20ate&redirect_uri=x-corvane-auth%3A%2F%2Foauth&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256"
        );
        let fresh = WebFlow::new(SCHEME_REDIRECT_URI).unwrap();
        assert!(fresh.state.len() >= 32 && fresh.code_verifier.len() >= 43);
        assert_ne!(fresh.state, fresh.code_verifier);
    }

    #[test]
    fn callback_query_parsing() {
        assert_eq!(
            parse_callback_query("code=18142422&state=e4cd2dea"),
            Some(("18142422".into(), "e4cd2dea".into()))
        );
        assert_eq!(
            parse_callback_query("?state=a%20b&code=c%2Bd&other=1"),
            Some(("c+d".into(), "a b".into()))
        );
        assert_eq!(parse_callback_query("code=only"), None);
    }

    #[test]
    fn loopback_listener_hands_over_the_code() {
        use std::io::{Read, Write};
        let (tx, rx) = std::sync::mpsc::channel();
        let listener = LoopbackListener::start(move |result| {
            let _ = tx.send(result);
        })
        .unwrap();
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", listener.port())).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        write!(
            stream,
            "GET /callback?code=xyz&state=123 HTTP/1.1\r\nHost: localhost\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Signed in"));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(30)).unwrap(),
            Some(("xyz".into(), "123".into()))
        );
    }
}
