//! Error / InstallGit / CLIInstalled (GHD `ui/cli-installed/cli-installed.tsx`)
//! dialogs: static content, one or two buttons.

use corvane_core::{Dispatcher, Popup};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::theme::sizes::*;

pub struct SimpleDialog {
    popup: Popup,
}

impl SimpleDialog {
    pub fn new(popup: Popup) -> Self {
        Self { popup }
    }
}

impl Render for SimpleDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        match &self.popup {
            Popup::InstallGit { reason } => dialog(
                "dialog-install-git",
                "Unable to locate Git",
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING())
                    .child(format!(
                        "Corvane was unable to find a usable Git on your system ({reason})."
                    ))
                    .child(
                        "Install the Xcode Command Line Tools (xcode-select --install) or run \
                         `brew install git`, then click Retry.",
                    ),
                vec![
                    DialogButton {
                        id: "install-git-cancel",
                        label: "Cancel".into(),
                        primary: false,
                        disabled: false,
                        on_click: Box::new(close),
                    },
                    DialogButton {
                        id: "install-git-retry",
                        label: "Retry".into(),
                        primary: true,
                        disabled: false,
                        on_click: Box::new(|_, cx| {
                            Dispatcher::close_popup(cx);
                            Dispatcher::detect_git(cx);
                        }),
                    },
                ],
                close,
                window,
                cx,
            )
            .into_any_element(),
            Popup::Error { title, message } => dialog(
                "dialog-error",
                title.clone(),
                div().child(message.clone()),
                vec![DialogButton {
                    id: "error-close",
                    label: "Close".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                }],
                close,
                window,
                cx,
            )
            .into_any_element(),
            Popup::CLIInstalled { path } => dialog(
                "cli-installed",
                "Command Line Tool Installed",
                crate::widgets::paragraph(vec![
                    "The command line tool has been installed at ".into(),
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(path.display().to_string())
                        .into_any_element()
                        .into(),
                    ".".into(),
                ]),
                vec![DialogButton {
                    id: "cli-installed-ok",
                    label: "Ok".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                }],
                close,
                window,
                cx,
            )
            .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}
