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

use crate::dialog::{DialogButton, DialogKind, dialog_with_kind_opts};
use crate::theme::sizes::*;
use crate::widgets::checkbox_row_focus;

pub struct MoveToApplicationsFolderDialog {
    /// `askToMoveToApplicationsFolder` (the checkbox shows its inverse).
    ask_again: bool,
    /// The autofocused checkbox's ring, until a mouse press.
    focus_visible: bool,
}

impl MoveToApplicationsFolderDialog {
    pub fn new() -> Self {
        Self {
            ask_again: true,
            focus_visible: true,
        }
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
            .flex()
            .flex_col()
            .gap(SPACING())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.focus_visible = false;
                    cx.notify();
                }),
            )
            .child(
                "We've detected that you're not running Corvane from the Applications folder of \
                 your machine. This could cause problems with the app, including impacting your \
                 ability to sign in.",
            )
            .child(
                "Do you want to move Corvane to the Applications folder now? This will also \
                 restart the app.",
            )
            .child(checkbox_row_focus(
                "move-to-applications-dont-ask",
                !ask_again,
                "Do not show this message again",
                self.focus_visible,
                move |checked, _, cx| {
                    weak.update(cx, |this, cx| {
                        this.ask_again = !checked;
                        cx.notify();
                    })
                    .ok();
                },
                cx,
            ));
        // `403-move-to-applications-backdrop-dismiss` (GHD:
        // `backdropDismissable={false}`)
        let backdrop_dismissable = corvane_core::AppState::try_global(cx).is_none_or(|s| {
            s.read(cx)
                .flags
                .bool(corvane_core::flags::ids::MOVE_TO_APPLICATIONS_BACKDROP_DISMISS)
        });
        dialog_with_kind_opts(
            "move-to-applications-folder",
            DialogKind::Warning,
            backdrop_dismissable,
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
