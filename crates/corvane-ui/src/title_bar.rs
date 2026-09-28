//! macOS title-bar strip (hiddenInset). 32 px on macOS 26; the traffic lights
//! are drawn by the system at the position set in `TitlebarOptions`.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::TITLE_BAR_HEIGHT;

pub fn title_bar(cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("title-bar")
        .w_full()
        .h(TITLE_BAR_HEIGHT)
        .flex_none()
        .bg(t.toolbar_background)
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
        .on_click(|event, window, _| {
            if event.click_count() == 2 {
                window.titlebar_double_click();
            }
        })
}
