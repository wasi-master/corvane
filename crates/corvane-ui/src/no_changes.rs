//! `NoChanges` blankslate: "No local changes" + suggested-action cards.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, primary_button};

pub struct SuggestedAction {
    pub id: &'static str,
    pub title: SharedString,
    pub description: Option<SharedString>,
    pub hint: SharedString,
    pub button_label: SharedString,
    pub primary: bool,
}

fn card(action: SuggestedAction, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let (bg, border) = if action.primary {
        (
            t.primary_suggested_action_background,
            t.primary_suggested_action_border,
        )
    } else {
        (t.box_background, t.box_border)
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_DOUBLE)
        .p(SPACING_DOUBLE)
        .border_1()
        .border_color(border)
        .rounded(BORDER_RADIUS)
        .bg(bg)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(FONT_SIZE)
                        .child(action.title),
                )
                .when_some(action.description, |d, desc| {
                    d.child(div().text_size(FONT_SIZE).child(desc))
                })
                .child(
                    div()
                        .text_size(FONT_SIZE)
                        .text_color(t.text_secondary)
                        .child(action.hint),
                ),
        )
        .child(if action.primary {
            primary_button(action.id, action.button_label, cx).into_any_element()
        } else {
            button(action.id, action.button_label, cx).into_any_element()
        })
}

pub fn no_changes(actions: Vec<SuggestedAction>, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("no-changes")
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap(SPACING)
        .px(px(45.))
        .pt(px(45.))
        .bg(t.background)
        .child(
            div()
                .flex()
                .flex_col()
                .mb(SPACING)
                .child(
                    div()
                        .text_size(FONT_SIZE_LG)
                        .line_height(px(34.))
                        .font_weight(FontWeight::LIGHT)
                        .child("No local changes"),
                )
                .child(
                    div()
                        .text_size(FONT_SIZE)
                        .text_color(t.text_secondary)
                        .max_w(px(520.))
                        .child(
                            "There are no uncommitted changes in this repository. Here are some friendly suggestions for what to do next.",
                        ),
                ),
        )
        .children(actions.into_iter().map(|a| card(a, cx)))
}
