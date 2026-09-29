//! View › Expand Active Resizable ⌘9 / Contract Active Resizable ⌘8 - GHD
//! `ui/resizable/resizable.tsx` (`handleMenuResizeEvent`) and `ui/app.tsx`
//! (`resizeActiveResizable`): the menu event goes to the element with keyboard
//! focus and the enclosing `Resizable` changes its width by 5 px, clamped to
//! its minimum and maximum. GPUI routes the action the same way: it bubbles
//! from the focused element, so only a wrapper around a focused pane handles
//! it, and the menu items are disabled when no pane has focus (GHD
//! `resizablePaneActive`).
//!
//! Deviation: GHD also announces the new width ("… width increased. Set to
//! N%") in an aria-live region; Corvane has no live regions.

use gpui_kit::component::resizable::ResizableState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{ContractActiveResizable, ExpandActiveResizable};

/// `handleMenuResizeEvent`'s step.
pub const MENU_RESIZE_STEP: Pixels = px(5.);

/// Resize the first panel of `resizable` by `delta` (clamped by the panel's
/// `size_range`; subscribers persist it like a drag).
fn nudge(resizable: &Entity<ResizableState>, delta: Pixels, window: &mut Window, cx: &mut App) {
    resizable.update(cx, |state, cx| {
        if let Some(width) = state.sizes().first().copied() {
            state.resize_panel(0, width + delta, window, cx);
        }
    });
}

/// Wrap a resizable panel's content so ⌘9 / ⌘8 resize it while keyboard
/// focus is inside. With `focus`, the wrapper is itself focusable and takes
/// focus on mouse down (a file list without its own focus handle).
pub fn active_resizable(
    id: impl Into<ElementId>,
    resizable: &Entity<ResizableState>,
    focus: Option<&FocusHandle>,
    content: impl IntoElement,
) -> Stateful<Div> {
    let expand = resizable.clone();
    let contract = resizable.clone();
    let focus = focus.cloned();
    div()
        .id(id)
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
            nudge(&expand, MENU_RESIZE_STEP, window, cx)
        })
        .on_action(move |_: &ContractActiveResizable, window, cx| {
            nudge(&contract, -MENU_RESIZE_STEP, window, cx)
        })
        .child(content)
}
