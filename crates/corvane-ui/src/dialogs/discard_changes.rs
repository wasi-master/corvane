//! GHD `ui/discard-changes/discard-changes-dialog.tsx`: warning dialog that
//! lists the files (up to 10), the Trash hint and the "do not show again"
//! opt-out - which Discard All Changes leaves out
//! (`showDiscardChangesSetting: false`); then Cancel holds focus.

use corvane_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogFrame, DialogKind, dialog_with_kind_framed};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::mono_font;
use crate::theme::sizes::*;
use crate::widgets::checkbox_row_focus;

const MAX_FILES_TO_LIST: usize = 10;

pub struct DiscardChangesDialog {
    repo: u64,
    paths: Vec<String>,
    all: bool,
    dont_show_again: bool,
    /// The autofocused checkbox's ring, until a mouse press.
    focus_visible: bool,
}

impl DiscardChangesDialog {
    pub fn new(repo: u64, paths: Vec<String>, all: bool) -> Self {
        Self {
            repo,
            paths,
            all,
            dont_show_again: false,
            focus_visible: true,
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
        let t = cx.ghd();
        let all = self.all;
        let dont_show = self.dont_show_again;
        let focus_visible = self.focus_visible;
        let weak = cx.weak_entity();
        let count = self.paths.len();
        let file_list = if count > MAX_FILES_TO_LIST {
            div().mb(SPACING()).child(format!(
                "Are you sure you want to discard all {count} changed files?"
            ))
        } else {
            // a flex column: block layout would collapse the paragraph's and
            // the list's margins, which GHD keeps apart (flow inside a <div>
            // whose <ul> has its own 10 px margins)
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .mb(SPACING())
                        .child("Are you sure you want to discard all changes to:"),
                )
                .child(
                    div()
                        .id("discard-file-list")
                        .max_h(zpx(175.))
                        .overflow_y_scroll()
                        .my(SPACING())
                        .flex()
                        .flex_col()
                        .children(self.paths.iter().map(|p| {
                            // `PathText`: the directory in the secondary colour
                            let (dir, name) = match p.rfind('/') {
                                Some(i) => (&p[..=i], &p[i + 1..]),
                                None => ("", p.as_str()),
                            };
                            div()
                                .flex()
                                .flex_row()
                                .line_height(zpx(18.))
                                .font_family(mono_font())
                                .when(!dir.is_empty(), |d| {
                                    d.child(
                                        div().text_color(t.text_secondary).child(dir.to_string()),
                                    )
                                })
                                .child(name.to_string())
                        }))
                        .with_scrollbar(),
                )
        };
        let content = div()
            .flex()
            .flex_col()
            .child(file_list)
            .child(
                div()
                    .when(!all, |d| d.mb(SPACING()))
                    .child("Changes can be restored by retrieving them from the Trash."),
            )
            .when(!all, |d| {
                d.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.focus_visible = false;
                        cx.notify();
                    }),
                )
                .child(checkbox_row_focus(
                    "discard-dont-show",
                    dont_show,
                    "Do not show this message again",
                    focus_visible,
                    move |value, _, cx| {
                        weak.update(cx, |this, cx| {
                            this.dont_show_again = value;
                            cx.notify();
                        })
                        .ok();
                    },
                    cx,
                ))
            });
        let repo = self.repo;
        let paths = self.paths.clone();
        let dont_show_again = self.dont_show_again;
        // GHD: with a destructive Ok, Cancel is the default (primary) button.
        dialog_with_kind_framed(
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
            DialogFrame {
                focus_primary: all,
                ..DialogFrame::default()
            },
            close,
            window,
            cx,
        )
    }
}
