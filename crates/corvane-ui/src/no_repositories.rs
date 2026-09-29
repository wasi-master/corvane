//! `#no-repositories` - first-launch blankslate ("Let's get started!").
//! `styles/ui/_no-repositories.scss`, `ui/no-repositories/no-repositories-view.tsx`.
//! Signed in, the first button creates the tutorial repository, or returns
//! to a paused tutorial (`renderTutorialRepositoryButton`).

use corvane_core::{Dispatcher, Popup};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

fn big_button(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    cx: &App,
) -> impl IntoElement {
    button_impl(id, icon, label, on_click, false, cx)
}

/// `type="submit"`: the blue variant of [`big_button`].
fn primary_big_button(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    cx: &App,
) -> impl IntoElement {
    button_impl(id, icon, label, on_click, true, cx)
}

fn button_impl(
    id: &'static str,
    icon: Octicon,
    label: &'static str,
    on_click: fn(&mut Window, &mut App),
    primary: bool,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
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
    div()
        .id(id)
        .w_full()
        .mb(SPACING())
        .p(SPACING())
        .flex()
        .flex_row()
        .items_center()
        .border_1()
        .rounded(BORDER_RADIUS())
        .bg(bg)
        .border_color(border)
        .text_color(text)
        .text_size(FONT_SIZE())
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).border_color(hover_border))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(
            octicon(icon, text)
                .size(zpx(24.))
                .my(SPACING_HALF())
                .ml(SPACING_HALF())
                .mr(SPACING()),
        )
        .child(div().flex_1().min_w_0().child(label))
}

/// `renderTutorialRepositoryButton`: only when signed in.
fn tutorial_button(cx: &App) -> Option<AnyElement> {
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
                cx,
            )
            .into_any_element()
        } else {
            primary_big_button(
                "nr-tutorial",
                Octicon::MortarBoard,
                "Create a Tutorial Repository…",
                |_, cx| Dispatcher::show_create_tutorial_repository(cx),
                cx,
            )
            .into_any_element()
        },
    )
}

pub fn no_repositories(cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("no-repositories")
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_center()
        .p(zpx(60.))
        .bg(t.background)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    // header
                    div()
                        .flex()
                        .flex_col()
                        .mb(zpx(40.))
                        .child(
                            div()
                                .text_size(zpx(42.))
                                .line_height(zpx(50.))
                                .font_weight(FontWeight::LIGHT)
                                .child("Let's get started!"),
                        )
                        .child(
                            div()
                                .text_size(FONT_SIZE())
                                .child("Add a repository to Corvane to start collaborating"),
                        ),
                )
                .child(
                    // `.content`: two 50 % panes
                    div()
                        .flex()
                        .flex_row()
                        .w_full()
                        .child(
                            div()
                                .w_1_2()
                                .pr(SPACING())
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .child(
                                    img("illustrations/empty-no-repo.svg")
                                        .w_full()
                                        .max_w(zpx(400.))
                                        .h(zpx(240.))
                                        .object_fit(ObjectFit::Contain),
                                ),
                        )
                        .child(
                            div()
                                .w_1_2()
                                .pl(SPACING())
                                .flex()
                                .flex_col()
                                .children(tutorial_button(cx))
                                .child(big_button(
                                    "nr-clone",
                                    Octicon::RepoClone,
                                    "Clone a Repository from the Internet…",
                                    |_, cx| {
                                        Dispatcher::show_popup(Popup::CloneRepository { url: None }, cx)
                                    },
                                    cx,
                                ))
                                .child(big_button(
                                    "nr-create",
                                    Octicon::Plus,
                                    "Create a New Repository on your Local Drive…",
                                    |_, cx| {
                                        Dispatcher::show_popup(Popup::CreateRepository { path: None }, cx)
                                    },
                                    cx,
                                ))
                                .child(big_button(
                                    "nr-add",
                                    Octicon::FileDirectory,
                                    "Add an Existing Repository from your Local Drive…",
                                    |_, cx| Dispatcher::show_popup(Popup::AddExistingRepository { path: None }, cx),
                                    cx,
                                ))
                                .child(
                                    // `.drag-drop-info`
                                    div()
                                        .mt(SPACING())
                                        .p(SPACING_DOUBLE())
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .rounded(BORDER_RADIUS())
                                        .border_2()
                                        .border_dashed()
                                        .border_color(t.box_border_contrast)
                                        .bg(t.box_alt_background)
                                        .child(octicon(Octicon::FileDirectory, t.text_secondary).mr(SPACING()))
                                        .child(
                                            div()
                                                .text_size(FONT_SIZE())
                                                .child("ProTip! You can drag & drop an existing repository folder here to add it to Corvane"),
                                        ),
                                ),
                        ),
                ),
        )
}
