//! Sign-in dialog (`ui/sign-in/sign-in.tsx`). GitHub.com uses the OAuth
//! device flow (code shown here, authorised in the browser); GitHub
//! Enterprise takes an address then a personal access token.

use corvane_core::{AppState, Dispatcher, SignInStep};
use corvane_github::Endpoint;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, labeled, primary_button, text_box};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    /// GHES only: enter the server address.
    EndpointEntry,
    /// Device flow (dot com) or token entry (GHES).
    Authentication,
    TokenEntry,
}

pub struct SignInDialog {
    state: Entity<AppState>,
    enterprise: bool,
    step: Step,
    endpoint: Option<Endpoint>,
    address: Entity<InputState>,
    token: Entity<InputState>,
}

impl SignInDialog {
    pub fn new(
        state: Entity<AppState>,
        enterprise: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let address =
            cx.new(|cx| InputState::new(window, cx).placeholder("https://github.example.com"));
        let token = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("ghp_…")
                .masked(true)
        });
        for e in [&address, &token] {
            cx.observe(e, |_, _, cx| cx.notify()).detach();
        }
        let (step, endpoint) = if enterprise {
            let handle = address.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
            (Step::EndpointEntry, None)
        } else {
            (Step::Authentication, Some(Endpoint::github_com()))
        };
        Self {
            state,
            enterprise,
            step,
            endpoint,
            address,
            token,
        }
    }

    fn title(&self) -> &'static str {
        if self.enterprise {
            "Sign in to GitHub Enterprise"
        } else {
            "Sign in to GitHub.com"
        }
    }

    fn continue_endpoint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.address.read(cx).value().to_string();
        if let Some(endpoint) = Endpoint::enterprise(&raw) {
            self.endpoint = Some(endpoint);
            self.step = Step::TokenEntry;
            let handle = self.token.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
            cx.notify();
        }
    }

    fn submit_token(&mut self, cx: &mut Context<Self>) {
        let token = self.token.read(cx).value().trim().to_string();
        if let (Some(endpoint), false) = (self.endpoint.clone(), token.is_empty()) {
            Dispatcher::sign_in_with_token(endpoint, token, cx);
        }
    }

    fn authentication_body(&self, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let sign_in = self.state.read(cx).sign_in.clone();
        let this = cx.entity();
        match sign_in.map(|s| s.step) {
            None => div()
                .flex()
                .flex_col()
                .items_start()
                .gap(SPACING)
                .child(
                    "Corvane will show you a one-time code and open GitHub in your browser. \
                     Enter the code there to authorise this app.",
                )
                .child(
                    primary_button("sign-in-browser", "Sign in using your browser", false, cx)
                        .on_click(|_, _, cx| {
                            Dispatcher::sign_in_device_flow(Endpoint::github_com(), cx)
                        }),
                )
                .child(
                    div()
                        .id("sign-in-token-link")
                        .text_color(t.link)
                        .cursor_pointer()
                        .child("Sign in with a personal access token instead")
                        .on_click(move |_, _, cx| {
                            this.update(cx, |d, cx| {
                                d.step = Step::TokenEntry;
                                cx.notify();
                            })
                        }),
                )
                .into_any_element(),
            Some(SignInStep::Requesting) => div()
                .text_color(t.text_secondary)
                .child("Requesting a sign-in code from GitHub…")
                .into_any_element(),
            Some(SignInStep::DeviceCode {
                user_code,
                verification_uri,
            }) => {
                let code_for_copy = user_code.clone();
                let uri = verification_uri.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING)
                    .child(format!("Enter this code at {verification_uri} to sign in:"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING)
                            .child(
                                div()
                                    .px(SPACING_DOUBLE)
                                    .py(SPACING)
                                    .rounded(BORDER_RADIUS)
                                    .border_1()
                                    .border_color(t.box_border_contrast)
                                    .bg(t.box_alt_background)
                                    .font_family(crate::theme::MONO_FONT)
                                    .text_size(px(28.))
                                    .line_height(px(34.))
                                    .child(user_code.clone()),
                            )
                            .child(
                                button("sign-in-copy", "Copy code", cx)
                                    .child(
                                        octicon(Octicon::Copy, t.secondary_button_text)
                                            .ml(SPACING_HALF),
                                    )
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            code_for_copy.clone(),
                                        ))
                                    }),
                            )
                            .child(
                                button("sign-in-open", "Open GitHub", cx)
                                    .child(
                                        octicon(Octicon::LinkExternal, t.secondary_button_text)
                                            .ml(SPACING_HALF),
                                    )
                                    .on_click(move |_, _, cx| cx.open_url(&uri)),
                            ),
                    )
                    .child(
                        div()
                            .text_color(t.text_secondary)
                            .child("Waiting for you to authorise Corvane in the browser…"),
                    )
                    .into_any_element()
            }
            Some(SignInStep::Verifying) => div()
                .text_color(t.text_secondary)
                .child("Signing in…")
                .into_any_element(),
            Some(SignInStep::Error(message)) => div()
                .flex()
                .flex_col()
                .gap(SPACING)
                .child(div().text_color(t.error).child(message))
                .child(
                    primary_button("sign-in-retry", "Try again", false, cx).on_click(|_, _, cx| {
                        Dispatcher::cancel_sign_in(cx);
                        Dispatcher::sign_in_device_flow(Endpoint::github_com(), cx)
                    }),
                )
                .into_any_element(),
        }
    }
}

impl Render for SignInDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| {
            Dispatcher::cancel_sign_in(cx);
            Dispatcher::close_popup(cx);
        };
        let this = cx.entity();
        let sign_in_error = self
            .state
            .read(cx)
            .sign_in
            .as_ref()
            .and_then(|s| match &s.step {
                SignInStep::Error(m) => Some(m.clone()),
                _ => None,
            });

        let (body, buttons): (AnyElement, Vec<DialogButton>) = match self.step {
            Step::EndpointEntry => (
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING)
                    .child(labeled(
                        "Enterprise address",
                        text_box("sign-in-address", &self.address, None, window, cx),
                        cx,
                    ))
                    .into_any_element(),
                vec![
                    DialogButton {
                        id: "sign-in-cancel",
                        label: "Cancel".into(),
                        primary: false,
                        disabled: false,
                        on_click: Box::new(close),
                    },
                    DialogButton {
                        id: "sign-in-continue",
                        label: "Continue".into(),
                        primary: true,
                        disabled: false,
                        on_click: Box::new(move |window, cx| {
                            this.update(cx, |d, cx| d.continue_endpoint(window, cx))
                        }),
                    },
                ],
            ),
            Step::TokenEntry => (
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING)
                    .child(
                        "Create a token with the repo, workflow, read:user and user:email scopes, \
                         then paste it here.",
                    )
                    .child(labeled(
                        "Personal access token",
                        text_box("sign-in-token", &self.token, None, window, cx),
                        cx,
                    ))
                    .when_some(sign_in_error, |d, m| {
                        d.child(div().text_color(t.error).child(m))
                    })
                    .into_any_element(),
                vec![
                    DialogButton {
                        id: "sign-in-cancel",
                        label: "Cancel".into(),
                        primary: false,
                        disabled: false,
                        on_click: Box::new(close),
                    },
                    DialogButton {
                        id: "sign-in-submit",
                        label: "Sign in".into(),
                        primary: true,
                        disabled: false,
                        on_click: Box::new(move |_, cx| {
                            this.update(cx, |d, cx| d.submit_token(cx))
                        }),
                    },
                ],
            ),
            Step::Authentication => (
                self.authentication_body(cx),
                vec![DialogButton {
                    id: "sign-in-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                }],
            ),
        };

        dialog(
            "sign-in",
            self.title(),
            div().w(px(560.)).child(body),
            buttons,
            close,
            window,
            cx,
        )
    }
}
