//! `#cloning-repository-view`: shown in the content area while `git clone` runs.

use corvane_core::CloneState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

pub fn cloning_view(clone: &CloneState, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let fraction = clone.value.unwrap_or(0.0).clamp(0.0, 1.0);
    div()
        .id("cloning-repository-view")
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(t.background)
        .child(
            div()
                .w(px(600.))
                .mt(px(-60.))
                .p(SPACING_DOUBLE)
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .mb(SPACING)
                        .child(
                            octicon(Octicon::DesktopDownload, t.text)
                                .size(px(32.))
                                .mr(SPACING)
                                .mt(px(4.)),
                        )
                        .child(
                            div()
                                .text_size(px(32.))
                                .font_weight(FontWeight::LIGHT)
                                .truncate()
                                .child(format!("Cloning {}", clone.path.display())),
                        ),
                )
                .child(
                    // `progress`: 10 px track, text-colour fill
                    div()
                        .w_full()
                        .h(px(10.))
                        .rounded(px(5.))
                        .bg(t.box_alt_background)
                        .overflow_hidden()
                        .child(div().h_full().w(relative(fraction)).bg(t.text)),
                )
                .child(
                    div()
                        .mt(SPACING)
                        .text_size(FONT_SIZE)
                        .text_color(t.text_secondary)
                        .truncate()
                        .child(clone.description.clone()),
                ),
        )
}
