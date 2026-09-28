//! Row rendering for the unified diff (split out of `diff_view.rs`): pure
//! functions over a snapshot of the view state, so `gpui::list` can render
//! only the visible rows.

use std::collections::BTreeMap;
use std::rc::Rc;

use corvane_core::{DiffHunk, DiffLineKind, DiffSelection, DiffSelectionType, Dispatcher};
use corvane_highlight::{Span, TokenClass};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::diff_view::{DIFF_LINE_HEIGHT, DiffView};
use crate::icons::{Octicon, octicon};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme};

/// `--hunk-handle-width-with-check-all`
pub const HANDLE_WIDTH: f32 = 16.;
/// `.line-number-check`
pub const CHECK_WIDTH: f32 = 20.;
/// `--width-line-number`
pub const LINE_NUMBER_WIDTH: f32 = 55.;

/// One unified-diff row with its absolute line index and, for changed lines,
/// the block of consecutive changes it belongs to (`hunkStartLine`, length).
pub struct Row {
    pub abs: u32,
    pub kind: DiffLineKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
    pub no_newline: bool,
    pub group: Option<(u32, u32)>,
}

/// GHD `temporarySelection`: a drag in progress over the line numbers.
#[derive(Clone, Copy, Debug)]
pub struct TempSelection {
    pub from: u32,
    pub to: u32,
    pub selected: bool,
}

/// Everything a row needs from the view, snapshotted per render.
pub struct RowContext {
    pub repo: u64,
    pub path: String,
    pub selection: DiffSelection,
    pub groups: BTreeMap<u32, DiffSelectionType>,
    pub selectable: bool,
    pub temp: Option<TempSelection>,
    pub hovered_group: Option<u32>,
    pub view: WeakEntity<DiffView>,
    /// Syntax spans per row (same indexing as the rows), once highlighted.
    pub tokens: Option<Rc<Vec<Vec<Span>>>>,
    /// Settings › Accessibility › Show check marks in the diff.
    pub show_check_marks: bool,
}

/// `.cm-s-default` colours; classes that inherit are not emitted by the highlighter.
pub fn token_color(class: TokenClass, t: &GhdTheme) -> Hsla {
    match class {
        TokenClass::Variable => t.syntax_variable,
        TokenClass::AltVariable => t.syntax_alt_variable,
        TokenClass::Keyword => t.syntax_keyword,
        TokenClass::Atom => t.syntax_atom,
        TokenClass::String => t.syntax_string,
        TokenClass::Qualifier => t.syntax_qualifier,
        TokenClass::Type => t.syntax_type,
        TokenClass::Comment => t.syntax_comment,
        TokenClass::Tag => t.syntax_tag,
        TokenClass::Attribute => t.syntax_attribute,
        TokenClass::Link => t.syntax_link,
        TokenClass::Header => t.syntax_header,
        TokenClass::Quote => t.syntax_quote,
    }
}

pub fn build_rows(hunks: &[DiffHunk]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for hunk in hunks {
        let mut block_start: Option<usize> = None;
        for (i, line) in hunk.lines.iter().enumerate() {
            let changed = matches!(line.kind, DiffLineKind::Add | DiffLineKind::Delete);
            rows.push(Row {
                abs: hunk.unified_diff_start + i as u32,
                kind: line.kind,
                old: line.old_line,
                new: line.new_line,
                text: line.text.replace('\t', "    "),
                no_newline: line.no_trailing_newline,
                group: None,
            });
            if changed {
                block_start.get_or_insert(rows.len() - 1);
            } else if let Some(start) = block_start.take() {
                let end = rows.len() - 1;
                close_block(&mut rows, start, end);
            }
        }
        if let Some(start) = block_start {
            let end = rows.len();
            close_block(&mut rows, start, end);
        }
    }
    rows
}

fn close_block(rows: &mut [Row], start: usize, end: usize) {
    let first = rows[start].abs;
    let len = (end - start) as u32;
    for row in &mut rows[start..end] {
        row.group = Some((first, len));
    }
}

/// `isInSelection`: stored selection combined with the drag in progress.
pub fn is_selected(sel: &DiffSelection, temp: Option<TempSelection>, line: u32) -> bool {
    let stored = sel.is_selected(line);
    match temp {
        None => stored,
        Some(t) => {
            let in_temp = line >= t.from.min(t.to) && line <= t.from.max(t.to);
            if t.selected {
                stored || in_temp
            } else {
                stored && !in_temp
            }
        }
    }
}

pub fn render_row(ctx: &RowContext, ix: usize, row: &Row, cx: &App) -> AnyElement {
    let t = cx.ghd();
    let abs = row.abs;
    let changed = matches!(row.kind, DiffLineKind::Add | DiffLineKind::Delete);
    let selectable = ctx.selectable;
    let (row_bg, row_text, gutter_bg, gutter_border) = match row.kind {
        DiffLineKind::Add => (
            t.diff_add_background,
            t.diff_add_text,
            t.diff_add_gutter_background,
            t.diff_add_border,
        ),
        DiffLineKind::Delete => (
            t.diff_delete_background,
            t.diff_delete_text,
            t.diff_delete_gutter_background,
            t.diff_delete_border,
        ),
        DiffLineKind::Context => (
            t.background,
            t.diff_text,
            t.diff_gutter_background,
            t.diff_border,
        ),
        DiffLineKind::Hunk => (
            t.diff_hunk_background,
            t.diff_hunk_text,
            t.diff_hunk_gutter_background,
            t.diff_hunk_gutter_background,
        ),
    };
    let prefix = match row.kind {
        DiffLineKind::Add => "  +  ",
        DiffLineKind::Delete => "  -  ",
        _ => "     ",
    };
    let content = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_row()
        .child(div().flex_none().whitespace_nowrap().child(prefix))
        .child(div().flex_1().min_w_0().child({
            let spans = ctx.tokens.as_ref().and_then(|tk| tk.get(ix));
            let text = SharedString::from(row.text.clone());
            match spans {
                Some(spans) if !spans.is_empty() => StyledText::new(text)
                    .with_highlights(spans.iter().map(|s| {
                        (
                            s.range.clone(),
                            HighlightStyle {
                                color: Some(token_color(s.class, t)),
                                ..Default::default()
                            },
                        )
                    }))
                    .into_any_element(),
                _ => text.into_any_element(),
            }
        }))
        .when(row.no_newline, |d| {
            d.child(
                div()
                    .flex_none()
                    .italic()
                    .ml(px(4.))
                    .text_color(t.diff_alt_text)
                    .child("No newline at end of file"),
            )
        });
    let handle_width = if selectable { HANDLE_WIDTH } else { 0. };
    let gutter_width = if selectable { CHECK_WIDTH } else { 0. } + 2. * LINE_NUMBER_WIDTH;

    let mut el = div()
        .id(("diff-row", abs as usize))
        .min_h(DIFF_LINE_HEIGHT)
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .items_stretch()
        .bg(row_bg)
        .text_color(row_text);

    if row.kind == DiffLineKind::Hunk {
        // `.hunk-info`: expansion handle (placeholder) spanning handle + gutter.
        return el
            .child(
                div()
                    .w(px(handle_width + gutter_width + 1.))
                    .flex_none()
                    .bg(gutter_bg),
            )
            .child(content)
            .into_any_element();
    }

    // ---- hunk handle strip (16 px) ----
    if selectable {
        let strip = if let Some((start, len)) = row.group {
            let kind = ctx
                .groups
                .get(&start)
                .copied()
                .unwrap_or(DiffSelectionType::None);
            let bg = if kind != DiffSelectionType::None {
                t.diff_selected_border
            } else {
                t.diff_empty_hunk_handle
            };
            let repo = ctx.repo;
            let path_for_click = ctx.path.clone();
            let view = ctx.view.clone();
            div()
                .id(("hunk-handle", abs as usize))
                .w(px(HANDLE_WIDTH))
                .flex_none()
                .bg(bg)
                .cursor_pointer()
                .on_hover(move |hovered: &bool, _, cx| {
                    let next = if *hovered { Some(start) } else { None };
                    view.update(cx, |this, cx| this.set_hovered_group(next, cx))
                        .ok();
                })
                .on_click(move |_, _, cx| {
                    Dispatcher::set_diff_lines(
                        repo,
                        path_for_click.clone(),
                        start,
                        len,
                        kind != DiffSelectionType::All,
                        cx,
                    )
                })
                .when(abs == start && len > 1 && ctx.show_check_marks, |d| {
                    d.flex().justify_center().items_start().pt(px(3.)).children(
                        match kind {
                            DiffSelectionType::All => Some(Octicon::DiffCheck),
                            DiffSelectionType::Partial => Some(Octicon::DiffDash),
                            DiffSelectionType::None => None,
                        }
                        .map(|icon| octicon(icon, white()).size(px(12.))),
                    )
                })
                .into_any_element()
        } else {
            // `.editable .row.context { border-left: 16px solid diff-border }`
            div()
                .w(px(HANDLE_WIDTH))
                .flex_none()
                .bg(t.diff_border)
                .into_any_element()
        };
        el = el.child(strip);
    }

    // ---- line numbers (`.line-number`) ----
    let selected = selectable && changed && is_selected(&ctx.selection, ctx.temp, abs);
    let group_hover = selectable
        && row
            .group
            .map(|(s, _)| ctx.hovered_group == Some(s))
            .unwrap_or(false);
    let base = (gutter_bg, gutter_border, t.diff_line_number);
    let normal = if selected {
        (
            t.diff_selected_background,
            t.diff_selected_border,
            t.diff_selected_text,
        )
    } else {
        base
    };
    let hover = if selected {
        (
            t.diff_hover_background,
            t.diff_hover_border,
            t.diff_hover_text,
        )
    } else if row.kind == DiffLineKind::Add {
        (
            t.diff_add_hover_background,
            t.diff_add_hover_border,
            t.diff_add_hover_text,
        )
    } else {
        (
            t.diff_delete_hover_background,
            t.diff_delete_hover_border,
            t.diff_delete_hover_text,
        )
    };
    let (num_bg, num_border, num_text) = if group_hover && changed {
        hover
    } else {
        normal
    };
    let stored_selected = ctx.selection.is_selected(abs);
    let number = |n: Option<u32>| {
        div()
            .flex_1()
            .flex()
            .justify_end()
            .items_center()
            .px(SPACING_HALF)
            .child(n.map(|n| n.to_string()).unwrap_or_default())
    };
    let view_for_down = ctx.view.clone();
    let gutter = div()
        .id(("diff-gutter", abs as usize))
        .w(px(gutter_width))
        .flex_none()
        .flex()
        .flex_row()
        .items_stretch()
        .bg(num_bg)
        .border_r_1()
        .border_color(num_border)
        .text_color(num_text)
        .when(selectable && changed, |d| {
            d.cursor_pointer()
                .hover(move |s| s.bg(hover.0).border_color(hover.1).text_color(hover.2))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    view_for_down
                        .update(cx, |this, cx| {
                            this.start_selection(
                                TempSelection {
                                    from: abs,
                                    to: abs,
                                    selected: !stored_selected,
                                },
                                cx,
                            )
                        })
                        .ok();
                })
        })
        .when(selectable, |d| {
            d.child(
                div()
                    .w(px(CHECK_WIDTH))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .items_center()
                    .when(selected && ctx.show_check_marks, |d| {
                        d.child(octicon(Octicon::DiffCheck, num_text).size(px(12.)))
                    }),
            )
        })
        .child(number(row.old).border_r_1().border_color(num_border))
        .child(number(row.new));

    el = el.child(gutter).child(content);
    if selectable && changed {
        // extend the drag as the pointer crosses changed rows
        let view = ctx.view.clone();
        el = el.on_mouse_move(move |_, _, cx| {
            view.update(cx, |this, cx| this.extend_selection(abs, cx))
                .ok();
        });
    }
    el.into_any_element()
}
