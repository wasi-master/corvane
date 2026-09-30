//! `PushBranchCommits` - GHD `ui/branches/push-branch-commits.tsx`: before a
//! pull request is created, publish the branch ("Publish Branch?") or push
//! its local commits ("Push Local Changes?", with "Create Without Pushing").
//!
//! Deviations: the base branch picked in Preview Pull Request is kept for
//! the compare page (GHD's `onConfirm` drops it), and a failed push leaves
//! the error on screen instead of opening the compare page anyway.

use corvane_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog_loading};
use crate::widgets::{Inline, code_ref, paragraph};

pub struct PushBranchCommitsDialog {
    repo: u64,
    branch: String,
    /// `None`: the branch is not published yet (`renderPublishView`).
    unpushed: Option<u32>,
    base: Option<String>,
    /// `isPushingOrPublishing`
    pushing: bool,
}

impl PushBranchCommitsDialog {
    pub fn new(repo: u64, branch: String, unpushed: Option<u32>, base: Option<String>) -> Self {
        Self {
            repo,
            branch,
            unpushed,
            base,
            pushing: false,
        }
    }
}

impl Render for PushBranchCommitsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let branch_ref = || Inline::Element(code_ref(self.branch.clone(), cx).into_any_element());
        let (title, first, second) = match self.unpushed {
            None => (
                mac_or("Publish Branch?", "Publish branch?"),
                paragraph(vec![
                    "Your branch must be published before opening a pull request.".into(),
                ]),
                paragraph(vec![
                    "Would you like to publish ".into(),
                    branch_ref(),
                    " now and open a pull request?".into(),
                ]),
            ),
            Some(count) => (
                mac_or("Push Local Changes?", "Push local changes?"),
                paragraph(vec![
                    format!(
                        "You have {count} local commit{} that haven't been pushed to the remote yet.",
                        if count == 1 { "" } else { "s" }
                    )
                    .into(),
                ]),
                paragraph(vec![
                    "Would you like to push your changes to ".into(),
                    branch_ref(),
                    " before creating your pull request?".into(),
                ]),
            ),
        };
        let content = div()
            .flex()
            .flex_col()
            .gap(crate::theme::sizes::SPACING())
            .child(first)
            .child(second);
        let (repo, base, pushing) = (self.repo, self.base.clone(), self.pushing);
        let cancel = match self.unpushed {
            None => DialogButton {
                id: "push-branch-commits-cancel",
                label: "Cancel".into(),
                primary: false,
                disabled: pushing,
                on_click: Box::new(close),
            },
            Some(_) => {
                let base = base.clone();
                DialogButton {
                    id: "push-branch-commits-create",
                    label: mac_or("Create Without Pushing", "Create without pushing").into(),
                    primary: false,
                    disabled: pushing,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::open_create_pull_request_in_browser(repo, base.clone(), cx);
                    }),
                }
            }
        };
        let entity = cx.entity();
        let ok = DialogButton {
            id: "push-branch-commits-ok",
            label: if self.unpushed.is_none() {
                mac_or("Publish Branch", "Publish branch")
            } else {
                mac_or("Push Commits", "Push commits")
            }
            .into(),
            primary: true,
            disabled: pushing,
            on_click: Box::new(move |_, cx| {
                entity.update(cx, |this, cx| {
                    this.pushing = true;
                    cx.notify();
                });
                Dispatcher::push_branch_commits_and_create_pull_request(repo, base.clone(), cx);
            }),
        };
        dialog_loading(
            "push-branch-commits",
            title,
            pushing,
            content,
            vec![cancel, ok],
            close,
            window,
            cx,
        )
    }
}
