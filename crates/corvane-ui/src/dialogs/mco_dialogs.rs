//! Multi-commit operation dialogs - GHD `ui/multi-commit-operation/`:
//! the step router (`multi-commit-operation.tsx` + `base-*.tsx`), the
//! rebase choose-branch step (`choose-branch/rebase-choose-branch-dialog.tsx`),
//! the cherry-pick target picker (`choose-target-branch.tsx`), and the shared
//! `dialog/{progress,conflicts,confirm-abort,warn-force-push}-dialog.tsx`,
//! plus `local-changes-overwritten-dialog.tsx` and the squash message popup
//! (`commit-message` in a dialog).

use corvane_core::{
    AppState, Dispatcher, ManualConflictResolution, McoStep, MultiCommitOperationKind, RetryAction,
    WorkingDirectoryFileChange, conflicted_files, resolved_files, unmerged_files,
};
use gpui_kit::component::input::{InputState, Textarea, TextareaState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::branch_list::group_branches;
use crate::context_menu::MenuItem;
use crate::dialog::{
    DialogButton, DialogFrame, DialogKind, dialog, dialog_framed, dialog_with_kind,
};
use crate::dialogs::branch_dialogs::{CreateBranchDialog, branch_picker, split_button};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, checkbox, link_button, text_box};

/// `MultiCommitOperation`: renders the dialog for the current step.
pub struct McoDialog {
    state: Entity<AppState>,
    repo: u64,
    filter: Entity<InputState>,
    /// The ChooseBranch list takes focus when a row is pressed.
    list_focus: FocusHandle,
    /// ChooseBranch step selection.
    selected_branch: Option<String>,
    /// WarnForcePush "Do not show this message again".
    dont_ask_force_push: bool,
    /// Cherry-pick › New Branch sub-dialog.
    create_branch: Option<Entity<CreateBranchDialog>>,
}

impl McoDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        // `FilterList` autofocuses its filter box
        let handle = filter.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Self {
            state,
            repo,
            filter,
            list_focus: cx.focus_handle(),
            selected_branch: None,
            dont_ask_force_push: false,
            create_branch: None,
        }
    }

    fn current_branch(&self, cx: &App) -> String {
        self.state
            .read(cx)
            .repo_states
            .get(&self.repo)
            .and_then(|r| r.info.as_ref())
            .and_then(|i| i.current_branch())
            .map(|b| b.name.clone())
            .unwrap_or_default()
    }

    // ---- ChooseBranch (rebase) ----

    fn rebase_choose_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let repo = self.repo;
        let close = move |_: &mut Window, cx: &mut App| Dispatcher::end_mco(repo, cx);
        let query = self.filter.read(cx).value().trim().to_string();
        let current = self.current_branch(cx);
        let (groups, preview) = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&repo);
            let info = rs.and_then(|r| r.info.as_ref());
            let groups = match (info, rs) {
                (Some(info), Some(rs)) => group_branches(
                    &info.branches,
                    rs.default_branch.as_deref(),
                    &rs.recent_branches,
                    &query,
                ),
                _ => Vec::new(),
            };
            (groups, rs.and_then(|r| r.rebase_preview.clone()))
        };
        let selected = self.selected_branch.clone();
        let preview = preview.filter(|p| Some(&p.base_branch) == selected.as_ref());
        let on_select = cx.listener(move |this, name: &String, _, cx| {
            this.selected_branch = Some(name.clone());
            Dispatcher::preview_rebase(repo, name.clone(), cx);
            cx.notify();
        });
        let list = branch_picker(
            "rebase",
            &self.filter,
            &self.list_focus,
            groups,
            &current,
            selected.as_deref(),
            std::rc::Rc::new(on_select),
            window,
            cx,
        );
        // `renderStatusPreview`
        let status: Option<AnyElement> = selected.as_ref().filter(|b| **b != current).map(|base| {
            let (icon, color, message): (Octicon, Hsla, AnyElement) = match &preview {
                None => (
                    Octicon::DotFill,
                    t.color_modified,
                    div()
                        .child("Checking for ability to rebase automatically…")
                        .into_any_element(),
                ),
                Some(p) if !p.valid => (
                    Octicon::X,
                    t.color_deleted,
                    div()
                        .child("Unable to start rebase. Check you have chosen a valid branch.")
                        .into_any_element(),
                ),
                Some(p) => {
                    let ahead = p.commits_ahead.len();
                    let behind = p.behind;
                    let row = div().flex().flex_row().flex_wrap().justify_center();
                    let msg = if behind > 0 && ahead == 0 {
                        row.child("This will fast-forward\u{a0}")
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.text)
                                    .child(current.clone()),
                            )
                            .child("\u{a0}by\u{a0}")
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.text)
                                    .child(format!(
                                        "{behind} {}",
                                        if behind == 1 { "commit" } else { "commits" }
                                    )),
                            )
                            .child("\u{a0}to match\u{a0}")
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.text)
                                    .child(base.clone()),
                            )
                    } else if behind > 0 && ahead > 0 {
                        row.child("This will update\u{a0}")
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.text)
                                    .child(current.clone()),
                            )
                            .child("\u{a0}by applying its\u{a0}")
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.text)
                                    .child(format!(
                                        "{ahead} {}",
                                        if ahead == 1 { "commit" } else { "commits" }
                                    )),
                            )
                            .child("\u{a0}on top of\u{a0}")
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.text)
                                    .child(base.clone()),
                            )
                    } else {
                        row.child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(t.text)
                                .child(current.clone()),
                        )
                        .child("\u{a0}is already up to date with\u{a0}")
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(t.text)
                                .child(base.clone()),
                        )
                    };
                    (Octicon::Check, t.color_new, msg.into_any_element())
                }
            };
            status_preview(icon, color, message, cx)
        });
        let can_start = preview.as_ref().is_some_and(|p| p.valid && p.behind > 0)
            && selected.as_deref() != Some(current.as_str());
        let base_for_ok = selected.clone();
        let content = div().flex().flex_col().child(list);
        // `getDialogTitle`: light "Rebase" with the branch in <strong>
        let title = div()
            .flex()
            .flex_row()
            .font_weight(FontWeight::LIGHT)
            .child("Rebase\u{a0}")
            .child(div().font_weight(FontWeight::NORMAL).child(current.clone()));
        // `DialogFooter`: the status preview over the `DropdownSelectButton`
        // (its checked option, Rebase)
        let footer = div()
            .flex()
            .flex_col()
            .pt(SPACING())
            .px(SPACING_DOUBLE())
            .pb(SPACING_DOUBLE())
            .border_t_1()
            .border_color(t.box_border)
            .children(status)
            .child(split_button(
                "rebase-start",
                "Rebase",
                !can_start,
                move |_, cx| {
                    let Some(base) = base_for_ok.clone() else {
                        return;
                    };
                    Dispatcher::start_rebase(repo, base, false, cx);
                },
                cx,
            ))
            .into_any_element();
        dialog_framed(
            "dialog-rebase-branch",
            title,
            format!("Rebase {current}"),
            content,
            DialogFrame {
                header_border: false,
                content_padding: false,
                footer: Some(footer),
                ..DialogFrame::default()
            },
            close,
            window,
            cx,
        )
        .into_any_element()
    }

    // ---- ChooseBranch (cherry-pick target) ----

    fn choose_target_branch(
        &mut self,
        commit_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let repo = self.repo;
        let close = move |_: &mut Window, cx: &mut App| Dispatcher::end_mco(repo, cx);
        let query = self.filter.read(cx).value().trim().to_string();
        let current = self.current_branch(cx);
        let groups = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&repo);
            let info = rs.and_then(|r| r.info.as_ref());
            match (info, rs) {
                (Some(info), Some(rs)) => group_branches(
                    &info.branches,
                    rs.default_branch.as_deref(),
                    &rs.recent_branches,
                    &query,
                ),
                _ => Vec::new(),
            }
        };
        let no_results = groups.is_empty();
        let selected = self.selected_branch.clone();
        let on_select = cx.listener(move |this, name: &String, _, cx| {
            this.selected_branch = Some(name.clone());
            cx.notify();
        });
        let list = branch_picker(
            "cherry-pick",
            &self.filter,
            &self.list_focus,
            groups,
            &current,
            selected.as_deref(),
            std::rc::Rc::new(on_select),
            window,
            cx,
        );
        let plural = if commit_count > 1 {
            "commits"
        } else {
            "commit"
        };
        let ok_label: String = if no_results {
            "Cherry-pick to New Branch".to_string()
        } else {
            match &selected {
                Some(branch) => format!("Cherry-pick {commit_count} {plural} to {branch}…"),
                None => format!("Cherry-pick {commit_count} {plural}"),
            }
        };
        let selected_is_current = selected.as_deref() == Some(current.as_str());
        let enabled = no_results || (selected.is_some() && !selected_is_current);
        let selected_for_ok = selected.clone();
        let query_for_ok = query.clone();
        let content = div()
            .w(zpx(450.))
            .mx(zpx(-20.))
            .mt(zpx(-20.))
            .flex()
            .flex_col()
            .child(list);
        let title = div()
            .font_weight(FontWeight::SEMIBOLD)
            .child(format!("Cherry-pick {commit_count} {plural} to a branch"));
        dialog_with_title(
            "dialog-cherry-pick",
            title,
            format!("Cherry-pick {commit_count} {plural} to a branch"),
            content,
            vec![DialogButton {
                id: "cherry-pick-ok",
                label: ok_label.into(),
                primary: true,
                disabled: !enabled,
                on_click: Box::new(move |_, cx| {
                    if !enabled {
                        return;
                    }
                    if no_results {
                        Dispatcher::cherry_pick_show_create_branch(repo, query_for_ok.clone(), cx);
                        return;
                    }
                    if let Some(branch) = selected_for_ok.clone() {
                        Dispatcher::cherry_pick_to_branch(repo, branch, cx);
                    }
                }),
            }],
            close,
            window,
            cx,
        )
        .into_any_element()
    }

    // ---- WarnForcePush ----

    fn warn_force_push(
        &mut self,
        kind: MultiCommitOperationKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let repo = self.repo;
        let close = move |_: &mut Window, cx: &mut App| Dispatcher::end_mco(repo, cx);
        let dont_ask = self.dont_ask_force_push;
        let label = kind.label();
        let lower = kind.lower();
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(format!("Are you sure you want to {lower}?"))
            .child(format!(
                "At the end of the {lower} flow, Corvane will enable you to force push the branch to update the upstream branch. Force pushing will alter the history on the remote and potentially cause problems for others collaborating on this branch."
            ))
            .child(
                div()
                    .id("force-push-dont-ask")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_ask_force_push = !this.dont_ask_force_push;
                        cx.notify();
                    }))
                    .child(checkbox("force-push-dont-ask-box", dont_ask, false, cx))
                    .child("Do not show this message again"),
            );
        dialog_with_kind(
            "dialog-warn-force-push",
            DialogKind::Warning,
            format!("{label} Will Require Force Push"),
            content,
            vec![
                DialogButton {
                    id: "force-push-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "force-push-begin",
                    label: format!("Begin {label}").into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::begin_after_force_push_warning(repo, !dont_ask, cx)
                    }),
                },
            ],
            close,
            window,
            cx,
        )
        .into_any_element()
    }

    // ---- ShowProgress ----

    fn progress(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let Some(mco) = self
            .state
            .read(cx)
            .repo_states
            .get(&self.repo)
            .and_then(|r| r.mco.clone())
        else {
            return div().into_any_element();
        };
        let p = mco.progress.clone();
        let content = div()
            .w(zpx(400.))
            .flex()
            .flex_col()
            .child(
                // <progress>
                div()
                    .w_full()
                    .h(zpx(6.))
                    .mb(SPACING())
                    .rounded(zpx(3.))
                    .bg(t.box_alt_background)
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .w(gpui_kit::relative(p.value.clamp(0., 1.)))
                            .bg(t.link),
                    ),
            )
            .child(
                // `.details`
                div()
                    .flex()
                    .flex_row()
                    .pt(SPACING())
                    .pb(SPACING_DOUBLE())
                    .child(green_circle(cx))
                    .child(
                        div()
                            .pl(SPACING())
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("Commit {} of {}", p.position, p.total)),
                            )
                            .child(div().child(p.current_summary.clone())),
                    ),
            );
        dialog(
            "dialog-mco-progress",
            format!("{} in progress", mco.kind().label()),
            content,
            Vec::new(),
            |_, _| {},
            window,
            cx,
        )
        .into_any_element()
    }

    // ---- ShowConflicts ----

    fn conflicts(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let repo = self.repo;
        let (mco, status, resolutions, workdir) = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&repo);
            (
                rs.and_then(|r| r.mco.clone()),
                rs.and_then(|r| r.status.clone()),
                rs.and_then(|r| r.conflict_state.as_ref())
                    .map(|c| c.manual_resolutions.clone())
                    .unwrap_or_default(),
                rs.and_then(|r| r.info.as_ref()).map(|i| i.workdir.clone()),
            )
        };
        let (Some(mco), Some(status)) = (mco, status) else {
            return div().into_any_element();
        };
        let kind = mco.kind();
        let label = kind.label();
        let our = mco.conflicts.our_branch.clone();
        let their = mco.conflicts.their_branch.clone();
        let unmerged: Vec<WorkingDirectoryFileChange> =
            unmerged_files(&status).into_iter().cloned().collect();
        let conflicted_count = conflicted_files(&status, &resolutions).len();
        let resolved_count = resolved_files(&status, &resolutions).len();
        let close = move |_: &mut Window, cx: &mut App| Dispatcher::hide_conflicts(repo, cx);

        let mut content = div().w(zpx(460.)).flex().flex_col();
        if resolved_count > 0 {
            // `DialogSuccess`
            content = content.child(
                div()
                    .mb(SPACING())
                    .px(SPACING())
                    .py(SPACING_HALF())
                    .rounded(BORDER_RADIUS())
                    .bg(t.box_alt_background)
                    .border_1()
                    .border_color(t.color_new)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .child(octicon(Octicon::Check, t.color_new))
                    .child(if conflicted_count == 0 {
                        "All conflicted files have been resolved.".to_string()
                    } else {
                        format!(
                            "{resolved_count} conflicted {} been resolved.",
                            if resolved_count == 1 {
                                "file has"
                            } else {
                                "files have"
                            }
                        )
                    }),
            );
        }
        if unmerged.is_empty() {
            // `renderAllResolved`
            content = content.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .py(SPACING())
                    .child(green_circle(cx))
                    .child(div().pl(SPACING()).child("All conflicts resolved")),
            );
        } else {
            content = content
                .child(
                    div()
                        .mb(SPACING_DOUBLE())
                        .text_size(FONT_SIZE_MD())
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(if conflicted_count == 1 {
                            "1 conflicted file".to_string()
                        } else {
                            format!("{conflicted_count} conflicted files")
                        }),
                )
                .child(
                    div()
                        .id("unmerged-files")
                        .max_h(zpx(285.))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap(SPACING_DOUBLE())
                        .children(unmerged.iter().map(|file| {
                            unmerged_file_row(
                                repo,
                                file,
                                resolutions.get(&file.path).copied(),
                                our.as_deref(),
                                their.as_deref(),
                                workdir.as_deref(),
                                cx,
                            )
                        }))
                        .with_scrollbar(),
                )
                .child(
                    div()
                        .mt(SPACING_DOUBLE())
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .child({
                            let workdir = workdir.clone();
                            link_button("open-in-shell", "Open in command line,", cx).on_click(
                                move |_, _, cx| {
                                    if let Some(dir) = &workdir {
                                        Dispatcher::open_in_shell(dir, cx);
                                    }
                                },
                            )
                        })
                        .child("\u{a0}your tool of choice, or close to resolve manually."),
                );
        }
        let can_continue = conflicted_count == 0;
        dialog(
            "dialog-conflicts",
            format!("Resolve conflicts before {label}"),
            content,
            vec![
                DialogButton {
                    id: "conflicts-abort",
                    label: format!("Abort {label}").into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| Dispatcher::request_abort_mco(repo, cx)),
                },
                DialogButton {
                    id: "conflicts-continue",
                    label: format!("Continue {label}").into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if can_continue {
                            Dispatcher::continue_after_conflicts(repo, cx);
                        }
                    }),
                },
            ],
            close,
            window,
            cx,
        )
        .into_any_element()
    }

    // ---- ConfirmAbort ----

    fn confirm_abort(
        &self,
        kind: MultiCommitOperationKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let repo = self.repo;
        let close = move |_: &mut Window, cx: &mut App| Dispatcher::return_to_conflicts(repo, cx);
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(format!("Are you sure you want to abort this {}?", kind.lower()))
            .child(
                "This will take you back to the original branch state and the conflicts you have already resolved will be discarded.",
            );
        dialog_with_kind(
            "dialog-confirm-abort",
            DialogKind::Warning,
            format!("Confirm Abort {}", kind.label()),
            content,
            vec![
                DialogButton {
                    id: "abort-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "abort-ok",
                    label: format!("Abort {}", kind.label()).into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| Dispatcher::abort_mco(repo, cx)),
                },
            ],
            close,
            window,
            cx,
        )
        .into_any_element()
    }
}

/// A dialog whose title is an element (bold branch names) rather than a string.
#[allow(clippy::too_many_arguments)]
fn dialog_with_title(
    id: &'static str,
    title: impl IntoElement,
    plain_title: String,
    content: impl IntoElement,
    buttons: Vec<DialogButton>,
    on_close: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    crate::dialog::dialog_with_title_element(
        id,
        title,
        plain_title,
        content,
        buttons,
        on_close,
        window,
        cx,
    )
}

/// `.green-circle` (22 px)
fn green_circle(cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .flex_none()
        .size(zpx(22.))
        .rounded_full()
        .bg(t.color_new)
        .flex()
        .items_center()
        .justify_center()
        .child(octicon(Octicon::Check, t.background))
}

/// `ActionStatusIcon` + `.merge-info` below the branch list.
fn status_preview(icon: Octicon, color: Hsla, message: AnyElement, cx: &App) -> AnyElement {
    // `.merge-status-component` in `#choose-branch`: the 20 px icon row
    // without its rule, then `.merge-info` (5 px above, 10 below)
    let t = cx.ghd();
    div()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .w_full()
                .h(zpx(20.))
                .flex()
                .justify_center()
                .child(octicon(icon, color)),
        )
        .child(
            div()
                .mt(SPACING_HALF())
                .mb(SPACING())
                .text_size(FONT_SIZE())
                .text_color(t.text_secondary)
                .text_center()
                .child(message),
        )
        .into_any_element()
}

/// GHD `getUnmergedStatusEntryDescription` / `getLabelForManualResolutionOption`.
fn side_label(entry: corvane_core::GitStatusEntry, branch: Option<&str>, resolved: bool) -> String {
    use corvane_core::GitStatusEntry as E;
    let suffix = branch.map(|b| format!(" from {b}")).unwrap_or_default();
    match (entry, resolved) {
        (E::Added, false) => format!("Use the added file{suffix}"),
        (E::Added, true) => format!("Using the added file{suffix}"),
        (E::Deleted, false) => format!(
            "Do not include this file{}",
            branch.map(|b| format!(" on {b}")).unwrap_or_default()
        ),
        (E::Deleted, true) => format!("Using the deleted file{suffix}"),
        (_, false) => format!("Use the modified file{suffix}"),
        (_, true) => format!("Using the modified file{suffix}"),
    }
}

/// `renderUnmergedFile`
#[allow(clippy::too_many_arguments)]
fn unmerged_file_row(
    repo: u64,
    file: &WorkingDirectoryFileChange,
    resolution: Option<ManualConflictResolution>,
    our: Option<&str>,
    their: Option<&str>,
    workdir: Option<&std::path::Path>,
    cx: &mut Context<McoDialog>,
) -> AnyElement {
    let t = cx.ghd();
    let path = file.path.clone();
    let status = file.status.clone();
    let unresolved = status.has_unresolved_conflicts(resolution);
    let full_path = workdir.map(|w| w.join(&file.path));
    let resolution_items = || -> Vec<MenuItem> {
        let (p1, p2) = (path.clone(), path.clone());
        vec![
            MenuItem::new(side_label(status.us(), our, false), move |_, cx| {
                Dispatcher::set_manual_resolution(
                    repo,
                    p1.clone(),
                    Some(ManualConflictResolution::Ours),
                    cx,
                )
            }),
            MenuItem::new(side_label(status.them(), their, false), move |_, cx| {
                Dispatcher::set_manual_resolution(
                    repo,
                    p2.clone(),
                    Some(ManualConflictResolution::Theirs),
                    cx,
                )
            }),
        ]
    };
    let path_text = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .pr(SPACING())
        .child(div().truncate().child(file.path.clone()));
    let row = div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .child(octicon(Octicon::FileCode, t.text).mr(SPACING()));
    if !unresolved {
        // resolved: status line + Undo (manual) + green check
        let summary = match (resolution, status.conflict_markers) {
            (_, Some(0)) => "No conflicts remaining".to_string(),
            (Some(ManualConflictResolution::Ours), _) => side_label(status.us(), our, true),
            (Some(ManualConflictResolution::Theirs), _) => side_label(status.them(), their, true),
            _ => "No conflicts remaining".to_string(),
        };
        let show_undo = summary != "No conflicts remaining";
        let undo_path = file.path.clone();
        return row
            .child(
                path_text.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.color_new)
                        .child(summary),
                ),
            )
            .when(show_undo, |d| {
                d.child(
                    button(
                        SharedString::from(format!("undo-{}", file.path)),
                        "Undo",
                        cx,
                    )
                    .mr(SPACING_HALF())
                    .on_click(move |_, _, cx| {
                        Dispatcher::set_manual_resolution(repo, undo_path.clone(), None, cx)
                    }),
                )
            })
            .child(green_circle(cx))
            .into_any_element();
    }
    match status.conflict_markers {
        Some(markers) => {
            // text conflict: "N conflicts" + Open in editor + ▾ menu
            let conflicts = markers.div_ceil(3);
            let open_path = full_path.clone();
            let items = resolution_items();
            let menu_path = full_path.clone();
            let rel_path = file.path.clone();
            row.child(
                path_text.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.color_conflicted)
                        .child(if conflicts == 1 {
                            "1 conflict".to_string()
                        } else {
                            format!("{conflicts} conflicts")
                        }),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .child(
                        button(
                            SharedString::from(format!("open-editor-{}", file.path)),
                            "Open in editor",
                            cx,
                        )
                        .rounded_tr(zpx(0.))
                        .rounded_br(zpx(0.))
                        .on_click(move |_, _, cx| {
                            if let Some(p) = &open_path {
                                cx.open_with_system(p);
                            }
                        }),
                    )
                    .child(
                        button(
                            SharedString::from(format!("resolve-menu-{}", file.path)),
                            "",
                            cx,
                        )
                        .rounded_tl(zpx(0.))
                        .rounded_bl(zpx(0.))
                        .ml(zpx(-1.))
                        .px(SPACING_HALF())
                        .child(octicon(Octicon::TriangleDown, t.secondary_button_text))
                        .on_click(cx.listener(
                            move |_, ev: &ClickEvent, window, cx| {
                                let mut menu_items = Vec::new();
                                if let Some(p) = menu_path.clone() {
                                    let p2 = p.clone();
                                    menu_items.push(MenuItem::new(
                                        "Open with Default Program",
                                        move |_, cx| {
                                            cx.open_with_system(&p);
                                        },
                                    ));
                                    menu_items.push(MenuItem::new(
                                        "Reveal in Finder",
                                        move |_, cx| {
                                            cx.reveal_path(&p2);
                                        },
                                    ));
                                    menu_items.push(MenuItem::separator());
                                }
                                let _ = &rel_path;
                                menu_items.extend(items.iter().cloned());
                                let position = ev.mouse_position().unwrap_or_default();
                                show_menu(menu_items, position, window, cx);
                            },
                        )),
                    ),
            )
            .into_any_element()
        }
        None => {
            // manual conflict (binary / deleted on one side): Resolve ▾
            let deleted_on = if status.us() == corvane_core::GitStatusEntry::Deleted {
                Some(our.unwrap_or("target branch"))
            } else if status.them() == corvane_core::GitStatusEntry::Deleted {
                Some(their.unwrap_or("target branch"))
            } else {
                None
            };
            let text = match deleted_on {
                Some(b) => format!("File does not exist on {b}."),
                None => "Manual conflict".to_string(),
            };
            let items = resolution_items();
            row.child(
                path_text.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.color_conflicted)
                        .child(text),
                ),
            )
            .child(
                button(
                    SharedString::from(format!("resolve-{}", file.path)),
                    "Resolve",
                    cx,
                )
                .gap(SPACING_HALF())
                .child(octicon(Octicon::TriangleDown, t.secondary_button_text))
                .on_click(cx.listener(move |_, ev: &ClickEvent, window, cx| {
                    let position = ev.mouse_position().unwrap_or_default();
                    show_menu(items.clone(), position, window, cx);
                })),
            )
            .into_any_element()
        }
    }
}

fn show_menu(items: Vec<MenuItem>, position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        crate::native_menu::show_context_menu(items, position, window, cx);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (items, position, window, cx);
    }
}

impl Render for McoDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mco = self
            .state
            .read(cx)
            .repo_states
            .get(&self.repo)
            .and_then(|r| r.mco.clone());
        let Some(mco) = mco else {
            return div().into_any_element();
        };
        match mco.step.clone() {
            McoStep::ChooseBranch => match mco.detail {
                corvane_core::McoDetail::CherryPick { commits, .. } => {
                    self.create_branch = None;
                    self.choose_target_branch(commits.len(), window, cx)
                }
                _ => self.rebase_choose_branch(window, cx),
            },
            McoStep::CreateBranch { initial_name } => {
                let repo = self.repo;
                if self.create_branch.is_none() {
                    let state = self.state.clone();
                    let view = cx.new(|cx| {
                        CreateBranchDialog::new_for_cherry_pick(
                            state,
                            repo,
                            initial_name,
                            window,
                            cx,
                        )
                    });
                    self.create_branch = Some(view);
                }
                self.create_branch.clone().unwrap().into_any_element()
            }
            McoStep::WarnForcePush => self.warn_force_push(mco.kind(), window, cx),
            McoStep::ShowProgress => self.progress(window, cx),
            McoStep::ShowConflicts => self.conflicts(window, cx),
            McoStep::ConfirmAbort => self.confirm_abort(mco.kind(), window, cx),
            McoStep::HideConflicts => div().into_any_element(),
        }
    }
}

// ---------------------------------------------------------------------------

/// `LocalChangesOverwrittenDialog`
pub struct LocalChangesOverwrittenDialog {
    state: Entity<AppState>,
    repo: u64,
    retry: RetryAction,
    files: Vec<String>,
}

impl LocalChangesOverwrittenDialog {
    pub fn new(state: Entity<AppState>, repo: u64, retry: RetryAction, files: Vec<String>) -> Self {
        Self {
            state,
            repo,
            retry,
            files,
        }
    }
}

impl Render for LocalChangesOverwrittenDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let repo = self.repo;
        let close = move |_: &mut Window, cx: &mut App| {
            Dispatcher::close_popup(cx);
            Dispatcher::end_mco(repo, cx);
        };
        let has_stash = self
            .state
            .read(cx)
            .repo_states
            .get(&repo)
            .is_some_and(|r| r.stash.is_some());
        let retry = self.retry.clone();
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(format!(
                "Unable to {} when changes are present on your branch.{}",
                self.retry.name(),
                if self.files.is_empty() {
                    ""
                } else {
                    " The following files would be overwritten:"
                }
            ))
            .when(!self.files.is_empty(), |d| {
                d.child(
                    div()
                        .id("overwritten-files")
                        .max_h(zpx(200.))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap(zpx(2.))
                        .font_family(crate::theme::mono_font())
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .children(self.files.iter().map(|f| div().truncate().child(f.clone())))
                        .with_scrollbar(),
                )
            })
            .when(!has_stash, |d| {
                d.child("You can stash your changes now and recover them afterwards.")
            });
        let mut buttons = vec![DialogButton {
            id: "overwritten-close",
            label: "Close".into(),
            primary: has_stash,
            disabled: false,
            on_click: Box::new(close),
        }];
        if !has_stash {
            buttons.push(DialogButton {
                id: "overwritten-stash",
                label: "Stash Changes and Continue".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(move |_, cx| {
                    Dispatcher::stash_and_retry(repo, retry.clone(), cx)
                }),
            });
        }
        dialog_with_kind(
            "dialog-local-changes-overwritten",
            DialogKind::Error,
            "Error",
            content,
            buttons,
            close,
            window,
            cx,
        )
    }
}

// ---------------------------------------------------------------------------

/// The `CommitMessage` popup used to name the squashed commit.
pub struct SquashCommitMessageDialog {
    repo: u64,
    to_squash: Vec<String>,
    onto: String,
    count: usize,
    summary: Entity<InputState>,
    description: Entity<TextareaState>,
}

impl SquashCommitMessageDialog {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: u64,
        to_squash: Vec<String>,
        onto: String,
        summary: String,
        description: String,
        count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let summary_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Summary (required)"));
        summary_state.update(cx, |s, cx| s.set_value(summary, window, cx));
        let description_state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(6)
                .placeholder("Description")
        });
        description_state.update(cx, |s, cx| s.set_value(description, window, cx));
        cx.observe(&summary_state, |_, _, cx| cx.notify()).detach();
        Self {
            repo,
            to_squash,
            onto,
            count,
            summary: summary_state,
            description: description_state,
        }
    }
}

impl Render for SquashCommitMessageDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let summary = self.summary.read(cx).value().trim().to_string();
        let description = self.description.read(cx).value().to_string();
        let disabled = summary.is_empty();
        let (repo, to_squash, onto, count) = (
            self.repo,
            self.to_squash.clone(),
            self.onto.clone(),
            self.count,
        );
        let content = div()
            .w(zpx(450.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(text_box("squash-summary", &self.summary, None, window, cx))
            .child(
                div()
                    .border_1()
                    .border_color(t.box_border_contrast)
                    .rounded(BORDER_RADIUS())
                    .bg(t.box_background)
                    .overflow_hidden()
                    .child(Textarea::new(&self.description)),
            );
        let title = format!("Squash {count} Commits");
        dialog(
            "dialog-squash-message",
            title.clone(),
            content,
            vec![
                DialogButton {
                    id: "squash-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "squash-ok",
                    label: title.into(),
                    primary: true,
                    disabled,
                    on_click: Box::new(move |_, cx| {
                        if disabled {
                            return;
                        }
                        let message = corvane_git::format_message(&summary, &description);
                        Dispatcher::close_popup(cx);
                        Dispatcher::squash(
                            repo,
                            to_squash.clone(),
                            onto.clone(),
                            message,
                            false,
                            cx,
                        );
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
