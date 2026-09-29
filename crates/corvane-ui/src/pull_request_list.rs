//! Pull Requests tab of the branch foldout - GHD
//! `ui/branches/{pull-request-list,pull-request-list-item,no-pull-requests}.tsx`
//! (`styles/ui/_branches.scss` `.pull-request-list`, `.no-pull-requests`):
//! 47 px rows with the PR icon, title, "#N opened … by author" subtitle and
//! the CI status, plus the blank slate. The rows live in the
//! `BranchFoldout`; this module renders them.

use corvane_core::filter::fuzzy_score;
use corvane_core::{Dispatcher, Popup, PullRequest, parse_iso8601};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ci_status::ci_status;
use crate::context_menu::MenuItem;
use crate::icons::{Octicon, octicon};
use crate::relative_time::relative;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::code_ref;

/// `RowHeight`
pub const PR_ROW_HEIGHT: Pixels = px(47.);

/// `getSubtitle`: `#12 opened 3 days ago by octocat • Draft`.
pub fn subtitle(pr: &PullRequest) -> String {
    let date = parse_iso8601(&pr.created_at)
        .map(|t| {
            if crate::format::prefer_absolute_dates() {
                crate::format::format_date(t)
            } else {
                relative(t)
            }
        })
        .unwrap_or_default();
    let text = format!("#{} opened {} by {}", pr.number, date, pr.author);
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

/// `PullRequestListItem`
pub fn pull_request_row(
    id: u64,
    pr: &PullRequest,
    selected: bool,
    status: Option<(
        corvane_core::CheckStatus,
        Option<corvane_core::CheckConclusion>,
    )>,
    cx: &App,
) -> AnyElement {
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
        .h(PR_ROW_HEIGHT)
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .px(SPACING)
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
            crate::native_menu::show_context_menu(
                vec![MenuItem::new(
                    "View Pull Request on GitHub",
                    move |_, cx| Dispatcher::open_pull_request(&pr, cx),
                )],
                ev.position,
                window,
                cx,
            );
        })
        .child(
            div()
                .flex_none()
                .self_start()
                .mt(px(2.))
                .ml(SPACING_HALF)
                .mr(SPACING)
                .pt(px(7.))
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
                .mr(SPACING_HALF)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(FONT_SIZE)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(pr.title.clone()),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(FONT_SIZE_SM)
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
                .min_w(px(16.))
                .mr(SPACING_HALF)
                .flex()
                .justify_center()
                .when_some(status, |d, (status, conclusion)| {
                    d.child(ci_status(status, conclusion))
                }),
        )
        .into_any_element()
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
        .p(SPACING)
        .text_size(FONT_SIZE)
        .child(
            img("illustrations/empty-no-pull-requests.svg")
                .w(px(200.))
                .mb(SPACING),
        )
        .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
        .when(!is_search && !loading, |d| {
            d.child(
                div()
                    .pb(SPACING)
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .justify_center()
                    .gap(px(3.))
                    .child("No open pull requests in")
                    .child(code_ref(repository_name, cx)),
            )
        })
        .child(
            div()
                .text_size(FONT_SIZE_SM)
                .text_color(t.text)
                .child(call_to_action),
        )
        .into_any_element()
}
