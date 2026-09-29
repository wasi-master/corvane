//! GHD `ui/check-runs/ci-check-run-rerun-dialog.tsx`
//! (`styles/ui/dialogs/_ci-check-run-rerun.scss`): which checks can be
//! re-run, and the request itself.

use corvane_core::{Dispatcher, GitHubRepository, RefCheck};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ci_status::ci_status;
use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

pub struct CiCheckRunRerunDialog {
    github: GitHubRepository,
    checks: Vec<RefCheck>,
    git_ref: String,
    failed_only: bool,
    loading_suites: bool,
    loading_rerun: bool,
    rerunnable: Vec<RefCheck>,
    non_rerunnable: Vec<RefCheck>,
}

impl CiCheckRunRerunDialog {
    pub fn new(
        github: GitHubRepository,
        checks: Vec<RefCheck>,
        git_ref: String,
        failed_only: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        // `determineRerunnability`
        let considered: Vec<RefCheck> = if failed_only {
            checks
                .iter()
                .filter(|c| c.conclusion == Some(corvane_core::CheckConclusion::Failure))
                .cloned()
                .collect()
        } else {
            checks.clone()
        };
        let weak = cx.weak_entity();
        Dispatcher::determine_rerunnable_checks(
            github.clone(),
            considered,
            move |rerunnable, non_rerunnable, cx| {
                weak.update(cx, |this, cx| {
                    this.loading_suites = false;
                    this.rerunnable = rerunnable;
                    this.non_rerunnable = non_rerunnable;
                    cx.notify();
                })
                .ok();
            },
            cx,
        );
        Self {
            github,
            checks,
            git_ref,
            failed_only,
            loading_suites: true,
            loading_rerun: false,
            rerunnable: Vec::new(),
            non_rerunnable: Vec::new(),
        }
    }

    /// `getTitle`
    fn title(&self, with_descriptor: bool) -> String {
        let s = if self.checks.len() == 1 { "" } else { "s" };
        let descriptor = match (with_descriptor, self.failed_only, self.checks.len()) {
            (true, true, _) => "Failed ",
            (true, false, 1) => "Single ",
            _ => "",
        };
        format!("Re-run {descriptor}Check{s}")
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.rerunnable.is_empty() || self.loading_suites || self.loading_rerun {
            return;
        }
        self.loading_rerun = true;
        cx.notify();
        let (github, git_ref, rerunnable, failed_only) = (
            self.github.clone(),
            self.git_ref.clone(),
            self.rerunnable.clone(),
            self.failed_only,
        );
        let pending = rerunnable.clone();
        Dispatcher::rerequest_check_suites(
            github.clone(),
            rerunnable,
            failed_only,
            move |_, cx| {
                // `manualRefreshSubscription`
                Dispatcher::set_commit_status_pending(&github, &git_ref, pending, cx);
                Dispatcher::close_popup(cx);
            },
            cx,
        );
    }
}

impl Render for CiCheckRunRerunDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let loading = self.loading_suites || self.loading_rerun;
        let content: AnyElement =
            if self.loading_suites && self.checks.len() > 1 {
                // `.loading-rerun-checks`
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .min_h(px(100.))
                    .child(img("illustrations/empty-no-pull-requests.svg").w(px(240.)))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Please wait"))
                    .child(
                        div()
                            .text_size(FONT_SIZE_SM)
                            .child("Determining which checks can be re-run."),
                    )
                    .into_any_element()
            } else {
                let dependents = (!self.rerunnable.is_empty()).then(|| {
                    let (name, adjective): (AnyElement, &str) = if self.checks.len() == 1 {
                        (
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.checks[0].name.clone())
                                .into_any_element(),
                            "its",
                        )
                    } else {
                        (div().child("these workflows").into_any_element(), "their")
                    };
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(3.))
                        .child("A new attempt of")
                        .child(name)
                        .child(format!(
                            "will be started, including all of {adjective} dependents:"
                        ))
                });
                let list = (!self.rerunnable.is_empty()).then(|| {
                    // `.check-run-rerun-list` (condensed rows)
                    div()
                        .id("check-run-rerun-list")
                        .my(SPACING)
                        .p(SPACING)
                        .max_h(px(300.))
                        .overflow_y_scroll()
                        .border_1()
                        .border_color(t.box_border)
                        .rounded(BORDER_RADIUS)
                        .flex()
                        .flex_col()
                        .children(self.rerunnable.iter().map(|check| {
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(SPACING)
                                .child(
                                    div()
                                        .flex_none()
                                        .p(px(3.))
                                        .child(ci_status(check.status, check.conclusion)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(check.name.clone()),
                                )
                        }))
                        .with_scrollbar()
                });
                let warning =
                    (!self.loading_suites && !self.non_rerunnable.is_empty()).then(|| {
                        let n = self.non_rerunnable.len();
                        let failed = if self.failed_only { "failed " } else { "" };
                        let prefix = if self.rerunnable.is_empty() {
                            format!("There are no {failed}checks that can be re-run")
                        } else {
                            format!(
                                "There {} {n} {failed}check{} that cannot be re-run",
                                if n == 1 { "is" } else { "are" },
                                if n == 1 { "" } else { "s" }
                            )
                        };
                        div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(SPACING_HALF)
                    .text_size(FONT_SIZE_SM)
                    .text_color(t.text_secondary)
                    .child(octicon(Octicon::Alert, t.dialog_warning).flex_none().mt(px(1.)))
                    .child(div().flex_1().child(format!(
                        "{prefix}. A check run cannot be re-run if the check is more than one \
                         month old, the check or its dependent has not completed, or the check \
                         is not configured to be re-run."
                    )))
                    });
                div()
                    .flex()
                    .flex_col()
                    .text_size(FONT_SIZE)
                    .children(dependents)
                    .children(list)
                    .children(warning)
                    .into_any_element()
            };
        let weak = cx.weak_entity();
        dialog(
            "rerun-check-runs",
            self.title(true),
            div().w(px(460.)).child(content),
            vec![
                DialogButton {
                    id: "rerun-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "rerun-ok",
                    label: self.title(false).into(),
                    primary: true,
                    disabled: self.rerunnable.is_empty() || loading,
                    on_click: Box::new(move |_, cx| {
                        weak.update(cx, |this, cx| this.submit(cx)).ok();
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
