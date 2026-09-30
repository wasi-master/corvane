//! `NoChanges` blankslate (`.changes-interstitial`): "No local changes" header
//! with the paper-stack illustration, then suggested-action groups.
//! `styles/ui/changes/_changes-interstitial.scss`, `ui/suggested-actions/*.scss`.
//!
//! Deviation (GHD `ui/changes/no-changes.tsx`): while the branch has an open
//! pull request a primary "View Pull Request" card leads the list (built in
//! `workspace.rs`, `725-no-changes-view-pull-request`); GHD shows no
//! remote action then.
//! Deviation (`726-restore-stash-suggestion`): with a stash on the branch the
//! first card is "Restore your stashed changes" with a primary Restore button
//! (built in `workspace.rs`; GHD `no-changes.tsx` offers only "View stash").

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, kbd_group, primary_button};

pub struct SuggestedAction {
    pub id: &'static str,
    pub on_click: crate::widgets::ClickAction,
    pub title: SharedString,
    pub description: Option<SharedString>,
    /// `p.discoverability` text before the key caps, e.g. "Repository menu or".
    pub hint: SharedString,
    /// Key caps rendered as `kbd` elements, e.g. `["⌘", "⇧", "A"]`.
    pub keys: &'static [&'static str],
    pub button_label: SharedString,
    pub primary: bool,
}

/// `.suggested-action`: base border, 20 px padding, row; primary variant tinted.
pub fn suggested_action_card(action: SuggestedAction, cx: &App) -> impl IntoElement {
    card(action, cx)
}

fn card(action: SuggestedAction, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let (bg, border) = if action.primary {
        (
            t.primary_suggested_action_background,
            t.primary_suggested_action_border,
        )
    } else {
        (t.background, t.box_border)
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .p(SPACING_DOUBLE())
        .border_1()
        .border_color(border)
        .rounded(BORDER_RADIUS())
        .bg(bg)
        .child(
            // `.text-wrapper`
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .mr(SPACING_DOUBLE())
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(FONT_SIZE())
                        .line_height(zpx(18.))
                        .child(action.title),
                )
                .when_some(action.description, |d, desc| {
                    d.child(
                        div()
                            .text_size(FONT_SIZE())
                            .line_height(zpx(18.))
                            .mb(SPACING_HALF())
                            .child(desc),
                    )
                })
                .when(!action.hint.is_empty() || !action.keys.is_empty(), |d| {
                    d.child(
                        // `p.discoverability`
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(zpx(4.))
                            .text_size(FONT_SIZE())
                            .line_height(zpx(18.))
                            .text_color(t.text_secondary)
                            .child(action.hint)
                            .child(kbd_group(action.keys, cx)),
                    )
                }),
        )
        .child({
            let on_click = action.on_click.clone();
            if action.primary {
                primary_button(action.id, action.button_label, false, cx)
                    .on_click(move |_, window, cx| on_click(window, cx))
                    .into_any_element()
            } else {
                button(action.id, action.button_label, cx)
                    .on_click(move |_, window, cx| on_click(window, cx))
                    .into_any_element()
            }
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
        .items_center()
        .p(zpx(40.))
        .bg(t.background)
        .child(
            // `.content`: full width, max 600 px, centred
            div()
                .w_full()
                .max_w(zpx(600.))
                .flex()
                .flex_col()
                .gap(SPACING())
                .child(
                    // `.interstitial-header`: text left, image bottom-aligned right
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .mb(SPACING())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .mr(SPACING_DOUBLE())
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(zpx(32.))
                                        .line_height(zpx(38.))
                                        .font_weight(FontWeight::LIGHT)
                                        .child("No local changes"),
                                )
                                .child(
                                    div()
                                        .text_size(FONT_SIZE())
                                        .line_height(zpx(18.))
                                        .text_color(t.text)
                                        .child(
                                            "There are no uncommitted changes in this repository. Here are some friendly suggestions for what to do next.",
                                        ),
                                ),
                        )
                        .child(
                            crate::widgets::blankslate_image("paper-stack.svg", cx)
                                .w(zpx(73.))
                                .h(zpx(70.))
                                .flex_none(),
                        ),
                )
                .children(actions.into_iter().map(|a| card(a, cx))),
        )
}

/// GHD `MultipleSelection` (`.panel.blankslate`): "N files selected".
pub fn multiple_selection(count: usize, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("multiple-selection")
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(SPACING_DOUBLE())
        .bg(t.background)
        .text_color(t.text_secondary)
        .text_size(FONT_SIZE())
        .child(
            crate::widgets::blankslate_image("multiple-files-selected.svg", cx)
                .w(zpx(200.))
                .h(zpx(120.)),
        )
        .child(format!("{count} files selected"))
}
