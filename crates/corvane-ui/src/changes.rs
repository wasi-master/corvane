//! Changes sidebar: filter header, "N changed files" row, file list, commit form.
//! `styles/ui/changes/{_changes-list,_commit-message}.scss`.

use corvane_core::{AppState, DiffSelection, Dispatcher, Tip};
use gpui_kit::component::Sizable;
use gpui_kit::component::input::{InputState, Textarea, TextareaState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::diff_view::status_icon;
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{avatar_placeholder, checkbox, primary_button, text_box};

pub struct ChangesSidebar {
    filter: Entity<InputState>,
    summary: Entity<InputState>,
    description: Entity<TextareaState>,
    state: Entity<AppState>,
}

impl ChangesSidebar {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        let summary = cx.new(|cx| InputState::new(window, cx).placeholder("Summary (required)"));
        let description = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder("Description")
        });
        Self {
            filter,
            summary,
            description,
            state,
        }
    }

    /// `.filtered-changes-list .header`: filter row + check-all row.
    fn header(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .px(SPACING)
            .py(SPACING_HALF)
            .bg(t.box_alt_background)
            .border_b_1()
            .border_color(t.box_border)
            .child(
                // Filter row: [Filter Options ▾][Filter…] as a joined button group
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(TEXT_FIELD_HEIGHT)
                    .child(
                        div()
                            .id("filter-options")
                            .h(TEXT_FIELD_HEIGHT)
                            .w(px(48.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(2.))
                            .border_1()
                            .border_color(t.secondary_button_border)
                            .rounded_l(BORDER_RADIUS)
                            .bg(t.secondary_button_background)
                            .cursor_pointer()
                            .child(octicon(Octicon::Filter, t.secondary_button_text))
                            .child(
                                octicon(Octicon::TriangleDown, t.secondary_button_text)
                                    .size(px(12.)),
                            ),
                    )
                    .child(
                        text_box("changes-filter", &self.filter, None, window, cx)
                            .rounded_l(px(0.))
                            .border_l_0(),
                    ),
            )
            .child(
                // "☑ N changed files"
                div()
                    .h(ROW_HEIGHT)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    // GHD shows the include-all box checked but disabled when there is nothing to commit.
                    .child({
                        let (count, include_all, repo_id) = self.header_state(cx);
                        checkbox("check-all", include_all != Some(false), count == 0, cx)
                            .when(include_all.is_none(), |d| d.opacity(0.7))
                            .when_some(repo_id.filter(|_| count > 0), |d, id| {
                                d.on_click(move |_, _, cx| Dispatcher::toggle_include_all(id, cx))
                            })
                    })
                    .child({
                        let (count, _, _) = self.header_state(cx);
                        div().text_size(FONT_SIZE).truncate().child(if count == 1 {
                            "1 changed file".to_string()
                        } else {
                            format!("{count} changed files")
                        })
                    }),
            )
    }

    fn branch_name(&self, cx: &App) -> SharedString {
        self.state
            .read(cx)
            .selected_state()
            .and_then(|s| s.info.as_ref())
            .and_then(|i| match &i.tip {
                Tip::Valid { branch } => Some(branch.name.clone()),
                Tip::Unborn { name } => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_default()
            .into()
    }

    fn header_state(&self, cx: &App) -> (usize, Option<bool>, Option<u64>) {
        let s = self.state.read(cx);
        let id = s.selected;
        let status = s.selected_state().and_then(|rs| rs.status.as_ref());
        (
            status.map(|st| st.files.len()).unwrap_or(0),
            status.map(|st| st.include_all()).unwrap_or(Some(true)),
            id,
        )
    }

    /// `ChangesList`: 29 px rows - checkbox, dimmed directory + bold name, status icon.
    fn list(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let repo_id = s.selected;
        let rs = s.selected_state();
        let files: Vec<_> = rs
            .and_then(|r| r.status.as_ref())
            .map(|st| st.files.clone())
            .unwrap_or_default();
        let selected = rs.and_then(|r| r.selected_file.clone());
        let hover_bg = t.list_item_hover_background;
        div()
            .id("changes-list")
            .flex_1()
            .min_h(px(100.))
            .overflow_y_scroll()
            .bg(t.background)
            .flex()
            .flex_col()
            .children(files.into_iter().map(|file| {
                let is_selected = selected.as_deref() == Some(file.path.as_str());
                let (icon, color) = status_icon(file.status.kind, t);
                let path_for_select = file.path.clone();
                let path_for_toggle = file.path.clone();
                let included = file.selection != DiffSelection::None;
                div()
                    .id(SharedString::from(format!("file-{}", file.path)))
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
                    .when_some(repo_id, move |d, id| {
                        d.on_click(move |_, _, cx| {
                            Dispatcher::select_file(id, path_for_select.clone(), cx)
                        })
                    })
                    .child(
                        checkbox(
                            SharedString::from(format!("include-{}", file.path)),
                            included,
                            false,
                            cx,
                        )
                        .when(file.selection == DiffSelection::Partial, |d| d.opacity(0.7))
                        .when_some(repo_id, move |d, id| {
                            d.on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                Dispatcher::toggle_file_included(id, path_for_toggle.clone(), cx)
                            })
                        }),
                    )
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
            }))
    }

    /// `.commit-message-component`
    fn commit_form(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .id("commit-message")
            .key_context("CommitMessage")
            .flex_none()
            .flex()
            .flex_col()
            .p(SPACING)
            .bg(t.box_alt_background)
            .border_t_1()
            .border_color(t.box_border)
            .child(
                // `.summary`: avatar + summary field
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .mb(SPACING)
                    .child(avatar_placeholder(AVATAR_SIZE, cx))
                    .child(text_box("commit-summary", &self.summary, None, window, cx)),
            )
            .child(
                // `.description-focus-container`: textarea + action bar
                div()
                    .flex()
                    .flex_col()
                    .mb(SPACING)
                    .border_1()
                    .border_color(t.box_border_contrast)
                    .rounded(BORDER_RADIUS)
                    .bg(t.box_background)
                    .overflow_hidden()
                    .child(
                        Textarea::new(&self.description)
                            .appearance(false)
                            .small()
                            .h(px(80.)),
                    )
                    .child(
                        // `.action-bar`: add co-authors | commit options
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF)
                            .px(SPACING)
                            .pb(px(8.))
                            .child(octicon(Octicon::PersonAdd, t.text_secondary))
                            .child(div().w(px(1.)).h(px(16.)).bg(t.box_border_contrast))
                            .child(octicon(Octicon::Gear, t.text_secondary)),
                    ),
            )
            .child(
                primary_button(
                    "commit",
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.))
                        .child("Commit to")
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.branch_name(cx)),
                        ),
                    self.summary.read(cx).value().trim().is_empty(),
                    cx,
                )
                .w_full(),
            )
    }
}

impl Render for ChangesSidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(self.header(window, cx))
            .child(self.list(cx))
            .child(self.commit_form(window, cx))
    }
}
