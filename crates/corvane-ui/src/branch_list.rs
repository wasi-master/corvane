//! Branch foldout - GHD `ui/branches/{branches-container,branch-list,
//! branch-list-item,group-branches,no-branches}.tsx`
//! (`styles/ui/_branches.scss`, `_no-branches.scss`, `_filter-list.scss`):
//! `[🔍 Filter][New Branch]`, groups Default Branch / Recent Branches /
//! Other Branches, 29 px rows (check or branch icon, name, relative date) and
//! the "Choose a branch to merge into <current>" footer. GitHub repositories
//! get the Branches / Pull Requests tab bar (`branches-container.tsx`); the
//! pull request rows come from `pull_request_list.rs`.
//!
//! Hovering a pull request row for 250 ms shows its quick view
//! (`onMouseEnterPullRequestListItem`); leaving the row hides it after 500 ms
//! unless the pointer reaches the quick view, and leaving the quick view
//! hides it at once (`onMouseLeavePullRequestQuickView`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, UNIX_EPOCH};

use corvane_core::filter::fuzzy_score;
use corvane_core::{AppState, Branch, BranchKind, BranchesTab, Dispatcher, Popup, Tip};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::widgets::IconButtonA11y;

use crate::icons::{Octicon, octicon, spin};
use crate::pull_request_list::{
    QUICK_VIEW_MAX_HEIGHT, matches_filter, no_pull_requests, pull_request_row, quick_view,
    quick_view_top,
};
use crate::relative_time::relative;
use crate::scrollbar::ScrollbarExt;
use crate::tab_bar::{TabModel, tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::ListRowA11y;
use crate::widgets::button;

/// `.branches-container { width: 365px }`
#[allow(non_snake_case)]
pub fn BRANCH_FOLDOUT_WIDTH() -> Pixels {
    zpx(365.)
}

pub struct BranchFoldout {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    /// The Pull Requests tab has its own filter text (`PullRequestList.filterText`).
    pr_filter: Entity<InputState>,
    /// `pullRequestBeingViewed`
    quick_view: Option<QuickView>,
    /// `pullRequestQuickViewTimerId` (dropping the task cancels it).
    quick_view_timer: Option<Task<()>>,
    /// Window-space bounds of the branches container and of each pull
    /// request row (by number), recorded while painting.
    container_bounds: Rc<Cell<Bounds<Pixels>>>,
    row_bounds: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// The quick view card's height from the last paint (`quickViewHeight`).
    quick_view_height: Rc<Cell<Pixels>>,
    /// The pointer is over the quick view (GPUI may report entering it
    /// before leaving the row, so the row's leave timer checks this).
    quick_view_hovered: bool,
}

/// The pull request whose quick view is shown, its parsed body, and the
/// hovered row's top.
struct QuickView {
    pr: corvane_core::PullRequest,
    body: Vec<corvane_core::markdown::Block>,
    row_top: Pixels,
}

/// `onMouseEnterPullRequestListItem` delay and the leave grace period.
const QUICK_VIEW_SHOW_DELAY: Duration = Duration::from_millis(250);
const QUICK_VIEW_HIDE_DELAY: Duration = Duration::from_millis(500);

pub struct BranchGroup {
    pub title: &'static str,
    pub branches: Vec<Branch>,
}

/// GHD `groupBranches` over the merged local + remote-only branch list.
pub fn group_branches(
    branches: &[Branch],
    default_branch: Option<&str>,
    recent: &[String],
    query: &str,
) -> Vec<BranchGroup> {
    let query = query.trim();
    let matches = |b: &Branch| query.is_empty() || fuzzy_score(query, &b.name).is_some();
    // `mergeRemoteAndLocalBranches`: remote branches with a local counterpart are hidden
    let locals: Vec<&Branch> = branches
        .iter()
        .filter(|b| b.kind == BranchKind::Local)
        .collect();
    let mut all: Vec<Branch> = locals.iter().map(|b| (*b).clone()).collect();
    for b in branches.iter().filter(|b| b.kind == BranchKind::Remote) {
        if !locals.iter().any(|l| l.name == b.name_without_remote()) {
            all.push(b.clone());
        }
    }
    all.sort_by_key(|b| b.name.to_lowercase());

    let mut groups = Vec::new();
    if let Some(default) = default_branch.and_then(|d| all.iter().find(|b| b.name == d))
        && matches(default)
    {
        groups.push(BranchGroup {
            title: "Default Branch",
            branches: vec![default.clone()],
        });
    }
    let recent_branches: Vec<Branch> = recent
        .iter()
        .filter(|n| Some(n.as_str()) != default_branch)
        .filter_map(|n| {
            all.iter()
                .find(|b| b.name == *n && b.kind == BranchKind::Local)
        })
        .filter(|b| matches(b))
        .cloned()
        .collect();
    if !recent_branches.is_empty() {
        groups.push(BranchGroup {
            title: "Recent Branches",
            branches: recent_branches,
        });
    }
    let other: Vec<Branch> = all
        .iter()
        .filter(|b| Some(b.name.as_str()) != default_branch)
        .filter(|b| !recent.contains(&b.name) || b.kind == BranchKind::Remote)
        .filter(|b| matches(b))
        .cloned()
        .collect();
    if !other.is_empty() {
        groups.push(BranchGroup {
            title: "Other Branches",
            branches: other,
        });
    }
    groups
}

impl BranchFoldout {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        let pr_filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        cx.observe(&pr_filter, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            filter,
            pr_filter,
            quick_view: None,
            quick_view_timer: None,
            container_bounds: Rc::new(Cell::new(Bounds::default())),
            row_bounds: Rc::new(RefCell::new(HashMap::new())),
            quick_view_height: Rc::new(Cell::new(QUICK_VIEW_MAX_HEIGHT())),
            quick_view_hovered: false,
        }
    }

    /// `onMouseEnterPullRequestListItem` / `onMouseLeavePullRequestListItem`
    fn hover_pull_request(
        &mut self,
        pr: corvane_core::PullRequest,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        self.quick_view_timer = None;
        if hovered {
            self.quick_view_hovered = false;
            if self.quick_view.take().is_some() {
                cx.notify();
            }
            let number = pr.number;
            self.quick_view_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(QUICK_VIEW_SHOW_DELAY).await;
                this.update(cx, |this, cx| {
                    let row_top = this
                        .row_bounds
                        .borrow()
                        .get(&number)
                        .map(|b| b.origin.y)
                        .unwrap_or_default();
                    let body = if pr.body.trim().is_empty() {
                        "_No description provided._"
                    } else {
                        pr.body.as_str()
                    };
                    this.quick_view = Some(QuickView {
                        body: corvane_core::markdown::parse(body),
                        pr,
                        row_top,
                    });
                    this.quick_view_timer = None;
                    cx.notify();
                })
                .ok();
            }));
        } else if !self.quick_view_hovered {
            self.quick_view_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(QUICK_VIEW_HIDE_DELAY).await;
                this.update(cx, |this, cx| {
                    if this.quick_view_hovered {
                        return;
                    }
                    this.quick_view = None;
                    this.quick_view_timer = None;
                    cx.notify();
                })
                .ok();
            }));
        }
    }

    /// `onMouseEnterPullRequestQuickView` / `onMouseLeavePullRequestQuickView`
    fn hover_quick_view(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.quick_view_timer = None;
        self.quick_view_hovered = hovered;
        if !hovered {
            self.quick_view = None;
            cx.notify();
        }
    }

    /// The quick view, placed right of the foldout next to the hovered row.
    fn render_quick_view(&self, window: &Window, cx: &Context<Self>) -> Option<AnyElement> {
        let view = self.quick_view.as_ref()?;
        let container = self.container_bounds.get();
        let height = self.quick_view_height.get();
        let top = quick_view_top(
            view.row_top,
            container.origin.y,
            window.viewport_size().height,
            height,
        );
        let pointer_top = view.row_top - container.origin.y - top
            + crate::pull_request_list::PR_ROW_HEIGHT() / 2.
            + zpx(1.);
        let status = self.state.read(cx).commit_status_summary(&view.pr);
        let entity = cx.entity().downgrade();
        Some(
            div()
                .absolute()
                .left(container.size.width)
                .top(top)
                .child(
                    quick_view(
                        &view.pr,
                        &view.body,
                        status,
                        pointer_top,
                        self.quick_view_height.clone(),
                        cx,
                    )
                    .on_hover(move |hovered, _, cx| {
                        entity
                            .update(cx, |this, cx| this.hover_quick_view(*hovered, cx))
                            .ok();
                    }),
                )
                .into_any_element(),
        )
    }

    pub fn focus_filter(&self, window: &mut Window, cx: &mut Context<Self>) {
        let input = if self.pull_requests_tab_shown(cx) {
            &self.pr_filter
        } else {
            &self.filter
        };
        let handle = input.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    /// The tab bar only exists for GitHub repositories.
    fn is_github(&self, cx: &App) -> bool {
        let s = self.state.read(cx);
        s.selected
            .and_then(|id| s.repository(id))
            .is_some_and(|r| r.github.is_some())
    }

    fn pull_requests_tab_shown(&self, cx: &App) -> bool {
        self.is_github(cx) && self.state.read(cx).branches_tab == BranchesTab::PullRequests
    }

    /// `PullRequestList`: filter row, group header, rows or the blank slate.
    fn pull_requests_tab(&self, id: u64, window: &Window, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let query = self.pr_filter.read(cx).value().trim().to_string();
        let s = self.state.read(cx);
        let loading = s.pull_requests_loading(id);
        let repository_name = s
            .repository(id)
            .and_then(|r| r.non_fork_github())
            .map(|gh| gh.full_name())
            .unwrap_or_default();
        let current = s.current_pull_request(id).map(|pr| pr.number);
        let rs = s.repo_states.get(&id);
        let on_default_branch = rs.is_some_and(|rs| {
            rs.default_branch.is_some()
                && rs.default_branch.as_deref()
                    == rs
                        .info
                        .as_ref()
                        .and_then(|i| i.current_branch())
                        .map(|b| b.name.as_str())
        });
        let all = s.pull_requests_for(id);
        let items: Vec<corvane_core::PullRequest> = all
            .iter()
            .filter(|pr| matches_filter(pr, &query))
            .cloned()
            .collect();
        let local_name = s.repository(id).map(|r| r.name()).unwrap_or_default();
        let rows: Vec<AnyElement> = items
            .iter()
            .map(|pr| {
                let status = s.commit_status_summary(pr);
                let entity = cx.entity().downgrade();
                let hovered_pr = pr.clone();
                let row_bounds = self.row_bounds.clone();
                let number = pr.number;
                pull_request_row(
                    id,
                    pr,
                    current == Some(pr.number),
                    status,
                    local_name.clone(),
                    cx,
                )
                .relative()
                .on_hover(move |hovered, _, cx| {
                    let pr = hovered_pr.clone();
                    entity
                        .update(cx, |this, cx| this.hover_pull_request(pr, *hovered, cx))
                        .ok();
                })
                .child(
                    canvas(
                        move |b, _, _| {
                            row_bounds.borrow_mut().insert(number, b);
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
                .into_any_element()
            })
            .collect();
        div()
            .id("pull-request-list")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                // `.filter-field-row` with `renderPostFilter` (the refresh button)
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING())
                    .p(SPACING())
                    .pb(SPACING_HALF())
                    .child(crate::widgets::filter_text_box(
                        "pull-request-filter",
                        &self.pr_filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    ))
                    .child(
                        button("pull-request-refresh", "", cx)
                            .flex_none()
                            .px(SPACING_HALF())
                            .when(loading, |d| d.opacity(0.6))
                            .icon_button_label("Refresh the list of pull requests")
                            .on_click(move |_, _, cx| {
                                if !loading {
                                    Dispatcher::refresh_pull_requests(id, true, cx)
                                }
                            })
                            .child({
                                let icon = octicon(Octicon::SyncClockwise, t.secondary_button_text);
                                if loading {
                                    spin(icon, "pull-request-refresh-spin")
                                } else {
                                    icon.into_any_element()
                                }
                            }),
                    ),
            )
            .child(if rows.is_empty() {
                no_pull_requests(
                    id,
                    repository_name,
                    !query.is_empty(),
                    loading && all.is_empty(),
                    on_default_branch,
                    cx,
                )
            } else {
                div()
                    .id("pull-request-rows")
                    .role(Role::List)
                    .aria_label("Pull requests")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .child(
                        // `.filter-list-group-header`
                        div()
                            .h(ROW_HEIGHT())
                            .pt(SPACING())
                            .px(SPACING())
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(FONT_SIZE())
                            .truncate()
                            .child(format!("Pull requests in {repository_name}")),
                    )
                    .children(rows)
                    .with_scrollbar()
                    .into_any_element()
            })
            .into_any_element()
    }

    fn row(&self, id: u64, branch: &Branch, current: bool, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let name = branch.name.clone();
        let date = branch
            .tip_time
            .filter(|s| *s > 0)
            .map(|s| relative(UNIX_EPOCH + Duration::from_secs(s as u64)));
        // `.list-item:hover`: `--list-item-hover-background-color`, text unchanged
        let list_hover = t.list_item_hover_background;
        let hover_bg = t.box_selected_active_background;
        let hover_text = t.box_selected_active_text;
        let branch_name_for_target = branch.name.clone();
        div()
            .id(SharedString::from(format!("branch-{}", branch.full_name)))
            .a11y_row(
                match &date {
                    Some(date) => format!("{}, {date}", branch.name),
                    None => branch.name.clone(),
                },
                current,
            )
            .h(ROW_HEIGHT())
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px(SPACING())
            .cursor_pointer()
            .when(current, |d| {
                d.bg(t.box_selected_background)
                    .text_color(t.box_selected_text)
            })
            .when(!current, move |d| {
                let target_name = branch_name_for_target.clone();
                d.hover(move |s| s.bg(list_hover))
                    .drag_over::<crate::history::CommitDrag>(move |s, _, _, _| {
                        s.bg(hover_bg).text_color(hover_text)
                    })
                    // `emitEnterDropTarget({ type: Branch })` → "Copy to <branch>" tooltip
                    .on_drag_move::<crate::history::CommitDrag>(move |ev, _, cx| {
                        if ev.bounds.contains(&ev.event.position) {
                            Dispatcher::set_drag_target(
                                Some(corvane_core::DropTarget::Branch(target_name.clone())),
                                cx,
                            );
                        }
                    })
            })
            .on_click(move |_, _, cx| {
                Dispatcher::close_foldout(cx);
                Dispatcher::checkout_branch(id, name.clone(), None, cx)
            })
            .on_drop({
                // `startCherryPickWithBranch`: drop commits on a branch to copy them there
                let target = branch.name.clone();
                move |drag: &crate::history::CommitDrag, _, cx| {
                    Dispatcher::set_drag_target(None, cx);
                    if current || drag.repo != id {
                        return;
                    }
                    Dispatcher::close_foldout(cx);
                    Dispatcher::start_cherry_pick_flow(id, drag.shas.clone(), cx);
                    Dispatcher::cherry_pick_to_branch(id, target.clone(), cx);
                }
            })
            .child(
                octicon(
                    if current {
                        Octicon::Check
                    } else {
                        Octicon::GitBranch
                    },
                    t.text,
                )
                .mr(SPACING_HALF()),
            )
            .child(
                div()
                    .flex_grow(2.)
                    .min_w_0()
                    .max_w(gpui_kit::relative(0.65))
                    .mr(SPACING_HALF())
                    .truncate()
                    .text_size(FONT_SIZE())
                    .child(branch.name.clone()),
            )
            .when_some(date, |d, date| {
                d.child(
                    div()
                        .flex_1()
                        .mr(SPACING_HALF())
                        .text_right()
                        .whitespace_nowrap()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .child(date),
                )
            })
    }

    /// `NoBranches`: shown when the filter matches nothing.
    fn no_branches(&self, id: u64, query: String, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .p(SPACING())
            .my(SPACING())
            .text_size(FONT_SIZE())
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Sorry, I can't find that branch"),
            )
            .child(
                div()
                    .mx(SPACING_DOUBLE())
                    .text_center()
                    .text_size(FONT_SIZE_SM())
                    .child("Do you want to create a new branch instead?"),
            )
            .child(
                crate::widgets::primary_button("no-branches-create", "Create New Branch", false, cx)
                    .m(SPACING_DOUBLE())
                    .w_full()
                    .on_click(move |_, _, cx| {
                        Dispatcher::close_foldout(cx);
                        Dispatcher::show_popup(
                            Popup::CreateBranch {
                                repo: id,
                                target_sha: None,
                                initial_name: query.clone(),
                            },
                            cx,
                        )
                    }),
            )
            .child(
                div()
                    .px(zpx(30.))
                    .text_center()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child("Protip! Press ⌘⇧N to quickly create a new branch from anywhere within the app"),
            )
    }
}

impl Render for BranchFoldout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let query = self.filter.read(cx).value().trim().to_string();
        let (id, groups, current, tip_valid) = {
            let s = self.state.read(cx);
            let id = s.selected;
            let rs = id.and_then(|id| s.repo_states.get(&id));
            let info = rs.and_then(|r| r.info.as_ref());
            let current = info
                .and_then(|i| i.current_branch())
                .map(|b| b.name.clone());
            let tip_valid = info.is_some_and(|i| matches!(i.tip, Tip::Valid { .. }));
            let groups = match (info, rs) {
                (Some(info), Some(rs)) => group_branches(
                    &info.branches,
                    rs.default_branch.as_deref(),
                    &rs.recent_branches,
                    &query,
                ),
                _ => Vec::new(),
            };
            (id, groups, current, tip_valid)
        };
        let Some(id) = id else {
            return div().into_any_element();
        };
        let query_for_new = query.clone();
        let is_github = self.is_github(cx);
        let tab = if is_github {
            self.state.read(cx).branches_tab
        } else {
            BranchesTab::Branches
        };
        let open_prs = self.state.read(cx).pull_requests_for(id).len();
        if tab == BranchesTab::PullRequests {
            // `CIStatus.subscribe` for every row on screen
            let refs: Vec<(corvane_core::GitHubRepository, String)> = {
                let query = self.pr_filter.read(cx).value().trim().to_string();
                self.state
                    .read(cx)
                    .pull_requests_for(id)
                    .iter()
                    .filter(|pr| matches_filter(pr, &query))
                    .filter_map(|pr| pr.base.repository.clone().map(|r| (r, pr.commit_ref())))
                    .collect()
            };
            for (base, git_ref) in refs {
                Dispatcher::touch_commit_status(&base, &git_ref, None, cx);
            }
            let container_bounds = self.container_bounds.clone();
            return div()
                .id("branches-container")
                .relative()
                .size_full()
                .flex()
                .flex_col()
                .min_h_0()
                .child(
                    canvas(move |b, _, _| container_bounds.set(b), |_, _, _, _| {})
                        .absolute()
                        .inset_0(),
                )
                .child(self.tab_bar(open_prs, tab, cx))
                .child(self.pull_requests_tab(id, window, cx))
                .children(self.merge_button_row(id, current.filter(|_| tip_valid), cx))
                .children(self.render_quick_view(window, cx))
                .into_any_element();
        }
        div()
            .id("branches-container")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .when(is_github, |d| d.child(self.tab_bar(open_prs, tab, cx)))
            .child(
                // `.filter-field-row`: [🔍 Filter][New Branch]
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING())
                    .p(SPACING())
                    .child(crate::widgets::filter_text_box(
                        "branch-filter",
                        &self.filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    ))
                    .child(button("new-branch", "New Branch", cx).flex_none().on_click(
                        move |_, _, cx| {
                            Dispatcher::close_foldout(cx);
                            Dispatcher::show_popup(
                                Popup::CreateBranch {
                                    repo: id,
                                    target_sha: None,
                                    initial_name: query_for_new.clone(),
                                },
                                cx,
                            )
                        },
                    )),
            )
            .child(if groups.is_empty() {
                self.no_branches(id, query, cx).into_any_element()
            } else {
                div()
                    .id("branches-list")
                    .role(Role::List)
                    .aria_label("Branches")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(groups.into_iter().map(|group| {
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                // `.filter-list-group-header`
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
                            .children(group.branches.iter().map(|b| {
                                self.row(id, b, current.as_deref() == Some(b.name.as_str()), cx)
                            }))
                    }))
                    .with_scrollbar()
                    .into_any_element()
            })
            .children(self.merge_button_row(id, current.filter(|_| tip_valid), cx))
            .into_any_element()
    }
}

impl BranchFoldout {
    /// `renderTabBar`: Branches | Pull Requests (with the open count bubble).
    fn tab_bar(&self, open_prs: usize, tab: BranchesTab, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        div()
            .flex_none()
            .border_t_1()
            .border_color(t.box_border)
            .child(tab_bar(
                vec![
                    TabModel {
                        id: "branches-tab",
                        label: "Branches".into(),
                        count: None,
                    },
                    TabModel {
                        id: "pull-requests-tab",
                        label: "Pull Requests".into(),
                        count: (open_prs > 0).then_some(open_prs),
                    },
                ],
                match tab {
                    BranchesTab::Branches => 0,
                    BranchesTab::PullRequests => 1,
                },
                |ix, _, cx| {
                    Dispatcher::change_branches_tab(
                        if ix == 0 {
                            BranchesTab::Branches
                        } else {
                            BranchesTab::PullRequests
                        },
                        cx,
                    )
                },
                cx,
            ))
            .into_any_element()
    }

    /// `.merge-button-row`: "Choose a branch to merge into <current>".
    fn merge_button_row(
        &self,
        id: u64,
        current: Option<String>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let t = cx.ghd();
        let current = current?;
        Some(
            div()
                .flex_none()
                .p(SPACING())
                .border_t_1()
                .border_color(t.box_border)
                .child(
                    button("merge-into-current", "", cx)
                        .w_full()
                        .justify_center()
                        .gap(SPACING_HALF())
                        .child(octicon(Octicon::GitMerge, t.secondary_button_text))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .child("Choose a branch to merge into\u{a0}")
                                .child(div().font_weight(FontWeight::SEMIBOLD).child(current)),
                        )
                        .on_click(move |_, _, cx| {
                            Dispatcher::close_foldout(cx);
                            Dispatcher::show_popup(
                                Popup::MergeBranch {
                                    repo: id,
                                    squash: false,
                                },
                                cx,
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}
