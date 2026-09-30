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
//!
//! Deviations: dates are the tip's committer date (GHD: author date); Other
//! Branches can be sorted newest first (`848-branch-list-sort-by-date`).
//! The filter ignores an `owner:` prefix (`849-branch-filter-strips-owner`).
//! Rows can tell local-only, tracked and remote-only branches apart by icon
//! (`850-branch-list-local-remote-icons`), and a filter-row toggle can
//! narrow the list to remote branches (`851-branch-list-remote-only`).
//! The context menu can start a rebase onto the branch
//! (`856-branch-menu-rebase-onto`).
//! Deviation (`852-branch-upstream-gone`): a local branch whose upstream was
//! deleted on the remote shows a cloud-offline icon after its name.
//! Deviation (`853-branch-list-ahead-behind`): local branch rows show their
//! commits to push / pull ("2↑ 1↓") or an upload icon when unpublished.
//! Deviation (`854-branch-list-stash-icon`): a local branch with a Desktop
//! stash shows the stash icon after its name (GHD `branch-list-item.tsx` does
//! not).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, UNIX_EPOCH};

use corvane_core::filter::fuzzy_score;
use corvane_core::{AppState, Branch, BranchKind, BranchesTab, Dispatcher, Popup, Tip};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::{FilterListPick, SelectNextFile, SelectPreviousFile};
use crate::widgets::GhdTooltip;
use crate::widgets::IconButtonA11y;

use crate::icons::{Octicon, octicon, spin};
use crate::pull_request_list::{
    QUICK_VIEW_MAX_HEIGHT, matches_filter, no_pull_requests, pull_request_row, quick_view,
    quick_view_top, signed_out_pull_requests,
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
    /// `FilterList` selection: the row last pressed or right-clicked (the
    /// current branch until then), and whether the list has keyboard focus.
    selected_row: Option<String>,
    /// The row drawn as selected this frame: the pressed row while it is
    /// visible, else the current branch, else - with a filter typed - the
    /// first match (`FilterList` moves the selection into the results).
    shown_selected: Option<String>,
    list_focus: FocusHandle,
    list_focused: bool,
    /// `851-branch-list-remote-only`: the list shows remote branches only.
    remote_only: bool,
    /// GHD `FilterList` keyboard selection: the branch row ↓ / ↑ moved to
    /// from the filter box (an index into the rows as shown, groups
    /// flattened); while set it is the list's selection.
    highlighted: Option<usize>,
    scroll: UniformListScrollHandle,
    /// The same for the Pull Requests tab.
    pr_highlighted: Option<usize>,
    pr_scroll: ScrollHandle,
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

/// Flag `849-branch-filter-strips-owner`: `owner:branch` (GitHub's
/// copy-branch-name format) filters by `branch`. `:` can't appear in a ref
/// name, so nothing that could match is lost.
fn strip_owner_prefix(query: &str, cx: &App) -> String {
    match query.split_once(':') {
        Some((owner, branch))
            if !owner.is_empty()
                && !branch.trim().is_empty()
                && AppState::global(cx)
                    .read(cx)
                    .flags
                    .bool(corvane_core::flags::ids::BRANCH_FILTER_STRIPS_OWNER) =>
        {
            branch.trim().to_string()
        }
        _ => query.to_string(),
    }
}

/// Flag `851-branch-list-remote-only`: every remote branch matching `query`
/// (those with a local counterpart too), in one "Remote Branches" group.
fn remote_group(branches: &[Branch], query: &str, cx: &App) -> Vec<BranchGroup> {
    let mut remote: Vec<Branch> = branches
        .iter()
        .filter(|b| b.kind == BranchKind::Remote)
        .filter(|b| query.is_empty() || fuzzy_score(query, &b.name).is_some())
        .cloned()
        .collect();
    remote.sort_by_key(|b| b.name.to_lowercase());
    if sort_by_date(cx) {
        remote.sort_by_key(|b| std::cmp::Reverse(b.tip_time.unwrap_or(0)));
    }
    if remote.is_empty() {
        Vec::new()
    } else {
        vec![BranchGroup {
            title: "Remote Branches",
            branches: remote,
        }]
    }
}

/// GHD `BranchesContainer.onBranchItemClick` (a click, or Enter in the
/// filter box): close the foldout and check the branch out, or ask first
/// (`864-confirm-branch-switch`).
fn checkout_branch_row(id: u64, name: String, current: bool, cx: &mut App) {
    Dispatcher::close_foldout(cx);
    if AppState::global(cx)
        .read(cx)
        .flags
        .bool(corvane_core::flags::ids::CONFIRM_BRANCH_SWITCH)
        && !current
    {
        Dispatcher::show_popup(
            Popup::ConfirmSwitchBranch {
                repo: id,
                branch: name,
            },
            cx,
        );
        return;
    }
    Dispatcher::checkout_branch(id, name, None, cx)
}

/// Flag `848-branch-list-sort-by-date`: Other Branches newest first.
pub fn sort_by_date(cx: &App) -> bool {
    AppState::try_global(cx).is_some_and(|s| {
        s.read(cx)
            .flags
            .bool(corvane_core::flags::ids::BRANCH_LIST_SORT_BY_DATE)
    })
}

/// GHD `groupBranches` over the merged local + remote-only branch list.
/// `newest_first` orders Other Branches by the tip's committer date (newest
/// first, then by name) instead of by name alone.
pub fn group_branches(
    branches: &[Branch],
    default_branch: Option<&str>,
    recent: &[String],
    query: &str,
    newest_first: bool,
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
    let mut other: Vec<Branch> = all
        .iter()
        .filter(|b| Some(b.name.as_str()) != default_branch)
        .filter(|b| !recent.contains(&b.name) || b.kind == BranchKind::Remote)
        .filter(|b| matches(b))
        .cloned()
        .collect();
    if newest_first {
        // stable: equal dates keep the name order
        other.sort_by_key(|b| std::cmp::Reverse(b.tip_time.unwrap_or(0)));
    }
    if !other.is_empty() {
        groups.push(BranchGroup {
            title: "Other Branches",
            branches: other,
        });
    }
    groups
}

/// Remote-tracking branches that [`group_branches`] hides behind their local
/// branch (`origin/main` when `main` exists), for the rebase list (flag
/// `832`): rebasing onto the fetched remote needs no pull of the local one.
pub fn remote_counterparts(branches: &[Branch], query: &str) -> Option<BranchGroup> {
    let query = query.trim();
    let mut remotes: Vec<Branch> = branches
        .iter()
        .filter(|b| b.kind == BranchKind::Remote && !b.name.ends_with("/HEAD"))
        .filter(|b| {
            branches
                .iter()
                .any(|l| l.kind == BranchKind::Local && l.name == b.name_without_remote())
        })
        .filter(|b| query.is_empty() || fuzzy_score(query, &b.name).is_some())
        .cloned()
        .collect();
    remotes.sort_by_key(|b| b.name.to_lowercase());
    (!remotes.is_empty()).then_some(BranchGroup {
        title: "Remote Branches",
        branches: remotes,
    })
}

impl BranchFoldout {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        let pr_filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        cx.observe(&filter, |this: &mut Self, _, cx| {
            this.highlighted = None;
            cx.notify()
        })
        .detach();
        cx.observe(&pr_filter, |this: &mut Self, _, cx| {
            this.pr_highlighted = None;
            cx.notify()
        })
        .detach();
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
            selected_row: None,
            shown_selected: None,
            list_focus: cx.focus_handle(),
            list_focused: false,
            remote_only: false,
            highlighted: None,
            scroll: UniformListScrollHandle::new(),
            pr_highlighted: None,
            pr_scroll: ScrollHandle::new(),
        }
    }

    /// The Branches tab's groups as the list shows them.
    fn branch_groups(&self, cx: &App) -> Vec<BranchGroup> {
        let query = self.filter_text(cx);
        let s = self.state.read(cx);
        let remote_only = self.remote_only
            && s.flags
                .bool(corvane_core::flags::ids::BRANCH_LIST_REMOTE_ONLY);
        let Some(rs) = s.selected.and_then(|id| s.repo_states.get(&id)) else {
            return Vec::new();
        };
        let Some(info) = rs.info.as_ref() else {
            return Vec::new();
        };
        if remote_only {
            remote_group(&info.branches, &query, cx)
        } else {
            group_branches(
                &info.branches,
                rs.default_branch.as_deref(),
                &rs.recent_branches,
                &query,
                sort_by_date(cx),
            )
        }
    }

    /// The Pull Requests tab's rows as the list shows them.
    fn pull_request_items(&self, id: u64, cx: &App) -> Vec<corvane_core::PullRequest> {
        let query = self.pr_filter.read(cx).value().trim().to_string();
        self.state
            .read(cx)
            .pull_requests_for(id)
            .iter()
            .filter(|pr| matches_filter(pr, &query))
            .cloned()
            .collect()
    }

    /// GHD `ui/lib/filter-list.tsx` `onFilterKeyDown`: ↓ / ↑ in the
    /// Branches filter box move through the branch rows (↑ from the filter
    /// starts at the last), clamped, skipping the group headers.
    fn move_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        let sizes: Vec<usize> = self
            .branch_groups(cx)
            .iter()
            .map(|g| g.branches.len())
            .collect();
        let Some(ix) = crate::filter_list::step(self.highlighted, delta, sizes.iter().sum()) else {
            return;
        };
        self.highlighted = Some(ix);
        // the list is uniform (headers and `.branches-list-item` rows are
        // both 30 px): its item index counts the headers above the row
        let mut before = 0;
        let headers = sizes
            .iter()
            .take_while(|size| {
                before += **size;
                before <= ix
            })
            .count()
            + 1;
        self.scroll
            .scroll_to_item(ix + headers, ScrollStrategy::Top);
        cx.notify();
    }

    /// GHD `ui/lib/filter-list.tsx` `onFilterKeyDown` (Enter) and
    /// `onEnterPressed`: Enter in the Branches filter box checks out the
    /// highlighted branch, else - with a filter typed - the first one, as a
    /// click does; with a filter that matches nothing it opens Create
    /// Branch with the text (`onEnterPressedWithoutFilteredItems`).
    fn pick_highlighted(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let query = self.filter_text(cx);
        let branches: Vec<Branch> = self
            .branch_groups(cx)
            .into_iter()
            .flat_map(|g| g.branches)
            .collect();
        let ix = self
            .highlighted
            .or_else(|| (!query.is_empty()).then_some(0));
        if let Some(branch) = ix.and_then(|ix| branches.get(ix)) {
            let current = {
                let s = self.state.read(cx);
                s.repo_states
                    .get(&id)
                    .and_then(|rs| rs.info.as_ref())
                    .and_then(|i| i.current_branch())
                    .is_some_and(|b| b.name == branch.name)
            };
            checkout_branch_row(id, branch.name.clone(), current, cx);
        } else if branches.is_empty() && !query.is_empty() {
            Dispatcher::close_foldout(cx);
            Dispatcher::show_popup(
                Popup::CreateBranch {
                    repo: id,
                    target_sha: None,
                    initial_name: query,
                },
                cx,
            );
        }
    }

    /// GHD `ui/lib/filter-list.tsx` `onFilterKeyDown` in the Pull Requests
    /// filter box (`SectionFilterList`): ↓ / ↑ move through the rows.
    fn move_pr_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let count = self.pull_request_items(id, cx).len();
        let Some(ix) = crate::filter_list::step(self.pr_highlighted, delta, count) else {
            return;
        };
        self.pr_highlighted = Some(ix);
        // one "Pull requests in …" header, then 47 px rows
        let top = crate::filter_list::row_top(
            &[count],
            ix,
            ROW_HEIGHT(),
            crate::pull_request_list::PR_ROW_HEIGHT(),
        );
        crate::filter_list::scroll_into_view(
            &self.pr_scroll,
            top,
            crate::pull_request_list::PR_ROW_HEIGHT(),
        );
        cx.notify();
    }

    /// GHD `ui/lib/filter-list.tsx` `onFilterKeyDown` (Enter): the
    /// highlighted pull request, else - with a filter typed - the first,
    /// checked out as a click does (`PullRequestList.onItemClick`).
    fn pick_pr_highlighted(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let query_empty = self.pr_filter.read(cx).value().trim().is_empty();
        let ix = self.pr_highlighted.or_else(|| (!query_empty).then_some(0));
        if let Some(pr) = ix.and_then(|ix| self.pull_request_items(id, cx).into_iter().nth(ix)) {
            Dispatcher::close_foldout(cx);
            Dispatcher::checkout_pull_request(id, pr, cx);
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

    /// The Branches tab's filter text (`847-new-branch-from-filter`), with
    /// an `owner:` prefix stripped as the list does.
    pub fn filter_text(&self, cx: &App) -> String {
        strip_owner_prefix(self.filter.read(cx).value().trim(), cx)
    }

    pub fn focus_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // a freshly opened list selects the current branch again
        self.selected_row = None;
        self.highlighted = None;
        self.pr_highlighted = None;
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
        // `pull-requests-signed-out`: no account for the endpoint, so the
        // list cannot load (GHD shows "You're all set!" and a refresh
        // button that does nothing)
        let signed_out_endpoint = s
            .flags
            .bool(corvane_core::flags::ids::PULL_REQUESTS_SIGNED_OUT)
            .then(|| s.repository(id).and_then(|r| r.non_fork_github()))
            .flatten()
            .filter(|gh| s.account_for(&gh.endpoint).is_none())
            .map(|gh| gh.endpoint.clone());
        let signed_out = signed_out_endpoint.is_some();
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
        let items = self.pull_request_items(id, cx);
        let highlighted = self.pr_highlighted.filter(|ix| *ix < items.len());
        let highlight_bg = t.box_selected_active_background;
        let highlight_text = t.box_selected_active_text;
        let local_name = s.repository(id).map(|r| r.name()).unwrap_or_default();
        let rows: Vec<AnyElement> = items
            .iter()
            .enumerate()
            .map(|(ix, pr)| {
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
                // the keyboard row (GHD's focused-list selection)
                .when(highlighted == Some(ix), |d| {
                    d.bg(highlight_bg).text_color(highlight_text)
                })
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
                    .key_context("PullRequestFilter")
                    .on_action(
                        cx.listener(|this, _: &SelectNextFile, _, cx| {
                            this.move_pr_highlight(1, cx)
                        }),
                    )
                    .on_action(cx.listener(|this, _: &SelectPreviousFile, _, cx| {
                        this.move_pr_highlight(-1, cx)
                    }))
                    .on_action(
                        cx.listener(|this, _: &FilterListPick, _, cx| this.pick_pr_highlighted(cx)),
                    )
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
                            .when(signed_out, |d| d.opacity(0.6).cursor_default())
                            .icon_button_label("Refresh the list of pull requests")
                            .on_click(move |_, _, cx| {
                                if !loading && !signed_out {
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
            .child(
                if let Some(endpoint) = signed_out_endpoint.filter(|_| rows.is_empty()) {
                    signed_out_pull_requests(repository_name, endpoint, cx)
                } else if rows.is_empty() {
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
                        .with_scrollbar_handle(&self.pr_scroll)
                        .into_any_element()
                },
            )
            .into_any_element()
    }

    /// `highlighted`: the filter box's keyboard row (GHD `FilterList`);
    /// `stashed`: the branch has a Desktop stash (`854-branch-list-stash-icon`);
    /// `tracking`: its upstream state (`852-branch-upstream-gone`).
    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        id: u64,
        branch: &Branch,
        current: bool,
        highlighted: bool,
        stashed: bool,
        tracking: Option<corvane_git::BranchTracking>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        // `853-branch-list-ahead-behind`: unpublished, or commits to push / pull
        let ahead_behind = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::BRANCH_LIST_AHEAD_BEHIND);
        let sync_state = (ahead_behind && branch.kind == BranchKind::Local)
            .then(|| match (&branch.upstream, tracking) {
                (None, _) => Some(("Not published".to_string(), None)),
                (Some(_), Some(t)) if !t.gone && (t.ahead > 0 || t.behind > 0) => Some((
                    format!("{} to push, {} to pull", t.ahead, t.behind),
                    Some((t.ahead, t.behind)),
                )),
                _ => None,
            })
            .flatten();
        let t = cx.ghd();
        let name = branch.name.clone();
        // the keyboard row (GHD `FilterList` moves its selection, focusing
        // the list) replaces the pointer / current-branch selection
        let keyboard = self.highlighted.is_some();
        let selected = if keyboard {
            highlighted
        } else {
            self.shown_selected.as_deref() == Some(branch.name.as_str())
        };
        let focused = keyboard || self.list_focused;
        let date = branch
            .tip_time
            .filter(|s| *s > 0)
            .map(|s| relative(UNIX_EPOCH + Duration::from_secs(s as u64)));
        // `.list-item:hover`: `--list-item-hover-background-color`, text unchanged
        let list_hover = t.list_item_hover_background;
        let hover_bg = t.box_selected_active_background;
        let hover_text = t.box_selected_active_text;
        let branch_name_for_target = branch.name.clone();
        let distinguish_remote = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::BRANCH_LIST_LOCAL_REMOTE_ICONS);
        div()
            .id(SharedString::from(format!("branch-{}", branch.full_name)))
            .a11y_row(
                match &date {
                    Some(date) => format!("{}, {date}", branch.name),
                    None => branch.name.clone(),
                },
                current,
            )
            // `.branches-list-item`: 30 px rows
            .h(zpx(30.))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px(SPACING())
            .cursor_pointer()
            // GHD `List.onRowMouseDown`: pressing (or right-clicking) a row
            // selects it and focuses the list; the click then checks it out
            .on_mouse_down(
                MouseButton::Left,
                cx.listener({
                    let name = branch.name.clone();
                    move |this, _, window, cx| this.select_row(name.clone(), window, cx)
                }),
            )
            .when(selected && focused, |d| {
                d.bg(t.box_selected_active_background)
                    .text_color(t.box_selected_active_text)
            })
            .when(selected && !focused, |d| {
                d.bg(t.box_selected_background)
                    .text_color(t.box_selected_text)
            })
            .when(
                !(selected && (focused || crate::widgets::selection_keeps_colour_on_hover(cx))),
                move |d| d.hover(move |s| s.bg(list_hover)),
            )
            .when(!current, move |d| {
                let target_name = branch_name_for_target.clone();
                d.drag_over::<crate::history::CommitDrag>(move |s, _, _, _| {
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
            .on_click(move |_, _, cx| checkout_branch_row(id, name.clone(), current, cx))
            // GHD `generateBranchContextMenuItems`
            .on_mouse_down(MouseButton::Right, {
                let branch = branch.clone();
                let this = cx.entity().downgrade();
                move |ev: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.update(cx, |this, cx| {
                        this.select_row(branch.name.clone(), window, cx)
                    })
                    .ok();
                    use crate::context_menu::MenuItem;
                    let local = branch.kind == BranchKind::Local;
                    // `856-branch-menu-rebase-onto`
                    let rebase_onto = AppState::global(cx)
                        .read(cx)
                        .flags
                        .bool(corvane_core::flags::ids::BRANCH_MENU_REBASE_ONTO)
                        .then(|| branch.name.clone());
                    let can_rebase = !current && {
                        let s = AppState::global(cx).read(cx);
                        s.repo_states.get(&id).is_some_and(|r| {
                            r.mco.is_none()
                                && r.info
                                    .as_ref()
                                    .is_some_and(|i| matches!(i.tip, Tip::Valid { .. }))
                        })
                    };
                    let (rename, copy, worktree, delete) = (
                        branch.name.clone(),
                        branch.name.clone(),
                        branch.name.clone(),
                        branch.name.clone(),
                    );
                    // Corvane addition (`857-update-branch-from-upstream`)
                    let update = (local && !current)
                        .then(|| branch.upstream_short())
                        .flatten()
                        .filter(|_| {
                            corvane_core::AppState::global(cx)
                                .read(cx)
                                .flags
                                .bool(corvane_core::flags::ids::UPDATE_BRANCH_FROM_UPSTREAM)
                        })
                        .map(|upstream| {
                            let name = branch.name.clone();
                            MenuItem::new(format!("Update from {upstream}"), move |_, cx| {
                                Dispatcher::close_foldout(cx);
                                Dispatcher::update_branch_from_upstream(id, name.clone(), cx)
                            })
                        });
                    let mut items = vec![
                        MenuItem::new("Rename…", move |_, cx| {
                            Dispatcher::close_foldout(cx);
                            Dispatcher::show_popup(
                                Popup::RenameBranch {
                                    repo: id,
                                    name: rename.clone(),
                                },
                                cx,
                            )
                        })
                        .enabled(local),
                        MenuItem::new("Copy Branch Name", move |_, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                        }),
                        MenuItem::new("Checkout in New Worktree…", move |_, cx| {
                            Dispatcher::close_foldout(cx);
                            Dispatcher::show_popup(
                                Popup::AddWorktree {
                                    repo: id,
                                    initial_branch_name: Some(worktree.clone()),
                                    initial_worktree_name: None,
                                },
                                cx,
                            )
                        }),
                        MenuItem::separator(),
                    ];
                    if let Some(base) = rebase_onto {
                        items.push(
                            MenuItem::new(
                                format!("Rebase Current Branch onto {base}…"),
                                move |_, cx| {
                                    Dispatcher::close_foldout(cx);
                                    Dispatcher::start_rebase_flow_onto(id, Some(base.clone()), cx);
                                },
                            )
                            .enabled(can_rebase),
                        );
                        items.push(MenuItem::separator());
                    }
                    items.extend([MenuItem::new("Delete…", move |_, cx| {
                        Dispatcher::close_foldout(cx);
                        Dispatcher::show_popup(
                            Popup::DeleteBranch {
                                repo: id,
                                name: delete.clone(),
                            },
                            cx,
                        )
                    })]);
                    if let Some(update) = update {
                        items.insert(2, update);
                    }
                    crate::native_menu::show_context_menu(items, ev.position, window, cx);
                }
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
                    } else if distinguish_remote {
                        // `850-branch-list-local-remote-icons`
                        match (branch.kind, branch.upstream.is_some()) {
                            (BranchKind::Remote, _) => Octicon::Server,
                            (BranchKind::Local, false) => Octicon::DeviceDesktop,
                            (BranchKind::Local, true) => Octicon::GitBranch,
                        }
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
            .when_some(sync_state, |d, (tooltip, counts)| {
                let content = match counts {
                    None => div()
                        .flex()
                        .child(octicon(Octicon::Upload, t.text_secondary))
                        .into_any_element(),
                    Some((ahead, behind)) => div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(zpx(2.))
                        .text_size(FONT_SIZE_SM())
                        .when(!selected, |d| d.text_color(t.text_secondary))
                        .when(ahead > 0, |d| d.child(format!("{ahead}↑")))
                        .when(behind > 0, |d| d.child(format!("{behind}↓")))
                        .into_any_element(),
                };
                d.child(
                    div()
                        .id(SharedString::from(format!(
                            "branch-sync-{}",
                            branch.full_name
                        )))
                        .flex_none()
                        .mr(SPACING_HALF())
                        .child(content)
                        .ghd_tooltip(tooltip),
                )
            })
            .when(tracking.is_some_and(|t| t.gone), |d| {
                d.child(
                    div()
                        .id(SharedString::from(format!(
                            "branch-gone-{}",
                            branch.full_name
                        )))
                        .flex_none()
                        .mr(SPACING_HALF())
                        .child(octicon(Octicon::CloudOffline, t.text_secondary))
                        .ghd_tooltip("Deleted on the remote"),
                )
            })
            .when(stashed, |d| {
                d.child(
                    div()
                        .id(SharedString::from(format!(
                            "branch-stash-{}",
                            branch.full_name
                        )))
                        .flex_none()
                        .mr(SPACING_HALF())
                        .child(octicon(Octicon::Stash, t.text_secondary))
                        .ghd_tooltip("Stashed changes"),
                )
            })
            .when_some(date, |d, date| {
                d.child(
                    div()
                        .flex_1()
                        .mr(SPACING_HALF())
                        .text_right()
                        .whitespace_nowrap()
                        .text_size(FONT_SIZE_SM())
                        .line_height(zpx(16.5))
                        // the selected row's date takes the row colour
                        .when(!selected, |d| d.text_color(t.text_secondary))
                        .child(date),
                )
            })
    }

    fn select_row(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_row = Some(name);
        self.highlighted = None;
        window.focus(&self.list_focus, cx);
        cx.notify();
    }

    /// `NoBranches`: shown when the filter matches nothing. `.no-branches`
    /// in a resizable `.branches-container`: 365 px wide, 10 px margin and
    /// padding, the illustration at full width (`.foldout .blankslate-image`).
    fn no_branches(&self, id: u64, query: String, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex_none()
            .w(zpx(365.))
            .mx_auto()
            .my(SPACING())
            .p(SPACING())
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .text_size(FONT_SIZE())
            .line_height(zpx(18.))
            // 257 × 85 at the 345 px content width
            .child(
                crate::widgets::blankslate_image("empty-no-branches.svg", cx)
                    .w(zpx(345.))
                    .h(zpx(345. * 85. / 257.)),
            )
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Sorry, I can't find that branch"),
            )
            .child(
                div()
                    .mx(SPACING_DOUBLE())
                    .text_size(FONT_SIZE_SM())
                    .line_height(zpx(16.5))
                    .child("Do you want to create a new branch instead?"),
            )
            .child(
                crate::widgets::primary_button(
                    "no-branches-create",
                    "Create New Branch",
                    false,
                    cx,
                )
                .m(SPACING_DOUBLE())
                .self_stretch()
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
                // `.protip` with a `KeyboardShortcut` (⌘⇧N) in the sentence
                crate::widgets::paragraph(vec![
                    "ProTip! Press ".into(),
                    // `kbd` inherits the 11 px `.protip` text
                    div()
                        .flex()
                        .flex_row()
                        .gap(zpx(2.))
                        .children(
                            ["⌘", "⇧", "N"]
                                .map(|k| crate::widgets::kbd(k, cx).text_size(FONT_SIZE_SM())),
                        )
                        .into_any_element()
                        .into(),
                    " to quickly create a new branch from anywhere within the app".into(),
                ])
                .justify_center()
                .px(SPACING() * 3.)
                .text_size(FONT_SIZE_SM())
                .line_height(zpx(16.5)),
            )
    }
}

impl Render for BranchFoldout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        self.list_focused = self.list_focus.is_focused(window);
        let query = strip_owner_prefix(self.filter.read(cx).value().trim(), cx);
        let remote_toggle = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::BRANCH_LIST_REMOTE_ONLY);
        let remote_only = remote_toggle && self.remote_only;
        let groups = self.branch_groups(cx);
        let (id, current, tip_valid, stashed, tracking) = {
            let s = self.state.read(cx);
            let id = s.selected;
            let rs = id.and_then(|id| s.repo_states.get(&id));
            let stashed = rs
                .filter(|_| {
                    s.flags
                        .bool(corvane_core::flags::ids::BRANCH_LIST_STASH_ICON)
                })
                .map(|rs| rs.stashed_branches.clone())
                .unwrap_or_default();
            let tracking = rs.map(|rs| rs.branch_tracking.clone()).unwrap_or_default();
            let info = rs.and_then(|r| r.info.as_ref());
            let current = info
                .and_then(|i| i.current_branch())
                .map(|b| b.name.clone());
            let tip_valid = info.is_some_and(|i| matches!(i.tip, Tip::Valid { .. }));
            (id, current, tip_valid, stashed, tracking)
        };
        let row_count: usize = groups.iter().map(|g| g.branches.len()).sum();
        self.highlighted = self.highlighted.filter(|ix| *ix < row_count);
        let highlighted = self.highlighted;
        let Some(id) = id else {
            return div().into_any_element();
        };
        let visible = |name: &str| {
            groups
                .iter()
                .any(|g| g.branches.iter().any(|b| b.name == name))
        };
        self.shown_selected = match (&self.selected_row, &current) {
            (Some(row), _) if visible(row) => Some(row.clone()),
            (None, Some(cur)) if visible(cur) => Some(cur.clone()),
            _ if !query.is_empty() => groups
                .iter()
                .flat_map(|g| g.branches.first())
                .next()
                .map(|b| b.name.clone()),
            _ => None,
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
                    .key_context("BranchFilter")
                    .on_action(
                        cx.listener(|this, _: &SelectNextFile, _, cx| this.move_highlight(1, cx)),
                    )
                    .on_action(cx.listener(|this, _: &SelectPreviousFile, _, cx| {
                        this.move_highlight(-1, cx)
                    }))
                    .on_action(
                        cx.listener(|this, _: &FilterListPick, _, cx| this.pick_highlighted(cx)),
                    )
                    .child(crate::widgets::filter_text_box(
                        "branch-filter",
                        &self.filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    ))
                    .when(remote_toggle, |d| {
                        d.child(
                            button("branch-remote-only", "", cx)
                                .flex_none()
                                .px(SPACING_HALF())
                                .when(remote_only, |d| d.bg(t.box_selected_background))
                                .icon_button_label(if remote_only {
                                    "Show all branches"
                                } else {
                                    "Show only remote branches"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.remote_only = !this.remote_only;
                                    this.highlighted = None;
                                    cx.notify();
                                }))
                                .child(octicon(Octicon::Server, t.secondary_button_text)),
                        )
                    })
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
                // the filter list keeps growing; `.no-branches` sits at its top
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(self.no_branches(id, query, cx))
                    .into_any_element()
            } else {
                // one uniform list of group headers and rows (both 30 px), so
                // only the rows on screen are built: the whole list was built
                // every frame (and every keystroke in the filter) before
                // (group, branch in group, branch row counting only branches:
                // the keyboard highlight's index)
                let mut row = 0;
                let mut items: Vec<(usize, Option<usize>, usize)> = Vec::new();
                for (g, group) in groups.iter().enumerate() {
                    items.push((g, None, row));
                    for b in 0..group.branches.len() {
                        items.push((g, Some(b), row));
                        row += 1;
                    }
                }
                let count = items.len();
                let groups = std::rc::Rc::new(groups);
                let current = current.clone();
                div()
                    .id("branches-list")
                    .track_focus(&self.list_focus)
                    .role(Role::List)
                    .aria_label("Branches")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        uniform_list(
                            "branches-list-rows",
                            count,
                            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                range
                                    .map(|ix| match items[ix] {
                                        // `.filter-list-group-header`
                                        (g, None, _) => div()
                                            .h(zpx(30.))
                                            .px(SPACING())
                                            .flex()
                                            .items_center()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_size(FONT_SIZE())
                                            .child(groups[g].title)
                                            .into_any_element(),
                                        (g, Some(b), row) => {
                                            let b = &groups[g].branches[b];
                                            this.row(
                                                id,
                                                b,
                                                current.as_deref() == Some(b.name.as_str()),
                                                highlighted == Some(row),
                                                b.kind == BranchKind::Local
                                                    && stashed.contains(&b.name),
                                                (b.kind == BranchKind::Local)
                                                    .then(|| tracking.get(&b.name).copied())
                                                    .flatten(),
                                                cx,
                                            )
                                            .into_any_element()
                                        }
                                    })
                                    .collect()
                            }),
                        )
                        .flex_1()
                        .min_h_0()
                        .with_scrollbar_handle(&self.scroll),
                    )
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
                        dot: false,
                        id: "branches-tab",
                        label: "Branches".into(),
                        count: None,
                    },
                    TabModel {
                        dot: false,
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
