//! `#no-repositories` - first-launch blankslate ("Let's get started!").
//! `styles/ui/_no-repositories.scss`, `ui/no-repositories/no-repositories-view.tsx`.
//! Signed in, the first button creates the tutorial repository, or returns
//! to a paused tutorial (`renderTutorialRepositoryButton`).
//!
//! Layout: a column centred in the window at its content width (the action
//! pane's max-content, which the pane's `width: 50%` then halves), the
//! Welcome's two graphics pinned top-right / bottom-right behind it, all
//! zoomed 1.2–1.5 by window width (`@media … { zoom }`).
//!
//! Deviation: GHD shows the signed-in account's cloneable repositories in a
//! second pane (`CloneableRepositoryFilterList`); Corvane always shows the
//! actions pane alone.

use std::sync::atomic::{AtomicBool, Ordering};

use corvane_core::{Dispatcher, Popup};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

/// The Clone button's `autoFocus` ring, until a mouse press takes focus
/// without `:focus-visible`.
static CLONE_FOCUS_VISIBLE: AtomicBool = AtomicBool::new(true);

/// `#no-repositories { zoom }` for the window width.
fn zoom(viewport_width: Pixels) -> f32 {
    let w = unzoom(viewport_width);
    if w >= 1800. {
        1.5
    } else if w >= 1600. {
        1.4
    } else if w >= 1400. {
        1.3
    } else if w >= 1366. {
        1.2
    } else {
        1.
    }
}

fn big_button(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    z: f32,
    cx: &App,
) -> Stateful<Div> {
    button_impl(id, icon, label, on_click, false, z, cx)
}

/// `type="submit"`: the blue variant of [`big_button`].
fn primary_big_button(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    z: f32,
    cx: &App,
) -> Stateful<Div> {
    button_impl(id, icon, label, on_click, true, z, cx)
}

fn button_impl(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    primary: bool,
    z: f32,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let s = |v: f32| zpx(v * z);
    let (bg, border, text, hover_bg, hover_border) = if primary {
        (
            t.button_background,
            t.button_background,
            t.button_text,
            t.button_hover_background,
            t.button_hover_background,
        )
    } else {
        (
            t.secondary_button_background,
            t.secondary_button_border,
            t.secondary_button_text,
            t.secondary_button_hover_background,
            t.secondary_button_hover_border,
        )
    };
    // `.button-group span .button-component`: 10 px padding, a 24 px icon
    // with 5 / 10 / 5 / 5 px margins, the title wrapping beside it
    div()
        .id(id)
        .w_full()
        .p(s(10.))
        .flex()
        .flex_row()
        .items_center()
        .border_1()
        .rounded(s(6.))
        .bg(bg)
        .border_color(border)
        .text_color(text)
        .text_size(s(12.))
        .line_height(s(14.17))
        .cursor_pointer()
        .hover(move |st| st.bg(hover_bg).border_color(hover_border))
        .on_mouse_down(MouseButton::Left, |_, _, _| {
            CLONE_FOCUS_VISIBLE.store(false, Ordering::Relaxed)
        })
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(
            octicon(icon, text)
                .flex_none()
                .size(s(24.))
                .mt(s(5.))
                .mb(s(5.))
                .ml(s(5.))
                .mr(s(10.)),
        )
        .child(div().flex_1().min_w_0().child(label))
}

/// A `.button-group span`: 10 px below each button; `ring` draws the
/// `autoFocus` outline 2 px outside the border.
fn slot(button: Stateful<Div>, ring: bool, z: f32, cx: &App) -> Div {
    let s = |v: f32| zpx(v * z);
    div()
        .relative()
        .mb(s(10.))
        .when(ring, |d| {
            d.child(
                div()
                    .absolute()
                    .top(-s(4.))
                    .left(-s(4.))
                    .right(-s(4.))
                    .bottom(-s(4.))
                    .border(s(2.))
                    .border_color(cx.ghd().focus)
                    .rounded(s(10.)),
            )
        })
        .child(button)
}

/// `renderTutorialRepositoryButton`: only when signed in.
fn tutorial_button(z: f32, cx: &App) -> Option<Stateful<Div>> {
    let s = corvane_core::AppState::try_global(cx)?.read(cx);
    if s.accounts.is_empty() {
        return None;
    }
    Some(
        if s.selected_tutorial_step() == corvane_core::tutorial::TutorialStep::Paused {
            primary_big_button(
                "nr-tutorial",
                Octicon::MortarBoard,
                "Return to In Progress Tutorial",
                |_, cx| Dispatcher::resume_tutorial(cx),
                z,
                cx,
            )
        } else {
            primary_big_button(
                "nr-tutorial",
                Octicon::MortarBoard,
                "Create a Tutorial Repository…",
                |_, cx| Dispatcher::show_create_tutorial_repository(cx),
                z,
                cx,
            )
        },
    )
}

pub fn no_repositories(window: &Window, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let viewport = window.viewport_size();
    let z = zoom(viewport.width);
    let s = |v: f32| zpx(v * z);
    let signed_in =
        corvane_core::AppState::try_global(cx).is_some_and(|s| !s.read(cx).accounts.is_empty());
    let ring = !signed_in && CLONE_FOCUS_VISIBLE.load(Ordering::Relaxed);
    // `103-product-name`; GHD's ProTip says "add it to Desktop" (the last word)
    let name = corvane_core::AppState::try_global(cx)
        .map(|s| s.read(cx).product_name().to_string())
        .unwrap_or_else(|| "Corvane".into());
    let short_name = name
        .split_whitespace()
        .last()
        .unwrap_or("Corvane")
        .to_string();
    // `height: 20%` / `40%` of the view, width from the SVGs' aspect,
    // capped at 20 % / 40 % of its width
    let top_h = (viewport.height * 0.2).min(viewport.width * 0.2 * (43.835956 / 42.2971));
    let bottom_h = (viewport.height * 0.4).min(viewport.width * 0.4 * (172.30263 / 113.99702));
    div()
        .id("no-repositories")
        .relative()
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_col()
        .items_center()
        .p(s(60.))
        .bg(t.background)
        .child(
            img("illustrations/welcome-illustration-left-top.svg")
                .absolute()
                .right(s(80.))
                .top(s(40.))
                .h(top_h)
                .w(top_h * (42.2971 / 43.835956)),
        )
        .child(
            img("illustrations/welcome-illustration-left-bottom.svg")
                .absolute()
                .right(s(10.))
                .bottom(s(10.))
                .h(bottom_h)
                .w(bottom_h * (113.99702 / 172.30263)),
        )
        .child(
            // `section`: as wide as the action pane's content (536 px for
            // these titles), the full height
            div()
                .flex_1()
                .w(s(536.1))
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    // header
                    div()
                        .flex()
                        .flex_col()
                        .mb(s(40.))
                        .child(
                            div()
                                .text_size(s(42.))
                                .line_height(s(63.))
                                .font_weight(FontWeight::LIGHT)
                                .child("Let's get started!"),
                        )
                        .child(
                            div().text_size(s(12.)).line_height(s(18.)).child(format!(
                                "Add a repository to {name} to start collaborating"
                            )),
                        ),
                )
                .child(
                    // `.content > .content-pane`: 50 %, the button group
                    // growing above the ProTip
                    div()
                        .flex_1()
                        .w_1_2()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .children(tutorial_button(z, cx).map(|b| slot(b, false, z, cx)))
                                .child(slot(
                                    big_button(
                                        "nr-clone",
                                        Octicon::RepoClone,
                                        "Clone a Repository from the Internet…",
                                        |_, cx| {
                                            Dispatcher::show_popup(
                                                Popup::CloneRepository { url: None },
                                                cx,
                                            )
                                        },
                                        z,
                                        cx,
                                    ),
                                    ring,
                                    z,
                                    cx,
                                ))
                                .child(slot(
                                    big_button(
                                        "nr-create",
                                        Octicon::Plus,
                                        "Create a New Repository on your Local Drive…",
                                        |_, cx| {
                                            Dispatcher::show_popup(
                                                Popup::CreateRepository { path: None },
                                                cx,
                                            )
                                        },
                                        z,
                                        cx,
                                    ),
                                    false,
                                    z,
                                    cx,
                                ))
                                .child(slot(
                                    big_button(
                                        "nr-add",
                                        Octicon::FileDirectory,
                                        "Add an Existing Repository from your Local Drive…",
                                        |_, cx| {
                                            Dispatcher::show_popup(
                                                Popup::AddExistingRepository { path: None },
                                                cx,
                                            )
                                        },
                                        z,
                                        cx,
                                    ),
                                    false,
                                    z,
                                    cx,
                                )),
                        )
                        .child(
                            // `.drag-drop-info`
                            div()
                                .p(s(20.))
                                .flex()
                                .flex_row()
                                .rounded(s(6.))
                                .border_2()
                                .border_dashed()
                                .border_color(t.tip_box_border)
                                .bg(t.tip_box_background)
                                .text_size(s(12.))
                                .line_height(s(18.))
                                .child(
                                    octicon(Octicon::LightBulb, t.text)
                                        .flex_none()
                                        .size(s(16.))
                                        .mr(s(10.)),
                                )
                                .child(
                                    div().flex_1().min_w_0().child(
                                        // `<strong>ProTip!</strong>` inline in the sentence
                                        StyledText::new(format!(
                                            "ProTip! You can drag & drop an existing repository \
                                             folder here to add it to {short_name}"
                                        ))
                                        .with_highlights(
                                            [(
                                                0..7,
                                                HighlightStyle {
                                                    font_weight: Some(FontWeight::SEMIBOLD),
                                                    ..Default::default()
                                                },
                                            )],
                                        ),
                                    ),
                                ),
                        ),
                ),
        )
}
