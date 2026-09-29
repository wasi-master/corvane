//! macOS title-bar strip (hiddenInset). 32 px on macOS 26; the traffic lights
//! are drawn by the system at the position set in `TitlebarOptions`.
//! `#desktop-app-title-bar` (darwin): gradient #3b3f46 → #2b2e33, 1 px black border.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::sizes::TITLE_BAR_HEIGHT;

pub fn title_bar(_cx: &App) -> impl IntoElement {
    div()
        .id("title-bar")
        .w_full()
        .h(TITLE_BAR_HEIGHT())
        .flex_none()
        .bg(linear_gradient(
            180.,
            linear_color_stop(rgb(0x3b3f46), 0.),
            linear_color_stop(rgb(0x2b2e33), 1.),
        ))
        .border_b_1()
        .border_color(rgb(0x000000))
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
        .on_click(|event, window, _| {
            if event.click_count() == 2 {
                window.titlebar_double_click();
            }
        })
}

/// `#desktop-app-title-bar.light-title-bar` (welcome flow and the
/// no-repositories blank slate on macOS): transparent, no border,
/// `position: fixed` over the content, still a window drag area.
pub fn light_title_bar() -> impl IntoElement {
    div()
        .id("light-title-bar")
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(TITLE_BAR_HEIGHT())
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
        .on_click(|event, window, _| {
            if event.click_count() == 2 {
                window.titlebar_double_click();
            }
        })
}
