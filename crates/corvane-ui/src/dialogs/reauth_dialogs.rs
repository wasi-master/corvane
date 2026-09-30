//! Re-authentication prompts: `ui/invalidated-token/invalidated-token.tsx`,
//! `ui/saml-reauth-required/saml-reauth-required.tsx` and
//! `ui/workflow-push-rejected/workflow-push-rejected.tsx`.
//!
//! Deviation: GHD re-authorises through the browser OAuth flow; Corvane's
//! sign-in is the device flow, so "Continue in Browser" opens the sign-in
//! dialog (the device code is entered in the browser). As in GHD, the push
//! (workflow scope) or the failed action (SAML) is retried once signing in
//! succeeds.

use corvane_core::{Account, Dispatcher, Popup, RetryAction};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::theme::sizes::zpx;
use crate::widgets::{Inline, code_ref, paragraph};

pub struct InvalidatedTokenDialog {
    account: Account,
}

impl InvalidatedTokenDialog {
    pub fn new(account: Account) -> Self {
        Self { account }
    }
}

impl Render for InvalidatedTokenDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let enterprise = !self.account.is_dotcom();
        let content = paragraph(vec![
            "Your account token has been invalidated and you have been signed out from your "
                .into(),
            Inline::Element(code_ref(self.account.host(), cx).into_any_element()),
            " account. Do you want to sign in again?".into(),
        ]);
        dialog_with_kind(
            "invalidated-token",
            DialogKind::Warning,
            mac_or("Invalidated Account Token", "Invalidated account token"),
            content,
            vec![
                DialogButton {
                    id: "invalidated-token-no",
                    label: "No".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "invalidated-token-yes",
                    label: "Yes".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::show_popup(Popup::SignIn { enterprise }, cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `WorkflowPushRejectedDialog`
pub struct WorkflowPushRejectedDialog {
    repo: u64,
    rejected_path: String,
}

impl WorkflowPushRejectedDialog {
    pub fn new(repo: u64, rejected_path: String) -> Self {
        Self {
            repo,
            rejected_path,
        }
    }
}

impl Render for WorkflowPushRejectedDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let repo = self.repo;
        let content = div()
            .w(zpx(460.))
            .flex()
            .flex_col()
            .gap(crate::theme::sizes::SPACING())
            .child(paragraph(vec![
                "The push was rejected by the server for containing a modification to the workflow file "
                    .into(),
                Inline::Element(code_ref(self.rejected_path.clone(), cx).into_any_element()),
                ". In order to be able to push to workflow files Corvane needs to request additional permissions."
                    .into(),
            ]))
            .child(paragraph(vec![
                "Would you like to sign in again to grant Corvane permission to update workflow files?"
                    .into(),
            ]));
        dialog_with_kind(
            "workflow-push-rejected",
            DialogKind::Error,
            mac_or("Push Rejected", "Push rejected"),
            content,
            vec![
                DialogButton {
                    id: "workflow-push-rejected-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "workflow-push-rejected-ok",
                    label: mac_or("Continue in Browser", "Continue in browser").into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        // `onSignIn`: sign in, then `dispatcher.push(repository)`
                        Dispatcher::sign_in_then_retry(
                            false,
                            repo,
                            Some(RetryAction::Push {
                                force_with_lease: false,
                                branch: None,
                                up_to: None,
                            }),
                            cx,
                        );
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `SAMLReauthRequiredDialog`
pub struct SamlReauthRequiredDialog {
    repo: u64,
    organization: String,
    enterprise: bool,
    retry: Option<RetryAction>,
}

impl SamlReauthRequiredDialog {
    pub fn new(
        repo: u64,
        organization: String,
        endpoint: String,
        retry: Option<RetryAction>,
    ) -> Self {
        Self {
            repo,
            organization,
            enterprise: endpoint != "https://api.github.com",
            retry,
        }
    }
}

impl Render for SamlReauthRequiredDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let enterprise = self.enterprise;
        let (repo, retry) = (self.repo, self.retry.clone());
        let content = div()
            .w(zpx(460.))
            .flex()
            .flex_col()
            .gap(crate::theme::sizes::SPACING())
            .child(paragraph(vec![format!(
                "The \"{}\" organization has enabled or enforced SAML SSO. To access this \
                 repository, you must sign in again and grant Corvane permission to access the \
                 organization's repositories.",
                self.organization
            )
            .into()]))
            .child(paragraph(vec![
                "Would you like to sign in again to grant Corvane permission to access the repository?"
                    .into(),
            ]));
        dialog_with_kind(
            "saml-reauth-required",
            DialogKind::Error,
            mac_or("Re-authorization Required", "Re-authorization required"),
            content,
            vec![
                DialogButton {
                    id: "saml-reauth-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "saml-reauth-ok",
                    label: mac_or("Continue in Browser", "Continue in browser").into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        // `onSignIn`: sign in, then `performRetry(retryAction)`
                        Dispatcher::sign_in_then_retry(enterprise, repo, retry.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
