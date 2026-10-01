//! `#missing-repository-view` - GHD `ui/missing-repository.tsx`
//! (`styles/ui/_missing-repository-view.scss`), shown in place of the
//! repository view for a repository marked missing. Two variants:
//!
//! - "Can't find "<name>"": the directory is gone or no longer a repository.
//!   "It was last seen at <path>. Check again." and the "Locate…", "Clone
//!   Again" (GitHub repositories only) and "Remove" buttons.
//! - "<name> is potentially unsafe": git refuses to work in it ("detected
//!   dubious ownership"); the directory git named and the "Trust Repository"
//!   (`git config --global --add safe.directory`) and "Remove" buttons.

use std::path::Path;

use corvane_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::icons::loading;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{Inline, button, code_ref, link_button, paragraph, primary_button};

/// `button { min-width: 120px }`
#[allow(non_snake_case)]
fn BUTTON_MIN_WIDTH() -> Pixels {
    zpx(120.)
}

/// The `UiView` frame: a 600 px column centred on the ~40 % line, the
/// `.title-container` (xl title + `.details`) above the `<Row>` of buttons.
fn frame(
    title: String,
    details: impl IntoElement,
    buttons: Vec<AnyElement>,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id("missing-repository-view")
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(t.background)
        .child(
            div()
                .w(zpx(600.))
                .mt(zpx(-60.))
                .p(SPACING_DOUBLE())
                .flex()
                .flex_col()
                .items_center()
                .child(
                    // `.title-container`
                    div()
                        .max_w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .mb(SPACING())
                        .child(
                            div()
                                .max_w_full()
                                .text_size(zpx(32.))
                                .font_weight(FontWeight::LIGHT)
                                .truncate()
                                .child(title),
                        )
                        .child(details),
                )
                .child(
                    // `<Row>` of buttons
                    div().flex().flex_row().gap(SPACING()).children(buttons),
                ),
        )
}

/// The "Can't find" variant (`isPathUnsafe` false). `can_clone_again`:
/// the repository has a GitHub clone URL (`canCloneAgain`).
pub fn missing_repository_view(
    id: u64,
    name: &str,
    path: &Path,
    can_clone_again: bool,
    cx: &App,
) -> impl IntoElement {
    // `.details`: centred text, the path as `.path` and "Check again." as a
    // `LinkButton`
    let mut parts: Vec<Inline> = vec!["It was last seen at ".into()];
    parts.extend(path_segments(&path.display().to_string(), cx));
    parts.push(". ".into());
    parts.push(Inline::Element(
        link_button("missing-repository-check-again", "Check\u{a0}again.", cx)
            .on_click(move |_, _, cx| Dispatcher::refresh_repository(id, cx))
            .into_any_element(),
    ));
    let details = div()
        .my(SPACING())
        .w_full()
        .child(paragraph(parts).justify_center());
    let mut buttons = vec![
        primary_button("locate-repository", "Locate…", false, cx)
            .min_w(BUTTON_MIN_WIDTH())
            .role(Role::Button)
            .aria_label("Locate…")
            .on_click(move |_, _, cx| Dispatcher::relocate_repository(id, cx))
            .into_any_element(),
    ];
    if can_clone_again {
        buttons.push(
            button("clone-again", "Clone Again", cx)
                .min_w(BUTTON_MIN_WIDTH())
                .role(Role::Button)
                .aria_label("Clone Again")
                .on_click(move |_, _, cx| Dispatcher::clone_again(id, cx))
                .into_any_element(),
        );
    }
    buttons.push(remove_button(id, cx));
    frame(format!("Can't find \"{name}\""), details, buttons, cx)
}

pub fn unsafe_repository_view(
    id: u64,
    name: &str,
    unsafe_path: &Path,
    trusting: bool,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let path = unsafe_path.display().to_string();
    let trust_label = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        // `<Loading />` (`.octicon.spin`, 11 px)
        .when(trusting, |d| {
            d.child(
                div()
                    .size(zpx(11.))
                    .flex()
                    .items_center()
                    .child(loading("trusting-spinner", t.button_text)),
            )
        })
        .child(mac_or("Trust Repository", "Trust repository"));
    let details = div()
        // `.details`: centred paragraphs
        .my(SPACING())
        .flex()
        .flex_col()
        .items_center()
        .gap(SPACING())
        .child(
            paragraph(vec![
                "The Git repository at ".into(),
                Inline::Element(code_ref(path, cx).into_any_element()),
                " appears to be owned by another user on your machine. Adding untrusted \
                 repositories may automatically execute files in the repository."
                    .into(),
            ])
            .justify_center(),
        )
        .child(
            paragraph(vec![
                "If you trust the owner of the directory you can add an exception for this \
                 directory in order to continue."
                    .into(),
            ])
            .justify_center(),
        );
    let buttons = vec![
        primary_button("trust-repository", trust_label, trusting, cx)
            .min_w(BUTTON_MIN_WIDTH())
            .role(Role::Button)
            .aria_label(mac_or("Trust Repository", "Trust repository"))
            .when(!trusting, |b| {
                b.on_click(move |_, _, cx| Dispatcher::trust_repository(id, cx))
            })
            .into_any_element(),
        remove_button(id, cx),
    ];
    frame(
        format!("{name} is potentially unsafe"),
        details,
        buttons,
        cx,
    )
}

/// "Remove": GHD removes a missing repository without asking
/// (`removeRepository(repository, false)`).
fn remove_button(id: u64, cx: &App) -> AnyElement {
    button("remove-missing-repository", "Remove", cx)
        .min_w(BUTTON_MIN_WIDTH())
        .role(Role::Button)
        .aria_label("Remove")
        .on_click(move |_, _, cx| Dispatcher::remove_repository(id, cx))
        .into_any_element()
}

/// `.path` (monospace on `--path-segment-background`, `<Ref>`'s box) as one
/// chip per `/`-segment: `paragraph` attaches consecutive elements, so they
/// read as one span but the line can break before any `/` like GHD's inline
/// span wraps inside the path.
fn path_segments(path: &str, cx: &App) -> Vec<Inline> {
    let t = cx.ghd();
    let mut segments: Vec<&str> = Vec::new();
    let mut start = 0;
    for (i, _) in path.match_indices('/').filter(|(i, _)| *i > 0) {
        segments.push(&path[start..i]);
        start = i;
    }
    segments.push(&path[start..]);
    let last = segments.len() - 1;
    segments
        .into_iter()
        .enumerate()
        .map(|(i, segment)| {
            Inline::Element(
                div()
                    .font_family(crate::theme::mono_font())
                    .py(SPACING_THIRD() - zpx(2.))
                    .my(zpx(2.) - SPACING_THIRD())
                    .bg(t.path_segment_background)
                    .when(i == 0, |d| d.pl(SPACING_THIRD()).rounded_l(BORDER_RADIUS()))
                    .when(i == last, |d| {
                        d.pr(SPACING_THIRD()).rounded_r(BORDER_RADIUS())
                    })
                    .child(SharedString::from(segment.to_string()))
                    .into_any_element(),
            )
        })
        .collect()
}
