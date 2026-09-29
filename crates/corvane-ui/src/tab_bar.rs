//! `.tab-bar.tabs` - the Changes | History switcher (29 px).
//! Tabs and bar share `--background-color`; hover uses `--tab-bar-hover-background-color`.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::counter;

/// `.tab-bar.vertical` item: 16 px icon + label.
pub struct VerticalTab {
    pub id: &'static str,
    pub label: SharedString,
    pub icon: Octicon,
}

/// Settings / Repository Settings side navigation (`.tab-bar.vertical`):
/// column with 10 px vertical padding; items ≥150 px wide, 10 px padding,
/// 3.33 px × 20 px margins, 6 px radius; selected = blue with white text,
/// otherwise the icon is `$gray-500` and hover tints the row.
pub fn vertical_tab_bar(
    tabs: Vec<VerticalTab>,
    selected: usize,
    on_select: impl Fn(usize, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let icon_muted = crate::theme::c(crate::theme::primer::GRAY_500);
    div()
        .id("vertical-tab-bar")
        .flex_none()
        .flex()
        .flex_col()
        .py(SPACING())
        .children(tabs.into_iter().enumerate().map(|(ix, tab)| {
            let is_selected = ix == selected;
            let on_select = on_select.clone();
            let hover_bg = t.tab_bar_hover_background;
            let (bg, text, icon) = if is_selected {
                (
                    t.tab_bar_active,
                    t.box_selected_active_text,
                    t.box_selected_active_text,
                )
            } else {
                (t.background, t.text, icon_muted)
            };
            div()
                .id(tab.id)
                .min_w(zpx(150.))
                .my(SPACING_THIRD())
                .mx(SPACING_DOUBLE())
                .p(SPACING())
                .rounded(BORDER_RADIUS())
                .flex()
                .flex_row()
                .items_center()
                .bg(bg)
                .text_color(text)
                .text_size(FONT_SIZE())
                .cursor_pointer()
                .when(!is_selected, move |d| d.hover(move |s| s.bg(hover_bg)))
                .on_click(move |_, window, cx| on_select(ix, window, cx))
                .child(octicon(tab.icon, icon).mr(SPACING()))
                .child(tab.label)
        }))
}

pub struct TabModel {
    pub id: &'static str,
    pub label: SharedString,
    pub count: Option<usize>,
}

pub fn tab_bar(
    tabs: Vec<TabModel>,
    selected: usize,
    on_select: impl Fn(usize, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let count = tabs.len();
    div()
        .id("tab-bar")
        .w_full()
        .h(TAB_BAR_HEIGHT())
        .flex_none()
        .flex()
        .flex_row()
        .bg(t.background)
        .children(tabs.into_iter().enumerate().map(|(ix, tab)| {
            let is_selected = ix == selected;
            let is_last = ix + 1 == count;
            let on_select = on_select.clone();
            let hover_bg = t.tab_bar_hover_background;
            div()
                .id(tab.id)
                .relative()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(t.background)
                .border_b_1()
                .border_color(t.box_border)
                .when(!is_last, |d| d.border_r_1())
                .text_color(t.text)
                .text_size(FONT_SIZE())
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(move |_, window, cx| on_select(ix, window, cx))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(tab.label)
                        .when_some(tab.count, |d, n| d.child(counter(n, cx))),
                )
                .when(is_selected, |d| {
                    // `box-shadow: inset 0 -3px 0 var(--tab-bar-active-color)`
                    d.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .h(zpx(3.))
                            .bg(t.tab_bar_active),
                    )
                })
        }))
}
