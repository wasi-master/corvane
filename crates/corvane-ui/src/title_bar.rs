//! macOS title-bar strip (hiddenInset). 32 px on macOS 26; the traffic lights
//! are drawn by the system at the position set in `TitlebarOptions`.
//! `#desktop-app-title-bar` (darwin): gradient #3b3f46 → #2b2e33, 1 px black border.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::TITLE_BAR_HEIGHT;

pub fn title_bar(cx: &App) -> impl IntoElement {
    // flag `109-light-toolbar`: a light gradient in the Light theme
    let (top, bottom, border) = if cx.ghd().light_title_bar {
        (0xf6f8fa, 0xeaeef2, 0xd0d7de)
    } else {
        (0x3b3f46, 0x2b2e33, 0x000000)
    };
    div()
        .id("title-bar")
        .w_full()
        .h(TITLE_BAR_HEIGHT())
        .flex_none()
        .bg(linear_gradient(
            180.,
            linear_color_stop(rgb(top), 0.),
            linear_color_stop(rgb(bottom), 1.),
        ))
        .border_b_1()
        .border_color(rgb(border))
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

/// The window icon off macOS: the 256 px app icon.
#[cfg(not(target_os = "macos"))]
pub fn window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    let png = include_bytes!("../../../assets/icon/Corvane-256.png");
    match image::load_from_memory_with_format(png, image::ImageFormat::Png) {
        Ok(icon) => Some(std::sync::Arc::new(icon.into_rgba8())),
        Err(err) => {
            tracing::warn!(%err, "could not decode the window icon");
            None
        }
    }
}
