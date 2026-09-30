//! "Create a New Repository" (`ui/add-repository/create-repository.tsx`):
//! name, description, path, README, and the bundled Git Ignore / License
//! templates (`corvane_core::templates`).
//!
//! Deviation (`224-alias-when-adding`): an optional Alias field names the
//! new repository in the list (GHD: Create Alias afterwards).
//!
//! Deviation (`220-create-repository-in-folder`): an "in this folder"
//! checkbox creates the repository in the Local Path folder itself (GHD
//! always adds a `<name>` subfolder); files already there (README.md,
//! .gitignore, LICENSE) are kept rather than replaced.
//!
//! Deviation (flag `readme-overwrite-warning`): with the README box ticked
//! and a `README.md` already in the target folder, the dialog shows the
//! overwrite warning GHD keeps behind `enableReadmeOverwriteWarning()` (beta
//! builds only; `renderReadmeOverwriteWarning` in create-repository.tsx).

use std::path::PathBuf;

use corvane_core::{AppState, Dispatcher};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog_with_footer_message};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    SelectHandler, SelectItem, button, checkbox, labeled, select_button_items, text_box,
};

pub struct CreateRepositoryDialog {
    state: Entity<AppState>,
    name: Entity<InputState>,
    description: Entity<InputState>,
    path: Entity<InputState>,
    readme: bool,
    /// Selected `.gitignore` template name (`NoGitIgnoreValue` = None).
    gitignore: Option<String>,
    /// Selected license name (`NoLicenseValue` = None).
    license: Option<String>,
    gitignore_names: Vec<String>,
    licenses: Vec<corvane_core::templates::License>,
    /// `220-create-repository-in-folder`: create in Local Path itself.
    in_folder: bool,
    /// `224-alias-when-adding`.
    alias: Entity<InputState>,
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
            gitignore: None,
            license: None,
            gitignore_names: corvane_core::templates::gitignore_names(),
            licenses: corvane_core::templates::licenses(),
            in_folder: false,
            alias: cx.new(|cx| InputState::new(window, cx).placeholder("optional")),
        }
    }

    /// The "in this folder" checkbox is ticked and the flag is on.
    fn in_folder(&self, cx: &App) -> bool {
        self.in_folder
            && self
                .state
                .read(cx)
                .flags
                .bool(corvane_core::flags::ids::CREATE_REPOSITORY_IN_FOLDER)
    }

    fn full_path(&self, cx: &App) -> Option<PathBuf> {
        let name = self.name.read(cx).value().trim().to_string();
        let dir = self.path.read(cx).value().trim().to_string();
        if name.is_empty() || dir.is_empty() {
            return None;
        }
        if self.in_folder(cx) {
            return Some(PathBuf::from(dir));
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
            prompt: Some(mac_or("Create Repository", "Create repository").into()),
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
        let name = self.name.read(cx).value().trim().to_string();
        if self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::ALIAS_WHEN_ADDING)
        {
            let alias = self.alias.read(cx).value().to_string();
            Dispatcher::alias_when_added(&path, alias, cx);
        }
        Dispatcher::create_repository(
            path,
            name,
            (!description.is_empty()).then_some(description),
            self.readme,
            self.gitignore.clone(),
            self.license.clone(),
            self.in_folder(cx),
            cx,
        );
    }

    /// `renderGitIgnores`: "None" + every bundled template.
    fn gitignore_select(&self, cx: &Context<Self>) -> impl IntoElement {
        let names = self.gitignore_names.clone();
        let selected = self
            .gitignore
            .as_ref()
            .and_then(|g| names.iter().position(|n| n == g))
            .map(|ix| ix + 1)
            .unwrap_or(0);
        let mut items: Vec<SelectItem> = vec![SelectItem::Option("None".into())];
        items.extend(names.iter().map(|n| SelectItem::Option(n.clone().into())));
        let weak = cx.weak_entity();
        let on_select: SelectHandler = std::rc::Rc::new(move |ix, _, cx| {
            let pick = if ix == 0 {
                None
            } else {
                names.get(ix - 1).cloned()
            };
            weak.update(cx, |this, cx| {
                this.gitignore = pick;
                cx.notify();
            })
            .ok();
        });
        select_button_items(
            "create-gitignore",
            self.gitignore.clone().unwrap_or_else(|| "None".to_string()),
            items,
            Some(selected),
            false,
            on_select,
            cx,
        )
    }

    /// `renderLicenses`: "None", the featured licenses, a separator, the rest.
    fn license_select(&self, cx: &Context<Self>) -> impl IntoElement {
        let names: Vec<String> = self.licenses.iter().map(|l| l.name.clone()).collect();
        let featured = self.licenses.iter().filter(|l| l.featured).count();
        let selected = self
            .license
            .as_ref()
            .and_then(|l| names.iter().position(|n| n == l))
            .map(|ix| ix + 1)
            .unwrap_or(0);
        let mut items: Vec<SelectItem> = vec![SelectItem::Option("None".into())];
        for (ix, name) in names.iter().enumerate() {
            if ix == featured && featured > 0 {
                items.push(SelectItem::Separator);
            }
            items.push(SelectItem::Option(name.clone().into()));
        }
        let weak = cx.weak_entity();
        let on_select: SelectHandler = std::rc::Rc::new(move |ix, _, cx| {
            let pick = if ix == 0 {
                None
            } else {
                names.get(ix - 1).cloned()
            };
            weak.update(cx, |this, cx| {
                this.license = pick;
                cx.notify();
            })
            .ok();
        });
        select_button_items(
            "create-license",
            self.license.clone().unwrap_or_else(|| "None".to_string()),
            items,
            Some(selected),
            false,
            on_select,
            cx,
        )
    }
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
        let in_folder_option = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::CREATE_REPOSITORY_IN_FOLDER);
        // `renderReadmeOverwriteWarning`
        let readme_warning = readme
            && self
                .state
                .read(cx)
                .flags
                .bool(corvane_core::flags::ids::README_OVERWRITE_WARNING)
            && full.as_deref().is_some_and(corvane_git::readme_exists);
        // `renderPathMessage`: "The repository will be created at <Ref>…</Ref>."
        let path_message = full.as_ref().filter(|_| !exists_as_repo).map(|path| {
            let path = path.display().to_string();
            let before = "The repository will be created at ";
            let text = format!("{before}{path}.");
            let style = window.text_style();
            let mut mono = style.clone();
            mono.font_family = crate::theme::mono_font().into();
            let mut path_run = mono.to_run(path.len());
            path_run.background_color = Some(t.path_segment_background);
            StyledText::new(text)
                .with_runs(vec![style.to_run(before.len()), path_run, style.to_run(1)])
                .into_any_element()
        });

        dialog_with_footer_message(
            "create-repository",
            mac_or("Create a New Repository", "Create a new repository"),
            div()
                .flex()
                .flex_col()
                .gap(SPACING())
                // `dialog#create-repository { width: 400px }` less border and padding
                .w(zpx(358.))
                .child(labeled(
                    "Name",
                    text_box("create-name", &self.name, None, window, cx),
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .gap(SPACING())
                        .child(labeled(
                            mac_or("Local Path", "Local path"),
                            text_box("create-path", &self.path, None, window, cx),
                            cx,
                        ))
                        .child(
                            button("create-choose", "Choose…", cx).flex_none().on_click(
                                cx.listener(|this, _, window, cx| this.choose(window, cx)),
                            ),
                        ),
                )
                .when(in_folder_option, |d| {
                    let weak = cx.weak_entity();
                    d.child(crate::widgets::checkbox_row(
                        "create-in-folder",
                        self.in_folder,
                        "Create the repository in this folder (no subfolder)",
                        move |checked, _, cx| {
                            weak.update(cx, |this, cx| {
                                this.in_folder = checked;
                                cx.notify();
                            })
                            .ok();
                        },
                        cx,
                    ))
                })
                .when(exists_as_repo, |d| {
                    let path = full.clone();
                    d.child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(zpx(4.))
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
                .when(
                    self.state
                        .read(cx)
                        .flags
                        .bool(corvane_core::flags::ids::ALIAS_WHEN_ADDING),
                    |d| {
                        d.child(labeled(
                            "Alias",
                            text_box("create-alias", &self.alias, None, window, cx),
                            cx,
                        ))
                    },
                )
                .child(labeled(
                    "Description",
                    text_box("create-description", &self.description, None, window, cx),
                    cx,
                ))
                .child(
                    div()
                        .id("create-readme")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF())
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.readme = !this.readme;
                            cx.notify();
                        }))
                        .child(checkbox("create-readme-box", readme, false, cx))
                        .child("Initialize this repository with a README"),
                )
                .when(readme_warning, |d| {
                    // `.warning-helper-text`
                    d.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(SPACING_HALF())
                            .text_size(FONT_SIZE_SM())
                            .text_color(t.text_secondary)
                            .child(
                                octicon(Octicon::Alert, t.dialog_warning)
                                    .flex_none()
                                    .mt(zpx(1.)),
                            )
                            .child(
                                "This directory contains a README.md file already. Checking \
                                 this box will result in the existing content being replaced.",
                            ),
                    )
                })
                .child(labeled(
                    mac_or("Git Ignore", "Git ignore"),
                    self.gitignore_select(cx),
                    cx,
                ))
                .child(labeled("License", self.license_select(cx), cx)),
            path_message,
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
                    label: mac_or("Create Repository", "Create repository").into(),
                    primary: true,
                    // `fullPath === null || creating || isRepository`
                    disabled: !can_create,
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
