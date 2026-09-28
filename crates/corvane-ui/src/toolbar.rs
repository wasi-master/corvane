//! `#desktop-app-toolbar`: Repository / Branch / Push-Pull buttons.
//! Geometry from `styles/ui/toolbar/{_toolbar,_button,_dropdown}.scss`.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

/// Static description of one toolbar button. Data comes from the app state;
/// for M0 the workspace supplies placeholders.
pub struct ToolbarButtonModel {
    pub id: &'static str,
    pub icon: Octicon,
    /// Small secondary line ("Current Repository").
    pub description: SharedString,
    /// Bold main line (repository / branch name).
    pub title: SharedString,
    pub width: Option<Pixels>,
    pub dropdown: bool,
}

pub fn toolbar_button(model: ToolbarButtonModel, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let hover_bg = t.toolbar_button_hover_background;
    let hover_text = t.toolbar_button_hover_text;
    div()
        .id(model.id)
        .h(TOOLBAR_BUTTON_HEIGHT)
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .p(SPACING)
        .border_r_1()
        .border_color(t.toolbar_button_border)
        .text_color(t.toolbar_text)
        .cursor_pointer()
        .overflow_hidden()
        .hover(move |s| s.bg(hover_bg).text_color(hover_text))
        .when_some(model.width, |d, w| d.w(w))
        .when(model.width.is_none(), |d| d.flex_1().min_w_0())
        .child(octicon(model.icon, t.toolbar_text).mr(SPACING))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .mr(SPACING)
                .child(
                    div()
                        .text_size(FONT_SIZE_SM)
                        .line_height(px(14.))
                        .text_color(t.toolbar_text_secondary)
                        .truncate()
                        .child(model.description),
                )
                .child(
                    div()
                        .text_size(FONT_SIZE)
                        .line_height(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(model.title),
                ),
        )
        .when(model.dropdown, |d| {
            d.child(octicon(Octicon::TriangleDown, t.toolbar_text))
        })
}

/// The toolbar row: 50 px tall including its 1 px bottom border.
pub fn toolbar(buttons: Vec<ToolbarButtonModel>, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("toolbar")
        .w_full()
        .h(TOOLBAR_HEIGHT)
        .flex_none()
        .flex()
        .flex_row()
        .min_w_0()
        .bg(t.toolbar_background)
        .border_b_1()
        .border_color(t.toolbar_border)
        .text_color(t.toolbar_text)
        .children(buttons.into_iter().map(|b| toolbar_button(b, cx)))
}
