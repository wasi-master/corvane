//! GHD `ui/discard-changes/discard-selection-dialog.tsx`: confirm before the
//! lines picked from the diff gutter menu are reverted in the working copy.

use corvane_core::{DiffSelection, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::theme::mono_font;
use crate::theme::sizes::*;
use crate::widgets::checkbox;

pub struct DiscardSelectionDialog {
    repo: u64,
    path: String,
    selection: DiffSelection,
    dont_show_again: bool,
}

impl DiscardSelectionDialog {
    pub fn new(repo: u64, path: String, selection: DiffSelection) -> Self {
        Self {
            repo,
            path,
            selection,
            dont_show_again: false,
        }
    }
}

impl Render for DiscardSelectionDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let content = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(SPACING())
                    .child("Are you sure you want to discard the selected changes to:"),
            )
            .child(
                div()
                    .mb(SPACING())
                    .font_family(mono_font())
                    .child(self.path.clone()),
            )
            .child(
                div()
                    .id("discard-selection-dont-show")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_show_again = !this.dont_show_again;
                        cx.notify();
                    }))
                    .child(checkbox(
                        "discard-selection-dont-show-checkbox",
                        self.dont_show_again,
                        false,
                        cx,
                    ))
                    .child("Do not show this message again"),
            );
        let repo = self.repo;
        let path = self.path.clone();
        let selection = self.selection.clone();
        let dont_show_again = self.dont_show_again;
        dialog_with_kind(
            "dialog-discard-selection",
            DialogKind::Warning,
            mac_or("Confirm Discard Changes", "Confirm discard changes"),
            content,
            vec![
                DialogButton {
                    id: "discard-selection-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "discard-selection-ok",
                    label: mac_or("Discard Changes", "Discard changes").into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_show_again {
                            Dispatcher::update_settings(cx, |s| s.confirm_discard_changes = false);
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::discard_selection(repo, path.clone(), selection.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
