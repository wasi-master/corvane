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
//!
//! Deviations (`.docs/deviations.md` › History): the commit menus
//! add Copy Commit Title / Message / URL and Copy SHAs (flag `809`); the
//! list scrolls back to the top when the branch changes (flag `808`); Revert
//! Changes in Commit(s) Without Committing (flag `815`); Push Up to This
//! Commit (flag `816`); a commit with a description gets a mark after its
//! summary (flag `803`); compact rows drop the author line (flag `802`); the
//! tag pill's tooltip lists every tag (flag `806`); Checkout Commit works on
//! the branch tip (flag `817`); a toggle before the compare box lists first
//! parents only (flag `807`); the compare list offers matching tags (flag
//! `825`); pushed tags can be deleted after a confirmation (flag `826`);
//! Cherry-pick Without Committing (flag `820`); Create Patch File(s) (flag
//! `821`).

use std::rc::Rc;

use corvane_core::{
    AppState, Commit, ComparisonMode, Dispatcher, DropTarget, Mergeability,
    MultiCommitOperationKind, Popup,
};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{
    CompareClear, CompareSelect, ExtendSelectionDown, ExtendSelectionUp, ReorderCancel,
    ReorderConfirm, ReorderMoveDown, ReorderMoveUp, SelectFirstFile, SelectLastFile,
    SelectNextFile, SelectPreviousFile,
};
use crate::branch_list::group_branches;
use crate::context_menu::{ContextMenu, MenuItem};
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative;
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{GhdTooltip, IconButtonA11y, ListRowA11y};
use crate::widgets::{avatar_image, avatar_lookup, kbd, primary_button};

/// `RowHeight` in `commit-list.tsx`
#[allow(non_snake_case)]
pub fn COMMIT_ROW_HEIGHT() -> Pixels {
    zpx(50.)
}

/// `802`: summary-only commit rows, 30 px tall.
fn compact_rows(cx: &App) -> bool {
    corvane_core::AppState::try_global(cx).is_some_and(|s| {
        s.read(cx)
            .flags
            .bool(corvane_core::flags::ids::COMPACT_COMMIT_ROWS)
    })
}

/// The commit row height: [`COMMIT_ROW_HEIGHT`], or 30 px for compact rows.
pub fn commit_row_height(cx: &App) -> Pixels {
    if compact_rows(cx) {
        zpx(30.)
    } else {
        COMMIT_ROW_HEIGHT()
    }
}

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
                        .ml(SPACING_THIRD())
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
                .bottom(zpx(-25.))
                .px(SPACING_THIRD())
                .py(zpx(1.))
                .rounded(zpx(1.))
                .bg(t.tooltip_background)
                .text_color(t.tooltip_text)
                .text_size(FONT_SIZE_SM())
                .whitespace_nowrap()
                .shadow(vec![BoxShadow {
                    color: t.shadow,
                    offset: point(zpx(0.), zpx(1.)),
                    blur_radius: css_blur(3.),
                    spread_radius: zpx(0.),
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
            .w(zpx(300.))
            .h(commit_row_height(cx))
            .mt(zpx(22.))
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
                        offset: point(zpx(2.), zpx(1.)),
                        blur_radius: css_blur(1.),
                        spread_radius: zpx(0.),
                        inset: false,
                    }])
                    .overflow_hidden()
                    .child(commit_row_contents(
                        &self.drag.commit,
                        t.text,
                        t.text_secondary,
                        None,
                        cx,
                    )),
            )
            .when(count > 1, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(zpx(-22.))
                        .left(zpx(20.))
                        .size(zpx(18.))
                        .rounded_full()
                        .bg(rgb(0xd73a49))
                        .text_color(white())
                        .text_size(FONT_SIZE_SM())
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
    list_scroll: UniformListScrollHandle,
    /// Repository and tip (branch name or detached sha) the list last showed;
    /// a change scrolls it back to the top (flag `808`).
    shown_tip: Option<(u64, String)>,
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
            list_scroll: UniformListScrollHandle::new(),
            shown_tip: None,
        }
    }

    /// Corvane (`603-focus-list-on-section-switch`).
    pub fn list_focus_handle(&self) -> FocusHandle {
        self.list_focus.clone()
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
        let mut groups = group_branches(
            &branches,
            default,
            &recent,
            &query,
            crate::branch_list::sort_by_date(cx),
        );
        // `825`: tags matching the filter, after the branches
        if !query.is_empty() && s.flags.bool(corvane_core::flags::ids::COMPARE_TAGS) {
            let tags: Vec<corvane_core::Branch> = rs
                .compare
                .tags
                .iter()
                .filter(|t| corvane_core::filter::fuzzy_score(&query, t).is_some())
                .map(|t| corvane_core::Branch {
                    name: t.clone(),
                    kind: corvane_core::BranchKind::Local,
                    full_name: format!("refs/tags/{t}"),
                    tip: None,
                    upstream: None,
                    tip_time: None,
                    remote_name: None,
                })
                .collect();
            if !tags.is_empty() {
                groups.push(crate::branch_list::BranchGroup {
                    title: "Tags",
                    branches: tags,
                });
            }
        }
        groups
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
                .pt(SPACING_DOUBLE())
                .flex()
                .justify_center()
                .text_size(FONT_SIZE())
                .text_color(t.text_secondary)
                .child("No branches to compare")
                .into_any_element();
        }
        let focused = self.focused_branch.clone();
        let weak = cx.weak_entity();
        // `.list-item:hover`: `--list-item-hover-background-color`, text unchanged
        let list_hover = t.list_item_hover_background;
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
                            .h(ROW_HEIGHT())
                            .pt(SPACING())
                            .px(SPACING())
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(FONT_SIZE())
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
                            .h(ROW_HEIGHT())
                            .w_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(SPACING())
                            .cursor_pointer()
                            .when(is_focused, |d| {
                                d.bg(t.box_selected_active_background)
                                    .text_color(t.box_selected_active_text)
                            })
                            .when(!is_focused, move |d| d.hover(move |s| s.bg(list_hover)))
                            .on_click(move |_, window, cx| {
                                let name = name.clone();
                                weak.update(cx, |this, cx| {
                                    this.choose_compare_branch(id, name, window, cx)
                                })
                                .ok();
                            })
                            .child(
                                octicon(
                                    if b.full_name.starts_with("refs/tags/") {
                                        Octicon::Tag
                                    } else {
                                        Octicon::GitBranch
                                    },
                                    t.text,
                                )
                                .mr(SPACING_HALF()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(FONT_SIZE())
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
                                        .child(octicon(icon, t.text_secondary).size(zpx(10.)))
                                };
                                d.child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(SPACING_HALF())
                                        .text_size(FONT_SIZE_SM())
                                        .text_color(t.text_secondary)
                                        .child(counter(ab.behind, Octicon::ArrowDown))
                                        .child(counter(ab.ahead, Octicon::ArrowUp)),
                                )
                            })
                    }))
            }))
            .with_scrollbar()
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
                .h(zpx(25.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(FONT_SIZE())
                .border_1()
                .border_color(if selected {
                    t.box_border_accent
                } else {
                    t.box_border
                })
                .when(first, |d| d.rounded_l(BORDER_RADIUS()))
                .when(!first, |d| d.rounded_r(BORDER_RADIUS()).ml(zpx(-1.)))
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
            .p(SPACING_HALF())
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
        // `838-no-merge-while-conflicted`: no second merge while conflicted
        let disabled = behind == 0
            || merge_status == Some(Mergeability::Invalid)
            || Dispatcher::merge_blocked_by_conflicts(id, cx);
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
            .p(SPACING())
            .border_t_1()
            .border_color(t.box_border)
            .text_size(FONT_SIZE_SM())
            .when_some(message, |d, message| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .mb(SPACING())
                        .child(
                            div()
                                .relative()
                                .w_full()
                                .h(zpx(20.))
                                .flex()
                                .justify_center()
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .right_0()
                                        .top(zpx(10.))
                                        .h(zpx(1.))
                                        .bg(t.box_border),
                                )
                                .child(
                                    div()
                                        .px(SPACING_HALF())
                                        .bg(t.background)
                                        .child(octicon(icon, color)),
                                ),
                        )
                        .child(
                            div()
                                .mt(SPACING_HALF())
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
                            .rounded_tr(zpx(0.))
                            .rounded_br(zpx(0.))
                            .on_click(move |_, _, cx| {
                                if !disabled {
                                    Dispatcher::compare_merge_action(id, op, cx);
                                }
                            }),
                    )
                    .child(
                        primary_button("compare-merge-options", "", disabled, cx)
                            .px(SPACING_HALF())
                            .rounded_tl(zpx(0.))
                            .rounded_bl(zpx(0.))
                            .ml(zpx(1.))
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
                                crate::native_menu::show_context_menu(items, position, window, cx);
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
        // GHD's Electron `Menu.popup`: an NSMenu on macOS, a views menu on Linux
        crate::native_menu::show_context_menu(items, position, window, cx);
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
        let (copy_items, revert_no_commit, pick_no_commit, patches) = {
            let flags = &self.state.read(cx).flags;
            (
                flags.bool(corvane_core::flags::ids::HISTORY_COPY_ITEMS),
                flags.bool(corvane_core::flags::ids::REVERT_WITHOUT_COMMITTING),
                flags.bool(corvane_core::flags::ids::CHERRY_PICK_WITHOUT_COMMITTING),
                flags.bool(corvane_core::flags::ids::CREATE_PATCH_FILES),
            )
        };
        // `809`: newest first, whatever the click order
        let shas_text = {
            let s = self.state.read(cx);
            let mut shas = selection.clone();
            if let Some(rs) = s.repo_states.get(&id) {
                shas.sort_by_key(|sha| {
                    rs.commits
                        .iter()
                        .position(|c| &c.sha == sha)
                        .unwrap_or(usize::MAX)
                });
            }
            shas.join("\n")
        };
        let weak = cx.weak_entity();
        let (s1, s2, s3, s4) = (
            selection.clone(),
            selection.clone(),
            selection.clone(),
            selection.clone(),
        );
        let (s5, s6) = (selection.clone(), selection);
        let onto = commit.sha.clone();
        let mut items = vec![
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
        if revert_no_commit {
            // `815`: newest first, staged, not committed
            items.push(
                MenuItem::new(
                    format!("Revert Changes in {count} Commits Without Committing"),
                    move |_, cx| Dispatcher::revert_commits_without_committing(id, s4.clone(), cx),
                )
                .enabled(!busy && !comparing),
            );
        }
        if pick_no_commit {
            // `820`: onto the current branch, staged, not committed
            items.push(
                MenuItem::new(
                    format!("Cherry-pick {count} Commits Without Committing"),
                    move |_, cx| Dispatcher::cherry_pick_without_committing(id, s5.clone(), cx),
                )
                .enabled(!busy),
            );
        }
        if patches {
            // `821`
            items.push(MenuItem::new(
                format!("Create {count} Patch Files…"),
                move |_, cx| create_patch_files(id, s6.clone(), cx),
            ));
        }
        if copy_items {
            items.push(MenuItem::separator());
            items.push(MenuItem::new("Copy SHAs", move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(shas_text.clone()))
            }));
        }
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
        let (html_url, is_head, busy, copy_items, revert_no_commit, unpushed, checkout_head) = {
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
                s.flags.bool(corvane_core::flags::ids::HISTORY_COPY_ITEMS),
                s.flags
                    .bool(corvane_core::flags::ids::REVERT_WITHOUT_COMMITTING),
                // `816`: one of the current branch's commits its upstream lacks
                s.flags
                    .bool(corvane_core::flags::ids::PUSH_UP_TO_COMMIT)
                    .then(|| {
                        let tracked = rs
                            .and_then(|r| r.info.as_ref())
                            .and_then(|i| i.current_branch())
                            .is_some_and(|b| b.upstream.is_some());
                        let ahead = rs
                            .filter(|_| tracked)
                            .and_then(|r| r.ahead_behind)
                            .map_or(0, |ab| ab.ahead as usize);
                        !comparing
                            && rs
                                .and_then(|r| r.commits.iter().position(|c| c.sha == commit.sha))
                                .is_some_and(|ix| ix < ahead)
                    }),
                // `817`: the branch tip can be checked out (detaching HEAD)
                s.flags.bool(corvane_core::flags::ids::CHECKOUT_HEAD_COMMIT)
                    && rs
                        .and_then(|r| r.info.as_ref())
                        .and_then(|i| i.current_branch())
                        .is_some(),
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
            .enabled(!is_head || checkout_head),
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
        ]);
        if revert_no_commit {
            items.push(MenuItem::new(
                "Revert Changes in Commit Without Committing",
                {
                    let sha = sha.clone();
                    move |_, cx| {
                        Dispatcher::revert_commits_without_committing(id, vec![sha.clone()], cx)
                    }
                },
            ));
        }
        items.extend([
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
            // GHD `getDeleteTagsMenuItem`: only tags still in `tagsToPush`
            // (created here, not pushed) can be deleted
            let (unpushed, delete_pushed) = {
                let s = self.state.read(cx);
                (
                    s.repository(id)
                        .map(|r| r.tags_to_push.clone())
                        .unwrap_or_default(),
                    s.flags.bool(corvane_core::flags::ids::DELETE_PUSHED_TAGS),
                )
            };
            // `826`: the others after a confirmation that can include the remote
            let delete = move |tag: &String| {
                let is_unpushed = unpushed.contains(tag);
                let tag = tag.clone();
                (
                    is_unpushed || delete_pushed,
                    move |_: &mut Window, cx: &mut App| {
                        if is_unpushed {
                            Dispatcher::delete_tag(id, tag.clone(), cx)
                        } else {
                            Dispatcher::show_popup(
                                Popup::ConfirmDeletePushedTag {
                                    repo: id,
                                    tag: tag.clone(),
                                },
                                cx,
                            )
                        }
                    },
                )
            };
            if commit.tags.len() == 1 {
                let tag = commit.tags[0].clone();
                let (enabled, action) = delete(&tag);
                items.push(MenuItem::new(format!("Delete tag {tag}"), action).enabled(enabled));
            } else {
                let entries = commit
                    .tags
                    .iter()
                    .map(|tag| {
                        let (enabled, action) = delete(tag);
                        MenuItem::new(tag.clone(), action).enabled(enabled)
                    })
                    .collect();
                items.push(MenuItem::submenu("Delete tag…", entries));
            }
        }
        items.push(
            MenuItem::new("Cherry-pick Commit…", {
                let sha = sha.clone();
                move |_, cx| Dispatcher::start_cherry_pick_flow(id, vec![sha.clone()], cx)
            })
            .enabled(!busy),
        );
        let (pick_no_commit, patches) = {
            let flags = &self.state.read(cx).flags;
            (
                flags.bool(corvane_core::flags::ids::CHERRY_PICK_WITHOUT_COMMITTING),
                flags.bool(corvane_core::flags::ids::CREATE_PATCH_FILES),
            )
        };
        if pick_no_commit {
            // `820`: onto the current branch (not the HEAD commit itself)
            items.push(
                MenuItem::new("Cherry-pick Commit Without Committing", {
                    let sha = sha.clone();
                    move |_, cx| {
                        Dispatcher::cherry_pick_without_committing(id, vec![sha.clone()], cx)
                    }
                })
                .enabled(!busy && !is_head),
            );
        }
        if patches {
            // `821`
            items.push(MenuItem::new("Create Patch File…", {
                let sha = sha.clone();
                move |_, cx| create_patch_files(id, vec![sha.clone()], cx)
            }));
        }
        if let Some(unpushed) = unpushed {
            items.push(
                MenuItem::new("Push Up to This Commit", {
                    let sha = sha.clone();
                    move |_, cx| Dispatcher::push_up_to(id, sha.clone(), cx)
                })
                .enabled(unpushed && !busy),
            );
        }
        items.extend([
            MenuItem::separator(),
            MenuItem::new("Copy SHA", {
                let sha = sha.clone();
                move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(sha.clone()))
            }),
        ]);
        let commit_url = html_url.clone().map(|u| format!("{u}/commit/{sha}"));
        if copy_items {
            // `809`: the title, the full message and the GitHub URL
            let title = commit.summary.clone();
            let message = if commit.body.is_empty() {
                commit.summary.clone()
            } else {
                format!("{}\n\n{}", commit.summary, commit.body)
            };
            items.push(MenuItem::new("Copy Commit Title", move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(title.clone()))
            }));
            items.push(MenuItem::new("Copy Commit Message", move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(message.clone()))
            }));
            items.push(
                MenuItem::new("Copy Commit URL", {
                    let url = commit_url.clone();
                    move |_, cx| {
                        if let Some(url) = &url {
                            cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
                        }
                    }
                })
                .enabled(commit_url.is_some()),
            );
        }
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
                let url = commit_url;
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
            .top(SPACING_HALF())
            .left(SPACING_HALF())
            .right(SPACING_HALF())
            .p(SPACING())
            .rounded(BORDER_RADIUS())
            .bg(t.background)
            .border_1()
            .border_color(t.box_border)
            .shadow(vec![BoxShadow {
                color: t.shadow,
                offset: point(zpx(0.), zpx(2.)),
                blur_radius: css_blur(7.),
                spread_radius: zpx(0.),
                inset: false,
            }])
            .text_size(FONT_SIZE())
            .flex()
            .flex_col()
            .gap(SPACING_HALF())
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
                    .gap(zpx(3.))
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
                    .gap(zpx(3.))
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
                .p(SPACING_DOUBLE())
                .text_center()
                .text_color(t.text_secondary)
                .child(message)
                .into_any_element();
        }
        // `808`: another branch (or repository) starts at the newest commit
        let tip = rs
            .and_then(|r| r.info.as_ref())
            .map(|info| match &info.tip {
                corvane_core::Tip::Detached { sha } => sha.clone(),
                tip => tip.branch_name().unwrap_or_default().to_string(),
            });
        let scroll_to_top = s
            .flags
            .bool(corvane_core::flags::ids::HISTORY_SCROLLS_TO_TOP_ON_BRANCH_CHANGE);
        if let Some(tip) = tip {
            let key = Some((id, tip));
            if self.shown_tip != key {
                if scroll_to_top && self.shown_tip.is_some() {
                    self.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                }
                self.shown_tip = key;
            }
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
            // GHD `ariaLabel="Commits"` on the list
            .role(Role::List)
            .aria_label("Commits")
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
            // ⌘↑ / ⌘↓ (GHD isHomeKey / isEndKey), ⇧↑ / ⇧↓ (`addSelection`)
            .on_action(cx.listener(move |this, _: &SelectFirstFile, _, cx| {
                this.step_selection(id, -(1 << 30), cx)
            }))
            .on_action(cx.listener(move |this, _: &SelectLastFile, _, cx| {
                this.step_selection(id, 1 << 30, cx)
            }))
            .on_action(cx.listener(move |this, _: &ExtendSelectionDown, _, cx| {
                this.extend_selection(id, 1, cx)
            }))
            .on_action(cx.listener(move |this, _: &ExtendSelectionUp, _, cx| {
                this.extend_selection(id, -1, cx)
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
                .min_h_0()
                .with_scrollbar_handle(&self.list_scroll),
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

    /// ⇧↑ / ⇧↓: extend the multi-selection from its moving end (GHD
    /// `List.addSelection`).
    fn extend_selection(&self, id: u64, delta: isize, cx: &mut App) {
        let next = {
            let s = self.state.read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let end = rs
                .selected_commits
                .last()
                .or(rs.selected_commit.as_ref())
                .and_then(|sha| rs.commits.iter().position(|c| &c.sha == sha));
            let Some(end) = end else { return };
            let ix = (end as isize + delta).clamp(0, rs.commits.len() as isize - 1) as usize;
            rs.commits.get(ix).map(|c| c.sha.clone())
        };
        if let Some(sha) = next {
            Dispatcher::extend_commit_selection(id, sha, cx);
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

/// `.commit .info` + tag indicators, shared with the drag element. `badge`
/// overrides the tag pill's (background, text) colours on selected rows.
pub(crate) fn commit_row_contents(
    commit: &Commit,
    text: Hsla,
    secondary: Hsla,
    badge: Option<(Hsla, Hsla)>,
    cx: &App,
) -> Div {
    let t = cx.ghd();
    let (badge_bg, badge_text) =
        badge.unwrap_or((t.list_item_badge_background, t.list_item_badge_text));
    let summary = if commit.summary.is_empty() {
        "Empty commit message".to_string()
    } else {
        // GHD `RichText`: emoji shortcodes as emoji
        corvane_core::text_tokens::with_emoji(&commit.summary)
    };
    let empty = commit.summary.is_empty();
    // `803`: a mark after the summary when the commit has a description
    let body_mark = !commit.body.trim().is_empty()
        && corvane_core::AppState::try_global(cx).is_some_and(|s| {
            s.read(cx)
                .flags
                .bool(corvane_core::flags::ids::COMMIT_BODY_INDICATOR)
        });
    let compact = compact_rows(cx);
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
        .pl(SPACING())
        .pr(SPACING() + SPACING_HALF())
        .text_color(text)
        .child(
            // `.info { margin-top: -4px }`: 18 px summary, 3 px gap, the
            // 16.5 px byline in 11 px text after the 20 px avatar stack
            div()
                .flex_1()
                .min_w(zpx(50.))
                .when(!compact, |d| d.mt(zpx(-4.)))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(
                            div()
                                .min_w_0()
                                .text_size(FONT_SIZE())
                                .line_height(zpx(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .when(empty, |d| d.text_color(secondary))
                                .child(summary),
                        )
                        .when(body_mark, |d| {
                            d.child(
                                div()
                                    .flex_none()
                                    .ml(SPACING_HALF())
                                    .child(octicon(Octicon::KebabHorizontal, secondary)),
                            )
                        }),
                )
                .when(!compact, |d| {
                    d.child(
                        div()
                            .mt(zpx(3.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(zpx(4.))
                            .child(avatar_image(
                                avatar_lookup(&commit.author.email, cx),
                                zpx(16.),
                                cx,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(FONT_SIZE_SM())
                                    .line_height(zpx(16.5))
                                    .text_color(secondary)
                                    .child(byline),
                            ),
                    )
                }),
        )
        // `.commit-indicators .tag-indicator`: the first tag as a 16 px pill
        // (5 px padding, 6 px radius, no icon); more tags peek out behind it
        // as a 10 px tab (`.tag-indicator-more`)
        .when(!commit.tags.is_empty(), |d| {
            let pill = div()
                .ml(SPACING())
                .h(zpx(16.))
                .max_w(gpui_kit::relative(0.5))
                .flex()
                .flex_row()
                .text_color(badge_text)
                .text_size(FONT_SIZE())
                .line_height(zpx(16.))
                .child(
                    div()
                        .min_w_0()
                        .px(SPACING_HALF())
                        .h(zpx(16.))
                        .rounded(BORDER_RADIUS())
                        .bg(badge_bg)
                        .truncate()
                        .child(commit.tags[0].clone()),
                )
                .when(commit.tags.len() > 1, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .w(SPACING())
                            .ml(zpx(-5.))
                            .h(zpx(16.))
                            .rounded_r(BORDER_RADIUS())
                            .bg(badge_bg),
                    )
                });
            // `806`: hovering the pill lists every tag
            if tags_tooltip(cx) {
                d.child(
                    pill.id(SharedString::from(format!("tags-{}", commit.sha)))
                        .ghd_tooltip(commit.tags.join("\n")),
                )
            } else {
                d.child(pill)
            }
        })
}

/// `806`: the commit's tags as a tooltip on the tag pill and the details' tag list.
pub(crate) fn tags_tooltip(cx: &App) -> bool {
    corvane_core::AppState::try_global(cx).is_some_and(|s| {
        s.read(cx)
            .flags
            .bool(corvane_core::flags::ids::TAGS_TOOLTIP)
    })
}

/// Flag `807`: a 27 px toggle before the compare box that lists first
/// parents only; accent-coloured with a dot while on (the Changes filter
/// button's `.active` look), disabled while comparing.
fn first_parent_button(on: bool, comparing: bool, cx: &App) -> AnyElement {
    let t = cx.ghd();
    let label = if comparing {
        "First-parent history does not apply to a comparison"
    } else if on {
        "Showing first-parent commits only"
    } else {
        "Show first-parent commits only"
    };
    div()
        .id("history-first-parent")
        .icon_button_label(label)
        .ghd_tooltip(label)
        .relative()
        .size(zpx(27.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(t.secondary_button_border)
        .rounded(BORDER_RADIUS())
        .bg(t.secondary_button_background)
        .when(comparing, |d| d.opacity(0.6))
        .when(!comparing, |d| {
            d.cursor_pointer()
                .on_click(move |_, _, cx| Dispatcher::set_history_first_parent(!on, cx))
        })
        .child(octicon(
            Octicon::Filter,
            if on {
                t.box_selected_active_background
            } else {
                t.secondary_button_text
            },
        ))
        .when(on, |d| {
            d.child(
                div()
                    .absolute()
                    .top(zpx(3.))
                    .right(zpx(3.))
                    .size(zpx(5.))
                    .rounded_full()
                    .bg(t.box_selected_active_background),
            )
        })
        .into_any_element()
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
    // `#commit-list .list-item.selected .commit .tag-name`
    let badge = match (is_selected && !hint.keyboard_selected, list_focused) {
        (true, true) => Some((
            t.list_item_selected_active_badge_background,
            t.list_item_selected_active_badge_text,
        )),
        (true, false) => Some((
            t.list_item_selected_badge_background,
            t.list_item_selected_badge_text,
        )),
        _ => None,
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
        .a11y_row(
            format!(
                "{}, {}, {}",
                if commit.summary.is_empty() {
                    "Empty commit message"
                } else {
                    commit.summary.as_str()
                },
                commit.author.name,
                relative(commit.author.date())
            ),
            is_selected,
        )
        .relative()
        .w_full()
        .h(commit_row_height(cx))
        .flex_none()
        .bg(bg)
        // `.has-highlighted-commits .list-item:not(.highlighted) { opacity: 30% }`
        .when(dimmed, |d| d.opacity(0.3))
        .border_b_1()
        .border_color(t.box_border)
        .cursor_pointer()
        // no hover colour: `.commit` paints `--background-color` over the
        // list item's hover background in GHD
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
        // GHD `List.onRowMouseDown`: plain presses select at once (not on
        // release), unless the row is part of a multi-selection being dragged
        .on_mouse_down(MouseButton::Left, {
            let sha = sha.clone();
            let list_focus = list_focus.clone();
            let multi = selection.len() > 1 && selection.contains(&sha);
            move |ev: &MouseDownEvent, window, cx| {
                let m = ev.modifiers;
                if m.secondary() || m.shift || m.control || multi {
                    return;
                }
                window.focus(&list_focus, cx);
                if !is_selected {
                    Dispatcher::select_commit(id, sha.clone(), cx);
                }
            }
        })
        .on_mouse_down(MouseButton::Right, {
            let weak = weak.clone();
            let sha = sha.clone();
            move |ev: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                // a right-click selects the commit unless it is selected
                if !is_selected && !selection.contains(&sha) {
                    Dispatcher::select_commit(id, sha.clone(), cx);
                }
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
        .child(commit_row_contents(commit, text, secondary, badge, cx))
        .when(hint.line_above, |d| {
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(zpx(2.))
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
                    .h(zpx(2.))
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
        // `807`: the first-parent toggle before the compare box
        let (first_parent_toggle, comparing) = {
            let s = self.state.read(cx);
            (
                s.flags
                    .bool(corvane_core::flags::ids::HISTORY_FIRST_PARENT)
                    .then_some(s.settings.history_first_parent),
                s.selected
                    .and_then(|id| s.repo_states.get(&id))
                    .is_some_and(|rs| rs.compare.is_comparing()),
            )
        };
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
                    .p(SPACING_HALF())
                    .bg(t.box_alt_background)
                    .border_b_1()
                    .border_color(t.box_border)
                    .when_some(first_parent_toggle, |d, on| {
                        d.flex()
                            .flex_row()
                            .gap(SPACING_HALF())
                            .child(first_parent_button(on, comparing, cx))
                    })
                    .child({
                        // `FancyTextBox`: 27 px, `--box-border-color` frame,
                        // a 9 px branch glyph 7 px in, the text at 27 px
                        let focused = self.compare.read(cx).focus_handle(cx).is_focused(window);
                        div().flex_1().min_w_0().child(
                            crate::widgets::filter_text_box(
                                "compare-branch",
                                &self.compare,
                                Some(octicon(Octicon::GitBranch, t.text).size(zpx(9.))),
                                window,
                                cx,
                            )
                            .h(zpx(27.))
                            .pl(zpx(7.))
                            .gap(zpx(1.))
                            .when(!focused, |d| d.border_color(t.box_border)),
                        )
                    }),
            )
            .child(body)
            .children(self.context_menu.clone())
    }
}

/// Flag `821`: ask for a folder, then write the patches there.
fn create_patch_files(id: u64, shas: Vec<String>, cx: &mut App) {
    let receiver = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Save Patches".into()),
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        if let Ok(Ok(Some(paths))) = receiver.await
            && let Some(dir) = paths.into_iter().next()
        {
            cx.update(|cx| Dispatcher::create_patch_files(id, shas, dir, cx));
        }
    })
    .detach();
}
