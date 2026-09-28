//! Unified diff viewer (`ui/diff/text-diff.tsx`, `styles/ui/_diff.scss`):
//! 29 px header with the path, then 20 px lines with two 50 px line-number
//! gutters, a marker column and monospace text.

use corvane_core::{Diff, DiffLineKind, FileStatusKind, WorkingDirectoryFileChange};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme, MONO_FONT};

pub const DIFF_LINE_HEIGHT: Pixels = px(20.);

/// Octicon + colour for a file status (`ui/octicons/status.ts`).
pub fn status_icon(kind: FileStatusKind, t: &GhdTheme) -> (Octicon, Hsla) {
    match kind {
        FileStatusKind::New | FileStatusKind::Untracked | FileStatusKind::Copied => {
            (Octicon::DiffAdded, t.color_new)
        }
        FileStatusKind::Modified => (Octicon::DiffModified, t.color_modified),
        FileStatusKind::Deleted => (Octicon::DiffRemoved, t.color_deleted),
        FileStatusKind::Renamed => (Octicon::DiffRenamed, t.color_renamed),
        FileStatusKind::Conflicted => (Octicon::Alert, t.color_conflicted),
    }
}

/// `.diff-header`: path (directory dimmed) + status icon, 29 px.
pub fn diff_header(file: &WorkingDirectoryFileChange, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let (icon, color) = status_icon(file.status.kind, t);
    div()
        .h(ROW_HEIGHT)
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .px(SPACING)
        .gap(SPACING)
        .bg(t.box_alt_background)
        .border_b_1()
        .border_color(t.box_border)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(FONT_SIZE)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .child(
                            div()
                                .text_color(t.text_secondary)
                                .child(file.directory().to_string()),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(file.file_name().to_string()),
                        ),
                ),
        )
        .child(octicon(Octicon::Gear, t.text_secondary))
        .child(octicon(icon, color))
}

fn gutter(number: Option<u32>, bg: Hsla, fg: Hsla, border: Hsla) -> Div {
    div()
        .w(DIFF_LINE_NUMBER_WIDTH)
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_end()
        .pr(SPACING_HALF)
        .bg(bg)
        .border_r_1()
        .border_color(border)
        .text_color(fg)
        .child(number.map(|n| n.to_string()).unwrap_or_default())
}

pub fn diff_view(diff: &Diff, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let base = div()
        .id("diff")
        .flex_1()
        .min_h_0()
        .w_full()
        .overflow_y_scroll()
        .bg(t.background)
        .font_family(MONO_FONT)
        .text_size(FONT_SIZE)
        .line_height(DIFF_LINE_HEIGHT)
        .text_color(t.diff_text);

    match diff {
        Diff::Text { hunks, truncated } => base
            .flex()
            .flex_col()
            .children(hunks.iter().flat_map(|hunk| {
                hunk.lines.iter().map(|line| {
                    let (bg, gutter_bg, gutter_fg, fg, marker) = match line.kind {
                        DiffLineKind::Hunk => (
                            t.diff_hunk_background,
                            t.diff_hunk_gutter_background,
                            t.diff_hunk_gutter,
                            t.diff_hunk_text,
                            "",
                        ),
                        DiffLineKind::Add => (
                            t.diff_add_background,
                            t.diff_add_gutter_background,
                            t.diff_add_gutter,
                            t.diff_add_text,
                            "+",
                        ),
                        DiffLineKind::Delete => (
                            t.diff_delete_background,
                            t.diff_delete_gutter_background,
                            t.diff_delete_gutter,
                            t.diff_delete_text,
                            "-",
                        ),
                        DiffLineKind::Context => (
                            t.background,
                            t.diff_gutter_background,
                            t.diff_line_number,
                            t.diff_text,
                            " ",
                        ),
                    };
                    let is_hunk = line.kind == DiffLineKind::Hunk;
                    div()
                        .w_full()
                        .min_h(DIFF_LINE_HEIGHT)
                        .flex()
                        .flex_row()
                        .items_stretch()
                        .bg(bg)
                        .text_color(fg)
                        .child(gutter(line.old_line, gutter_bg, gutter_fg, t.diff_gutter))
                        .child(gutter(line.new_line, gutter_bg, gutter_fg, t.diff_gutter))
                        .child(
                            div()
                                .w(px(20.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(marker),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .pr(SPACING)
                                .py(px(2.))
                                .when(is_hunk, |d| d.text_color(t.diff_hunk_text))
                                .child(if is_hunk {
                                    line.text.clone()
                                } else {
                                    line.text.replace('\t', "    ")
                                }),
                        )
                })
            }))
            .when(*truncated, |d| {
                d.child(
                    div()
                        .p(SPACING_DOUBLE)
                        .text_color(t.text_secondary)
                        .child("The diff is too large to show in full."),
                )
            })
            .into_any_element(),
        Diff::Binary => blankslate("This binary file has changed.", cx).into_any_element(),
        Diff::Empty => blankslate("No changes to show.", cx).into_any_element(),
        Diff::TooLarge => {
            blankslate("The diff is too large to be displayed by default.", cx).into_any_element()
        }
        Diff::Submodule => {
            blankslate("Submodule changes are not shown yet.", cx).into_any_element()
        }
    }
}

fn blankslate(message: &'static str, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .p(SPACING_DOUBLE)
        .text_color(t.text_secondary)
        .child(message)
}
