//! History sidebar: compare-to-branch box + commit list (M0: empty list).

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::text_box;

pub struct HistorySidebar {
    compare: Entity<InputState>,
}

impl HistorySidebar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let compare =
            cx.new(|cx| InputState::new(window, cx).placeholder("Select Branch to Compare…"));
        Self { compare }
    }
}

impl Render for HistorySidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                // `#compare-view .compare-form`
                div()
                    .flex_none()
                    .p(SPACING_HALF)
                    .bg(t.box_alt_background)
                    .border_b_1()
                    .border_color(t.box_border)
                    .child(text_box(
                        "compare-branch",
                        &self.compare,
                        Some(octicon(Octicon::GitBranch, t.text_secondary)),
                        window,
                        cx,
                    )),
            )
            .child(div().flex_1().bg(t.background))
    }
}
