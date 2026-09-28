//! Foldouts: panels anchored under toolbar buttons, closed by clicking the
//! overlay or Esc (`#foldout-container`, `styles/ui/_foldout.scss`).

use corvane_core::{Dispatcher, Foldout};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::repository_list::RepositoryFoldout;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

/// Renders the overlay + the open foldout panel, positioned below the toolbar.
/// `panel_x` / `panel_width` come from the toolbar button geometry.
pub fn foldout_layer(
    foldout: Foldout,
    panel_x: Pixels,
    panel_width: Pixels,
    repository_foldout: &Entity<RepositoryFoldout>,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let top = TITLE_BAR_HEIGHT + TOOLBAR_HEIGHT;
    let panel: AnyElement = match foldout {
        Foldout::Repository => repository_foldout.clone().into_any_element(),
        Foldout::Branch | Foldout::PushPull => div()
            .p(SPACING)
            .text_color(t.text_secondary)
            .child("Coming soon")
            .into_any_element(),
    };
    deferred(
        div()
            .id("foldout-container")
            .absolute()
            .top(top)
            .left_0()
            .right_0()
            .bottom_0()
            .child(
                // `.overlay`: click anywhere outside the panel closes it
                div()
                    .id("foldout-overlay")
                    .absolute()
                    .inset_0()
                    .bg(t.overlay)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| Dispatcher::close_foldout(cx)),
            )
            .child(
                // `.foldout`
                div()
                    .id("foldout")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(panel_x)
                    .w(panel_width)
                    .flex()
                    .flex_col()
                    .bg(t.background)
                    .text_color(t.text)
                    .border_r_1()
                    .border_color(t.box_border)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(panel),
            ),
    )
    .with_priority(10)
}
