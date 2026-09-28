//! `.tab-bar.tabs` - the Changes | History switcher (29 px).

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::counter;

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
        .h(TAB_BAR_HEIGHT)
        .flex_none()
        .flex()
        .flex_row()
        .bg(t.tab_bar_background)
        .children(tabs.into_iter().enumerate().map(|(ix, tab)| {
            let is_selected = ix == selected;
            let is_last = ix + 1 == count;
            let on_select = on_select.clone();
            let hover_bg = t.tab_bar_hover_background;
            div()
                .id(tab.id)
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .border_b_1()
                .border_color(t.box_border)
                .when(!is_last, |d| d.border_r_1())
                .text_color(t.text)
                .text_size(FONT_SIZE)
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(move |_, window, cx| on_select(ix, window, cx))
                .child(
                    div()
                        .relative()
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
                            .h(px(3.))
                            .bg(t.tab_bar_active),
                    )
                    .relative()
                })
        }))
}
