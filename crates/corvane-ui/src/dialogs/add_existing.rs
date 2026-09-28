//! "Add Local Repository" (`ui/add-repository/add-existing-repository.tsx`).

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
        Self { state, path }
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
        if let (Some(path), Some(PathStatus::Repository)) =
            (self.resolved_path(cx), self.status(cx))
        {
            Dispatcher::close_popup(cx);
            Dispatcher::add_repository(path, cx);
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
        let status = self.status(cx);
        let can_add = status == Some(PathStatus::Repository);
        let error: Option<AnyElement> = match status {
            Some(PathStatus::NotARepository) => Some(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(4.))
                    .text_color(t.error)
                    .child("This directory does not appear to be a Git repository.")
                    .child({
                        let path = self.resolved_path(cx);
                        div()
                            .id("create-instead")
                            .text_color(t.link)
                            .cursor_pointer()
                            .child("Would you like to create a repository here instead?")
                            .on_click(move |_, _, cx| {
                                Dispatcher::show_popup(
                                    Popup::CreateRepository { path: path.clone() },
                                    cx,
                                )
                            })
                    })
                    .into_any_element(),
            ),
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
                .gap(SPACING)
                .w(px(560.))
                .child(
                    // `Row`: [Local Path text box][Choose…]
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .gap(SPACING)
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
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "add-existing-ok",
                    label: "Add Repository".into(),
                    primary: true,
                    on_click: Box::new(move |_, cx| {
                        if can_add {
                            this.update(cx, |d, cx| d.submit(cx));
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
