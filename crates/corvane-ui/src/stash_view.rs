//! Stash viewer - GHD `ui/stashing/{stash-diff-viewer,stash-diff-header}.tsx`
//! (`styles/ui/_stash-diff-viewer.scss`): "Stashed changes" header with
//! Restore / Discard, then a resizable file list beside the read-only diff.

use corvane_core::{AppState, CommittedFileChange, Dispatcher};
use gpui_kit::component::resizable::{
    ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::diff_view::{DiffSource, DiffView, diff_header, status_icon};
use crate::icons::octicon;
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::ListRowA11y;
use crate::widgets::{button, primary_button};

const FILE_LIST_MIN: Pixels = px(100.);
const FILE_LIST_MAX: Pixels = px(600.);

pub struct StashDiffViewer {
    state: Entity<AppState>,
    diff: Entity<DiffView>,
    resizable: Entity<ResizableState>,
    file_list_width: Pixels,
    /// The file list takes focus on click so ⌘9 / ⌘8 resize it.
    file_list_focus: FocusHandle,
}

impl StashDiffViewer {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let diff = cx.new(|cx| DiffView::new(state.clone(), DiffSource::Stash, cx));
        let file_list_width = px(state.read(cx).settings.commit_summary_width);
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
        }
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
                                    stash_file_row(id, file, is_selected, cx)
                                })
                                .collect()
                        })
                        .flex_1()
                        .min_h_0()
                        .with_scrollbar(),
                    ),
            )
            .into_any_element()
    }
}

fn stash_file_row(id: u64, file: &CommittedFileChange, is_selected: bool, cx: &App) -> AnyElement {
    let t = cx.ghd();
    let hover_bg = t.list_item_hover_background;
    let (icon, color) = status_icon(file.status.kind, t);
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
        .h(ROW_HEIGHT)
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF)
        .px(SPACING)
        .cursor_pointer()
        .when(is_selected, |d| {
            d.bg(t.box_selected_background)
                .text_color(t.box_selected_text)
        })
        .when(!is_selected, move |d| d.hover(move |s| s.bg(hover_bg)))
        .on_click(move |_, _, cx| Dispatcher::select_stash_file(id, path.clone(), cx))
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
                        .child(div().child(file.file_name().to_string())),
                ),
        )
        .child(octicon(icon, color))
        .into_any_element()
}

impl Render for StashDiffViewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .px(SPACING_DOUBLE)
                    .py(px(30.))
                    .border_b_1()
                    .border_color(t.box_border)
                    .child(
                        div()
                            .mb(SPACING)
                            .text_size(px(32.))
                            .line_height(px(32.))
                            .font_weight(FontWeight::LIGHT)
                            .child("Stashed changes"),
                    )
                    .child(
                        div()
                            .mt(SPACING)
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(
                                primary_button("stash-restore", "Restore", false, cx)
                                    .mr(SPACING)
                                    .on_click(move |_, _, cx| Dispatcher::pop_stash(id, cx)),
                            )
                            .child(
                                button("stash-discard", "Discard", cx).mr(SPACING).on_click(
                                    move |_, _, cx| Dispatcher::request_drop_stash(id, cx),
                                ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .ml(SPACING_HALF)
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .text_size(FONT_SIZE)
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
                            .size_range(FILE_LIST_MIN..FILE_LIST_MAX)
                            .child(crate::active_resizable::active_resizable(
                                "stash-file-list-resizable",
                                &self.resizable,
                                Some(&self.file_list_focus),
                                crate::active_resizable::ResizableDescription::new(
                                    "Stash file list",
                                    FILE_LIST_MIN..FILE_LIST_MAX,
                                ),
                                self.file_list(id, cx),
                            )),
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
                                .child(self.diff.clone()),
                        ),
                    ),
            )
            .into_any_element()
    }
}
