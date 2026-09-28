//! History sidebar - GHD `ui/history/compare.tsx` + `commit-list.tsx` +
//! `commit-list-item.tsx` (`styles/ui/history/_history.scss`,
//! `_commit-list.scss`): "Select Branch to Compare…" box, then 50 px commit
//! rows (bold summary; avatar + "author • time" byline; tag badges). The list
//! is virtualized and pages in `COMMIT_BATCH_SIZE` commits as it scrolls.
//! Compare-to-branch, multi-select and the unpushed indicator come with the
//! branch/remote milestones.

use std::rc::Rc;

use corvane_core::{AppState, Commit, Dispatcher, Popup};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{ContextMenu, MenuItem};
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{avatar_placeholder, text_box};

/// `RowHeight` in `commit-list.tsx`
pub const COMMIT_ROW_HEIGHT: Pixels = px(50.);

pub struct HistorySidebar {
    state: Entity<AppState>,
    compare: Entity<InputState>,
    list_focus: FocusHandle,
    context_menu: Option<Entity<ContextMenu>>,
}

impl HistorySidebar {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let compare =
            cx.new(|cx| InputState::new(window, cx).placeholder("Select Branch to Compare…"));
        Self {
            state,
            compare,
            list_focus: cx.focus_handle(),
            context_menu: None,
        }
    }

    fn open_menu(
        &mut self,
        items: Vec<MenuItem>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = cx.new(|cx| ContextMenu::new(position, items, window, cx));
        cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.context_menu = None;
            cx.notify();
        })
        .detach();
        self.context_menu = Some(menu);
        cx.notify();
    }

    /// GHD `onCommitsSelectedContextMenu` (single commit). Items whose
    /// operations are not implemented yet are shown disabled.
    fn open_commit_menu(
        &mut self,
        commit: Commit,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (id, html_url, is_head) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else { return };
            let html_url = s
                .repository(id)
                .and_then(|r| r.github.as_ref())
                .map(|g| g.html_url.clone());
            let is_head = s
                .repo_states
                .get(&id)
                .and_then(|rs| rs.commits.first())
                .map(|c| c.sha == commit.sha)
                .unwrap_or(false);
            (id, html_url, is_head)
        };
        Dispatcher::select_commit(id, commit.sha.clone(), cx);
        let sha = commit.sha.clone();
        let mut items = Vec::new();
        if is_head {
            items.push(MenuItem::new("Amend Commit…", {
                let sha = sha.clone();
                move |_, cx| Dispatcher::start_amending(id, sha.clone(), cx)
            }));
            items.push(MenuItem::new("Undo Commit…", move |_, cx| {
                Dispatcher::request_undo_commit(id, cx)
            }));
        }
        items.extend([
            MenuItem::new("Reset to Commit…", {
                let sha = sha.clone();
                move |_, cx| Dispatcher::request_reset_to_commit(id, sha.clone(), cx)
            })
            .enabled(!is_head),
            MenuItem::new("Checkout Commit", {
                let sha = sha.clone();
                move |_, cx| Dispatcher::request_checkout_commit(id, sha.clone(), cx)
            })
            .enabled(!is_head),
            // TODO(M4): reorder / cherry-pick / create branch need the branch layer
            MenuItem::new("Reorder Commit", |_, _| {}).enabled(false),
            MenuItem::new("Revert Changes in Commit", {
                let sha = sha.clone();
                move |_, cx| Dispatcher::revert_commit(id, sha.clone(), cx)
            }),
            MenuItem::separator(),
            MenuItem::new("Create Branch from Commit", |_, _| {}).enabled(false),
            MenuItem::new("Create Tag…", {
                let sha = sha.clone();
                move |_, cx| {
                    Dispatcher::show_popup(
                        Popup::CreateTag {
                            repo: id,
                            sha: sha.clone(),
                        },
                        cx,
                    )
                }
            }),
        ]);
        if !commit.tags.is_empty() {
            items.push(MenuItem::separator());
            if commit.tags.len() == 1 {
                let tag = commit.tags[0].clone();
                items.push(MenuItem::new(format!("Delete tag {tag}"), move |_, cx| {
                    Dispatcher::delete_tag(id, tag.clone(), cx)
                }));
            } else {
                let entries = commit
                    .tags
                    .iter()
                    .map(|tag| {
                        let tag = tag.clone();
                        MenuItem::new(tag.clone(), move |_, cx| {
                            Dispatcher::delete_tag(id, tag.clone(), cx)
                        })
                    })
                    .collect();
                items.push(MenuItem::submenu("Delete tag…", entries));
            }
        }
        items.extend([
            MenuItem::new("Cherry-pick Commit…", |_, _| {}).enabled(false),
            MenuItem::separator(),
            MenuItem::new("Copy SHA", {
                let sha = sha.clone();
                move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(sha.clone()))
            }),
        ]);
        let tags = commit.tags.join(" ");
        items.push(
            MenuItem::new(
                if commit.tags.len() > 1 {
                    "Copy Tags"
                } else {
                    "Copy Tag"
                },
                move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(tags.clone())),
            )
            .enabled(!commit.tags.is_empty()),
        );
        items.push(
            MenuItem::new("View on GitHub", {
                let url = html_url.clone().map(|u| format!("{u}/commit/{sha}"));
                move |_, cx| {
                    if let Some(url) = &url {
                        Dispatcher::open_url(url, cx);
                    }
                }
            })
            .enabled(html_url.is_some()),
        );
        self.open_menu(items, position, window, cx);
    }

    fn commit_list(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let Some(id) = s.selected else {
            return div().flex_1().into_any_element();
        };
        let rs = s.repo_states.get(&id);
        let commits: Rc<Vec<Commit>> = Rc::new(rs.map(|r| r.commits.clone()).unwrap_or_default());
        let selected = rs.and_then(|r| r.selected_commit.clone());
        let exhausted = rs.map(|r| r.commits_exhausted).unwrap_or(true);
        let loaded = rs.map(|r| r.info.is_some()).unwrap_or(false);
        if commits.is_empty() {
            let message = if loaded && exhausted {
                "No history"
            } else {
                ""
            };
            return div()
                .flex_1()
                .flex()
                .items_start()
                .justify_center()
                .pt(SPACING_DOUBLE)
                .text_color(t.text_secondary)
                .child(message)
                .into_any_element();
        }
        let weak = cx.weak_entity();
        let list_focus = self.list_focus.clone();
        let count = commits.len();
        // GHD keeps the active (blue) selection while the row's context menu is open
        let menu_open = self.context_menu.is_some();
        div()
            .id("commit-list")
            .track_focus(&self.list_focus)
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                uniform_list("commit-list-rows", count, move |range, window, cx| {
                    // GHD `onScroll` → `loadNextCommitBatch` near the end of the list
                    if !exhausted && range.end + 20 >= count {
                        Dispatcher::load_commits(id, true, cx);
                    }
                    let focused = list_focus.is_focused(window) || menu_open;
                    range
                        .map(|ix| {
                            let commit = &commits[ix];
                            let is_selected = selected.as_deref() == Some(commit.sha.as_str());
                            commit_row(
                                id,
                                commit,
                                is_selected,
                                focused,
                                weak.clone(),
                                list_focus.clone(),
                                cx,
                            )
                        })
                        .collect()
                })
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }
}

/// `CommitListItem`
fn commit_row(
    id: u64,
    commit: &Commit,
    is_selected: bool,
    list_focused: bool,
    weak: WeakEntity<HistorySidebar>,
    list_focus: FocusHandle,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let sha = commit.sha.clone();
    let summary = if commit.summary.is_empty() {
        "Empty commit message".to_string()
    } else {
        commit.summary.clone()
    };
    let empty = commit.summary.is_empty();
    let byline = format!(
        "{} • {}",
        commit.author.name,
        relative(commit.author.date())
    );
    let (bg, text, secondary, badge_bg, badge_text) = if is_selected && list_focused {
        (
            t.box_selected_active_background,
            t.box_selected_active_text,
            t.box_selected_active_text,
            t.list_item_selected_active_badge_background,
            t.list_item_selected_active_badge_text,
        )
    } else if is_selected {
        (
            t.box_selected_background,
            t.box_selected_text,
            t.box_selected_text,
            t.list_item_selected_badge_background,
            t.list_item_selected_badge_text,
        )
    } else {
        (
            t.background,
            t.text,
            t.text_secondary,
            t.list_item_badge_background,
            t.list_item_badge_text,
        )
    };
    let commit_for_menu = commit.clone();
    div()
        .id(SharedString::from(format!("commit-{}", commit.sha)))
        .w_full()
        .h(COMMIT_ROW_HEIGHT)
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .pl(SPACING)
        .pr(SPACING + SPACING_HALF)
        .bg(bg)
        .text_color(text)
        .border_b_1()
        .border_color(t.box_border)
        .cursor_pointer()
        .when(!is_selected, |d| {
            let hover = t.list_item_hover_background;
            d.hover(move |s| s.bg(hover))
        })
        .on_click({
            let sha = sha.clone();
            let list_focus = list_focus.clone();
            move |_, window, cx| {
                window.focus(&list_focus, cx);
                Dispatcher::select_commit(id, sha.clone(), cx)
            }
        })
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                let position = ev.position;
                let commit = commit_for_menu.clone();
                window.focus(&list_focus, cx);
                weak.update(cx, |this, cx| {
                    this.open_commit_menu(commit, position, window, cx)
                })
                .ok();
            },
        )
        .child(
            // `.info`
            div()
                .flex_1()
                .min_w(px(50.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(FONT_SIZE)
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .when(empty, |d| d.text_color(secondary))
                        .child(summary),
                )
                .child(
                    // `.description`: avatar stack + byline
                    div()
                        .mt(px(3.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF)
                        .child(avatar_placeholder(px(16.), cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(FONT_SIZE)
                                .text_color(secondary)
                                .child(byline),
                        ),
                ),
        )
        .when(!commit.tags.is_empty(), |d| {
            // `.commit-indicators .tag-indicator`
            d.child(
                div()
                    .ml(SPACING)
                    .h(px(16.))
                    .max_w(gpui_kit::relative(0.5))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .child(octicon(Octicon::Tag, badge_text).size(px(12.)))
                    .child(
                        div()
                            .px(px(6.))
                            .h(px(16.))
                            .rounded(px(8.))
                            .bg(badge_bg)
                            .text_color(badge_text)
                            .text_size(FONT_SIZE_SM)
                            .line_height(px(16.))
                            .truncate()
                            .child(commit.tags[0].clone()),
                    )
                    .when(commit.tags.len() > 1, |d| {
                        d.child(
                            div()
                                .px(px(6.))
                                .h(px(16.))
                                .rounded(px(8.))
                                .bg(badge_bg)
                                .text_color(badge_text)
                                .text_size(FONT_SIZE_SM)
                                .line_height(px(16.))
                                .child(format!("+{}", commit.tags.len() - 1)),
                        )
                    }),
            )
        })
        .into_any_element()
}

impl Render for HistorySidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                // `#compare-view .compare-form`
                div()
                    .flex_none()
                    .p(SPACING_HALF)
                    .bg(t.box_alt_background)
                    .border_b_1()
                    .border_color(t.box_border)
                    .child(text_box(
                        "compare-branch",
                        &self.compare,
                        Some(octicon(Octicon::GitBranch, t.text_secondary)),
                        window,
                        cx,
                    )),
            )
            .child(self.commit_list(cx))
            .children(self.context_menu.clone())
    }
}
