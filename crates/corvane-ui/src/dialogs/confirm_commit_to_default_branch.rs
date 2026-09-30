//! Corvane addition (`275-confirm-commit-to-default-branch`, no GHD
//! counterpart): committing on the repository's default branch asks first.
//! Laid out like `unknown_authors.rs`.

use corvane_core::{Dispatcher, Popup};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};

pub struct ConfirmCommitToDefaultBranchDialog {
    repo: u64,
    branch: String,
    summary: String,
    description: String,
    unknown_co_authors: Vec<String>,
}

impl ConfirmCommitToDefaultBranchDialog {
    pub fn new(
        repo: u64,
        branch: String,
        summary: String,
        description: String,
        unknown_co_authors: Vec<String>,
    ) -> Self {
        Self {
            repo,
            branch,
            summary,
            description,
            unknown_co_authors,
        }
    }
}

impl Render for ConfirmCommitToDefaultBranchDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let content = div().child(format!(
            "You're about to commit directly to {}, the default branch. Are you sure you want \
             to commit?",
            self.branch
        ));
        let (repo, summary, description, unknown) = (
            self.repo,
            self.summary.clone(),
            self.description.clone(),
            self.unknown_co_authors.clone(),
        );
        dialog_with_kind(
            "confirm-commit-to-default-branch",
            DialogKind::Warning,
            "Commit to Default Branch",
            content,
            vec![
                DialogButton {
                    id: "confirm-commit-to-default-branch-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "confirm-commit-to-default-branch-ok",
                    label: "Commit".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        // the unknown co-authors prompt still follows
                        if unknown.is_empty() {
                            Dispatcher::commit(repo, summary.clone(), description.clone(), cx);
                        } else {
                            Dispatcher::show_popup(
                                Popup::UnknownAuthors {
                                    repo,
                                    usernames: unknown.clone(),
                                    summary: summary.clone(),
                                    description: description.clone(),
                                },
                                cx,
                            );
                        }
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
