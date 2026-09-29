//! Row rendering for the unified diff (split out of `diff_view.rs`): pure
//! functions over a snapshot of the view state, so `gpui::list` can render
//! only the visible rows. Rows are built from the (possibly expanded) hunks
//! of `diff_expansion`; `Row::original` keeps the model's line index so
//! selections and discard patches ignore expanded context.

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::rc::Rc;

use corvane_core::{DiffLineKind, DiffSelection, DiffSelectionType, Dispatcher};
use corvane_highlight::{Span, TokenClass};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::diff_expansion::{ExpansionKind, HunkExpansionType, XHunk};
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

/// GHD `DiffRangeType`: what a block of consecutive changes contains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeType {
    Additions,
    Deletions,
    Mixed,
}

impl RangeType {
    /// GHD `getDiscardLabel`: "Discard Added Line…", "Discard Modified Lines".
    pub fn discard_label(self, lines: u32, confirm: bool) -> String {
        let kind = match self {
            RangeType::Additions => "Added",
            RangeType::Deletions => "Removed",
            RangeType::Mixed => "Modified",
        };
        let plural = if lines > 1 { "s" } else { "" };
        let suffix = if confirm { "…" } else { "" };
        format!("Discard {kind} Line{plural}{suffix}")
    }
}

/// One unified-diff row: its index in the expanded diff, its index in the
/// original diff (changed lines and original context only), and, for changed
/// lines, the block of consecutive changes it belongs to (`hunkStartLine`,
/// length, kind).
pub struct Row {
    pub abs: u32,
    pub original: Option<u32>,
    pub kind: DiffLineKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
    pub no_newline: bool,
    pub group: Option<(u32, u32)>,
    pub group_type: Option<RangeType>,
    pub hunk_index: usize,
    /// Hunk header rows: which expansion handles to show.
    pub expansion: HunkExpansionType,
}

impl Row {
    pub fn height(&self) -> Pixels {
        if self.kind == DiffLineKind::Hunk && self.expansion == HunkExpansionType::Both {
            DIFF_LINE_HEIGHT * 2.
        } else {
            DIFF_LINE_HEIGHT
        }
    }
}

/// GHD `temporarySelection`: a drag in progress over the line numbers
/// (original line indices).
#[derive(Clone, Copy, Debug)]
pub struct TempSelection {
    pub from: u32,
    pub to: u32,
    pub selected: bool,
}

/// One ⌘F hit: row index + byte range in the row text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub row: usize,
    pub range: Range<usize>,
}

/// Search hits grouped per row for rendering (`bool` = the selected hit).
pub struct SearchIndex {
    pub by_row: HashMap<usize, Vec<(Range<usize>, bool)>>,
}

/// Everything a row needs from the view, snapshotted per render.
pub struct RowContext {
    pub repo: u64,
    pub path: String,
    pub selection: DiffSelection,
    pub groups: BTreeMap<u32, DiffSelectionType>,
    pub selectable: bool,
    /// Diff Settings › Hide Whitespace Changes: line interaction shows a hint instead.
    pub hide_whitespace: bool,
    pub temp: Option<TempSelection>,
    pub hovered_group: Option<u32>,
    pub view: WeakEntity<DiffView>,
    /// Syntax spans per row (same indexing as the rows), once highlighted.
    pub tokens: Option<Rc<Vec<Vec<Span>>>>,
    pub search: Option<Rc<SearchIndex>>,
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

pub fn build_rows(hunks: &[XHunk]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for (hunk_index, hunk) in hunks.iter().enumerate() {
        let mut block_start: Option<usize> = None;
        for (i, xl) in hunk.lines.iter().enumerate() {
            let line = &xl.line;
            let changed = matches!(line.kind, DiffLineKind::Add | DiffLineKind::Delete);
            rows.push(Row {
                abs: hunk.unified_diff_start + i as u32,
                original: xl.original,
                kind: line.kind,
                old: line.old_line,
                new: line.new_line,
                text: line.text.replace('\t', "    "),
                no_newline: line.no_trailing_newline,
                group: None,
                group_type: None,
                hunk_index,
                expansion: if line.kind == DiffLineKind::Hunk {
                    hunk.expansion
                } else {
                    HunkExpansionType::None
                },
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
    let Some(first) = rows[start].original else {
        return;
    };
    let len = (end - start) as u32;
    let mut adds = false;
    let mut dels = false;
    for row in &rows[start..end] {
        match row.kind {
            DiffLineKind::Add => adds = true,
            DiffLineKind::Delete => dels = true,
            _ => {}
        }
    }
    let kind = match (adds, dels) {
        (true, false) => RangeType::Additions,
        (false, true) => RangeType::Deletions,
        _ => RangeType::Mixed,
    };
    for row in &mut rows[start..end] {
        row.group = Some((first, len));
        row.group_type = Some(kind);
    }
}

/// GHD `calcSearchTokens`: case-insensitive substring hits, hunk rows skipped.
pub fn search_rows(rows: &[Row], query: &str) -> Vec<SearchHit> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_ascii_lowercase();
    let mut hits = Vec::new();
    for (ix, row) in rows.iter().enumerate() {
        if row.kind == DiffLineKind::Hunk {
            continue;
        }
        let hay = row.text.to_ascii_lowercase();
        let mut from = 0;
        while let Some(pos) = hay[from..].find(&needle) {
            let start = from + pos;
            let end = start + needle.len();
            hits.push(SearchHit {
                row: ix,
                range: start..end,
            });
            from = end;
        }
    }
    hits
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

/// Syntax colours and search backgrounds as one sorted, non-overlapping
/// highlight list for `StyledText`.
fn merge_highlights(
    spans: &[Span],
    hits: &[(Range<usize>, bool)],
    len: usize,
    t: &GhdTheme,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut cuts: Vec<usize> = vec![0, len];
    for s in spans {
        cuts.push(s.range.start.min(len));
        cuts.push(s.range.end.min(len));
    }
    for (r, _) in hits {
        cuts.push(r.start.min(len));
        cuts.push(r.end.min(len));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = Vec::new();
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a >= b {
            continue;
        }
        let color = spans
            .iter()
            .find(|s| s.range.start <= a && s.range.end >= b)
            .map(|s| token_color(s.class, t));
        let hit = hits.iter().find(|(r, _)| r.start <= a && r.end >= b);
        if color.is_none() && hit.is_none() {
            continue;
        }
        let (background, fg) = match hit {
            // `.cm-search-result.cm-selected`: system Highlight
            Some((_, true)) => (
                Some(t.box_selected_active_background),
                Some(t.box_selected_active_text),
            ),
            // `.cm-search-result`: rgba(255, 255, 0, 0.4)
            Some((_, false)) => (Some(hsla(1. / 6., 1., 0.5, 0.4)), None),
            None => (None, None),
        };
        out.push((
            a..b,
            HighlightStyle {
                color: fg.or(color),
                background_color: background,
                ..Default::default()
            },
        ));
    }
    out
}

/// GHD `getHunkExpansionElementInfo`: icon, tooltip and target per handle.
fn expansion_handle(
    ctx: &RowContext,
    row: &Row,
    kind: HunkExpansionType,
    width: f32,
    height: Pixels,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let (icon, title, target, direction) = match kind {
        HunkExpansionType::Up => (
            Octicon::FoldUp,
            "Expand Up",
            row.hunk_index,
            ExpansionKind::Up,
        ),
        HunkExpansionType::Down => (
            Octicon::FoldDown,
            "Expand Down",
            row.hunk_index.saturating_sub(1),
            ExpansionKind::Down,
        ),
        HunkExpansionType::Short => (
            Octicon::Fold,
            "Expand All",
            row.hunk_index,
            ExpansionKind::Up,
        ),
        _ => {
            return div()
                .w(px(width))
                .h(height)
                .flex_none()
                .bg(t.diff_hunk_gutter_background)
                .into_any_element();
        }
    };
    let view = ctx.view.clone();
    let view_for_menu = ctx.view.clone();
    let hover_bg = t.diff_hover_background;
    let hover_text = t.diff_hover_text;
    div()
        .id(("hunk-expansion", row.abs as usize * 2 + direction as usize))
        .w(px(width))
        .h(height)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .bg(t.diff_hunk_gutter_background)
        .text_color(t.diff_hunk_text)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).text_color(hover_text))
        .tooltip(crate::widgets::tooltip(title))
        .on_click(move |_, _, cx| {
            view.update(cx, |this, cx| this.expand(target, direction, cx))
                .ok();
        })
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            view_for_menu
                .update(cx, |this, cx| this.expand_menu(ev.position, window, cx))
                .ok();
        })
        .child(octicon(icon, t.diff_hunk_text).size(px(16.)))
        .into_any_element()
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
    let view_for_text_menu = ctx.view.clone();
    let content = div()
        .id(("diff-text", abs as usize))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_row()
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            view_for_text_menu
                .update(cx, |this, cx| this.expand_menu(ev.position, window, cx))
                .ok();
        })
        .child(div().flex_none().whitespace_nowrap().child(prefix))
        .child(div().flex_1().min_w_0().child({
            let spans: &[Span] = ctx
                .tokens
                .as_ref()
                .and_then(|tk| tk.get(ix))
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let hits: &[(Range<usize>, bool)] = ctx
                .search
                .as_ref()
                .and_then(|s| s.by_row.get(&ix))
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let text = SharedString::from(row.text.clone());
            if spans.is_empty() && hits.is_empty() {
                text.into_any_element()
            } else {
                let highlights = merge_highlights(spans, hits, row.text.len(), t);
                StyledText::new(text)
                    .with_highlights(highlights)
                    .into_any_element()
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
    let height = row.height();

    let mut el = div()
        .id(("diff-row", abs as usize))
        .min_h(height)
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .items_stretch()
        .bg(row_bg)
        .text_color(row_text);

    if row.kind == DiffLineKind::Hunk {
        // `.hunk-info`: the gutter holds the expansion handle(s)
        let width = handle_width + gutter_width + 1.;
        let gutter: AnyElement = match row.expansion {
            HunkExpansionType::Both => div()
                .flex_none()
                .flex()
                .flex_col()
                .child(expansion_handle(
                    ctx,
                    row,
                    HunkExpansionType::Down,
                    width,
                    DIFF_LINE_HEIGHT,
                    cx,
                ))
                .child(expansion_handle(
                    ctx,
                    row,
                    HunkExpansionType::Up,
                    width,
                    DIFF_LINE_HEIGHT,
                    cx,
                ))
                .into_any_element(),
            kind => expansion_handle(ctx, row, kind, width, height, cx),
        };
        return el
            .child(gutter)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .child(content),
            )
            .into_any_element();
    }

    // ---- hunk handle strip (16 px) ----
    if selectable {
        let strip = if let (Some((start, len)), Some(group_type)) = (row.group, row.group_type) {
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
            let view_for_menu = ctx.view.clone();
            let view_for_hint = ctx.view.clone();
            let hide_whitespace = ctx.hide_whitespace;
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
                .on_click(move |ev: &ClickEvent, _, cx| {
                    if hide_whitespace {
                        view_for_hint
                            .update(cx, |this, cx| this.show_whitespace_hint(ev.position(), cx))
                            .ok();
                        return;
                    }
                    Dispatcher::set_diff_lines(
                        repo,
                        path_for_click.clone(),
                        start,
                        len,
                        kind != DiffSelectionType::All,
                        cx,
                    )
                })
                .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
                    view_for_menu
                        .update(cx, |this, cx| {
                            this.hunk_menu(start, len, group_type, ev.position, window, cx)
                        })
                        .ok();
                })
                .when(
                    row.original == Some(start) && len > 1 && ctx.show_check_marks,
                    |d| {
                        d.flex().justify_center().items_start().pt(px(3.)).children(
                            match kind {
                                DiffSelectionType::All => Some(Octicon::DiffCheck),
                                DiffSelectionType::Partial => Some(Octicon::DiffDash),
                                DiffSelectionType::None => None,
                            }
                            .map(|icon| octicon(icon, white()).size(px(12.))),
                        )
                    },
                )
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
    let original = row.original.unwrap_or(abs);
    let selected = selectable && changed && is_selected(&ctx.selection, ctx.temp, original);
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
    let stored_selected = ctx.selection.is_selected(original);
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
    let view_for_menu = ctx.view.clone();
    let hide_whitespace = ctx.hide_whitespace;
    let group_type = row.group_type;
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
                .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                    view_for_down
                        .update(cx, |this, cx| {
                            if hide_whitespace {
                                this.show_whitespace_hint(ev.position, cx);
                                return;
                            }
                            this.start_selection(
                                TempSelection {
                                    from: original,
                                    to: original,
                                    selected: !stored_selected,
                                },
                                cx,
                            )
                        })
                        .ok();
                })
                .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
                    if hide_whitespace {
                        return;
                    }
                    if let Some(group_type) = group_type {
                        view_for_menu
                            .update(cx, |this, cx| {
                                this.line_menu(original, group_type, ev.position, window, cx)
                            })
                            .ok();
                    }
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
    if selectable && changed && !hide_whitespace {
        // extend the drag as the pointer crosses changed rows
        let view = ctx.view.clone();
        el = el.on_mouse_move(move |_, _, cx| {
            view.update(cx, |this, cx| this.extend_selection(original, cx))
                .ok();
        });
    }
    el.into_any_element()
}

#[cfg(test)]
mod tests {
    // explicit imports: `gpui_kit::*` would shadow `#[test]` with GPUI's macro
    use super::{RangeType, SearchHit, build_rows, search_rows};
    use corvane_core::{DiffHunk, DiffLine, DiffLineKind};

    fn hunk() -> DiffHunk {
        let l = |kind, text: &str| DiffLine {
            kind,
            text: text.into(),
            old_line: None,
            new_line: None,
            no_trailing_newline: false,
        };
        DiffHunk {
            unified_diff_start: 0,
            header: "@@ -1,3 +1,3 @@".into(),
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 3,
            lines: vec![
                l(DiffLineKind::Hunk, "@@ -1,3 +1,3 @@"),
                l(DiffLineKind::Context, "Alpha"),
                l(DiffLineKind::Delete, "beta"),
                l(DiffLineKind::Add, "Beta"),
                l(DiffLineKind::Add, "gamma"),
                l(DiffLineKind::Context, "delta"),
            ],
        }
    }

    #[test]
    fn groups_carry_original_indices_and_kind() {
        let x = crate::diff_expansion::from_hunks(&[hunk()], None);
        let rows = build_rows(&x);
        assert_eq!(rows[2].group, Some((2, 3)));
        assert_eq!(rows[2].group_type, Some(RangeType::Mixed));
        assert_eq!(rows[4].original, Some(4));
        assert_eq!(rows[1].group, None);
    }

    #[test]
    fn search_is_case_insensitive_and_skips_hunk_rows() {
        let x = crate::diff_expansion::from_hunks(&[hunk()], None);
        let rows = build_rows(&x);
        let hits = search_rows(&rows, "BETA");
        assert_eq!(
            hits,
            vec![
                SearchHit {
                    row: 2,
                    range: 0..4
                },
                SearchHit {
                    row: 3,
                    range: 0..4
                }
            ]
        );
        assert!(search_rows(&rows, "@@").is_empty());
        assert!(search_rows(&rows, "").is_empty());
    }

    #[test]
    fn discard_labels_match_ghd() {
        assert_eq!(
            RangeType::Additions.discard_label(1, true),
            "Discard Added Line…"
        );
        assert_eq!(
            RangeType::Mixed.discard_label(3, false),
            "Discard Modified Lines"
        );
        assert_eq!(
            RangeType::Deletions.discard_label(2, true),
            "Discard Removed Lines…"
        );
    }
}
