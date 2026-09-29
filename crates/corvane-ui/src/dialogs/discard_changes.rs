//! GHD `ui/discard-changes/discard-changes-dialog.tsx`: warning dialog that
//! lists the files (up to 10), the Trash hint and the "do not show again" opt-out.

use corvane_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::scrollbar::ScrollbarExt;
use crate::theme::MONO_FONT;
use crate::theme::sizes::*;
use crate::widgets::checkbox;

const MAX_FILES_TO_LIST: usize = 10;

pub struct DiscardChangesDialog {
    repo: u64,
    paths: Vec<String>,
    all: bool,
    dont_show_again: bool,
}

impl DiscardChangesDialog {
    pub fn new(repo: u64, paths: Vec<String>, all: bool) -> Self {
        Self {
            repo,
            paths,
            all,
            dont_show_again: false,
        }
    }
}

impl Render for DiscardChangesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (title, ok_label) = if self.all {
            ("Confirm Discard All Changes", "Discard All Changes")
        } else {
            ("Confirm Discard Changes", "Discard Changes")
        };
        let count = self.paths.len();
        let file_list = if count > MAX_FILES_TO_LIST {
            div().mb(SPACING).child(format!(
                "Are you sure you want to discard all {count} changed files?"
            ))
        } else {
            div()
                .child(
                    div()
                        .mb(SPACING)
                        .child("Are you sure you want to discard all changes to:"),
                )
                .child(
                    div()
                        .id("discard-file-list")
                        .max_h(px(175.))
                        .overflow_y_scroll()
                        .my(SPACING)
                        .flex()
                        .flex_col()
                        .children(
                            self.paths
                                .iter()
                                .map(|p| div().font_family(MONO_FONT).child(p.clone())),
                        )
                        .with_scrollbar(),
                )
        };
        let content = div()
            .flex()
            .flex_col()
            .child(file_list)
            .child(
                div()
                    .mb(SPACING)
                    .child("Changes can be restored by retrieving them from the Trash."),
            )
            .child(
                div()
                    .id("discard-dont-show")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_show_again = !this.dont_show_again;
                        cx.notify();
                    }))
                    .child(checkbox(
                        "discard-dont-show-checkbox",
                        self.dont_show_again,
                        false,
                        cx,
                    ))
                    .child("Do not show this message again"),
            );
        let repo = self.repo;
        let paths = self.paths.clone();
        let dont_show_again = self.dont_show_again;
        // GHD: with a destructive Ok, Cancel is the default (primary) button.
        dialog_with_kind(
            "dialog-discard-changes",
            DialogKind::Warning,
            title,
            content,
            vec![
                DialogButton {
                    id: "discard-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "discard-ok",
                    label: ok_label.into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_show_again {
                            Dispatcher::update_settings(cx, |s| s.confirm_discard_changes = false);
                        }
                        Dispatcher::discard_changes(repo, paths.clone(), cx);
                        Dispatcher::close_popup(cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
