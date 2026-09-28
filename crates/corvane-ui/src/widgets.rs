//! Small GHD-styled primitives: buttons, checkbox, counter badge, avatar, text box, kbd.

use gpui_kit::component::Sizable;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme};

/// `.button-component` - secondary button (25 px, radius 6).
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let t = cx.ghd();
    let hover_bg = t.secondary_button_hover_background;
    let hover_border = t.secondary_button_hover_border;
    base_button(id, t)
        .bg(t.secondary_button_background)
        .border_color(t.secondary_button_border)
        .text_color(t.secondary_button_text)
        .hover(move |s| s.bg(hover_bg).border_color(hover_border))
        .child(label.into())
}

/// `.button-component-primary` - blue primary button. Disabled = 60 % opacity.
pub fn primary_button(
    id: impl Into<ElementId>,
    label: impl IntoElement,
    disabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let hover_bg = t.button_hover_background;
    base_button(id, t)
        .bg(t.button_background)
        .border_color(t.button_background)
        .text_color(t.button_text)
        .when(disabled, |d| d.opacity(0.6).cursor_default())
        .when(!disabled, move |d| d.hover(move |s| s.bg(hover_bg)))
        .child(label)
}

fn base_button(id: impl Into<ElementId>, _t: &GhdTheme) -> Stateful<Div> {
    div()
        .id(id)
        .h(BUTTON_HEIGHT)
        .px(SPACING)
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .rounded(BORDER_RADIUS)
        .text_size(FONT_SIZE)
        .whitespace_nowrap()
        .cursor_pointer()
}

/// macOS-style 13 px checkbox. Checked = blue fill + white check; disabled =
/// grey fill + muted check (GHD's "0 changed files" state).
pub fn checkbox(
    id: impl Into<ElementId>,
    checked: bool,
    disabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let base = div()
        .id(id)
        .size(CHECKBOX_SIZE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .border_1()
        .when(!disabled, |d| d.cursor_pointer());
    match (checked, disabled) {
        (true, false) => base
            .bg(t.button_background)
            .border_color(t.button_background)
            .child(octicon(Octicon::Check, t.button_text).size(px(11.))),
        (true, true) => base
            .bg(t.box_selected_background)
            .border_color(t.box_selected_background)
            .child(octicon(Octicon::Check, t.text_secondary).size(px(11.))),
        (false, _) => base.bg(t.background).border_color(t.box_border_contrast),
    }
}

/// `.counter` pill used in the Changes tab and list rows.
pub fn counter(count: usize, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .ml(px(4.))
        .px(px(5.))
        .py(px(2.))
        .rounded(px(20.))
        .bg(t.tab_bar_count_background)
        .text_color(t.tab_bar_count_text)
        .text_size(FONT_SIZE_XS)
        .font_weight(FontWeight::SEMIBOLD)
        .line_height(px(11.))
        .child(count.to_string())
}

/// `kbd` - one key cap: radius 6, base border, 1/2 px padding, min 16 px tall,
/// min-width 1.5em, 2 px gap between caps (darwin).
pub fn kbd(key: impl Into<SharedString>, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .min_h(px(16.))
        .min_w(px(18.))
        .px(px(2.))
        .py(px(1.))
        .rounded(BORDER_RADIUS)
        .border_1()
        .border_color(t.box_border_contrast.opacity(0.5))
        .bg(t.box_background)
        .text_size(FONT_SIZE)
        .line_height(px(12.))
        .text_color(t.text)
        .child(key.into())
}

/// A row of key caps, e.g. `["⌘", "⇧", "A"]`.
pub fn kbd_group(keys: &[&'static str], cx: &App) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.))
        .children(keys.iter().map(|k| kbd(*k, cx)))
}

/// GHD `textboxish` chrome around a gpui-kit `Input`: 25 px, contrast border,
/// radius 6, `box_background`, 0/5 px padding, blue border + 1 px halo on focus.
pub fn text_box(
    id: impl Into<ElementId>,
    state: &Entity<InputState>,
    prefix: Option<Svg>,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let focused = state.read(cx).focus_handle(cx).is_focused(window);
    div()
        .id(id)
        .h(TEXT_FIELD_HEIGHT)
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF)
        .px(SPACING_HALF)
        .border_1()
        .rounded(BORDER_RADIUS)
        .bg(t.box_background)
        .border_color(if focused {
            t.focus
        } else {
            t.box_border_contrast
        })
        .when(focused, |d| {
            d.shadow(vec![BoxShadow {
                color: t.text_field_focus_shadow,
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
                inset: false,
            }])
        })
        .when_some(prefix, |d, icon| d.child(icon))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(Input::new(state).appearance(false).xsmall()),
        )
}

/// `TextBox` label above a field (`.text-box-component > label`, 3.33 px gap).
pub fn labeled(label: impl Into<SharedString>, field: impl IntoElement, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(SPACING_THIRD)
        .child(
            div()
                .text_size(FONT_SIZE)
                .text_color(t.text)
                .child(label.into()),
        )
        .child(field)
}

/// Round avatar placeholder (`.avatar`), 25 px unless overridden.
pub fn avatar_placeholder(size: Pixels, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .size(size)
        .flex_none()
        .rounded_full()
        .bg(t.box_alt_background)
        .border_1()
        .border_color(t.box_border)
}
