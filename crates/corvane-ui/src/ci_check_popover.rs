//! The check-run popover under the pull request badge - GHD
//! `ui/check-runs/{ci-check-run-popover,ci-check-run-list,ci-check-run-list-item,
//! ci-check-run-step-list-header,ci-check-run-actions-job-step-list,
//! ci-check-run-no-steps,ci-check-re-run-button}.tsx`
//! (`styles/ui/check-runs/*.scss`): 440 px wide, header with the
//! completeness indicator, title and summary, the Re-run button, then the
//! check runs grouped by workflow, expandable to their job steps.
//!
//! Deviation (flag `rerun-needs-push-access`): when the stored record of the
//! pull request's base repository says the user can only read it, the Re-run
//! button and the per-job re-run are hidden (GHD shows them and the re-run
//! request fails).
//!
//! Corvane addition (flag `319-ci-popover-pull-request-link`): the header's
//! summary line ends with an "Open #N on GitHub" link to the pull request.

use std::cell::Cell;
use std::rc::Rc;

use corvane_core::{
    AppState, CheckConclusion, CombinedRefCheck, Dispatcher, GitHubRepository, Popup, RefCheck,
    combined_status_summary, group_check_runs,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::widgets::IconButtonA11y;

use crate::ci_status::{ci_status, color_for, effective_conclusion, symbol_for_log_step};
use crate::context_menu::{MenuItem, mac_or};
use crate::icons::{Octicon, octicon, spin};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, c, primer};
use crate::widgets::button;

/// Re-runs the given checks (`failed_only` second).
pub(crate) type RerunChecks = Rc<dyn Fn(Vec<RefCheck>, bool, &mut App)>;
/// Re-runs one job.
pub(crate) type RerunJob = Rc<dyn Fn(&mut App)>;

/// `.ci-check-list-popover .popover-component { width: 440px }`
#[allow(non_snake_case)]
fn POPOVER_WIDTH() -> Pixels {
    zpx(440.)
}

pub struct CiCheckPopover {
    state: Entity<AppState>,
    /// The badge's window-space rectangle; the popover hangs below it.
    anchor: Rc<Cell<Bounds<Pixels>>>,
    /// `checkRunExpanded`
    expanded: Option<u64>,
    /// `hasUserToggledCheckRun`
    user_toggled: bool,
    /// The PR the expansion state belongs to; a new PR resets it.
    pr_number: Option<u64>,
}

struct Snapshot {
    repo: u64,
    github: GitHubRepository,
    pr_number: u64,
    git_ref: String,
    check: Option<CombinedRefCheck>,
    dotcom: bool,
    /// `rerun-needs-push-access`: the stored record of the base repository
    /// says the user can only read it, so nothing can be re-run.
    read_only: bool,
}

impl CiCheckPopover {
    pub fn new(
        state: Entity<AppState>,
        anchor: Rc<Cell<Bounds<Pixels>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            anchor,
            expanded: None,
            user_toggled: false,
            pr_number: None,
        }
    }

    fn snapshot(&self, cx: &App) -> Option<Snapshot> {
        let s = self.state.read(cx);
        let id = s.selected?;
        let pr = s.current_pull_request(id)?;
        let github = pr.base.repository.clone()?;
        let git_ref = pr.commit_ref();
        let read_only = s
            .flags
            .bool(corvane_core::flags::ids::RERUN_NEEDS_PUSH_ACCESS)
            && s.repository(id)
                .and_then(|r| r.github.as_ref())
                .filter(|gh| {
                    gh.endpoint == github.endpoint
                        && gh.owner.eq_ignore_ascii_case(&github.owner)
                        && gh.name.eq_ignore_ascii_case(&github.name)
                })
                .is_some_and(|gh| !gh.has_write_permission());
        Some(Snapshot {
            repo: id,
            pr_number: pr.number,
            check: s.commit_status(&github, &git_ref).cloned(),
            dotcom: github.endpoint == "https://api.github.com",
            read_only,
            github,
            git_ref,
        })
    }

    /// `setupStateAfterCheckRunPropChange`: the first failed check with job
    /// steps starts expanded.
    fn sync_expansion(&mut self, snap: &Snapshot) {
        if self.pr_number != Some(snap.pr_number) {
            self.pr_number = Some(snap.pr_number);
            self.expanded = None;
            self.user_toggled = false;
        }
        if self.user_toggled {
            return;
        }
        let checks = snap
            .check
            .as_ref()
            .map(|c| c.checks.as_slice())
            .unwrap_or(&[]);
        self.expanded = group_check_runs(checks).iter().find_map(|(_, group)| {
            group
                .iter()
                .find(|c| c.is_failure() && c.job_steps.is_some())
                .map(|c| c.id)
        });
    }

    fn toggle(&mut self, id: u64, cx: &mut Context<Self>) {
        self.expanded = if self.expanded == Some(id) {
            None
        } else {
            Some(id)
        };
        self.user_toggled = true;
        cx.notify();
    }

    fn rerun(snap: &Snapshot, checks: Vec<RefCheck>, failed_only: bool, cx: &mut App) {
        Dispatcher::show_popup(
            Popup::CICheckRunRerun {
                repo: snap.repo,
                github: snap.github.clone(),
                checks,
                git_ref: snap.git_ref.clone(),
                failed_only,
            },
            cx,
        );
    }

    /// `renderHeader`
    fn header(&self, snap: &Snapshot, checks: &[RefCheck], cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let loading = snap.check.is_none();
        let failing = |c: &RefCheck| c.conclusion.is_some_and(|c| c.is_failing());
        let some_pending_no_failures = !loading
            && checks.iter().any(|c| c.conclusion.is_none())
            && !checks.iter().any(failing);
        let successful_ish = |c: &RefCheck| {
            matches!(
                c.conclusion,
                Some(CheckConclusion::Success)
                    | Some(CheckConclusion::Neutral)
                    | Some(CheckConclusion::Skipped)
            )
        };
        let all_success = !loading
            && !some_pending_no_failures
            && !checks
                .iter()
                .any(|c| c.conclusion.is_some() && !successful_ish(c));
        let all_failure = !loading
            && !some_pending_no_failures
            && !checks.iter().any(|c| c.conclusion.is_none() || !failing(c));
        let (title, title_color): (&'static str, Hsla) = if loading {
            ("Checks Summary", t.text)
        } else if some_pending_no_failures {
            (
                "Some checks haven't completed yet",
                c(primer::YELLOW_700_DARKEN_10),
            )
        } else if all_failure {
            ("All checks have failed", c(primer::RED_500))
        } else if all_success {
            ("All checks have passed", t.text)
        } else {
            ("Some checks were not successful", c(primer::RED_500))
        };
        let summary = if checks.is_empty() {
            String::new()
        } else {
            let conclusions: Vec<Option<CheckConclusion>> = checks
                .iter()
                .map(|c| effective_conclusion(c.status, c.conclusion))
                .collect();
            combined_status_summary(&conclusions, "check")
        };
        // `renderCompletenessIndicator`: spinner, check, x or the donut
        let indicator: AnyElement = if loading {
            spin(
                octicon(Octicon::SyncClockwise, t.text_secondary).size(zpx(30.)),
                "ci-check-run-loading",
            )
        } else if all_success {
            octicon(Octicon::CheckCircleFill, c(primer::GREEN_500))
                .size(zpx(30.))
                .into_any_element()
        } else if all_failure {
            octicon(Octicon::XCircleFill, c(primer::RED_500))
                .size(zpx(30.))
                .into_any_element()
        } else {
            donut(checks)
        };
        let can_rerun_failed = snap.dotcom;
        let rerun_disabled = checks.is_empty() || loading;
        let snap_for_menu = Snapshot {
            repo: snap.repo,
            github: snap.github.clone(),
            pr_number: snap.pr_number,
            git_ref: snap.git_ref.clone(),
            check: None,
            dotcom: snap.dotcom,
            read_only: snap.read_only,
        };
        let checks_for_menu = checks.to_vec();
        let pr_link = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::CI_POPOVER_PULL_REQUEST_LINK);
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .p(SPACING())
            .bg(t.box_alt_background)
            .border_b_1()
            .border_color(t.box_border)
            .rounded_t(BORDER_RADIUS())
            .child(
                div()
                    .flex_none()
                    .mr(SPACING())
                    .my(SPACING_HALF())
                    .child(indicator),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(FONT_SIZE_MD())
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(title_color)
                            .child(title),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(SPACING_HALF())
                            .text_size(FONT_SIZE_SM())
                            .text_color(t.text_secondary)
                            .child(summary)
                            .when(pr_link, |d| {
                                let repo = snap.repo;
                                d.child(
                                    crate::widgets::link_button(
                                        "ci-open-pull-request",
                                        format!("Open #{} on GitHub", snap.pr_number),
                                        cx,
                                    )
                                    .on_click(
                                        move |_, _, cx| Dispatcher::show_pull_request(repo, cx),
                                    ),
                                )
                            }),
                    ),
            )
            .when(!snap.read_only, |d| {
                let snap = snap_for_menu;
                d.child(rerun_button(
                    "ci-rerun",
                    &checks_for_menu,
                    rerun_disabled,
                    can_rerun_failed,
                    Rc::new(move |checks, failed_only, cx| {
                        Self::rerun(&snap, checks, failed_only, cx)
                    }),
                    cx,
                ))
            })
            .into_any_element()
    }

    /// `CICheckRunListItem` + its expanded steps region.
    fn check_item(&self, snap: &Snapshot, check: &RefCheck, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let expanded = self.expanded == Some(check.id);
        let id = check.id;
        let external_url = check
            .html_url
            .clone()
            .unwrap_or_else(|| format!("{}/pull/{}", snap.github.html_url, snap.pr_number));
        let entity = cx.entity().downgrade();
        let row = check_run_row(check, false, expanded, cx).on_click(move |_, _, cx| {
            entity.update(cx, |this, cx| this.toggle(id, cx)).ok();
        });
        if !expanded {
            return row.into_any_element();
        }
        let rerun_check = check.clone();
        let snap_for_rerun = Snapshot {
            repo: snap.repo,
            github: snap.github.clone(),
            pr_number: snap.pr_number,
            git_ref: snap.git_ref.clone(),
            check: None,
            dotcom: snap.dotcom,
            read_only: snap.read_only,
        };
        // `.ci-steps-container`
        let steps_region = div()
            .p(SPACING())
            .bg(t.box_alt_background)
            .border_b_1()
            .border_color(t.box_border)
            .when(check.job_steps.is_none(), |d| d.h(zpx(150.)).p(zpx(0.)))
            .child(check_run_steps(
                check,
                external_url,
                (snap.dotcom && !snap.read_only).then(|| {
                    Rc::new(move |cx: &mut App| {
                        Self::rerun(&snap_for_rerun, vec![rerun_check.clone()], false, cx)
                    }) as RerunJob
                }),
                cx,
            ));
        div()
            .flex()
            .flex_col()
            .child(row)
            .child(steps_region)
            .into_any_element()
    }
}

/// `CICheckReRunButton`: "Re-run Checks", or "Re-run ▾" with a Failed / All
/// menu when failed checks can be re-run on their own. `on_rerun` gets the
/// checks and `failed_only`.
pub(crate) fn rerun_button(
    id: &'static str,
    checks: &[RefCheck],
    disabled: bool,
    can_rerun_failed: bool,
    on_rerun: RerunChecks,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let failed_exist = checks
        .iter()
        .any(|c| c.conclusion == Some(CheckConclusion::Failure));
    let menu = can_rerun_failed && failed_exist;
    let checks = checks.to_vec();
    button(id, "", cx)
        .flex_none()
        .gap(SPACING_HALF())
        .when(disabled, |d| d.opacity(0.6))
        .child(octicon(Octicon::SyncClockwise, t.secondary_button_text))
        .child(if menu {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(zpx(2.))
                .child("Re-run")
                .child(octicon(Octicon::TriangleDown, t.secondary_button_text))
                .into_any_element()
        } else {
            div().child("Re-run Checks").into_any_element()
        })
        .on_click(move |ev, window, cx| {
            if disabled {
                return;
            }
            if !menu {
                on_rerun(checks.clone(), false, cx);
                return;
            }
            let (failed, all) = (on_rerun.clone(), on_rerun.clone());
            let (failed_checks, all_checks) = (checks.clone(), checks.clone());
            crate::native_menu::show_context_menu(
                vec![
                    MenuItem::new(
                        mac_or("Re-run Failed Checks", "Re-run failed checks"),
                        move |_, cx| failed(failed_checks.clone(), true, cx),
                    ),
                    MenuItem::new(
                        mac_or("Re-run All Checks", "Re-run all checks"),
                        move |_, cx| all(all_checks.clone(), false, cx),
                    ),
                ],
                ev.position(),
                window,
                cx,
            );
        })
}

/// `CICheckRunListItem`: status symbol, name + description, and the
/// expansion chevron unless the list is `selectable` (then `active` means
/// selected).
pub(crate) fn check_run_row(
    check: &RefCheck,
    selectable: bool,
    active: bool,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let hover_bg = t.box_selected_background;
    div()
        .id(SharedString::from(format!("check-run-{}", check.id)))
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .border_b_1()
        .border_color(t.box_border)
        .bg(if selectable && active {
            t.box_selected_background
        } else {
            t.background
        })
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .child(
            div()
                .flex_none()
                .my(zpx(15.))
                .ml(SPACING())
                .child(ci_status(check.status, check.conclusion)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .mx(SPACING())
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_size(FONT_SIZE())
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(check.name.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .child(check.description.clone()),
                ),
        )
        .when(!selectable, |d| {
            d.child(div().flex_none().mr(SPACING_HALF()).child(octicon(
                if active {
                    Octicon::ChevronUp
                } else {
                    Octicon::ChevronDown
                },
                t.text_secondary,
            )))
        })
}

/// `.ci-check-run-list-group-header`
pub(crate) fn check_run_group_header(name: String, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .px(SPACING())
        .py(SPACING_HALF())
        .bg(t.box_alt_background)
        .border_b_1()
        .border_color(t.box_border)
        .truncate()
        .text_size(FONT_SIZE())
        .child(name)
}

/// A check's job steps (`CICheckRunStepListHeader` +
/// `CICheckRunActionsJobStepList`), or `CICheckRunNoStepItem` when it has
/// none. `on_rerun_job` is `None` where single jobs cannot be re-run.
pub(crate) fn check_run_steps(
    check: &RefCheck,
    external_url: String,
    on_rerun_job: Option<RerunJob>,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let view_url = external_url.clone();
    match &check.job_steps {
        Some(steps) => {
            let conclusions: Vec<Option<CheckConclusion>> = steps
                .iter()
                .map(|s| effective_conclusion(s.status, s.conclusion))
                .collect();
            let header_url = external_url.clone();
            div()
                .flex()
                .flex_col()
                .child(
                    // `CICheckRunStepListHeader`
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .border_b_1()
                        .border_color(t.box_border)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .mb(zpx(3.))
                                .text_size(FONT_SIZE())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.text_secondary)
                                .child(combined_status_summary(&conclusions, "step")),
                        )
                        .when_some(on_rerun_job, |d, rerun| {
                            d.child(
                                icon_button(
                                    "check-rerun-job",
                                    Octicon::SyncClockwise,
                                    format!("Re-run {}", check.name),
                                    cx,
                                )
                                .on_click(move |_, _, cx| rerun(cx)),
                            )
                        })
                        .child(
                            icon_button(
                                "check-view-external",
                                Octicon::LinkExternal,
                                format!("View {} on GitHub", check.name),
                                cx,
                            )
                            .on_click(move |_, _, cx| {
                                corvane_core::Dispatcher::open_url(&header_url, cx)
                            }),
                        ),
                )
                .children(steps.iter().map(|step| {
                    let step_url = format!("{}/#step:{}:1", external_url, step.number);
                    let duration = corvane_core::check_duration_ms(
                        step.started_at.as_deref(),
                        step.completed_at.as_deref(),
                    )
                    .map(corvane_core::format_precise_duration)
                    .unwrap_or_default();
                    let conclusion = effective_conclusion(step.status, step.conclusion);
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(
                            div()
                                .flex_none()
                                .py(SPACING_HALF())
                                .pr(SPACING())
                                .mt(zpx(2.))
                                .child(octicon(
                                    symbol_for_log_step(step.status, step.conclusion),
                                    color_for(conclusion),
                                )),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(FONT_SIZE())
                                .child(step.name.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .py(SPACING_HALF())
                                .px(SPACING())
                                .text_size(FONT_SIZE())
                                .text_color(t.text_secondary)
                                .child(duration),
                        )
                        .child(
                            icon_button(
                                SharedString::from(format!("step-{}-{}", check.id, step.number)),
                                Octicon::LinkExternal,
                                format!("View {} on GitHub", step.name),
                                cx,
                            )
                            .on_click(move |_, _, cx| {
                                corvane_core::Dispatcher::open_url(&step_url, cx)
                            }),
                        )
                }))
                .into_any_element()
        }
        None => div()
            .size_full()
            .flex()
            .flex_row()
            .items_center()
            .p(SPACING_DOUBLE())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .text_size(FONT_SIZE())
                    .child("There are no steps to display for this check.")
                    .child(
                        button("check-no-steps-view", "", cx)
                            .mt(SPACING())
                            .gap(SPACING_HALF())
                            .child("View check details")
                            .child(octicon(Octicon::LinkExternal, t.secondary_button_text))
                            .on_click(move |_, _, cx| {
                                corvane_core::Dispatcher::open_url(&view_url, cx)
                            }),
                    ),
            )
            .child(
                // `ci-check-run-no-steps`: a plain <img>, no dark filter
                img("illustrations/paper-stack.svg")
                    .flex_1()
                    .ml(SPACING_DOUBLE())
                    .h(zpx(120.)),
            )
            .into_any_element(),
    }
}

/// The small transparent square buttons of the step header / step rows.
fn icon_button(
    id: impl Into<ElementId>,
    icon: Octicon,
    tooltip: String,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.ghd();
    let hover_bg = t.box_selected_background;
    let hover_border = t.secondary_button_hover_border;
    div()
        .id(id)
        .flex_none()
        .p(SPACING_HALF())
        .mb(zpx(2.))
        .rounded(BORDER_RADIUS())
        .border_1()
        .border_color(t.box_alt_background)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).border_color(hover_border))
        .icon_button_label(tooltip)
        .child(octicon(icon, t.text_secondary))
}

/// `Donut`: a 30 px ring split by the checks' states (drawn as arcs).
fn donut(checks: &[RefCheck]) -> AnyElement {
    let total = checks.len().max(1) as f32;
    let mut segments: Vec<(Hsla, f32)> = Vec::new();
    for check in checks {
        let conclusion = effective_conclusion(check.status, check.conclusion);
        let color = if conclusion.is_none() {
            c(primer::YELLOW_700_DARKEN_10)
        } else {
            color_for(conclusion)
        };
        match segments.iter_mut().find(|(col, _)| *col == color) {
            Some((_, n)) => *n += 1.,
            None => segments.push((color, 1.)),
        }
    }
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let center = bounds.center();
            let radius = bounds.size.width.min(bounds.size.height) / 2.;
            let inner = radius * 0.62;
            let mut start = -std::f32::consts::FRAC_PI_2;
            for (color, n) in &segments {
                let sweep = n / total * std::f32::consts::TAU;
                let steps = ((sweep / 0.15).ceil() as usize).max(2);
                let mut path = Path::new(point(
                    center.x + radius * start.cos(),
                    center.y + radius * start.sin(),
                ));
                for i in 1..=steps {
                    let a = start + sweep * i as f32 / steps as f32;
                    path.line_to(point(
                        center.x + radius * a.cos(),
                        center.y + radius * a.sin(),
                    ));
                }
                for i in (0..=steps).rev() {
                    let a = start + sweep * i as f32 / steps as f32;
                    path.line_to(point(
                        center.x + inner * a.cos(),
                        center.y + inner * a.sin(),
                    ));
                }
                window.paint_path(path, *color);
                start += sweep;
            }
        },
    )
    .size(zpx(30.))
    .into_any_element()
}

impl Render for CiCheckPopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let Some(snap) = self.snapshot(cx) else {
            return div().into_any_element();
        };
        self.sync_expansion(&snap);
        let checks: Vec<RefCheck> = snap
            .check
            .as_ref()
            .map(|c| c.checks.clone())
            .unwrap_or_default();
        let anchor = self.anchor.get();
        let viewport = window.viewport_size();
        // `PopoverAnchorPosition.Bottom`: centred under the badge
        let x = (anchor.origin.x + anchor.size.width / 2. - POPOVER_WIDTH() / 2.)
            .max(zpx(8.))
            .min(viewport.width - POPOVER_WIDTH() - zpx(8.));
        let y = anchor.origin.y + anchor.size.height + zpx(8.);
        let max_list_height = viewport.height * 0.7;
        let list: AnyElement = if snap.check.is_none() {
            // `renderCheckRunLoadings`
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .p(SPACING())
                .pb(SPACING_DOUBLE())
                .child(
                    crate::widgets::blankslate_image("empty-no-pull-requests.svg", cx).w(zpx(240.)),
                )
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Stand By"))
                .child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .child("Check runs incoming!"),
                )
                .into_any_element()
        } else {
            let groups = group_check_runs(&checks);
            let single_other = groups.len() == 1 && groups[0].0 == "Other";
            div()
                .id("ci-check-run-list")
                .max_h(max_list_height)
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .children(groups.into_iter().map(|(name, items)| {
                    div()
                        .flex()
                        .flex_col()
                        .when(!single_other, |d| d.child(check_run_group_header(name, cx)))
                        .children(items.iter().map(|check| self.check_item(&snap, check, cx)))
                }))
                .with_scrollbar()
                .into_any_element()
        };
        deferred(
            anchored().position(point(zpx(0.), zpx(0.))).child(
                div()
                    .id("ci-check-popover-layer")
                    .relative()
                    .w(viewport.width)
                    .h(viewport.height)
                    .child(
                        // click outside closes (`onClickOutside`)
                        div()
                            .id("ci-check-popover-overlay")
                            .absolute()
                            .inset_0()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                Dispatcher::set_show_ci_status_popover(false, cx)
                            }),
                    )
                    .child(
                        div()
                            .id("ci-check-popover")
                            .absolute()
                            .left(x)
                            .top(y)
                            .w(POPOVER_WIDTH())
                            .flex()
                            .flex_col()
                            .bg(t.box_background)
                            .text_color(t.text)
                            .text_size(FONT_SIZE())
                            .border_1()
                            .border_color(t.box_border)
                            .rounded(BORDER_RADIUS())
                            .shadow_lg()
                            .overflow_hidden()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(self.header(&snap, &checks, cx))
                            .child(list),
                    ),
            ),
        )
        .with_priority(30)
        .into_any_element()
    }
}
