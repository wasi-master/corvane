//! "Add Local Repository" (`ui/add-repository/add-existing-repository.tsx`).
//! Like GHD 3.6.6 the path is only checked on submit (`addRepository` →
//! `validatePath`), the warning then staying until the next check - unless
//! flag `205-add-local-validates-while-typing` checks it on every change and
//! keeps Add Repository disabled until the path is a repository.

use std::path::PathBuf;

use corvane_core::{AppState, Dispatcher, Popup};
use corvane_git::PathStatus;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, labeled, text_box};

pub struct AddExistingRepositoryDialog {
    #[allow(dead_code)]
    state: Entity<AppState>,
    path: Entity<InputState>,
    /// The last `validatePath` result that warrants a warning
    /// (`showNonGitRepositoryWarning` / `isRepositoryBare`).
    warning: Option<PathStatus>,
}

impl AddExistingRepositoryDialog {
    pub fn new(
        state: Entity<AppState>,
        initial: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let path = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("repository path");
            if let Some(p) = &initial {
                s = s.default_value(p.display().to_string());
            }
            s
        });
        cx.observe(&path, |_, _, cx| cx.notify()).detach();
        let handle = path.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Self {
            state,
            path,
            warning: None,
        }
    }

    fn resolved_path(&self, cx: &App) -> Option<PathBuf> {
        let raw = self.path.read(cx).value();
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        let expanded = if let Some(rest) = raw.strip_prefix("~/") {
            dirs_home().join(rest)
        } else {
            PathBuf::from(raw)
        };
        Some(expanded)
    }

    fn status(&self, cx: &App) -> Option<PathStatus> {
        self.resolved_path(cx).map(|p| corvane_git::path_status(&p))
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add Repository".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(p) = paths.into_iter().next()
            {
                this.update_in(cx, |d, window, cx| {
                    d.path
                        .update(cx, |s, cx| s.set_value(p.display().to_string(), window, cx));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let status = self.status(cx);
        match (self.resolved_path(cx), status) {
            (Some(path), Some(PathStatus::Repository)) => {
                Dispatcher::close_popup(cx);
                Dispatcher::add_repository(path, cx);
            }
            (_, status) => {
                self.warning =
                    status.filter(|s| matches!(s, PathStatus::NotARepository | PathStatus::Bare));
                cx.notify();
            }
        }
    }
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

impl Render for AddExistingRepositoryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let has_path = self.resolved_path(cx).is_some();
        let live = corvane_core::AppState::global(cx)
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::ADD_LOCAL_VALIDATES_WHILE_TYPING);
        let current = live.then(|| self.status(cx)).flatten();
        let status = if live {
            current.filter(|s| matches!(s, PathStatus::NotARepository | PathStatus::Bare))
        } else {
            self.warning.filter(|_| has_path)
        };
        let add_disabled = live && current != Some(PathStatus::Repository);
        let error: Option<AnyElement> = match status {
            // `buildNotAGitRepositoryError`: two paragraphs, the second
            // linking "create a repository"
            Some(PathStatus::NotARepository) => Some({
                let path = self.resolved_path(cx);
                div()
                    .flex()
                    .flex_col()
                    .text_color(t.error)
                    .child("This directory does not appear to be a Git repository.")
                    .child(crate::widgets::paragraph(vec![
                        "Would you like to".into(),
                        crate::widgets::link_button("create-instead", "create a repository", cx)
                            .on_click(move |_, _, cx| {
                                Dispatcher::show_popup(
                                    Popup::CreateRepository { path: path.clone() },
                                    cx,
                                )
                            })
                            .into_any_element()
                            .into(),
                        "here instead?".into(),
                    ]))
                    .into_any_element()
            }),
            Some(PathStatus::Bare) => Some(
                div()
                    .text_color(t.error)
                    .child("This directory appears to be a bare repository. Bare repositories are not currently supported.")
                    .into_any_element(),
            ),
            _ => None,
        };

        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let this = cx.entity();
        dialog(
            "add-existing-repository",
            "Add Local Repository",
            div()
                .flex()
                .flex_col()
                .gap(SPACING())
                .child(
                    // `Row`: [Local Path text box][Choose…]
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .gap(SPACING())
                        .child(labeled(
                            "Local Path",
                            text_box("add-existing-path", &self.path, None, window, cx),
                            cx,
                        ))
                        .child(
                            button("add-existing-choose", "Choose…", cx)
                                .flex_none()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.choose(window, cx)),
                                ),
                        ),
                )
                .children(error),
            vec![
                DialogButton {
                    id: "add-existing-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "add-existing-ok",
                    label: "Add Repository".into(),
                    primary: true,
                    disabled: add_disabled,
                    on_click: Box::new(move |_, cx| {
                        this.update(cx, |d, cx| d.submit(cx));
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
