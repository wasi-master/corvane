//! "Create a New Repository" (`ui/add-repository/create-repository.tsx`).
//! Git-ignore and license templates are not bundled yet; the
//! selects render with "None" so the layout matches.

use std::path::PathBuf;

use corvane_core::{AppState, Dispatcher};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, checkbox, labeled, text_box};

pub struct CreateRepositoryDialog {
    #[allow(dead_code)]
    state: Entity<AppState>,
    name: Entity<InputState>,
    description: Entity<InputState>,
    path: Entity<InputState>,
    readme: bool,
}

impl CreateRepositoryDialog {
    pub fn new(
        state: Entity<AppState>,
        initial_path: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let default_dir = state
            .read(cx)
            .settings
            .clone_dir
            .clone()
            .unwrap_or_else(corvane_platform::paths::default_clone_dir);
        let (initial_name, initial_dir) = match initial_path {
            Some(p) => (
                p.file_name().map(|n| n.to_string_lossy().into_owned()),
                p.parent().map(|d| d.to_path_buf()).unwrap_or(default_dir),
            ),
            None => (None, default_dir),
        };
        let name = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("repository name");
            if let Some(n) = initial_name {
                s = s.default_value(n);
            }
            s
        });
        let description = cx.new(|cx| InputState::new(window, cx));
        let path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("repository path")
                .default_value(initial_dir.display().to_string())
        });
        for e in [&name, &description, &path] {
            cx.observe(e, |_, _, cx| cx.notify()).detach();
        }
        let handle = name.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Self {
            state,
            name,
            description,
            path,
            readme: false,
        }
    }

    fn full_path(&self, cx: &App) -> Option<PathBuf> {
        let name = self.name.read(cx).value().trim().to_string();
        let dir = self.path.read(cx).value().trim().to_string();
        if name.is_empty() || dir.is_empty() {
            return None;
        }
        // GHD sanitises the folder name (`sanitizedRepositoryName`)
        let folder: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        Some(PathBuf::from(dir).join(folder))
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Create Repository".into()),
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
        let Some(path) = self.full_path(cx) else {
            return;
        };
        let description = self.description.read(cx).value().trim().to_string();
        Dispatcher::create_repository(
            path,
            (!description.is_empty()).then_some(description),
            self.readme,
            cx,
        );
    }
}

fn select_placeholder(id: &'static str, value: &'static str, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    div()
        .id(id)
        .h(TEXT_FIELD_HEIGHT)
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px(SPACING_HALF)
        .border_1()
        .rounded(BORDER_RADIUS)
        .bg(t.box_background)
        .border_color(t.box_border_contrast)
        .child(value)
        .child(octicon(Octicon::TriangleDown, t.text_secondary).size(px(12.)))
}

impl Render for CreateRepositoryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let full = self.full_path(cx);
        let exists_as_repo = full
            .as_ref()
            .map(|p| corvane_git::path_status(p) == corvane_git::PathStatus::Repository)
            .unwrap_or(false);
        let can_create = full.is_some() && !exists_as_repo;
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let this = cx.entity();
        let readme = self.readme;

        dialog(
            "create-repository",
            "Create a New Repository",
            div()
                .flex()
                .flex_col()
                .gap(SPACING)
                .w(px(560.))
                .child(labeled(
                    "Name",
                    text_box("create-name", &self.name, None, window, cx),
                    cx,
                ))
                .child(labeled(
                    "Description",
                    text_box("create-description", &self.description, None, window, cx),
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .gap(SPACING)
                        .child(labeled(
                            "Local Path",
                            text_box("create-path", &self.path, None, window, cx),
                            cx,
                        ))
                        .child(
                            button("create-choose", "Choose…", cx).flex_none().on_click(
                                cx.listener(|this, _, window, cx| this.choose(window, cx)),
                            ),
                        ),
                )
                .when(exists_as_repo, |d| {
                    let path = full.clone();
                    d.child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(4.))
                            .text_color(t.text_secondary)
                            .child(format!(
                                "The directory {} appears to be a Git repository.",
                                path.as_ref()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_default()
                            ))
                            .child(
                                div()
                                    .id("add-instead")
                                    .text_color(t.link)
                                    .cursor_pointer()
                                    .child("Would you like to add this repository instead?")
                                    .on_click(move |_, _, cx| {
                                        if let Some(p) = path.clone() {
                                            Dispatcher::close_popup(cx);
                                            Dispatcher::add_repository(p, cx);
                                        }
                                    }),
                            ),
                    )
                })
                .child(
                    div()
                        .id("create-readme")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.readme = !this.readme;
                            cx.notify();
                        }))
                        .child(checkbox("create-readme-box", readme, false, cx))
                        .child("Initialize this repository with a README"),
                )
                .child(labeled(
                    "Git Ignore",
                    select_placeholder("create-gitignore", "None", cx),
                    cx,
                ))
                .child(labeled(
                    "License",
                    select_placeholder("create-license", "None", cx),
                    cx,
                )),
            vec![
                DialogButton {
                    id: "create-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "create-ok",
                    label: "Create Repository".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if can_create {
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
