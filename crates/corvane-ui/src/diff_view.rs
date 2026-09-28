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

use corvane_core::{AppState, Diff, DiffSelection, DiffSelectionType, FileStatusKind};
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
pub fn diff_header(path: &str, kind: FileStatusKind, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let (icon, color) = status_icon(kind, t);
    let (directory, file_name) = match path.rfind('/') {
        Some(i) => (&path[..=i], &path[i + 1..]),
        None => ("", path),
    };
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
                                .child(directory.to_string()),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(file_name.to_string()),
                        ),
                ),
        )
        .child(octicon(Octicon::Gear, t.text_secondary))
        .child(octicon(icon, color))
}

/// Which diff of the repository state the view shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffSource {
    /// Changes tab: the selected working-directory file (selectable lines).
    WorkingDirectory,
    /// History tab: the selected file of the selected commit (read-only).
    Commit,
    /// Stash viewer: the selected stashed file (read-only).
    Stash,
}

pub struct DiffView {
    state: Entity<AppState>,
    source: DiffSource,
    temp: Option<TempSelection>,
    hovered_group: Option<u32>,
    list_state: ListState,
    rows: Rc<Vec<Row>>,
    /// (repo, path, diff generation) the cached rows were built from.
    rows_key: Option<(u64, String, u64)>,
    /// Syntax spans for `rows`, filled in by a background task.
    tokens: Option<Rc<Vec<Vec<corvane_highlight::Span>>>>,
}

impl DiffView {
    pub fn new(state: Entity<AppState>, source: DiffSource, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            source,
            temp: None,
            hovered_group: None,
            list_state: ListState::new(0, ListAlignment::Top, px(200.)),
            rows: Rc::new(Vec::new()),
            rows_key: None,
            tokens: None,
        }
    }

    /// Tokenize the rows off the main thread (GHD: highlighter web worker).
    fn highlight(&mut self, key: (u64, String, u64), cx: &mut Context<Self>) {
        self.tokens = None;
        let path = key.1.clone();
        let lines: Vec<Option<String>> = self
            .rows
            .iter()
            .map(|r| (r.kind != corvane_core::DiffLineKind::Hunk).then(|| r.text.clone()))
            .collect();
        let task = cx.background_executor().spawn(async move {
            // hunk header rows are fed as empty lines so parser state and indices line up
            let texts: Vec<&str> = lines.iter().map(|l| l.as_deref().unwrap_or("")).collect();
            corvane_highlight::highlight_lines(&path, texts)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.rows_key.as_ref() == Some(&key)
                    && let Some(tokens) = result
                {
                    this.tokens = Some(Rc::new(tokens));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
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
        let (repo, path, kind, selection, other, rebuilt) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else {
                return div().flex_1().into_any_element();
            };
            let Some(rs) = s.repo_states.get(&id) else {
                return div().flex_1().into_any_element();
            };
            let (path, kind, selection, diff, generation) = match self.source {
                DiffSource::WorkingDirectory => {
                    let file = rs.selected_file.as_ref().and_then(|p| {
                        rs.status
                            .as_ref()
                            .and_then(|st| st.files.iter().find(|f| &f.path == p))
                    });
                    let (Some(file), Some(diff)) = (file, rs.diff.as_ref()) else {
                        return div().flex_1().into_any_element();
                    };
                    (
                        file.path.clone(),
                        file.status.kind,
                        file.selection.clone(),
                        diff,
                        rs.diff_generation,
                    )
                }
                DiffSource::Commit => {
                    let file = rs.commit_selected_file.as_ref().and_then(|p| {
                        rs.changeset
                            .as_ref()
                            .and_then(|c| c.files.iter().find(|f| &f.path == p))
                    });
                    let (Some(file), Some(diff)) = (file, rs.commit_diff.as_ref()) else {
                        return div().flex_1().into_any_element();
                    };
                    (
                        file.path.clone(),
                        file.status.kind,
                        DiffSelection::all(),
                        diff,
                        rs.commit_diff_generation,
                    )
                }
                DiffSource::Stash => {
                    let file = rs.stash_selected_file.as_ref().and_then(|p| {
                        rs.stash_files
                            .as_ref()
                            .and_then(|files| files.iter().find(|f| &f.path == p))
                    });
                    let (Some(file), Some(diff)) = (file, rs.stash_diff.as_ref()) else {
                        return div().flex_1().into_any_element();
                    };
                    (
                        file.path.clone(),
                        file.status.kind,
                        DiffSelection::all(),
                        diff,
                        rs.stash_diff_generation,
                    )
                }
            };
            let key = (id, path.clone(), generation);
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
            (id, path, kind, selection, other, rebuilt)
        };
        if let Some(message) = other {
            return blankslate(message, cx).into_any_element();
        }
        if let Some((key, rows)) = rebuilt {
            self.rows = rows;
            self.rows_key = Some(key.clone());
            self.temp = None;
            self.hovered_group = None;
            self.list_state.reset(self.rows.len());
            self.highlight(key, cx);
        }

        let t = cx.ghd();
        // `canSelect`: working-directory files that are not conflicted.
        let selectable =
            self.source == DiffSource::WorkingDirectory && kind != FileStatusKind::Conflicted;
        let mut groups: BTreeMap<u32, DiffSelectionType> = BTreeMap::new();
        for row in self.rows.iter() {
            if let Some((start, len)) = row.group {
                groups
                    .entry(start)
                    .or_insert_with(|| selection.range_kind(start, len));
            }
        }
        let ctx = Rc::new(RowContext {
            repo,
            path: path.clone(),
            selection: selection.clone(),
            groups,
            selectable,
            temp: self.temp,
            hovered_group: self.hovered_group,
            view: cx.weak_entity(),
            tokens: self.tokens.clone(),
            show_check_marks: corvane_core::AppState::try_global(cx)
                .is_none_or(|s| s.read(cx).settings.show_diff_check_marks),
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
                    render_row(&ctx, ix, &rows[ix], cx)
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
