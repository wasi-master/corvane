//! Branch foldout - GHD `ui/branches/{branches-container,branch-list,
//! branch-list-item,group-branches,no-branches}.tsx`
//! (`styles/ui/_branches.scss`, `_no-branches.scss`, `_filter-list.scss`):
//! `[🔍 Filter][New Branch]`, groups Default Branch / Recent Branches /
//! Other Branches, 29 px rows (check or branch icon, name, relative date) and
//! the "Choose a branch to merge into <current>" footer. GitHub repositories
//! get the Branches / Pull Requests tab bar (`branches-container.tsx`); the
//! pull request rows come from `pull_request_list.rs`.

use std::time::{Duration, UNIX_EPOCH};

use corvane_core::filter::fuzzy_score;
use corvane_core::{AppState, Branch, BranchKind, BranchesTab, Dispatcher, Popup, Tip};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::pull_request_list::{matches_filter, no_pull_requests, pull_request_row};
use crate::relative_time::relative;
use crate::tab_bar::{TabModel, tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, text_box};

/// `.branches-container { width: 365px }`
pub const BRANCH_FOLDOUT_WIDTH: Pixels = px(365.);

pub struct BranchFoldout {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    /// The Pull Requests tab has its own filter text (`PullRequestList.filterText`).
    pr_filter: Entity<InputState>,
}

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
        }
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
        let rows: Vec<AnyElement> = items
            .iter()
            .map(|pr| {
                let status = s.commit_status_summary(pr);
                pull_request_row(id, pr, current == Some(pr.number), status, cx)
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
                    .gap(SPACING)
                    .p(SPACING)
                    .pb(SPACING_HALF)
                    .child(text_box(
                        "pull-request-filter",
                        &self.pr_filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    ))
                    .child(
                        button("pull-request-refresh", "", cx)
                            .flex_none()
                            .px(SPACING_HALF)
                            .when(loading, |d| d.opacity(0.6))
                            .tooltip(crate::widgets::tooltip("Refresh the list of pull requests"))
                            .on_click(move |_, _, cx| {
                                if !loading {
                                    Dispatcher::refresh_pull_requests(id, true, cx)
                                }
                            })
                            .child(octicon(Octicon::Sync, t.secondary_button_text)),
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
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .child(
                        // `.filter-list-group-header`
                        div()
                            .h(ROW_HEIGHT)
                            .pt(SPACING)
                            .px(SPACING)
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(FONT_SIZE)
                            .truncate()
                            .child(format!("Pull requests in {repository_name}")),
                    )
                    .children(rows)
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
        let hover_bg = t.box_selected_active_background;
        let hover_text = t.box_selected_active_text;
        let branch_name_for_target = branch.name.clone();
        div()
            .id(SharedString::from(format!("branch-{}", branch.full_name)))
            .h(ROW_HEIGHT)
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px(SPACING)
            .cursor_pointer()
            .when(current, |d| {
                d.bg(t.box_selected_background)
                    .text_color(t.box_selected_text)
            })
            .when(!current, move |d| {
                let target_name = branch_name_for_target.clone();
                d.hover(move |s| s.bg(hover_bg).text_color(hover_text))
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
                .mr(SPACING_HALF),
            )
            .child(
                div()
                    .flex_grow(2.)
                    .min_w_0()
                    .max_w(gpui_kit::relative(0.65))
                    .mr(SPACING_HALF)
                    .truncate()
                    .text_size(FONT_SIZE)
                    .child(branch.name.clone()),
            )
            .when_some(date, |d, date| {
                d.child(
                    div()
                        .flex_1()
                        .mr(SPACING_HALF)
                        .text_right()
                        .whitespace_nowrap()
                        .text_size(FONT_SIZE_SM)
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
            .p(SPACING)
            .my(SPACING)
            .text_size(FONT_SIZE)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Sorry, I can't find that branch"),
            )
            .child(
                div()
                    .mx(SPACING_DOUBLE)
                    .text_center()
                    .text_size(FONT_SIZE_SM)
                    .child("Do you want to create a new branch instead?"),
            )
            .child(
                crate::widgets::primary_button("no-branches-create", "Create New Branch", false, cx)
                    .m(SPACING_DOUBLE)
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
                    .px(px(30.))
                    .text_center()
                    .text_size(FONT_SIZE_SM)
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
            return div()
                .id("branches-container")
                .size_full()
                .flex()
                .flex_col()
                .min_h_0()
                .child(self.tab_bar(open_prs, tab, cx))
                .child(self.pull_requests_tab(id, window, cx))
                .children(self.merge_button_row(id, current.filter(|_| tip_valid), cx))
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
                    .gap(SPACING)
                    .p(SPACING)
                    .child(text_box(
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
                                    .h(ROW_HEIGHT)
                                    .pt(SPACING)
                                    .px(SPACING)
                                    .flex()
                                    .items_center()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(FONT_SIZE)
                                    .child(group.title),
                            )
                            .children(group.branches.iter().map(|b| {
                                self.row(id, b, current.as_deref() == Some(b.name.as_str()), cx)
                            }))
                    }))
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
                .p(SPACING)
                .border_t_1()
                .border_color(t.box_border)
                .child(
                    button("merge-into-current", "", cx)
                        .w_full()
                        .justify_center()
                        .gap(SPACING_HALF)
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
