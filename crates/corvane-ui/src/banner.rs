//! Banners under the toolbar - GHD `ui/banners/*.tsx`
//! (`styles/ui/_banners.scss`, `banners/_successful.scss`,
//! `banners/_conflicts.scss`): a 30 px strip with a green check (successes)
//! or an alert icon (conflicts), the message with bold branch names, an
//! optional "Undo" / "View conflicts" link and, when dismissable, an ✕.
//! `update_banner` is GHD's `UpdateAvailable` banner.
//!
//! Deviation (`311-undo-delete-branch`): "Deleted branch" / "Restored
//! branch" banners, with an Undo that recreates the deleted branch, are
//! Corvane's (GHD deletes branches without a way back).

use corvane_core::{AvailableUpdate, Banner, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::widgets::{IconButtonA11y, ListRowA11y};

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::link_button;

#[allow(non_snake_case)]
pub fn BANNER_HEIGHT() -> Pixels {
    zpx(30.)
}

fn strong(text: impl Into<SharedString>) -> Div {
    div().font_weight(FontWeight::SEMIBOLD).child(text.into())
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "commit" } else { "commits" }
}

/// The message as text runs (`true` = bold, for branch names).
fn parts(banner: &Banner) -> Vec<(String, bool)> {
    let t = |s: &str| (s.to_string(), false);
    let b = |s: &String| (s.clone(), true);
    match banner {
        Banner::SuccessfulMerge {
            our_branch,
            their_branch,
        } => match their_branch {
            Some(their) => vec![
                t("Successfully merged\u{a0}"),
                b(their),
                t("\u{a0}into\u{a0}"),
                b(our_branch),
            ],
            None => vec![t("Successfully merged into\u{a0}"), b(our_branch)],
        },
        Banner::SuccessfulRebase {
            target_branch,
            base_branch,
        } => match base_branch {
            Some(base) => vec![
                t("Successfully rebased\u{a0}"),
                b(target_branch),
                t("\u{a0}onto\u{a0}"),
                b(base),
            ],
            None => vec![t("Successfully rebased\u{a0}"), b(target_branch)],
        },
        Banner::BranchAlreadyUpToDate {
            our_branch,
            their_branch,
        } => match their_branch {
            Some(their) => vec![
                b(our_branch),
                t("\u{a0}is already up to date with\u{a0}"),
                b(their),
            ],
            None => vec![b(our_branch), t("\u{a0}is already up to date")],
        },
        Banner::SuccessfulCherryPick {
            target_branch,
            count,
            ..
        } => vec![
            (
                format!("Successfully copied {count} {} to\u{a0}", plural(*count)),
                false,
            ),
            b(target_branch),
            t("."),
        ],
        Banner::CherryPickUndone {
            target_branch,
            count,
        } => vec![
            (
                format!(
                    "Cherry-pick undone. Successfully removed the {count} copied {} from\u{a0}",
                    plural(*count)
                ),
                false,
            ),
            b(target_branch),
            t("."),
        ],
        Banner::SuccessfulSquash { count, .. } => vec![(
            format!("Successfully squashed {count} {}.", plural(*count)),
            false,
        )],
        Banner::SquashUndone { count } => vec![(
            format!("Squash of {count} {} undone.", plural(*count)),
            false,
        )],
        Banner::SuccessfulReorder { count, .. } => vec![(
            format!("Successfully reordered {count} {}.", plural(*count)),
            false,
        )],
        Banner::ReorderUndone { count } => vec![(
            format!("Reorder of {count} {} undone.", plural(*count)),
            false,
        )],
        Banner::BranchDeleted { branch, .. } => vec![t("Deleted branch\u{a0}"), b(branch)],
        Banner::BranchRestored { branch } => vec![t("Restored branch\u{a0}"), b(branch)],
        Banner::ConflictsFound {
            description,
            branch,
            ..
        } => match branch {
            Some(branch) => vec![
                (
                    format!("Resolve conflicts to continue {description}\u{a0}"),
                    false,
                ),
                b(branch),
                t("."),
            ],
            None => vec![(
                format!("Resolve conflicts to continue {description}."),
                false,
            )],
        },
    }
}

/// The message line as a row of text runs (bold runs for branch names).
fn message(banner: &Banner) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .whitespace_nowrap()
        .children(parts(banner).into_iter().map(|(text, bold)| {
            if bold {
                strong(text).into_any_element()
            } else {
                div().child(text).into_any_element()
            }
        }))
}

/// The message as VoiceOver announces it.
fn plain_message(banner: &Banner) -> String {
    parts(banner)
        .into_iter()
        .map(|(text, _)| text)
        .collect::<String>()
        .replace('\u{a0}', " ")
}

/// `renderBanner`
pub fn banner_bar(banner: &Banner, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let is_conflicts = matches!(banner, Banner::ConflictsFound { .. });
    let icon = if is_conflicts {
        octicon(Octicon::Alert, t.text).mr(SPACING())
    } else {
        octicon(Octicon::CheckCircleFill, t.color_new).mr(SPACING())
    };
    let action: Option<AnyElement> = match banner {
        Banner::SuccessfulCherryPick { repo, .. }
        | Banner::SuccessfulSquash { repo, .. }
        | Banner::SuccessfulReorder { repo, .. } => {
            let repo = *repo;
            Some(
                link_button("banner-undo", "Undo", cx)
                    .ml(SPACING_HALF())
                    .on_click(move |_, _, cx| {
                        Dispatcher::clear_banner(cx);
                        Dispatcher::undo_mco(repo, cx);
                    })
                    .into_any_element(),
            )
        }
        Banner::BranchDeleted { repo, branch, sha } => {
            let (repo, branch, sha) = (*repo, branch.clone(), sha.clone());
            Some(
                link_button("banner-undo", "Undo", cx)
                    .ml(SPACING_HALF())
                    .on_click(move |_, _, cx| {
                        Dispatcher::clear_banner(cx);
                        Dispatcher::restore_deleted_branch(repo, branch.clone(), sha.clone(), cx);
                    })
                    .into_any_element(),
            )
        }
        Banner::ConflictsFound { repo, .. } => {
            let repo = *repo;
            Some(
                link_button("banner-view-conflicts", "View conflicts", cx)
                    .ml(SPACING_HALF())
                    .on_click(move |_, _, cx| Dispatcher::show_conflicts(repo, cx))
                    .into_any_element(),
            )
        }
        _ => None,
    };
    let close_color = t.text_secondary;
    let close_hover = t.text;
    div()
        .id("banner")
        // announced when it appears (GHD renders banners in an aria-live region)
        .a11y_live(plain_message(banner))
        .w_full()
        .h(BANNER_HEIGHT())
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .pl(SPACING())
        .overflow_hidden()
        .bg(t.background)
        .border_b_1()
        .border_color(t.box_border)
        .text_size(FONT_SIZE())
        .text_color(t.text)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .child(icon)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(div().min_w_0().truncate().child(message(banner)))
                        .children(action),
                ),
        )
        .when(banner.dismissable(), |d| {
            d.child(
                div()
                    .id("banner-close")
                    .icon_button_label("Dismiss this message")
                    .mx(SPACING())
                    .flex_none()
                    .size(zpx(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(close_color)
                    .hover(move |s| s.text_color(close_hover))
                    .on_click(|_, _, cx| Dispatcher::clear_banner(cx))
                    .child(octicon(Octicon::X, close_color)),
            )
        })
}

/// GHD `ui/banners/update-available.tsx` (`#update-available`,
/// `banners/_update-available.scss`): a desktop-download icon in the warning
/// icon colour, "Corvane N is available", "what's new" opens the release
/// notes and "install and restart" installs (`updateNow`). A Homebrew
/// install is told to `brew upgrade corvane` instead. Always dismissable
/// (Corvane has no prioritised updates).
pub fn update_banner(update: &AvailableUpdate, homebrew: bool, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let version = update.version.clone();
    let plain = if homebrew {
        format!(
            "Corvane {version} is available. Run brew upgrade corvane to install it, or see what's new."
        )
    } else {
        format!("Corvane {version} is available. See what's new or install and restart.")
    };
    let whats_new = link_button("update-banner-whats-new", "what's new", cx)
        .on_click(|_, _, cx| Dispatcher::show_update_release_notes(cx));
    let message = div()
        .flex()
        .flex_row()
        .items_center()
        .whitespace_nowrap()
        .child(format!("Corvane {version} is available.\u{a0}"))
        .map(|d| {
            if homebrew {
                d.child("Run\u{a0}")
                    .child(
                        div()
                            .font_family(crate::theme::mono_font())
                            .child("brew upgrade corvane"),
                    )
                    .child("\u{a0}to install it, or see\u{a0}")
                    .child(whats_new)
                    .child(".")
            } else {
                d.child("See\u{a0}")
                    .child(whats_new)
                    .child("\u{a0}or\u{a0}")
                    .child(
                        link_button("update-banner-install", "install and restart", cx)
                            .on_click(|_, _, cx| Dispatcher::install_update(cx)),
                    )
                    .child(".")
            }
        });
    let close_color = t.text_secondary;
    let close_hover = t.text;
    div()
        .id("update-available")
        .a11y_live(plain)
        .w_full()
        .h(BANNER_HEIGHT())
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .pl(SPACING())
        .overflow_hidden()
        .bg(t.background)
        .border_b_1()
        .border_color(t.box_border)
        .text_size(FONT_SIZE())
        .text_color(t.text)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .child(octicon(Octicon::DesktopDownload, t.banner_warning_icon).mr(SPACING()))
                .child(div().flex_1().min_w_0().truncate().child(message)),
        )
        .child(
            div()
                .id("update-banner-close")
                .icon_button_label("Dismiss this message")
                .mx(SPACING())
                .flex_none()
                .size(zpx(16.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(close_color)
                .hover(move |s| s.text_color(close_hover))
                .on_click(|_, _, cx| Dispatcher::dismiss_update_banner(cx))
                .child(octicon(Octicon::X, close_color)),
        )
}
