//! GHD `ui/secret-scanning/{push-protection-error-dialog,
//! bypass-push-protection-dialog,push-protection-error-location}.tsx`
//! (`styles/ui/dialogs/_push-protection.scss`).

use std::collections::HashSet;

use corvane_core::{BypassReason, Dispatcher, Popup, SecretLocation, SecretScanResult};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog, dialog_with_kind};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::theme::{c, mono_font, primer};
use crate::widgets::{Inline, link_button, paragraph, segmented_option};

pub struct PushProtectionErrorDialog {
    repo: u64,
    secrets: Vec<SecretScanResult>,
    bypassed: Vec<String>,
    /// `showMoreLocations` per secret id.
    expanded: HashSet<String>,
}

impl PushProtectionErrorDialog {
    pub fn new(repo: u64, secrets: Vec<SecretScanResult>, bypassed: Vec<String>) -> Self {
        Self {
            repo,
            secrets,
            bypassed,
            expanded: HashSet::new(),
        }
    }

    /// `renderLocation`
    fn location(&self, location: &SecretLocation, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let sha = location.commit_sha.clone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING())
            .text_size(FONT_SIZE())
            .child(
                div()
                    .flex_none()
                    .min_w(zpx(95.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .child(octicon(Octicon::GitCommit, t.text_secondary))
                    .child(
                        div()
                            .font_family(mono_font())
                            .child(location.commit_sha.chars().take(7).collect::<String>()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "copy-sha-{}",
                                location.commit_sha
                            )))
                            .cursor_pointer()
                            .tooltip(crate::widgets::tooltip("Copy the full SHA"))
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(sha.clone()))
                            })
                            .child(octicon(Octicon::Copy, t.text_secondary)),
                    ),
            )
            .child(div().flex_1().min_w_0().truncate().child(format!(
                "{} at line {}",
                location.path, location.line_number
            )))
            .into_any_element()
    }

    /// `renderBypassButton`
    fn bypass_button(&self, secret: &SecretScanResult, cx: &Context<Self>) -> AnyElement {
        let id = SharedString::from(format!("bypass-{}", secret.id));
        if secret.requires_approval {
            let url = secret.bypass_url.clone();
            return link_button(id, "Bypass", cx)
                .on_click(move |_, _, cx| cx.open_url(&url))
                .into_any_element();
        }
        if self.bypassed.contains(&secret.id) {
            return div()
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF())
                .child("Bypassed")
                .child(octicon(Octicon::Check, c(primer::GREEN_500)))
                .into_any_element();
        }
        let (repo, secret, secrets, bypassed) = (
            self.repo,
            secret.clone(),
            self.secrets.clone(),
            self.bypassed.clone(),
        );
        link_button(id, "Bypass", cx)
            .on_click(move |_, _, cx| {
                Dispatcher::show_popup(
                    Popup::BypassPushProtection {
                        repo,
                        secret: secret.clone(),
                        secrets: secrets.clone(),
                        bypassed: bypassed.clone(),
                    },
                    cx,
                )
            })
            .into_any_element()
    }
}

impl Render for PushProtectionErrorDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let docs = |id: &'static str, label: &'static str, url: &'static str, cx: &App| {
            Inline::Element(
                link_button(id, label, cx)
                    .on_click(move |_, _, cx| cx.open_url(url))
                    .into_any_element(),
            )
        };
        let bullet = |text: &'static str| {
            div()
                .flex()
                .flex_row()
                .gap(SPACING_HALF())
                .child("•")
                .child(text)
        };
        let secrets: Vec<AnyElement> = self
            .secrets
            .iter()
            .map(|secret| {
                let duplicates = self
                    .secrets
                    .iter()
                    .filter(|s| s.description == secret.description)
                    .count()
                    > 1;
                let title = if duplicates {
                    format!("{} ({})", secret.description, secret.id)
                } else {
                    secret.description.clone()
                };
                let expanded = self.expanded.contains(&secret.id);
                let more = secret.locations.len() > 1;
                let toggle_id = secret.id.clone();
                div()
                    .flex()
                    .flex_col()
                    .p(SPACING())
                    .border_t_1()
                    .border_color(t.box_border)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .mb(SPACING_HALF())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(self.bypass_button(secret, cx)),
                    )
                    .children(secret.locations.first().map(|first| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(div().flex_1().min_w_0().child(self.location(first, cx)))
                            .when(more, |d| {
                                d.child(
                                    div()
                                        .id(SharedString::from(format!("more-{}", secret.id)))
                                        .flex_none()
                                        .px(SPACING_HALF())
                                        .cursor_pointer()
                                        .tooltip(crate::widgets::tooltip(if expanded {
                                            "Show Less Locations"
                                        } else {
                                            "Show More locations"
                                        }))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if !this.expanded.remove(&toggle_id) {
                                                this.expanded.insert(toggle_id.clone());
                                            }
                                            cx.notify();
                                        }))
                                        .child(octicon(Octicon::KebabHorizontal, t.text_secondary)),
                                )
                            })
                    }))
                    .when(expanded, |d| {
                        d.children(
                            secret
                                .locations
                                .iter()
                                .skip(1)
                                .map(|location| self.location(location, cx)),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let content = div()
            .w(zpx(460.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(paragraph(vec![
                docs(
                    "secret-scanning-docs",
                    "Secret Scanning",
                    "https://docs.github.com/code-security/secret-scanning/protecting-pushes-with-secret-scanning",
                    cx,
                ),
                " found secret(s) in the commit(s) you attempted to push.".into(),
            ]))
            .child(paragraph(vec![
                "Allowing secrets risks exposure. Consider ".into(),
                docs(
                    "secret-remediation-docs",
                    "removing the secret from your commit and commit history.",
                    "https://docs.github.com/code-security/secret-scanning/working-with-secret-scanning-and-push-protection/working-with-push-protection-in-the-github-ui#resolving-a-blocked-commit",
                    cx,
                ),
            ]))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child("Exposing this secret can allow someone to:")
                    .child(bullet("Verify the identity of the secret(s)"))
                    .child(bullet("Know which resources the secret(s) can access"))
                    .child(bullet("Act on behalf of the secret's owner"))
                    .child(bullet(
                        "Push the secret(s) to this repository without being blocked",
                    )),
            )
            .child(
                // `ul.secret-list`
                div()
                    .id("secret-list")
                    .max_h(zpx(200.))
                    .overflow_y_scroll()
                    .border_1()
                    .border_color(t.box_border)
                    .flex()
                    .flex_col()
                    .children(secrets).with_scrollbar(),
            );
        dialog_with_kind(
            "push-protection-error",
            DialogKind::Error,
            "Push Blocked: Secret Detected",
            content,
            vec![DialogButton {
                id: "push-protection-ok",
                label: "OK".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(close),
            }],
            close,
            window,
            cx,
        )
    }
}

pub struct BypassPushProtectionDialog {
    repo: u64,
    secret: SecretScanResult,
    secrets: Vec<SecretScanResult>,
    bypassed: Vec<String>,
    reason: BypassReason,
}

impl BypassPushProtectionDialog {
    pub fn new(
        repo: u64,
        secret: SecretScanResult,
        secrets: Vec<SecretScanResult>,
        bypassed: Vec<String>,
    ) -> Self {
        Self {
            repo,
            secret,
            secrets,
            bypassed,
            reason: BypassReason::FalsePositive,
        }
    }
}

impl Render for BypassPushProtectionDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let reason = self.reason;
        let items = [
            (
                BypassReason::UsedInTests,
                "bypass-reason-tests",
                "It's used in tests",
                "The secret poses no risk. If anyone finds it, they cannot do any damage or gain access to sensitive information.",
            ),
            (
                BypassReason::FalsePositive,
                "bypass-reason-false-positive",
                "It's a false positive",
                "The detected string is not a secret",
            ),
            (
                BypassReason::WillFixLater,
                "bypass-reason-later",
                "I'll fix it later",
                "The secret is real, I understand the risk, and I will need to revoke it. This will open a security alert and notify admins of this repository.",
            ),
        ];
        let count = items.len();
        let content =
            div()
                .w(zpx(440.))
                .flex()
                .flex_col()
                .child(div().mb(SPACING()).child(format!(
                    "Why are you bypassing this {}?",
                    self.secret.description
                )))
                .children(items.into_iter().enumerate().map(
                    |(ix, (key, id, title, description))| {
                        segmented_option(
                            id,
                            title,
                            description,
                            reason == key,
                            ix == 0,
                            ix + 1 == count,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.reason = key;
                            cx.notify();
                        }))
                    },
                ));
        let (repo, secret, secrets, bypassed) = (
            self.repo,
            self.secret.clone(),
            self.secrets.clone(),
            self.bypassed.clone(),
        );
        dialog(
            "bypass-push-protection",
            "Bypass Push Detection",
            content,
            vec![
                DialogButton {
                    id: "bypass-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "bypass-ok",
                    label: "Allow me to expose this secret".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::bypass_push_protection(
                            repo,
                            secret.clone(),
                            reason,
                            secrets.clone(),
                            bypassed.clone(),
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
