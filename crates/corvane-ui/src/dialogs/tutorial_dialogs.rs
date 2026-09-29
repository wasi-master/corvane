//! Tutorial dialogs - GHD `ui/no-repositories/create-tutorial-repository-dialog.tsx`
//! (`#create-tutorial-repository-dialog`, 450 px, `styles/ui/dialogs/_create-tutorial-repository.scss`)
//! and `ui/tutorial/confirm-exit-tutorial.tsx`.

use corvane_core::{Account, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog, dialog_loading};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, MONO_FONT};
use crate::widgets::{Inline, code_ref, link_button, paragraph};

/// `getHTMLURL`: the web address of an API endpoint.
fn html_url(endpoint: &str) -> String {
    if endpoint.trim_end_matches('/') == "https://api.github.com" {
        return "https://github.com".to_string();
    }
    endpoint
        .trim_end_matches('/')
        .trim_end_matches("/api/v3")
        .to_string()
}

/// `CreateTutorialRepository`: "Start tutorial".
pub struct CreateTutorialRepositoryDialog {
    account: Account,
    progress: Option<(String, u8, Option<String>)>,
}

impl CreateTutorialRepositoryDialog {
    pub fn new(account: Account, progress: Option<(String, u8, Option<String>)>) -> Self {
        Self { account, progress }
    }
}

impl Render for CreateTutorialRepositoryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let loading = self.progress.is_some();
        // `dismissDisabled={loading}`
        let close = move |_: &mut Window, cx: &mut App| {
            if !loading {
                Dispatcher::close_popup(cx)
            }
        };
        let site = html_url(&self.account.endpoint);
        let friendly = self.account.host();
        let content = div()
            .w(zpx(410.))
            .flex()
            .flex_col()
            .child(paragraph(vec![
                "This will create a repository on your local machine, and push it to your account "
                    .into(),
                Inline::Element(
                    code_ref(format!("@{}", self.account.login), cx).into_any_element(),
                ),
                " on ".into(),
                Inline::Element(
                    link_button("tutorial-endpoint", friendly, cx)
                        .on_click(move |_, _, cx| Dispatcher::open_url(&site, cx))
                        .into_any_element(),
                ),
                ". This repository will only be visible to you, and not visible publicly.".into(),
            ]))
            .children(self.progress.clone().map(|(title, value, detail)| {
                // `.progress-container`
                div()
                    .mt(SPACING())
                    .flex()
                    .flex_col()
                    .gap(zpx(4.))
                    .child(title)
                    .child(
                        div()
                            .w_full()
                            .h(zpx(6.))
                            .rounded(zpx(3.))
                            .bg(t.box_alt_background)
                            .border_1()
                            .border_color(t.box_border)
                            .child(
                                div()
                                    .h_full()
                                    .rounded(zpx(3.))
                                    .bg(t.button_background)
                                    .w(relative(f32::from(value.min(100)) / 100.)),
                            ),
                    )
                    .children(detail.map(|d| {
                        div()
                            .font_family(MONO_FONT)
                            .text_size(FONT_SIZE_SM())
                            .text_color(t.text_secondary)
                            .truncate()
                            .child(d)
                    }))
            }));
        let account = self.account.clone();
        dialog_loading(
            "create-tutorial-repository-dialog",
            "Start tutorial",
            loading,
            content,
            vec![
                DialogButton {
                    id: "tutorial-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: loading,
                    on_click: Box::new(|_, cx| Dispatcher::close_popup(cx)),
                },
                DialogButton {
                    id: "tutorial-continue",
                    label: "Continue".into(),
                    primary: true,
                    disabled: loading,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::create_tutorial_repository(account.clone(), cx)
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `ConfirmExitTutorial`
pub struct ConfirmExitTutorialDialog;

impl Render for ConfirmExitTutorialDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        dialog(
            "confirm-exit-tutorial",
            "Exit Tutorial",
            div().w(zpx(360.)).child(
                "Are you sure you want to leave the tutorial? This will bring you back to the \
                 home screen.",
            ),
            vec![
                DialogButton {
                    id: "exit-tutorial-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(|_, cx| Dispatcher::close_popup(cx)),
                },
                DialogButton {
                    id: "exit-tutorial-ok",
                    label: "Exit Tutorial".into(),
                    primary: true,
                    disabled: false,
                    // `onExitTutorialToHomeScreen` → `pauseTutorial`
                    on_click: Box::new(|_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::pause_tutorial(cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
