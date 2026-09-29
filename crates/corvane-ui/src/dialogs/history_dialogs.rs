//! History-operation dialogs: `ui/reset/warning-before-reset.tsx`,
//! `ui/checkout/confirm-checkout-commit.tsx`, `ui/create-tag/create-tag-dialog.tsx`,
//! `ui/undo/warn-local-changes-before-undo.tsx`.

use corvane_core::Dispatcher;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog, dialog_with_kind};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{checkbox, text_box};

/// `WarningBeforeReset`
pub struct ResetToCommitDialog {
    repo: u64,
    sha: String,
}

impl ResetToCommitDialog {
    pub fn new(repo: u64, sha: String) -> Self {
        Self { repo, sha }
    }
}

impl Render for ResetToCommitDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, sha) = (self.repo, self.sha.clone());
        dialog_with_kind(
            "dialog-reset-to-commit",
            DialogKind::Warning,
            "Reset to Commit",
            div().child(
                "You have changes in progress. Resetting to a previous commit might result in \
                 some of these changes being lost. Do you want to continue anyway?",
            ),
            vec![
                DialogButton {
                    id: "reset-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "reset-continue",
                    label: "Continue".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::reset_to_commit(repo, sha.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `ConfirmCheckoutCommit`
pub struct CheckoutCommitDialog {
    repo: u64,
    sha: String,
    dont_show_again: bool,
}

impl CheckoutCommitDialog {
    pub fn new(repo: u64, sha: String) -> Self {
        Self {
            repo,
            sha,
            dont_show_again: false,
        }
    }
}

impl Render for CheckoutCommitDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, sha, dont_show_again) = (self.repo, self.sha.clone(), self.dont_show_again);
        let content = div()
            .flex()
            .flex_col()
            .child(div().mb(SPACING).child(
                "Checking out a commit will create a detached HEAD, and you will no longer be on \
                 any branch. Are you sure you want to checkout this commit?",
            ))
            .child(
                div()
                    .id("checkout-dont-show")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_show_again = !this.dont_show_again;
                        cx.notify();
                    }))
                    .child(checkbox(
                        "checkout-dont-show-box",
                        self.dont_show_again,
                        false,
                        cx,
                    ))
                    .child("Do not show this message again"),
            );
        dialog_with_kind(
            "dialog-checkout-commit",
            DialogKind::Warning,
            "Checkout Commit?",
            content,
            vec![
                DialogButton {
                    id: "checkout-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "checkout-ok",
                    label: "Checkout".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_show_again {
                            Dispatcher::update_settings(cx, |s| s.confirm_checkout_commit = false);
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::checkout_commit(repo, sha.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `CreateTag`
pub struct CreateTagDialog {
    repo: u64,
    sha: String,
    name: Entity<InputState>,
}

/// GHD `MaxTagNameLength`
const MAX_TAG_NAME_LENGTH: usize = 245;

impl CreateTagDialog {
    pub fn new(repo: u64, sha: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx));
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        Self { repo, sha, name }
    }
}

impl Render for CreateTagDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let name = self.name.read(cx).value().trim().to_string();
        let error = if name.len() > MAX_TAG_NAME_LENGTH {
            Some(format!(
                "The tag name cannot be longer than {MAX_TAG_NAME_LENGTH} characters"
            ))
        } else {
            None
        };
        let disabled = error.is_some() || name.is_empty();
        let (repo, sha) = (self.repo, self.sha.clone());
        let t = cx.ghd();
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING_HALF)
            .when_some(error, |d, message| {
                d.child(
                    div()
                        .mb(SPACING_HALF)
                        .text_color(t.form_error_text)
                        .child(message),
                )
            })
            .child(div().child("Name"))
            .child(text_box("tag-name", &self.name, None, window, cx));
        dialog(
            "dialog-create-tag",
            "Create a Tag",
            content,
            vec![
                DialogButton {
                    id: "tag-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "tag-create",
                    label: "Create Tag".into(),
                    primary: true,
                    disabled,
                    on_click: Box::new(move |_, cx| {
                        if disabled {
                            return;
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::create_tag(repo, name.clone(), sha.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `WarnLocalChangesBeforeUndo`
pub struct WarnLocalChangesBeforeUndoDialog {
    repo: u64,
    dont_show_again: bool,
}

impl WarnLocalChangesBeforeUndoDialog {
    pub fn new(repo: u64) -> Self {
        Self {
            repo,
            dont_show_again: false,
        }
    }
}

impl Render for WarnLocalChangesBeforeUndoDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, dont_show_again) = (self.repo, self.dont_show_again);
        let content = div()
            .flex()
            .flex_col()
            .child(div().mb(SPACING).child(
                "You have changes in progress. Undoing the commit might result in some of these \
                 changes being lost. Do you want to continue anyway?",
            ))
            .child(
                div()
                    .id("undo-dont-show")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_show_again = !this.dont_show_again;
                        cx.notify();
                    }))
                    .child(checkbox(
                        "undo-dont-show-box",
                        self.dont_show_again,
                        false,
                        cx,
                    ))
                    .child("Do not show this message again"),
            );
        dialog_with_kind(
            "dialog-warn-undo",
            DialogKind::Warning,
            "Undo Commit",
            content,
            vec![
                DialogButton {
                    id: "undo-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "undo-continue",
                    label: "Continue".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_show_again {
                            Dispatcher::update_settings(cx, |s| s.confirm_undo_commit = false);
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::undo_commit(repo, cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

/// `ConfirmDiscardStash`
pub struct ConfirmDiscardStashDialog {
    repo: u64,
    dont_show_again: bool,
}

impl ConfirmDiscardStashDialog {
    pub fn new(repo: u64) -> Self {
        Self {
            repo,
            dont_show_again: false,
        }
    }
}

impl Render for ConfirmDiscardStashDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, dont_show_again) = (self.repo, self.dont_show_again);
        let content = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(SPACING)
                    .child("Are you sure you want to discard these stashed changes?"),
            )
            .child(
                div()
                    .id("discard-stash-dont-show")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_show_again = !this.dont_show_again;
                        cx.notify();
                    }))
                    .child(checkbox(
                        "discard-stash-dont-show-box",
                        self.dont_show_again,
                        false,
                        cx,
                    ))
                    .child("Do not show this message again"),
            );
        dialog_with_kind(
            "dialog-discard-stash",
            DialogKind::Warning,
            "Discard Stash?",
            content,
            vec![
                DialogButton {
                    id: "discard-stash-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "discard-stash-ok",
                    label: "Discard".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_show_again {
                            Dispatcher::update_settings(cx, |s| s.confirm_discard_stash = false);
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::drop_stash(repo, cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
