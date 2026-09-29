//! View › Expand Active Resizable ⌘9 / Contract Active Resizable ⌘8 - GHD
//! `ui/resizable/resizable.tsx` (`handleMenuResizeEvent`) and `ui/app.tsx`
//! (`resizeActiveResizable`): the menu event goes to the element with keyboard
//! focus and the enclosing `Resizable` changes its width by 5 px, clamped to
//! its minimum and maximum. GPUI routes the action the same way: it bubbles
//! from the focused element, so only a wrapper around a focused pane handles
//! it, and the menu items are disabled when no pane has focus (GHD
//! `resizablePaneActive`).
//!
//! Each step is announced as GHD's `updateResizeMessage` does ("Repository
//! sidebar width increased. Set to 42%") through a polite live region inside
//! the pane (`AriaLiveContainer`). Deviation: the percentage is that of the
//! new width; GHD computes it before applying the step, so it lags by one.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;

use gpui_kit::component::resizable::ResizableState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{ContractActiveResizable, ExpandActiveResizable};
use crate::widgets::ListRowA11y;

/// `handleMenuResizeEvent`'s step.
pub const MENU_RESIZE_STEP: Pixels = px(5.);

thread_local! {
    /// GHD `resizeMessage` state, per resizable.
    static MESSAGES: RefCell<HashMap<EntityId, SharedString>> = RefCell::new(HashMap::new());
}

/// What the announcement says about a pane: GHD's `description` prop and
/// the panel's `minimumWidth..maximumWidth`.
#[derive(Clone)]
pub struct ResizableDescription {
    pub description: &'static str,
    pub range: Range<Pixels>,
}

impl ResizableDescription {
    pub fn new(description: &'static str, range: Range<Pixels>) -> Self {
        Self { description, range }
    }

    /// `getResizePercentage` for `width`.
    fn percentage(&self, width: Pixels) -> i64 {
        let (min, max) = (f32::from(self.range.start), f32::from(self.range.end));
        if max <= min {
            return 100;
        }
        (((f32::from(width) - min) / (max - min)) * 100.).round() as i64
    }

    /// `updateResizeMessage`
    fn message(&self, increased: bool, width: Pixels) -> String {
        format!(
            "{} width {}. Set to {}%",
            self.description,
            if increased { "increased" } else { "decreased" },
            self.percentage(width)
        )
    }
}

/// Resize the first panel of `resizable` by `delta` (clamped by the panel's
/// `size_range`; subscribers persist it like a drag), then announce it.
fn nudge(
    resizable: &Entity<ResizableState>,
    delta: Pixels,
    about: &ResizableDescription,
    window: &mut Window,
    cx: &mut App,
) {
    let width = resizable.update(cx, |state, cx| {
        let width = state.sizes().first().copied()?;
        state.resize_panel(0, width + delta, window, cx);
        state.sizes().first().copied()
    });
    if let Some(width) = width {
        let message: SharedString = about.message(delta > px(0.), width).into();
        MESSAGES.with(|m| m.borrow_mut().insert(resizable.entity_id(), message));
        window.refresh();
    }
}

/// Wrap a resizable panel's content so ⌘9 / ⌘8 resize it while keyboard
/// focus is inside. With `focus`, the wrapper is itself focusable and takes
/// focus on mouse down (a file list without its own focus handle).
pub fn active_resizable(
    id: impl Into<ElementId>,
    resizable: &Entity<ResizableState>,
    focus: Option<&FocusHandle>,
    about: ResizableDescription,
    content: impl IntoElement,
) -> Stateful<Div> {
    let expand = resizable.clone();
    let contract = resizable.clone();
    let focus = focus.cloned();
    let message = MESSAGES.with(|m| m.borrow().get(&resizable.entity_id()).cloned());
    let expand_about = about.clone();
    div()
        .id(id)
        .relative()
        .size_full()
        .when_some(focus, |d, focus| {
            let handle = focus.clone();
            d.track_focus(&focus)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    if !handle.contains_focused(window, cx) {
                        window.focus(&handle, cx);
                    }
                })
        })
        .on_action(move |_: &ExpandActiveResizable, window, cx| {
            nudge(&expand, MENU_RESIZE_STEP, &expand_about, window, cx)
        })
        .on_action(move |_: &ContractActiveResizable, window, cx| {
            nudge(&contract, -MENU_RESIZE_STEP, &about, window, cx)
        })
        .child(content)
        // `AriaLiveContainer`: invisible, announced when the message changes
        .when_some(message, |d, message| {
            d.child(
                div()
                    .id("resize-message")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size(px(1.))
                    .overflow_hidden()
                    .a11y_live(message),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn messages_like_ghd() {
        let about = ResizableDescription::new("Repository sidebar", px(220.)..px(900.));
        assert_eq!(
            about.message(true, px(559.)),
            "Repository sidebar width increased. Set to 50%"
        );
        assert_eq!(
            about.message(false, px(220.)),
            "Repository sidebar width decreased. Set to 0%"
        );
    }
}
