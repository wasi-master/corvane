//! History sidebar - GHD `ui/history/compare.tsx` + `commit-list.tsx` +
//! `commit-list-item.tsx` (`styles/ui/history/_history.scss`,
//! `_commit-list.scss`, `drag-elements/_commit-drag-element.scss`):
//! "Select Branch to Compare…" box, then 50 px commit rows (bold summary;
//! avatar + "author • time" byline; tag badges). The list is virtualized and
//! pages in `COMMIT_BATCH_SIZE` commits as it scrolls. Commits multi-select
//! with ⌘/⇧-click, drag to squash onto another commit, to reorder (drop
//! between rows) or to cherry-pick onto a branch in the branch foldout, and
//! "Reorder Commit" starts the keyboard insertion mode (↑/↓, ⏎, Esc).
//! Compare-to-branch and the unpushed indicator come with the remote
//! milestone. Drop tooltips ("Copy to …", "Squash N commits")
//! are not shown; the drop targets highlight instead.

use std::rc::Rc;

use corvane_core::{AppState, Commit, Dispatcher, Popup};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{
    ReorderCancel, ReorderConfirm, ReorderMoveDown, ReorderMoveUp, SelectNextFile,
    SelectPreviousFile,
};
use crate::context_menu::{ContextMenu, MenuItem};
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{avatar_placeholder, kbd, text_box};

/// `RowHeight` in `commit-list.tsx`
pub const COMMIT_ROW_HEIGHT: Pixels = px(50.);

/// GHD `CommitDragData`: what a commit drag carries (drop targets in the
/// branch foldout and toolbar read it too).
#[derive(Clone, Debug)]
pub struct CommitDrag {
    pub repo: u64,
    /// The dragged selection, click order.
    pub shas: Vec<String>,
    /// The commit the drag started on, rendered in the drag element.
    pub commit: Commit,
}

/// `CommitDragElement`: the row that follows the cursor, with a red count
/// badge for multi-commit drags.
pub struct CommitDragElement {
    drag: CommitDrag,
}

impl Render for CommitDragElement {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let count = self.drag.shas.len();
        div()
            .relative()
            .w(px(300.))
            .h(COMMIT_ROW_HEIGHT)
            .mt(px(22.))
            .child(
                div()
                    .size_full()
                    .bg(t.background)
                    .text_color(t.text)
                    .border_t_1()
                    .border_l_1()
                    .border_color(t.box_border)
                    .shadow(vec![BoxShadow {
                        color: t.box_border,
                        offset: point(px(2.), px(1.)),
                        blur_radius: px(1.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .overflow_hidden()
                    .child(commit_row_contents(
                        &self.drag.commit,
                        t.text,
                        t.text_secondary,
                        cx,
                    )),
            )
            .when(count > 1, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(-22.))
                        .left(px(20.))
                        .size(px(18.))
                        .rounded_full()
                        .bg(rgb(0xd73a49))
                        .text_color(white())
                        .text_size(FONT_SIZE_SM)
                        .font_weight(FontWeight::MEDIUM)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(count.to_string()),
                )
            })
    }
}

/// Where a dragged commit would land on a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DropHint {
    /// Squash the dragged commits onto this row's commit.
    Squash(usize),
    /// Insert the dragged commits before row `n` (`n == count` = after the last).
    InsertAt(usize),
}

pub struct HistorySidebar {
    state: Entity<AppState>,
    compare: Entity<InputState>,
    list_focus: FocusHandle,
    context_menu: Option<Entity<ContextMenu>>,
    /// Live drop target while a commit drag is over the list.
    drop_hint: Option<DropHint>,
    /// Keyboard reorder mode (`keyboardReorderData`): shas + insertion row.
    reorder: Option<(Vec<String>, usize)>,
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
            drop_hint: None,
            reorder: None,
        }
    }

    fn open_menu(
        &mut self,
        items: Vec<MenuItem>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // macOS: a real NSMenu (GHD's Electron `Menu.popup`); the GPUI menu is the fallback.
        #[cfg(target_os = "macos")]
        {
            crate::native_menu::show_context_menu(items, position, window, cx);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let menu = cx.new(|cx| ContextMenu::new(position, items, window, cx));
            cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
                this.context_menu = None;
                cx.notify();
            })
            .detach();
            self.context_menu = Some(menu);
            cx.notify();
        }
    }

    fn selection(&self, id: u64, cx: &App) -> Vec<String> {
        self.state
            .read(cx)
            .repo_states
            .get(&id)
            .map(|r| r.selected_commits.clone())
            .unwrap_or_default()
    }

    fn mco_in_progress(&self, id: u64, cx: &App) -> bool {
        self.state
            .read(cx)
            .repo_states
            .get(&id)
            .is_some_and(|r| r.mco.is_some())
    }

    /// `onRowContextMenu`
    fn open_row_menu(
        &mut self,
        commit: Commit,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.reorder.is_some() {
            return;
        }
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let selection = self.selection(id, cx);
        if selection.len() > 1 && selection.contains(&commit.sha) {
            self.open_multi_commit_menu(id, commit, selection, position, window, cx);
        } else {
            self.open_commit_menu(id, commit, position, window, cx);
        }
    }

    /// `getContextMenuMultipleCommits`
    fn open_multi_commit_menu(
        &mut self,
        id: u64,
        commit: Commit,
        selection: Vec<String>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = selection.len();
        let busy = self.mco_in_progress(id, cx);
        let weak = cx.weak_entity();
        let (s1, s2, s3) = (selection.clone(), selection.clone(), selection);
        let onto = commit.sha.clone();
        let items = vec![
            MenuItem::new(format!("Cherry-pick {count} Commits…"), move |_, cx| {
                Dispatcher::start_cherry_pick_flow(id, s1.clone(), cx)
            })
            .enabled(!busy),
            MenuItem::new(format!("Squash {count} Commits…"), move |_, cx| {
                Dispatcher::request_squash(id, s2.clone(), onto.clone(), cx)
            })
            .enabled(!busy),
            MenuItem::new(format!("Reorder {count} Commits…"), move |_, cx| {
                weak.update(cx, |this, cx| {
                    this.start_keyboard_reorder(id, s3.clone(), cx)
                })
                .ok();
            })
            .enabled(!busy),
        ];
        self.open_menu(items, position, window, cx);
    }

    /// GHD `getContextMenuForSingleCommit`.
    fn open_commit_menu(
        &mut self,
        id: u64,
        commit: Commit,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (html_url, is_head, busy) = {
            let s = self.state.read(cx);
            let html_url = s
                .repository(id)
                .and_then(|r| r.github.as_ref())
                .map(|g| g.html_url.clone());
            let rs = s.repo_states.get(&id);
            let is_head = rs
                .and_then(|rs| rs.commits.first())
                .map(|c| c.sha == commit.sha)
                .unwrap_or(false);
            (html_url, is_head, rs.is_some_and(|r| r.mco.is_some()))
        };
        Dispatcher::select_commit(id, commit.sha.clone(), cx);
        let sha = commit.sha.clone();
        let weak = cx.weak_entity();
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
            MenuItem::new("Reorder Commit", {
                let sha = sha.clone();
                move |_, cx| {
                    weak.update(cx, |this, cx| {
                        this.start_keyboard_reorder(id, vec![sha.clone()], cx)
                    })
                    .ok();
                }
            })
            .enabled(!busy),
            MenuItem::new("Revert Changes in Commit", {
                let sha = sha.clone();
                move |_, cx| Dispatcher::revert_commit(id, sha.clone(), cx)
            }),
            MenuItem::separator(),
            MenuItem::new("Create Branch from Commit", {
                let sha = sha.clone();
                move |_, cx| {
                    Dispatcher::show_popup(
                        Popup::CreateBranch {
                            repo: id,
                            target_sha: Some(sha.clone()),
                            initial_name: String::new(),
                        },
                        cx,
                    )
                }
            }),
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
            MenuItem::new("Cherry-pick Commit…", {
                let sha = sha.clone();
                move |_, cx| Dispatcher::start_cherry_pick_flow(id, vec![sha.clone()], cx)
            })
            .enabled(!busy),
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

    // ---- keyboard reorder (`onKeyboardReorder`, list keyboard insertion) ----

    fn start_keyboard_reorder(&mut self, id: u64, shas: Vec<String>, cx: &mut Context<Self>) {
        let first = {
            let s = self.state.read(cx);
            s.repo_states
                .get(&id)
                .and_then(|r| {
                    shas.iter()
                        .filter_map(|sha| r.commits.iter().position(|c| &c.sha == sha))
                        .min()
                })
                .unwrap_or(0)
        };
        Dispatcher::select_commits(id, shas.clone(), cx);
        self.reorder = Some((shas, first));
        cx.notify();
    }

    fn cancel_keyboard_reorder(&mut self, cx: &mut Context<Self>) {
        if self.reorder.take().is_some() {
            cx.notify();
        }
    }

    fn move_insertion(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self
            .state
            .read(cx)
            .selected_state()
            .map(|r| r.commits.len())
            .unwrap_or(0);
        if let Some((_, insertion)) = self.reorder.as_mut() {
            let next = (*insertion as isize + delta).clamp(0, count as isize) as usize;
            *insertion = next;
            cx.notify();
        }
    }

    fn confirm_keyboard_reorder(&mut self, cx: &mut Context<Self>) {
        let Some((shas, insertion)) = self.reorder.take() else {
            return;
        };
        cx.notify();
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        self.drop_insertion(id, shas, insertion, cx);
    }

    /// `onDropDataInsertion`: the dragged commits go right before row
    /// `insertion` (the commit above becomes `beforeCommit`).
    fn drop_insertion(&self, id: u64, shas: Vec<String>, insertion: usize, cx: &mut App) {
        let commits: Vec<String> = self
            .state
            .read(cx)
            .repo_states
            .get(&id)
            .map(|r| r.commits.iter().map(|c| c.sha.clone()).collect())
            .unwrap_or_default();
        if commits.is_empty() || insertion > commits.len() {
            return;
        }
        let mut indexes: Vec<usize> = shas
            .iter()
            .filter_map(|sha| commits.iter().position(|c| c == sha))
            .collect();
        indexes.sort_unstable();
        let contiguous = indexes.windows(2).all(|w| w[1] == w[0] + 1);
        if contiguous && let Some(&first) = indexes.first() {
            let base = insertion.checked_sub(1);
            let above_themselves =
                (base.is_none() && first == 0) || base == Some(first.wrapping_sub(1));
            let within_themselves = base.is_some_and(|b| indexes.contains(&b));
            if above_themselves || within_themselves {
                return;
            }
        }
        let before = insertion.checked_sub(1).map(|i| commits[i].clone());
        Dispatcher::reorder_commits(id, shas, before, false, cx);
    }

    /// `.reorder-commits-hint-popover`
    fn reorder_hint(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .absolute()
            .top(SPACING_HALF)
            .left(SPACING_HALF)
            .right(SPACING_HALF)
            .p(SPACING)
            .rounded(BORDER_RADIUS)
            .bg(t.background)
            .border_1()
            .border_color(t.box_border)
            .shadow(vec![BoxShadow {
                color: t.shadow,
                offset: point(px(0.), px(2.)),
                blur_radius: px(7.),
                spread_radius: px(0.),
                inset: false,
            }])
            .text_size(FONT_SIZE)
            .flex()
            .flex_col()
            .gap(SPACING_HALF)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Reorder Commits"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(3.))
                    .child("Use")
                    .child(kbd("↑", cx))
                    .child(kbd("↓", cx))
                    .child("to choose a new location."),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(3.))
                    .child("Press")
                    .child(kbd("⏎", cx))
                    .child("to confirm."),
            )
    }

    fn commit_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let Some(id) = s.selected else {
            return div().flex_1().into_any_element();
        };
        let rs = s.repo_states.get(&id);
        let commits: Rc<Vec<Commit>> = Rc::new(rs.map(|r| r.commits.clone()).unwrap_or_default());
        let selected: Rc<Vec<String>> =
            Rc::new(rs.map(|r| r.selected_commits.clone()).unwrap_or_default());
        let exhausted = rs.map(|r| r.commits_exhausted).unwrap_or(true);
        let loaded = rs.map(|r| r.info.is_some()).unwrap_or(false);
        let draggable = rs.is_some_and(|r| r.mco.is_none()) && self.reorder.is_none();
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
        let drop_hint = self.drop_hint;
        let reorder = self.reorder.clone();
        let in_reorder = reorder.is_some();
        div()
            .id("commit-list")
            .key_context("HistoryList")
            .track_focus(&self.list_focus)
            .on_action(cx.listener(|this, _: &ReorderMoveUp, _, cx| this.move_insertion(-1, cx)))
            .on_action(cx.listener(|this, _: &ReorderMoveDown, _, cx| this.move_insertion(1, cx)))
            .on_action(
                cx.listener(|this, _: &ReorderConfirm, _, cx| this.confirm_keyboard_reorder(cx)),
            )
            .on_action(
                cx.listener(|this, _: &ReorderCancel, _, cx| this.cancel_keyboard_reorder(cx)),
            )
            .on_action(cx.listener(move |this, _: &SelectNextFile, _, cx| {
                if this.reorder.is_some() {
                    this.move_insertion(1, cx);
                } else {
                    this.step_selection(id, 1, cx);
                }
            }))
            .on_action(cx.listener(move |this, _: &SelectPreviousFile, _, cx| {
                if this.reorder.is_some() {
                    this.move_insertion(-1, cx);
                } else {
                    this.step_selection(id, -1, cx);
                }
            }))
            .relative()
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
                            let is_selected = selected.contains(&commit.sha);
                            let insertion_here = match (&reorder, drop_hint) {
                                (Some((_, at)), _) => Some(*at),
                                (None, Some(DropHint::InsertAt(at))) => Some(at),
                                _ => None,
                            };
                            let row_hint = RowHint {
                                squash_target: drop_hint == Some(DropHint::Squash(ix)),
                                line_above: insertion_here == Some(ix),
                                line_below: insertion_here == Some(ix + 1) && ix + 1 == count,
                                keyboard_selected: in_reorder && is_selected,
                            };
                            commit_row(
                                id,
                                ix,
                                commit,
                                is_selected,
                                focused && !in_reorder,
                                draggable,
                                selected.clone(),
                                row_hint,
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
            .when(in_reorder, |d| d.child(self.reorder_hint(cx)))
            .into_any_element()
    }

    /// ↑/↓ moves a single-row selection (`onSelectedRowChanged`).
    fn step_selection(&self, id: u64, delta: isize, cx: &mut App) {
        let next = {
            let s = self.state.read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let current = rs
                .selected_commit
                .as_ref()
                .and_then(|sha| rs.commits.iter().position(|c| &c.sha == sha));
            let ix = match current {
                Some(ix) => (ix as isize + delta).clamp(0, rs.commits.len() as isize - 1) as usize,
                None => 0,
            };
            rs.commits.get(ix).map(|c| c.sha.clone())
        };
        if let Some(sha) = next {
            Dispatcher::select_commit(id, sha, cx);
        }
    }

    /// Drag-move over a row: pick squash (middle) or insertion (edges).
    fn update_drop_hint(&mut self, hint: Option<DropHint>, cx: &mut Context<Self>) {
        if self.drop_hint != hint {
            self.drop_hint = hint;
            cx.notify();
        }
    }

    /// Drop on a row: squash or reorder, per the last hint.
    fn drop_on_row(&mut self, id: u64, row: usize, drag: &CommitDrag, cx: &mut Context<Self>) {
        let hint = self.drop_hint.take();
        cx.notify();
        if drag.repo != id {
            return;
        }
        let commits: Vec<String> = self
            .state
            .read(cx)
            .repo_states
            .get(&id)
            .map(|r| r.commits.iter().map(|c| c.sha.clone()).collect())
            .unwrap_or_default();
        match hint {
            Some(DropHint::Squash(target)) if target == row => {
                let Some(onto) = commits.get(row) else { return };
                if drag.shas.iter().all(|s| s == onto) {
                    return;
                }
                Dispatcher::request_squash(id, drag.shas.clone(), onto.clone(), cx);
            }
            Some(DropHint::InsertAt(at)) => self.drop_insertion(id, drag.shas.clone(), at, cx),
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Default)]
struct RowHint {
    squash_target: bool,
    line_above: bool,
    line_below: bool,
    keyboard_selected: bool,
}

/// `.commit .info` + tag indicators, shared with the drag element.
fn commit_row_contents(commit: &Commit, text: Hsla, secondary: Hsla, cx: &App) -> Div {
    let t = cx.ghd();
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
    div()
        .size_full()
        .flex()
        .flex_row()
        .items_center()
        .pl(SPACING)
        .pr(SPACING + SPACING_HALF)
        .text_color(text)
        .child(
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
            d.child(
                div()
                    .ml(SPACING)
                    .h(px(16.))
                    .max_w(gpui_kit::relative(0.5))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .child(octicon(Octicon::Tag, t.list_item_badge_text).size(px(12.)))
                    .child(
                        div()
                            .px(px(6.))
                            .h(px(16.))
                            .rounded(px(8.))
                            .bg(t.list_item_badge_background)
                            .text_color(t.list_item_badge_text)
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
                                .bg(t.list_item_badge_background)
                                .text_color(t.list_item_badge_text)
                                .text_size(FONT_SIZE_SM)
                                .line_height(px(16.))
                                .child(format!("+{}", commit.tags.len() - 1)),
                        )
                    }),
            )
        })
}

/// `CommitListItem`
#[allow(clippy::too_many_arguments)]
fn commit_row(
    id: u64,
    ix: usize,
    commit: &Commit,
    is_selected: bool,
    list_focused: bool,
    draggable: bool,
    selection: Rc<Vec<String>>,
    hint: RowHint,
    weak: WeakEntity<HistorySidebar>,
    list_focus: FocusHandle,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let sha = commit.sha.clone();
    let (bg, text, secondary) = if hint.keyboard_selected {
        (
            t.box_selected_background,
            t.box_selected_text,
            t.box_selected_text,
        )
    } else if is_selected && list_focused {
        (
            t.box_selected_active_background,
            t.box_selected_active_text,
            t.box_selected_active_text,
        )
    } else if is_selected {
        (
            t.box_selected_background,
            t.box_selected_text,
            t.box_selected_text,
        )
    } else if hint.squash_target {
        (t.list_item_hover_background, t.text, t.text_secondary)
    } else {
        (t.background, t.text, t.text_secondary)
    };
    let commit_for_menu = commit.clone();
    let commit_for_drag = commit.clone();
    let drag_shas: Vec<String> = if selection.contains(&commit.sha) && !selection.is_empty() {
        selection.as_ref().clone()
    } else {
        vec![commit.sha.clone()]
    };
    let weak_for_move = weak.clone();
    let weak_for_drop = weak.clone();
    let line = t.box_selected_active_background;
    div()
        .id(SharedString::from(format!("commit-{}", commit.sha)))
        .relative()
        .w_full()
        .h(COMMIT_ROW_HEIGHT)
        .flex_none()
        .bg(bg)
        .border_b_1()
        .border_color(t.box_border)
        .cursor_pointer()
        .when(!is_selected && !hint.squash_target, |d| {
            let hover = t.list_item_hover_background;
            d.hover(move |s| s.bg(hover))
        })
        .on_click({
            let sha = sha.clone();
            let list_focus = list_focus.clone();
            move |ev: &ClickEvent, window, cx| {
                window.focus(&list_focus, cx);
                let modifiers = ev.modifiers();
                if modifiers.secondary() {
                    Dispatcher::toggle_commit_selection(id, sha.clone(), cx);
                } else if modifiers.shift {
                    Dispatcher::extend_commit_selection(id, sha.clone(), cx);
                } else {
                    Dispatcher::select_commit(id, sha.clone(), cx);
                }
            }
        })
        .on_mouse_down(MouseButton::Right, {
            let weak = weak.clone();
            move |ev: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                let position = ev.position;
                let commit = commit_for_menu.clone();
                window.focus(&list_focus, cx);
                weak.update(cx, |this, cx| {
                    this.open_row_menu(commit, position, window, cx)
                })
                .ok();
            }
        })
        .when(draggable, |d| {
            d.on_drag(
                CommitDrag {
                    repo: id,
                    shas: drag_shas,
                    commit: commit_for_drag,
                },
                |drag, _, _, cx| {
                    let drag = drag.clone();
                    cx.new(|_| CommitDragElement { drag })
                },
            )
        })
        .on_drag_move::<CommitDrag>(move |ev, _, cx| {
            let bounds = ev.bounds;
            let pos = ev.event.position;
            let hint = if bounds.contains(&pos) {
                let rel = (pos.y - bounds.origin.y) / bounds.size.height;
                if rel < 0.25 {
                    Some(DropHint::InsertAt(ix))
                } else if rel > 0.75 {
                    Some(DropHint::InsertAt(ix + 1))
                } else {
                    Some(DropHint::Squash(ix))
                }
            } else {
                None
            };
            if hint.is_some() {
                weak_for_move
                    .update(cx, |this, cx| this.update_drop_hint(hint, cx))
                    .ok();
            }
        })
        .on_drop(move |drag: &CommitDrag, _, cx| {
            weak_for_drop
                .update(cx, |this, cx| this.drop_on_row(id, ix, drag, cx))
                .ok();
        })
        .child(commit_row_contents(commit, text, secondary, cx))
        .when(hint.line_above, |d| {
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(2.))
                    .bg(line),
            )
        })
        .when(hint.line_below, |d| {
            d.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(2.))
                    .bg(line),
            )
        })
        .into_any_element()
}

impl Render for HistorySidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        // drop hints only live while a drag is in flight
        if self.drop_hint.is_some() && !cx.has_active_drag() {
            self.drop_hint = None;
        }
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
