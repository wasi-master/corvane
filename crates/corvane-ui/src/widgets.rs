//! Small GHD-styled primitives: buttons, checkbox, counter badge, avatar, text box, kbd.

use gpui_kit::component::Sizable;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::native_menu::NativeMenu;
use gpui_kit::prelude::*;
use gpui_kit::*;

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

/// `.button-component.small-button` - the secondary button at 21 px, 11 px
/// text, 5 px side padding (Undo in the changes sidebar, list row actions).
pub fn small_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> Stateful<Div> {
    button(id, label, cx)
        .h(zpx(21.))
        .px(SPACING_HALF())
        .text_size(FONT_SIZE_SM())
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
        .h(BUTTON_HEIGHT())
        .px(SPACING())
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .rounded(BORDER_RADIUS())
        .text_size(FONT_SIZE())
        .whitespace_nowrap()
        .cursor_pointer()
}

/// GHD `LinkButton`: link-coloured inline text, underlined on hover.
pub fn link_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let hover = t.link_hover;
    // Settings › Accessibility › Underline links (`body.underline-links`).
    let underline =
        corvane_core::AppState::try_global(cx).is_some_and(|s| s.read(cx).settings.underline_links);
    let label: SharedString = label.into();
    div()
        .id(id)
        // a `Link` node VoiceOver can name and press
        .role(Role::Link)
        .aria_label(label.clone())
        .flex_none()
        .text_size(FONT_SIZE())
        .text_color(t.link)
        .cursor_pointer()
        .when(underline, |d| d.underline())
        .hover(move |s| s.text_color(hover).underline())
        .child(label)
}

/// Shared handler for `select_button` choices (index of the picked option).
pub type SelectHandler = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>;
/// Shared click handler.
pub type ClickAction = std::rc::Rc<dyn Fn(&mut Window, &mut App)>;

/// One piece of an inline paragraph.
pub enum Inline {
    Text(SharedString),
    Element(AnyElement),
}

impl From<&'static str> for Inline {
    fn from(s: &'static str) -> Self {
        Inline::Text(s.into())
    }
}

impl From<String> for Inline {
    fn from(s: String) -> Self {
        Inline::Text(s.into())
    }
}

impl From<AnyElement> for Inline {
    fn from(el: AnyElement) -> Self {
        Inline::Element(el)
    }
}

/// GHD `<p>` mixing text with `<LinkButton>` / `<Ref>` children. A flex-row
/// text child never shrinks in GPUI, so long sentences would overflow;
/// splitting the text into words gives real line wrapping around the inline
/// elements. Text that touches an element with no space (`"(" + chip`,
/// `chip + "."`) stays attached.
pub fn paragraph(parts: Vec<Inline>) -> Div {
    const GAP: f32 = 3.;
    let mut row = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(zpx(GAP))
        .line_height(zpx(18.));
    let mut attach_next = false;
    for part in parts {
        match part {
            Inline::Text(text) => {
                let starts_attached = !text.starts_with(char::is_whitespace);
                let ends_attached = !text.ends_with(char::is_whitespace);
                let mut first = true;
                for word in text.split_whitespace() {
                    let attach = first && starts_attached && attach_next;
                    row = row.child(
                        div()
                            .when(attach, |d| d.ml(zpx(-GAP)))
                            .child(SharedString::from(word.to_string())),
                    );
                    first = false;
                }
                if !first {
                    attach_next = ends_attached;
                }
            }
            Inline::Element(el) => {
                row = row.child(div().when(attach_next, |d| d.ml(zpx(-GAP))).child(el));
                attach_next = true;
            }
        }
    }
    row
}

/// `<Ref>`: inline monospace code on the alt background.
pub fn code_ref(text: impl Into<SharedString>, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .font_family(crate::theme::mono_font())
        .px(zpx(3.))
        .rounded(zpx(3.))
        .bg(t.box_alt_background)
        .child(text.into())
}

/// Dialog `h2` (`_dialog.scss`: 14 px semibold, 10 px below).
pub fn section_heading(text: impl Into<SharedString>, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .text_size(FONT_SIZE_MD())
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.text)
        .mb(SPACING())
        .child(text.into())
}

/// `.settings-description`: 11 px secondary text, 10 px above.
pub fn settings_description(cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .mt(SPACING())
        .text_size(FONT_SIZE_SM())
        .line_height(zpx(16.))
        .text_color(t.text_secondary)
}

/// `Checkbox` with its label (`.checkbox-component`): 13 px box, 5 px gap.
pub fn checkbox_row(
    id: &'static str,
    checked: bool,
    label: impl IntoElement,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        .cursor_pointer()
        .on_click(move |_, window, cx| on_toggle(!checked, window, cx))
        .child(checkbox(
            ElementId::from(SharedString::from(format!("{id}-box"))),
            checked,
            false,
            cx,
        ))
        .child(div().flex_1().min_w_0().text_size(FONT_SIZE()).child(label))
}

/// Chromium's native `<input type="radio">` with GHD's `accent-color`:
/// 13 px circle, accent fill with a 5 px dot when selected.
pub fn radio(id: impl Into<ElementId>, selected: bool, cx: &App) -> Stateful<Div> {
    let t = cx.ghd();
    div()
        .id(id)
        .size(zpx(13.))
        .flex_none()
        .rounded_full()
        .border_1()
        .border_color(if selected { t.accent } else { t.control_border })
        .bg(if selected {
            t.accent
        } else {
            t.control_background
        })
        .flex()
        .items_center()
        .justify_center()
        .when(selected, |d| {
            d.child(div().size(zpx(5.)).rounded_full().bg(t.control_background))
        })
}

/// `RadioButton` row: radio + label, 5 px apart; rows 5 px apart.
pub fn radio_row(
    id: &'static str,
    selected: bool,
    label: impl IntoElement,
    on_select: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        .cursor_pointer()
        .on_click(move |_, window, cx| on_select(window, cx))
        .child(radio(
            ElementId::from(SharedString::from(format!("{id}-radio"))),
            selected,
            cx,
        ))
        .child(div().flex_1().min_w_0().text_size(FONT_SIZE()).child(label))
}

/// An entry of a `select_button_items` popup.
pub enum SelectItem {
    Option(SharedString),
    /// `<option disabled>────</option>`: a native menu separator.
    Separator,
}

/// Native `<select>` as macOS renders it in GHD: a textboxish 25 px popup
/// button showing the current value with a ▾ caret; clicking opens a native
/// menu of `options` and calls `on_select(index)`.
pub fn select_button(
    id: impl Into<ElementId>,
    value: impl Into<SharedString>,
    options: Vec<SharedString>,
    selected: Option<usize>,
    disabled: bool,
    on_select: SelectHandler,
    cx: &App,
) -> Stateful<Div> {
    select_button_items(
        id,
        value,
        options.into_iter().map(SelectItem::Option).collect(),
        selected,
        disabled,
        on_select,
        cx,
    )
}

/// `select_button` with separators; `on_select` gets the index among the
/// `Option` items only.
pub fn select_button_items(
    id: impl Into<ElementId>,
    value: impl Into<SharedString>,
    items: Vec<SelectItem>,
    selected: Option<usize>,
    disabled: bool,
    on_select: SelectHandler,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let hover_bg = t.secondary_button_hover_background;
    div()
        .id(id)
        .h(TEXT_FIELD_HEIGHT())
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(SPACING_HALF())
        .pl(SPACING_HALF())
        .pr(zpx(4.))
        .border_1()
        .rounded(BORDER_RADIUS())
        .bg(t.box_background)
        .border_color(t.box_border_contrast)
        .text_size(FONT_SIZE())
        .text_color(t.text)
        .when(disabled, |d| d.opacity(0.6))
        .when(!disabled, |d| {
            d.cursor_pointer().hover(move |s| s.bg(hover_bg)).on_click(
                move |ev: &ClickEvent, window, cx| {
                    let mut option_ix = 0;
                    let menu_items: Vec<crate::context_menu::MenuItem> = items
                        .iter()
                        .map(|item| match item {
                            SelectItem::Separator => crate::context_menu::MenuItem::separator(),
                            SelectItem::Option(label) => {
                                let on_select = on_select.clone();
                                let ix = option_ix;
                                option_ix += 1;
                                crate::context_menu::MenuItem::checkbox(
                                    label.clone(),
                                    selected == Some(ix),
                                    move |window, cx| on_select(ix, window, cx),
                                )
                            }
                        })
                        .collect();
                    let position = ev.mouse_position().unwrap_or_default();
                    #[cfg(target_os = "macos")]
                    crate::native_menu::show_context_menu(menu_items, position, window, cx);
                    #[cfg(not(target_os = "macos"))]
                    let _ = (menu_items, position, window, cx);
                },
            )
        })
        .child(div().flex_1().min_w_0().truncate().child(value.into()))
        .child(
            crate::icons::octicon(crate::icons::Octicon::TriangleDown, t.text_secondary)
                .size(zpx(12.)),
        )
}

/// GHD `DialogError` (`.dialog-banner.dialog-error`): a full-width red band
/// under the dialog header. It cancels the content padding itself so it can
/// be the first child of any dialog content.
pub fn dialog_error_banner(message: impl Into<SharedString>, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .mx(zpx(-20.))
        .mt(zpx(-20.))
        .mb(SPACING_DOUBLE())
        .px(SPACING_DOUBLE())
        .py(SPACING())
        .bg(t.form_error_background)
        .border_t_1()
        .border_b_1()
        .border_color(t.form_error_border)
        .text_color(t.form_error_text)
        .text_size(FONT_SIZE())
        .line_height(zpx(18.))
        .child(message.into())
}

/// GHD `CallToAction`: text on the left, a ≥120 px primary button on the right.
pub fn call_to_action(
    id: &'static str,
    body: impl IntoElement,
    action_title: impl Into<SharedString>,
    on_action: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(SPACING())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(FONT_SIZE())
                .line_height(zpx(18.))
                .child(body),
        )
        .child(
            primary_button(id, action_title.into(), false, cx)
                .min_w(zpx(120.))
                .flex_none()
                .on_click(move |_, window, cx| on_action(window, cx)),
        )
}

/// 13 px checkbox. GHD renders a bare `<input type="checkbox">`, so this is
/// Chromium's native control (`ui/native_theme/native_theme_base.cc`
/// `PaintCheckbox`) with GHD's `accent-color` (`_globals.scss` `body`):
/// 2 px radius, 1 px border; checked = `accent` fill with the check drawn in
/// the control background colour; disabled = grey fill (GHD's "0 changed
/// files" state).
pub fn checkbox(
    id: impl Into<ElementId>,
    checked: bool,
    disabled: bool,
    cx: &App,
) -> Stateful<Div> {
    checkbox_tristate(id, Some(checked), disabled, cx)
}

/// `CheckboxValue::Mixed` (`None`) draws the accent fill with a dash, like
/// Chromium's indeterminate checkbox that GHD renders.
///
/// Hover/pressed tints of Chromium's native control are not reproduced.
pub fn checkbox_tristate(
    id: impl Into<ElementId>,
    value: Option<bool>,
    disabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let base = div()
        .id(id)
        .size(CHECKBOX_SIZE())
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(zpx(2.))
        .border_1()
        .when(!disabled, |d| d.cursor_pointer());
    let (fill, glyph) = match (value, disabled) {
        (Some(false), false) => {
            return base.bg(t.control_background).border_color(t.control_border);
        }
        (Some(false), true) => {
            return base
                .bg(t.control_disabled_background)
                .border_color(t.control_disabled_border);
        }
        (_, false) => (t.accent, t.control_background),
        (_, true) => (t.control_disabled_accent, t.control_disabled_glyph),
    };
    let path = if value.is_none() {
        "controls/checkbox-dash-13.svg"
    } else {
        "controls/checkbox-check-13.svg"
    };
    base.bg(fill).border_color(fill).child(
        svg()
            .path(path)
            .size(CHECKBOX_SIZE())
            .flex_none()
            .text_color(glyph),
    )
}

/// `.counter` pill used in the Changes tab and list rows.
pub fn counter(count: usize, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .ml(zpx(4.))
        .px(zpx(5.))
        .py(zpx(2.))
        .rounded(zpx(20.))
        .bg(t.tab_bar_count_background)
        .text_color(t.tab_bar_count_text)
        .text_size(FONT_SIZE_XS())
        .font_weight(FontWeight::SEMIBOLD)
        .line_height(zpx(11.))
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
        .min_h(zpx(16.))
        .min_w(zpx(18.))
        .px(zpx(2.))
        .py(zpx(1.))
        .rounded(BORDER_RADIUS())
        .border_1()
        .border_color(t.box_border_contrast.opacity(0.5))
        .bg(t.box_background)
        .text_size(FONT_SIZE())
        .line_height(zpx(12.))
        .text_color(t.text)
        .child(key.into())
}

/// A row of key caps, e.g. `["⌘", "⇧", "A"]`.
pub fn kbd_group(keys: &[&'static str], cx: &App) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(zpx(2.))
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
    text_box_with_menu(id, state, prefix, None, window, cx)
}

/// GHD `TextBox` with `displayClearButton` (filter lists, the changes
/// filter, compare, diff search): the text stops 25 px before the end and a
/// 25 px ✕ button (`button.clear-button`, `--text-color`) clears the value
/// while there is one.
pub fn filter_text_box(
    id: impl Into<ElementId>,
    state: &Entity<InputState>,
    prefix: Option<Svg>,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let has_value = !state.read(cx).value().is_empty();
    let clear_state = state.clone();
    text_box_with_menu(id, state, prefix, None, window, cx)
        .relative()
        // `padding-inline-end: var(--text-field-height)`, less the kit's 4 px
        .pr(TEXT_FIELD_HEIGHT() - zpx(4.))
        .when(has_value, |d| {
            d.child(
                div()
                    .id("clear-button")
                    .icon_button_label("Clear")
                    .absolute()
                    .right(zpx(0.))
                    .top(zpx(-1.))
                    .size(TEXT_FIELD_HEIGHT())
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(crate::icons::octicon(crate::icons::Octicon::X, t.text))
                    .on_click(move |_, window, cx| {
                        clear_state.update(cx, |input, cx| input.set_value("", window, cx));
                    }),
            )
        })
}

/// A custom right-click menu for an input (the kit's native edit menu is
/// replaced wholesale, so builders add Cut/Copy/Paste themselves).
pub type InputMenuBuilder = std::rc::Rc<dyn Fn(NativeMenu, &mut Window, &mut App) -> NativeMenu>;

/// [`text_box`] with an optional context-menu builder.
pub fn text_box_with_menu(
    id: impl Into<ElementId>,
    state: &Entity<InputState>,
    prefix: Option<Svg>,
    menu: Option<InputMenuBuilder>,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let focused = state.read(cx).focus_handle(cx).is_focused(window);
    div()
        .id(id)
        .h(TEXT_FIELD_HEIGHT())
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        // `input { padding: 0 5px }`: the kit's xsmall input pads 4 px on
        // each side itself, so the frame adds 1 px (and a prefix icon gets
        // its own 5 px)
        .pl(if prefix.is_some() {
            SPACING_HALF()
        } else {
            zpx(1.)
        })
        .pr(zpx(1.))
        .border_1()
        .rounded(BORDER_RADIUS())
        .bg(t.box_background)
        .border_color(if focused {
            t.focus
        } else {
            t.box_border_contrast
        })
        .when(focused, |d| {
            d.shadow(vec![BoxShadow {
                color: t.text_field_focus_shadow,
                offset: point(zpx(0.), zpx(0.)),
                blur_radius: zpx(0.),
                spread_radius: zpx(1.),
                inset: false,
            }])
        })
        .when_some(prefix, |d, icon| d.child(icon))
        .child(
            div()
                .flex_1()
                .min_w_0()
                // GHD inputs use the body font size (`--font-size`, 12 px);
                // the kit's `xsmall` would shrink text + placeholder to `text_xs`.
                .child({
                    let input = Input::new(state)
                        .appearance(false)
                        .xsmall()
                        .text_size(FONT_SIZE());
                    match menu {
                        Some(build) => {
                            input.context_menu(move |menu, window, cx| build(menu, window, cx))
                        }
                        None => input,
                    }
                }),
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
        .gap(SPACING_THIRD())
        .child(
            div()
                .text_size(FONT_SIZE())
                .text_color(t.text)
                .child(label.into()),
        )
        .child(field)
}

/// `Avatar`: the cached image when resolved, else the grey placeholder.
pub fn avatar_image(path: Option<std::path::PathBuf>, size: Pixels, cx: &App) -> AnyElement {
    match path {
        Some(path) => img(path)
            .size(size)
            .flex_none()
            .rounded_full()
            .into_any_element(),
        None => avatar_placeholder(size, cx).into_any_element(),
    }
}

/// Cached avatar for a commit e-mail (request it with
/// `Dispatcher::request_avatar_for_email` from a render with `&mut App`).
pub fn avatar_lookup(email: &str, cx: &App) -> Option<std::path::PathBuf> {
    corvane_core::AppState::try_global(cx)
        .and_then(|s| corvane_core::avatar_for_email(&s.read(cx).avatars, email))
}

/// Cached avatar for an account `avatar_url`.
pub fn avatar_lookup_url(url: &str, cx: &App) -> Option<std::path::PathBuf> {
    corvane_core::AppState::try_global(cx)
        .and_then(|s| corvane_core::avatar_for_url(&s.read(cx).avatars, url))
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

/// One row of GHD's `VerticalSegmentedControl` (`_vertical-segmented-control.scss`):
/// radio + bold title + secondary description, bordered, rounded at the ends.
#[allow(clippy::too_many_arguments)]
pub fn segmented_option(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    selected: bool,
    first: bool,
    last: bool,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let hover_bg = t.box_hover_background;
    let hover_text = t.box_hover_text;
    div()
        .id(id)
        .w_full()
        .flex()
        .flex_row()
        .items_start()
        .p(SPACING())
        .border_1()
        .border_color(t.box_border)
        .when(!last, |d| d.border_b_0())
        .when(first, |d| d.rounded_t(BORDER_RADIUS()))
        .when(last, |d| d.rounded_b(BORDER_RADIUS()))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).text_color(hover_text))
        .child(
            // radio
            div()
                .mx(SPACING_HALF())
                .mt(zpx(4.))
                .size(zpx(13.))
                .flex_none()
                .rounded_full()
                .border_1()
                .border_color(if selected {
                    t.button_background
                } else {
                    t.box_border_contrast
                })
                .bg(if selected {
                    t.button_background
                } else {
                    t.background
                })
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |d| {
                    d.child(div().size(zpx(5.)).rounded_full().bg(t.button_text))
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(title.into()),
                )
                .child(div().text_color(t.text_secondary).child(description.into())),
        )
}

/// GHD `.tool-tip-contents` (darwin): the small dark caption below a control.
struct TextTooltip {
    /// Lines separated by `\n`.
    text: SharedString,
    /// A `<strong>` byte range of `text`.
    bold: Option<std::ops::Range<usize>>,
    /// `direction` + the target's bounds; `None` anchors to the pointer.
    anchor: Option<(Bounds<Pixels>, TooltipDirection)>,
}

/// GHD `DefaultTooltipDelay` (`ui/lib/tooltip.tsx`).
pub const TOOLTIP_DELAY: std::time::Duration = std::time::Duration::from_millis(400);

/// GHD `TooltipDirection`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TooltipDirection {
    North,
    NorthEast,
    NorthWest,
    South,
    SouthEast,
    SouthWest,
    East,
    West,
}

/// GHD `getTooltipRectRelativeTo`: the tooltip's rect for `direction`
/// around `target`, leaving room for the 6 px arrow.
fn tooltip_rect(
    target: Bounds<Pixels>,
    direction: TooltipDirection,
    size: Size<Pixels>,
) -> Bounds<Pixels> {
    use TooltipDirection::*;
    let (tip_x, tip_w) = (px(10.), px(6.));
    let x_mid = target.left() + target.size.width / 2.;
    let y_top = target.top() - size.height;
    let y_mid = target.top() + target.size.height / 2. - size.height / 2.;
    let origin = match direction {
        NorthEast => point(x_mid - tip_x - tip_w, y_top - tip_w),
        North => point(x_mid - size.width / 2., y_top - tip_w),
        NorthWest => point(x_mid - size.width + tip_x + tip_w, y_top - tip_w),
        East => point(target.right() + tip_w, y_mid),
        SouthEast => point(x_mid - tip_x - tip_w, target.bottom() + tip_w),
        South => point(x_mid - size.width / 2., target.bottom() + tip_w),
        SouthWest => point(x_mid - size.width + tip_x + tip_w, target.bottom() + tip_w),
        West => point(target.left() - size.width - tip_w, y_mid),
    };
    Bounds::new(origin, size)
}

/// GHD `getDirection`: the desired direction when it fits, else the others
/// (its own side first); pointer-anchored tooltips try south-east, then
/// north-east. Falls back to south.
fn tooltip_direction(
    desired: Option<TooltipDirection>,
    target: Bounds<Pixels>,
    window: Bounds<Pixels>,
    size: Size<Pixels>,
) -> TooltipDirection {
    use TooltipDirection::*;
    let fits = |d: TooltipDirection| {
        let r = tooltip_rect(target, d, size);
        r.left() >= window.left()
            && r.top() >= window.top()
            && r.right() <= window.right()
            && r.bottom() <= window.bottom()
    };
    let all = [
        North, NorthEast, NorthWest, South, SouthEast, SouthWest, East, West,
    ];
    let mut order: Vec<TooltipDirection> = match desired {
        Some(d) if matches!(d, South | SouthEast | SouthWest) => {
            vec![d, South, SouthEast, SouthWest]
        }
        Some(d) if matches!(d, North | NorthEast | NorthWest) => {
            vec![d, North, NorthEast, NorthWest]
        }
        Some(d) => vec![d],
        None => vec![SouthEast, NorthEast],
    };
    for d in all {
        if !order.contains(&d) {
            order.push(d);
        }
    }
    order.into_iter().find(|d| fits(*d)).unwrap_or(South)
}

impl Render for TextTooltip {
    // GHD `Tooltip` without a `direction`: placed around a 20 px square at the
    // pointer (`mouseRect`), 300 px at most, 5 / 10 px padding, 11 px text,
    // a 6 px arrow pointing back at the pointer.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use TooltipDirection::*;
        let t = cx.ghd();
        let mouse = window.mouse_position();
        let (target, desired) = match self.anchor {
            Some((bounds, direction)) => (bounds, Some(direction)),
            None => (
                Bounds::new(mouse - point(px(10.), px(10.)), size(px(20.), px(20.))),
                None,
            ),
        };
        let viewport = Bounds::new(Point::default(), window.viewport_size());
        let font_size = FONT_SIZE_SM();
        let line_height = font_size * 1.5;
        let (pad_x, pad_y) = (SPACING(), SPACING_HALF());
        let max_text = zpx(300.) - pad_x * 2.;
        let mut style = window.text_style();
        style.font_size = font_size.into();
        let run = style.to_run(self.text.len());
        let text_size = window
            .text_system()
            .shape_text(self.text.clone(), font_size, &[run], Some(max_text), None)
            .ok()
            .map(|lines| {
                lines.iter().fold(size(px(0.), px(0.)), |acc, line| {
                    let s = line.size(line_height);
                    size(acc.width.max(s.width), acc.height + s.height)
                })
            })
            .unwrap_or(size(max_text, line_height));
        let box_size = size(
            text_size.width.ceil() + pad_x * 2.,
            text_size.height + pad_y * 2.,
        );
        let direction = tooltip_direction(desired, target, viewport, box_size);
        let rect = tooltip_rect(target, direction, box_size);
        let bg = t.tooltip_background;
        // `::before`: a 12 × 6 triangle outside the box
        let arrow = |path: &'static str, w: f32, h: f32| {
            svg()
                .path(path)
                .w(zpx(w))
                .h(zpx(h))
                .text_color(bg)
                .absolute()
        };
        let arrow = match direction {
            SouthEast => arrow("ui/tooltip-arrow-up.svg", 12., 6.)
                .top(zpx(-6.))
                .left(zpx(10.)),
            South => arrow("ui/tooltip-arrow-up.svg", 12., 6.)
                .top(zpx(-6.))
                .left(box_size.width / 2. - zpx(6.)),
            SouthWest => arrow("ui/tooltip-arrow-up.svg", 12., 6.)
                .top(zpx(-6.))
                .right(zpx(10.)),
            NorthEast => arrow("ui/tooltip-arrow-down.svg", 12., 6.)
                .bottom(zpx(-6.))
                .left(zpx(10.)),
            North => arrow("ui/tooltip-arrow-down.svg", 12., 6.)
                .bottom(zpx(-6.))
                .left(box_size.width / 2. - zpx(6.)),
            NorthWest => arrow("ui/tooltip-arrow-down.svg", 12., 6.)
                .bottom(zpx(-6.))
                .right(zpx(10.)),
            East => arrow("ui/tooltip-arrow-left.svg", 6., 12.)
                .left(zpx(-6.))
                .top(box_size.height / 2. - zpx(6.)),
            West => arrow("ui/tooltip-arrow-right.svg", 6., 12.)
                .right(zpx(-6.))
                .top(box_size.height / 2. - zpx(6.)),
        };
        anchored().position(rect.origin.map(|v| v.round())).child(
            div()
                .relative()
                .w(box_size.width)
                .px(pad_x)
                .py(pad_y)
                .rounded(BORDER_RADIUS())
                .bg(bg)
                .text_color(t.tooltip_text)
                .text_size(font_size)
                .line_height(line_height)
                .shadow(vec![BoxShadow {
                    color: t.tooltip_shadow,
                    offset: point(zpx(0.), zpx(8.)),
                    blur_radius: zpx(24.),
                    spread_radius: zpx(0.),
                    inset: false,
                }])
                .child(arrow)
                .child(
                    StyledText::new(self.text.clone()).with_highlights(self.bold.clone().map(
                        |range| {
                            (
                                range,
                                HighlightStyle {
                                    font_weight: Some(FontWeight::BOLD),
                                    ..Default::default()
                                },
                            )
                        },
                    )),
                ),
        )
    }
}

/// GHD tooltips on any element: the caption tooltip after
/// [`TOOLTIP_DELAY`].
pub trait GhdTooltip: StatefulInteractiveElement + Sized {
    fn ghd_tooltip(self, text: impl Into<SharedString>) -> Self {
        let text: SharedString = text.into();
        self.tooltip(tooltip(text))
            .tooltip_show_delay(TOOLTIP_DELAY)
    }
}

impl<E: StatefulInteractiveElement> GhdTooltip for E {}

/// Accessibility for icon-only controls, after GHD's `ariaLabel` +
/// `tooltip` pairs: a `Button` node VoiceOver announces by `label`, and the
/// caption tooltip.
pub trait IconButtonA11y: StatefulInteractiveElement + Sized {
    /// `Button` role, accessible name and tooltip.
    fn icon_button_label(self, label: impl Into<SharedString>) -> Self {
        let label: SharedString = label.into();
        self.a11y_button(label.clone()).ghd_tooltip(label)
    }

    /// `Button` role and accessible name only (the tooltip is set elsewhere
    /// or differs from the name).
    fn a11y_button(self, label: impl Into<SharedString>) -> Self {
        self.role(Role::Button).aria_label(label)
    }
}

impl<E: StatefulInteractiveElement> IconButtonA11y for E {}

/// VoiceOver for list rows and banners (GHD `List` rows are `role="option"`
/// with `aria-selected`; `AriaLiveContainer` for banners).
pub trait ListRowA11y: StatefulInteractiveElement + Sized {
    /// A `Row` node (macOS `AXRow`, which carries the selection) named
    /// `label`, with its selected state.
    fn a11y_row(self, label: impl Into<SharedString>, selected: bool) -> Self {
        self.role(Role::Row)
            .aria_label(label)
            .aria_selected(selected)
    }

    /// A polite live region: VoiceOver announces `text` when the element
    /// appears or the text changes.
    fn a11y_live(self, text: impl Into<SharedString>) -> Self {
        let text: SharedString = text.into();
        let value = text.to_string();
        self.role(Role::Status)
            .aria_label(text)
            .a11y_synthetic_children(move |tree| {
                let node = tree.parent_node();
                node.set_live(accesskit::Live::Polite);
                node.set_value(value);
            })
    }
}

impl<E: StatefulInteractiveElement> ListRowA11y for E {}

/// A file status as VoiceOver reads it (GHD `mapStatus`).
pub fn status_label(kind: corvane_core::FileStatusKind) -> &'static str {
    use corvane_core::FileStatusKind::*;
    match kind {
        New | Untracked => "New",
        Modified => "Modified",
        Deleted => "Deleted",
        Copied => "Copied",
        Renamed => "Renamed",
        Conflicted => "Conflicted",
    }
}

/// A `.tooltip(...)` builder with GHD's caption look, anchored to the pointer.
pub fn tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView {
    let text: SharedString = text.into();
    move |_, cx| {
        cx.new(|_| TextTooltip {
            text: text.clone(),
            bold: None,
            anchor: None,
        })
        .into()
    }
}

/// [`tooltip`] with a bold byte range (`<strong>`); `\n` breaks lines.
pub fn rich_tooltip(
    text: impl Into<SharedString>,
    bold: std::ops::Range<usize>,
) -> impl Fn(&mut Window, &mut App) -> AnyView {
    let text: SharedString = text.into();
    move |_, cx| {
        cx.new(|_| TextTooltip {
            text: text.clone(),
            bold: Some(bold.clone()),
            anchor: None,
        })
        .into()
    }
}

/// GHD `<Tooltip direction={…}>`: the caption tooltip placed around `el`
/// (a probe child records its bounds) after [`TOOLTIP_DELAY`].
pub fn with_directed_tooltip(
    el: Stateful<Div>,
    text: impl Into<SharedString>,
    direction: TooltipDirection,
) -> Stateful<Div> {
    let text: SharedString = text.into();
    let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::default()));
    let probe = bounds.clone();
    el.relative()
        .child(
            canvas(move |b, _, _| probe.set(b), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .tooltip(move |_, cx| {
            cx.new(|_| TextTooltip {
                text: text.clone(),
                bold: None,
                anchor: Some((bounds.get(), direction)),
            })
            .into()
        })
        .tooltip_show_delay(TOOLTIP_DELAY)
}

/// GHD `.blankslate-image`: an `illustrations/<name>` picture, drawn in the
/// dark themes through `invert() grayscale(1) brightness(8) contrast(0.6)`,
/// which `tools/illustrations/darken.py` bakes into `illustrations/dark/`.
pub fn blankslate_image(name: &str, cx: &App) -> Img {
    if cx.ghd().appearance == crate::theme::Appearance::Dark {
        img(format!("illustrations/dark/{name}"))
    } else {
        img(format!("illustrations/{name}"))
    }
}
