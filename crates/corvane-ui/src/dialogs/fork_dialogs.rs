//! GHD `ui/forks/create-fork-dialog.tsx` (`styles/ui/dialogs/_create-fork.scss`)
//! and `ui/choose-fork-settings/choose-fork-settings-dialog.tsx`
//! (`_fork-settings.scss`), plus the `ForkSettingsDescription` list shared
//! with Repository Settings › Fork Behavior; also
//! `ui/upstream-already-exists/upstream-already-exists.tsx`.

use corvane_core::{AppState, Dispatcher, ForkContributionTarget, GitHubRepository};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog, dialog_with_kind};
use crate::dialogs::branch_dialogs::ref_chip;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{Inline, link_button, paragraph, segmented_option};

/// `ForkSettingsDescription`
pub fn fork_settings_description(
    github: &GitHubRepository,
    target: ForkContributionTarget,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let name = match (target, &github.parent) {
        (ForkContributionTarget::Parent, Some(parent)) => parent.full_name(),
        _ => github.full_name(),
    };
    let item = |lead: &'static str, tail: &'static str| {
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(3.))
            .child("•")
            .child(lead)
            .child(div().font_weight(FontWeight::SEMIBOLD).child(name.clone()))
            .child(tail)
    };
    div()
        .mt(SPACING)
        .flex()
        .flex_col()
        .text_size(FONT_SIZE_SM)
        .text_color(t.text_secondary)
        .child(item(
            "Pull requests targeting",
            "will be shown in the pull request list.",
        ))
        .child(item("Issues will be created in", "."))
        .child(item("\"View on GitHub\" will open", "in the browser."))
        .child(item("New branches will be based on", "'s default branch."))
        .child(item(
            "Autocompletion of user and issues will be based on",
            ".",
        ))
        .into_any_element()
}

pub struct CreateForkDialog {
    state: Entity<AppState>,
    repo: u64,
    loading: bool,
    error: Option<String>,
}

impl CreateForkDialog {
    pub fn new(state: Entity<AppState>, repo: u64, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            repo,
            loading: false,
            error: None,
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        cx.notify();
        let weak = cx.weak_entity();
        Dispatcher::create_fork(
            self.repo,
            move |error, cx| {
                weak.update(cx, |this, cx| {
                    this.loading = false;
                    this.error = error;
                    cx.notify();
                })
                .ok();
            },
            cx,
        );
    }
}

impl Render for CreateForkDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (github, login) = {
            let s = self.state.read(cx);
            let github = s.repository(self.repo).and_then(|r| r.github.clone());
            let login = github
                .as_ref()
                .and_then(|gh| s.account_for(&gh.endpoint))
                .map(|a| a.login.clone())
                .unwrap_or_default();
            (github, login)
        };
        let Some(github) = github else {
            return div().into_any_element();
        };
        let fork_name = format!("{login}/{}", github.name);
        let bold = |text: String| {
            Inline::Element(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(text)
                    .into_any_element(),
            )
        };
        let content: AnyElement = match &self.error {
            None => div()
                .w(px(460.))
                .flex()
                .flex_col()
                .gap(SPACING)
                .child(paragraph(vec![
                    "It looks like you don’t have write access to ".into(),
                    bold(github.full_name()),
                    ". If you should, please check with a repository administrator.".into(),
                ]))
                .child(paragraph(vec![
                    "Do you want to create a fork of this repository at ".into(),
                    bold(fork_name.clone()),
                    " to continue?".into(),
                ]))
                .into_any_element(),
            Some(error) => div()
                .w(px(460.))
                .flex()
                .flex_col()
                .gap(SPACING)
                .child(paragraph(vec![
                    "Creating your fork ".into(),
                    bold(fork_name.clone()),
                    " failed. You can try ".into(),
                    Inline::Element(
                        link_button(
                            "create-fork-manually",
                            "creating the fork manually on GitHub",
                            cx,
                        )
                        .on_click({
                            let url = github.html_url.clone();
                            move |_, _, cx| cx.open_url(&url)
                        })
                        .into_any_element(),
                    ),
                    ".".into(),
                ]))
                .child(
                    div()
                        .mt(SPACING)
                        .p(SPACING)
                        .rounded(BORDER_RADIUS)
                        .bg(t.box_alt_background)
                        .font_family(crate::theme::MONO_FONT)
                        .text_size(FONT_SIZE_SM)
                        .child(error.clone()),
                )
                .into_any_element(),
        };
        let weak = cx.weak_entity();
        let buttons = if self.error.is_some() {
            vec![DialogButton {
                id: "create-fork-close",
                label: "Close".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(close),
            }]
        } else {
            vec![
                DialogButton {
                    id: "create-fork-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: self.loading,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "create-fork-ok",
                    label: "Fork This Repository".into(),
                    primary: true,
                    disabled: self.loading,
                    on_click: Box::new(move |_, cx| {
                        weak.update(cx, |this, cx| this.submit(cx)).ok();
                    }),
                },
            ]
        };
        if self.error.is_some() {
            dialog_with_kind(
                "create-fork",
                DialogKind::Error,
                "Do you want to fork this repository?",
                content,
                buttons,
                close,
                window,
                cx,
            )
            .into_any_element()
        } else {
            dialog(
                "create-fork",
                "Do you want to fork this repository?",
                content,
                buttons,
                close,
                window,
                cx,
            )
            .into_any_element()
        }
    }
}

pub struct ChooseForkSettingsDialog {
    state: Entity<AppState>,
    repo: u64,
    target: ForkContributionTarget,
}

impl ChooseForkSettingsDialog {
    pub fn new(state: Entity<AppState>, repo: u64, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let target = state
            .read(cx)
            .repository(repo)
            .map(|r| r.fork_contribution_target())
            .unwrap_or_default();
        Self {
            state,
            repo,
            target,
        }
    }
}

impl Render for ChooseForkSettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let Some(github) = self
            .state
            .read(cx)
            .repository(self.repo)
            .and_then(|r| r.github.clone())
        else {
            return div().into_any_element();
        };
        let parent_name = github
            .parent
            .as_ref()
            .map(|p| p.full_name())
            .unwrap_or_default();
        let own_name = github.full_name();
        let selected = self.target;
        let content = div()
            .w(px(440.))
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(SPACING)
                    .child("This repository is a fork. How do you plan to use it?"),
            )
            .child(
                segmented_option(
                    "fork-target-parent",
                    "To contribute to the parent project",
                    format!("We will help you contribute to the {parent_name} repository"),
                    selected == ForkContributionTarget::Parent,
                    true,
                    false,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.target = ForkContributionTarget::Parent;
                    cx.notify();
                })),
            )
            .child(
                segmented_option(
                    "fork-target-self",
                    "For my own purposes",
                    format!("We will help you contribute to the {own_name} repository"),
                    selected == ForkContributionTarget::Own,
                    false,
                    true,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.target = ForkContributionTarget::Own;
                    cx.notify();
                })),
            )
            .child(fork_settings_description(&github, selected, cx));
        let repo = self.repo;
        dialog(
            "fork-settings",
            "How are you planning to use this fork?",
            content,
            vec![
                DialogButton {
                    id: "fork-settings-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "fork-settings-ok",
                    label: "Continue".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::set_fork_contribution_target(repo, selected, cx);
                        Dispatcher::close_popup(cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
        .into_any_element()
    }
}

// ---------------------------------------------------------------------------

/// GHD `UpstreamAlreadyExists`: the fork's `upstream` remote does not point
/// at the parent. Update is destructive, so Ignore is the default button.
pub struct UpstreamAlreadyExistsDialog {
    state: Entity<AppState>,
    repo: u64,
    existing_url: String,
}

impl UpstreamAlreadyExistsDialog {
    pub fn new(state: Entity<AppState>, repo: u64, existing_url: String) -> Self {
        Self {
            state,
            repo,
            existing_url,
        }
    }
}

impl Render for UpstreamAlreadyExistsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, parent) = {
            let s = self.state.read(cx);
            let repository = s.repository(self.repo);
            (
                repository.map(|r| r.name()).unwrap_or_default(),
                repository
                    .and_then(|r| r.github.as_ref())
                    .and_then(|gh| gh.parent.as_deref())
                    .cloned(),
            )
        };
        let repo = self.repo;
        // GHD dismisses without choosing on Escape / close
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let Some(parent) = parent else {
            return div().into_any_element();
        };
        let chip = |text: String| Inline::Element(ref_chip(text, cx).into_any_element());
        let bullet = |label: &'static str, value: String| {
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(SPACING_HALF)
                .pl(SPACING)
                .child("•")
                .child(paragraph(vec![label.into(), chip(value)]))
        };
        let content = div()
            .w(px(460.))
            .flex()
            .flex_col()
            .gap(SPACING)
            .child(paragraph(vec![
                "The repository ".into(),
                chip(name),
                " is a fork of ".into(),
                chip(parent.full_name()),
                ", but its ".into(),
                chip(corvane_core::forks::UPSTREAM_REMOTE_NAME.to_string()),
                " remote points elsewhere.".into(),
            ]))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF)
                    .child(bullet("Current: ", self.existing_url.clone()))
                    .child(bullet("Expected: ", parent.clone_url.clone())),
            )
            .child(paragraph(vec![
                "Would you like to update the remote to use the expected URL?".into(),
            ]));
        dialog_with_kind(
            "upstream-already-exists",
            DialogKind::Warning,
            "Upstream Already Exists",
            content,
            vec![
                DialogButton {
                    id: "upstream-already-exists-ignore",
                    label: "Ignore".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::ignore_existing_upstream_remote(repo, cx);
                        Dispatcher::close_popup(cx);
                    }),
                },
                DialogButton {
                    id: "upstream-already-exists-update",
                    label: "Update".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::update_existing_upstream_remote(repo, cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
        .into_any_element()
    }
}
