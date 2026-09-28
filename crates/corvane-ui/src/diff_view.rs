//! Unified diff viewer - GHD `ui/diff/side-by-side-diff.tsx` in unified mode
//! (`styles/ui/_side-by-side-diff.scss`, `.unified-diff.editable`): 29 px
//! header, then 20 px rows of [16 px hunk handle][20 px check mark | 55 | 55
//! line numbers][prefix + text] in 11 px monospace. Clicking or dragging over
//! line numbers toggles lines for the next commit; the handle strip toggles a
//! whole block of consecutive changes (`hunkStartLine` groups). Rows are
//! virtualized with `gpui::list` (GHD: react-virtualized + CellMeasurer).
//! Hunk expansion (fold up/down) is not implemented yet.

use std::collections::BTreeMap;
use std::rc::Rc;

use corvane_core::{AppState, Diff, DiffSelectionType, FileStatusKind, WorkingDirectoryFileChange};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::diff_view_rows::{Row, RowContext, TempSelection, build_rows, render_row};
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

pub struct DiffView {
    state: Entity<AppState>,
    temp: Option<TempSelection>,
    hovered_group: Option<u32>,
    list_state: ListState,
    rows: Rc<Vec<Row>>,
    /// (repo, path, diff generation) the cached rows were built from.
    rows_key: Option<(u64, String, u64)>,
}

impl DiffView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            temp: None,
            hovered_group: None,
            list_state: ListState::new(0, ListAlignment::Top, px(200.)),
            rows: Rc::new(Vec::new()),
            rows_key: None,
        }
    }

    pub fn set_hovered_group(&mut self, group: Option<u32>, cx: &mut Context<Self>) {
        if self.hovered_group != group {
            self.hovered_group = group;
            cx.notify();
        }
    }

    /// `onStartSelection`
    pub fn start_selection(&mut self, temp: TempSelection, cx: &mut Context<Self>) {
        self.temp = Some(temp);
        cx.notify();
    }

    /// `onUpdateSelection`: the pointer crossed a changed row.
    pub fn extend_selection(&mut self, line: u32, cx: &mut Context<Self>) {
        if let Some(t) = self.temp.as_mut()
            && t.to != line
        {
            t.to = line;
            cx.notify();
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
            corvane_core::Dispatcher::set_diff_lines(id, path, from, len, t.selected, cx);
        }
        cx.notify();
    }
}

impl Render for DiffView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Snapshot what the rows need, rebuilding the row cache when the diff changed.
        let (repo, file, other, rebuilt) = {
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
            let key = (id, file.path.clone(), rs.diff_generation);
            let mut rebuilt = None;
            let mut other = None;
            match diff {
                Diff::Text { hunks, .. } => {
                    if self.rows_key.as_ref() != Some(&key) {
                        rebuilt = Some((key, Rc::new(build_rows(hunks))));
                    }
                }
                Diff::Binary => other = Some("This binary file has changed."),
                Diff::Empty => other = Some("No changes to show."),
                Diff::TooLarge => other = Some("The diff is too large to be displayed by default."),
                Diff::Submodule => other = Some("Submodule changes are not shown yet."),
            }
            (id, file.clone(), other, rebuilt)
        };
        if let Some(message) = other {
            return blankslate(message, cx).into_any_element();
        }
        if let Some((key, rows)) = rebuilt {
            self.rows = rows;
            self.rows_key = Some(key);
            self.temp = None;
            self.hovered_group = None;
            self.list_state.reset(self.rows.len());
        }

        let t = cx.ghd();
        // `canSelect`: working-directory files that are not conflicted.
        let selectable = file.status.kind != FileStatusKind::Conflicted;
        let mut groups: BTreeMap<u32, DiffSelectionType> = BTreeMap::new();
        for row in self.rows.iter() {
            if let Some((start, len)) = row.group {
                groups
                    .entry(start)
                    .or_insert_with(|| file.selection.range_kind(start, len));
            }
        }
        let ctx = Rc::new(RowContext {
            repo,
            path: file.path.clone(),
            selection: file.selection.clone(),
            groups,
            selectable,
            temp: self.temp,
            hovered_group: self.hovered_group,
            view: cx.weak_entity(),
        });
        let rows = self.rows.clone();
        div()
            .id("diff")
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(t.background)
            .font_family(MONO_FONT)
            .text_size(FONT_SIZE_SM)
            .line_height(DIFF_LINE_HEIGHT)
            .text_color(t.diff_text)
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.end_selection(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.end_selection(cx)),
            )
            .child(
                list(self.list_state.clone(), move |ix, _window, cx| {
                    render_row(&ctx, &rows[ix], cx)
                })
                .size_full(),
            )
            .into_any_element()
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
