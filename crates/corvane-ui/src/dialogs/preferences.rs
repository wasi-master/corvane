//! Settings dialog (`ui/preferences/preferences.tsx` + the per-tab
//! components). Vertical tab bar on the left, ~560 px wide; Cancel / Save.
//!
//! Deviations from GHD 3.6.6: no Copilot tab, no Hooks sub-tab under Git
//! (hook environment loading is not implemented), no Usage section under
//! Advanced (no telemetry), no Git Credential Manager toggle and no
//! Formatting section (behind a feature flag in GHD).

use std::rc::Rc;

use corvane_core::{
    AppState, Dispatcher, Popup, PreferencesSave, PreferencesTab, Settings, TAB_SIZE_DEFAULT,
    ThemeSetting, UncommittedChangesStrategy,
};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::icons::Octicon;
use crate::tab_bar::{TabModel, VerticalTab, tab_bar, vertical_tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    SelectHandler, avatar_placeholder, button, call_to_action, checkbox_row, labeled, link_button,
    radio, radio_row, section_heading, select_button, settings_description, text_box,
};

const TABS: [PreferencesTab; 8] = [
    PreferencesTab::Accounts,
    PreferencesTab::Integrations,
    PreferencesTab::Git,
    PreferencesTab::Appearance,
    PreferencesTab::Notifications,
    PreferencesTab::Prompts,
    PreferencesTab::Advanced,
    PreferencesTab::Accessibility,
];

const TAB_SIZES: [u32; 9] = [1, 2, 3, 4, 5, 6, 8, 10, 12];

/// GHD `Git` sub-tabs (Hooks omitted).
#[derive(Clone, Copy, PartialEq, Eq)]
enum GitTab {
    Author,
    DefaultBranch,
}

/// `OtherEmailSelectValue`
const OTHER_EMAIL: &str = "Other";

pub struct PreferencesDialog {
    state: Entity<AppState>,
    tab: PreferencesTab,
    git_tab: GitTab,
    /// Working copy of the settings; written back on Save.
    draft: Settings,
    name: Entity<InputState>,
    email: Entity<InputState>,
    default_branch: Entity<InputState>,
    /// Author email chosen from the account emails; `None` = "Other" (text box).
    email_choice: Option<String>,
    /// The git config arrived and the fields were filled from it.
    git_loaded: bool,
}

impl PreferencesDialog {
    pub fn new(
        state: Entity<AppState>,
        tab: PreferencesTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = state.read(cx).settings.clone();
        let name = cx.new(|cx| InputState::new(window, cx));
        let email = cx.new(|cx| InputState::new(window, cx));
        let default_branch = cx.new(|cx| InputState::new(window, cx));
        for input in [&name, &email, &default_branch] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        cx.observe_in(&state, window, |this, state, window, cx| {
            this.fill_from_git_config(&state, window, cx);
            cx.notify();
        })
        .detach();
        let mut this = Self {
            state: state.clone(),
            tab,
            git_tab: GitTab::Author,
            draft,
            name,
            email,
            default_branch,
            email_choice: None,
            git_loaded: false,
        };
        this.fill_from_git_config(&state, window, cx);
        this
    }

    /// `isLoadingGitConfig` → fields filled once the global config is read.
    fn fill_from_git_config(
        &mut self,
        state: &Entity<AppState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.git_loaded {
            return;
        }
        let Some(config) = state.read(cx).global_git.clone() else {
            return;
        };
        self.git_loaded = true;
        let name = config.name.unwrap_or_default();
        let email = config.email.unwrap_or_default();
        self.name.update(cx, |s, cx| s.set_value(name, window, cx));
        self.email
            .update(cx, |s, cx| s.set_value(email.clone(), window, cx));
        self.default_branch
            .update(cx, |s, cx| s.set_value(config.default_branch, window, cx));
        let emails = self.account_emails(cx);
        self.email_choice = emails.iter().find(|e| **e == email).cloned();
        if !emails.is_empty() && self.email_choice.is_none() && email.is_empty() {
            // GHD preselects the first account email when nothing is configured.
            self.email_choice = emails.first().cloned();
            if let Some(first) = emails.first() {
                self.email
                    .update(cx, |s, cx| s.set_value(first.clone(), window, cx));
            }
        }
    }

    fn account_emails(&self, cx: &App) -> Vec<String> {
        let s = self.state.read(cx);
        let mut out: Vec<String> = Vec::new();
        for account in &s.accounts {
            for email in &account.emails {
                if !out.iter().any(|e| e.eq_ignore_ascii_case(email)) {
                    out.push(email.clone());
                }
            }
        }
        out
    }

    fn save(&self, cx: &mut App) {
        Dispatcher::save_preferences(
            PreferencesSave {
                settings: self.draft.clone(),
                name: self.name.read(cx).value().to_string(),
                email: self.email.read(cx).value().to_string(),
                default_branch: self.default_branch.read(cx).value().to_string(),
            },
            cx,
        );
    }

    fn edit<F: Fn(&mut Settings, bool) + 'static>(
        &self,
        cx: &Context<Self>,
        apply: F,
    ) -> impl Fn(bool, &mut Window, &mut App) + 'static {
        let weak = cx.weak_entity();
        move |value, _, cx| {
            weak.update(cx, |this, cx| {
                apply(&mut this.draft, value);
                cx.notify();
            })
            .ok();
        }
    }

    // ---- tabs ----

    fn accounts_tab(&self, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let dotcom = s.dotcom_account().cloned();
        let enterprise: Vec<_> = s
            .accounts
            .iter()
            .filter(|a| !a.is_dotcom())
            .cloned()
            .collect();
        let account_row = |account: &corvane_core::Account, id: &'static str| {
            let endpoint = account.endpoint.clone();
            let (title, subtitle) = if account.is_dotcom() {
                (
                    account
                        .name
                        .clone()
                        .unwrap_or_else(|| account.login.clone()),
                    format!("@{}", account.login),
                )
            } else {
                let title = match &account.name {
                    Some(name) if name != &account.login => {
                        format!("@{} ({name})", account.login)
                    }
                    _ => format!("@{}", account.login),
                };
                (title, format!("https://{}", account.host()))
            };
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING)
                .mb(SPACING)
                .child(avatar_placeholder(px(34.), cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(title),
                        )
                        .child(
                            div()
                                .text_color(t.text_secondary)
                                .truncate()
                                .child(subtitle),
                        ),
                )
                .child(
                    button(id, "Sign Out", cx)
                        .on_click(move |_, _, cx| Dispatcher::sign_out(endpoint.clone(), cx)),
                )
        };
        div()
            .flex()
            .flex_col()
            .child(section_heading("GitHub.com", cx))
            .child(match &dotcom {
                Some(account) => account_row(account, "prefs-signout-dotcom").into_any_element(),
                None => call_to_action(
                    "prefs-signin-dotcom",
                    "Sign in to your GitHub.com account to access your repositories.",
                    "Sign Into GitHub.com",
                    |_, cx| Dispatcher::show_popup(Popup::SignIn { enterprise: false }, cx),
                    cx,
                )
                .mb(SPACING)
                .into_any_element(),
            })
            .child(section_heading("GitHub Enterprise", cx))
            .children(
                enterprise
                    .iter()
                    .map(|account| account_row(account, "prefs-signout-enterprise")),
            )
            .child(if enterprise.is_empty() {
                call_to_action(
                    "prefs-signin-enterprise",
                    "If you are using GitHub Enterprise at work, sign in to it to get access to your repositories.",
                    "Sign Into GitHub Enterprise",
                    |_, cx| Dispatcher::show_popup(Popup::SignIn { enterprise: true }, cx),
                    cx,
                )
                .into_any_element()
            } else {
                button("prefs-add-enterprise", "Add GitHub Enterprise account", cx)
                    .on_click(|_, _, cx| {
                        Dispatcher::show_popup(Popup::SignIn { enterprise: true }, cx)
                    })
                    .into_any_element()
            })
            .into_any_element()
    }

    fn integrations_tab(&self, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let editors: Vec<SharedString> = s.editors.iter().map(|e| e.name.clone().into()).collect();
        let shells: Vec<SharedString> = s
            .shells
            .iter()
            .map(|s| SharedString::from(s.shell.label()))
            .collect();
        let editor_value = self
            .draft
            .external_editor
            .clone()
            .or_else(|| s.editors.first().map(|e| e.name.clone()));
        let editor_ix = editor_value
            .as_ref()
            .and_then(|v| editors.iter().position(|e| e.as_ref() == v));
        let shell_value = s.shell_label();
        let shell_ix = shells.iter().position(|e| e.as_ref() == shell_value);
        let weak = cx.weak_entity();
        let editor_names = editors.clone();
        let on_editor: SelectHandler = Rc::new(move |ix, _, cx| {
            let name = editor_names.get(ix).map(|n| n.to_string());
            weak.update(cx, |this, cx| {
                this.draft.external_editor = name;
                cx.notify();
            })
            .ok();
        });
        let weak = cx.weak_entity();
        let shell_names = shells.clone();
        let on_shell: SelectHandler = Rc::new(move |ix, _, cx| {
            let name = shell_names.get(ix).map(|n| n.to_string());
            weak.update(cx, |this, cx| {
                this.draft.shell = name;
                cx.notify();
            })
            .ok();
        });
        div()
            .flex()
            .flex_col()
            .gap(SPACING)
            .child(if editors.is_empty() {
                // `.select-component.no-options-found`
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_THIRD)
                    .child("External Editor")
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .text_color(t.text_secondary)
                            .child("No editors found.\u{a0}")
                            .child(
                                link_button(
                                    "prefs-install-editor",
                                    "Install Visual Studio Code?",
                                    cx,
                                )
                                .on_click(|_, _, cx| {
                                    Dispatcher::open_url(
                                        corvane_platform::editors::SUGGESTED_EDITOR_URL,
                                        cx,
                                    )
                                }),
                            ),
                    )
                    .into_any_element()
            } else {
                labeled(
                    "External Editor",
                    select_button(
                        "prefs-editor",
                        editor_value.unwrap_or_default(),
                        editors,
                        editor_ix,
                        false,
                        on_editor,
                        cx,
                    ),
                    cx,
                )
                .into_any_element()
            })
            .child(labeled(
                "Shell",
                select_button(
                    "prefs-shell",
                    shell_value,
                    shells,
                    shell_ix,
                    false,
                    on_shell,
                    cx,
                ),
                cx,
            ))
            .into_any_element()
    }

    fn git_tab(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let weak = cx.weak_entity();
        let tabs = tab_bar(
            vec![
                TabModel {
                    id: "prefs-git-author",
                    label: "Author".into(),
                    count: None,
                },
                TabModel {
                    id: "prefs-git-default-branch",
                    label: "Default branch".into(),
                    count: None,
                },
            ],
            match self.git_tab {
                GitTab::Author => 0,
                GitTab::DefaultBranch => 1,
            },
            move |ix, _, cx| {
                weak.update(cx, |this, cx| {
                    this.git_tab = if ix == 0 {
                        GitTab::Author
                    } else {
                        GitTab::DefaultBranch
                    };
                    cx.notify();
                })
                .ok();
            },
            cx,
        );
        let edit_config = |cx: &App| {
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .mt(SPACING)
                .text_size(FONT_SIZE_SM)
                .text_color(t.text_secondary)
                .child("These preferences will\u{a0}")
                .child(
                    link_button(
                        "prefs-edit-gitconfig",
                        "edit your global Git config file",
                        cx,
                    )
                    .text_size(FONT_SIZE_SM)
                    .on_click(|_, _, cx| Dispatcher::edit_global_git_config(cx)),
                )
                .child(".")
        };
        let body = match self.git_tab {
            GitTab::Author => {
                let emails = self.account_emails(cx);
                let mut options: Vec<SharedString> = emails
                    .iter()
                    .map(|e| SharedString::from(e.clone()))
                    .collect();
                options.push(OTHER_EMAIL.into());
                let selected_ix = match &self.email_choice {
                    Some(choice) => emails.iter().position(|e| e == choice),
                    None => Some(emails.len()),
                };
                let weak = cx.weak_entity();
                let emails_for_select = emails.clone();
                let on_email: SelectHandler = Rc::new(move |ix, window, cx| {
                    let choice = emails_for_select.get(ix).cloned();
                    weak.update(cx, |this, cx| {
                        if let Some(email) = &choice {
                            this.email
                                .update(cx, |s, cx| s.set_value(email.clone(), window, cx));
                        }
                        this.email_choice = choice;
                        cx.notify();
                    })
                    .ok();
                });
                let email_value = self.email.read(cx).value().to_string();
                let warn = !emails.is_empty()
                    && !email_value.trim().is_empty()
                    && !emails
                        .iter()
                        .any(|e| e.eq_ignore_ascii_case(email_value.trim()));
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING)
                    .child(labeled(
                        "Name",
                        text_box("prefs-git-name", &self.name, None, window, cx),
                        cx,
                    ))
                    .when(!emails.is_empty(), |d| {
                        d.child(labeled(
                            "Email",
                            select_button(
                                "prefs-git-email-select",
                                self.email_choice
                                    .clone()
                                    .unwrap_or_else(|| OTHER_EMAIL.to_string()),
                                options,
                                selected_ix,
                                false,
                                on_email,
                                cx,
                            ),
                            cx,
                        ))
                    })
                    .when(emails.is_empty() || self.email_choice.is_none(), |d| {
                        d.child(if emails.is_empty() {
                            labeled(
                                "Email",
                                text_box("prefs-git-email", &self.email, None, window, cx),
                                cx,
                            )
                            .into_any_element()
                        } else {
                            text_box("prefs-git-email", &self.email, None, window, cx)
                                .into_any_element()
                        })
                    })
                    .when(warn, |d| {
                        // `GitEmailNotFoundWarning`
                        d.child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .text_size(FONT_SIZE_SM)
                                .text_color(t.text_secondary)
                                .child(
                                    "This email address doesn't match your GitHub account, so your commits will be wrongly attributed.\u{a0}",
                                )
                                .child(
                                    link_button("prefs-email-learn-more", "Learn more", cx)
                                        .text_size(FONT_SIZE_SM)
                                        .on_click(|_, _, cx| {
                                            Dispatcher::open_url(
                                                "https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-personal-account-on-github/managing-email-preferences/setting-your-commit-email-address",
                                                cx,
                                            )
                                        }),
                                ),
                        )
                    })
                    .child(edit_config(cx))
                    .into_any_element()
            }
            GitTab::DefaultBranch => {
                let value = self.default_branch.read(cx).value().to_string();
                let sanitized = crate::dialogs::branch_dialogs::sanitize_ref_name(value.trim());
                let warning = (!value.trim().is_empty() && sanitized != value.trim())
                    .then(|| format!("Will be saved as {sanitized}."));
                div()
                    .flex()
                    .flex_col()
                    .child(section_heading("Default branch name for new repositories", cx))
                    .child(text_box(
                        "prefs-default-branch",
                        &self.default_branch,
                        None,
                        window,
                        cx,
                    ))
                    .when_some(warning, |d, warning| {
                        d.child(
                            div()
                                .mt(SPACING_THIRD)
                                .text_size(FONT_SIZE_SM)
                                .text_color(t.text_secondary)
                                .child(warning),
                        )
                    })
                    .child(
                        settings_description(cx)
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .child("GitHub's default branch name is\u{a0}")
                            .child(mono("main", t))
                            .child(
                                ". You may want to change it due to different workflows, or because your integrations still require the historical default branch name of\u{a0}",
                            )
                            .child(mono("master", t))
                            .child("."),
                    )
                    .child(edit_config(cx))
                    .into_any_element()
            }
        };
        // `.dialog-content.git-preferences { padding: 0 }`: the sub tab bar is
        // flush with the tab container; its content gets the 20 px padding back.
        div()
            .m(px(-20.))
            .flex()
            .flex_col()
            .child(tabs)
            .child(div().p(SPACING_DOUBLE).child(body))
            .into_any_element()
    }

    fn appearance_tab(&self, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let themes = [
            (ThemeSetting::Light, "Light"),
            (ThemeSetting::Dark, "Dark"),
            (ThemeSetting::System, "System"),
        ];
        let selected = self.draft.theme;
        let swatches = div()
            .flex()
            .flex_row()
            .mx(px(-5.))
            .children(themes.iter().map(|(theme, label)| {
                let theme = *theme;
                let is_selected = theme == selected;
                let weak = cx.weak_entity();
                let id: &'static str = match theme {
                    ThemeSetting::Light => "prefs-theme-light",
                    ThemeSetting::Dark => "prefs-theme-dark",
                    ThemeSetting::System => "prefs-theme-system",
                };
                let image = |path: &'static str| {
                    img(path)
                        .w_full()
                        .h(px(60.))
                        .object_fit(ObjectFit::Cover)
                        .border_b_1()
                        .border_color(t.box_border)
                };
                div()
                    .id(id)
                    .flex_1()
                    .min_w_0()
                    .p(SPACING_HALF)
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        weak.update(cx, |this, cx| {
                            this.draft.theme = theme;
                            cx.notify();
                        })
                        .ok();
                    })
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .rounded(BORDER_RADIUS)
                            .border_1()
                            .border_color(if is_selected {
                                t.box_selected_active_background
                            } else {
                                t.box_border
                            })
                            .overflow_hidden()
                            .pb(SPACING_HALF)
                            .child(match theme {
                                ThemeSetting::Light => {
                                    image("illustrations/ghd_light.svg").into_any_element()
                                }
                                ThemeSetting::Dark => {
                                    image("illustrations/ghd_dark.svg").into_any_element()
                                }
                                // `.system-theme-swatch`: light on the left half, dark on the right.
                                ThemeSetting::System => div()
                                    .relative()
                                    .w_full()
                                    .h(px(60.))
                                    .border_b_1()
                                    .border_color(t.box_border)
                                    .overflow_hidden()
                                    .child(
                                        div().absolute().inset_0().child(
                                            img("illustrations/ghd_light.svg")
                                                .w_full()
                                                .h(px(60.))
                                                .object_fit(ObjectFit::Cover),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .bottom_0()
                                            .right_0()
                                            .w_1_2()
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .bottom_0()
                                                    .right_0()
                                                    .w(px(230.))
                                                    .child(
                                                        img("illustrations/ghd_dark.svg")
                                                            .w_full()
                                                            .h(px(60.))
                                                            .object_fit(ObjectFit::Cover),
                                                    ),
                                            ),
                                    )
                                    .into_any_element(),
                            })
                            .child(
                                div()
                                    .mt(SPACING_HALF)
                                    .px(SPACING)
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(SPACING_HALF)
                                    .child(radio(
                                        ElementId::from(SharedString::from(format!("{id}-radio"))),
                                        is_selected,
                                        cx,
                                    ))
                                    .child(*label),
                            ),
                    )
            }));
        let tab_size = self.draft.tab_size;
        let options: Vec<SharedString> = TAB_SIZES
            .iter()
            .map(|n| {
                if *n == TAB_SIZE_DEFAULT {
                    format!("{n} (default)").into()
                } else {
                    n.to_string().into()
                }
            })
            .collect();
        let selected_ix = TAB_SIZES.iter().position(|n| *n == tab_size);
        let weak = cx.weak_entity();
        let on_tab_size: SelectHandler = Rc::new(move |ix, _, cx| {
            if let Some(n) = TAB_SIZES.get(ix) {
                weak.update(cx, |this, cx| {
                    this.draft.tab_size = *n;
                    cx.notify();
                })
                .ok();
            }
        });
        div()
            .flex()
            .flex_col()
            .child(section_heading("Theme", cx))
            .child(swatches)
            .child(div().mt(SPACING).child(section_heading("Diff", cx)))
            .child(labeled(
                "Tab Size",
                select_button(
                    "prefs-tab-size",
                    options
                        .get(selected_ix.unwrap_or(3))
                        .cloned()
                        .unwrap_or_default(),
                    options,
                    selected_ix,
                    false,
                    on_tab_size,
                    cx,
                ),
                cx,
            ))
            .into_any_element()
    }

    fn notifications_tab(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .child(section_heading("Notifications", cx))
            .child(checkbox_row(
                "prefs-notifications",
                self.draft.notifications_enabled,
                "Enable notifications",
                self.edit(cx, |s, v| s.notifications_enabled = v),
                cx,
            ))
            .child(settings_description(cx).child(
                "Allows the display of notifications when high-signal events take place in the current repository.",
            ))
            .into_any_element()
    }

    fn prompts_tab(&self, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let strategy = d.uncommitted_changes_strategy;
        let strategies = [
            (
                UncommittedChangesStrategy::AskForConfirmation,
                "prefs-strategy-ask",
                "Ask me where I want the changes to go",
            ),
            (
                UncommittedChangesStrategy::MoveToNewBranch,
                "prefs-strategy-move",
                "Always bring my changes to my new branch",
            ),
            (
                UncommittedChangesStrategy::StashOnCurrentBranch,
                "prefs-strategy-stash",
                "Always stash and leave my changes on the current branch",
            ),
        ];
        div()
            .flex()
            .flex_col()
            .child(section_heading("Show a confirmation dialog before...", cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF)
                    .child(checkbox_row(
                        "prefs-confirm-remove",
                        d.confirm_repository_removal,
                        "Removing repositories",
                        self.edit(cx, |s, v| s.confirm_repository_removal = v),
                        cx,
                    ))
                    .child(checkbox_row(
                        "prefs-confirm-discard",
                        d.confirm_discard_changes,
                        "Discarding changes",
                        self.edit(cx, |s, v| s.confirm_discard_changes = v),
                        cx,
                    ))
                    .child(checkbox_row(
                        "prefs-confirm-discard-permanently",
                        d.confirm_discard_changes_permanently,
                        "Discarding changes permanently",
                        self.edit(cx, |s, v| s.confirm_discard_changes_permanently = v),
                        cx,
                    ))
                    .child(checkbox_row(
                        "prefs-confirm-discard-stash",
                        d.confirm_discard_stash,
                        "Discarding stash",
                        self.edit(cx, |s, v| s.confirm_discard_stash = v),
                        cx,
                    ))
                    .child(checkbox_row(
                        "prefs-confirm-checkout-commit",
                        d.confirm_checkout_commit,
                        "Checking out a commit",
                        self.edit(cx, |s, v| s.confirm_checkout_commit = v),
                        cx,
                    ))
                    .child(checkbox_row(
                        "prefs-confirm-force-push",
                        d.confirm_force_push,
                        "Force pushing",
                        self.edit(cx, |s, v| s.confirm_force_push = v),
                        cx,
                    ))
                    .child(checkbox_row(
                        "prefs-confirm-undo",
                        d.confirm_undo_commit,
                        "Undo commit",
                        self.edit(cx, |s, v| s.confirm_undo_commit = v),
                        cx,
                    ))
                    .child(checkbox_row(
                        "prefs-confirm-filtered",
                        d.confirm_commit_filtered_changes,
                        "Committing changes hidden by filter",
                        self.edit(cx, |s, v| s.confirm_commit_filtered_changes = v),
                        cx,
                    )),
            )
            .child(div().mt(SPACING).child(section_heading(
                "If I have changes and I switch branches...",
                cx,
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF)
                    .children(strategies.iter().map(|(value, id, label)| {
                        let value = *value;
                        let weak = cx.weak_entity();
                        radio_row(
                            id,
                            strategy == value,
                            *label,
                            move |_, cx| {
                                weak.update(cx, |this, cx| {
                                    this.draft.uncommitted_changes_strategy = value;
                                    cx.notify();
                                })
                                .ok();
                            },
                            cx,
                        )
                    })),
            )
            .child(
                div()
                    .mt(SPACING)
                    .child(section_heading("Commit Length", cx)),
            )
            .child(checkbox_row(
                "prefs-commit-length",
                d.show_commit_length_warning,
                "Show commit length warning",
                self.edit(cx, |s, v| s.show_commit_length_warning = v),
                cx,
            ))
            .into_any_element()
    }

    fn advanced_tab(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .child(section_heading("Background updates", cx))
            .child(checkbox_row(
                "prefs-indicators",
                self.draft.repository_indicators_enabled,
                "Show status icons in the repository list",
                self.edit(cx, |s, v| s.repository_indicators_enabled = v),
                cx,
            ))
            .child(
                settings_description(cx)
                    .flex()
                    .flex_col()
                    .gap(SPACING)
                    .child(
                        "These icons indicate which repositories have local or remote changes, and require the periodic fetching of repositories that are not currently selected.",
                    )
                    .child(
                        "Turning this off will not stop the periodic fetching of your currently selected repository, but may improve overall app performance for users with many repositories.",
                    ),
            )
            .into_any_element()
    }

    fn accessibility_tab(&self, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let underline = self.draft.underline_links;
        div()
            .flex()
            .flex_col()
            .child(section_heading("Accessibility", cx))
            .child(checkbox_row(
                "prefs-underline-links",
                underline,
                "Underline links",
                self.edit(cx, |s, v| s.underline_links = v),
                cx,
            ))
            .child(
                settings_description(cx)
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .child(
                        "When enabled, Corvane will underline links in commit messages, comments, and other text fields. This can help make links easier to distinguish.\u{a0}",
                    )
                    .child(
                        // `.example-link`: a non-interactive preview of the setting.
                        div()
                            .text_color(t.link)
                            .when(underline, |d| d.underline())
                            .child("This is an example link"),
                    ),
            )
            .child(div().mt(SPACING).child(checkbox_row(
                "prefs-diff-check-marks",
                self.draft.show_diff_check_marks,
                "Show check marks in the diff",
                self.edit(cx, |s, v| s.show_diff_check_marks = v),
                cx,
            )))
            .child(settings_description(cx).child(
                "When enabled, check marks will be displayed along side the line numbers and groups of line numbers in the diff when committing. When disabled, the line number controls will be less prominent.",
            ))
            .into_any_element()
    }
}

/// `<Ref>`: inline monospace code.
fn mono(text: &'static str, t: &crate::theme::GhdTheme) -> Div {
    div()
        .font_family(crate::theme::MONO_FONT)
        .px(px(3.))
        .rounded(px(3.))
        .bg(t.box_alt_background)
        .child(text)
}

impl Render for PreferencesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let selected = TABS.iter().position(|t| *t == self.tab).unwrap_or(0);
        let weak = cx.weak_entity();
        let nav = vertical_tab_bar(
            vec![
                VerticalTab {
                    id: "prefs-tab-accounts",
                    label: "Accounts".into(),
                    icon: Octicon::Home,
                },
                VerticalTab {
                    id: "prefs-tab-integrations",
                    label: "Integrations".into(),
                    icon: Octicon::Person,
                },
                VerticalTab {
                    id: "prefs-tab-git",
                    label: "Git".into(),
                    icon: Octicon::GitCommit,
                },
                VerticalTab {
                    id: "prefs-tab-appearance",
                    label: "Appearance".into(),
                    icon: Octicon::Paintbrush,
                },
                VerticalTab {
                    id: "prefs-tab-notifications",
                    label: "Notifications".into(),
                    icon: Octicon::Bell,
                },
                VerticalTab {
                    id: "prefs-tab-prompts",
                    label: "Prompts".into(),
                    icon: Octicon::Question,
                },
                VerticalTab {
                    id: "prefs-tab-advanced",
                    label: "Advanced".into(),
                    icon: Octicon::Gear,
                },
                VerticalTab {
                    id: "prefs-tab-accessibility",
                    label: "Accessibility".into(),
                    icon: Octicon::Accessibility,
                },
            ],
            selected,
            move |ix, _, cx| {
                weak.update(cx, |this, cx| {
                    this.tab = TABS[ix];
                    cx.notify();
                })
                .ok();
            },
            cx,
        );
        let body = match self.tab {
            PreferencesTab::Accounts => self.accounts_tab(cx),
            PreferencesTab::Integrations => self.integrations_tab(cx),
            PreferencesTab::Git => self.git_tab(window, cx),
            PreferencesTab::Appearance => self.appearance_tab(cx),
            PreferencesTab::Notifications => self.notifications_tab(cx),
            PreferencesTab::Prompts => self.prompts_tab(cx),
            PreferencesTab::Advanced => self.advanced_tab(cx),
            PreferencesTab::Accessibility => self.accessibility_tab(cx),
        };
        // `.preferences-container`: nav | bordered tab container. The dialog
        // content's 20 px padding is cancelled so the nav sits flush.
        let content = div()
            .w(px(560.))
            .mx(px(-20.))
            .my(px(-20.))
            .min_h(px(360.))
            .flex()
            .flex_row()
            .items_stretch()
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .border_l_1()
                    .border_color(t.box_border)
                    .p(SPACING_DOUBLE)
                    .child(body),
            );
        let weak = cx.weak_entity();
        dialog(
            "dialog-preferences",
            "Settings",
            content,
            vec![
                DialogButton {
                    id: "prefs-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "prefs-save",
                    label: "Save".into(),
                    primary: true,
                    on_click: Box::new(move |_, cx| {
                        weak.update(cx, |this, cx| this.save(cx)).ok();
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
