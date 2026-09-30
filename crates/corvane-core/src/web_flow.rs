//! Browser (web application) OAuth flow - GHD `SignInStore.authenticateWithBrowser`
//! / `resolveOAuthRequest` (`lib/stores/sign-in-store.ts`) and
//! `.docs/oauth.md`, on `corvane_github::auth::WebFlow`. The device
//! flow stays the default (no client secret is bundled); this is the
//! alternative behind "Sign in with your browser's session" for OAuth apps
//! that allow PKCE (or builds with `CORVANE_GITHUB_CLIENT_SECRET`). On GitHub
//! Enterprise the app is the host's (`Dispatcher::oauth_client_id`), its
//! secret from the keychain or `CORVANE_GHES_OAUTH`.
//!
//! The callback arrives as `x-corvane-auth://oauth?code=…&state=…` through
//! `Dispatcher::handle_app_url`, or on the loopback listener
//! (`http://127.0.0.1:<port>/callback`) when the URL scheme cannot be used
//! (a bare binary outside the bundle, on Linux one whose `.desktop` entry
//! is not the scheme's handler, or `CORVANE_OAUTH_LOOPBACK=1`).
//! `state` must match the flow that opened the browser; the token only ever
//! goes to the Keychain (`finish_sign_in`).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use corvane_github::Endpoint;
use corvane_github::auth::{LoopbackListener, SCHEME_REDIRECT_URI, WebFlow};
use gpui_kit::App;
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::{PendingWebFlow, SignInState, SignInStep};

/// Whether the callback must come over the loopback listener.
fn use_loopback() -> bool {
    std::env::var_os("CORVANE_OAUTH_LOOPBACK").is_some()
        || !corvane_platform::url_schemes::auth_callback_registered()
}

/// Why an OAuth sign-in cannot start on `endpoint`.
pub(crate) fn no_oauth_app_message(endpoint: &corvane_github::Endpoint) -> String {
    format!(
        "No OAuth app is set up for {}. Register one there, enter its client ID, or sign in with a personal access token.",
        endpoint.host()
    )
}

/// The client secret for the browser flow's token exchange on `endpoint`:
/// the build's on GitHub.com; on GitHub Enterprise the keychain's for
/// (host, client ID), else a `CORVANE_GHES_OAUTH` entry with that client ID.
/// Touches the keychain: call off the foreground.
pub(crate) fn oauth_client_secret(
    endpoint: &corvane_github::Endpoint,
    client_id: &str,
) -> Option<String> {
    if endpoint.is_dotcom() {
        return corvane_github::CLIENT_SECRET.map(str::to_string);
    }
    let host = endpoint.host().to_ascii_lowercase();
    corvane_platform::keychain::oauth_client_secret(&host, client_id)
        .ok()
        .flatten()
        .or_else(|| {
            corvane_github::OAuthApp::built_in(endpoint)
                .filter(|app| app.client_id == client_id)
                .and_then(|app| app.client_secret)
        })
}

impl Dispatcher {
    /// `authenticateWithBrowser`: start the web flow for `endpoint` and open
    /// GitHub's authorize page.
    pub fn sign_in_web_flow(endpoint: Endpoint, cx: &mut App) {
        let client_id = Self::oauth_client_id(&endpoint, cx);
        let state = Self::state(cx);
        state.update(cx, |s, cx| {
            if let Some(existing) = s.sign_in.as_ref() {
                existing
                    .cancel
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
            s.sign_in = Some(SignInState {
                endpoint: endpoint.api_base.clone(),
                step: SignInStep::Requesting,
                cancel: Arc::new(AtomicBool::new(false)),
                web_flow: None,
            });
            cx.notify();
        });
        let Some(client_id) = client_id else {
            Self::set_sign_in_step(SignInStep::Error(no_oauth_app_message(&endpoint)), cx);
            return;
        };
        let (flow, loopback) = if use_loopback() {
            let (tx, rx) = async_channel::unbounded::<Option<(String, String)>>();
            let listener = match LoopbackListener::start(move |result| {
                let _ = tx.send_blocking(result);
            }) {
                Ok(listener) => listener,
                Err(err) => {
                    Self::set_sign_in_step(SignInStep::Error(err.to_string()), cx);
                    return;
                }
            };
            cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
                if let Ok(Some((code, state))) = rx.recv().await {
                    cx.update(|cx| Self::complete_web_flow(code, state, cx));
                }
            })
            .detach();
            match WebFlow::new(listener.redirect_uri.clone()) {
                Ok(flow) => (flow, Some(Arc::new(listener))),
                Err(err) => {
                    Self::set_sign_in_step(SignInStep::Error(err.to_string()), cx);
                    return;
                }
            }
        } else {
            match WebFlow::new(SCHEME_REDIRECT_URI) {
                Ok(flow) => (flow, None),
                Err(err) => {
                    Self::set_sign_in_step(SignInStep::Error(err.to_string()), cx);
                    return;
                }
            }
        };
        let authorize_url = flow.authorize_url(&endpoint, &client_id);
        info!(redirect = %flow.redirect_uri, "starting the browser sign-in");
        state.update(cx, |s, cx| {
            if let Some(sign_in) = s.sign_in.as_mut() {
                sign_in.web_flow = Some(PendingWebFlow { flow, loopback });
                sign_in.step = SignInStep::Browser {
                    authorize_url: authorize_url.clone(),
                };
            }
            cx.notify();
        });
        crate::Dispatcher::open_url(&authorize_url, cx);
    }

    /// `resolveOAuthRequest`: the callback's `code` and `state`; the state
    /// must be the pending flow's, then the code is exchanged for a token.
    pub fn complete_web_flow(code: String, state: String, cx: &mut App) {
        let pending = {
            let s = Self::state(cx).read(cx);
            s.sign_in.as_ref().and_then(|si| {
                si.web_flow
                    .as_ref()
                    .map(|w| (w.flow.clone(), Endpoint::from_api_base(&si.endpoint)))
            })
        };
        let Some((flow, endpoint)) = pending else {
            warn!("OAuth callback without a browser sign-in in progress");
            return;
        };
        if flow.state != state {
            // "This is likely due to a browser reloading the callback URL."
            Self::set_sign_in_step(
                SignInStep::Error(
                    "The sign-in response did not match this sign-in attempt. Start again from Corvane."
                        .into(),
                ),
                cx,
            );
            return;
        }
        let Some(client_id) = Self::oauth_client_id(&endpoint, cx) else {
            Self::set_sign_in_step(SignInStep::Error(no_oauth_app_message(&endpoint)), cx);
            return;
        };
        Self::set_sign_in_step(SignInStep::Verifying, cx);
        let exchange_endpoint = endpoint.clone();
        spawn_bg(
            cx,
            move || {
                let secret = oauth_client_secret(&exchange_endpoint, &client_id);
                let result =
                    flow.exchange_code(&exchange_endpoint, &client_id, secret.as_deref(), &code);
                (result, secret.is_some())
            },
            move |(result, had_secret), cx| match result {
                Ok((token, scopes)) => {
                    Self::state(cx).update(cx, |s, _| {
                        if let Some(sign_in) = s.sign_in.as_mut() {
                            sign_in.web_flow = None;
                        }
                    });
                    Self::finish_sign_in_public(endpoint, token, scopes, cx);
                }
                Err(err) => {
                    let hint = if had_secret {
                        ""
                    } else if endpoint.is_dotcom() {
                        " (this build has no client secret; the OAuth app must allow PKCE, or use the device flow)"
                    } else {
                        " (no client secret is set for this OAuth app; enter it, or use the device flow)"
                    };
                    Self::set_sign_in_step(SignInStep::Error(format!("{err}{hint}")), cx);
                }
            },
        );
    }
}
