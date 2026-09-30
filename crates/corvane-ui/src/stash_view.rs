//! Stash viewer - GHD `ui/stashing/{stash-diff-viewer,stash-diff-header}.tsx`
//! (`styles/ui/_stash-diff-viewer.scss`): "Stashed changes" header with
//! Restore / Discard, then a resizable file list beside the read-only diff.

use corvane_core::{AppState, CommittedFileChange, Dispatcher};
use gpui_kit::component::resizable::{
    ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{
    ExtendSelectionDown, ExtendSelectionUp, SelectFirstFile, SelectLastFile, SelectNextFile,
    SelectPreviousFile,
};
use crate::diff_view::{DiffSource, DiffView, diff_header, status_icon};
use crate::icons::octicon;
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::ListRowA11y;
use crate::widgets::{button, primary_button};

#[allow(non_snake_case)]
fn FILE_LIST_MIN() -> Pixels {
    zpx(100.)
}
#[allow(non_snake_case)]
fn FILE_LIST_MAX() -> Pixels {
    zpx(600.)
}

pub struct StashDiffViewer {
    state: Entity<AppState>,
    diff: Entity<DiffView>,
    resizable: Entity<ResizableState>,
    file_list_width: Pixels,
    /// The file list takes focus on click so ⌘9 / ⌘8 resize it.
    file_list_focus: FocusHandle,
    /// `file_list_focus` held focus at the last render (active selection colours).
    file_list_focused: bool,
    file_scroll: UniformListScrollHandle,
}

impl StashDiffViewer {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let diff = cx.new(|cx| DiffView::new(state.clone(), DiffSource::Stash, cx));
        let file_list_width = zpx(state.read(cx).settings.commit_summary_width);
        let resizable = cx.new(|_| ResizableState::default());
        cx.subscribe(&resizable, |this, state, _: &ResizablePanelEvent, cx| {
            if let Some(width) = state.read(cx).sizes().first().copied()
                && width != this.file_list_width
            {
                this.file_list_width = width;
                cx.notify();
            }
        })
        .detach();
        Self {
            state,
            diff,
            resizable,
            file_list_width,
            file_list_focus: cx.focus_handle(),
            file_list_focused: false,
            file_scroll: UniformListScrollHandle::new(),
        }
    }

    /// The repository, the stash's files in list order and the selected
    /// file's index.
    fn file_order(&self, cx: &App) -> Option<(u64, Vec<String>, Option<usize>)> {
        let s = self.state.read(cx);
        let id = s.selected?;
        let rs = s.repo_states.get(&id)?;
        let order: Vec<String> = rs
            .stash_files
            .as_ref()
            .map(|f| f.iter().map(|f| f.path.clone()).collect())
            .unwrap_or_default();
        let current = rs
            .stash_selected_file
            .as_ref()
            .and_then(|p| order.iter().position(|o| o == p));
        Some((id, order, current))
    }

    /// GHD `List.moveSelection` on the stash's `FileList` (↑ / ↓, and ⌥↓ /
    /// ⌥↑ from the diff; single selection, so ⇧↑ / ⇧↓ too): the file
    /// `delta` rows away, clamped at the ends, scrolled into view.
    pub fn select_relative(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some((id, order, current)) = self.file_order(cx) else {
            return;
        };
        if let Some(ix) = corvane_core::list_selection::step_index(order.len(), current, delta) {
            self.select_index(id, &order, ix, cx);
        }
    }

    /// Home / End, ⌘↑ / ⌘↓: the first or last file.
    fn select_edge(&mut self, last: bool, cx: &mut Context<Self>) {
        let Some((id, order, _)) = self.file_order(cx) else {
            return;
        };
        if !order.is_empty() {
            let ix = if last { order.len() - 1 } else { 0 };
            self.select_index(id, &order, ix, cx);
        }
    }

    fn select_index(&mut self, id: u64, order: &[String], ix: usize, cx: &mut Context<Self>) {
        Dispatcher::select_stash_file(id, order[ix].clone(), cx);
        self.file_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
    }

    fn file_list(&self, id: u64, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let rs = s.repo_states.get(&id);
        let files: Vec<CommittedFileChange> =
            rs.and_then(|r| r.stash_files.clone()).unwrap_or_default();
        let selected = rs.and_then(|r| r.stash_selected_file.clone());
        let count = files.len();
        let files = std::rc::Rc::new(files);
        let scroll = self.file_scroll.clone();
        let focused = self.file_list_focused;
        div()
            .size_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(t.box_border)
            .child(
                // a `List` node owning the file rows
                div()
                    .id("stash-files")
                    .role(Role::List)
                    .aria_label("Changed files")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        uniform_list("stash-file-rows", count, move |range, _, cx| {
                            range
                                .map(|ix| {
                                    let file = &files[ix];
                                    let is_selected =
                                        selected.as_deref() == Some(file.path.as_str());
                                    stash_file_row(id, file, is_selected, focused, cx)
                                })
                                .collect()
                        })
                        .flex_1()
                        .min_h_0()
                        .with_scrollbar_handle(&scroll),
                    ),
            )
            .into_any_element()
    }
}

fn stash_file_row(
    id: u64,
    file: &CommittedFileChange,
    is_selected: bool,
    list_focused: bool,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let hover_bg = t.list_item_hover_background;
    let (icon, color) = status_icon(file.status.kind, t);
    // `.focus-within .list-item.selected`: the icon takes the row's colour
    let color = if is_selected && list_focused {
        t.box_selected_active_text
    } else {
        color
    };
    let path = file.path.clone();
    div()
        .id(SharedString::from(format!("stash-file-{}", file.path)))
        .a11y_row(
            format!(
                "{}, {}",
                file.path,
                crate::widgets::status_label(file.status.kind)
            ),
            is_selected,
        )
        .w_full()
        .h(ROW_HEIGHT())
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        .px(SPACING())
        .cursor_pointer()
        .when(is_selected, |d| {
            if list_focused {
                d.bg(t.box_selected_active_background)
                    .text_color(t.box_selected_active_text)
            } else {
                d.bg(t.box_selected_background)
                    .text_color(t.box_selected_text)
            }
        })
        // `.list-item:hover` outranks `.list-item.selected` (flag 104 keeps it)
        .when(
            !(is_selected && (list_focused || crate::widgets::selection_keeps_colour_on_hover(cx))),
            move |d| d.hover(move |s| s.bg(hover_bg)),
        )
        .on_click(move |_, _, cx| Dispatcher::select_stash_file(id, path.clone(), cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(FONT_SIZE())
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .child(
                            div()
                                // `.list-item.selected .dirname` inherits the row colour
                                .text_color(match (is_selected, list_focused) {
                                    (true, true) => t.box_selected_active_text,
                                    (true, false) => t.box_selected_text,
                                    _ => t.text_secondary,
                                })
                                .child(file.directory().to_string()),
                        )
                        .child(div().child(file.file_name().to_string())),
                ),
        )
        .child(octicon(icon, color))
        .into_any_element()
}

impl Render for StashDiffViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.file_list_focused = self.file_list_focus.is_focused(window);
        let t = cx.ghd();
        let (id, selected_file) = {
            let s = self.state.read(cx);
            let id = s.selected;
            let rs = id.and_then(|id| s.repo_states.get(&id));
            (
                id,
                rs.and_then(|r| {
                    let path = r.stash_selected_file.as_ref()?;
                    r.stash_files
                        .as_ref()?
                        .iter()
                        .find(|f| &f.path == path)
                        .map(|f| (f.path.clone(), f.status.kind))
                }),
            )
        };
        let Some(id) = id else {
            return div().size_full().into_any_element();
        };
        div()
            .id("stash-diff-viewer")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(t.background)
            .child(
                // `.header`
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .px(SPACING_DOUBLE())
                    .py(zpx(30.))
                    .border_b_1()
                    .border_color(t.box_border)
                    .child(
                        div()
                            .mb(SPACING())
                            .text_size(zpx(32.))
                            .line_height(zpx(32.))
                            .font_weight(FontWeight::LIGHT)
                            .child("Stashed changes"),
                    )
                    .child(
                        div()
                            .mt(SPACING())
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(
                                primary_button("stash-restore", "Restore", false, cx)
                                    .mr(SPACING())
                                    .on_click(move |_, _, cx| Dispatcher::pop_stash(id, cx)),
                            )
                            .child(
                                button("stash-discard", "Discard", cx)
                                    .mr(SPACING())
                                    .on_click(move |_, _, cx| {
                                        Dispatcher::request_drop_stash(id, cx)
                                    }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .ml(SPACING_HALF())
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .text_size(FONT_SIZE())
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Restore"))
                                    .child(
                                        "\u{a0}will move your stashed files to the Changes list.",
                                    ),
                            ),
                    ),
            )
            .child(
                h_resizable("stash-details")
                    .with_state(&self.resizable)
                    .with_handle_appearance(std::rc::Rc::new(|_, _, _| {
                        Some(div().into_any_element())
                    }))
                    .child(
                        resizable_panel()
                            .size(self.file_list_width)
                            .size_range(FILE_LIST_MIN()..FILE_LIST_MAX())
                            .child(
                                crate::active_resizable::active_resizable(
                                    "stash-file-list-resizable",
                                    &self.resizable,
                                    Some(&self.file_list_focus),
                                    crate::active_resizable::ResizableDescription::new(
                                        "Stash file list",
                                        FILE_LIST_MIN()..FILE_LIST_MAX(),
                                    ),
                                    self.file_list(id, cx),
                                )
                                .key_context("StashFileList")
                                .on_action(cx.listener(|this, _: &SelectNextFile, _, cx| {
                                    this.select_relative(1, cx)
                                }))
                                .on_action(cx.listener(|this, _: &SelectPreviousFile, _, cx| {
                                    this.select_relative(-1, cx)
                                }))
                                .on_action(cx.listener(|this, _: &ExtendSelectionDown, _, cx| {
                                    this.select_relative(1, cx)
                                }))
                                .on_action(cx.listener(|this, _: &ExtendSelectionUp, _, cx| {
                                    this.select_relative(-1, cx)
                                }))
                                .on_action(cx.listener(|this, _: &SelectFirstFile, _, cx| {
                                    this.select_edge(false, cx)
                                }))
                                .on_action(cx.listener(
                                    |this, _: &SelectLastFile, _, cx| this.select_edge(true, cx),
                                )),
                            ),
                    )
                    .child(
                        resizable_panel().child(
                            div()
                                .size_full()
                                .flex()
                                .flex_col()
                                .min_h_0()
                                .when_some(selected_file, |d, (path, kind)| {
                                    d.child(diff_header(&path, kind, &self.diff, cx))
                                })
                                .child(DiffView::embed(&self.diff)),
                        ),
                    ),
            )
            .into_any_element()
    }
}
