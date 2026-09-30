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

use crate::widgets::GhdTooltip;
use crate::widgets::IconButtonA11y;
use crate::widgets::ListRowA11y;

use crate::diff_view::{DiffSource, DiffView, diff_header, status_icon};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, mono_font};
use crate::widgets::{avatar_image, avatar_lookup, link_button};

/// `commitSummaryWidth` constraints (GHD `constrain(250, 100, 600)`).
#[allow(non_snake_case)]
fn FILE_LIST_MIN() -> Pixels {
    zpx(100.)
}
#[allow(non_snake_case)]
fn FILE_LIST_MAX() -> Pixels {
    zpx(600.)
}

pub struct SelectedCommitView {
    state: Entity<AppState>,
    diff: Entity<DiffView>,
    resizable: Entity<ResizableState>,
    file_list_width: Pixels,
    /// The file list takes focus on click so ⌘9 / ⌘8 resize it.
    file_list_focus: FocusHandle,
    /// `file_list_focus` held focus at the last render (active selection colours).
    file_list_focused: bool,
}

impl SelectedCommitView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let diff = cx.new(|cx| DiffView::new(state.clone(), DiffSource::Commit, cx));
        let file_list_width = zpx(state.read(cx).settings.commit_summary_width);
        let resizable = cx.new(|_| ResizableState::default());
        cx.subscribe(&resizable, |this, state, _: &ResizablePanelEvent, cx| {
            if let Some(width) = state.read(cx).sizes().first().copied()
                && width != this.file_list_width
            {
                this.file_list_width = width;
                Dispatcher::update_settings(cx, |s| s.commit_summary_width = unzoom(width));
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
                        .pt(SPACING())
                        .px(SPACING())
                        .pb(SPACING_HALF())
                        .text_size(FONT_SIZE_MD())
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(zpx(16.))
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
                            .px(SPACING())
                            .pb(SPACING_HALF())
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF())
                            .text_size(FONT_SIZE_SM())
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
                                .text_size(FONT_SIZE_SM())
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
                .mr(SPACING())
                .text_size(FONT_SIZE_SM())
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
                        .pt(SPACING())
                        .px(SPACING())
                        .pb(SPACING_HALF())
                        .text_size(FONT_SIZE_MD())
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(zpx(16.))
                        .when(empty, |d| d.text_color(t.text_secondary))
                        // the expander follows the title (`margin-left: 10px`)
                        .child(div().min_w_0().child(title))
                        .child(
                            div()
                                .id("commit-summary-expander")
                                .a11y_button(if expanded {
                                    "Collapse commit details"
                                } else {
                                    "Expand commit details"
                                })
                                .ghd_tooltip(if expanded { "Collapse" } else { "Expand" })
                                .ml(SPACING())
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
                        .px(SPACING())
                        .pb(SPACING_HALF())
                        .flex()
                        .flex_col()
                        .when(expanded, |d| d.max_h(zpx(400.)))
                        .when(!commit.body.is_empty(), |d| {
                            // `.ecs-description-text`: a 5 px padded box in
                            // `--box-alt-background-color`, 5 px above the meta row
                            d.child(
                                div().pb(SPACING_HALF()).child(
                                    // `.ecs-description-scroll-view`: 30–80 px
                                    // while collapsed
                                    div()
                                        .when(!expanded, |d| {
                                            d.min_h(zpx(30.)).max_h(zpx(80.)).overflow_hidden()
                                        })
                                        .child(
                                            div()
                                                .p(SPACING_HALF())
                                                .bg(t.box_alt_background)
                                                .font_family(mono_font())
                                                .text_size(FONT_SIZE_SM())
                                                .line_height(zpx(16.5))
                                                .child(description),
                                        ),
                                ),
                            )
                        })
                        .child(
                            // `.ecs-meta`
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .items_center()
                                .line_height(zpx(16.5))
                                .child(
                                    meta_item(div())
                                        .gap(zpx(4.))
                                        .child(avatar_image(
                                            avatar_lookup(&commit.author.email, cx),
                                            zpx(16.),
                                            cx,
                                        ))
                                        .child(commit.author.name.clone()),
                                )
                                .child(
                                    meta_item(div())
                                        .child(octicon(Octicon::GitCommit, t.text))
                                        .child(div().pl(SPACING_HALF()).child(if expanded {
                                            commit.sha.clone()
                                        } else {
                                            commit.short_sha().to_string()
                                        }))
                                        .child({
                                            let sha = commit.sha.clone();
                                            // `.copy-button`: 16 × 14 with a 12 px icon
                                            div()
                                                .id("copy-sha")
                                                .icon_button_label("Copy the full SHA")
                                                .ml(SPACING_HALF())
                                                .w(zpx(16.))
                                                .h(zpx(14.))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .on_click(move |_, _, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(sha.clone()),
                                                    )
                                                })
                                                .child(
                                                    octicon(Octicon::Copy, t.text).size(zpx(12.)),
                                                )
                                        }),
                                )
                                .when(added > 0 || deleted > 0, |d| {
                                    // `.lines-added-deleted { margin-left: auto }`
                                    d.child(
                                        meta_item(div())
                                            .ml_auto()
                                            .when(expanded, |d| {
                                                d.child(
                                                    octicon(Octicon::FileDiff, t.text_secondary)
                                                        .mr(SPACING_HALF()),
                                                )
                                            })
                                            .child(
                                                div()
                                                    .pr(SPACING_HALF())
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
                                                    .pr(SPACING_HALF())
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
                                            .child(octicon(Octicon::Tag, t.text).mr(SPACING_HALF()))
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
        let focus = self.file_list_focus.clone();
        let focused = self.file_list_focused;
        div()
            .size_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(t.box_border)
            .child(
                div()
                    .h(zpx(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(SPACING())
                    .bg(t.box_alt_background)
                    .border_b_1()
                    .border_color(t.box_border)
                    .text_size(FONT_SIZE())
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
                // a `List` node owning the file rows
                div()
                    .id("commit-files")
                    .role(Role::List)
                    .aria_label("Changed files")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        uniform_list("commit-file-rows", count, move |range, _, cx| {
                            range
                                .map(|ix| {
                                    let file = &files[ix];
                                    let is_selected =
                                        selected.as_deref() == Some(file.path.as_str());
                                    commit_file_row(id, file, is_selected, &focus, focused, cx)
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

/// History `FileList` row: dimmed directory + name, status icon (no checkbox).
/// GHD `SelectedCommits.onContextMenu`: open / reveal, copy paths, View on
/// GitHub; a file gone from disk gets a single disabled item.
fn open_commit_file_menu(
    id: u64,
    path: &str,
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::context_menu::MenuItem;
    let state = AppState::global(cx).read(cx);
    let Some(repo) = state.repository(id) else {
        return;
    };
    let full = repo.path.join(path);
    let editor_label = state.editor_label();
    let rs = state.repo_states.get(&id);
    let selected: Vec<String> = rs.map(|r| r.selected_commits.clone()).unwrap_or_default();
    // `localCommitSHAs`: here the newest unpushed commit is known
    let local = rs
        .and_then(|r| r.last_commit.as_ref())
        .is_some_and(|c| selected.first() == Some(&c.sha));
    let github = repo.github.clone();
    let items = if !full.exists() {
        vec![MenuItem::new("File Does Not Exist on Disk", |_, _| {}).enabled(false)]
    } else {
        let (reveal, editor, default, copy_full) =
            (full.clone(), full.clone(), full.clone(), full.clone());
        let relative = path.to_string();
        let view_label = match &github {
            Some(gh) if gh.endpoint != "https://api.github.com" => "View on GitHub Enterprise",
            _ => "View on GitHub",
        };
        let view_url = github.as_ref().and_then(|gh| {
            selected
                .first()
                .map(|sha| format!("{}/blob/{sha}/{path}", gh.html_url))
        });
        vec![
            MenuItem::new("Reveal in Finder", move |_, cx| cx.reveal_path(&reveal)),
            MenuItem::new(format!("Open in {editor_label}"), move |_, cx| {
                Dispatcher::open_in_editor(editor.clone(), cx)
            }),
            // `isSafeFileExtension` is always true on macOS
            MenuItem::new("Open with Default Program", move |_, cx| {
                cx.open_with_system(&default)
            }),
            MenuItem::separator(),
            MenuItem::new("Copy File Path", move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    copy_full.to_string_lossy().to_string(),
                ))
            }),
            MenuItem::new("Copy Relative File Path", move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(relative.clone()))
            }),
            MenuItem::separator(),
            MenuItem::new(view_label, move |_, cx| {
                if let Some(url) = &view_url {
                    Dispatcher::open_url(url, cx)
                }
            })
            .enabled(selected.len() == 1 && !local && github.is_some()),
        ]
    };
    #[cfg(target_os = "macos")]
    crate::native_menu::show_context_menu(items, position, window, cx);
    #[cfg(not(target_os = "macos"))]
    let _ = (items, position, window);
}

fn commit_file_row(
    id: u64,
    file: &CommittedFileChange,
    is_selected: bool,
    focus: &FocusHandle,
    list_focused: bool,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let hover_bg = t.list_item_hover_background;
    let (icon, color) = status_icon(file.status.kind, t);
    // `.focus-within .list-item.selected` has no status fill: the icon
    // takes the row's text colour
    let color = if is_selected && list_focused {
        t.box_selected_active_text
    } else {
        color
    };
    let path = file.path.clone();
    let menu_path = file.path.clone();
    div()
        .id(SharedString::from(format!("commit-file-{}", file.path)))
        // GHD `SelectedCommits.onContextMenu`
        .on_mouse_down(MouseButton::Right, {
            let focus = focus.clone();
            move |ev: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                // a right-click focuses the list and selects the file first
                // (`List.onRowMouseDown`)
                window.focus(&focus, cx);
                if !is_selected {
                    Dispatcher::select_commit_file(id, menu_path.clone(), cx);
                }
                open_commit_file_menu(id, &menu_path, ev.position, window, cx);
            }
        })
        // presses select at once and focus the list
        .on_mouse_down(MouseButton::Left, {
            let focus = focus.clone();
            let path = file.path.clone();
            move |_, window, cx| {
                window.focus(&focus, cx);
                if !is_selected {
                    Dispatcher::select_commit_file(id, path.clone(), cx);
                }
            }
        })
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
        // `.list-item { border-bottom: 1px solid var(--box-border-color) }`
        .border_b_1()
        .border_color(t.box_border)
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
        .when(
            !(is_selected && (list_focused || crate::widgets::selection_keeps_colour_on_hover(cx))),
            move |d| d.hover(move |s| s.bg(hover_bg)),
        )
        .on_click(move |_, _, cx| Dispatcher::select_commit_file(id, path.clone(), cx))
        .child(
            // GHD `PathText` keeps the file name visible and truncates the
            // directory part when the row is too narrow.
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .text_size(FONT_SIZE())
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        // `.list-item.selected .dirname` inherits the row colour
                        .text_color(match (is_selected, list_focused) {
                            (true, true) => t.box_selected_active_text,
                            (true, false) => t.box_selected_text,
                            _ => t.text_secondary,
                        })
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.file_list_focused = self.file_list_focus.is_focused(window);
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
                    .gap(SPACING_HALF())
                    .child("•")
                    .child(text)
            };
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_start()
                .p(SPACING_DOUBLE())
                .gap(SPACING_HALF())
                .bg(t.background)
                .text_color(t.text_secondary)
                .text_size(FONT_SIZE())
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
                            .size_range(FILE_LIST_MIN()..FILE_LIST_MAX())
                            .child(crate::active_resizable::active_resizable(
                                "commit-file-list-resizable",
                                &self.resizable,
                                Some(&self.file_list_focus),
                                crate::active_resizable::ResizableDescription::new(
                                    "Selected commit file list",
                                    FILE_LIST_MIN()..FILE_LIST_MAX(),
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
