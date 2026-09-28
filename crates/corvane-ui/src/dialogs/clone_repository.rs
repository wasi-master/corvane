//! "Clone a Repository" (`ui/clone-repository/clone-repository.tsx`):
//! tabs GitHub.com | GitHub Enterprise | URL. The account tabs need sign-in
//! (next step); the URL tab is fully functional.

use std::path::PathBuf;

use corvane_core::{AppState, Dispatcher, Popup};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::tab_bar::{TabModel, tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, labeled, primary_button, text_box};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    DotCom,
    Enterprise,
    Url,
}

pub struct CloneRepositoryDialog {
    state: Entity<AppState>,
    tab: Tab,
    url: Entity<InputState>,
    path: Entity<InputState>,
    /// The user edited the path by hand; stop deriving it from the URL.
    path_edited: bool,
    last_derived: String,
}

impl CloneRepositoryDialog {
    pub fn new(
        state: Entity<AppState>,
        initial_url: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let clone_dir = state
            .read(cx)
            .settings
            .clone_dir
            .clone()
            .unwrap_or_else(corvane_platform::paths::default_clone_dir);
        let url = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("URL or username/repository");
            if let Some(u) = &initial_url {
                s = s.default_value(u.clone());
            }
            s
        });
        let path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("repository path")
                .default_value(clone_dir.display().to_string())
        });
        cx.observe_in(&url, window, |this, _, window, cx| {
            this.derive_path(window, cx);
            cx.notify()
        })
        .detach();
        cx.observe(&path, |_, _, cx| cx.notify()).detach();
        let handle = url.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        let tab = if initial_url.is_some() {
            Tab::Url
        } else {
            Tab::DotCom
        };
        Self {
            state,
            tab,
            url,
            path,
            path_edited: false,
            last_derived: clone_dir.display().to_string(),
        }
    }

    /// GHD keeps `<clone dir>/<repo name>` in sync with the URL until edited.
    fn derive_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.path.read(cx).value().to_string();
        if current != self.last_derived {
            self.path_edited = true;
        }
        if self.path_edited {
            return;
        }
        let base = self
            .state
            .read(cx)
            .settings
            .clone_dir
            .clone()
            .unwrap_or_else(corvane_platform::paths::default_clone_dir);
        let url = self.url.read(cx).value().to_string();
        let derived = match corvane_git::normalize_clone_url(&url)
            .and_then(|u| corvane_git::repository_name_from_url(&u))
        {
            Some(name) => base.join(name),
            None => base,
        }
        .display()
        .to_string();
        if derived != current {
            self.last_derived = derived.clone();
            self.path
                .update(cx, |s, cx| s.set_value(derived, window, cx));
        }
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Clone".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(p) = paths.into_iter().next()
            {
                this.update_in(cx, |d, window, cx| {
                    d.path_edited = true;
                    d.path
                        .update(cx, |s, cx| s.set_value(p.display().to_string(), window, cx));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn clone_target(&self, cx: &App) -> Option<(String, PathBuf)> {
        let url = corvane_git::normalize_clone_url(&self.url.read(cx).value())?;
        let path = self.path.read(cx).value().trim().to_string();
        if path.is_empty() {
            return None;
        }
        Some((url, PathBuf::from(path)))
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if let Some((url, path)) = self.clone_target(cx) {
            Dispatcher::clone_repository(url, path, cx);
        }
    }

    fn url_tab(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .flex()
            .flex_col()
            .gap(SPACING)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_THIRD)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child("Repository URL or GitHub username and repository")
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .child("(")
                                    .child(
                                        div()
                                            .font_family(crate::theme::MONO_FONT)
                                            .px(px(3.))
                                            .rounded(px(3.))
                                            .bg(t.box_alt_background)
                                            .child("hubot/cool-repo"),
                                    )
                                    .child(")"),
                            ),
                    )
                    .child(text_box("clone-url", &self.url, None, window, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap(SPACING)
                    .child(labeled(
                        "Local Path",
                        text_box("clone-path", &self.path, None, window, cx),
                        cx,
                    ))
                    .child(
                        button("clone-choose", "Choose…", cx)
                            .flex_none()
                            .on_click(cx.listener(|this, _, window, cx| this.choose(window, cx))),
                    ),
            )
    }

    fn account_tab(&self, enterprise: bool, cx: &Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let signed_in = self
            .state
            .read(cx)
            .accounts
            .iter()
            .any(|a| a.is_dotcom() != enterprise);
        let enterprise_flag = enterprise;
        let host = if enterprise {
            "GitHub Enterprise"
        } else {
            "GitHub.com"
        };
        div()
            .flex()
            .flex_col()
            .gap(SPACING)
            .py(SPACING)
            .child(if signed_in {
                div()
                    .text_color(t.text_secondary)
                    .child(
                        "Your repositories will appear here once the GitHub layer lands.",
                    )
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(SPACING)
                    .child(format!(
                        "Sign in to your {host} account to access your repositories."
                    ))
                    .child(
                        primary_button("clone-sign-in", "Sign In", false, cx).on_click(
                            move |_, _, cx| {
                                Dispatcher::show_popup(
                                    Popup::SignIn {
                                        enterprise: enterprise_flag,
                                    },
                                    cx,
                                )
                            },
                        ),
                    )
                    .into_any_element()
            })
    }
}

impl Render for CloneRepositoryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_clone = self.tab == Tab::Url && self.clone_target(cx).is_some();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let this = cx.entity();
        let selected = match self.tab {
            Tab::DotCom => 0,
            Tab::Enterprise => 1,
            Tab::Url => 2,
        };
        let tabs_entity = cx.entity();
        let body: AnyElement = match self.tab {
            Tab::DotCom => self.account_tab(false, cx).into_any_element(),
            Tab::Enterprise => self.account_tab(true, cx).into_any_element(),
            Tab::Url => self.url_tab(window, cx).into_any_element(),
        };

        dialog(
            "clone-repository",
            "Clone a Repository",
            div()
                .flex()
                .flex_col()
                .w(px(560.))
                .child(div().mb(SPACING).child(tab_bar(
                    vec![
                        TabModel {
                            id: "clone-tab-dotcom",
                            label: "GitHub.com".into(),
                            count: None,
                        },
                        TabModel {
                            id: "clone-tab-enterprise",
                            label: "GitHub Enterprise".into(),
                            count: None,
                        },
                        TabModel {
                            id: "clone-tab-url",
                            label: "URL".into(),
                            count: None,
                        },
                    ],
                    selected,
                    move |ix, _, cx| {
                        tabs_entity.update(cx, |d, cx| {
                            d.tab = match ix {
                                0 => Tab::DotCom,
                                1 => Tab::Enterprise,
                                _ => Tab::Url,
                            };
                            cx.notify();
                        })
                    },
                    cx,
                )))
                .child(body),
            vec![
                DialogButton {
                    id: "clone-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "clone-ok",
                    label: "Clone".into(),
                    primary: true,
                    on_click: Box::new(move |_, cx| {
                        if can_clone {
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
