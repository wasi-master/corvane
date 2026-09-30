//! App-level dialogs: About (`ui/about/about.tsx`), Remove Repository
//! confirmation (`ui/remove-repository/confirm-remove-repository.tsx`), the
//! external editor error (`ui/editor/editor-error.tsx`) and the shell error
//! (`ui/shell/shell-error.tsx`).

use corvane_core::{AppState, Dispatcher, Popup, PreferencesTab, UpdateStatus};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{ListRowA11y, link_button};

/// `402-about-extras`: the architecture after the version and the Source
/// code link.
fn about_extras(cx: &App) -> bool {
    AppState::global(cx)
        .read(cx)
        .flags
        .bool(corvane_core::flags::ids::ABOUT_EXTRAS)
}

pub struct AboutDialog {
    state: Entity<AppState>,
    version: String,
}

impl AboutDialog {
    pub fn new(state: Entity<AppState>, version: String, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, version }
    }

    /// `renderUpdateDetails` + `renderUpdateButton`: what the updater is
    /// doing, and "Check for Updates" / "Quit and Install Update".
    fn update_section(&self, cx: &App) -> Div {
        let t = cx.ghd();
        // `.update-status` and the button `Row`, each 10 px above what follows
        let section = div().w_full().flex().flex_col().items_center();
        if !corvane_core::updater::updates_enabled() {
            // `renderUpdateDetails` without `canCheckForUpdates`: a <p>, no button
            return section.child(div().mb(SPACING()).text_align(TextAlign::Center).child(
                "The application is currently running in development and will not \
                         receive any updates.",
            ));
        }
        let (status, last_check) = {
            let s = self.state.read(cx);
            (s.update.status.clone(), s.update.last_successful_check)
        };
        let info = |text: String, loading: bool| {
            div()
                .id("about-update-status")
                .a11y_live(text.clone())
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(SPACING_HALF())
                .text_align(TextAlign::Center)
                .when(loading, |d| {
                    d.child(crate::icons::loading("about-update-spinner", t.text))
                })
                .child(text)
        };
        let details: Option<AnyElement> = match &status {
            UpdateStatus::Checking => {
                Some(info("Checking for updates…".into(), true).into_any_element())
            }
            UpdateStatus::Downloading {
                received, total, ..
            } => {
                let text = match total {
                    Some(total) if *total > 0 => {
                        format!("Downloading update… {}%", (received * 100 / total).min(100))
                    }
                    _ => "Downloading update…".to_string(),
                };
                Some(info(text, true).into_any_element())
            }
            UpdateStatus::Installing => {
                Some(info("Installing update…".into(), true).into_any_element())
            }
            UpdateStatus::NotAvailable => last_check.map(|at| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .whitespace_nowrap()
                    .child("You have the latest version (last checked\u{a0}")
                    .child(crate::relative_time::relative(at))
                    .child(")")
                    .into_any_element()
            }),
            UpdateStatus::Ready { .. } => Some(
                info(
                    "An update has been downloaded and is ready to be installed.".into(),
                    false,
                )
                .into_any_element(),
            ),
            UpdateStatus::AvailableViaHomebrew { update } => Some(
                info(
                    format!(
                        "Corvane {} is available. Run brew upgrade corvane to install it.",
                        update.version
                    ),
                    false,
                )
                .into_any_element(),
            ),
            UpdateStatus::NotChecked => None,
        };
        let button = match &status {
            UpdateStatus::Ready { .. } => {
                crate::widgets::button("about-install-update", "Quit and Install Update", cx)
                    .on_click(|_, _, cx| Dispatcher::install_update(cx))
            }
            _ => {
                let enabled = status.can_check();
                crate::widgets::button("about-check-updates", "Check for Updates", cx)
                    .when(!enabled, |d| d.opacity(0.6))
                    .on_click(move |_, _, cx| {
                        if enabled {
                            Dispatcher::check_for_updates(true, cx);
                        }
                    })
            }
        };
        section
            .children(details.map(|d| div().mb(SPACING()).child(d)))
            .child(div().mb(SPACING()).child(button))
    }
}

impl Render for AboutDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let version = self.version.clone();
        let version_for_copy = version.clone();
        // `renderUpdateErrors`: no successful check on record yet
        let no_check_yet = corvane_core::updater::updates_enabled()
            && self.state.read(cx).update.last_successful_check.is_none();
        // `About`: an untitled dialog (no header), everything centred
        let content = div()
            .flex()
            .flex_col()
            .items_center()
            .text_align(TextAlign::Center)
            .when(no_check_yet, |d| {
                d.child(crate::widgets::dialog_error_banner(
                    "Couldn't determine the last time an update check was performed. You may be \
                     running an old version. Please try manually checking for updates.",
                    cx,
                ))
            })
            .child(img("icon/Corvane-256.png").size(zpx(64.)).mb(SPACING()))
            .child(
                div()
                    .mb(zpx(6.))
                    .text_size(FONT_SIZE_MD())
                    .line_height(zpx(21.))
                    .font_weight(FontWeight::BOLD)
                    .child("About Corvane"),
            )
            .child(
                // "Version x (arch) (release notes)"; the version copies on click
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .id("about-version")
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    version_for_copy.clone(),
                                ))
                            })
                            .child(if about_extras(cx) {
                                format!("Version {version} ({})", std::env::consts::ARCH)
                            } else {
                                format!("Version {version}")
                            }),
                    )
                    .child("\u{a0}(")
                    .child(
                        link_button("about-release-notes", "release notes", cx).on_click(
                            |_, _, cx| {
                                Dispatcher::open_url(
                                    corvane_core::release_notes::RELEASE_NOTES_URL,
                                    cx,
                                )
                            },
                        ),
                    )
                    .child(")"),
            )
            .child(self.update_section(cx))
            .child(
                // `.terms-and-license-container`: 10 px above, 3 px padded links
                div()
                    .mt(SPACING())
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        // GHD `onShowAcknowledgements`
                        link_button("about-license", "License and Open Source Notices", cx)
                            .p(zpx(3.))
                            .on_click(|_, _, cx| {
                                Dispatcher::show_popup(Popup::Acknowledgements, cx)
                            }),
                    )
                    // `402-about-extras`
                    .when(about_extras(cx), |d| {
                        d.child(
                            link_button("about-source", "Source code", cx)
                                .p(zpx(3.))
                                .on_click(|_, _, cx| {
                                    Dispatcher::open_url(
                                        "https://github.com/wasi-master/corvane",
                                        cx,
                                    )
                                }),
                        )
                    }),
            );
        crate::dialog::dialog_with_frame(
            "dialog-about",
            "About Corvane",
            content,
            vec![DialogButton {
                id: "about-close",
                label: "Close".into(),
                primary: true,
                disabled: false,
                on_click: Box::new(close),
            }],
            crate::dialog::DialogFrame {
                show_header: false,
                focus_primary: true,
                ..Default::default()
            },
            close,
            window,
            cx,
        )
    }
}

/// `ConfirmRemoveRepository`: warning dialog with "Also move this repository
/// to Trash"; Remove is destructive so Cancel is the default button.
pub struct ConfirmRemoveRepositoryDialog {
    state: Entity<AppState>,
    repo: u64,
    move_to_trash: bool,
    /// The autofocused checkbox's ring, until a mouse press.
    focus_visible: bool,
}

impl ConfirmRemoveRepositoryDialog {
    pub fn new(state: Entity<AppState>, repo: u64) -> Self {
        Self {
            state,
            repo,
            move_to_trash: false,
            focus_visible: true,
        }
    }
}

impl Render for ConfirmRemoveRepositoryDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (name, path, missing) = {
            let s = self.state.read(cx);
            match s.repository(self.repo) {
                Some(r) => (r.name(), r.path.display().to_string(), r.missing),
                None => (String::new(), String::new(), true),
            }
        };
        let repo = self.repo;
        let trash = self.move_to_trash;
        let weak = cx.weak_entity();
        let content = div()
            .flex()
            .flex_col()
            .child(div().mb(SPACING()).child(format!(
                "Are you sure you want to remove the repository \"{name}\" from Corvane?"
            )))
            .child(
                // `.description`: 11 px secondary text, the path as a <Ref>
                div()
                    .mb(SPACING())
                    .flex()
                    .flex_col()
                    .text_size(FONT_SIZE_SM())
                    .line_height(zpx(16.5))
                    .text_color(t.text_secondary)
                    .child("The repository will be removed from Corvane:")
                    // `<Ref>` breaks anywhere (`word-break: break-all`): a
                    // monospace run on the path-segment background
                    .child({
                        let style = window.text_style();
                        let mut mono = style.clone();
                        mono.font_family = crate::theme::mono_font().into();
                        let mut run = mono.to_run(path.len());
                        run.background_color = Some(t.path_segment_background);
                        StyledText::new(path).with_runs(vec![run])
                    }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.focus_visible = false;
                    cx.notify();
                }),
            )
            .when(!missing, |d| {
                d.child(crate::widgets::checkbox_row_focus(
                    "remove-repo-trash",
                    trash,
                    "Also move this repository to Trash",
                    self.focus_visible,
                    move |value, _, cx| {
                        weak.update(cx, |this, cx| {
                            this.move_to_trash = value;
                            cx.notify();
                        })
                        .ok();
                    },
                    cx,
                ))
            });
        dialog_with_kind(
            "dialog-confirm-remove-repository",
            DialogKind::Warning,
            "Remove Repository",
            content,
            vec![
                DialogButton {
                    id: "remove-repo-cancel",
                    label: "Cancel".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "remove-repo-ok",
                    label: "Remove".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        if trash {
                            Dispatcher::remove_repository_and_trash(repo, cx);
                        } else {
                            Dispatcher::remove_repository(repo, cx);
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

/// `ExternalEditorError` / `OpenShellFailed`: error dialogs whose secondary
/// button opens Settings › Integrations (or the suggested editor's site).
pub struct IntegrationErrorDialog {
    popup: Popup,
}

impl IntegrationErrorDialog {
    pub fn new(popup: Popup) -> Self {
        Self { popup }
    }
}

impl Render for IntegrationErrorDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (id, title, message, secondary): (
            &'static str,
            &'static str,
            String,
            Option<DialogButton>,
        ) = match &self.popup {
            Popup::ExternalEditorError {
                message,
                suggest_default_editor,
                open_preferences,
            } => {
                let secondary = if *suggest_default_editor {
                    Some(DialogButton {
                        id: "editor-error-download",
                        label: format!(
                            "Download {}",
                            corvane_platform::editors::SUGGESTED_EDITOR_NAME
                        )
                        .into(),
                        primary: false,
                        disabled: false,
                        on_click: Box::new(|_, cx| {
                            Dispatcher::close_popup(cx);
                            Dispatcher::open_url(
                                corvane_platform::editors::SUGGESTED_EDITOR_URL,
                                cx,
                            );
                        }),
                    })
                } else if *open_preferences {
                    Some(DialogButton {
                        id: "editor-error-settings",
                        label: "Open Settings".into(),
                        primary: false,
                        disabled: false,
                        on_click: Box::new(|_, cx| {
                            Dispatcher::open_preferences(PreferencesTab::Integrations, cx)
                        }),
                    })
                } else {
                    None
                };
                (
                    "dialog-external-editor-error",
                    "Unable to Open External Editor",
                    message.clone(),
                    secondary,
                )
            }
            Popup::ShellError { message } => (
                "dialog-shell-error",
                "Unable to Open Shell",
                message.clone(),
                Some(DialogButton {
                    id: "shell-error-settings",
                    label: "Open Settings".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(|_, cx| {
                        Dispatcher::open_preferences(PreferencesTab::Integrations, cx)
                    }),
                }),
            ),
            _ => ("dialog-integration-error", "Error", String::new(), None),
        };
        let mut buttons = Vec::new();
        if let Some(secondary) = secondary {
            buttons.push(secondary);
        }
        buttons.push(DialogButton {
            id: "integration-error-close",
            label: "Close".into(),
            primary: true,
            disabled: false,
            on_click: Box::new(close),
        });
        dialog_with_kind(
            id,
            DialogKind::Error,
            title,
            div().w(zpx(450.)).child(message),
            buttons,
            close,
            window,
            cx,
        )
    }
}
