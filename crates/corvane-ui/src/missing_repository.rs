//! `#missing-repository-view` - GHD `ui/missing-repository.tsx`
//! (`styles/ui/_missing-repository-view.scss`), the variant for a
//! repository git refuses to work in ("detected dubious ownership"):
//! "<name> is potentially unsafe", the directory git named and the "Trust
//! Repository" (`git config --global --add safe.directory`) and "Remove"
//! buttons.
//!
//! The "Can't find" variant for a deleted directory (Locate…, Clone Again,
//! Check again) is not built yet.

use std::path::Path;

use corvane_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::loading;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{Inline, button, code_ref, paragraph, primary_button};

/// `button { min-width: 120px }`
#[allow(non_snake_case)]
fn BUTTON_MIN_WIDTH() -> Pixels {
    zpx(120.)
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
        .child("Trust Repository");
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
                                .child(format!("{name} is potentially unsafe")),
                        )
                        .child(
                            // `.details`: centred paragraphs
                            div()
                                .my(SPACING())
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(SPACING())
                                .child(
                                    paragraph(vec![
                                        "The Git repository at".into(),
                                        Inline::Element(code_ref(path, cx).into_any_element()),
                                        " appears to be owned by another user on your machine. \
                                         Adding untrusted repositories may automatically execute \
                                         files in the repository."
                                            .into(),
                                    ])
                                    .justify_center(),
                                )
                                .child(
                                    paragraph(vec![
                                        "If you trust the owner of the directory you can add an \
                                         exception for this directory in order to continue."
                                            .into(),
                                    ])
                                    .justify_center(),
                                ),
                        ),
                )
                .child(
                    // `<Row>` of buttons
                    div()
                        .flex()
                        .flex_row()
                        .gap(SPACING())
                        .child(
                            primary_button("trust-repository", trust_label, trusting, cx)
                                .min_w(BUTTON_MIN_WIDTH())
                                .role(Role::Button)
                                .aria_label("Trust Repository")
                                .when(!trusting, |b| {
                                    b.on_click(move |_, _, cx| Dispatcher::trust_repository(id, cx))
                                }),
                        )
                        .child(
                            button("remove-unsafe-repository", "Remove", cx)
                                .min_w(BUTTON_MIN_WIDTH())
                                .role(Role::Button)
                                .aria_label("Remove")
                                .on_click(move |_, _, cx| Dispatcher::remove_repository(id, cx)),
                        ),
                ),
        )
}
