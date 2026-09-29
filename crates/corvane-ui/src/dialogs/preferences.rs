//! Settings dialog (`ui/preferences/preferences.tsx` + the per-tab
//! components). Vertical tab bar on the left, ~560 px wide; Cancel / Save.
//!
//! Deviations from GHD 3.6.6: no Copilot tab, no Hooks sub-tab under Git
//! (hook environment loading is not implemented), no Usage section under
//! Advanced (no telemetry), no Git Credential Manager toggle and no
//! Formatting section (behind a feature flag in GHD).

use std::path::Path;
use std::rc::Rc;

use corvane_core::{
    AppState, Dispatcher, Popup, PreferencesSave, PreferencesTab, Settings, TAB_SIZE_DEFAULT,
    ThemeSetting, UncommittedChangesStrategy,
};
use corvane_platform::notifications::NotificationPermission;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, dialog};
use crate::icons::Octicon;
use crate::tab_bar::{TabModel, VerticalTab, tab_bar, vertical_tab_bar};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    Inline, SelectHandler, button, call_to_action, checkbox_row, code_ref, labeled, link_button,
    paragraph, radio, radio_row, section_heading, select_button, settings_description, text_box,
};

/// `isValidCustomIntegration`: the bundle id of a `.app` path (kept when the
/// path did not change, `mdls` is a subprocess).
fn bundle_id_for(path: &str, previous: Option<&corvane_core::CustomIntegration>) -> Option<String> {
    if !path.ends_with(".app") {
        return None;
    }
    match previous {
        Some(prev) if prev.path == path && prev.bundle_id.is_some() => prev.bundle_id.clone(),
        _ => corvane_platform::custom_integration::app_bundle_id(Path::new(path)),
    }
}

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
    Hooks,
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
    /// GHD `Notifications` state: `getNotificationsPermission()` result.
    notification_permission: Option<NotificationPermission>,
    /// `CustomIntegrationForm` inputs (Integrations tab).
    custom_editor_path: Entity<InputState>,
    custom_editor_args: Entity<InputState>,
    custom_shell_path: Entity<InputState>,
    custom_shell_args: Entity<InputState>,
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
        let mut custom_input =
            |placeholder: &'static str, value: String, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(placeholder)
                        .default_value(value)
                })
            };
        let editor = draft.custom_editor.clone().unwrap_or_default();
        let shell = draft.custom_shell.clone().unwrap_or_default();
        let custom_editor_path = custom_input("Path to executable", editor.path, cx);
        let custom_editor_args = custom_input("Command line arguments", editor.arguments, cx);
        let custom_shell_path = custom_input("Path to executable", shell.path, cx);
        let custom_shell_args = custom_input("Command line arguments", shell.arguments, cx);
        for input in [&custom_editor_path, &custom_editor_args] {
            cx.observe(input, |this, _, cx| {
                let path = this.custom_editor_path.read(cx).value().trim().to_string();
                let bundle_id = bundle_id_for(&path, this.draft.custom_editor.as_ref());
                this.draft.custom_editor = Some(corvane_core::CustomIntegration {
                    path,
                    arguments: this.custom_editor_args.read(cx).value().trim().to_string(),
                    bundle_id,
                });
                cx.notify();
            })
            .detach();
        }
        for input in [&custom_shell_path, &custom_shell_args] {
            cx.observe(input, |this, _, cx| {
                let path = this.custom_shell_path.read(cx).value().trim().to_string();
                let bundle_id = bundle_id_for(&path, this.draft.custom_shell.as_ref());
                this.draft.custom_shell = Some(corvane_core::CustomIntegration {
                    path,
                    arguments: this.custom_shell_args.read(cx).value().trim().to_string(),
                    bundle_id,
                });
                cx.notify();
            })
            .detach();
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
            notification_permission: None,
            custom_editor_path,
            custom_editor_args,
            custom_shell_path,
            custom_shell_args,
        };
        this.fill_from_git_config(&state, window, cx);
        this.poll_notification_permission(cx);
        this
    }

    /// `updateNotificationsState`: the centre answers on its own queue, so
    /// ask off the main thread.
    fn poll_notification_permission(&mut self, cx: &mut Context<Self>) {
        let task = cx
            .background_executor()
            .spawn(async move { corvane_platform::notifications::permission() });
        cx.spawn(async move |this, cx| {
            let permission = task.await;
            this.update(cx, |this, cx| {
                this.notification_permission = Some(permission);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `onGrantNotificationPermission`: show the system prompt, then poll
    /// until the status leaves `Default` (the prompt is asynchronous).
    fn grant_notification_permission(&mut self, cx: &mut Context<Self>) {
        corvane_platform::notifications::request_permission();
        cx.spawn(async move |this, cx| {
            for _ in 0..60 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                let permission = cx
                    .background_executor()
                    .spawn(async move { corvane_platform::notifications::permission() })
                    .await;
                let done = this
                    .update(cx, |this, cx| {
                        this.notification_permission = Some(permission);
                        cx.notify();
                        permission != NotificationPermission::Default
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
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
                .child(crate::widgets::avatar_image(
                    account
                        .avatar_url
                        .as_deref()
                        .and_then(|u| crate::widgets::avatar_lookup_url(u, cx)),
                    px(34.),
                    cx,
                ))
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

    /// `CustomIntegrationForm`: Path + Choose…, Arguments, validation messages.
    fn custom_form(
        &self,
        id: &'static str,
        path: &Entity<InputState>,
        args: &Entity<InputState>,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let t = cx.ghd();
        let path_text = path.read(cx).value().trim().to_string();
        let args_text = args.read(cx).value().trim().to_string();
        let path_error = (!path_text.is_empty()
            && !corvane_platform::custom_integration::path_looks_valid(&path_text))
        .then_some("This path does not appear to be a valid executable.");
        let args_error = if args_text.is_empty() {
            None
        } else {
            match corvane_platform::custom_integration::parse_arguments(&args_text) {
                None => Some("These arguments are not valid.".to_string()),
                Some(argv) if !corvane_platform::custom_integration::has_target_path(&argv) => {
                    Some(format!(
                        "Arguments must include the target path placeholder ({}).",
                        corvane_platform::custom_integration::TARGET_PATH_ARGUMENT
                    ))
                }
                Some(_) => None,
            }
        };
        let input_error = |message: String| {
            // `.input-description.input-description-error`
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF)
                .text_size(FONT_SIZE_SM)
                .text_color(t.form_error_text)
                .child(crate::icons::octicon(Octicon::Alert, t.input_icon_error).size(px(12.)))
                .child(message)
        };
        let path_for_choose = path.clone();
        div()
            .flex()
            .flex_col()
            .gap(SPACING_HALF)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap(SPACING)
                    .child(labeled(
                        "Path",
                        text_box(
                            SharedString::from(format!("{id}-path")),
                            path,
                            None,
                            window,
                            cx,
                        ),
                        cx,
                    ))
                    .child(
                        button(SharedString::from(format!("{id}-choose")), "Choose…", cx)
                            .flex_none()
                            .on_click(move |_, window, cx| {
                                // apps are directories on macOS, so allow both
                                let receiver = cx.prompt_for_paths(PathPromptOptions {
                                    files: true,
                                    directories: true,
                                    multiple: false,
                                    prompt: Some("Choose".into()),
                                });
                                let path = path_for_choose.clone();
                                window
                                    .spawn(cx, async move |cx| {
                                        if let Ok(Ok(Some(paths))) = receiver.await
                                            && let Some(p) = paths.into_iter().next()
                                        {
                                            path.update_in(cx, |s, window, cx| {
                                                s.set_value(p.display().to_string(), window, cx)
                                            })
                                            .ok();
                                        }
                                    })
                                    .detach();
                            }),
                    ),
            )
            .when_some(path_error, |d, message| {
                d.child(input_error(message.to_string()))
            })
            .child(labeled(
                "Arguments",
                text_box(
                    SharedString::from(format!("{id}-args")),
                    args,
                    None,
                    window,
                    cx,
                ),
                cx,
            ))
            .when_some(args_error, |d, message| d.child(input_error(message)))
    }

    fn integrations_tab(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let editors: Vec<SharedString> = s.editors.iter().map(|e| e.name.clone().into()).collect();
        let shells: Vec<SharedString> = s
            .shells
            .iter()
            .map(|s| SharedString::from(s.shell.label()))
            .collect();
        let use_custom_editor = self.draft.use_custom_editor;
        let use_custom_shell = self.draft.use_custom_shell;
        // `CustomIntegrationValue`: the last option configures a custom integration.
        let mut editor_options = editors.clone();
        editor_options.push("Configure Custom Editor…".into());
        let editor_value = if use_custom_editor {
            "Configure Custom Editor…".to_string()
        } else {
            self.draft
                .external_editor
                .clone()
                .or_else(|| s.editors.first().map(|e| e.name.clone()))
                .unwrap_or_default()
        };
        let editor_ix = if use_custom_editor {
            Some(editors.len())
        } else {
            editors.iter().position(|e| e.as_ref() == editor_value)
        };
        let mut shell_options = shells.clone();
        shell_options.push("Configure Custom Shell…".into());
        let shell_value = if use_custom_shell {
            "Configure Custom Shell…".to_string()
        } else {
            self.draft
                .shell
                .clone()
                .unwrap_or_else(|| corvane_platform::shells::DEFAULT_SHELL.label().to_string())
        };
        let shell_ix = if use_custom_shell {
            Some(shells.len())
        } else {
            shells.iter().position(|e| e.as_ref() == shell_value)
        };
        let weak = cx.weak_entity();
        let editor_names = editors.clone();
        let on_editor: SelectHandler = Rc::new(move |ix, _, cx| {
            let name = editor_names.get(ix).map(|n| n.to_string());
            let custom = ix == editor_names.len();
            weak.update(cx, |this, cx| {
                this.draft.use_custom_editor = custom;
                if !custom {
                    this.draft.external_editor = name;
                } else if this.draft.custom_editor.is_none() {
                    this.draft.custom_editor = Some(corvane_core::CustomIntegration::default());
                }
                cx.notify();
            })
            .ok();
        });
        let weak = cx.weak_entity();
        let shell_names = shells.clone();
        let on_shell: SelectHandler = Rc::new(move |ix, _, cx| {
            let name = shell_names.get(ix).map(|n| n.to_string());
            let custom = ix == shell_names.len();
            weak.update(cx, |this, cx| {
                this.draft.use_custom_shell = custom;
                if !custom {
                    this.draft.shell = name;
                } else if this.draft.custom_shell.is_none() {
                    this.draft.custom_shell = Some(corvane_core::CustomIntegration::default());
                }
                cx.notify();
            })
            .ok();
        });
        div()
            .flex()
            .flex_col()
            .gap(SPACING)
            .child(labeled(
                "External Editor",
                select_button(
                    "prefs-editor",
                    editor_value,
                    editor_options,
                    editor_ix,
                    false,
                    on_editor,
                    cx,
                ),
                cx,
            ))
            .when(editors.is_empty(), |d| {
                // `renderNoExternalEditorHint`
                d.child(
                    paragraph(vec![
                        "No other editors found. ".into(),
                        link_button("prefs-install-editor", "Install Visual Studio Code?", cx)
                            .on_click(|_, _, cx| {
                                Dispatcher::open_url(
                                    corvane_platform::editors::SUGGESTED_EDITOR_URL,
                                    cx,
                                )
                            })
                            .into_any_element()
                            .into(),
                    ])
                    .text_color(t.text_secondary),
                )
            })
            .when(use_custom_editor, |d| {
                d.child(self.custom_form(
                    "custom-editor",
                    &self.custom_editor_path,
                    &self.custom_editor_args,
                    window,
                    cx,
                ))
            })
            .child(labeled(
                "Shell",
                select_button(
                    "prefs-shell",
                    shell_value,
                    shell_options,
                    shell_ix,
                    false,
                    on_shell,
                    cx,
                ),
                cx,
            ))
            .when(use_custom_shell, |d| {
                d.child(self.custom_form(
                    "custom-shell",
                    &self.custom_shell_path,
                    &self.custom_shell_args,
                    window,
                    cx,
                ))
            })
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
                TabModel {
                    id: "prefs-git-hooks",
                    label: "Hooks".into(),
                    count: None,
                },
            ],
            match self.git_tab {
                GitTab::Author => 0,
                GitTab::DefaultBranch => 1,
                GitTab::Hooks => 2,
            },
            move |ix, _, cx| {
                weak.update(cx, |this, cx| {
                    this.git_tab = match ix {
                        0 => GitTab::Author,
                        1 => GitTab::DefaultBranch,
                        _ => GitTab::Hooks,
                    };
                    cx.notify();
                })
                .ok();
            },
            cx,
        );
        let edit_config = |cx: &App| {
            paragraph(vec![
                "These preferences will ".into(),
                link_button(
                    "prefs-edit-gitconfig",
                    "edit your global Git config file",
                    cx,
                )
                .text_size(FONT_SIZE_SM)
                .on_click(|_, _, cx| Dispatcher::edit_global_git_config(cx))
                .into_any_element()
                .into(),
                ".".into(),
            ])
            .mt(SPACING)
            .text_size(FONT_SIZE_SM)
            .text_color(t.text_secondary)
        };
        let body = match self.git_tab {
            GitTab::Hooks => div().into_any_element(),
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
                            paragraph(vec![
                                "This email address doesn't match your GitHub account, so your commits will be wrongly attributed. ".into(),
                                link_button("prefs-email-learn-more", "Learn more", cx)
                                    .text_size(FONT_SIZE_SM)
                                    .on_click(|_, _, cx| {
                                        Dispatcher::open_url(
                                            "https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-personal-account-on-github/managing-email-preferences/setting-your-commit-email-address",
                                            cx,
                                        )
                                    })
                                    .into_any_element()
                                    .into(),
                            ])
                            .text_size(FONT_SIZE_SM)
                            .text_color(t.text_secondary),
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
                        paragraph(vec![
                            "GitHub's default branch name is ".into(),
                            code_ref("main", cx).into_any_element().into(),
                            ". You may want to change it due to different workflows, or because your integrations still require the historical default branch name of ".into(),
                            code_ref("master", cx).into_any_element().into(),
                            ".".into(),
                        ])
                        .mt(SPACING)
                        .text_size(FONT_SIZE_SM)
                        .text_color(t.text_secondary),
                    )
                    .child(edit_config(cx))
                    .into_any_element()
            }
        };
        let body = match self.git_tab {
            GitTab::Hooks => self.hooks_tab(cx),
            _ => body,
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

    /// `renderHooksSettings`
    fn hooks_tab(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.draft.enable_git_hook_env;
        div()
            .flex()
            .flex_col()
            .child(checkbox_row(
                "prefs-hook-env",
                enabled,
                "Load Git hook environment variables from shell",
                self.edit(cx, |s, v| s.enable_git_hook_env = v),
                cx,
            ))
            .child(settings_description(cx).child(
                "When enabled, Corvane will attempt to load environment variables from your shell when executing Git hooks. This is useful if your Git hooks depend on environment variables set in your shell configuration files, a common practice for version managers such as nvm, rbenv, asdf, etc.",
            ))
            .when(enabled, |d| {
                d.child(div().mt(SPACING).child(checkbox_row(
                    "prefs-hook-env-cache",
                    self.draft.cache_git_hook_env,
                    "Cache Git hook environment variables",
                    self.edit(cx, |s, v| s.cache_git_hook_env = v),
                    cx,
                )))
                .child(settings_description(cx).child(
                    "Cache hook environment variables to improve performance. Disable if your hooks rely on frequently changing environment variables.",
                ))
            })
            .into_any_element()
    }

    /// `renderFormatting`: Date / Time / Number format + absolute dates.
    fn formatting_section(&self, cx: &Context<Self>) -> Div {
        let date_options: Vec<SharedString> = crate::format::DATE_FORMATS
            .iter()
            .map(|p| format!("{} ({p})", crate::format::date_example(p)).into())
            .collect();
        let date_ix = crate::format::DATE_FORMATS
            .iter()
            .position(|p| *p == self.draft.date_format);
        let weak = cx.weak_entity();
        let on_date: SelectHandler = Rc::new(move |ix, _, cx| {
            if let Some(p) = crate::format::DATE_FORMATS.get(ix) {
                weak.update(cx, |this, cx| {
                    this.draft.date_format = p.to_string();
                    cx.notify();
                })
                .ok();
            }
        });
        let time_options: Vec<SharedString> = crate::format::TIME_FORMATS
            .iter()
            .map(|p| format!("{} ({p})", crate::format::time_example(p)).into())
            .collect();
        let time_ix = crate::format::TIME_FORMATS
            .iter()
            .position(|p| *p == self.draft.time_format);
        let weak = cx.weak_entity();
        let on_time: SelectHandler = Rc::new(move |ix, _, cx| {
            if let Some(p) = crate::format::TIME_FORMATS.get(ix) {
                weak.update(cx, |this, cx| {
                    this.draft.time_format = p.to_string();
                    cx.notify();
                })
                .ok();
            }
        });
        let number_options: Vec<SharedString> = crate::format::NUMBER_FORMATS
            .iter()
            .map(|k| crate::format::number_example(k).into())
            .collect();
        let number_ix = crate::format::NUMBER_FORMATS
            .iter()
            .position(|k| *k == self.draft.number_format);
        let weak = cx.weak_entity();
        let on_number: SelectHandler = Rc::new(move |ix, _, cx| {
            if let Some(k) = crate::format::NUMBER_FORMATS.get(ix) {
                weak.update(cx, |this, cx| {
                    this.draft.number_format = k.to_string();
                    cx.notify();
                })
                .ok();
            }
        });
        let pick = |options: &[SharedString], ix: Option<usize>| {
            ix.and_then(|i| options.get(i).cloned()).unwrap_or_default()
        };
        div()
            .mt(SPACING)
            .flex()
            .flex_col()
            .child(section_heading("Formatting", cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(SPACING)
                    .mb(SPACING)
                    .child(labeled(
                        "Date Format",
                        select_button(
                            "prefs-date-format",
                            pick(&date_options, date_ix),
                            date_options.clone(),
                            date_ix,
                            false,
                            on_date,
                            cx,
                        ),
                        cx,
                    ))
                    .child(labeled(
                        "Time Format",
                        select_button(
                            "prefs-time-format",
                            pick(&time_options, time_ix),
                            time_options.clone(),
                            time_ix,
                            false,
                            on_time,
                            cx,
                        ),
                        cx,
                    )),
            )
            .child(labeled(
                "Number Format",
                select_button(
                    "prefs-number-format",
                    pick(&number_options, number_ix),
                    number_options.clone(),
                    number_ix,
                    false,
                    on_number,
                    cx,
                ),
                cx,
            ))
            .child(div().my(SPACING).child(checkbox_row(
                "prefs-absolute-dates",
                self.draft.prefer_absolute_dates,
                "Prefer absolute dates over relative",
                self.edit(cx, |s, v| s.prefer_absolute_dates = v),
                cx,
            )))
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
            .child(self.formatting_section(cx))
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
        let t = cx.ghd();
        let settings_url =
            corvane_platform::notifications::settings_url(corvane_platform::BUNDLE_ID);
        let permission = self
            .draft
            .notifications_enabled
            .then_some(self.notification_permission)
            .flatten();
        let notifications_link = |id: &'static str, cx: &Context<Self>| {
            let url = settings_url.clone();
            link_button(id, "Notifications Settings", cx)
                .on_click(move |_, _, cx| cx.open_url(&url))
                .into_any_element()
        };
        // `renderNotificationHint`
        let mut parts: Vec<Inline> = vec![
            "Allows the display of notifications when high-signal events take place in the current repository."
                .into(),
        ];
        let mut warning: Option<Div> = None;
        match permission {
            None | Some(NotificationPermission::Unsupported) => {}
            Some(NotificationPermission::Default) => {
                parts.push(" You need to ".into());
                parts.push(Inline::Element(
                    link_button("prefs-notifications-grant", "grant permission", cx)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.grant_notification_permission(cx)),
                        )
                        .into_any_element(),
                ));
                parts.push(" to display these notifications from Corvane.".into());
            }
            Some(NotificationPermission::Denied) => {
                // `.setting-hint-warning`
                warning = Some(
                    div().mt(SPACING).child(paragraph(vec![
                        Inline::Element(
                            div()
                                .text_color(t.dialog_warning)
                                .child("⚠️")
                                .into_any_element(),
                        ),
                        " Corvane has no permission to display notifications. Please, enable them in the ".into(),
                        Inline::Element(notifications_link("prefs-notifications-settings", cx)),
                        ".".into(),
                    ])),
                );
            }
            Some(NotificationPermission::Granted) => {
                parts.push(
                    " Make sure notifications are properly configured for Corvane in the ".into(),
                );
                parts.push(Inline::Element(notifications_link(
                    "prefs-notifications-settings",
                    cx,
                )));
                parts.push(".".into());
            }
        }
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
            .child(
                settings_description(cx)
                    .child(paragraph(parts).line_height(px(16.)))
                    .children(warning),
            )
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
                        "prefs-confirm-worktree-removal",
                        d.confirm_worktree_removal,
                        "Removing worktrees",
                        self.edit(cx, |s, v| s.confirm_worktree_removal = v),
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
        let t = cx.ghd();
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
            .child(div().mt(SPACING).child(section_heading("Network and credentials", cx)))
            .child(checkbox_row(
                "prefs-credential-manager",
                self.draft.use_external_credential_helper,
                "Use Git Credential Manager",
                self.edit(cx, |s, v| s.use_external_credential_helper = v),
                cx,
            ))
            .child(
                paragraph(vec![
                    "Use ".into(),
                    link_button("prefs-gcm-link", "Git Credential Manager", cx)
                        .text_size(FONT_SIZE_SM)
                        .on_click(|_, _, cx| Dispatcher::open_url("https://gh.io/gcm", cx))
                        .into_any_element()
                        .into(),
                    " for private repositories outside of GitHub.com. This feature is experimental and subject to change.".into(),
                ])
                .mt(SPACING)
                .text_size(FONT_SIZE_SM)
                .text_color(t.text_secondary),
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
                paragraph(vec![
                    "When enabled, Corvane will underline links in commit messages, comments, and other text fields. This can help make links easier to distinguish. ".into(),
                    // `.example-link`: a non-interactive preview of the setting.
                    div()
                        .text_color(t.link)
                        .when(underline, |d| d.underline())
                        .child("This is an example link")
                        .into_any_element()
                        .into(),
                ])
                .mt(SPACING)
                .text_size(FONT_SIZE_SM)
                .text_color(t.text_secondary),
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

impl Render for PreferencesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let avatar_urls: Vec<String> = self
            .state
            .read(cx)
            .accounts
            .iter()
            .filter_map(|a| a.avatar_url.clone())
            .collect();
        for url in avatar_urls {
            Dispatcher::request_avatar_url(&url, cx);
        }
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
            PreferencesTab::Integrations => self.integrations_tab(window, cx),
            PreferencesTab::Git => self.git_tab(window, cx),
            PreferencesTab::Appearance => self.appearance_tab(cx),
            PreferencesTab::Notifications => self.notifications_tab(cx),
            PreferencesTab::Prompts => self.prompts_tab(cx),
            PreferencesTab::Advanced => self.advanced_tab(cx),
            PreferencesTab::Accessibility => self.accessibility_tab(cx),
        };
        // `.preferences-container`: nav | bordered tab container. The dialog
        // content's 20 px padding is cancelled so the nav sits flush.
        // `renderErrors`: an invalid author name blocks saving (`gitAuthorNameIsValid`).
        let name_valid = corvane_core::git_author_name_is_valid(self.name.read(cx).value().trim());
        let error = (!name_valid).then_some(corvane_core::INVALID_GIT_AUTHOR_NAME_MESSAGE);
        let content = div()
            .w(px(560.))
            .mx(px(-20.))
            .my(px(-20.))
            .flex()
            .flex_col()
            .when_some(error, |d, message| {
                d.child(
                    crate::widgets::dialog_error_banner(message, cx)
                        .mx(px(0.))
                        .mt(px(0.))
                        .mb(px(0.)),
                )
            })
            .child(
                div()
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
                    ),
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
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "prefs-save",
                    label: "Save".into(),
                    primary: true,
                    disabled: !name_valid,
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
