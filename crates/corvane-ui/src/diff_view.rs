//! Unified diff viewer - GHD `ui/diff/side-by-side-diff.tsx` in unified mode
//! (`styles/ui/_side-by-side-diff.scss`, `.unified-diff.editable`): 29 px
//! header, then 20 px rows of [16 px hunk handle][20 px check mark | 55 | 55
//! line numbers][prefix + text] in 11 px monospace. Clicking or dragging over
//! line numbers toggles lines for the next commit; the handle strip toggles a
//! whole block of consecutive changes (`hunkStartLine` groups). Hunk expansion
//! (fold up/down) is not implemented yet.

use std::collections::BTreeMap;

use corvane_core::{
    AppState, Diff, DiffHunk, DiffLineKind, DiffSelection, DiffSelectionType, Dispatcher,
    FileStatusKind, WorkingDirectoryFileChange,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme, MONO_FONT};

pub const DIFF_LINE_HEIGHT: Pixels = px(20.);
/// `--hunk-handle-width-with-check-all`
const HANDLE_WIDTH: f32 = 16.;
/// `.line-number-check`
const CHECK_WIDTH: f32 = 20.;
/// `--width-line-number`
const LINE_NUMBER_WIDTH: f32 = 55.;

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

/// One unified-diff row with its absolute line index and, for changed lines,
/// the block of consecutive changes it belongs to (`hunkStartLine`, length).
struct Row {
    abs: u32,
    kind: DiffLineKind,
    old: Option<u32>,
    new: Option<u32>,
    text: String,
    no_newline: bool,
    group: Option<(u32, u32)>,
}

/// GHD `temporarySelection`: a drag in progress over the line numbers.
struct TempSelection {
    from: u32,
    to: u32,
    selected: bool,
}

pub struct DiffView {
    state: Entity<AppState>,
    temp: Option<TempSelection>,
    hovered_group: Option<u32>,
}

impl DiffView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            temp: None,
            hovered_group: None,
        }
    }

    fn rows(hunks: &[DiffHunk]) -> Vec<Row> {
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
                    text: line.text.clone(),
                    no_newline: line.no_trailing_newline,
                    group: None,
                });
                if changed {
                    block_start.get_or_insert(rows.len() - 1);
                } else if let Some(start) = block_start.take() {
                    let end = rows.len() - 1;
                    Self::close_block(&mut rows, start, end);
                }
            }
            if let Some(start) = block_start {
                let end = rows.len();
                Self::close_block(&mut rows, start, end);
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
    fn is_selected(&self, sel: &DiffSelection, line: u32) -> bool {
        let stored = sel.is_selected(line);
        match &self.temp {
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

    /// `onEndSelection`: commit the dragged range.
    fn end_selection(&mut self, cx: &mut Context<Self>) {
        let Some(t) = self.temp.take() else { return };
        let target = {
            let s = self.state.read(cx);
            s.selected.and_then(|id| {
                s.repo_states
                    .get(&id)
                    .and_then(|r| r.selected_file.clone())
                    .map(|p| (id, p))
            })
        };
        if let Some((id, path)) = target {
            let from = t.from.min(t.to);
            let len = t.from.max(t.to) - from + 1;
            Dispatcher::set_diff_lines(id, path, from, len, t.selected, cx);
        }
        cx.notify();
    }

    fn text_diff(
        &self,
        id: u64,
        file: &WorkingDirectoryFileChange,
        hunks: &[DiffHunk],
        truncated: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let t = cx.ghd();
        // `canSelect`: working-directory files that are not conflicted.
        let selectable = file.status.kind != FileStatusKind::Conflicted;
        let rows = Self::rows(hunks);
        let mut groups: BTreeMap<u32, DiffSelectionType> = BTreeMap::new();
        for row in &rows {
            if let Some((start, len)) = row.group {
                groups
                    .entry(start)
                    .or_insert_with(|| file.selection.range_kind(start, len));
            }
        }
        div()
            .id("diff")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .bg(t.background)
            .font_family(MONO_FONT)
            .text_size(FONT_SIZE_SM)
            .line_height(DIFF_LINE_HEIGHT)
            .text_color(t.diff_text)
            .flex()
            .flex_col()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.end_selection(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.end_selection(cx)),
            )
            .children(rows.iter().map(|row| {
                self.row(
                    id,
                    &file.path,
                    row,
                    &file.selection,
                    &groups,
                    selectable,
                    cx,
                )
            }))
            .when(truncated, |d| {
                d.child(
                    div()
                        .p(SPACING_DOUBLE)
                        .text_color(t.text_secondary)
                        .child("The diff is too large to show in full."),
                )
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        id: u64,
        path: &str,
        row: &Row,
        sel: &DiffSelection,
        groups: &BTreeMap<u32, DiffSelectionType>,
        selectable: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let t = cx.ghd();
        let abs = row.abs;
        let changed = matches!(row.kind, DiffLineKind::Add | DiffLineKind::Delete);
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
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(row.text.replace('\t', "    ")),
            )
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
                let kind = groups
                    .get(&start)
                    .copied()
                    .unwrap_or(DiffSelectionType::None);
                let bg = if kind != DiffSelectionType::None {
                    t.diff_selected_border
                } else {
                    t.diff_empty_hunk_handle
                };
                let path_for_click = path.to_string();
                div()
                    .id(("hunk-handle", abs as usize))
                    .w(px(HANDLE_WIDTH))
                    .flex_none()
                    .bg(bg)
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        let next = if *hovered { Some(start) } else { None };
                        if this.hovered_group != next {
                            this.hovered_group = next;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        Dispatcher::set_diff_lines(
                            id,
                            path_for_click.clone(),
                            start,
                            len,
                            kind != DiffSelectionType::All,
                            cx,
                        )
                    }))
                    .when(abs == start && len > 1, |d| {
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
        let selected = selectable && changed && self.is_selected(sel, abs);
        let group_hover = selectable
            && row
                .group
                .map(|(s, _)| self.hovered_group == Some(s))
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
        let stored_selected = sel.is_selected(abs);
        let number = |n: Option<u32>| {
            div()
                .flex_1()
                .flex()
                .justify_end()
                .items_center()
                .px(SPACING_HALF)
                .child(n.map(|n| n.to_string()).unwrap_or_default())
        };
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.temp = Some(TempSelection {
                                from: abs,
                                to: abs,
                                selected: !stored_selected,
                            });
                            cx.notify();
                        }),
                    )
            })
            .when(selectable, |d| {
                d.child(
                    div()
                        .w(px(CHECK_WIDTH))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .items_center()
                        .when(selected, |d| {
                            d.child(octicon(Octicon::DiffCheck, num_text).size(px(12.)))
                        }),
                )
            })
            .child(number(row.old).border_r_1().border_color(num_border))
            .child(number(row.new));

        el = el.child(gutter).child(content);
        if selectable && changed {
            // extend the drag as the pointer crosses changed rows
            el = el.on_mouse_move(cx.listener(move |this, _, _, cx| {
                if let Some(t) = this.temp.as_mut()
                    && t.to != abs
                {
                    t.to = abs;
                    cx.notify();
                }
            }));
        }
        el.into_any_element()
    }
}

impl Render for DiffView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.state.read(cx);
        let Some(id) = s.selected else {
            return div().flex_1().into_any_element();
        };
        let Some(rs) = s.repo_states.get(&id) else {
            return div().flex_1().into_any_element();
        };
        let file = rs.selected_file.as_ref().and_then(|p| {
            rs.status
                .as_ref()
                .and_then(|st| st.files.iter().find(|f| &f.path == p))
        });
        let (Some(file), Some(diff)) = (file, rs.diff.as_ref()) else {
            return div().flex_1().into_any_element();
        };
        match diff {
            Diff::Text { hunks, truncated } => self
                .text_diff(id, file, hunks, *truncated, cx)
                .into_any_element(),
            Diff::Binary => blankslate("This binary file has changed.", cx).into_any_element(),
            Diff::Empty => blankslate("No changes to show.", cx).into_any_element(),
            Diff::TooLarge => blankslate("The diff is too large to be displayed by default.", cx)
                .into_any_element(),
            Diff::Submodule => {
                blankslate("Submodule changes are not shown yet.", cx).into_any_element()
            }
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
