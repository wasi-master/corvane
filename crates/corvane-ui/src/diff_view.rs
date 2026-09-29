//! Diff viewer - GHD `ui/diff/index.tsx` routing plus `side-by-side-diff.tsx`
//! in unified mode (`styles/ui/_side-by-side-diff.scss`, `.unified-diff.editable`):
//! 29 px header, then 20 px rows of [16 px hunk handle][20 px check mark | 55 |
//! 55 line numbers][prefix + text] in 11 px monospace. Clicking or dragging
//! over line numbers toggles lines for the next commit; the handle strip
//! toggles a whole block of consecutive changes (`hunkStartLine` groups).
//! Hunk headers carry fold handles (`diff_expansion`), the gutter has a
//! right-click "Discard … Lines" menu, ⌘F searches the rows, the header gear
//! opens Diff Settings, and binary / image / submodule / large diffs get their
//! own panels. Rows are virtualized with `gpui::list`.
//!
//! Deviation: "Split" (side-by-side) rendering is not implemented; the radio
//! button is shown disabled.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;

use corvane_core::{
    AppState, Diff, DiffSelection, DiffSelectionType, Dispatcher, FileStatusKind, SubmoduleDiff,
};
use gpui_kit::component::input::{Escape, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::Find;
use crate::context_menu::{ContextMenu, MenuItem};
use crate::diff_expansion::{
    DEFAULT_DIFF_EXPANSION_STEP, ExpansionKind, HunkExpansionType, XHunk, expand_hunk,
    expand_whole, from_hunks,
};
use crate::diff_view_rows::{
    RangeType, Row, RowContext, SearchHit, SearchIndex, TempSelection, build_rows, render_row,
    search_rows,
};
use crate::icons::{Octicon, octicon};
use crate::image_diff::ImageDiff;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme, MONO_FONT};
use crate::widgets::{
    Inline, button, checkbox_row, code_ref, link_button, paragraph, primary_button, radio_row,
    text_box,
};

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

/// `.diff-header`: path (directory dimmed), the Diff Settings gear and the
/// status icon, 29 px. The gear toggles the popover owned by `view`.
pub fn diff_header(
    path: &str,
    kind: FileStatusKind,
    view: &Entity<DiffView>,
    cx: &App,
) -> impl IntoElement {
    let t = cx.ghd();
    let (icon, color) = status_icon(kind, t);
    let (directory, file_name) = match path.rfind('/') {
        Some(i) => (&path[..=i], &path[i + 1..]),
        None => ("", path),
    };
    let gear_bounds = view.read(cx).gear_bounds.clone();
    let view = view.clone();
    let hover = t.text_secondary;
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
        .child(
            // `.diff-options-component > button`
            div()
                .id("diff-options-button")
                .relative()
                .flex()
                .items_center()
                .cursor_pointer()
                .text_color(t.text)
                .hover(move |s| s.text_color(hover))
                .child(
                    canvas(move |bounds, _, _| gear_bounds.set(bounds), |_, _, _, _| {})
                        .absolute()
                        .inset_0(),
                )
                .child(octicon(Octicon::Gear, t.text_secondary))
                .on_click(move |_, _, cx| {
                    view.update(cx, |this, cx| this.toggle_options(cx));
                }),
        )
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

/// One render's snapshot of the repository state.
struct Snapshot {
    repo: u64,
    repo_path: std::path::PathBuf,
    path: String,
    kind: FileStatusKind,
    selection: DiffSelection,
    diff: Diff,
    contents: Option<Arc<Vec<String>>>,
    key: (u64, String, u64),
    hide_whitespace: bool,
    confirm_discard: bool,
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
    /// The hunks as shown (expanded copies of the model's).
    hunks: Rc<Vec<XHunk>>,
    /// New-side file lines for expansion (`fileContents.newContents`).
    contents: Option<Arc<Vec<String>>>,
    /// GHD `diffToRestore !== null`: "Collapse Expanded Lines" is available.
    expanded: bool,
    /// GHD `forceShowLargeDiff`.
    show_large: bool,
    // ⌘F (`DiffSearchInput`); the input is created on first use (needs a window)
    searching: bool,
    search_input: Option<Entity<InputState>>,
    search_query: String,
    hits: Vec<SearchHit>,
    selected_hit: Option<usize>,
    // Diff Settings popover (`DiffOptions`)
    options_open: bool,
    pub(crate) gear_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// `WhitespaceHintPopover` anchor.
    whitespace_hint: Option<Point<Pixels>>,
    image: Option<Entity<ImageDiff>>,
    context_menu: Option<Entity<ContextMenu>>,
    focus_handle: FocusHandle,
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
            hunks: Rc::new(Vec::new()),
            contents: None,
            expanded: false,
            show_large: false,
            searching: false,
            search_input: None,
            search_query: String::new(),
            hits: Vec::new(),
            selected_hit: None,
            options_open: false,
            gear_bounds: Rc::new(Cell::new(Bounds::default())),
            whitespace_hint: None,
            image: None,
            context_menu: None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn snapshot(&self, cx: &App) -> Option<Snapshot> {
        let s = self.state.read(cx);
        let id = s.selected?;
        let rs = s.repo_states.get(&id)?;
        let repo_path = s.repository(id)?.path.clone();
        let (path, kind, selection, diff, generation, contents, hide_whitespace) = match self.source
        {
            DiffSource::WorkingDirectory => {
                let file = rs.selected_file.as_ref().and_then(|p| {
                    rs.status
                        .as_ref()
                        .and_then(|st| st.files.iter().find(|f| &f.path == p))
                })?;
                (
                    file.path.clone(),
                    file.status.kind,
                    file.selection.clone(),
                    rs.diff.clone()?,
                    rs.diff_generation,
                    rs.diff_contents.clone(),
                    s.settings.hide_whitespace_in_changes_diff,
                )
            }
            DiffSource::Commit => {
                let file = rs.commit_selected_file.as_ref().and_then(|p| {
                    rs.changeset
                        .as_ref()
                        .and_then(|c| c.files.iter().find(|f| &f.path == p))
                })?;
                (
                    file.path.clone(),
                    file.status.kind,
                    DiffSelection::all(),
                    rs.commit_diff.clone()?,
                    rs.commit_diff_generation,
                    rs.commit_diff_contents.clone(),
                    s.settings.hide_whitespace_in_history_diff,
                )
            }
            DiffSource::Stash => {
                let file = rs.stash_selected_file.as_ref().and_then(|p| {
                    rs.stash_files
                        .as_ref()
                        .and_then(|files| files.iter().find(|f| &f.path == p))
                })?;
                (
                    file.path.clone(),
                    file.status.kind,
                    DiffSelection::all(),
                    rs.stash_diff.clone()?,
                    rs.stash_diff_generation,
                    rs.stash_diff_contents.clone(),
                    s.settings.hide_whitespace_in_history_diff,
                )
            }
        };
        Some(Snapshot {
            repo: id,
            repo_path,
            key: (id, path.clone(), generation),
            path,
            kind,
            selection,
            diff,
            contents,
            hide_whitespace,
            confirm_discard: s.settings.confirm_discard_changes,
        })
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
        let generation = self.rows.len();
        let task = cx.background_executor().spawn(async move {
            // hunk header rows are fed as empty lines so parser state and indices line up
            let texts: Vec<&str> = lines.iter().map(|l| l.as_deref().unwrap_or("")).collect();
            corvane_highlight::highlight_lines(&path, texts)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.rows_key.as_ref() == Some(&key)
                    && this.rows.len() == generation
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

    /// Rebuild rows after the hunks changed (new diff or expansion).
    fn rebuild_rows(&mut self, key: (u64, String, u64), cx: &mut Context<Self>) {
        let old_len = self.rows.len();
        self.rows = Rc::new(build_rows(&self.hunks));
        self.rows_key = Some(key.clone());
        self.list_state.splice(0..old_len, self.rows.len());
        self.refresh_search();
        self.highlight(key, cx);
    }

    /// A different diff arrived: start over (`componentDidUpdate` in GHD).
    fn load(&mut self, snap: &Snapshot, cx: &mut Context<Self>) {
        self.temp = None;
        self.hovered_group = None;
        self.expanded = false;
        self.show_large = false;
        self.whitespace_hint = None;
        self.contents = snap.contents.clone();
        self.hunks = Rc::new(match snap.diff.hunks() {
            Some(hunks) => from_hunks(hunks, self.contents.as_ref().map(|c| c.len())),
            None => Vec::new(),
        });
        self.image = match &snap.diff {
            Diff::Image { previous, current } => {
                let (previous, current, kind) = (previous.clone(), current.clone(), snap.kind);
                Some(cx.new(|cx| ImageDiff::new(previous.as_ref(), current.as_ref(), kind, cx)))
            }
            _ => None,
        };
        let old_len = self.rows.len();
        self.rows = Rc::new(build_rows(&self.hunks));
        self.rows_key = Some(snap.key.clone());
        self.list_state.reset(self.rows.len());
        let _ = old_len;
        self.refresh_search();
        self.highlight(snap.key.clone(), cx);
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
            Dispatcher::set_diff_lines(id, path, from, len, t.selected, cx);
        }
        cx.notify();
    }

    // ---- expansion ----

    fn can_expand(&self) -> bool {
        self.contents.as_ref().is_some_and(|c| !c.is_empty())
    }

    /// `onExpandHunk`
    pub fn expand(&mut self, hunk_index: usize, kind: ExpansionKind, cx: &mut Context<Self>) {
        let Some(contents) = self.contents.clone() else {
            return;
        };
        let Some(key) = self.rows_key.clone() else {
            return;
        };
        if let Some(hunks) = expand_hunk(
            &self.hunks,
            hunk_index,
            kind,
            &contents,
            DEFAULT_DIFF_EXPANSION_STEP,
        ) {
            self.hunks = Rc::new(hunks);
            self.expanded = true;
            self.rebuild_rows(key, cx);
            cx.notify();
        }
    }

    /// `onExpandWholeFile`
    fn expand_whole_file(&mut self, cx: &mut Context<Self>) {
        let Some(contents) = self.contents.clone() else {
            return;
        };
        let Some(key) = self.rows_key.clone() else {
            return;
        };
        if let Some(hunks) = expand_whole(self.hunks.as_ref().clone(), &contents) {
            self.hunks = Rc::new(hunks);
            self.expanded = true;
            self.rebuild_rows(key, cx);
            cx.notify();
        }
    }

    /// `onCollapseExpandedLines`
    fn collapse(&mut self, cx: &mut Context<Self>) {
        let Some(snap) = self.snapshot(cx) else {
            return;
        };
        if let Some(hunks) = snap.diff.hunks() {
            self.hunks = Rc::new(from_hunks(hunks, self.contents.as_ref().map(|c| c.len())));
        }
        self.expanded = false;
        self.rebuild_rows(snap.key, cx);
        cx.notify();
    }

    // ---- context menus ----

    fn open_menu(
        &mut self,
        items: Vec<MenuItem>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(target_os = "macos")]
        {
            crate::native_menu::show_context_menu(items, position, window, cx);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let menu = cx.new(|cx| ContextMenu::new(items, position, window, cx));
            self.context_menu = Some(menu);
            cx.notify();
        }
    }

    /// GHD `buildExpandMenuItem`.
    fn expand_menu_item(&self, cx: &Context<Self>) -> Option<MenuItem> {
        if !self.can_expand() {
            return None;
        }
        let weak = cx.weak_entity();
        Some(if self.expanded {
            MenuItem::new("Collapse Expanded Lines", move |_, cx| {
                weak.update(cx, |this, cx| this.collapse(cx)).ok();
            })
        } else {
            let enabled =
                self.hunks.len() != 1 || self.hunks[0].expansion != HunkExpansionType::None;
            MenuItem::new("Expand Whole File", move |_, cx| {
                weak.update(cx, |this, cx| this.expand_whole_file(cx)).ok();
            })
            .enabled(enabled)
        })
    }

    /// `onContextMenuExpandHunk` / `onContextMenuText`
    pub fn expand_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(item) = self.expand_menu_item(cx) {
            self.open_menu(vec![item], position, window, cx);
        }
    }

    /// `onContextMenuLine`: discard one changed line.
    pub fn line_menu(
        &mut self,
        line: u32,
        kind: RangeType,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.discard_menu(line, 1, kind, position, window, cx);
    }

    /// `onContextMenuHunk`: discard a whole block of changes.
    pub fn hunk_menu(
        &mut self,
        start: u32,
        len: u32,
        kind: RangeType,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.discard_menu(start, len, kind, position, window, cx);
    }

    fn discard_menu(
        &mut self,
        start: u32,
        len: u32,
        kind: RangeType,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(snap) = self.snapshot(cx) else {
            return;
        };
        if self.source != DiffSource::WorkingDirectory
            || snap.kind == FileStatusKind::Conflicted
            || snap.hide_whitespace
        {
            return;
        }
        let label = kind.discard_label(len, snap.confirm_discard);
        let (repo, path) = (snap.repo, snap.path.clone());
        let item = MenuItem::new(label, move |_, cx| {
            let selection = DiffSelection::none().with_range(start, len, true);
            Dispatcher::request_discard_selection(repo, path.clone(), selection, cx);
        });
        self.open_menu(vec![item], position, window, cx);
    }

    // ---- search ----

    fn show_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = match self.search_input.clone() {
            Some(input) => input,
            None => {
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search…"));
                cx.subscribe(&input, |this, _, ev: &InputEvent, cx| match ev {
                    InputEvent::PressEnter { shift, .. } => this.search(!*shift, cx),
                    InputEvent::Blur => this.close_search(cx),
                    _ => {}
                })
                .detach();
                self.search_input = Some(input.clone());
                input
            }
        };
        if !self.searching {
            self.searching = true;
            self.search_query.clear();
            self.hits.clear();
            self.selected_hit = None;
            input.update(cx, |s, cx| s.set_value("", window, cx));
        }
        let handle = input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    /// `onSearchCancel`
    fn close_search(&mut self, cx: &mut Context<Self>) {
        if !self.searching {
            return;
        }
        self.searching = false;
        self.search_query.clear();
        self.hits.clear();
        self.selected_hit = None;
        cx.notify();
    }

    /// `onSearch`: a new query starts at the first hit; the same query moves
    /// to the next / previous one (wrapping).
    fn search(&mut self, forward: bool, cx: &mut Context<Self>) {
        let query = self
            .search_input
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();
        if query.is_empty() {
            self.search_query.clear();
            self.hits.clear();
            self.selected_hit = None;
            cx.notify();
            return;
        }
        if query != self.search_query || self.hits.is_empty() {
            self.search_query = query;
            self.hits = search_rows(&self.rows, &self.search_query);
            self.selected_hit = (!self.hits.is_empty()).then_some(0);
        } else if let Some(ix) = self.selected_hit {
            let n = self.hits.len();
            self.selected_hit = Some(if forward {
                (ix + 1) % n
            } else {
                (ix + n - 1) % n
            });
        }
        if let Some(hit) = self.selected_hit.and_then(|ix| self.hits.get(ix)) {
            self.list_state.scroll_to_reveal_item(hit.row);
        }
        cx.notify();
    }

    /// Recompute the hits for the same query after the rows changed.
    fn refresh_search(&mut self) {
        if self.search_query.is_empty() {
            return;
        }
        self.hits = search_rows(&self.rows, &self.search_query);
        self.selected_hit = self
            .selected_hit
            .filter(|ix| *ix < self.hits.len())
            .or((!self.hits.is_empty()).then_some(0));
    }

    fn search_index(&self) -> Option<Rc<SearchIndex>> {
        if self.hits.is_empty() {
            return None;
        }
        let mut by_row: HashMap<usize, Vec<(std::ops::Range<usize>, bool)>> = HashMap::new();
        for (ix, hit) in self.hits.iter().enumerate() {
            by_row
                .entry(hit.row)
                .or_default()
                .push((hit.range.clone(), self.selected_hit == Some(ix)));
        }
        Some(Rc::new(SearchIndex { by_row }))
    }

    // ---- popovers ----

    pub fn toggle_options(&mut self, cx: &mut Context<Self>) {
        self.options_open = !self.options_open;
        cx.notify();
    }

    pub fn show_whitespace_hint(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.whitespace_hint = Some(position);
        cx.notify();
    }

    /// `DiffOptions` popover: Diff Settings › Whitespace, Diff display.
    fn options_popover(&self, snap: &Snapshot, window: &Window, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let anchor = self.gear_bounds.get();
        let width = px(250.);
        let x = (anchor.right() - width).max(px(0.));
        let y = anchor.bottom() + px(4.);
        let history = self.source != DiffSource::WorkingDirectory;
        let interactive = self.source == DiffSource::WorkingDirectory;
        let hide = snap.hide_whitespace;
        let split = self.state.read(cx).settings.show_side_by_side_diff;
        let _ = window;
        let legend = |text: &str| {
            div()
                .font_weight(FontWeight::BOLD)
                .mb(px(6.))
                .child(text.to_string())
        };
        deferred(
            anchored()
                .position(point(x, y))
                .snap_to_window_with_margin(px(8.))
                .child(
                    div()
                        .id("diff-options-popover")
                        .occlude()
                        .w(width)
                        .p(SPACING)
                        .flex()
                        .flex_col()
                        .text_size(FONT_SIZE)
                        .text_color(t.text)
                        .bg(t.background)
                        .border_1()
                        .border_color(t.box_border)
                        .rounded(BORDER_RADIUS)
                        .shadow(vec![BoxShadow {
                            color: hsla(0., 0., 0., 0.3),
                            offset: point(px(0.), px(0.)),
                            blur_radius: px(8.),
                            spread_radius: px(0.),
                            inset: false,
                        }])
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.options_open = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(FONT_SIZE_MD)
                                .mb(px(8.))
                                .child("Diff Settings"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .mb(px(8.))
                                .child(legend("Whitespace"))
                                .child(checkbox_row(
                                    "diff-hide-whitespace",
                                    hide,
                                    "Hide Whitespace Changes",
                                    move |checked, _, cx| {
                                        Dispatcher::set_hide_whitespace_in_diff(
                                            history, checked, cx,
                                        )
                                    },
                                    cx,
                                ))
                                .when(interactive, |d| {
                                    d.child(div().mt(px(6.)).text_color(t.text_secondary).child(
                                        "Interacting with individual lines or hunks \
                                                 will be disabled while hiding whitespace.",
                                    ))
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(legend("Diff display"))
                                .child(radio_row(
                                    "diff-display-unified",
                                    !split,
                                    "Unified",
                                    |_, cx| Dispatcher::set_show_side_by_side_diff(false, cx),
                                    cx,
                                ))
                                .child(
                                    radio_row("diff-display-split", split, "Split", |_, _| {}, cx)
                                        .opacity(0.6)
                                        .cursor_default(),
                                ),
                        ),
                ),
        )
        .with_priority(3)
        .into_any_element()
    }

    /// `WhitespaceHintPopover`: "Show whitespace changes?"
    fn whitespace_hint_popover(&self, anchor: Point<Pixels>, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let history = self.source != DiffSource::WorkingDirectory;
        deferred(
            anchored()
                .position(anchor)
                .snap_to_window_with_margin(px(8.))
                .child(
                    div()
                        .id("whitespace-hint")
                        .occlude()
                        .w(px(225.))
                        .p(SPACING)
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .text_size(FONT_SIZE)
                        .text_color(t.text)
                        .bg(t.background)
                        .border_1()
                        .border_color(t.box_border)
                        .rounded(BORDER_RADIUS)
                        .shadow(vec![BoxShadow {
                            color: hsla(0., 0., 0., 0.3),
                            offset: point(px(0.), px(0.)),
                            blur_radius: px(8.),
                            spread_radius: px(0.),
                            inset: false,
                        }])
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.whitespace_hint = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(FONT_SIZE_MD)
                                .child("Show whitespace changes?"),
                        )
                        .child(
                            div().text_color(t.text_secondary).child(
                                "Selecting lines is disabled when hiding whitespace changes.",
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_end()
                                .gap(SPACING_HALF)
                                .mt(px(4.))
                                .child(
                                    primary_button("whitespace-hint-yes", "Yes", false, cx)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.whitespace_hint = None;
                                            Dispatcher::set_hide_whitespace_in_diff(
                                                history, false, cx,
                                            );
                                        })),
                                )
                                .child(button("whitespace-hint-no", "No", cx).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.whitespace_hint = None;
                                        cx.notify();
                                    }),
                                )),
                        ),
                ),
        )
        .with_priority(3)
        .into_any_element()
    }

    // ---- panels ----

    /// `.panel.empty` and friends: a centred message.
    fn panel(&self, message: impl Into<SharedString>, cx: &App) -> AnyElement {
        let t = cx.ghd();
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .p(SPACING_DOUBLE)
            .text_size(FONT_SIZE)
            .text_color(t.text_secondary)
            .child(message.into())
            .into_any_element()
    }

    /// GHD `renderText` with no hunks.
    fn empty_panel(&self, snap: &Snapshot, cx: &App) -> AnyElement {
        let message = match snap.kind {
            FileStatusKind::New | FileStatusKind::Untracked => "The file is empty",
            FileStatusKind::Renamed => "The file was renamed but not changed",
            FileStatusKind::Conflicted => {
                "The file is in conflict and must be resolved via the command line."
            }
            _ if snap.hide_whitespace => "Only whitespace changes found",
            _ => "No content changes found",
        };
        self.panel(message, cx)
    }

    /// GHD `BinaryFile`.
    fn binary_panel(&self, snap: &Snapshot, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let full_path = snap.repo_path.join(&snap.path);
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .pt(SPACING)
            .text_size(FONT_SIZE)
            .text_color(t.text)
            .child(
                div()
                    .py(SPACING)
                    .pb(SPACING_HALF)
                    .child("This binary file has changed."),
            )
            .child(
                div().py(SPACING_HALF).child(
                    link_button("binary-open", "Open file in external program.", cx)
                        .on_click(move |_, _, cx| cx.open_with_system(&full_path)),
                ),
            )
            .into_any_element()
    }

    /// GHD `renderLargeTextDiff`.
    fn large_diff_panel(&self, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .m(SPACING_DOUBLE)
            .gap(SPACING)
            .text_size(FONT_SIZE)
            .text_color(t.text_secondary)
            .child(
                img("illustrations/ufo-alert.svg")
                    .max_h(px(150.))
                    .object_fit(ObjectFit::Contain),
            )
            .child(div().child("The diff is too large to be displayed by default."))
            .child(div().child(
                "You can try to show it anyway, but performance may be negatively impacted.",
            ))
            .child(
                button("show-large-diff", "Show Diff", cx).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.show_large = true;
                        cx.notify();
                    },
                )),
            )
            .into_any_element()
    }

    /// GHD `SubmoduleDiff` (`.changes-interstitial.submodule-diff`).
    fn submodule_panel(&self, diff: &SubmoduleDiff, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let read_only = self.source != DiffSource::WorkingDirectory;
        let item = |icon: Octicon, color: Hsla, body: Div| {
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(SPACING)
                .mb(SPACING)
                .child(div().flex_none().pt(px(2.)).child(octicon(icon, color)))
                .child(div().flex_1().min_w_0().child(body))
        };
        let sha = |sha: &str| -> Vec<Inline> {
            let full = sha.to_string();
            vec![
                Inline::Element(
                    code_ref(sha.chars().take(7).collect::<String>(), cx).into_any_element(),
                ),
                Inline::Element(
                    div()
                        .id(SharedString::from(format!("copy-{sha}")))
                        .cursor_pointer()
                        .ml(px(2.))
                        .child(octicon(Octicon::Copy, t.text_secondary).size(px(14.)))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(full.clone()))
                        })
                        .into_any_element(),
                ),
            ]
        };
        let mut items: Vec<AnyElement> = Vec::new();
        // repository link
        if let Some(url) = diff.url.as_deref()
            && let Some(gh) = corvane_core::github_from_remote(url, &[])
        {
            let html_url = gh.html_url.clone();
            let host = html_url
                .split('/')
                .nth(2)
                .unwrap_or("github.com")
                .to_string();
            let label = if host == "github.com" {
                format!("{}/{}", gh.owner, gh.name)
            } else {
                format!("{}/{} ({host})", gh.owner, gh.name)
            };
            items.push(
                item(
                    Octicon::Info,
                    t.text_secondary,
                    paragraph(vec![
                        "This is a submodule based on the repository".into(),
                        Inline::Element(
                            link_button("submodule-repo-link", label, cx)
                                .on_click(move |_, _, cx| cx.open_url(&html_url))
                                .into_any_element(),
                        ),
                        ".".into(),
                    ]),
                )
                .into_any_element(),
            );
        }
        // commit change
        let verb = if read_only { "was" } else { "has been" };
        let suffix: Vec<Inline> = if read_only {
            vec![]
        } else {
            vec!["This change can be committed to the parent repository.".into()]
        };
        match (&diff.old_sha, &diff.new_sha) {
            (Some(old), Some(new)) => {
                let mut parts: Vec<Inline> = vec!["This submodule changed its commit from".into()];
                parts.extend(sha(old));
                parts.push("to".into());
                parts.extend(sha(new));
                parts.push(".".into());
                parts.extend(suffix);
                items.push(
                    item(Octicon::DiffModified, t.color_modified, paragraph(parts))
                        .into_any_element(),
                );
            }
            (None, Some(new)) => {
                let mut parts: Vec<Inline> =
                    vec![format!("This submodule {verb} added pointing at commit").into()];
                parts.extend(sha(new));
                parts.push(".".into());
                parts.extend(suffix);
                items.push(
                    item(Octicon::DiffAdded, t.color_new, paragraph(parts)).into_any_element(),
                );
            }
            (Some(old), None) => {
                let mut parts: Vec<Inline> = vec![
                    format!("This submodule {verb} removed while it was pointing at commit").into(),
                ];
                parts.extend(sha(old));
                parts.push(".".into());
                parts.extend(suffix);
                items.push(
                    item(Octicon::DiffRemoved, t.color_deleted, paragraph(parts))
                        .into_any_element(),
                );
            }
            (None, None) => {}
        }
        // working-tree changes inside the submodule
        if diff.status.untracked_changes || diff.status.modified_changes {
            let changes = match (diff.status.untracked_changes, diff.status.modified_changes) {
                (true, true) => "modified and untracked",
                (true, false) => "untracked",
                _ => "modified",
            };
            items.push(
                item(
                    Octicon::FileDiff,
                    t.color_new,
                    paragraph(vec![
                        format!(
                            "This submodule has {changes} changes. Those changes must be committed \
                             inside of the submodule before they can be part of the parent \
                             repository."
                        )
                        .into(),
                    ]),
                )
                .into_any_element(),
            );
        }
        let full_path = diff.full_path.clone();
        let open_action = diff.url.is_some().then(|| {
            crate::no_changes::suggested_action_card(
                crate::no_changes::SuggestedAction {
                    id: "open-submodule",
                    on_click: Rc::new(move |_, cx| {
                        Dispatcher::open_submodule(full_path.clone(), cx)
                    }),
                    title: "Open this submodule on GitHub Desktop".into(),
                    description: Some(
                        "You can open this submodule on GitHub Desktop as a normal repository to \
                         manage and commit any changes in it."
                            .into(),
                    ),
                    hint: "".into(),
                    keys: &[],
                    button_label: "Open Repository".into(),
                    primary: true,
                },
                cx,
            )
        });
        div()
            .id("submodule-diff")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .p(px(40.))
            .text_size(FONT_SIZE)
            .text_color(t.text)
            .child(
                div()
                    .w_full()
                    .max_w(px(600.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::LIGHT)
                            .mb(SPACING_DOUBLE)
                            .child("Submodule changes"),
                    )
                    .children(items)
                    .children(open_action),
            )
            .into_any_element()
    }
}

impl Render for DiffView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(snap) = self.snapshot(cx) else {
            return div().flex_1().into_any_element();
        };
        if self.rows_key.as_ref() != Some(&snap.key) {
            self.load(&snap, cx);
        }
        let background = cx.ghd().background;
        let options = self
            .options_open
            .then(|| self.options_popover(&snap, window, cx));

        let body: AnyElement = match &snap.diff {
            Diff::Text { .. } | Diff::LargeText { .. } if snap.diff.line_count() > 0 => {
                if matches!(snap.diff, Diff::LargeText { .. }) && !self.show_large {
                    self.large_diff_panel(cx)
                } else {
                    self.text_diff(&snap, window, cx)
                }
            }
            Diff::Text { .. } | Diff::LargeText { .. } | Diff::Empty => self.empty_panel(&snap, cx),
            Diff::Binary => self.binary_panel(&snap, cx),
            Diff::Image { .. } => match self.image.clone() {
                Some(image) => image.into_any_element(),
                None => self.panel("This binary file has changed.", cx),
            },
            Diff::TooLarge => self.panel("The diff is too large to be displayed.", cx),
            Diff::Submodule(sub) => self.submodule_panel(sub, cx),
        };
        div()
            .id("diff-container")
            .track_focus(&self.focus_handle)
            .key_context("Diff")
            .on_action(cx.listener(|this, _: &Find, window, cx| {
                this.show_search(window, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &Escape, _, cx| {
                if this.searching {
                    this.close_search(cx);
                    cx.stop_propagation();
                }
            }))
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .bg(background)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if !this.focus_handle.contains_focused(window, cx) {
                        window.focus(&this.focus_handle, cx);
                    }
                }),
            )
            .child(body)
            .children(options)
            .children(
                self.whitespace_hint
                    .map(|anchor| self.whitespace_hint_popover(anchor, cx)),
            )
            .children(self.context_menu.clone())
            .into_any_element()
    }
}

impl DiffView {
    /// The virtualized rows plus the search box (`DiffSearchInput`).
    fn text_diff(
        &mut self,
        snap: &Snapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.ghd();
        // `canSelect`: working-directory files that are not conflicted.
        let selectable =
            self.source == DiffSource::WorkingDirectory && snap.kind != FileStatusKind::Conflicted;
        let mut groups: BTreeMap<u32, DiffSelectionType> = BTreeMap::new();
        for row in self.rows.iter() {
            if let Some((start, len)) = row.group {
                groups
                    .entry(start)
                    .or_insert_with(|| snap.selection.range_kind(start, len));
            }
        }
        let ctx = Rc::new(RowContext {
            repo: snap.repo,
            path: snap.path.clone(),
            selection: snap.selection.clone(),
            groups,
            selectable,
            hide_whitespace: snap.hide_whitespace,
            temp: self.temp,
            hovered_group: self.hovered_group,
            view: cx.weak_entity(),
            tokens: self.tokens.clone(),
            search: self.search_index(),
            show_check_marks: AppState::try_global(cx)
                .is_none_or(|s| s.read(cx).settings.show_diff_check_marks),
        });
        let rows = self.rows.clone();
        let search = self
            .search_input
            .clone()
            .filter(|_| self.searching)
            .map(|input| {
                // `.diff-search`: top right, hanging from the header
                div()
                    .absolute()
                    .top_0()
                    .right(SPACING)
                    .w(px(250.))
                    .p(SPACING_HALF)
                    .bg(t.background)
                    .border_1()
                    .border_t_0()
                    .border_color(t.box_border)
                    .rounded_b(BORDER_RADIUS)
                    .child(text_box("diff-search", &input, None, window, cx))
            });
        div()
            .id("diff")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
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
            .children(search)
            .into_any_element()
    }
}
