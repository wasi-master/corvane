//! Branch dialogs: `ui/create-branch/create-branch-dialog.tsx`,
//! `ui/rename-branch/rename-branch-dialog.tsx`, `ui/delete-branch/delete-branch-dialog.tsx`,
//! `ui/stash-changes/{stash-and-switch-branch,overwrite-stashed-changes}-dialog.tsx`
//! and the merge `ChooseBranch` step (`merge-choose-branch-dialog.tsx`).

use corvane_core::{
    AppState, BranchKind, Dispatcher, Mergeability, Tip, UncommittedChangesStrategy,
};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::branch_list::group_branches;
use crate::dialog::{DialogButton, DialogKind, dialog, dialog_with_kind};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{checkbox, segmented_option, text_box};

/// `sanitizedRefName`: what GHD's `RefNameTextBox` turns the input into.
pub fn sanitize_ref_name(input: &str) -> String {
    let mut out = String::new();
    for c in input.trim().chars() {
        if c.is_whitespace() {
            out.push('-');
        } else if !matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\' | '"' | '\'')
            && !c.is_control()
        {
            out.push(c);
        }
    }
    while out.starts_with('.') || out.starts_with('/') || out.starts_with('-') {
        out.remove(0);
    }
    while out.ends_with('/') || out.ends_with('.') {
        out.pop();
    }
    out.replace("..", "-").replace("@{", "-").replace("//", "/")
}

fn ref_chip(name: impl Into<SharedString>, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .px(px(4.))
        .rounded(px(3.))
        .bg(t.box_alt_background)
        .border_1()
        .border_color(t.box_border)
        .font_family(crate::theme::MONO_FONT)
        .text_size(FONT_SIZE_SM)
        .child(name.into())
}

// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum StartPoint {
    DefaultBranch,
    CurrentBranch,
}

pub struct CreateBranchDialog {
    state: Entity<AppState>,
    repo: u64,
    target_sha: Option<String>,
    name: Entity<InputState>,
    start_point: StartPoint,
    /// Cherry-pick › New Branch: "Cherry-pick to New Branch" / "Create Branch and Cherry-pick".
    cherry_pick: bool,
}

impl CreateBranchDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        target_sha: Option<String>,
        initial_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx));
        if !initial_name.is_empty() {
            name.update(cx, |s, cx| s.set_value(initial_name, window, cx));
        }
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            repo,
            target_sha,
            name,
            start_point: StartPoint::DefaultBranch,
            cherry_pick: false,
        }
    }

    /// The `CreateBranch` step of a cherry-pick (`renderCreateBranch`).
    pub fn new_for_cherry_pick(
        state: Entity<AppState>,
        repo: u64,
        initial_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::new(state, repo, None, initial_name, window, cx);
        this.cherry_pick = true;
        this
    }
}

impl Render for CreateBranchDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cherry_pick = self.cherry_pick;
        let repo_for_close = self.repo;
        let close = move |_: &mut Window, cx: &mut App| {
            if cherry_pick {
                Dispatcher::end_mco(repo_for_close, cx);
            } else {
                Dispatcher::close_popup(cx);
            }
        };
        let t = cx.ghd();
        let raw = self.name.read(cx).value().to_string();
        let name = sanitize_ref_name(&raw);
        let (tip, default_branch, existing, target_commit) = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&self.repo);
            let info = rs.and_then(|r| r.info.as_ref());
            let existing: Vec<String> = info
                .map(|i| {
                    i.branches
                        .iter()
                        .filter(|b| b.kind == BranchKind::Local)
                        .map(|b| b.name.clone())
                        .collect()
                })
                .unwrap_or_default();
            let target = self.target_sha.as_ref().and_then(|sha| {
                rs.and_then(|r| r.commits.iter().find(|c| &c.sha == sha))
                    .map(|c| (c.summary.clone(), c.short_sha().to_string()))
            });
            (
                info.map(|i| i.tip.clone()).unwrap_or(Tip::Unknown),
                rs.and_then(|r| r.default_branch.clone()),
                existing,
                target,
            )
        };
        let exists = existing.contains(&name);
        let disabled = name.is_empty() || exists;
        let current = tip.branch_name().map(|s| s.to_string());

        // Where the branch starts from (`renderBranchDescription`).
        let mut description: Vec<AnyElement> = Vec::new();
        let mut start_point: Option<String> = None;
        if let Some((summary, short)) = &target_commit {
            description.push(
                div()
                    .child(format!(
                        "Your new branch will be based on the commit '{summary}' ({short}) from your repository."
                    ))
                    .into_any_element(),
            );
            start_point = self.target_sha.clone();
        } else {
            match &tip {
                Tip::Detached { sha } => description.push(
                    div()
                        .child(format!(
                            "You do not currently have any branch checked out (your HEAD reference is detached). As such your new branch will be based on your currently checked out commit ({}).",
                            &sha[..sha.len().min(7)]
                        ))
                        .into_any_element(),
                ),
                Tip::Unborn { .. } => description.push(
                    div()
                        .child("Your current branch is unborn (does not contain any commits). Creating a new branch will rename the current branch.")
                        .into_any_element(),
                ),
                Tip::Valid { branch } => {
                    let current_name = branch.name.clone();
                    match &default_branch {
                        Some(default) if *default != current_name => {
                            let selected = self.start_point;
                            start_point = Some(if selected == StartPoint::DefaultBranch {
                                default.clone()
                            } else {
                                current_name.clone()
                            });
                            description.push(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(div().mb(px(5.)).child("Create branch based on…"))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                segmented_option(
                                                    "start-default",
                                                    default.clone(),
                                                    "The default branch in your repository. Pick this to start on something new that's not dependent on your current branch.",
                                                    selected == StartPoint::DefaultBranch,
                                                    true,
                                                    false,
                                                    cx,
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.start_point = StartPoint::DefaultBranch;
                                                    cx.notify();
                                                })),
                                            )
                                            .child(
                                                segmented_option(
                                                    "start-current",
                                                    current_name.clone(),
                                                    "The currently checked out branch. Pick this if you need to build on work done on this branch.",
                                                    selected == StartPoint::CurrentBranch,
                                                    false,
                                                    true,
                                                    cx,
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.start_point = StartPoint::CurrentBranch;
                                                    cx.notify();
                                                })),
                                            ),
                                    )
                                    .into_any_element(),
                            );
                        }
                        _ => {
                            let is_default = default_branch.as_deref() == Some(current_name.as_str());
                            description.push(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .items_center()
                                    .child("Your new branch will be based on your currently checked out branch (")
                                    .child(ref_chip(current_name.clone(), cx))
                                    .child(if is_default { ").\u{a0}" } else { ")." })
                                    .when(is_default, |d| {
                                        d.child(ref_chip(current_name.clone(), cx))
                                            .child("\u{a0}is the default branch for your repository.")
                                    })
                                    .into_any_element(),
                            );
                        }
                    }
                }
                Tip::Unknown => {}
            }
        }
        let _ = current;

        let repo = self.repo;
        let unborn = matches!(tip, Tip::Unborn { .. });
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING)
            .when(exists, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.))
                        .text_color(t.form_error_text)
                        .child("A branch named")
                        .child(ref_chip(name.clone(), cx))
                        .child("already exists"),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF)
                    .child("Name")
                    .child(text_box("branch-name", &self.name, None, window, cx)),
            )
            .children(description);
        let name_for_ok = name.clone();
        dialog(
            "dialog-create-branch",
            if cherry_pick {
                "Cherry-pick to New Branch"
            } else {
                "Create a Branch"
            },
            content,
            vec![
                DialogButton {
                    id: "create-branch-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "create-branch-ok",
                    label: if cherry_pick {
                        "Create Branch and Cherry-pick".into()
                    } else {
                        "Create Branch".into()
                    },
                    primary: true,
                    on_click: Box::new(move |_, cx| {
                        if disabled {
                            return;
                        }
                        if cherry_pick {
                            Dispatcher::cherry_pick_to_new_branch(
                                repo,
                                name_for_ok.clone(),
                                start_point.clone(),
                                cx,
                            );
                            return;
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::create_branch(
                            repo,
                            name_for_ok.clone(),
                            start_point.clone(),
                            unborn,
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

// ---------------------------------------------------------------------------

pub struct RenameBranchDialog {
    state: Entity<AppState>,
    repo: u64,
    branch: String,
    name: Entity<InputState>,
}

impl RenameBranchDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        branch: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx));
        name.update(cx, |s, cx| s.set_value(branch.clone(), window, cx));
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            repo,
            branch,
            name,
        }
    }
}

impl Render for RenameBranchDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let t = cx.ghd();
        let new_name = sanitize_ref_name(&self.name.read(cx).value());
        let (upstream, existing) = {
            let s = self.state.read(cx);
            let info = s.repo_states.get(&self.repo).and_then(|r| r.info.as_ref());
            let branch = info.and_then(|i| i.branches.iter().find(|b| b.name == self.branch));
            (
                branch.and_then(|b| b.upstream_short().map(|u| u.to_string())),
                info.map(|i| {
                    i.branches
                        .iter()
                        .filter(|b| b.kind == BranchKind::Local)
                        .map(|b| b.name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            )
        };
        let exists = new_name != self.branch && existing.contains(&new_name);
        let disabled = new_name.is_empty() || new_name == self.branch || exists;
        let (repo, old) = (self.repo, self.branch.clone());
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING)
            .when_some(upstream, |d, upstream| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(SPACING_HALF)
                        .child(octicon(Octicon::Alert, t.dialog_warning))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .items_center()
                                .child("This branch is tracking\u{a0}")
                                .child(ref_chip(upstream, cx))
                                .child("\u{a0}and renaming this branch will not change the branch name on the remote."),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF)
                    .child("Name")
                    .child(text_box("rename-branch-name", &self.name, None, window, cx)),
            )
            .when(exists, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.))
                        .text_color(t.form_error_text)
                        .child("A branch named")
                        .child(ref_chip(new_name.clone(), cx))
                        .child("already exists"),
                )
            });
        let name_for_ok = new_name.clone();
        dialog(
            "dialog-rename-branch",
            "Rename Branch",
            content,
            vec![
                DialogButton {
                    id: "rename-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "rename-ok",
                    label: format!("Rename {}", self.branch).into(),
                    primary: true,
                    on_click: Box::new(move |_, cx| {
                        if disabled {
                            return;
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::rename_branch(repo, old.clone(), name_for_ok.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

// ---------------------------------------------------------------------------

pub struct DeleteBranchDialog {
    state: Entity<AppState>,
    repo: u64,
    branch: String,
    include_remote: bool,
}

impl DeleteBranchDialog {
    pub fn new(state: Entity<AppState>, repo: u64, branch: String) -> Self {
        Self {
            state,
            repo,
            branch,
            include_remote: false,
        }
    }
}

impl Render for DeleteBranchDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let exists_on_remote = {
            let s = self.state.read(cx);
            let info = s.repo_states.get(&self.repo).and_then(|r| r.info.as_ref());
            info.and_then(|i| i.branches.iter().find(|b| b.name == self.branch))
                .and_then(|b| b.upstream_short().map(|u| u.to_string()))
                .map(|u| {
                    info.is_some_and(|i| {
                        i.branches
                            .iter()
                            .any(|b| b.kind == BranchKind::Remote && b.name == u)
                    })
                })
                .unwrap_or(false)
        };
        let (repo, name, include_remote) = (self.repo, self.branch.clone(), self.include_remote);
        let content = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(SPACING)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .child("Delete branch")
                    .child(ref_chip(self.branch.clone(), cx))
                    .child("?"),
            )
            .child(div().mb(SPACING).child("This action cannot be undone."))
            .when(exists_on_remote, |d| {
                d.child(div().mb(SPACING).font_weight(FontWeight::SEMIBOLD).child(
                    "The branch also exists on the remote, do you wish to delete it there as well?",
                ))
                .child(
                    div()
                        .id("delete-remote-row")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.include_remote = !this.include_remote;
                            cx.notify();
                        }))
                        .child(checkbox(
                            "delete-remote-box",
                            self.include_remote,
                            false,
                            cx,
                        ))
                        .child("Yes, delete this branch on the remote"),
                )
            });
        dialog_with_kind(
            "dialog-delete-branch",
            DialogKind::Warning,
            "Delete Branch",
            content,
            vec![
                DialogButton {
                    id: "delete-branch-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "delete-branch-ok",
                    label: "Delete".into(),
                    primary: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::delete_branch(repo, name.clone(), include_remote, cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

// ---------------------------------------------------------------------------

pub struct StashAndSwitchBranchDialog {
    state: Entity<AppState>,
    repo: u64,
    branch: String,
    action: UncommittedChangesStrategy,
}

impl StashAndSwitchBranchDialog {
    pub fn new(state: Entity<AppState>, repo: u64, branch: String) -> Self {
        Self {
            state,
            repo,
            branch,
            action: UncommittedChangesStrategy::StashOnCurrentBranch,
        }
    }
}

impl Render for StashAndSwitchBranchDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let t = cx.ghd();
        let (current, has_stash) = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&self.repo);
            (
                rs.and_then(|r| r.info.as_ref())
                    .and_then(|i| i.current_branch())
                    .map(|b| b.name.clone())
                    .unwrap_or_default(),
                rs.is_some_and(|r| r.stash.is_some()),
            )
        };
        let (repo, branch, action) = (self.repo, self.branch.clone(), self.action);
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING)
            .when(has_stash && action == UncommittedChangesStrategy::StashOnCurrentBranch, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF)
                        .child(octicon(Octicon::Alert, t.dialog_warning))
                        .child("Your current stash will be overwritten by creating a new stash"),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .mb(px(5.))
                            .child("You have changes on this branch. What would you like to do with them?"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                segmented_option(
                                    "stash-leave",
                                    format!("Leave my changes on {current}"),
                                    "Your in-progress work will be stashed on this branch for you to return to later",
                                    action == UncommittedChangesStrategy::StashOnCurrentBranch,
                                    true,
                                    false,
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.action = UncommittedChangesStrategy::StashOnCurrentBranch;
                                    cx.notify();
                                })),
                            )
                            .child(
                                segmented_option(
                                    "stash-bring",
                                    format!("Bring my changes to {}", self.branch),
                                    "Your in-progress work will follow you to the new branch",
                                    action == UncommittedChangesStrategy::MoveToNewBranch,
                                    false,
                                    true,
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.action = UncommittedChangesStrategy::MoveToNewBranch;
                                    cx.notify();
                                })),
                            ),
                    ),
            );
        dialog(
            "dialog-stash-and-switch",
            "Switch Branch",
            content,
            vec![
                DialogButton {
                    id: "switch-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "switch-ok",
                    label: "Switch Branch".into(),
                    primary: true,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::checkout_branch(repo, branch.clone(), Some(action), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

// ---------------------------------------------------------------------------

pub struct ConfirmOverwriteStashDialog {
    repo: u64,
    branch: String,
}

impl ConfirmOverwriteStashDialog {
    pub fn new(repo: u64, branch: String) -> Self {
        Self { repo, branch }
    }
}

impl Render for ConfirmOverwriteStashDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, branch) = (self.repo, self.branch.clone());
        dialog_with_kind(
            "dialog-overwrite-stash",
            DialogKind::Warning,
            "Overwrite Stash?",
            div().child(
                "Are you sure you want to proceed? This will overwrite your existing stash with your current changes.",
            ),
            vec![
                DialogButton {
                    id: "overwrite-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "overwrite-ok",
                    label: "Overwrite".into(),
                    primary: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::checkout_branch(
                            repo,
                            branch.clone(),
                            Some(UncommittedChangesStrategy::StashOnCurrentBranch),
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

// ---------------------------------------------------------------------------

/// `MergeChooseBranchDialog`: pick a branch, preview the commit count, merge.
pub struct MergeBranchDialog {
    state: Entity<AppState>,
    repo: u64,
    squash: bool,
    filter: Entity<InputState>,
    selected: Option<String>,
}

impl MergeBranchDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        squash: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter"));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            repo,
            squash,
            filter,
            selected: None,
        }
    }
}

impl Render for MergeBranchDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let t = cx.ghd();
        let query = self.filter.read(cx).value().trim().to_string();
        let (current, groups, preview) = {
            let s = self.state.read(cx);
            let rs = s.repo_states.get(&self.repo);
            let info = rs.and_then(|r| r.info.as_ref());
            let current = info
                .and_then(|i| i.current_branch())
                .map(|b| b.name.clone())
                .unwrap_or_default();
            let groups = match (info, rs) {
                (Some(info), Some(rs)) => group_branches(
                    &info.branches,
                    rs.default_branch.as_deref(),
                    &rs.recent_branches,
                    &query,
                ),
                _ => Vec::new(),
            };
            (current, groups, rs.and_then(|r| r.merge_preview.clone()))
        };
        let repo = self.repo;
        let selected = self.selected.clone();
        let preview = preview.filter(|p| Some(&p.branch) == selected.as_ref());
        let title = if self.squash {
            format!("Squash and Merge into {current}")
        } else {
            format!("Merge into {current}")
        };
        let on_select = cx.listener(move |this, name: &String, _, cx| {
            this.selected = Some(name.clone());
            Dispatcher::preview_merge(repo, name.clone(), cx);
            cx.notify();
        });
        let list = branch_picker(
            "merge",
            &self.filter,
            groups,
            &current,
            selected.as_deref(),
            std::rc::Rc::new(on_select),
            window,
            cx,
        );
        // `.merge-status-component` (`MergeStatusHeader`)
        let bold = |text: String| div().font_weight(FontWeight::SEMIBOLD).child(text);
        let status: Option<AnyElement> = selected.as_ref().map(|branch| {
            let row = || div().flex().flex_row().flex_wrap().justify_center();
            let (icon, color, message): (Octicon, Hsla, AnyElement) = match &preview {
                None => (
                    Octicon::DotFill,
                    t.color_modified,
                    div()
                        .child("Checking for ability to merge automatically...")
                        .into_any_element(),
                ),
                Some(p) if p.mergeability == Some(Mergeability::Invalid) => (
                    Octicon::X,
                    t.color_deleted,
                    div()
                        .child("Unable to merge unrelated histories in this repository")
                        .into_any_element(),
                ),
                Some(p) if p.commits == 0 => (
                    Octicon::Check,
                    t.color_new,
                    row()
                        .child(bold(current.clone()))
                        .child("\u{a0}is already up to date with\u{a0}")
                        .child(bold(branch.clone()))
                        .into_any_element(),
                ),
                Some(p) => {
                    let commits = format!(
                        "{} {}",
                        p.commits,
                        if p.commits == 1 { "commit" } else { "commits" }
                    );
                    match p.mergeability {
                        Some(Mergeability::Conflicts(n)) => (
                            Octicon::Alert,
                            t.color_modified,
                            row()
                                .child("There will be\u{a0}")
                                .child(bold(format!(
                                    "{n} conflicted {}",
                                    if n == 1 { "file" } else { "files" }
                                )))
                                .child("\u{a0}when merging\u{a0}")
                                .child(bold(branch.clone()))
                                .child("\u{a0}into\u{a0}")
                                .child(bold(current.clone()))
                                .into_any_element(),
                        ),
                        _ => (
                            Octicon::Check,
                            t.color_new,
                            row()
                                .child("This will merge\u{a0}")
                                .child(bold(commits))
                                .child("\u{a0}from\u{a0}")
                                .child(bold(branch.clone()))
                                .child("\u{a0}into\u{a0}")
                                .child(bold(current.clone()))
                                .into_any_element(),
                        ),
                    }
                }
            };
            div()
                .flex()
                .flex_col()
                .items_center()
                .px(SPACING_DOUBLE)
                .pt(SPACING_HALF)
                .pb(SPACING)
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(px(20.))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .top(px(10.))
                                .h(px(1.))
                                .bg(t.box_border),
                        )
                        .child(
                            div()
                                .px(SPACING_HALF)
                                .bg(t.background)
                                .child(octicon(icon, color)),
                        ),
                )
                .child(
                    div()
                        .text_size(FONT_SIZE)
                        .text_color(t.text_secondary)
                        .text_center()
                        .child(message),
                )
                .into_any_element()
        });
        // `canStartOperation`: conflicts are fine (resolved afterwards), nothing to merge is not
        let can_start = preview
            .as_ref()
            .is_some_and(|p| p.commits > 0 && p.mergeability != Some(Mergeability::Invalid))
            && selected.as_deref() != Some(current.as_str());
        let selected_for_ok = selected.clone();
        let squash = self.squash;
        let content = div()
            .w(px(450.))
            .mx(px(-20.))
            .mt(px(-20.))
            .flex()
            .flex_col()
            .child(list)
            .children(status);
        dialog(
            "dialog-merge-branch",
            title,
            content,
            vec![DialogButton {
                id: "merge-ok",
                label: if squash {
                    "Squash and merge".into()
                } else {
                    "Create a merge commit".into()
                },
                primary: true,
                on_click: Box::new(move |_, cx| {
                    let Some(branch) = selected_for_ok.clone() else {
                        return;
                    };
                    if !can_start {
                        return;
                    }
                    Dispatcher::close_popup(cx);
                    Dispatcher::merge_branch(repo, branch, squash, cx);
                }),
            }],
            close,
            window,
            cx,
        )
    }
}

/// Callback for a branch chosen in `branch_picker`.
pub type BranchSelect = std::rc::Rc<dyn Fn(&String, &mut Window, &mut App)>;

/// The filter box + grouped branch list shared by the merge, rebase and
/// cherry-pick choose-branch dialogs (`BranchList` inside `ChooseBranchDialog`).
#[allow(clippy::too_many_arguments)]
pub fn branch_picker(
    id_prefix: &'static str,
    filter: &Entity<InputState>,
    groups: Vec<crate::branch_list::BranchGroup>,
    current: &str,
    selected: Option<&str>,
    on_select: BranchSelect,
    window: &Window,
    cx: &App,
) -> Div {
    let t = cx.ghd();
    let hover_bg = t.box_selected_active_background;
    let hover_text = t.box_selected_active_text;
    let current = current.to_string();
    let selected = selected.map(str::to_string);
    let list = div()
        .id(SharedString::from(format!("{id_prefix}-branch-list")))
        .h(px(300.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .children(groups.into_iter().map(|group| {
            let current = current.clone();
            let selected = selected.clone();
            let on_select = on_select.clone();
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(ROW_HEIGHT)
                        .pt(SPACING)
                        .px(SPACING_DOUBLE)
                        .flex()
                        .items_center()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(FONT_SIZE)
                        .child(group.title),
                )
                .children(group.branches.into_iter().map(move |b| {
                    let is_current = b.name == current;
                    let is_selected = selected.as_deref() == Some(b.name.as_str());
                    let name = b.name.clone();
                    let on_select = on_select.clone();
                    div()
                        .id(SharedString::from(format!(
                            "{id_prefix}-branch-{}",
                            b.full_name
                        )))
                        .h(ROW_HEIGHT)
                        .w_full()
                        .flex()
                        .flex_row()
                        .items_center()
                        .px(SPACING_DOUBLE)
                        .cursor_pointer()
                        .when(is_selected, |d| {
                            d.bg(t.box_selected_active_background)
                                .text_color(t.box_selected_active_text)
                        })
                        .when(!is_selected, move |d| {
                            d.hover(move |s| s.bg(hover_bg).text_color(hover_text))
                        })
                        .on_click(move |_, window, cx| {
                            if !is_current {
                                on_select(&name, window, cx);
                            }
                        })
                        .child(
                            octicon(
                                if is_current {
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
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(FONT_SIZE)
                                .child(b.name.clone()),
                        )
                }))
        }));
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .px(SPACING_DOUBLE)
                .pb(SPACING)
                .border_b_1()
                .border_color(t.box_border)
                .child(text_box(
                    SharedString::from(format!("{id_prefix}-filter")),
                    filter,
                    Some(octicon(Octicon::Search, t.text_secondary)),
                    window,
                    cx,
                )),
        )
        .child(list)
}
