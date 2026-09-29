//! Remote dialogs - GHD `ui/publish-repository/{publish,publish-repository}.tsx`,
//! `ui/push-needs-pull/push-needs-pull-warning.tsx`,
//! `ui/rebase/confirm-force-push.tsx`, `ui/generic-git-auth/generic-git-auth.tsx`
//! and `ui/lfs/initialize-lfs.tsx`.

use corvane_core::{Account, AppState, Dispatcher, RetryAction};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::MenuItem;
use crate::dialog::{DialogButton, DialogKind, dialog, dialog_with_kind};
use crate::icons::{Octicon, octicon};
use crate::tab_bar::{TabModel, tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    Inline, button, checkbox, code_ref, link_button, paragraph, primary_button, text_box,
};

/// GHD `sanitizedRepositoryName`: only `[A-Za-z0-9_.-]`, others become `-`.
pub fn sanitized_repository_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// `Publish`: GitHub.com / GitHub Enterprise tabs, name, description,
/// private checkbox, organisation.
pub struct PublishRepositoryDialog {
    state: Entity<AppState>,
    repo: u64,
    tab: usize,
    name: Entity<InputState>,
    description: Entity<InputState>,
    private: bool,
    org: Option<String>,
    orgs: Vec<String>,
    orgs_loaded_for: Option<String>,
}

impl PublishRepositoryDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial = state
            .read(cx)
            .repository(repo)
            .map(|r| {
                r.alias.clone().unwrap_or_else(|| {
                    r.path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                })
            })
            .unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx));
        name.update(cx, |s, cx| s.set_value(initial, window, cx));
        let description = cx.new(|cx| InputState::new(window, cx));
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        cx.observe(&description, |_, _, cx| cx.notify()).detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let has_dotcom = state.read(cx).accounts.iter().any(is_dotcom);
        let has_enterprise = state.read(cx).accounts.iter().any(|a| !is_dotcom(a));
        Self {
            state,
            repo,
            tab: if !has_dotcom && has_enterprise { 1 } else { 0 },
            name,
            description,
            private: true,
            org: None,
            orgs: Vec::new(),
            orgs_loaded_for: None,
        }
    }

    fn account(&self, cx: &App) -> Option<Account> {
        let dotcom = self.tab == 0;
        self.state
            .read(cx)
            .accounts
            .iter()
            .find(|a| is_dotcom(a) == dotcom)
            .cloned()
    }

    /// `fetchOrgs` for the account picked on the current tab.
    fn load_orgs(&mut self, account: &Account, cx: &mut Context<Self>) {
        if self.orgs_loaded_for.as_deref() == Some(account.endpoint.as_str()) {
            return;
        }
        self.orgs_loaded_for = Some(account.endpoint.clone());
        let Some(token) = corvane_platform::keychain::token(&account.host(), &account.login)
            .ok()
            .flatten()
        else {
            return;
        };
        let endpoint = corvane_github::Endpoint::from_api_base(&account.endpoint);
        let task = cx.background_executor().spawn(async move {
            corvane_github::Client::new(endpoint, token)
                .user_orgs()
                .unwrap_or_default()
        });
        cx.spawn(async move |this, cx| {
            let orgs = task.await;
            this.update(cx, |this, cx| {
                this.orgs = orgs;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn is_dotcom(account: &Account) -> bool {
    account.host() == "github.com"
}

impl Render for PublishRepositoryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let repo = self.repo;
        let account = self.account(cx);
        if let Some(account) = &account {
            self.load_orgs(&account.clone(), cx);
        }
        let t = cx.ghd();
        let publishing = self
            .state
            .read(cx)
            .repo_states
            .get(&repo)
            .is_some_and(|r| r.publishing);
        let raw_name = self.name.read(cx).value().to_string();
        let name = sanitized_repository_name(raw_name.trim());
        let description = self.description.read(cx).value().to_string();
        let tabs = tab_bar(
            vec![
                TabModel {
                    id: "publish-dotcom",
                    label: "GitHub.com".into(),
                    count: None,
                },
                TabModel {
                    id: "publish-enterprise",
                    label: "GitHub Enterprise".into(),
                    count: None,
                },
            ],
            self.tab,
            {
                let weak = cx.weak_entity();
                move |ix, _, cx| {
                    weak.update(cx, |this, cx| {
                        this.tab = ix;
                        this.org = None;
                        this.orgs.clear();
                        this.orgs_loaded_for = None;
                        cx.notify();
                    })
                    .ok();
                }
            },
            cx,
        );
        let body: AnyElement = match &account {
            None => {
                // `renderSignInTab` (`CallToAction`)
                let enterprise = self.tab == 1;
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(SPACING())
                    .child(div().flex_1().child(if enterprise {
                        "If you are using GitHub Enterprise at work, sign in to it to get access to your repositories."
                    } else {
                        "Sign in to your GitHub.com account to access your repositories."
                    }))
                    .child(
                        primary_button("publish-sign-in", "Sign In", false, cx).on_click(
                            move |_, _, cx| {
                                Dispatcher::show_popup(
                                    corvane_core::Popup::SignIn { enterprise },
                                    cx,
                                )
                            },
                        ),
                    )
                    .into_any_element()
            }
            Some(_) => {
                let private = self.private;
                let org_label = self.org.clone().unwrap_or_else(|| "None".to_string());
                let orgs = self.orgs.clone();
                let weak = cx.weak_entity();
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(SPACING_HALF())
                            .child("Name")
                            .child(text_box("publish-name", &self.name, None, window, cx)),
                    )
                    .when(
                        !raw_name.trim().is_empty() && raw_name.trim() != name,
                        |d| {
                            // `.warning-helper-text`
                            d.child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(SPACING_HALF())
                                    .text_size(FONT_SIZE_SM())
                                    .text_color(t.text_secondary)
                                    .child(octicon(Octicon::Alert, t.dialog_warning))
                                    .child(format!("Will be created as {name}")),
                            )
                        },
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(SPACING_HALF())
                            .child("Description")
                            .child(text_box(
                                "publish-description",
                                &self.description,
                                None,
                                window,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .id("publish-private")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF())
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.private = !this.private;
                                cx.notify();
                            }))
                            .child(checkbox("publish-private-box", private, false, cx))
                            .child("Keep this code private"),
                    )
                    .when(!orgs.is_empty(), |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(SPACING_HALF())
                                .child("Organization")
                                .child(
                                    button("publish-org", "", cx)
                                        .w_full()
                                        .justify_between()
                                        .child(org_label)
                                        .child(octicon(
                                            Octicon::TriangleDown,
                                            t.secondary_button_text,
                                        ))
                                        .on_click(move |ev: &ClickEvent, window, cx| {
                                            let mut items = vec![MenuItem::new("None", {
                                                let weak = weak.clone();
                                                move |_, cx| {
                                                    weak.update(cx, |this, cx| {
                                                        this.org = None;
                                                        cx.notify();
                                                    })
                                                    .ok();
                                                }
                                            })];
                                            for org in &orgs {
                                                let org = org.clone();
                                                let weak = weak.clone();
                                                items.push(MenuItem::new(
                                                    org.clone(),
                                                    move |_, cx| {
                                                        let org = org.clone();
                                                        weak.update(cx, |this, cx| {
                                                            this.org = Some(org);
                                                            cx.notify();
                                                        })
                                                        .ok();
                                                    },
                                                ));
                                            }
                                            let position = ev.mouse_position().unwrap_or_default();
                                            #[cfg(target_os = "macos")]
                                            crate::native_menu::show_context_menu(
                                                items, position, window, cx,
                                            );
                                            #[cfg(not(target_os = "macos"))]
                                            let _ = (items, position, window, cx);
                                        }),
                                ),
                        )
                    })
                    .into_any_element()
            }
        };
        let content = div()
            .w(zpx(450.))
            .mx(zpx(-20.))
            .mt(zpx(-20.))
            .flex()
            .flex_col()
            .child(tabs)
            .child(div().p(SPACING_DOUBLE()).child(body));
        let mut buttons = vec![DialogButton {
            id: "publish-cancel",
            label: "Cancel".into(),
            primary: false,
            disabled: false,
            on_click: Box::new(close),
        }];
        if let Some(account) = account {
            let disabled = name.is_empty() || publishing;
            let org = self.org.clone();
            let private = self.private;
            buttons.push(DialogButton {
                id: "publish-ok",
                label: if publishing {
                    "Publishing…".into()
                } else {
                    "Publish Repository".into()
                },
                primary: true,
                disabled,
                on_click: Box::new(move |_, cx| {
                    if disabled {
                        return;
                    }
                    Dispatcher::publish_repository(
                        repo,
                        name.clone(),
                        description.clone(),
                        private,
                        account.clone(),
                        org.clone(),
                        cx,
                    );
                }),
            });
        }
        dialog(
            "dialog-publish-repository",
            "Publish Repository",
            content,
            buttons,
            close,
            window,
            cx,
        )
    }
}

// ---------------------------------------------------------------------------

/// `PushNeedsPullWarning`
pub struct PushNeedsPullDialog {
    repo: u64,
}

impl PushNeedsPullDialog {
    pub fn new(repo: u64) -> Self {
        Self { repo }
    }
}

impl Render for PushNeedsPullDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let repo = self.repo;
        dialog_with_kind(
            "dialog-push-needs-pull",
            DialogKind::Warning,
            "Newer Commits on Remote",
            div().w(zpx(450.)).child(
                "Corvane is unable to push commits to this branch because there are commits on the remote that are not present on your local branch. Fetch these new commits before pushing in order to reconcile them with your local commits.",
            ),
            vec![
                DialogButton {
                    id: "needs-pull-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "needs-pull-fetch",
                    label: "Fetch".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::fetch(repo, false, cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

// ---------------------------------------------------------------------------

/// `ConfirmForcePush`
pub struct ConfirmForcePushDialog {
    repo: u64,
    upstream: String,
    dont_ask_again: bool,
}

impl ConfirmForcePushDialog {
    pub fn new(repo: u64, upstream: String) -> Self {
        Self {
            repo,
            upstream,
            dont_ask_again: false,
        }
    }
}

impl Render for ConfirmForcePushDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (repo, dont_ask) = (self.repo, self.dont_ask_again);
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(paragraph(vec![
                "A force push will rewrite history on ".into(),
                code_ref(self.upstream.clone(), cx).into_any_element().into(),
                ". Any collaborators working on this branch will need to reset their own local branch to match the history of the remote.".into(),
            ]))
            .child(
                div()
                    .id("force-push-dont-ask")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dont_ask_again = !this.dont_ask_again;
                        cx.notify();
                    }))
                    .child(checkbox("force-push-dont-ask-box", dont_ask, false, cx))
                    .child("Do not show this message again"),
            );
        dialog_with_kind(
            "dialog-confirm-force-push",
            DialogKind::Warning,
            "Are you sure you want to force push?",
            content,
            vec![
                DialogButton {
                    id: "force-push-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "force-push-ok",
                    label: "I'm sure".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        if dont_ask {
                            Dispatcher::update_settings(cx, |s| s.confirm_force_push = false);
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::push(repo, true, None, cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

// ---------------------------------------------------------------------------

/// `GenericGitAuthentication`
pub struct GenericGitAuthDialog {
    repo: u64,
    remote_url: String,
    host: String,
    fixed_username: Option<String>,
    retry: RetryAction,
    username: Entity<InputState>,
    password: Entity<InputState>,
}

impl GenericGitAuthDialog {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: u64,
        remote_url: String,
        host: String,
        username: Option<String>,
        retry: RetryAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let username_state = cx.new(|cx| InputState::new(window, cx));
        if let Some(u) = &username {
            username_state.update(cx, |s, cx| s.set_value(u.clone(), window, cx));
        }
        let password = cx.new(|cx| InputState::new(window, cx));
        cx.observe(&username_state, |_, _, cx| cx.notify()).detach();
        cx.observe(&password, |_, _, cx| cx.notify()).detach();
        Self {
            repo,
            remote_url,
            host,
            fixed_username: username,
            retry,
            username: username_state,
            password,
        }
    }
}

impl Render for GenericGitAuthDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let username = self
            .fixed_username
            .clone()
            .unwrap_or_else(|| self.username.read(cx).value().trim().to_string());
        let password = self.password.read(cx).value().to_string();
        let disabled = username.is_empty() || password.is_empty();
        let (repo, host, retry) = (self.repo, self.host.clone(), self.retry.clone());
        let mono = |text: String| {
            div()
                .font_family(crate::theme::MONO_FONT)
                .px(zpx(3.))
                .rounded(zpx(3.))
                .bg(t.box_alt_background)
                .child(text)
        };
        let content = div()
            .w(zpx(450.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child({
                let mut parts: Vec<Inline> = vec![
                    "We were unable to authenticate with ".into(),
                    mono(self.remote_url.clone()).into_any_element().into(),
                ];
                match &self.fixed_username {
                    Some(u) => {
                        parts.push(". Please enter the password for the user ".into());
                        parts.push(mono(u.clone()).into_any_element().into());
                        parts.push(" to try again.".into());
                    }
                    None => parts
                        .push(". Please enter your username and password to try again.".into()),
                }
                paragraph(parts)
            })
            .when(self.fixed_username.is_none(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(SPACING_HALF())
                        .child("Username")
                        .child(text_box("auth-username", &self.username, None, window, cx)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF())
                    .child("Password")
                    .child(text_box("auth-password", &self.password, None, window, cx)),
            )
            .child(
                paragraph(vec![
                    "Depending on your repository's hosting service, you might need to use a Personal Access Token (PAT) as your password. Learn more about creating a PAT in the ".into(),
                    link_button("auth-docs", "integration docs", cx)
                        .text_size(FONT_SIZE_SM())
                        .on_click(|_, _, cx| {
                            Dispatcher::open_url(
                                "https://github.com/desktop/desktop/tree/development/docs/integrations",
                                cx,
                            )
                        })
                        .into_any_element()
                        .into(),
                    ".".into(),
                ])
                .text_size(FONT_SIZE_SM())
                .text_color(t.text_secondary),
            );
        dialog(
            "dialog-generic-git-auth",
            "Authentication Failed",
            content,
            vec![
                DialogButton {
                    id: "auth-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "auth-save",
                    label: "Save and Retry".into(),
                    primary: true,
                    disabled,
                    on_click: Box::new(move |_, cx| {
                        if disabled {
                            return;
                        }
                        Dispatcher::save_generic_credentials(
                            host.clone(),
                            username.clone(),
                            password.clone(),
                            repo,
                            retry.clone(),
                            cx,
                        );
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}

// ---------------------------------------------------------------------------

/// `InitializeLFS`
pub struct InitializeLfsDialog {
    state: Entity<AppState>,
    repos: Vec<u64>,
}

impl InitializeLfsDialog {
    pub fn new(state: Entity<AppState>, repos: Vec<u64>) -> Self {
        Self { state, repos }
    }
}

impl Render for InitializeLfsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let repos = self.repos.clone();
        let paths: Vec<String> = {
            let s = self.state.read(cx);
            repos
                .iter()
                .filter_map(|id| s.repository(*id))
                .map(|r| r.path.to_string_lossy().to_string())
                .collect()
        };
        let plural = repos.len() != 1;
        let content = div()
            .w(zpx(450.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(paragraph(vec![
                if plural {
                    "The repositories use "
                } else {
                    "This repository uses "
                }
                .into(),
                link_button("lfs-link", "Git LFS", cx)
                    .on_click(|_, _, cx| Dispatcher::open_url("https://git-lfs.github.com/", cx))
                    .into_any_element()
                    .into(),
                format!(
                    ". To contribute to {}, Git LFS must first be initialized. Would you like to do so now?",
                    if plural { "them" } else { "it" }
                )
                .into(),
            ]))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(zpx(2.))
                    .font_family(crate::theme::MONO_FONT)
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .children(paths.into_iter().map(|p| div().truncate().child(p))),
            );
        dialog(
            "dialog-initialize-lfs",
            "Initialize Git LFS",
            content,
            vec![
                DialogButton {
                    id: "lfs-not-now",
                    label: "Not Now".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "lfs-init",
                    label: "Initialize Git LFS".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::install_lfs_hooks(repos.clone(), cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
