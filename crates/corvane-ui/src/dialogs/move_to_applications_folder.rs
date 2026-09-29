//! GHD `ui/move-to-applications-folder.tsx`: offered at launch when the app
//! runs from outside the Applications folder.
//!
//! Deviation: GHD's dialog is not dismissed by a backdrop click
//! (`backdropDismissable={false}`); Corvane's dialog frame closes on the
//! backdrop like every other dialog, which counts as "Not Now" without
//! saving the checkbox.

use corvane_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::theme::sizes::*;
use crate::widgets::checkbox_row;

pub struct MoveToApplicationsFolderDialog {
    /// `askToMoveToApplicationsFolder` (the checkbox shows its inverse).
    ask_again: bool,
}

impl MoveToApplicationsFolderDialog {
    pub fn new() -> Self {
        Self { ask_again: true }
    }
}

impl Default for MoveToApplicationsFolderDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for MoveToApplicationsFolderDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let weak = cx.weak_entity();
        let ask_again = self.ask_again;
        let content = div()
            .w(zpx(420.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                "We've detected that you're not running Corvane from the Applications folder of \
                 your machine. This could cause problems with the app, including impacting your \
                 ability to sign in.",
            )
            .child(
                "Do you want to move Corvane to the Applications folder now? This will also \
                 restart the app.",
            )
            .child(checkbox_row(
                "move-to-applications-dont-ask",
                !ask_again,
                "Do not show this message again",
                move |checked, _, cx| {
                    weak.update(cx, |this, cx| {
                        this.ask_again = !checked;
                        cx.notify();
                    })
                    .ok();
                },
                cx,
            ));
        dialog_with_kind(
            "move-to-applications-folder",
            DialogKind::Warning,
            "Move Corvane to the Applications folder?",
            content,
            vec![
                DialogButton {
                    id: "move-to-applications-not-now",
                    label: "Not Now".into(),
                    primary: false,
                    disabled: false,
                    // `onNotNow`
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::set_ask_to_move_to_applications_folder(ask_again, cx);
                    }),
                },
                DialogButton {
                    id: "move-to-applications-ok",
                    label: "Move and Restart".into(),
                    primary: true,
                    disabled: false,
                    // `onSubmit`
                    on_click: Box::new(|_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::move_to_applications_folder(cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
