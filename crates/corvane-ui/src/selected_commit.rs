//! Right pane of the History tab - GHD `ui/history/selected-commits.tsx`
//! with `expandable-commit-summary.tsx` and `file-list.tsx`
//! (`styles/ui/history/_expandable-commit-summary.scss`, `_commit-details.scss`):
//! title + expander, description, meta row (author, sha + copy, +adds −dels,
//! tags), then a resizable 250 px file list next to the commit's diff.

use corvane_core::{AppState, CommittedFileChange, Dispatcher, Popup, UnreachableCommitsTab};
use gpui_kit::component::resizable::{
    ResizablePanelEvent, ResizableState, h_resizable, resizable_panel,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::diff_view::{DiffSource, DiffView, diff_header, status_icon};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, MONO_FONT};
use crate::widgets::{avatar_image, avatar_lookup, link_button};

/// `commitSummaryWidth` constraints (GHD `constrain(250, 100, 600)`).
const FILE_LIST_MIN: Pixels = px(100.);
const FILE_LIST_MAX: Pixels = px(600.);

pub struct SelectedCommitView {
    state: Entity<AppState>,
    diff: Entity<DiffView>,
    resizable: Entity<ResizableState>,
    file_list_width: Pixels,
    /// The file list takes focus on click so ⌘9 / ⌘8 resize it.
    file_list_focus: FocusHandle,
}

impl SelectedCommitView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let diff = cx.new(|cx| DiffView::new(state.clone(), DiffSource::Commit, cx));
        let file_list_width = px(state.read(cx).settings.commit_summary_width);
        let resizable = cx.new(|_| ResizableState::default());
        cx.subscribe(&resizable, |this, state, _: &ResizablePanelEvent, cx| {
            if let Some(width) = state.read(cx).sizes().first().copied()
                && width != this.file_list_width
            {
                this.file_list_width = width;
                Dispatcher::update_settings(cx, |s| s.commit_summary_width = f32::from(width));
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

    /// `ExpandableCommitSummary` for a contiguous multi-commit selection:
    /// "Showing changes from N commits" (+ how many are unreachable).
    fn multi_summary(&self, id: u64, cx: &Context<Self>) -> Option<AnyElement> {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let rs = s.repo_states.get(&id)?;
        let selected = rs.selected_commits.len();
        if selected <= 1 {
            return None;
        }
        let shas_in_diff: Vec<String> = rs.shas_in_diff.clone();
        let shas_not_in_diff: Vec<String> = rs
            .selected_commits
            .iter()
            .filter(|sha| !rs.shas_in_diff.contains(sha))
            .cloned()
            .collect();
        let not_in_diff = shas_not_in_diff.len();
        let in_diff = selected - not_in_diff;
        // `onHighlightShas`: hovering either count dims the other rows.
        let highlight = |shas: Vec<String>| {
            move |hovered: &bool, _: &mut Window, cx: &mut App| {
                Dispatcher::set_highlighted_shas(
                    id,
                    if *hovered { shas.clone() } else { Vec::new() },
                    cx,
                )
            }
        };
        Some(
            div()
                .id("expandable-commit-summary")
                .flex_none()
                .flex()
                .flex_col()
                .border_b_1()
                .border_color(t.box_border)
                .child(
                    div()
                        .id("commits-in-diff")
                        .pt(SPACING)
                        .px(SPACING)
                        .pb(SPACING_HALF)
                        .text_size(FONT_SIZE_MD)
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(16.))
                        .on_hover(highlight(shas_in_diff))
                        .child(format!(
                            "Showing changes from {in_diff} {}",
                            if in_diff == 1 { "commit" } else { "commits" }
                        )),
                )
                .when(not_in_diff > 0, |d| {
                    // `renderCommitsNotReachable` (`.commit-unreachable-info`)
                    d.child(
                        div()
                            .px(SPACING)
                            .pb(SPACING_HALF)
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF)
                            .text_size(FONT_SIZE_SM)
                            .text_color(t.text_secondary)
                            .child(octicon(Octicon::Info, t.text_secondary))
                            .child(
                                link_button(
                                    "commits-not-in-diff",
                                    format!(
                                        "{not_in_diff} unreachable {}",
                                        if not_in_diff == 1 {
                                            "commit"
                                        } else {
                                            "commits"
                                        }
                                    ),
                                    cx,
                                )
                                .text_size(FONT_SIZE_SM)
                                .on_hover(highlight(shas_not_in_diff))
                                .on_click(move |_, _, cx| {
                                    Dispatcher::set_highlighted_shas(id, Vec::new(), cx);
                                    Dispatcher::show_popup(
                                        Popup::UnreachableCommits {
                                            repo: id,
                                            tab: UnreachableCommitsTab::Unreachable,
                                        },
                                        cx,
                                    )
                                }),
                            )
                            .child("not included."),
                    )
                })
                .into_any_element(),
        )
    }

    /// `ExpandableCommitSummary`
    fn summary(&self, id: u64, cx: &Context<Self>) -> Option<AnyElement> {
        if let Some(multi) = self.multi_summary(id, cx) {
            return Some(multi);
        }
        let t = cx.ghd();
        let s = self.state.read(cx);
        let rs = s.repo_states.get(&id)?;
        let sha = rs.selected_commit.as_ref()?;
        let commit = rs.commits.iter().find(|c| &c.sha == sha)?.clone();
        let expanded = rs.commit_summary_expanded;
        let (added, deleted) = rs
            .changeset
            .as_ref()
            .map(|c| (c.lines_added, c.lines_deleted))
            .unwrap_or((0, 0));
        let empty = commit.summary.is_empty();
        let title = if empty {
            "Empty commit message".to_string()
        } else {
            commit.summary.clone()
        };
        let description = if expanded {
            commit.body.clone()
        } else {
            // `-webkit-line-clamp: 3`
            commit.body.lines().take(3).collect::<Vec<_>>().join("\n")
        };
        let meta_item = |d: Div| {
            d.flex()
                .flex_row()
                .items_center()
                .mr(SPACING)
                .text_size(FONT_SIZE_SM)
        };
        Some(
            div()
                .id("expandable-commit-summary")
                .flex_none()
                .flex()
                .flex_col()
                .min_h_0()
                .border_b_1()
                .border_color(t.box_border)
                .child(
                    // `.ecs-title`
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .pt(SPACING)
                        .px(SPACING)
                        .pb(SPACING_HALF)
                        .text_size(FONT_SIZE_MD)
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(16.))
                        .when(empty, |d| d.text_color(t.text_secondary))
                        .child(div().flex_1().min_w_0().child(title))
                        .child(
                            div()
                                .id("commit-summary-expander")
                                .ml(SPACING)
                                .flex_none()
                                .cursor_pointer()
                                .on_click(move |_, _, cx| {
                                    Dispatcher::set_commit_summary_expanded(id, !expanded, cx)
                                })
                                .child(octicon(
                                    if expanded {
                                        Octicon::Fold
                                    } else {
                                        Octicon::Unfold
                                    },
                                    t.text,
                                )),
                        ),
                )
                .child(
                    // `.beneath-summary`
                    div()
                        .id("ecs-scroll")
                        .overflow_y_scroll()
                        .px(SPACING)
                        .pb(SPACING_HALF)
                        .flex()
                        .flex_col()
                        .when(expanded, |d| d.max_h(px(400.)))
                        .when(!commit.body.is_empty(), |d| {
                            d.child(
                                div()
                                    .pb(SPACING_HALF)
                                    .font_family(MONO_FONT)
                                    .text_size(FONT_SIZE_SM)
                                    .child(description),
                            )
                        })
                        .child(
                            // `.ecs-meta`
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .items_center()
                                .child(
                                    meta_item(div())
                                        .gap(px(4.))
                                        .child(avatar_image(
                                            avatar_lookup(&commit.author.email, cx),
                                            px(16.),
                                            cx,
                                        ))
                                        .child(commit.author.name.clone()),
                                )
                                .child(
                                    meta_item(div())
                                        .child(octicon(Octicon::GitCommit, t.text_secondary))
                                        .child(div().pl(SPACING_HALF).child(if expanded {
                                            commit.sha.clone()
                                        } else {
                                            commit.short_sha().to_string()
                                        }))
                                        .child({
                                            let sha = commit.sha.clone();
                                            div()
                                                .id("copy-sha")
                                                .ml(SPACING_HALF)
                                                .cursor_pointer()
                                                .on_click(move |_, _, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(sha.clone()),
                                                    )
                                                })
                                                .child(octicon(Octicon::Copy, t.text_secondary))
                                        }),
                                )
                                .when(added > 0 || deleted > 0, |d| {
                                    d.child(
                                        meta_item(div())
                                            .when(expanded, |d| {
                                                d.child(
                                                    octicon(Octicon::FileDiff, t.text_secondary)
                                                        .mr(SPACING_HALF),
                                                )
                                            })
                                            .child(
                                                div()
                                                    .pr(SPACING_HALF)
                                                    .text_color(t.color_new)
                                                    .child(if expanded {
                                                        format!(
                                                            "{} added lines",
                                                            crate::format::format_count(added)
                                                        )
                                                    } else {
                                                        format!(
                                                            "+{}",
                                                            crate::format::format_count(added)
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .pr(SPACING_HALF)
                                                    .text_color(t.color_deleted)
                                                    .child(if expanded {
                                                        format!("{deleted} removed lines")
                                                    } else {
                                                        format!("-{deleted}")
                                                    }),
                                            ),
                                    )
                                })
                                .when(!commit.tags.is_empty(), |d| {
                                    d.child(
                                        meta_item(div())
                                            .min_w_0()
                                            .child(
                                                octicon(Octicon::Tag, t.text_secondary)
                                                    .mr(SPACING_HALF),
                                            )
                                            .child(div().truncate().child(commit.tags.join(", "))),
                                    )
                                }),
                        )
                        .with_scrollbar(),
                )
                .into_any_element(),
        )
    }

    /// `FileList` + `.file-list-header`
    fn file_list(&self, id: u64, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let rs = s.repo_states.get(&id);
        let files: Vec<CommittedFileChange> = rs
            .and_then(|r| r.changeset.as_ref())
            .map(|c| c.files.clone())
            .unwrap_or_default();
        let selected = rs.and_then(|r| r.commit_selected_file.clone());
        if rs.and_then(|r| r.changeset.as_ref()).is_some() && files.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(t.text_secondary)
                .child("No files in commit")
                .into_any_element();
        }
        let count = files.len();
        let files = std::rc::Rc::new(files);
        div()
            .size_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(t.box_border)
            .child(
                div()
                    .h(px(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(SPACING)
                    .bg(t.box_alt_background)
                    .border_b_1()
                    .border_color(t.box_border)
                    .text_size(FONT_SIZE)
                    .child(if count == 1 {
                        "1 changed file".to_string()
                    } else {
                        format!(
                            "{} changed files",
                            crate::format::format_count(count as u64)
                        )
                    }),
            )
            .child(
                uniform_list("commit-file-rows", count, move |range, _, cx| {
                    range
                        .map(|ix| {
                            let file = &files[ix];
                            let is_selected = selected.as_deref() == Some(file.path.as_str());
                            commit_file_row(id, file, is_selected, cx)
                        })
                        .collect()
                })
                .flex_1()
                .min_h_0()
                .with_scrollbar(),
            )
            .into_any_element()
    }
}

/// History `FileList` row: dimmed directory + name, status icon (no checkbox).
fn commit_file_row(id: u64, file: &CommittedFileChange, is_selected: bool, cx: &App) -> AnyElement {
    let t = cx.ghd();
    let hover_bg = t.list_item_hover_background;
    let (icon, color) = status_icon(file.status.kind, t);
    let path = file.path.clone();
    div()
        .id(SharedString::from(format!("commit-file-{}", file.path)))
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
        .on_click(move |_, _, cx| Dispatcher::select_commit_file(id, path.clone(), cx))
        .child(
            // GHD `PathText` keeps the file name visible and truncates the
            // directory part when the row is too narrow.
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .text_size(FONT_SIZE)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(t.text_secondary)
                        .child(file.directory().to_string()),
                )
                .child(
                    div()
                        .flex_none()
                        .max_w_full()
                        .truncate()
                        .child(file.file_name().to_string()),
                ),
        )
        .child(octicon(icon, color))
        .into_any_element()
}

impl Render for SelectedCommitView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let author_email = self.state.read(cx).selected_state().and_then(|rs| {
            let sha = rs.selected_commit.as_ref()?;
            rs.commits
                .iter()
                .find(|c| &c.sha == sha)
                .map(|c| c.author.email.clone())
        });
        if let Some(email) = author_email {
            Dispatcher::request_avatar_for_email(&email, cx);
        }
        let t = cx.ghd();
        let (id, has_commit, selected_file, non_contiguous) = {
            let s = self.state.read(cx);
            let id = s.selected;
            let rs = id.and_then(|id| s.repo_states.get(&id));
            (
                id,
                rs.map(|r| r.selected_commit.is_some()).unwrap_or(false),
                rs.and_then(|r| {
                    let path = r.commit_selected_file.as_ref()?;
                    r.changeset
                        .as_ref()?
                        .files
                        .iter()
                        .find(|f| &f.path == path)
                        .map(|f| (f.path.clone(), f.status.kind))
                }),
                rs.is_some_and(|r| r.selected_commits.len() > 1 && !r.commits_contiguous),
            )
        };
        if non_contiguous {
            // `renderMultipleCommitsBlankSlate`
            let bullet = |text: &'static str| {
                div()
                    .flex()
                    .flex_row()
                    .gap(SPACING_HALF)
                    .child("•")
                    .child(text)
            };
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_start()
                .p(SPACING_DOUBLE)
                .gap(SPACING_HALF)
                .bg(t.background)
                .text_color(t.text_secondary)
                .text_size(FONT_SIZE)
                .child("Unable to display diff when multiple non-consecutive selected.")
                .child("You can:")
                .child(bullet(
                    "Select a single commit or a range of consecutive commits to view a diff.",
                ))
                .child(bullet(
                    "Drag the commits to the branch menu to cherry-pick them.",
                ))
                .child(bullet("Drag the commits to squash or reorder them."))
                .child(bullet("Right click on multiple commits to see options."))
                .into_any_element();
        }
        let Some(id) = id.filter(|_| has_commit) else {
            // GHD `NoCommitSelected`
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(t.background)
                .text_color(t.text_secondary)
                .child("No commit selected")
                .into_any_element();
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(t.background)
            .children(self.summary(id, cx))
            .child(
                h_resizable("commit-details")
                    .with_state(&self.resizable)
                    .with_handle_appearance(std::rc::Rc::new(|_, _, _| {
                        Some(div().into_any_element())
                    }))
                    .child(
                        resizable_panel()
                            .size(self.file_list_width)
                            .size_range(FILE_LIST_MIN..FILE_LIST_MAX)
                            .child(crate::active_resizable::active_resizable(
                                "commit-file-list-resizable",
                                &self.resizable,
                                Some(&self.file_list_focus),
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
