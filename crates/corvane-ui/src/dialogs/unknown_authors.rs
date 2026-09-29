//! GHD `ui/unknown-authors/unknown-authors-dialog.tsx`: co-author handles
//! that could not be resolved before committing.

use corvane_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::theme::mono_font;
use crate::theme::sizes::*;

const MAX_AUTHORS_TO_LIST: usize = 10;

pub struct UnknownAuthorsDialog {
    repo: u64,
    usernames: Vec<String>,
    summary: String,
    description: String,
}

impl UnknownAuthorsDialog {
    pub fn new(repo: u64, usernames: Vec<String>, summary: String, description: String) -> Self {
        Self {
            repo,
            usernames,
            summary,
            description,
        }
    }
}

impl Render for UnknownAuthorsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let content = if self.usernames.len() > MAX_AUTHORS_TO_LIST {
            div().child(format!(
                "{} users weren't found and won't be added as co-authors of this commit. Are \
                 you sure you want to commit?",
                self.usernames.len()
            ))
        } else {
            div()
                .flex()
                .flex_col()
                .gap(SPACING())
                .child(
                    "These users weren't found and won't be added as co-authors of this commit. \
                     Are you sure you want to commit?",
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .pl(SPACING_DOUBLE())
                        .font_family(mono_font())
                        .children(self.usernames.iter().map(|u| format!("• @{u}"))),
                )
        };
        let (repo, summary, description) =
            (self.repo, self.summary.clone(), self.description.clone());
        dialog_with_kind(
            "unknown-authors",
            DialogKind::Warning,
            "Unknown Co-Authors",
            content,
            vec![
                DialogButton {
                    id: "unknown-authors-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "unknown-authors-ok",
                    label: "Commit Anyway".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::commit(repo, summary.clone(), description.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
