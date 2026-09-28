//! Modal dialog chrome (`styles/ui/_dialog.scss`): overlay, 400–600 px box,
//! 50 px header with close button, 20 px padded content, footer buttons.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

pub type ClickHandler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

pub struct DialogButton {
    pub id: &'static str,
    pub label: SharedString,
    pub primary: bool,
    pub on_click: ClickHandler,
}

pub fn dialog(
    id: &'static str,
    title: impl Into<SharedString>,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let close_for_overlay = on_close.clone();
    let viewport = window.viewport_size();
    deferred(
        anchored().position(point(px(0.), px(0.))).child(
            div()
                .id(id)
                .w(viewport.width)
                .h(viewport.height)
                .flex()
                .items_center()
                .justify_center()
                .bg(t.overlay)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    close_for_overlay(window, cx)
                })
                .child(
                    div()
                        .id("dialog-box")
                        .min_w(px(400.))
                        .max_w(px(600.))
                        .flex()
                        .flex_col()
                        .rounded(BORDER_RADIUS)
                        .bg(t.background)
                        .text_color(t.text)
                        .border_1()
                        .border_color(t.box_border)
                        .shadow(vec![BoxShadow {
                            color: t.shadow,
                            offset: point(px(0.), px(2.)),
                            blur_radius: px(7.),
                            spread_radius: px(0.),
                            inset: false,
                        }])
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            // header
                            div()
                                .h(px(50.))
                                .flex_none()
                                .flex()
                                .flex_row()
                                .items_center()
                                .px(SPACING_DOUBLE)
                                .border_b_1()
                                .border_color(t.box_border)
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(FONT_SIZE_MD)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title.into()),
                                )
                                .child({
                                    let on_close = on_close.clone();
                                    div()
                                        .id("dialog-close")
                                        .size(px(16.))
                                        .cursor_pointer()
                                        .on_click(move |_, window, cx| on_close(window, cx))
                                        .child(octicon(Octicon::X, t.text_secondary))
                                }),
                        )
                        .child(
                            div()
                                .p(SPACING_DOUBLE)
                                .text_size(FONT_SIZE)
                                .line_height(px(18.))
                                .child(content),
                        )
                        .child(
                            // footer
                            div()
                                .flex_none()
                                .flex()
                                .flex_row()
                                .justify_end()
                                .gap(SPACING)
                                .px(SPACING_DOUBLE)
                                .pb(SPACING_DOUBLE)
                                .children(buttons.into_iter().map(|b| {
                                    let on_click = b.on_click;
                                    if b.primary {
                                        crate::widgets::primary_button(b.id, b.label, false, cx)
                                            .min_w(px(120.))
                                            .on_click(move |_, window, cx| on_click(window, cx))
                                            .into_any_element()
                                    } else {
                                        crate::widgets::button(b.id, b.label, cx)
                                            .min_w(px(120.))
                                            .on_click(move |_, window, cx| on_click(window, cx))
                                            .into_any_element()
                                    }
                                })),
                        ),
                ),
        ),
    )
    .with_priority(20)
}
