//! Pull Requests tab of the branch foldout - GHD
//! `ui/branches/{pull-request-list,pull-request-list-item,no-pull-requests}.tsx`
//! (`styles/ui/_branches.scss` `.pull-request-list`, `.no-pull-requests`):
//! 47 px rows with the PR icon, title, "#N opened … by author" subtitle and
//! the CI status, plus the blank slate. The rows live in the
//! `BranchFoldout`; this module renders them.
//!
//! The row context menu is GHD `pull-request-list-item-context-menu.tsx`
//! ("View Pull Request on GitHub", "Checkout in New Worktree…" →
//! `AddWorktree` with the head branch and `<repository>-<number>`).
//!
//! `quick_view` is GHD `ui/pull-request-quick-view.tsx`
//! (`styles/ui/_pull-request-quick-view.scss`, `enablePullRequestQuickView`):
//! a 400 px card right of the foldout with the "Review requested" header and
//! "View on GitHub", the Open / Draft pill, the title with the `#N` + CI
//! badge, and the body as Markdown (max 500 px, scrolling), with a pointer at
//! the hovered row. Corvane adds the list item's "opened … by author" line
//! under the title.

use corvane_core::filter::fuzzy_score;
use corvane_core::{Dispatcher, Popup, PullRequest, parse_iso8601};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ci_status::ci_status;
use crate::context_menu::MenuItem;
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative;
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::ListRowA11y;
use crate::widgets::code_ref;

/// `RowHeight`
#[allow(non_snake_case)]
pub fn PR_ROW_HEIGHT() -> Pixels {
    zpx(47.)
}

/// `opened 3 days ago by octocat`
fn opened_by(pr: &PullRequest) -> String {
    let date = parse_iso8601(&pr.created_at)
        .map(|t| {
            if crate::format::prefer_absolute_dates() {
                crate::format::format_date(t)
            } else {
                relative(t)
            }
        })
        .unwrap_or_default();
    format!("opened {} by {}", date, pr.author)
}

/// `getSubtitle`: `#12 opened 3 days ago by octocat • Draft`.
pub fn subtitle(pr: &PullRequest) -> String {
    let text = format!("#{} {}", pr.number, opened_by(pr));
    if pr.draft {
        format!("{text} • Draft")
    } else {
        text
    }
}

/// The filter list matches the title or the subtitle.
pub fn matches_filter(pr: &PullRequest, query: &str) -> bool {
    query.is_empty()
        || fuzzy_score(query, &pr.title).is_some()
        || fuzzy_score(query, &subtitle(pr)).is_some()
}

/// A pull request's CI status, as `ci_status` draws it.
pub type CiSummary = Option<(
    corvane_core::CheckStatus,
    Option<corvane_core::CheckConclusion>,
)>;

/// `PullRequestListItem`; `repository_name` names the worktree that
/// "Checkout in New Worktree…" proposes.
pub fn pull_request_row(
    id: u64,
    pr: &PullRequest,
    selected: bool,
    status: CiSummary,
    repository_name: String,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let hover_bg = t.box_selected_active_background;
    let hover_text = t.box_selected_active_text;
    let icon_color = if pr.draft {
        t.pr_draft_icon
    } else {
        t.pr_open_icon
    };
    let pr_for_click = pr.clone();
    let pr_for_menu = pr.clone();
    div()
        .id(SharedString::from(format!("pull-request-{}", pr.number)))
        .a11y_row(format!("{}, {}", pr.title, subtitle(pr)), selected)
        .h(PR_ROW_HEIGHT())
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .px(SPACING())
        .cursor_pointer()
        .when(selected, |d| {
            d.bg(t.box_selected_background)
                .text_color(t.box_selected_text)
        })
        .when(!selected, move |d| {
            d.hover(move |s| s.bg(hover_bg).text_color(hover_text))
        })
        .on_click(move |_, _, cx| {
            Dispatcher::close_foldout(cx);
            Dispatcher::checkout_pull_request(id, pr_for_click.clone(), cx);
        })
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            // `generatePullRequestContextMenuItems`
            let pr = pr_for_menu.clone();
            let head = pr.head.ref_name.clone();
            let worktree_name = format!("{repository_name}-{}", pr.number);
            crate::native_menu::show_context_menu(
                vec![
                    MenuItem::new("View Pull Request on GitHub", move |_, cx| {
                        Dispatcher::open_pull_request(&pr, cx)
                    }),
                    MenuItem::new("Checkout in New Worktree…", move |_, cx| {
                        Dispatcher::close_foldout(cx);
                        Dispatcher::show_popup(
                            Popup::AddWorktree {
                                repo: id,
                                initial_branch_name: Some(head.clone()),
                                initial_worktree_name: Some(worktree_name.clone()),
                            },
                            cx,
                        )
                    }),
                ],
                ev.position,
                window,
                cx,
            );
        })
        .child(
            div()
                .flex_none()
                .self_start()
                .mt(zpx(2.))
                .ml(SPACING_HALF())
                .mr(SPACING())
                .pt(zpx(7.))
                .child(octicon(
                    if pr.draft {
                        Octicon::GitPullRequestDraft
                    } else {
                        Octicon::GitPullRequest
                    },
                    icon_color,
                )),
        )
        .child(
            // `.info`
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .mr(SPACING_HALF())
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(FONT_SIZE())
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(pr.title.clone()),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(FONT_SIZE_SM())
                        .font_weight(FontWeight::LIGHT)
                        .text_color(if selected {
                            t.box_selected_text
                        } else {
                            t.text_secondary
                        })
                        .child(subtitle(pr)),
                ),
        )
        .child(
            // `.ci-status-container`
            div()
                .flex_none()
                .min_w(zpx(16.))
                .mr(SPACING_HALF())
                .flex()
                .justify_center()
                .when_some(status, |d, (status, conclusion)| {
                    d.child(ci_status(status, conclusion))
                }),
        )
}

/// `maxQuickViewHeight`: 500 px body + 56 px header.
#[allow(non_snake_case)]
pub fn QUICK_VIEW_MAX_HEIGHT() -> Pixels {
    zpx(556.)
}
/// `.pull-request-quick-view-contents { min-width: 400px }`
#[allow(non_snake_case)]
fn QUICK_VIEW_WIDTH() -> Pixels {
    zpx(400.)
}

/// `calculatePosition`: the card's top relative to the foldout's top, given
/// the hovered row's top (both window-space) and the window height.
pub fn quick_view_top(
    row_top: Pixels,
    list_top: Pixels,
    window_height: Pixels,
    height: Pixels,
) -> Pixels {
    let clamp = |v: Pixels, min: Pixels, max: Pixels| v.min(max).max(min);
    let (min_top, max_top) = (zpx(0.), window_height - list_top - height);
    if window_height - row_top > height {
        return clamp(row_top - list_top, min_top, max_top);
    }
    if row_top - height > zpx(0.) {
        return clamp(
            row_top - list_top - height + PR_ROW_HEIGHT(),
            min_top,
            max_top,
        );
    }
    let middle = row_top + PR_ROW_HEIGHT() / 2. - height / 2.;
    clamp(middle, min_top, max_top)
}

/// `PullRequestQuickView`: the card (with its 10 px side margins) and the
/// pointer at `pointer_top` (card-relative centre of the hovered row).
pub fn quick_view(
    pr: &PullRequest,
    body: &[corvane_core::markdown::Block],
    status: CiSummary,
    pointer_top: Pixels,
    measured_height: std::rc::Rc<std::cell::Cell<Pixels>>,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let pill = if pr.draft {
        t.pr_draft_icon
    } else {
        t.pr_open_icon
    };
    let pr_for_view = pr.clone();
    let (border, background) = (t.box_border, t.background);
    div()
        .id("pull-request-quick-view")
        .relative()
        .occlude()
        .px(SPACING())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            // `.pull-request-quick-view-contents`
            div()
                .relative()
                .w(QUICK_VIEW_WIDTH())
                .flex()
                .flex_col()
                .bg(t.background)
                .text_color(t.text)
                .text_size(FONT_SIZE())
                .rounded(BORDER_RADIUS())
                .child(
                    canvas(
                        move |b, window, _| {
                            if measured_height.get() != b.size.height {
                                measured_height.set(b.size.height);
                                window.refresh();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
                .child(
                    // `.header`
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .p(SPACING_DOUBLE())
                        .pb(SPACING())
                        .border_b_1()
                        .border_color(t.box_border)
                        .child(octicon(Octicon::ListUnordered, t.text))
                        .child(
                            div()
                                .flex_1()
                                .pl(SPACING_DOUBLE())
                                .child("Review requested"),
                        )
                        .child(
                            crate::widgets::button("quick-view-on-github", "", cx)
                                .flex_none()
                                .gap(SPACING_HALF())
                                .role(Role::Link)
                                .aria_label("View on GitHub")
                                .child("View on GitHub")
                                .child(octicon(Octicon::LinkExternal, t.secondary_button_text))
                                .on_click(move |_, _, cx| {
                                    Dispatcher::open_pull_request(&pr_for_view, cx)
                                }),
                        ),
                )
                .child(
                    // `.pull-request`
                    div()
                        .id("quick-view-pull-request")
                        .max_h(zpx(500.))
                        .overflow_y_scroll()
                        .p(SPACING_DOUBLE())
                        .child(
                            // `.status`
                            div().flex().flex_row().child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .py(SPACING_HALF())
                                    .px(SPACING())
                                    .rounded(zpx(28.))
                                    .bg(pill)
                                    .text_color(gpui_kit::white())
                                    .text_size(FONT_SIZE_MD())
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(octicon(
                                        if pr.draft {
                                            Octicon::GitPullRequestDraft
                                        } else {
                                            Octicon::GitPullRequest
                                        },
                                        gpui_kit::white(),
                                    ))
                                    .child(div().ml(SPACING_HALF()).child(if pr.draft {
                                        "Draft"
                                    } else {
                                        "Open"
                                    })),
                            ),
                        )
                        .child(
                            // `.title`: h2 + `PullRequestBadge`
                            div()
                                .my(SPACING())
                                .flex()
                                .flex_col()
                                .gap(SPACING_HALF())
                                .child(
                                    div()
                                        .text_size(zpx(18.))
                                        .line_height(zpx(24.))
                                        .font_weight(FontWeight::BOLD)
                                        .child(pr.title.clone()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(SPACING_HALF())
                                        .child(
                                            div()
                                                .flex_none()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .h(zpx(22.))
                                                .px(SPACING_HALF())
                                                .rounded(BORDER_RADIUS())
                                                .border_1()
                                                .border_color(t.box_border)
                                                .text_size(FONT_SIZE_SM())
                                                .child(format!("#{}", pr.number))
                                                .when_some(status, |d, (status, conclusion)| {
                                                    d.child(
                                                        ci_status(status, conclusion)
                                                            .ml(SPACING_HALF()),
                                                    )
                                                }),
                                        )
                                        .child(
                                            div()
                                                .min_w_0()
                                                .truncate()
                                                .text_size(FONT_SIZE_SM())
                                                .text_color(t.text_secondary)
                                                .child(opened_by(pr)),
                                        ),
                                ),
                        )
                        .child(crate::markdown::markdown(
                            "quick-view-body",
                            body,
                            pr.base.repository.as_ref().map(|r| r.html_url.as_str()),
                            cx,
                        ))
                        .with_scrollbar(),
                ),
        )
        .child(
            // `.pull-request-pointer`: a left-pointing triangle outlined in
            // the border colour, 7 px above the row's centre
            canvas(
                |_, _, _| {},
                move |b, _, window, _| {
                    let o = b.origin;
                    let tri = |dx: f32, half: f32| {
                        let mut path = Path::new(point(o.x + zpx(dx), o.y + zpx(8.)));
                        path.line_to(point(o.x + zpx(dx + half), o.y + zpx(8. - half)));
                        path.line_to(point(o.x + zpx(dx + half), o.y + zpx(8. + half)));
                        path.line_to(point(o.x + zpx(dx), o.y + zpx(8.)));
                        path
                    };
                    window.paint_path(tri(2., 8.), border);
                    window.paint_path(tri(3.5, 7.), background);
                },
            )
            .absolute()
            .left(zpx(0.))
            .top(pointer_top - zpx(8.))
            .w(zpx(12.))
            .h(zpx(16.)),
        )
}

/// `NoPullRequests`
pub fn no_pull_requests(
    id: u64,
    repository_name: String,
    is_search: bool,
    loading: bool,
    on_default_branch: bool,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let title: &'static str = if is_search {
        "Sorry, I can't find that pull request!"
    } else if loading {
        "Hang tight"
    } else {
        "You're all set!"
    };
    let link = |id_str: &'static str, label: &'static str, cx: &App| {
        crate::widgets::link_button(id_str, label, cx)
    };
    let call_to_action: AnyElement = if loading {
        div()
            .child("Loading pull requests as fast as I can!")
            .into_any_element()
    } else if on_default_branch {
        crate::widgets::paragraph(vec![
            "Would you like to ".into(),
            crate::widgets::Inline::Element(
                link("no-prs-create-branch", "create a new branch", cx)
                    .on_click(move |_, _, cx| {
                        Dispatcher::close_foldout(cx);
                        Dispatcher::show_popup(
                            Popup::CreateBranch {
                                repo: id,
                                target_sha: None,
                                initial_name: String::new(),
                            },
                            cx,
                        )
                    })
                    .into_any_element(),
            ),
            " and get going on your next project?".into(),
        ])
        .justify_center()
        .into_any_element()
    } else {
        crate::widgets::paragraph(vec![
            "Would you like to ".into(),
            crate::widgets::Inline::Element(
                link("no-prs-create-pr", "create a pull request", cx)
                    .on_click(move |_, _, cx| {
                        Dispatcher::close_foldout(cx);
                        Dispatcher::create_pull_request(id, cx)
                    })
                    .into_any_element(),
            ),
            " from the current branch?".into(),
        ])
        .justify_center()
        .into_any_element()
    };
    div()
        .id("no-pull-requests")
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .text_center()
        .p(SPACING())
        .text_size(FONT_SIZE())
        .child(
            img("illustrations/empty-no-pull-requests.svg")
                .w(zpx(200.))
                .mb(SPACING()),
        )
        .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
        .when(!is_search && !loading, |d| {
            d.child(
                div()
                    .pb(SPACING())
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .justify_center()
                    .gap(zpx(3.))
                    .child("No open pull requests in")
                    .child(code_ref(repository_name, cx)),
            )
        })
        .child(
            div()
                .text_size(FONT_SIZE_SM())
                .text_color(t.text)
                .child(call_to_action),
        )
        .into_any_element()
}
