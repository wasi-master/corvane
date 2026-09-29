//! Browser (web application) OAuth flow - GHD `SignInStore.authenticateWithBrowser`
//! / `resolveOAuthRequest` (`lib/stores/sign-in-store.ts`) and
//! `.docs/oauth.md`, on `corvane_github::auth::WebFlow`. The device
//! flow stays the default (no client secret is bundled); this is the
//! alternative behind "Sign in with your browser's session" for OAuth apps
//! that allow PKCE (or builds with `CORVANE_GITHUB_CLIENT_SECRET`).
//!
//! The callback arrives as `x-corvane-auth://oauth?code=…&state=…` through
//! `Dispatcher::handle_app_url`, or on the loopback listener
//! (`http://127.0.0.1:<port>/callback`) when the URL scheme cannot be used
//! (a bare binary outside the bundle, or `CORVANE_OAUTH_LOOPBACK=1`).
//! `state` must match the flow that opened the browser; the token only ever
//! goes to the Keychain (`finish_sign_in`).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use corvane_github::auth::{LoopbackListener, SCHEME_REDIRECT_URI, WebFlow};
use corvane_github::{CLIENT_ID, CLIENT_SECRET, Endpoint};
use gpui_kit::App;
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::{PendingWebFlow, SignInState, SignInStep};

/// Whether the callback must come over the loopback listener.
fn use_loopback() -> bool {
    std::env::var_os("CORVANE_OAUTH_LOOPBACK").is_some()
        || corvane_platform::app_location::running_bundle().is_none()
}

impl Dispatcher {
    /// `authenticateWithBrowser`: start the web flow for `endpoint` and open
    /// GitHub's authorize page.
    pub fn sign_in_web_flow(endpoint: Endpoint, cx: &mut App) {
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
        let authorize_url = flow.authorize_url(&endpoint, CLIENT_ID);
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
        cx.open_url(&authorize_url);
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
        Self::set_sign_in_step(SignInStep::Verifying, cx);
        let exchange_endpoint = endpoint.clone();
        spawn_bg(
            cx,
            move || flow.exchange_code(&exchange_endpoint, CLIENT_ID, CLIENT_SECRET, &code),
            move |result, cx| match result {
                Ok((token, scopes)) => {
                    Self::state(cx).update(cx, |s, _| {
                        if let Some(sign_in) = s.sign_in.as_mut() {
                            sign_in.web_flow = None;
                        }
                    });
                    Self::finish_sign_in_public(endpoint, token, scopes, cx);
                }
                Err(err) => {
                    let hint = if CLIENT_SECRET.is_none() {
                        " (this build has no client secret; the OAuth app must allow PKCE, or use the device flow)"
                    } else {
                        ""
                    };
                    Self::set_sign_in_step(SignInStep::Error(format!("{err}{hint}")), cx);
                }
            },
        );
    }
}
