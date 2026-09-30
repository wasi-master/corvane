//! Settings dialog (`ui/preferences/preferences.tsx` + the per-tab
//! components). 600 px wide, a 190 px vertical tab bar beside tab content at
//! least 440 px tall; Cancel / Save.
//!
//! Deviations from GHD 3.6.6: no Copilot tab, no Copilot prompt checkbox (flag
//! `512-copilot-prompt-omitted`; with it off the checkbox is shown and only
//! stored), no Hooks sub-tab under Git (hook environment loading is not
//! implemented), no
//! Usage section under Advanced (no telemetry; "Save crash reports locally"
//! and Optional components sit there instead, scrolling within 440 px), and
//! no Formatting section (behind a feature flag in GHD).
//! With flag `custom-editor-name` the custom editor form has a Name box used
//! in "Open in …" labels (GHD `CustomIntegrationForm` has path and arguments).

use std::path::Path;
use std::rc::Rc;

use corvane_core::{
    AppState, Dispatcher, Popup, PreferencesSave, PreferencesTab, Settings, SyntaxHighlighter,
    TAB_SIZE_DEFAULT, ThemeSetting, UncommittedChangesStrategy,
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
    Inline, ListRowA11y, SelectHandler, button, checkbox_row, code_ref, labeled, link_button,
    paragraph, radio, radio_row, section_heading, select_button, settings_description, text_box,
};

/// Settings › Advanced › "Save crash reports locally" (where
/// `corvane_platform::crash_reports` writes and looks).
#[cfg(target_os = "macos")]
const CRASH_REPORTS_DESCRIPTION: &str = "When Corvane crashes, a report is saved in \
     ~/Library/Logs/Corvane/crashes and pointed out at the next launch, together with macOS's \
     own crash reports. Reports never leave this Mac.";
#[cfg(not(target_os = "macos"))]
const CRASH_REPORTS_DESCRIPTION: &str = "When Corvane crashes, a report is saved in \
     ~/.local/state/corvane/crashes and pointed out at the next launch, together with the \
     system's own crash reports. Reports never leave this computer.";

/// Error messages start lowercase (they follow "could not …"); a sentence
/// of their own starts with a capital.
fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

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
    /// The Accounts tab's `dialog-preferred-focus` button shows its focus
    /// ring until a mouse press moves focus without `:focus-visible`.
    preferred_focus_visible: bool,
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
    /// The custom editor's menu name (flag `custom-editor-name`).
    custom_editor_name: Entity<InputState>,
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
        let custom_editor_name = custom_input("Custom Editor", editor.name, cx);
        let custom_shell_path = custom_input("Path to executable", shell.path, cx);
        let custom_shell_args = custom_input("Command line arguments", shell.arguments, cx);
        for input in [
            &custom_editor_path,
            &custom_editor_args,
            &custom_editor_name,
        ] {
            cx.observe(input, |this, _, cx| {
                let path = this.custom_editor_path.read(cx).value().trim().to_string();
                let bundle_id = bundle_id_for(&path, this.draft.custom_editor.as_ref());
                this.draft.custom_editor = Some(corvane_core::CustomIntegration {
                    path,
                    arguments: this.custom_editor_args.read(cx).value().trim().to_string(),
                    bundle_id,
                    name: this.custom_editor_name.read(cx).value().trim().to_string(),
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
                    name: String::new(),
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
            preferred_focus_visible: true,
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
            custom_editor_name,
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
                .gap(SPACING())
                .mb(SPACING())
                .child(crate::widgets::avatar_image(
                    account
                        .avatar_url
                        .as_deref()
                        .and_then(|u| crate::widgets::avatar_lookup_url(u, cx)),
                    zpx(34.),
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
                None => accounts_call_to_action(
                    "prefs-signin-dotcom",
                    "Sign in to your GitHub.com account to access your repositories.",
                    "Sign Into GitHub.com",
                    self.preferred_focus_visible,
                    |_, cx| Dispatcher::show_popup(Popup::SignIn { enterprise: false }, cx),
                    cx,
                )
                .mb(SPACING())
                .into_any_element(),
            })
            .child(section_heading("GitHub Enterprise", cx))
            .children(
                enterprise
                    .iter()
                    .map(|account| account_row(account, "prefs-signout-enterprise")),
            )
            .child(if enterprise.is_empty() {
                accounts_call_to_action(
                    "prefs-signin-enterprise",
                    "If you are using GitHub Enterprise at work, sign in to it to get access to your repositories.",
                    "Sign Into GitHub Enterprise",
                    false,
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
    /// `name`: the custom editor's menu-name box (flag `custom-editor-name`).
    fn custom_form(
        &self,
        id: &'static str,
        path: &Entity<InputState>,
        args: &Entity<InputState>,
        name: Option<&Entity<InputState>>,
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
                .gap(SPACING_HALF())
                .text_size(FONT_SIZE_SM())
                .text_color(t.form_error_text)
                .child(crate::icons::octicon(Octicon::Alert, t.input_icon_error).size(zpx(12.)))
                .child(message)
        };
        let path_for_choose = path.clone();
        div()
            .flex()
            .flex_col()
            .gap(SPACING_HALF())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap(SPACING())
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
            .when_some(name, |d, name| {
                d.child(labeled(
                    "Name",
                    text_box(
                        SharedString::from(format!("{id}-name")),
                        name,
                        None,
                        window,
                        cx,
                    ),
                    cx,
                ))
            })
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
            .gap(SPACING())
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
                let name = s
                    .flags
                    .bool(corvane_core::flags::ids::CUSTOM_EDITOR_NAME)
                    .then_some(&self.custom_editor_name);
                d.child(self.custom_form(
                    "custom-editor",
                    &self.custom_editor_path,
                    &self.custom_editor_args,
                    name,
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
                    None,
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
                    dot: false,
                    id: "prefs-git-author",
                    label: "Author".into(),
                    count: None,
                },
                TabModel {
                    dot: false,
                    id: "prefs-git-default-branch",
                    label: "Default branch".into(),
                    count: None,
                },
                TabModel {
                    dot: false,
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
                .text_size(FONT_SIZE_SM())
                .on_click(|_, _, cx| Dispatcher::edit_global_git_config(cx))
                .into_any_element()
                .into(),
                ".".into(),
            ])
            .mt(SPACING())
            .text_size(FONT_SIZE_SM())
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
                    .gap(SPACING())
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
                        // `GitConfigUserForm` labels only the account-email
                        // select; flag `602-git-config-email-label` labels the
                        // lone text box too
                        let label_lone = self
                            .state
                            .read(cx)
                            .flags
                            .bool(corvane_core::flags::ids::GIT_CONFIG_EMAIL_LABEL);
                        d.child(if emails.is_empty() && label_lone {
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
                                    .text_size(FONT_SIZE_SM())
                                    .on_click(|_, _, cx| {
                                        Dispatcher::open_url(
                                            "https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-personal-account-on-github/managing-email-preferences/setting-your-commit-email-address",
                                            cx,
                                        )
                                    })
                                    .into_any_element()
                                    .into(),
                            ])
                            .text_size(FONT_SIZE_SM())
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
                                .mt(SPACING_THIRD())
                                .text_size(FONT_SIZE_SM())
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
                        .mt(SPACING())
                        .text_size(FONT_SIZE_SM())
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
            .m(zpx(-20.))
            .flex()
            .flex_col()
            .child(tabs)
            .child(div().p(SPACING_DOUBLE()).child(body))
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
                d.child(div().mt(SPACING()).child(checkbox_row(
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
            .mt(SPACING())
            .flex()
            .flex_col()
            .child(section_heading("Formatting", cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(SPACING())
                    .mb(SPACING())
                    .child(
                        labeled(
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
                        )
                        // the selects size to their longest option: 207 px here,
                        // the time format takes the rest
                        .flex_none()
                        .w(zpx(207.)),
                    )
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
            .child(div().my(SPACING()).child(checkbox_row(
                "prefs-absolute-dates",
                self.draft.prefer_absolute_dates,
                "Prefer absolute dates over relative",
                self.edit(cx, |s, v| s.prefer_absolute_dates = v),
                cx,
            )))
    }

    fn appearance_tab(&self, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        // `101-high-contrast-theme` off: GHD's three swatches, and a saved
        // High Contrast shows as Dark (the setting itself is kept)
        let high_contrast = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::HIGH_CONTRAST_THEME);
        let themes: Vec<(ThemeSetting, &str)> = [
            (ThemeSetting::Light, "Light"),
            (ThemeSetting::Dark, "Dark"),
            (ThemeSetting::System, "System"),
            // Corvane addition (GHD has no high contrast theme)
            (ThemeSetting::HighContrast, "High Contrast"),
        ]
        .into_iter()
        .filter(|(theme, _)| high_contrast || *theme != ThemeSetting::HighContrast)
        .collect();
        let selected = corvane_core::flags::effective_theme(self.draft.theme, high_contrast);
        let swatches = div()
            .flex()
            .flex_row()
            .mx(zpx(-5.))
            .children(themes.iter().map(|(theme, label)| {
                let theme = *theme;
                let is_selected = theme == selected;
                let compact = theme == ThemeSetting::HighContrast;
                let weak = cx.weak_entity();
                let id: &'static str = match theme {
                    ThemeSetting::Light => "prefs-theme-light",
                    ThemeSetting::Dark => "prefs-theme-dark",
                    ThemeSetting::System => "prefs-theme-system",
                    ThemeSetting::HighContrast => "prefs-theme-high-contrast",
                };
                let image = |path: &'static str| {
                    img(path)
                        .w_full()
                        .h(zpx(60.))
                        .object_fit(ObjectFit::Cover)
                        .border_b_1()
                        .border_color(t.box_border)
                };
                div()
                    .id(id)
                    .flex_1()
                    .min_w_0()
                    .p(SPACING_HALF())
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
                            // as tall as the two-line High Contrast swatch
                            .h_full()
                            .flex()
                            .flex_col()
                            .rounded(BORDER_RADIUS())
                            .border_1()
                            .border_color(if is_selected {
                                t.box_selected_active_background
                            } else {
                                t.box_border
                            })
                            .overflow_hidden()
                            .pb(SPACING_HALF())
                            .child(match theme {
                                ThemeSetting::Light => {
                                    image("illustrations/ghd_light.svg").into_any_element()
                                }
                                ThemeSetting::Dark => {
                                    image("illustrations/ghd_dark.svg").into_any_element()
                                }
                                ThemeSetting::HighContrast => {
                                    image("illustrations/ghd_high_contrast.svg").into_any_element()
                                }
                                // `.system-theme-swatch`: light on the left half, dark on the right.
                                ThemeSetting::System => div()
                                    .relative()
                                    .w_full()
                                    .h(zpx(60.))
                                    .border_b_1()
                                    .border_color(t.box_border)
                                    .overflow_hidden()
                                    .child(
                                        div().absolute().inset_0().child(
                                            img("illustrations/ghd_light.svg")
                                                .w_full()
                                                .h(zpx(60.))
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
                                                    .w(zpx(230.))
                                                    .child(
                                                        img("illustrations/ghd_dark.svg")
                                                            .w_full()
                                                            .h(zpx(60.))
                                                            .object_fit(ObjectFit::Cover),
                                                    ),
                                            ),
                                    )
                                    .into_any_element(),
                            })
                            .child(
                                div()
                                    .mt(SPACING_HALF())
                                    // the fourth swatch's two-line label needs the room
                                    .px(if compact { zpx(3.) } else { SPACING() })
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(if compact { zpx(3.) } else { SPACING_HALF() })
                                    .child(radio(
                                        ElementId::from(SharedString::from(format!("{id}-radio"))),
                                        is_selected,
                                        cx,
                                    ))
                                    .child(if compact {
                                        div().min_w_0().line_height(zpx(14.)).child(*label)
                                    } else {
                                        div().whitespace_nowrap().child(*label)
                                    }),
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
        let tree_sitter = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::TREE_SITTER_HIGHLIGHTING);
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
            // the absolute-dates checkbox's 10 px margin spaces the section
            .child(section_heading("Diff", cx))
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
            // Corvane addition: `105-tree-sitter-highlighting`
            .when(tree_sitter, |d| d.child(self.syntax_highlighter_field(cx)))
            .into_any_element()
    }

    /// Appearance › Diff › Syntax highlighting: the engine and, when the
    /// choice needs grammars this build does not have, their download.
    fn syntax_highlighter_field(&self, cx: &Context<Self>) -> AnyElement {
        const CHOICES: [(SyntaxHighlighter, &str); 3] = [
            (SyntaxHighlighter::GitHubDesktop, "GitHub Desktop"),
            (
                SyntaxHighlighter::TreeSitterFallback,
                "Tree-sitter for other languages",
            ),
            (SyntaxHighlighter::TreeSitter, "Tree-sitter"),
        ];
        let t = cx.ghd();
        let current = self.draft.syntax_highlighter;
        let options: Vec<SharedString> = CHOICES.iter().map(|(_, l)| (*l).into()).collect();
        let selected_ix = CHOICES.iter().position(|(c, _)| *c == current);
        let weak = cx.weak_entity();
        let on_select: SelectHandler = Rc::new(move |ix, _, cx| {
            if let Some((choice, _)) = CHOICES.get(ix) {
                weak.update(cx, |this, cx| {
                    this.draft.syntax_highlighter = *choice;
                    cx.notify();
                })
                .ok();
            }
        });
        let description = match current {
            SyntaxHighlighter::GitHubDesktop => {
                "The CodeMirror modes GitHub Desktop uses; files in other languages \
                 get syntect's grammars."
            }
            SyntaxHighlighter::TreeSitterFallback => {
                "GitHub Desktop's colours where it highlights a language, tree-sitter for \
                 the others."
            }
            SyntaxHighlighter::TreeSitter => {
                "Tree-sitter for every language it has a grammar for, GitHub Desktop's \
                 highlighter for the rest."
            }
        };
        let packs = &self.state.read(cx).packs;
        let missing = packs.missing_for(current);
        let status = missing.map(|kind| {
            let (text, action) = self.pack_state(kind, "prefs-syntax-pack", cx);
            let text = format!("{}: {text}", kind.title());
            let error = packs.errors.get(&kind).map(|e| capitalize(e));
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .mt(SPACING())
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap(SPACING())
                        .child(
                            div()
                                .id("prefs-syntax-pack-status")
                                .a11y_live(text.clone())
                                .flex_1()
                                .min_w_0()
                                .text_size(FONT_SIZE_SM())
                                .child(text),
                        )
                        .children(action.map(|a| div().flex_none().child(a))),
                )
                .children(error.map(|e| {
                    div()
                        .mt(SPACING_HALF())
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.error)
                        .child(e)
                }))
        });
        div()
            .flex()
            .flex_col()
            .mt(SPACING())
            .child(labeled(
                "Syntax highlighting",
                select_button(
                    "prefs-syntax-highlighter",
                    options
                        .get(selected_ix.unwrap_or(0))
                        .cloned()
                        .unwrap_or_default(),
                    options,
                    selected_ix,
                    false,
                    on_select,
                    cx,
                ),
                cx,
            ))
            .child(settings_description(cx).child(description))
            .children(status)
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
                .on_click(move |_, _, cx| corvane_core::Dispatcher::open_url(&url, cx))
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
                    div().mt(SPACING()).child(paragraph(vec![
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
                    .child(paragraph(parts).line_height(zpx(16.)))
                    .children(warning),
            )
            .into_any_element()
    }

    fn prompts_tab(&self, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let copilot_prompt_omitted = self
            .state
            .read(cx)
            .flags
            .bool(corvane_core::flags::ids::COPILOT_PROMPT_OMITTED);
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
                    .gap(SPACING_HALF())
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
                    // `512-copilot-prompt-omitted`
                    .when(!copilot_prompt_omitted, |el| {
                        el.child(checkbox_row(
                            "prefs-confirm-commit-message-override",
                            d.confirm_commit_message_override,
                            "Overriding commit message with generated message",
                            self.edit(cx, |s, v| s.confirm_commit_message_override = v),
                            cx,
                        ))
                    })
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
            .child(div().mt(SPACING()).child(section_heading(
                "If I have changes and I switch branches...",
                cx,
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF())
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
                    .mt(SPACING())
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
        let (crash_reports, offered_packs) = {
            use corvane_core::flags::ids;
            let flags = &self.state.read(cx).flags;
            (
                flags.bool(ids::CRASH_REPORTS),
                corvane_core::offered_packs(flags),
            )
        };
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
                    .gap(SPACING())
                    .child(
                        "These icons indicate which repositories have local or remote changes, and require the periodic fetching of repositories that are not currently selected.",
                    )
                    .child(
                        "Turning this off will not stop the periodic fetching of your currently selected repository, but may improve overall app performance for users with many repositories.",
                    ),
            )
            .child(div().mt(SPACING()).child(section_heading("Network and credentials", cx)))
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
                        .text_size(FONT_SIZE_SM())
                        .on_click(|_, _, cx| Dispatcher::open_url("https://gh.io/gcm", cx))
                        .into_any_element()
                        .into(),
                    " for private repositories outside of GitHub.com. This feature is experimental and subject to change.".into(),
                ])
                .mt(SPACING())
                .text_size(FONT_SIZE_SM())
                .text_color(t.text_secondary),
            )
            // Corvane addition in place of GHD's Usage section (no telemetry);
            // `501-crash-reports`
            .when(crash_reports, |d| {
                d.child(div().mt(SPACING()).child(section_heading("Crash reports", cx)))
                    .child(checkbox_row(
                        "prefs-save-crash-reports",
                        self.draft.save_crash_reports,
                        "Save crash reports locally",
                        self.edit(cx, |s, v| s.save_crash_reports = v),
                        cx,
                    ))
                    .child(
                        settings_description(cx).child(CRASH_REPORTS_DESCRIPTION),
                    )
            })
            // Corvane addition: on-demand packs;
            // `502-optional-components`, `105-tree-sitter-highlighting`
            .when(!offered_packs.is_empty(), |d| {
                d.child(div().mt(SPACING()).child(section_heading("Optional components", cx)))
                    .children(offered_packs.iter().map(|kind| self.pack_row(*kind, cx)))
            })
            .into_any_element()
    }

    /// One on-demand pack: its state (compiled in / installed / downloading /
    /// not installed) and the Download / Remove action.
    fn pack_row(&self, kind: corvane_packs::PackKind, cx: &Context<Self>) -> AnyElement {
        use corvane_packs::PackKind;
        let t = cx.ghd();
        let packs = &self.state.read(cx).packs;
        let description = match kind {
            PackKind::SyntaxExtended => {
                "Syntax highlighting for the languages beyond the built-in set \
                 (two-face's full grammar collection). Downloaded from Corvane's \
                 GitHub releases and verified before use."
            }
            PackKind::TreeSitterAll => {
                "Tree-sitter grammars for every language, for Appearance › Syntax \
                 highlighting. Downloaded from Corvane's GitHub releases and verified \
                 before use."
            }
            PackKind::TreeSitterRest => {
                "Tree-sitter grammars for the languages GitHub Desktop does not \
                 highlight, enough for \"Tree-sitter for other languages\"."
            }
            PackKind::GitPortable => "A private copy of Git for machines without one.",
            PackKind::GitLfs => "Git Large File Storage for repositories that use it.",
        };
        let id = format!("prefs-pack-{}", kind.name());
        let (status, action) = self.pack_state(kind, &id, cx);
        let error = packs.errors.get(&kind).map(|e| capitalize(e));
        div()
            .flex()
            .flex_col()
            .mt(SPACING())
            .child(div().font_weight(FontWeight::SEMIBOLD).child(kind.title()))
            .child(settings_description(cx).mt(zpx(2.)).child(description))
            .child(
                div()
                    .mt(SPACING())
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(SPACING())
                    .child(
                        div()
                            .id(SharedString::from(format!("{id}-status")))
                            .a11y_live(status.clone())
                            .flex_1()
                            .min_w_0()
                            .text_size(FONT_SIZE_SM())
                            .child(status),
                    )
                    .children(action.map(|a| div().flex_none().child(a))),
            )
            .children(error.map(|e| {
                div()
                    .mt(SPACING_HALF())
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.error)
                    .child(e)
            }))
            .into_any_element()
    }

    /// A pack's status line and its action (Download / Update + Remove),
    /// with element ids under `id`.
    fn pack_state(
        &self,
        kind: corvane_packs::PackKind,
        id: &str,
        cx: &Context<Self>,
    ) -> (String, Option<AnyElement>) {
        use corvane_packs::PackKind;
        let packs = &self.state.read(cx).packs;
        let entry = packs
            .manifest
            .as_ref()
            .and_then(|m| m.entry_for(kind, env!("CARGO_PKG_VERSION")).cloned());
        let size_mb = |bytes: u64| format!("{:.1} MB", bytes as f64 / 1_048_576.0);
        let element_id = |suffix: &str| SharedString::from(format!("{id}-{suffix}"));
        if packs.bundled(kind) {
            return ("Included in this build.".to_string(), None);
        }
        if let Some(progress) = packs.progress.get(&kind) {
            let text = match progress.total {
                Some(total) if total > 0 => format!(
                    "Downloading… {}%",
                    (progress.received * 100 / total).min(100)
                ),
                _ if packs.manifest_loading => "Checking what is available…".to_string(),
                _ => "Downloading…".to_string(),
            };
            return (text, None);
        }
        if let Some(installed) = packs.installed.get(&kind) {
            let newer = entry
                .as_ref()
                .filter(|e| e.version != installed.version)
                .map(|e| e.version.clone());
            let text = match newer {
                Some(v) => format!(
                    "Installed (version {}; {v} is available).",
                    installed.version
                ),
                None => format!("Installed (version {}).", installed.version),
            };
            let mut actions = div().flex().flex_row().items_center().gap(SPACING());
            if entry
                .as_ref()
                .is_some_and(|e| e.version != installed.version)
            {
                actions = actions.child(
                    button(element_id("update"), "Update", cx)
                        .on_click(move |_, _, cx| Dispatcher::install_pack(kind, cx)),
                );
            }
            actions = actions.child(
                button(element_id("remove"), "Remove", cx)
                    .on_click(move |_, _, cx| Dispatcher::uninstall_pack(kind, cx)),
            );
            return (text, Some(actions.into_any_element()));
        }
        if kind == PackKind::TreeSitterRest && packs.available(PackKind::TreeSitterAll) {
            return (
                "Not needed: the grammars for every language are installed.".to_string(),
                None,
            );
        }
        let label = match &entry {
            Some(e) if e.size > 0 => format!("Download ({})", size_mb(e.size)),
            _ => "Download".to_string(),
        };
        let text = match (&entry, &packs.manifest_error, packs.manifest_loading) {
            (Some(_), _, _) => "Not installed.".to_string(),
            (None, _, true) => "Not installed. Checking what is available…".to_string(),
            (None, Some(err), _) => format!("Not installed. {}", capitalize(err)),
            (None, None, _) if packs.manifest.is_some() => {
                "Not installed. No version for this Corvane is published.".to_string()
            }
            (None, None, _) => "Not installed.".to_string(),
        };
        let enabled = entry.is_some();
        (
            text,
            Some(
                button(element_id("download"), label, cx)
                    .when(!enabled, |d| d.opacity(0.6))
                    .on_click(move |_, _, cx| {
                        if enabled {
                            Dispatcher::install_pack(kind, cx);
                        }
                    })
                    .into_any_element(),
            ),
        )
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
                .mt(SPACING())
                .text_size(FONT_SIZE_SM())
                .text_color(t.text_secondary),
            )
            .child(div().mt(SPACING()).child(checkbox_row(
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
        // the 600 px dialog less its border
        let content = div()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.preferred_focus_visible {
                        this.preferred_focus_visible = false;
                        cx.notify();
                    }
                }),
            )
            .w(zpx(598.))
            .mx(zpx(-20.))
            .my(zpx(-20.))
            .flex()
            .flex_col()
            .when_some(error, |d, message| {
                d.child(
                    crate::widgets::dialog_error_banner(message, cx)
                        .mx(zpx(0.))
                        .mt(zpx(0.))
                        .mb(zpx(0.)),
                )
            })
            .child(
                div()
                    // `dialog#preferences .dialog-content { min-height: 440px }`
                    .min_h(zpx(440.))
                    .flex()
                    .flex_row()
                    .items_stretch()
                    .child(nav)
                    .child(
                        div()
                            .id("prefs-tab-container")
                            .flex_1()
                            .min_w_0()
                            .border_l_1()
                            .border_color(t.box_border)
                            .p(SPACING_DOUBLE())
                            // Advanced carries Corvane's extra sections (crash
                            // reports, optional components): it scrolls inside
                            // GHD's 440 px instead of growing the dialog
                            .when(self.tab == PreferencesTab::Advanced, |d| {
                                d.max_h(zpx(440.)).overflow_y_scroll()
                            })
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

/// `#preferences .accounts-tab .call-to-action { display: block }`: the text
/// (10 px right margin), then the button 10 px below at its own width.
fn accounts_call_to_action(
    id: &'static str,
    body: &'static str,
    action_title: &'static str,
    focused: bool,
    on_action: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let t = cx.ghd();
    div()
        .flex()
        .flex_col()
        .items_start()
        .child(div().self_stretch().mr(SPACING()).child(body))
        .child(
            div()
                .relative()
                .mt(SPACING())
                .when(focused, |d| d.child(crate::widgets::focus_ring(cx)))
                .child(
                    crate::widgets::primary_button(id, action_title, false, cx)
                        // `:focus` takes the hover background
                        .when(focused, |d| d.bg(t.button_hover_background))
                        .on_click(move |_, window, cx| on_action(window, cx)),
                ),
        )
}
