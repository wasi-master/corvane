//! App-level dialogs: About (`ui/about/about.tsx`), Remove Repository
//! confirmation (`ui/remove-repository/confirm-remove-repository.tsx`), the
//! external editor error (`ui/editor/editor-error.tsx`) and the shell error
//! (`ui/shell/shell-error.tsx`).

use corvane_core::{AppState, Dispatcher, Popup, PreferencesTab};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog, dialog_with_kind};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{checkbox_row, link_button};

pub struct AboutDialog {
    version: String,
}

impl AboutDialog {
    pub fn new(version: String) -> Self {
        Self { version }
    }
}

impl Render for AboutDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let version = self.version.clone();
        let version_for_copy = version.clone();
        let content = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .items_center()
            .gap(SPACING)
            .child(img("icon/Corvane-256.png").size(px(64.)))
            .child(
                div()
                    .text_size(FONT_SIZE_MD)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Corvane"),
            )
            .child(
                // `.version-text`: click copies the version.
                div()
                    .id("about-version")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(version_for_copy.clone()))
                    })
                    .child(format!("Version {version}"))
                    .child(octicon(Octicon::Copy, t.text_secondary).size(px(12.))),
            )
            .child(
                div()
                    .text_align(TextAlign::Center)
                    .text_color(t.text_secondary)
                    .child(
                        "A native GitHub Desktop, in Rust. Updates arrive with the first release.",
                    ),
            )
            .child(
                div()
                    .mt(SPACING)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING)
                    .child(
                        link_button("about-license", "License", cx).on_click(|_, _, cx| {
                            Dispatcher::open_url(
                                "https://github.com/wasi-master/corvane/blob/main/LICENSE",
                                cx,
                            )
                        }),
                    )
                    .child(div().text_color(t.text_secondary).child("·"))
                    .child(
                        link_button("about-source", "Source code", cx).on_click(|_, _, cx| {
                            Dispatcher::open_url("https://github.com/wasi-master/corvane", cx)
                        }),
                    ),
            );
        dialog(
            "dialog-about",
            "About Corvane",
            content,
            vec![DialogButton {
                id: "about-close",
                label: "Close".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(close),
            }],
            close,
            window,
            cx,
        )
    }
}

/// `ConfirmRemoveRepository`: warning dialog with "Also move this repository
/// to Trash"; Remove is destructive so Cancel is the default button.
pub struct ConfirmRemoveRepositoryDialog {
    state: Entity<AppState>,
    repo: u64,
    move_to_trash: bool,
}

impl ConfirmRemoveRepositoryDialog {
    pub fn new(state: Entity<AppState>, repo: u64) -> Self {
        Self {
            state,
            repo,
            move_to_trash: false,
        }
    }
}

impl Render for ConfirmRemoveRepositoryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (name, path, missing) = {
            let s = self.state.read(cx);
            match s.repository(self.repo) {
                Some(r) => (r.name(), r.path.display().to_string(), r.missing),
                None => (String::new(), String::new(), true),
            }
        };
        let repo = self.repo;
        let trash = self.move_to_trash;
        let weak = cx.weak_entity();
        let content = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .gap(SPACING)
            .child(format!(
                "Are you sure you want to remove the repository \"{name}\" from Corvane?"
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING)
                    .child("The repository will be removed from Corvane:")
                    .child(
                        div()
                            .font_family(crate::theme::MONO_FONT)
                            .px(px(3.))
                            .rounded(px(3.))
                            .bg(t.box_alt_background)
                            .child(path),
                    ),
            )
            .when(!missing, |d| {
                d.child(checkbox_row(
                    "remove-repo-trash",
                    trash,
                    "Also move this repository to Trash",
                    move |value, _, cx| {
                        weak.update(cx, |this, cx| {
                            this.move_to_trash = value;
                            cx.notify();
                        })
                        .ok();
                    },
                    cx,
                ))
            });
        dialog_with_kind(
            "dialog-confirm-remove-repository",
            DialogKind::Warning,
            "Remove Repository",
            content,
            vec![
                DialogButton {
                    id: "remove-repo-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "remove-repo-ok",
                    label: "Remove".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        if trash {
                            Dispatcher::remove_repository_and_trash(repo, cx);
                        } else {
                            Dispatcher::remove_repository(repo, cx);
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

/// `ExternalEditorError` / `OpenShellFailed`: error dialogs whose secondary
/// button opens Settings › Integrations (or the suggested editor's site).
pub struct IntegrationErrorDialog {
    popup: Popup,
}

impl IntegrationErrorDialog {
    pub fn new(popup: Popup) -> Self {
        Self { popup }
    }
}

impl Render for IntegrationErrorDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (id, title, message, secondary): (
            &'static str,
            &'static str,
            String,
            Option<DialogButton>,
        ) = match &self.popup {
            Popup::ExternalEditorError {
                message,
                suggest_default_editor,
                open_preferences,
            } => {
                let secondary = if *suggest_default_editor {
                    Some(DialogButton {
                        id: "editor-error-download",
                        label: format!(
                            "Download {}",
                            corvane_platform::editors::SUGGESTED_EDITOR_NAME
                        )
                        .into(),
                        primary: false,
                        disabled: false,
                        on_click: Box::new(|_, cx| {
                            Dispatcher::close_popup(cx);
                            Dispatcher::open_url(
                                corvane_platform::editors::SUGGESTED_EDITOR_URL,
                                cx,
                            );
                        }),
                    })
                } else if *open_preferences {
                    Some(DialogButton {
                        id: "editor-error-settings",
                        label: "Open Settings".into(),
                        primary: false,
                        disabled: false,
                        on_click: Box::new(|_, cx| {
                            Dispatcher::open_preferences(PreferencesTab::Integrations, cx)
                        }),
                    })
                } else {
                    None
                };
                (
                    "dialog-external-editor-error",
                    "Unable to Open External Editor",
                    message.clone(),
                    secondary,
                )
            }
            Popup::ShellError { message } => (
                "dialog-shell-error",
                "Unable to Open Shell",
                message.clone(),
                Some(DialogButton {
                    id: "shell-error-settings",
                    label: "Open Settings".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(|_, cx| {
                        Dispatcher::open_preferences(PreferencesTab::Integrations, cx)
                    }),
                }),
            ),
            _ => ("dialog-integration-error", "Error", String::new(), None),
        };
        let mut buttons = Vec::new();
        if let Some(secondary) = secondary {
            buttons.push(secondary);
        }
        buttons.push(DialogButton {
            id: "integration-error-close",
            label: "Close".into(),
            primary: true,
            disabled: false,
            on_click: Box::new(close),
        });
        dialog_with_kind(
            id,
            DialogKind::Error,
            title,
            div().w(px(450.)).child(message),
            buttons,
            close,
            window,
            cx,
        )
    }
}
