//! Changes sidebar: filter header, "N changed files" row, file list, commit form.
//! `styles/ui/changes/{_changes-list,_commit-message}.scss`.

use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{avatar_placeholder, checkbox, primary_button};

pub struct ChangesSidebar {
    filter: Entity<InputState>,
    summary: Entity<InputState>,
    description: Entity<TextareaState>,
    branch_name: SharedString,
    changed_files: usize,
}

impl ChangesSidebar {
    pub fn new(branch_name: SharedString, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        let summary = cx.new(|cx| InputState::new(window, cx).placeholder("Summary (required)"));
        let description = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder("Description")
        });
        Self {
            filter,
            summary,
            description,
            branch_name,
            changed_files: 0,
        }
    }

    fn header(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .px(SPACING)
            .py(SPACING_HALF)
            .bg(t.box_alt_background)
            .border_b_1()
            .border_color(t.box_border)
            .child(
                // Filter row: [Filter Options ▾] [Filter…]
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(TEXT_FIELD_HEIGHT)
                    .child(
                        div()
                            .id("filter-options")
                            .h(TEXT_FIELD_HEIGHT)
                            .w(px(48.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(2.))
                            .border_1()
                            .border_color(t.secondary_button_border)
                            .rounded_l(BORDER_RADIUS)
                            .bg(t.secondary_button_background)
                            .text_color(t.secondary_button_text)
                            .cursor_pointer()
                            .child(octicon(Octicon::Filter, t.secondary_button_text))
                            .child(
                                octicon(Octicon::TriangleDown, t.secondary_button_text)
                                    .size(px(12.)),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.filter).h(TEXT_FIELD_HEIGHT)),
                    ),
            )
            .child(
                // "☑ N changed files"
                div()
                    .h(ROW_HEIGHT)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .child(checkbox("check-all", self.changed_files > 0, cx))
                    .child(
                        div()
                            .text_size(FONT_SIZE)
                            .truncate()
                            .child(format!("{} changed files", self.changed_files)),
                    ),
            )
    }

    fn list(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div().flex_1().min_h(px(100.)).bg(t.background)
    }

    fn commit_form(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .id("commit-message")
            .key_context("CommitMessage")
            .flex_none()
            .flex()
            .flex_col()
            .p(SPACING)
            .bg(t.box_alt_background)
            .border_t_1()
            .border_color(t.box_border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .mb(SPACING)
                    .child(avatar_placeholder(AVATAR_SIZE, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.summary).h(TEXT_FIELD_HEIGHT)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .mb(SPACING)
                    .border_1()
                    .border_color(t.box_border_contrast)
                    .rounded(BORDER_RADIUS)
                    .bg(t.background)
                    .overflow_hidden()
                    .child(Textarea::new(&self.description).bordered(false).h(px(80.)))
                    .child(
                        // action bar: add co-authors | gear
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF)
                            .px(SPACING)
                            .pb(px(8.))
                            .text_color(t.text_secondary)
                            .child(octicon(Octicon::PersonAdd, t.text_secondary))
                            .child(div().w(px(1.)).h(px(16.)).bg(t.box_border_contrast))
                            .child(octicon(Octicon::Gear, t.text_secondary)),
                    ),
            )
            .child(
                primary_button(
                    "commit",
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.))
                        .child("Commit to")
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.branch_name.clone()),
                        ),
                    cx,
                )
                .w_full(),
            )
    }
}

impl Render for ChangesSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(self.header(cx))
            .child(self.list(cx))
            .child(self.commit_form(cx))
    }
}
