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

use corvane_core::{
    AppState, Commit, ComparisonMode, Dispatcher, DropTarget, Mergeability,
    MultiCommitOperationKind, Popup,
};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{
    CompareClear, CompareSelect, ReorderCancel, ReorderConfirm, ReorderMoveDown, ReorderMoveUp,
    SelectNextFile, SelectPreviousFile,
};
use crate::branch_list::group_branches;
use crate::context_menu::{ContextMenu, MenuItem};
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{avatar_image, avatar_lookup, kbd, primary_button, text_box};

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
    state: Entity<AppState>,
}

impl CommitDragElement {
    fn new(drag: CommitDrag, cx: &mut Context<Self>) -> Self {
        let state = AppState::global(cx);
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { drag, state }
    }

    /// `renderDragToolTip`: what a drop would do at the current target.
    fn tooltip(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let t = cx.ghd();
        let target = self.state.read(cx).drag_target.clone()?;
        let content: AnyElement = match target {
            DropTarget::Branch(name) => div()
                .flex()
                .flex_row()
                .child("Copy to")
                .child(
                    div()
                        .ml(SPACING_THIRD)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(name),
                )
                .into_any_element(),
            DropTarget::Commit => div()
                .child(format!("Squash {} commits", self.drag.shas.len() + 1))
                .into_any_element(),
            DropTarget::InsertionPoint { count } => div()
                .child(format!(
                    "Move {} here",
                    if count == 1 { "commit" } else { "commits" }
                ))
                .into_any_element(),
        };
        Some(
            // `.tool-tip-contents` (darwin): title-tooltip look under the box
            div()
                .absolute()
                .left_0()
                .bottom(px(-25.))
                .px(SPACING_THIRD)
                .py(px(1.))
                .rounded(px(1.))
                .bg(t.tooltip_background)
                .text_color(t.tooltip_text)
                .text_size(FONT_SIZE_SM)
                .whitespace_nowrap()
                .shadow(vec![BoxShadow {
                    color: t.shadow,
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(3.),
                    spread_radius: px(0.),
                    inset: false,
                }])
                .child(content)
                .into_any_element(),
        )
    }
}

impl Render for CommitDragElement {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let count = self.drag.shas.len();
        let tooltip = self.tooltip(cx);
        div()
            .relative()
            .w(px(300.))
            .h(COMMIT_ROW_HEIGHT)
            .mt(px(22.))
            .children(tooltip)
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
    /// Focus edge detection for the compare box (`onTextBoxFocused`).
    compare_was_focused: bool,
    /// Keyboard-focused branch in the compare list (`focusedBranch`).
    focused_branch: Option<String>,
    /// Merge call to action dropdown choice (`selectedOperation`).
    merge_option: MultiCommitOperationKind,
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
            compare_was_focused: false,
            focused_branch: None,
            merge_option: MultiCommitOperationKind::Merge,
        }
    }

    /// Branch › Compare to Branch: focus the compare box, which opens the list.
    pub fn focus_compare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.compare.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        if let Some(id) = self.state.read(cx).selected {
            Dispatcher::set_compare_branch_list_visible(id, true, cx);
        }
    }

    fn compare_query(&self, cx: &App) -> String {
        self.compare.read(cx).value().trim().to_string()
    }

    /// Branches offered for comparison: everything but the current branch
    /// (`_initializeCompare`), grouped like the branch foldout.
    fn compare_groups(&self, id: u64, cx: &App) -> Vec<crate::branch_list::BranchGroup> {
        let query = self.compare_query(cx);
        let s = self.state.read(cx);
        let Some(rs) = s.repo_states.get(&id) else {
            return Vec::new();
        };
        let Some(info) = rs.info.as_ref() else {
            return Vec::new();
        };
        let current = info.current_branch().map(|b| b.name.clone());
        let branches: Vec<corvane_core::Branch> = info
            .branches
            .iter()
            .filter(|b| Some(&b.name) != current.as_ref())
            .cloned()
            .collect();
        let default = rs
            .default_branch
            .as_deref()
            .filter(|d| Some(*d) != current.as_deref());
        let recent: Vec<String> = rs
            .recent_branches
            .iter()
            .filter(|r| Some(*r) != current.as_ref())
            .cloned()
            .collect();
        group_branches(&branches, default, &recent, &query)
    }

    fn compare_branch_names(&self, id: u64, cx: &App) -> Vec<String> {
        self.compare_groups(id, cx)
            .into_iter()
            .flat_map(|g| g.branches.into_iter().map(|b| b.name))
            .collect()
    }

    /// Enter in the compare box (`onBranchFilterKeyDown`).
    fn compare_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let names = self.compare_branch_names(id, cx);
        let pick = self
            .focused_branch
            .clone()
            .filter(|b| names.contains(b))
            .or_else(|| names.first().cloned());
        match pick {
            Some(branch) if !self.compare_query(cx).is_empty() || self.focused_branch.is_some() => {
                self.choose_compare_branch(id, branch, window, cx);
            }
            _ => {
                Dispatcher::exit_compare(id, cx);
                Dispatcher::set_compare_branch_list_visible(id, false, cx);
                window.blur(cx);
            }
        }
    }

    /// Escape / clear (`handleEscape`): back to the history.
    /// GHD `onBranchFilterBlur`: hide the list; an empty filter with nothing
    /// chosen drops back to the plain history.
    fn compare_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        self.focused_branch = None;
        Dispatcher::set_compare_branch_list_visible(id, false, cx);
        if self.compare_query(cx).is_empty() {
            Dispatcher::exit_compare(id, cx);
        }
        window.blur(cx);
        cx.notify();
    }

    fn compare_clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        self.focused_branch = None;
        self.compare.update(cx, |s, cx| s.set_value("", window, cx));
        Dispatcher::exit_compare(id, cx);
        Dispatcher::set_compare_branch_list_visible(id, false, cx);
        window.blur(cx);
        cx.notify();
    }

    fn choose_compare_branch(
        &mut self,
        id: u64,
        branch: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focused_branch = None;
        self.compare
            .update(cx, |s, cx| s.set_value(branch.clone(), window, cx));
        Dispatcher::compare_to_branch(id, branch, ComparisonMode::Behind, cx);
        window.blur(cx);
        cx.notify();
    }

    fn move_focused_branch(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let names = self.compare_branch_names(id, cx);
        if names.is_empty() {
            return;
        }
        let current = self
            .focused_branch
            .as_ref()
            .and_then(|b| names.iter().position(|n| n == b));
        let next = match current {
            Some(ix) => (ix as isize + delta).clamp(0, names.len() as isize - 1) as usize,
            None => 0,
        };
        self.focused_branch = Some(names[next].clone());
        cx.notify();
    }

    /// `CompareBranchListItem` rows inside `BranchList`.
    fn compare_branch_list(&self, id: u64, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let groups = self.compare_groups(id, cx);
        let counts = self
            .state
            .read(cx)
            .repo_states
            .get(&id)
            .map(|r| r.compare.branch_counts.clone())
            .unwrap_or_default();
        if groups.is_empty() {
            return div()
                .flex_1()
                .pt(SPACING_DOUBLE)
                .flex()
                .justify_center()
                .text_size(FONT_SIZE)
                .text_color(t.text_secondary)
                .child("No branches to compare")
                .into_any_element();
        }
        let focused = self.focused_branch.clone();
        let weak = cx.weak_entity();
        let hover_bg = t.box_selected_active_background;
        let hover_text = t.box_selected_active_text;
        div()
            .id("compare-branch-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(groups.into_iter().map(|group| {
                let counts = counts.clone();
                let focused = focused.clone();
                let weak = weak.clone();
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(ROW_HEIGHT)
                            .pt(SPACING)
                            .px(SPACING)
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(FONT_SIZE)
                            .child(group.title),
                    )
                    .children(group.branches.into_iter().map(move |b| {
                        let is_focused = focused.as_deref() == Some(b.name.as_str());
                        let name = b.name.clone();
                        let weak = weak.clone();
                        let ab = counts.get(&b.name).cloned();
                        div()
                            .id(SharedString::from(format!(
                                "compare-branch-{}",
                                b.full_name
                            )))
                            .h(ROW_HEIGHT)
                            .w_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(SPACING)
                            .cursor_pointer()
                            .when(is_focused, |d| {
                                d.bg(t.box_selected_active_background)
                                    .text_color(t.box_selected_active_text)
                            })
                            .when(!is_focused, move |d| {
                                d.hover(move |s| s.bg(hover_bg).text_color(hover_text))
                            })
                            .on_click(move |_, window, cx| {
                                let name = name.clone();
                                weak.update(cx, |this, cx| {
                                    this.choose_compare_branch(id, name, window, cx)
                                })
                                .ok();
                            })
                            .child(octicon(Octicon::GitBranch, t.text).mr(SPACING_HALF))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(FONT_SIZE)
                                    .child(b.name.clone()),
                            )
                            .when_some(ab, |d, ab| {
                                // `.branch-commit-counter`: behind ↓, ahead ↑
                                let counter = |n: u32, icon: Octicon| {
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .child(n.to_string())
                                        .child(octicon(icon, t.text_secondary).size(px(10.)))
                                };
                                d.child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(SPACING_HALF)
                                        .text_size(FONT_SIZE_SM)
                                        .text_color(t.text_secondary)
                                        .child(counter(ab.behind, Octicon::ArrowDown))
                                        .child(counter(ab.ahead, Octicon::ArrowUp)),
                                )
                            })
                    }))
            }))
            .into_any_element()
    }

    /// Behind (N) | Ahead (M) tabs (`#compare-view .tab-bar`).
    fn compare_tabs(
        &self,
        id: u64,
        mode: ComparisonMode,
        ahead: u32,
        behind: u32,
        cx: &Context<Self>,
    ) -> Div {
        let t = cx.ghd();
        let tab = |label: String, selected: bool, first: bool, target: ComparisonMode| {
            let hover_bg = t.tab_bar_hover_background;
            div()
                .id(if first {
                    "compare-tab-behind"
                } else {
                    "compare-tab-ahead"
                })
                .flex_1()
                .h(px(25.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(FONT_SIZE)
                .border_1()
                .border_color(if selected {
                    t.box_border_accent
                } else {
                    t.box_border
                })
                .when(first, |d| d.rounded_l(BORDER_RADIUS))
                .when(!first, |d| d.rounded_r(BORDER_RADIUS).ml(px(-1.)))
                .bg(if selected {
                    t.box_selected_active_background
                } else {
                    t.tab_bar_background
                })
                .text_color(if selected {
                    t.box_selected_active_text
                } else {
                    t.text
                })
                .cursor_pointer()
                .when(!selected, move |d| d.hover(move |s| s.bg(hover_bg)))
                .on_click(move |_, _, cx| Dispatcher::set_comparison_mode(id, target, cx))
                .child(label)
        };
        div()
            .flex_none()
            .flex()
            .flex_row()
            .p(SPACING_HALF)
            .border_b_1()
            .border_color(t.box_border)
            .child(tab(
                format!("Behind ({behind})"),
                mode == ComparisonMode::Behind,
                true,
                ComparisonMode::Behind,
            ))
            .child(tab(
                format!("Ahead ({ahead})"),
                mode == ComparisonMode::Ahead,
                false,
                ComparisonMode::Ahead,
            ))
    }

    /// `MergeCallToActionWithConflicts` (`.merge-cta`).
    fn merge_cta(
        &self,
        id: u64,
        branch: &str,
        behind: u32,
        merge_status: Option<Mergeability>,
        cx: &Context<Self>,
    ) -> Div {
        let t = cx.ghd();
        let current = self
            .state
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|r| r.info.as_ref())
            .and_then(|i| i.current_branch())
            .map(|b| b.name.clone())
            .unwrap_or_default();
        let op = self.merge_option;
        let bold = |text: String| div().font_weight(FontWeight::SEMIBOLD).child(text);
        let row = || div().flex().flex_row().flex_wrap().items_center();
        let plural = if behind == 1 { "commit" } else { "commits" };
        let rebase_message = || {
            row()
                .child("This will update\u{a0}")
                .child(bold(current.clone()))
                .child("\u{a0}by applying its commits on top of\u{a0}")
                .child(bold(branch.to_string()))
                .into_any_element()
        };
        let (icon, color, message): (Octicon, Hsla, Option<AnyElement>) = if behind == 0 {
            (Octicon::Check, t.color_new, None)
        } else if op == MultiCommitOperationKind::Rebase {
            (Octicon::Check, t.color_new, Some(rebase_message()))
        } else {
            match merge_status {
                None => (
                    Octicon::DotFill,
                    t.color_modified,
                    Some(
                        div()
                            .child(format!(
                                "Checking for ability to {} automatically…",
                                op.lower()
                            ))
                            .into_any_element(),
                    ),
                ),
                Some(Mergeability::Invalid) => (
                    Octicon::X,
                    t.color_deleted,
                    Some(
                        div()
                            .child("Unable to merge unrelated histories in this repository")
                            .into_any_element(),
                    ),
                ),
                Some(Mergeability::Conflicts(n)) => (
                    Octicon::Alert,
                    t.color_modified,
                    Some(
                        row()
                            .child("There will be\u{a0}")
                            .child(bold(format!(
                                "{n} conflicted {}",
                                if n == 1 { "file" } else { "files" }
                            )))
                            .child("\u{a0}when merging\u{a0}")
                            .child(bold(branch.to_string()))
                            .child("\u{a0}into\u{a0}")
                            .child(bold(current.clone()))
                            .into_any_element(),
                    ),
                ),
                Some(Mergeability::Clean) => (
                    Octicon::Check,
                    t.color_new,
                    Some(
                        row()
                            .child("This will merge\u{a0}")
                            .child(bold(format!("{behind} {plural}")))
                            .child("\u{a0}from\u{a0}")
                            .child(bold(branch.to_string()))
                            .child("\u{a0}into\u{a0}")
                            .child(bold(current.clone()))
                            .into_any_element(),
                    ),
                ),
            }
        };
        let disabled = behind == 0 || merge_status == Some(Mergeability::Invalid);
        let label = match op {
            MultiCommitOperationKind::Squash => "Squash and merge",
            MultiCommitOperationKind::Rebase => "Rebase",
            _ => "Create a merge commit",
        };
        let weak = cx.weak_entity();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .p(SPACING)
            .border_t_1()
            .border_color(t.box_border)
            .text_size(FONT_SIZE_SM)
            .when_some(message, |d, message| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .mb(SPACING)
                        .child(
                            div()
                                .relative()
                                .w_full()
                                .h(px(20.))
                                .flex()
                                .justify_center()
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .right_0()
                                        .top(px(10.))
                                        .h(px(1.))
                                        .bg(t.box_border),
                                )
                                .child(
                                    div()
                                        .px(SPACING_HALF)
                                        .bg(t.background)
                                        .child(octicon(icon, color)),
                                ),
                        )
                        .child(
                            div()
                                .mt(SPACING_HALF)
                                .text_center()
                                .text_color(t.text_secondary)
                                .child(message),
                        ),
                )
            })
            .child(
                // `DropdownSelectButton`
                div()
                    .flex()
                    .flex_row()
                    .child(
                        primary_button("compare-merge", label, disabled, cx)
                            .flex_1()
                            .rounded_tr(px(0.))
                            .rounded_br(px(0.))
                            .on_click(move |_, _, cx| {
                                if !disabled {
                                    Dispatcher::compare_merge_action(id, op, cx);
                                }
                            }),
                    )
                    .child(
                        primary_button("compare-merge-options", "", disabled, cx)
                            .px(SPACING_HALF)
                            .rounded_tl(px(0.))
                            .rounded_bl(px(0.))
                            .ml(px(1.))
                            .child(octicon(Octicon::TriangleDown, white()))
                            .on_click(move |ev: &ClickEvent, window, cx| {
                                if disabled {
                                    return;
                                }
                                let position = ev.mouse_position().unwrap_or_default();
                                let options = [
                                    (MultiCommitOperationKind::Merge, "Create a merge commit"),
                                    (MultiCommitOperationKind::Squash, "Squash and merge"),
                                    (MultiCommitOperationKind::Rebase, "Rebase"),
                                ];
                                let items: Vec<MenuItem> = options
                                    .iter()
                                    .map(|(kind, label)| {
                                        let kind = *kind;
                                        let weak = weak.clone();
                                        MenuItem::checkbox(*label, kind == op, move |_, cx| {
                                            weak.update(cx, |this, cx| {
                                                this.merge_option = kind;
                                                cx.notify();
                                            })
                                            .ok();
                                        })
                                    })
                                    .collect();
                                #[cfg(target_os = "macos")]
                                crate::native_menu::show_context_menu(items, position, window, cx);
                                #[cfg(not(target_os = "macos"))]
                                let _ = (items, position, window, cx);
                            }),
                    ),
            )
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
        let comparing = self
            .state
            .read(cx)
            .repo_states
            .get(&id)
            .is_some_and(|r| r.compare.is_comparing());
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
            .enabled(!busy && !comparing),
            MenuItem::new(format!("Reorder {count} Commits…"), move |_, cx| {
                weak.update(cx, |this, cx| {
                    this.start_keyboard_reorder(id, s3.clone(), cx)
                })
                .ok();
            })
            .enabled(!busy && !comparing),
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
            let comparing = rs.is_some_and(|r| r.compare.is_comparing());
            let is_head = !comparing
                && rs
                    .and_then(|rs| rs.commits.first())
                    .map(|c| c.sha == commit.sha)
                    .unwrap_or(false);
            (
                html_url,
                is_head,
                rs.is_some_and(|r| r.mco.is_some()) || comparing,
            )
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
        let comparing = rs.is_some_and(|r| r.compare.is_comparing());
        let commits: Rc<Vec<Commit>> =
            Rc::new(rs.map(|r| r.visible_commits().clone()).unwrap_or_default());
        let selected: Rc<Vec<String>> =
            Rc::new(rs.map(|r| r.selected_commits.clone()).unwrap_or_default());
        let highlighted: Rc<Vec<String>> =
            Rc::new(rs.map(|r| r.highlighted_shas.clone()).unwrap_or_default());
        let exhausted = comparing || rs.map(|r| r.commits_exhausted).unwrap_or(true);
        let loaded = rs.map(|r| r.info.is_some()).unwrap_or(false);
        let draggable = rs.is_some_and(|r| r.mco.is_none()) && self.reorder.is_none() && !comparing;
        if commits.is_empty() {
            let compare_loading = rs.is_some_and(|r| r.compare.loading);
            let message: String = match rs.map(|r| &r.compare.form) {
                Some(corvane_core::CompareForm::Branch { branch, mode, .. })
                    if !compare_loading =>
                {
                    match mode {
                        ComparisonMode::Ahead => {
                            format!("The compared branch ({branch}) is up to date with your branch")
                        }
                        ComparisonMode::Behind => {
                            format!("Your branch is up to date with the compared branch ({branch})")
                        }
                    }
                }
                _ if loaded && exhausted && !compare_loading => "No history".to_string(),
                _ => String::new(),
            };
            return div()
                .flex_1()
                .flex()
                .items_start()
                .justify_center()
                .p(SPACING_DOUBLE)
                .text_center()
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
                            let dimmed =
                                !highlighted.is_empty() && !highlighted.contains(&commit.sha);
                            Dispatcher::request_avatar_for_email(&commit.author.email, cx);
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
                                dimmed,
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
        Dispatcher::set_drag_target(None, cx);
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
pub(crate) fn commit_row_contents(commit: &Commit, text: Hsla, secondary: Hsla, cx: &App) -> Div {
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
                        .child(avatar_image(
                            avatar_lookup(&commit.author.email, cx),
                            px(16.),
                            cx,
                        ))
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
    dimmed: bool,
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
        // `.has-highlighted-commits .list-item:not(.highlighted) { opacity: 30% }`
        .when(dimmed, |d| d.opacity(0.3))
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
                    cx.new(|cx| CommitDragElement::new(drag, cx))
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
                let target = match hint {
                    Some(DropHint::Squash(_)) => Some(DropTarget::Commit),
                    Some(DropHint::InsertAt(_)) => Some(DropTarget::InsertionPoint {
                        count: ev.drag(cx).shas.len(),
                    }),
                    None => None,
                };
                Dispatcher::set_drag_target(target, cx);
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
        // drop hints only live while a drag is in flight
        if !cx.has_active_drag() {
            if self.drop_hint.is_some() {
                self.drop_hint = None;
            }
            if self.state.read(cx).drag_target.is_some() {
                Dispatcher::set_drag_target(None, cx);
            }
        }
        let (id, show_list, form, merge_status) = {
            let s = self.state.read(cx);
            let id = s.selected;
            let rs = id.and_then(|id| s.repo_states.get(&id));
            (
                id,
                rs.is_some_and(|r| r.compare.show_branch_list),
                rs.map(|r| r.compare.form.clone()),
                rs.and_then(|r| r.compare.merge_status),
            )
        };
        // `onTextBoxFocused` → show the branch list
        let focused = self.compare.read(cx).focus_handle(cx).is_focused(window);
        if focused
            && !self.compare_was_focused
            && let Some(id) = id
        {
            Dispatcher::set_compare_branch_list_visible(id, true, cx);
        }
        self.compare_was_focused = focused;
        let body: AnyElement = match (id, show_list, form) {
            (Some(id), true, _) => self.compare_branch_list(id, cx),
            (
                Some(id),
                false,
                Some(corvane_core::CompareForm::Branch {
                    branch,
                    mode,
                    ahead_behind,
                }),
            ) => div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(self.compare_tabs(id, mode, ahead_behind.ahead, ahead_behind.behind, cx))
                .child(self.commit_list(cx))
                .when(mode == ComparisonMode::Behind, |d| {
                    d.child(self.merge_cta(id, &branch, ahead_behind.behind, merge_status, cx))
                })
                .into_any_element(),
            _ => self.commit_list(cx).into_any_element(),
        };
        let t = cx.ghd();
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            // `onBranchFilterBlur`: clicking anywhere else closes the branch list.
            .when(show_list, |d| {
                d.on_mouse_down_out(
                    cx.listener(|this, _, window, cx| this.compare_blur(window, cx)),
                )
            })
            .child(
                // `#compare-view .compare-form`
                div()
                    .id("compare-form")
                    .key_context("CompareFilter")
                    .on_action(cx.listener(|this, _: &CompareSelect, window, cx| {
                        this.compare_select(window, cx)
                    }))
                    .on_action(cx.listener(|this, _: &CompareClear, window, cx| {
                        this.compare_clear(window, cx)
                    }))
                    .on_action(cx.listener(|this, _: &SelectNextFile, _, cx| {
                        this.move_focused_branch(1, cx)
                    }))
                    .on_action(cx.listener(|this, _: &SelectPreviousFile, _, cx| {
                        this.move_focused_branch(-1, cx)
                    }))
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
            .child(body)
            .children(self.context_menu.clone())
    }
}
