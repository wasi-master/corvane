//! Modal dialog chrome (`styles/ui/_dialog.scss`): overlay, 400–600 px box,
//! 50 px header with close button, 20 px padded content, footer buttons.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme};

pub type ClickHandler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

/// GHD `Dialog type`: warning/error dialogs show a 24 px icon left of the content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Normal,
    Warning,
    Error,
}

pub struct DialogButton {
    pub id: &'static str,
    pub label: SharedString,
    pub primary: bool,
    /// GHD `okButtonDisabled`: 60 % opacity, clicks ignored.
    pub disabled: bool,
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
    dialog_with_kind(
        id,
        DialogKind::Normal,
        title,
        content,
        buttons,
        on_close,
        window,
        cx,
    )
}

/// `.dialog.warning` / `.dialog.error`: content gets `margin-left: 20px` and
/// `padding-left: 20px + 24px icon`, so the text starts 64 px from the edge.
fn dialog_content(kind: DialogKind, content: impl IntoElement, t: &GhdTheme) -> Div {
    let base = div()
        .p(SPACING_DOUBLE)
        .text_size(FONT_SIZE)
        .line_height(px(18.));
    match kind {
        DialogKind::Normal => base.child(content),
        DialogKind::Warning | DialogKind::Error => {
            let color = if kind == DialogKind::Warning {
                t.dialog_warning
            } else {
                t.dialog_error
            };
            base.flex()
                .flex_row()
                .items_start()
                .gap(SPACING_DOUBLE)
                .child(
                    octicon(Octicon::Alert, color)
                        .size(px(24.))
                        .flex_none()
                        .mt(px(5.)),
                )
                .child(div().flex_1().min_w_0().child(content))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn dialog_with_kind(
    id: &'static str,
    kind: DialogKind,
    title: impl Into<SharedString>,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let title: SharedString = title.into();
    dialog_impl(
        id,
        kind,
        div().child(title).into_any_element(),
        content,
        buttons,
        on_close,
        window,
        cx,
    )
}

/// A dialog whose title is an element (bold branch names inside the title).
#[allow(clippy::too_many_arguments)]
pub fn dialog_with_title_element(
    id: &'static str,
    title: impl IntoElement,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    dialog_impl(
        id,
        DialogKind::Normal,
        title.into_any_element(),
        content,
        buttons,
        on_close,
        window,
        cx,
    )
}

#[allow(clippy::too_many_arguments)]
fn dialog_impl(
    id: &'static str,
    kind: DialogKind,
    title: AnyElement,
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
                                        .child(title),
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
                        .child(dialog_content(kind, content, t))
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
                                    let disabled = b.disabled;
                                    if b.primary {
                                        crate::widgets::primary_button(b.id, b.label, disabled, cx)
                                            .min_w(px(120.))
                                            .when(!disabled, |d| {
                                                d.on_click(move |_, window, cx| {
                                                    on_click(window, cx)
                                                })
                                            })
                                            .into_any_element()
                                    } else {
                                        crate::widgets::button(b.id, b.label, cx)
                                            .min_w(px(120.))
                                            .when(disabled, |d| d.opacity(0.6).cursor_default())
                                            .when(!disabled, |d| {
                                                d.on_click(move |_, window, cx| {
                                                    on_click(window, cx)
                                                })
                                            })
                                            .into_any_element()
                                    }
                                })),
                        ),
                ),
        ),
    )
    .with_priority(20)
}
