//! Pull request notification dialogs - GHD
//! `ui/notifications/pull-request-review.tsx`, `pull-request-comment.tsx`,
//! `pull-request-comment-like.tsx`, `pull-request-review-helpers.ts` and
//! `pull-request-checks-failed.tsx` (`styles/ui/dialogs/_pull-request-comment-like.scss`,
//! `styles/ui/_pull-request-checks-failed.scss`).
//!
//! Review / comment: the PR icon and "title #n" as the header, a timeline
//! item (40 px avatar, the review-state bubble, "login verb your pull request
//! <time>") joined by dashed timeline lines to the Markdown body in a bubble,
//! "Open in Browser" left of the buttons. The OK button switches to the
//! repository and pull request unless the review is an approval or nothing
//! needs switching (then it is a lone "Ok").
//!
//! Checks failed: "N checks failed in your pull request" with the Re-run
//! button in the header, the check list (40 %) next to the selected check's
//! job steps (60 %), the question and "Switch to Pull Request" in the footer.
//!
//! Opened by a notification click (`corvane_core::notifications`); GHD's
//! Alive events are not ported, so only Test Notifications posts them
//! (`CORVANE_POPUP=pr-review|pr-comment|pr-checks-failed` shows them with
//! sample data).
//!
//! Deviations: "Switch to Pull Request" closes the dialog and then
//! switches, instead of spinning in the header until the checkout finished.
//! The checks arrive with the job steps the commit status store already
//! fetched, so there is no "Stand By" loading pane. Re-running replaces the
//! dialog with the re-run dialog (Corvane has one popup at a time).

use std::rc::Rc;

use corvane_core::{Dispatcher, GitHubRepository, Popup, PullRequest, RefCheck, group_check_runs};
use corvane_github::api::{
    ApiIdentity, ApiIssueComment, ApiPullRequestReview, ApiPullRequestReviewState,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ci_check_popover::{
    RerunChecks, RerunJob, check_run_group_header, check_run_row, check_run_steps, rerun_button,
};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme};
use crate::widgets::{IconButtonA11y, button, link_button, primary_button};

/// `#pull-request-review, #pull-request-comment { min-width: 500px }`
#[allow(non_snake_case)]
fn COMMENT_LIKE_MIN_WIDTH() -> Pixels {
    zpx(500.)
}
/// `--avatar-size: 40px`
#[allow(non_snake_case)]
fn AVATAR() -> Pixels {
    zpx(40.)
}
/// The timeline's x: avatar + `margin: 0 var(--spacing)` + half the 30 px icon.
#[allow(non_snake_case)]
fn TIMELINE_X() -> Pixels {
    zpx(64.)
}

/// `getVerbForPullRequestReview`
fn review_verb(state: ApiPullRequestReviewState) -> &'static str {
    match state {
        ApiPullRequestReviewState::Approved => "approved",
        ApiPullRequestReviewState::ChangesRequested => "requested changes on",
        _ => "reviewed",
    }
}

/// `getPullRequestReviewStateIcon`: symbol, foreground, background.
fn review_icon(state: ApiPullRequestReviewState, t: &GhdTheme) -> (Octicon, Hsla, Hsla) {
    match state {
        ApiPullRequestReviewState::Approved => (
            Octicon::Check,
            t.pr_approved_icon,
            t.pr_approved_icon_background,
        ),
        ApiPullRequestReviewState::ChangesRequested => (
            Octicon::FileDiff,
            t.pr_changes_requested_icon,
            t.pr_changes_requested_icon_background,
        ),
        _ => (
            Octicon::Eye,
            t.pr_commented_icon,
            t.pr_commented_icon_background,
        ),
    }
}

/// The OK button's title (`renderFooterContent`).
fn switch_title(
    should_change_repository: bool,
    should_checkout_branch: bool,
) -> Option<&'static str> {
    if should_change_repository {
        Some("Switch to Repository and Pull Request")
    } else if should_checkout_branch {
        Some("Switch to Pull Request")
    } else {
        None
    }
}

/// A notification dialog frame: auto-height header (`height: unset`) with a
/// title element, optional accessory and the close button; content; a
/// bordered footer. The backdrop does not dismiss (`backdropDismissable={false}`).
#[allow(clippy::too_many_arguments)]
fn frame(
    id: &'static str,
    plain_title: SharedString,
    title: AnyElement,
    accessory: Option<AnyElement>,
    (min_w, max_w): (Pixels, Pixels),
    content: AnyElement,
    footer: AnyElement,
    window: &Window,
    cx: &App,
) -> impl IntoElement + use<> {
    let t = cx.ghd();
    let viewport = crate::theme::page_size(window);
    let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
    deferred(
        anchored().position(point(zpx(0.), zpx(0.))).child(
            div()
                .id(id)
                // modal: nothing underneath takes hover, clicks or wheel
                .occlude()
                .w(viewport.width)
                .h(viewport.height)
                .flex()
                .items_center()
                .justify_center()
                .bg(t.dialog_backdrop)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("dialog-box")
                        .role(Role::Dialog)
                        .aria_label(plain_title.clone())
                        .child(crate::dialog::window_title(plain_title))
                        .min_w(min_w)
                        .max_w(max_w)
                        .flex()
                        .flex_col()
                        .rounded(BORDER_RADIUS())
                        .bg(t.background)
                        .text_color(t.text)
                        .text_size(FONT_SIZE())
                        .border_1()
                        .border_color(t.box_border)
                        .shadow(vec![BoxShadow {
                            color: t.shadow,
                            offset: point(zpx(0.), zpx(2.)),
                            blur_radius: css_blur(7.),
                            spread_radius: zpx(0.),
                            inset: false,
                        }])
                        .child(
                            // `.dialog-header`
                            div()
                                .flex_none()
                                .flex()
                                .flex_row()
                                .items_center()
                                .p(SPACING_DOUBLE())
                                .border_b_1()
                                .border_color(t.box_border)
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_size(FONT_SIZE_MD())
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                                .children(accessory)
                                .child(
                                    div()
                                        .id("dialog-close")
                                        .flex_none()
                                        .icon_button_label("Close")
                                        .size(zpx(16.))
                                        .cursor_pointer()
                                        .on_click(move |_, window, cx| close(window, cx))
                                        .child(octicon(Octicon::X, t.text_secondary)),
                                ),
                        )
                        .child(content)
                        .child(
                            // `.dialog-footer`
                            div()
                                .flex_none()
                                .p(SPACING_DOUBLE())
                                .border_t_1()
                                .border_color(t.box_border)
                                .child(footer),
                        ),
                ),
        ),
    )
    .with_priority(20)
}

/// `OkCancelButtonGroup` (macOS order: Cancel, then OK on the right).
fn ok_cancel(
    ok: SharedString,
    cancel: Option<SharedString>,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .gap(SPACING_HALF())
        .children(cancel.map(|label| {
            button("notification-cancel", label, cx)
                .min_w(zpx(120.))
                .on_click(|_, _, cx| Dispatcher::close_popup(cx))
        }))
        .child(
            primary_button("notification-ok", ok, false, cx)
                .min_w(zpx(120.))
                .on_click(move |_, window, cx| on_ok(window, cx)),
        )
}

/// A dashed `.timeline-line` (1 × 24 px): `top` fades in
/// (`stroke-dasharray: 1 1 2 1 3 1 15`), `bottom` fades out.
fn timeline_line(top: bool, color: Hsla) -> impl IntoElement {
    const TOP: [(f32, bool); 7] = [
        (1., true),
        (1., false),
        (2., true),
        (1., false),
        (3., true),
        (1., false),
        (15., true),
    ];
    let pattern: Vec<(f32, bool)> = if top {
        TOP.to_vec()
    } else {
        TOP.iter().rev().copied().collect()
    };
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let mut y = bounds.origin.y;
            for (len, on) in &pattern {
                if *on {
                    window.paint_quad(fill(
                        Bounds::new(point(bounds.origin.x, y), size(zpx(1.), zpx(*len))),
                        color,
                    ));
                }
                y += zpx(*len);
            }
        },
    )
    .w(zpx(1.))
    .h(zpx(24.))
}

/// The bubble's upward arrow (`.comment-bubble::before/::after`): a border
/// triangle with a background triangle one pixel lower on top.
fn bubble_arrow(border: Hsla, background: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let o = bounds.origin;
            let tri = |dy: f32| {
                let mut path = Path::new(point(o.x + zpx(8.), o.y + zpx(dy)));
                path.line_to(point(o.x + zpx(16.), o.y + zpx(8. + dy)));
                path.line_to(point(o.x, o.y + zpx(8. + dy)));
                path.line_to(point(o.x + zpx(8.), o.y + zpx(dy)));
                path
            };
            window.paint_path(tri(0.), border);
            window.paint_path(tri(1.), background);
        },
    )
    .absolute()
    .left(zpx(6.))
    .top(zpx(-7.))
    .w(zpx(16.))
    .h(zpx(9.))
}

/// The pull request as data for `PullRequestCommentLike`.
struct CommentLike {
    id: &'static str,
    repo: u64,
    pull_request: PullRequest,
    user: ApiIdentity,
    verb: &'static str,
    icon: (Octicon, Hsla, Hsla),
    event_date: Option<std::time::SystemTime>,
    external_url: String,
    body: Vec<corvane_core::markdown::Block>,
    ok: Option<&'static str>,
}

impl CommentLike {
    fn render(&self, window: &Window, cx: &App) -> impl IntoElement + use<> {
        let t = cx.ghd();
        let pr = &self.pull_request;
        let title = format!("{} #{}", pr.title, pr.number);
        let header =
            div()
                .flex()
                .flex_row()
                .items_center()
                .child(
                    octicon(
                        if pr.draft {
                            Octicon::GitPullRequestDraft
                        } else {
                            Octicon::GitPullRequest
                        },
                        if pr.draft {
                            t.pr_draft_icon
                        } else {
                            t.pr_open_icon
                        },
                    )
                    .flex_none()
                    .mr(SPACING_HALF()),
                )
                .child(div().flex_1().min_w_0().child(
                    StyledText::new(title.clone()).with_highlights([(
                        pr.title.len() + 1..title.len(),
                        HighlightStyle {
                            font_weight: Some(FontWeight::NORMAL),
                            color: Some(t.text_secondary),
                            ..Default::default()
                        },
                    )]),
                ))
                .into_any_element();

        let with_comment = !self.body.is_empty();
        let line = t.pr_timeline_line;
        let avatar = self
            .user
            .avatar_url
            .as_deref()
            .and_then(|url| crate::widgets::avatar_lookup_url(url, cx));
        let (symbol, fg, bg) = self.icon;
        let relative = self
            .event_date
            .map(crate::relative_time::relative)
            .unwrap_or_default();
        // `renderTimelineItem`: avatar, review icon, "login verb your pull request <time>"
        let item = div()
            .relative()
            .mt(zpx(6.))
            .flex()
            .flex_row()
            .items_center()
            .child(crate::widgets::avatar_image(avatar, AVATAR(), cx))
            .child(
                div()
                    .flex_none()
                    .size(zpx(30.))
                    .mx(SPACING())
                    .rounded_full()
                    .bg(bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(octicon(symbol, fg)),
            )
            .child(div().flex_1().min_w_0().child(summary(
                &self.user,
                self.verb,
                &relative,
                &self.external_url,
                cx,
            )));
        let mut timeline = div()
            .relative()
            .flex()
            .flex_col()
            // the top line runs from above the item into the review icon
            .child(
                div()
                    .absolute()
                    .left(TIMELINE_X())
                    .top(zpx(-9.))
                    .child(timeline_line(true, line)),
            )
            .child(item);
        if with_comment {
            // `.timeline-item.with-comment::after`: into the bubble's arrow
            timeline = timeline
                .child(
                    div()
                        .absolute()
                        .left(TIMELINE_X())
                        .top(zpx(42.))
                        .w(zpx(1.))
                        .h(zpx(12.))
                        .bg(line),
                )
                .child(
                    // `.comment-bubble-container`
                    div()
                        .relative()
                        .mt(zpx(15.))
                        .ml(SPACING() + AVATAR())
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .relative()
                                .border_1()
                                .border_color(line)
                                .rounded(zpx(6.))
                                .p(SPACING())
                                .child(bubble_arrow(line, t.background))
                                .child(div().pl(zpx(15.)).child(crate::markdown::markdown(
                                    self.id,
                                    &self.body,
                                    pr.base.repository.as_ref().map(|r| r.html_url.as_str()),
                                    cx,
                                ))),
                        )
                        .child(
                            div()
                                .ml(zpx(14.))
                                .mb(zpx(-14.))
                                .child(timeline_line(false, line)),
                        ),
                );
        } else {
            timeline = timeline.child(
                div()
                    .absolute()
                    .left(TIMELINE_X())
                    .top(zpx(42.))
                    .child(timeline_line(false, line)),
            );
        }
        let content = div()
            .id(SharedString::from(format!("{}-content", self.id)))
            .max_h(zpx(300.))
            .overflow_y_scroll()
            .p(SPACING_DOUBLE())
            .line_height(zpx(18.))
            .child(timeline)
            .with_scrollbar()
            .into_any_element();

        let (repo, pr_for_ok, ok) = (self.repo, pr.clone(), self.ok);
        let external = self.external_url.clone();
        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .child(
                // `.footer-links`
                div().flex_1().flex().items_center().child(
                    link_button(
                        SharedString::from(format!("{}-open-in-browser", self.id)),
                        "Open in Browser",
                        cx,
                    )
                    .on_click(move |_, _, cx| Dispatcher::open_url(&external, cx)),
                ),
            )
            .child(ok_cancel(
                ok.unwrap_or("Ok").into(),
                ok.map(|_| "Dismiss".into()),
                move |_, cx| match ok {
                    Some(_) => Dispatcher::switch_to_pull_request(repo, pr_for_ok.clone(), cx),
                    None => Dispatcher::close_popup(cx),
                },
                cx,
            ))
            .into_any_element();
        frame(
            self.id,
            title.into(),
            header,
            None,
            (COMMENT_LIKE_MIN_WIDTH(), zpx(600.)),
            content,
            footer,
            window,
            cx,
        )
    }
}

/// "**login** verb your pull request <time>", login and time as links.
fn summary(
    user: &ApiIdentity,
    verb: &str,
    relative: &str,
    external_url: &str,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let underline =
        corvane_core::AppState::try_global(cx).is_some_and(|s| s.read(cx).settings.underline_links);
    let text = format!("{} {verb} your pull request {relative}", user.login);
    let login = 0..user.login.len();
    let date = text.len() - relative.len()..text.len();
    let link = |bold: bool| HighlightStyle {
        color: Some(t.link),
        font_weight: bold.then_some(FontWeight::BOLD),
        underline: underline.then_some(UnderlineStyle {
            thickness: zpx(1.),
            color: None,
            wavy: false,
        }),
        ..Default::default()
    };
    let urls = [
        user.html_url.clone().unwrap_or_default(),
        external_url.to_string(),
    ];
    InteractiveText::new(
        "notification-summary",
        StyledText::new(text)
            .with_highlights([(login.clone(), link(true)), (date.clone(), link(false))]),
    )
    .on_click(vec![login, date], move |ix, _, cx| {
        if let Some(url) = urls.get(ix).filter(|u| !u.is_empty()) {
            Dispatcher::open_url(url, cx);
        }
    })
    .into_any_element()
}

/// `PullRequestReview`
pub struct PullRequestReviewDialog {
    inner: CommentLike,
}

impl PullRequestReviewDialog {
    pub fn new(
        repo: u64,
        pull_request: PullRequest,
        review: ApiPullRequestReview,
        should_checkout_branch: bool,
        should_change_repository: bool,
        cx: &mut App,
    ) -> Self {
        if let Some(url) = &review.user.avatar_url {
            Dispatcher::request_avatar_url(url, cx);
        }
        // only non-approved reviews offer to switch
        let ok = (review.state != ApiPullRequestReviewState::Approved)
            .then(|| switch_title(should_change_repository, should_checkout_branch))
            .flatten();
        let icon = review_icon(review.state, cx.ghd());
        Self {
            inner: CommentLike {
                id: "pull-request-review",
                repo,
                verb: review_verb(review.state),
                icon,
                event_date: corvane_core::parse_iso8601(&review.submitted_at),
                external_url: review.html_url.clone(),
                body: corvane_core::markdown::parse(&review.body),
                user: review.user,
                pull_request,
                ok,
            },
        }
    }
}

impl Render for PullRequestReviewDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.inner.render(window, cx)
    }
}

/// `PullRequestComment`
pub struct PullRequestCommentDialog {
    inner: CommentLike,
}

impl PullRequestCommentDialog {
    pub fn new(
        repo: u64,
        pull_request: PullRequest,
        comment: ApiIssueComment,
        should_checkout_branch: bool,
        should_change_repository: bool,
        cx: &mut App,
    ) -> Self {
        if let Some(url) = &comment.user.avatar_url {
            Dispatcher::request_avatar_url(url, cx);
        }
        let icon = review_icon(ApiPullRequestReviewState::Commented, cx.ghd());
        Self {
            inner: CommentLike {
                id: "pull-request-comment",
                repo,
                verb: "commented on",
                icon,
                event_date: corvane_core::parse_iso8601(&comment.created_at),
                external_url: comment.html_url.clone(),
                body: corvane_core::markdown::parse(&comment.body),
                user: comment.user,
                pull_request,
                ok: switch_title(should_change_repository, should_checkout_branch),
            },
        }
    }
}

impl Render for PullRequestCommentDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.inner.render(window, cx)
    }
}

/// `PullRequestChecksFailed`
pub struct PullRequestChecksFailedDialog {
    repo: u64,
    pull_request: PullRequest,
    checks: Vec<RefCheck>,
    selected: Option<u64>,
    should_change_repository: bool,
}

impl PullRequestChecksFailedDialog {
    pub fn new(
        repo: u64,
        pull_request: PullRequest,
        checks: Vec<RefCheck>,
        should_change_repository: bool,
    ) -> Self {
        let selected = checks
            .iter()
            .find(|c| c.is_failure())
            .or(checks.first())
            .map(|c| c.id);
        Self {
            repo,
            pull_request,
            checks,
            selected,
            should_change_repository,
        }
    }

    fn github(&self) -> Option<GitHubRepository> {
        self.pull_request.base.repository.clone()
    }

    fn rerun(repo: u64, github: GitHubRepository, git_ref: String) -> RerunChecks {
        Rc::new(move |checks, failed_only, cx| {
            Dispatcher::show_popup(
                Popup::CICheckRunRerun {
                    repo,
                    github: github.clone(),
                    checks,
                    git_ref: git_ref.clone(),
                    failed_only,
                },
                cx,
            )
        })
    }
}

impl Render for PullRequestChecksFailedDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let pr = &self.pull_request;
        let failed = self.checks.iter().filter(|c| c.is_failure()).count();
        let plural = if failed > 1 { "checks" } else { "check" };
        let headline = format!("{failed} {plural} failed in your pull request");
        let pr_title = format!("{} #{}", pr.title, pr.number);
        let github = self.github();
        let dotcom = github
            .as_ref()
            .is_some_and(|g| g.endpoint == "https://api.github.com");
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .child(
                octicon(Octicon::XCircleFill, t.status_error)
                    .size(zpx(20.))
                    .flex_none()
                    .mr(SPACING()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(headline.clone())
                    .child(
                        div()
                            .font_weight(FontWeight::NORMAL)
                            .text_size(FONT_SIZE())
                            .child(StyledText::new(pr_title.clone()).with_highlights([(
                                pr.title.len() + 1..pr_title.len(),
                                HighlightStyle {
                                    color: Some(t.text_secondary),
                                    ..Default::default()
                                },
                            )])),
                    ),
            )
            .into_any_element();
        let accessory = github.clone().map(|github| {
            div()
                .flex_none()
                .mx(SPACING())
                .text_size(FONT_SIZE())
                .font_weight(FontWeight::NORMAL)
                .child(rerun_button(
                    "checks-failed-rerun",
                    &self.checks,
                    self.checks.is_empty(),
                    dotcom,
                    Self::rerun(self.repo, github, pr.commit_ref()),
                    cx,
                ))
                .into_any_element()
        });

        // `CICheckRunList selectable`
        let groups = group_check_runs(&self.checks);
        let single_other = groups.len() == 1 && groups[0].0 == "Other";
        let entity = cx.entity().downgrade();
        let list = div()
            .id("checks-failed-list")
            .w(relative(0.4))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(t.box_border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .children(groups.into_iter().map(|(name, items)| {
                        div()
                            .flex()
                            .flex_col()
                            .when(!single_other, |d| {
                                d.child(check_run_group_header(name, cx).pl(SPACING_DOUBLE()))
                            })
                            .children(items.iter().map(|check| {
                                let id = check.id;
                                let entity = entity.clone();
                                check_run_row(check, true, self.selected == Some(id), cx)
                                    .pl(SPACING())
                                    .on_click(move |_, _, cx| {
                                        entity
                                            .update(cx, |this, cx| {
                                                this.selected = Some(id);
                                                cx.notify();
                                            })
                                            .ok();
                                    })
                            }))
                    })),
            )
            .with_scrollbar();
        let selected = self
            .selected
            .and_then(|id| self.checks.iter().find(|c| c.id == id));
        let steps = div()
            .id("checks-failed-steps")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(SPACING())
            .bg(t.box_alt_background)
            .children(selected.map(|check| {
                let external = check.html_url.clone().unwrap_or_else(|| {
                    format!(
                        "{}/pull/{}",
                        github
                            .as_ref()
                            .map(|g| g.html_url.as_str())
                            .unwrap_or_default(),
                        pr.number
                    )
                });
                let rerun_job = github.clone().filter(|_| dotcom).map(|github| {
                    let rerun = Self::rerun(self.repo, github, pr.commit_ref());
                    let check = check.clone();
                    Rc::new(move |cx: &mut App| rerun(vec![check.clone()], false, cx)) as RerunJob
                });
                check_run_steps(check, external, rerun_job, cx)
            }))
            .with_scrollbar();
        let content = div()
            .h(zpx(300.))
            .w_full()
            .flex()
            .flex_row()
            .overflow_hidden()
            .child(list)
            .child(steps)
            .into_any_element();

        let (repo, pr_for_ok) = (self.repo, pr.clone());
        let them = if failed > 1 { "them" } else { "it" };
        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            // `.footer-question`
            .child(div().flex_1().min_w_0().pr(SPACING()).child(format!(
                "Do you want to switch to that Pull Request now and start fixing {them}?"
            )))
            .child(ok_cancel(
                if self.should_change_repository {
                    "Switch to Repository and Pull Request"
                } else {
                    "Switch to Pull Request"
                }
                .into(),
                Some("Dismiss".into()),
                move |_, cx| Dispatcher::switch_to_pull_request(repo, pr_for_ok.clone(), cx),
                cx,
            ))
            .into_any_element();
        frame(
            "pull-request-checks-failed",
            headline.into(),
            header,
            accessory,
            (zpx(600.), zpx(600.)),
            content,
            footer,
            window,
            cx,
        )
    }
}
