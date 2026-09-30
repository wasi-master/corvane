//! Row rendering for the unified diff (split out of `diff_view.rs`): pure
//! functions over a snapshot of the view state, so `gpui::list` can render
//! only the visible rows. Rows are built from the (possibly expanded) hunks
//! of `diff_expansion`; `Row::original` keeps the model's line index so
//! selections and discard patches ignore expanded context.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::rc::Rc;

use corvane_core::{DiffLineKind, DiffSelection, DiffSelectionType, Dispatcher};
use corvane_highlight::{Span, TokenClass};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::widgets::GhdTooltip;
use crate::widgets::IconButtonA11y;

use crate::diff_expansion::{ExpansionKind, HunkExpansionType, XHunk};
use crate::diff_view::{DIFF_LINE_HEIGHT, DiffView, TextSelectionSnapshot};
use crate::icons::{Octicon, octicon};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme};

/// `--hunk-handle-width-with-check-all`
pub const HANDLE_WIDTH: f32 = 16.;
/// `.line-number-check`
pub const CHECK_WIDTH: f32 = 20.;

/// GHD `getLineWidthFromDigitCount(getNumberOfDigits(diff.maxLineNumber))`
/// (`diff-helpers.tsx`): one line-number column, sized for the largest line
/// number in the diff. `--width-line-number: 55px` is only the CSS default;
/// the row overrides it with this inline width.
pub fn line_number_width(max_line_number: u32) -> f32 {
    let digits = max_line_number.checked_ilog10().map_or(1, |d| d + 1);
    digits.max(3) as f32 * 10. + 5.
}

/// The largest old/new line number among `rows` (GHD `diff.maxLineNumber`).
/// Both sides only grow down the diff, so the last numbered rows hold them.
pub fn max_line_number(rows: &[Row]) -> u32 {
    let (mut old, mut new) = (None, None);
    for row in rows.iter().rev() {
        old = old.or(row.old);
        new = new.or(row.new);
        if old.is_some() && new.is_some() {
            break;
        }
    }
    old.unwrap_or(0).max(new.unwrap_or(0))
}

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
        match (self.kind, self.expansion) {
            (DiffLineKind::Hunk, HunkExpansionType::Both) => DIFF_LINE_HEIGHT() * 2.,
            // `.hunk-expansion-handle button`: 16 px icon + 4 / 1 px padding +
            // 1 px margins grows the row to 23 px (measured in GHD 3.6.6);
            // a hunk without a handle stays 20 px
            (DiffLineKind::Hunk, HunkExpansionType::Up)
            | (DiffLineKind::Hunk, HunkExpansionType::Down)
            | (DiffLineKind::Hunk, HunkExpansionType::Short) => zpx(23.),
            _ => DIFF_LINE_HEIGHT(),
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
    /// `lineNumberWidth`: one line-number column (`line_number_width`).
    pub line_number_width: f32,
    /// The text selection (ordered), if any.
    pub text_selection: Option<TextSelectionSnapshot>,
    pub text_bounds: TextBounds,
}

impl RowContext {
    /// The selected byte range of the text at (`list_ix`, `column`).
    fn selection_range(&self, list_ix: usize, column: Column, len: usize) -> Option<Range<usize>> {
        let sel = self.text_selection.as_ref()?;
        if sel.column != column || list_ix < sel.start.row || list_ix > sel.end.row {
            return None;
        }
        let start = if list_ix == sel.start.row {
            sel.start.col.min(len)
        } else {
            0
        };
        let end = if list_ix == sel.end.row {
            sel.end.col.min(len)
        } else {
            len
        };
        (start < end).then_some(start..end)
    }
}

/// The selectable text of a row: records its bounds for hit-testing, starts
/// a text selection on mouse down (shift extends) and paints the selection.
fn selectable_text(
    ctx: &RowContext,
    list_ix: usize,
    column: Column,
    text: &str,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
) -> Div {
    let bounds = ctx.text_bounds.clone();
    let view = ctx.view.clone();
    let body: AnyElement = if highlights.is_empty() {
        SharedString::from(text.to_string()).into_any_element()
    } else {
        StyledText::new(SharedString::from(text.to_string()))
            .with_highlights(highlights)
            .into_any_element()
    };
    div()
        .flex_1()
        .min_w_0()
        .relative()
        .cursor_text()
        .child(
            canvas(
                move |b, _, _| {
                    bounds.borrow_mut().insert((list_ix, column), b);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .child(body)
        .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
            view.update(cx, |this, cx| {
                this.start_text_selection(
                    list_ix,
                    column,
                    ev.position,
                    ev.modifiers.shift,
                    window,
                    cx,
                )
            })
            .ok();
        })
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

/// Syntax colours, search backgrounds and the intra-line change range
/// (`diff-add-inner` / `diff-delete-inner`) as one sorted, non-overlapping
/// highlight list for `StyledText`.
fn merge_highlights(
    spans: &[Span],
    hits: &[(Range<usize>, bool)],
    inner: Option<(Range<usize>, Hsla)>,
    selection: Option<Range<usize>>,
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
    if let Some((r, _)) = &inner {
        cuts.push(r.start.min(len));
        cuts.push(r.end.min(len));
    }
    if let Some(r) = &selection {
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
        let inner_bg = inner
            .as_ref()
            .filter(|(r, _)| r.start <= a && r.end >= b)
            .map(|(_, c)| *c);
        let selected = selection
            .as_ref()
            .is_some_and(|r| r.start <= a && r.end >= b);
        if color.is_none() && hit.is_none() && inner_bg.is_none() && !selected {
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
            None => (inner_bg, None),
        };
        // the text selection paints over everything else
        let background = if selected {
            Some(t.text_selection_background)
        } else {
            background
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
                .w(zpx(width))
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
        .w(zpx(width))
        .h(height)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .bg(t.diff_hunk_gutter_background)
        .text_color(t.diff_hunk_text)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).text_color(hover_text))
        .a11y_button(title)
        .ghd_tooltip(title)
        .on_click(move |_, _, cx| {
            view.update(cx, |this, cx| this.expand(target, direction, cx))
                .ok();
        })
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            view_for_menu
                .update(cx, |this, cx| this.expand_menu(ev.position, window, cx))
                .ok();
        })
        .child(octicon(icon, t.diff_hunk_text).size(zpx(16.)))
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
                .update(cx, |this, cx| this.text_menu(ev.position, window, cx))
                .ok();
        })
        .child(div().flex_none().whitespace_nowrap().child(prefix))
        .child({
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
            let selection = ctx.selection_range(ix, Column::Before, row.text.len());
            let highlights = if spans.is_empty() && hits.is_empty() && selection.is_none() {
                Vec::new()
            } else {
                merge_highlights(spans, hits, None, selection, row.text.len(), t)
            };
            selectable_text(ctx, ix, Column::Before, &row.text, highlights)
        })
        .when(row.no_newline, |d| {
            d.child(
                div()
                    .flex_none()
                    .italic()
                    .ml(zpx(4.))
                    .text_color(t.diff_alt_text)
                    .child("No newline at end of file"),
            )
        });
    // `.has-check-all-control`: 16 px strip with check marks, 4 px without
    let check_marks = selectable && ctx.show_check_marks;
    let handle_width = match (selectable, check_marks) {
        (false, _) => 0.,
        (true, true) => HANDLE_WIDTH,
        (true, false) => 4.,
    };
    // `lineGutterWidth`: both columns, plus the check column when shown
    let gutter_width = if check_marks { CHECK_WIDTH } else { 0. } + 2. * ctx.line_number_width;
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
                    DIFF_LINE_HEIGHT(),
                    cx,
                ))
                .child(expansion_handle(
                    ctx,
                    row,
                    HunkExpansionType::Up,
                    width,
                    DIFF_LINE_HEIGHT(),
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
                .w(zpx(handle_width))
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
                        d.flex()
                            .justify_center()
                            .items_start()
                            .pt(zpx(3.))
                            .children(
                                match kind {
                                    DiffSelectionType::All => Some(Octicon::DiffCheck),
                                    DiffSelectionType::Partial => Some(Octicon::DiffDash),
                                    DiffSelectionType::None => None,
                                }
                                .map(|icon| octicon(icon, white()).size(zpx(12.))),
                            )
                    },
                )
                .into_any_element()
        } else {
            // `.editable .row.context { border-left: 16px solid diff-border }`
            div()
                .w(zpx(handle_width))
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
            .px(SPACING_HALF())
            .child(n.map(|n| n.to_string()).unwrap_or_default())
    };
    let view_for_down = ctx.view.clone();
    let view_for_menu = ctx.view.clone();
    let hide_whitespace = ctx.hide_whitespace;
    let group_type = row.group_type;
    let gutter = div()
        .id(("diff-gutter", abs as usize))
        .w(zpx(gutter_width))
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
        .when(check_marks, |d| {
            d.child(
                div()
                    .w(zpx(CHECK_WIDTH))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .items_center()
                    .when(selected, |d| {
                        d.child(octicon(Octicon::DiffCheck, num_text).size(zpx(12.)))
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

// ---- split (side-by-side) rows ----

/// GHD `MaxIntraLineDiffStringLength`.
pub const MAX_INTRA_LINE_DIFF_LEN: usize = 1024;

/// GHD `relativeChanges`: the byte ranges of `a` and `b` that remain once the
/// common prefix and suffix are stripped.
pub fn relative_changes(a: &str, b: &str) -> (Range<usize>, Range<usize>) {
    let ac: Vec<(usize, char)> = a.char_indices().collect();
    let bc: Vec<(usize, char)> = b.char_indices().collect();
    let max = ac.len().min(bc.len());
    let mut prefix = 0;
    while prefix < max && ac[prefix].1 == bc[prefix].1 {
        prefix += 1;
    }
    let remaining = max - prefix;
    let mut suffix = 0;
    while suffix < remaining && ac[ac.len() - 1 - suffix].1 == bc[bc.len() - 1 - suffix].1 {
        suffix += 1;
    }
    let range = |chars: &[(usize, char)], text: &str| {
        let start = chars.get(prefix).map(|c| c.0).unwrap_or(text.len());
        let end = if suffix == 0 {
            text.len()
        } else {
            chars[chars.len() - suffix].0
        };
        start..end.max(start)
    };
    (range(&ac, a), range(&bc, b))
}

/// One side of a split row: the unified row it shows and, for paired
/// modified lines, the changed range highlighted with the inner colour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitSide {
    pub unified: usize,
    pub inner: Option<Range<usize>>,
}

/// GHD `DiffRow` in side-by-side mode (`getDiffRowsFromHunk` / `getModifiedRows`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SplitRow {
    Hunk { unified: usize },
    Context { unified: usize },
    Added { after: SplitSide },
    Deleted { before: SplitSide },
    Modified { before: SplitSide, after: SplitSide },
}

impl SplitRow {
    /// The unified rows this split row shows (before, after).
    pub fn unified_rows(&self) -> (Option<usize>, Option<usize>) {
        match self {
            SplitRow::Hunk { unified } | SplitRow::Context { unified } => {
                (Some(*unified), Some(*unified))
            }
            SplitRow::Added { after } => (None, Some(after.unified)),
            SplitRow::Deleted { before } => (Some(before.unified), None),
            SplitRow::Modified { before, after } => (Some(before.unified), Some(after.unified)),
        }
    }
}

/// Pair the added and deleted lines of every block of changes: paired lines
/// become `Modified` rows (with intra-line ranges when the block has as many
/// additions as deletions), the rest stay on their own side.
pub fn build_split_rows(rows: &[Row]) -> Vec<SplitRow> {
    let mut out = Vec::with_capacity(rows.len());
    let mut i = 0;
    while i < rows.len() {
        match rows[i].kind {
            DiffLineKind::Hunk => {
                out.push(SplitRow::Hunk { unified: i });
                i += 1;
            }
            DiffLineKind::Context => {
                out.push(SplitRow::Context { unified: i });
                i += 1;
            }
            DiffLineKind::Add | DiffLineKind::Delete => {
                let start = i;
                while i < rows.len()
                    && matches!(rows[i].kind, DiffLineKind::Add | DiffLineKind::Delete)
                {
                    i += 1;
                }
                let added: Vec<usize> = (start..i)
                    .filter(|&ix| rows[ix].kind == DiffLineKind::Add)
                    .collect();
                let deleted: Vec<usize> = (start..i)
                    .filter(|&ix| rows[ix].kind == DiffLineKind::Delete)
                    .collect();
                let with_tokens = added.len() == deleted.len();
                let pairs = added.len().min(deleted.len());
                for k in 0..pairs {
                    let (d, a) = (deleted[k], added[k]);
                    let (before_inner, after_inner) = if with_tokens
                        && rows[d].text.len() < MAX_INTRA_LINE_DIFF_LEN
                        && rows[a].text.len() < MAX_INTRA_LINE_DIFF_LEN
                    {
                        let (b, af) = relative_changes(&rows[d].text, &rows[a].text);
                        (Some(b), Some(af))
                    } else {
                        (None, None)
                    };
                    out.push(SplitRow::Modified {
                        before: SplitSide {
                            unified: d,
                            inner: before_inner,
                        },
                        after: SplitSide {
                            unified: a,
                            inner: after_inner,
                        },
                    });
                }
                for &d in &deleted[pairs..] {
                    out.push(SplitRow::Deleted {
                        before: SplitSide {
                            unified: d,
                            inner: None,
                        },
                    });
                }
                for &a in &added[pairs..] {
                    out.push(SplitRow::Added {
                        after: SplitSide {
                            unified: a,
                            inner: None,
                        },
                    });
                }
            }
        }
    }
    out
}

/// For each unified row, the split row that shows it.
pub fn unified_to_split(split: &[SplitRow], unified_len: usize) -> Vec<usize> {
    let mut map = vec![0; unified_len];
    for (ix, row) in split.iter().enumerate() {
        let (b, a) = row.unified_rows();
        for u in [b, a].into_iter().flatten() {
            if u < unified_len {
                map[u] = ix;
            }
        }
    }
    map
}

/// Which column of a split row a side belongs to (`DiffColumn`); unified
/// rows count as `Before`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Column {
    Before,
    After,
}

/// Where the text of every rendered row sits on screen, keyed by list index
/// and column: filled while rows paint, read by the text-selection drag.
pub type TextBounds = Rc<RefCell<HashMap<(usize, Column), Bounds<Pixels>>>>;

/// `.line-number` of one side: `[check][number]`, selectable when the line
/// is a change (`renderLineNumber`).
#[allow(clippy::too_many_arguments)]
fn split_line_number(
    ctx: &RowContext,
    row: &Row,
    number: Option<u32>,
    column: Column,
    changed: bool,
    empty: bool,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let selectable = ctx.selectable;
    let check_width = if selectable && ctx.show_check_marks {
        CHECK_WIDTH
    } else {
        0.
    };
    let original = row.original.unwrap_or(row.abs);
    let selected = selectable && changed && is_selected(&ctx.selection, ctx.temp, original);
    let group_hover = selectable
        && row
            .group
            .map(|(s, _)| ctx.hovered_group == Some(s))
            .unwrap_or(false);
    let (gutter_bg, gutter_border, text) = if empty {
        (
            t.diff_empty_row_gutter_background,
            t.diff_border,
            t.diff_line_number,
        )
    } else {
        match (changed, column) {
            (true, Column::Before) => (
                t.diff_delete_gutter_background,
                t.diff_delete_border,
                t.diff_line_number,
            ),
            (true, Column::After) => (
                t.diff_add_gutter_background,
                t.diff_add_border,
                t.diff_line_number,
            ),
            (false, _) => (t.diff_gutter_background, t.diff_border, t.diff_line_number),
        }
    };
    let normal = if selected {
        (
            t.diff_selected_background,
            t.diff_selected_border,
            t.diff_selected_text,
        )
    } else {
        (gutter_bg, gutter_border, text)
    };
    let hover = if selected {
        (
            t.diff_hover_background,
            t.diff_hover_border,
            t.diff_hover_text,
        )
    } else if column == Column::After {
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
    let (bg, border, fg) = if group_hover && changed {
        hover
    } else {
        normal
    };
    let stored_selected = ctx.selection.is_selected(original);
    let view_for_down = ctx.view.clone();
    let view_for_menu = ctx.view.clone();
    let hide_whitespace = ctx.hide_whitespace;
    let group_type = row.group_type;
    let side = if column == Column::Before { 0 } else { 1 };
    div()
        .id(("split-gutter", row.abs as usize * 2 + side))
        .w(zpx(ctx.line_number_width + check_width))
        .flex_none()
        .flex()
        .flex_row()
        .items_stretch()
        .bg(bg)
        .when(column == Column::Before, |d| d.border_l_1())
        .when(column == Column::After, |d| d.border_r_1())
        .border_color(border)
        .text_color(fg)
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
        .when(check_width > 0., |d| {
            d.child(
                div()
                    .w(zpx(CHECK_WIDTH))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .items_center()
                    .when(selected, |d| {
                        d.child(octicon(Octicon::DiffCheck, fg).size(zpx(12.)))
                    }),
            )
        })
        .child(
            div()
                .flex_1()
                .flex()
                .justify_end()
                .items_center()
                .px(SPACING_HALF())
                .child(number.map(|n| n.to_string()).unwrap_or_default()),
        )
        .into_any_element()
}

/// `.content` of one side: prefix + text with syntax, search and inner-change
/// highlights (`renderContent`).
fn split_content(
    ctx: &RowContext,
    list_ix: usize,
    column: Column,
    row: &Row,
    prefix: &'static str,
    inner: Option<(Range<usize>, Hsla)>,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let unified = row.abs as usize;
    let spans: &[Span] = ctx
        .tokens
        .as_ref()
        .and_then(|tk| tk.get(unified))
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let hits: &[(Range<usize>, bool)] = ctx
        .search
        .as_ref()
        .and_then(|s| s.by_row.get(&unified))
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let selection = ctx.selection_range(list_ix, column, row.text.len());
    let highlights =
        if spans.is_empty() && hits.is_empty() && inner.is_none() && selection.is_none() {
            Vec::new()
        } else {
            merge_highlights(spans, hits, inner, selection, row.text.len(), t)
        };
    let body = selectable_text(ctx, list_ix, column, &row.text, highlights);
    let view_for_menu = ctx.view.clone();
    div()
        .id(("split-text", unified))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_row()
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            view_for_menu
                .update(cx, |this, cx| this.text_menu(ev.position, window, cx))
                .ok();
        })
        .child(div().flex_none().whitespace_nowrap().child(prefix))
        .child(body)
        .when(row.no_newline, |d| {
            d.child(
                div()
                    .flex_none()
                    .italic()
                    .ml(zpx(4.))
                    .text_color(t.diff_alt_text)
                    .child("No newline at end of file"),
            )
        })
        .into_any_element()
}

/// `.hunk-handle`: the block toggle laid over a changed split row's
/// place holder (context rows get borders instead).
fn split_handle(ctx: &RowContext, row: &Row, width: f32, cx: &App) -> AnyElement {
    let t = cx.ghd();
    // without a selectable group only the place holder shows
    if row.group.is_none() {
        return div().into_any_element();
    }
    let (start, len) = row.group.unwrap_or((0, 0));
    let group_type = row.group_type.unwrap_or(RangeType::Mixed);
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
        .id(("split-hunk-handle", row.abs as usize))
        .absolute()
        .top_0()
        .bottom_0()
        .left(gpui_kit::relative(0.5))
        .ml(zpx(-width / 2.))
        .w(zpx(width))
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
                d.flex()
                    .justify_center()
                    .items_start()
                    .pt(zpx(3.))
                    .children(
                        match kind {
                            DiffSelectionType::All => Some(Octicon::DiffCheck),
                            DiffSelectionType::Partial => Some(Octicon::DiffDash),
                            DiffSelectionType::None => None,
                        }
                        .map(|icon| octicon(icon, white()).size(zpx(12.))),
                    )
            },
        )
        .into_any_element()
}

/// GHD `SideBySideDiffRow` in split mode.
pub fn render_split_row(
    ctx: &RowContext,
    ix: usize,
    row: &SplitRow,
    rows: &[Row],
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let selectable = ctx.selectable;
    // `--hunk-handle-width`: 4 px, 16 px with the check-all control
    let handle_width = if selectable && ctx.show_check_marks {
        HANDLE_WIDTH
    } else {
        4.
    };
    let base = div()
        .id(("split-row", ix))
        .relative()
        .min_h(DIFF_LINE_HEIGHT())
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .items_stretch();

    match row {
        SplitRow::Hunk { unified } => {
            let r = &rows[*unified];
            let check = if selectable && ctx.show_check_marks {
                CHECK_WIDTH
            } else {
                0.
            };
            let width = ctx.line_number_width + check;
            let height = r.height();
            let gutter: AnyElement = match r.expansion {
                HunkExpansionType::Both => div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .child(expansion_handle(
                        ctx,
                        r,
                        HunkExpansionType::Down,
                        width,
                        DIFF_LINE_HEIGHT(),
                        cx,
                    ))
                    .child(expansion_handle(
                        ctx,
                        r,
                        HunkExpansionType::Up,
                        width,
                        DIFF_LINE_HEIGHT(),
                        cx,
                    ))
                    .into_any_element(),
                kind => expansion_handle(ctx, r, kind, width, height, cx),
            };
            base.min_h(height)
                .bg(t.diff_hunk_background)
                .text_color(t.diff_hunk_text)
                .child(gutter)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .child(split_content(ctx, ix, Column::Before, r, "     ", None, cx)),
                )
                .into_any_element()
        }
        SplitRow::Context { unified } => {
            let r = &rows[*unified];
            let side = |column: Column| {
                let number = if column == Column::Before {
                    r.old
                } else {
                    r.new
                };
                let ln = split_line_number(ctx, r, number, column, false, false, cx);
                let content = split_content(ctx, ix, column, r, "     ", None, cx);
                let mut d = div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_stretch()
                    .bg(t.background)
                    .text_color(t.diff_text)
                    // `.editable .row.context .before/.after`: half the
                    // handle width as a border on each side of the centre
                    .when(selectable, |d| {
                        let half = zpx(handle_width / 2.);
                        let d = d.border_color(t.diff_border);
                        if column == Column::Before {
                            d.border_r(half)
                        } else {
                            d.border_l(half)
                        }
                    });
                // `.editable .row .before { flex-direction: row-reverse }`
                if selectable && column == Column::Before {
                    d = d.child(content).child(ln);
                } else {
                    d = d.child(ln).child(content);
                }
                d
            };
            base.child(side(Column::Before))
                .child(side(Column::After))
                .into_any_element()
        }
        SplitRow::Added { .. } | SplitRow::Deleted { .. } | SplitRow::Modified { .. } => {
            let (before, after) = match row {
                SplitRow::Added { after } => (None, Some(after)),
                SplitRow::Deleted { before } => (Some(before), None),
                SplitRow::Modified { before, after } => (Some(before), Some(after)),
                _ => (None, None),
            };
            let group_row = before
                .or(after)
                .map(|s| &rows[s.unified])
                .expect("a changed split row has a side");
            let side = |column: Column, side: Option<&SplitSide>| {
                let (bg, fg) = match (side.is_some(), column) {
                    (false, _) => (t.diff_empty_row_background, t.diff_text),
                    (true, Column::Before) => (t.diff_delete_background, t.diff_delete_text),
                    (true, Column::After) => (t.diff_add_background, t.diff_add_text),
                };
                let mut d = div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_stretch()
                    .bg(bg)
                    .text_color(fg);
                let (ln, content): (AnyElement, AnyElement) = match side {
                    Some(s) => {
                        let r = &rows[s.unified];
                        let number = if column == Column::Before {
                            r.old
                        } else {
                            r.new
                        };
                        let inner_color = if column == Column::Before {
                            t.diff_delete_inner_background
                        } else {
                            t.diff_add_inner_background
                        };
                        let inner = s.inner.clone().map(|r| (r, inner_color));
                        let prefix = if column == Column::Before {
                            "  -  "
                        } else {
                            "  +  "
                        };
                        (
                            split_line_number(ctx, r, number, column, true, false, cx),
                            split_content(ctx, ix, column, r, prefix, inner, cx),
                        )
                    }
                    None => (
                        split_line_number(ctx, group_row, None, column, false, true, cx),
                        div().flex_1().into_any_element(),
                    ),
                };
                if selectable && column == Column::Before {
                    d = d.child(content).child(ln);
                } else {
                    d = d.child(ln).child(content);
                }
                if let Some(s) = side.filter(|_| selectable) {
                    // extend the drag as the pointer crosses changed rows
                    let original = rows[s.unified].original.unwrap_or(rows[s.unified].abs);
                    let view = ctx.view.clone();
                    let hide_whitespace = ctx.hide_whitespace;
                    d = d.on_mouse_move(move |_, _, cx| {
                        if !hide_whitespace {
                            view.update(cx, |this, cx| this.extend_selection(original, cx))
                                .ok();
                        }
                    });
                }
                d
            };
            // `.hunk-handle-place-holder` keeps the handle's width in the
            // flow; the interactive handle is laid over it
            base.child(side(Column::Before, before))
                .when(selectable, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .w(zpx(handle_width))
                            .bg(t.diff_empty_hunk_handle),
                    )
                })
                .child(side(Column::After, after))
                .when(selectable, |d| {
                    d.child(split_handle(ctx, group_row, handle_width, cx))
                })
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    // explicit imports: `gpui_kit::*` would shadow `#[test]` with GPUI's macro
    use super::{
        RangeType, SearchHit, SplitRow, build_rows, build_split_rows, relative_changes,
        search_rows, unified_to_split,
    };
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

    #[test]
    fn relative_changes_trim_common_ends() {
        assert_eq!(
            relative_changes("hello world", "hello there world"),
            (6..6, 6..12)
        );
        assert_eq!(relative_changes("abc", "abc"), (3..3, 3..3));
        assert_eq!(relative_changes("café x", "café y"), (6..7, 6..7));
        assert_eq!(relative_changes("", "new"), (0..0, 0..3));
    }

    #[test]
    fn split_rows_pair_changes() {
        let x = crate::diff_expansion::from_hunks(&[hunk()], None);
        let rows = build_rows(&x);
        let split = build_split_rows(&rows);
        // hunk, context, modified(beta/Beta), added(gamma), context
        assert_eq!(split.len(), 5);
        match &split[2] {
            SplitRow::Modified { before, after } => {
                assert_eq!((before.unified, after.unified), (2, 3));
                // counts differ (1 deleted, 2 added) → no intra-line ranges
                assert_eq!(before.inner, None);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(split[3], SplitRow::Added { .. }));
        let map = unified_to_split(&split, rows.len());
        assert_eq!(map[3], 2);
        assert_eq!(map[4], 3);
    }
}
