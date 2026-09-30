//! Corvane entry point: logging, persisted settings, GPUI application, window.

mod askpass;
mod assets;
mod dev_samples;
mod logging;
mod menus;
#[cfg(feature = "snapshots")]
mod parity_control;

use std::sync::Arc;
use std::time::Instant;

use corvane_core::{Dispatcher, Popup, Section, StoreExt, ThemeSetting};
use corvane_ui::actions::*;
use corvane_ui::workspace::Workspace;
use gpui_kit::*;
use tracing::{debug, error, info, warn};

fn main() {
    // `GIT_ASKPASS` runs this same binary; answer git and exit before touching GPUI.
    if std::env::var_os("CORVANE_ASKPASS").is_some() {
        askpass::run();
    }
    let started = Instant::now();
    let _log_guard = logging::init();
    info!(version = env!("CARGO_PKG_VERSION"), "starting corvane");
    // writes a local report only while "Save crash reports locally" is on
    corvane_platform::crash_reports::install_panic_hook(env!("CARGO_PKG_VERSION"));
    phase(started, "logging initialised");

    let store = match corvane_store::Store::open_in(corvane_platform::paths::app_support_dir()) {
        Ok(store) => Arc::new(store),
        Err(err) => {
            error!(
                ?err,
                "could not open settings store; falling back to a temporary one"
            );
            // per process, so several instances can fall back at once
            let tmp = std::env::temp_dir().join(format!("corvane-fallback-{}", std::process::id()));
            Arc::new(corvane_store::Store::open_in(tmp).expect("temporary store"))
        }
    };
    let settings = store.settings().unwrap_or_default();
    // Feature flags: the stored preset + overrides, then CORVANE_FLAGS for
    // this session (bad entries are logged and skipped). Resolved here too
    // because the theme is applied before `AppState` exists.
    let flag_overrides = store.flags().unwrap_or_default();
    let (flags_env, flag_errors) = corvane_core::flags::env::from_env();
    for err in &flag_errors {
        warn!("{err}");
    }
    let launch_flags = corvane_core::Flags::resolve(&flag_overrides, &flags_env);
    phase(started, "store opened");

    let app = gpui_kit::application().with_assets(assets::Assets);
    phase(started, "application created");
    // `app.on('activate')`: the Dock icon shows the hidden window again.
    #[cfg(target_os = "macos")]
    app.on_reopen(|cx| {
        for handle in cx.windows() {
            handle
                .update(cx, |_, window, cx| {
                    corvane_ui::native_window::show_window(window, cx)
                })
                .ok();
        }
    });

    // `x-corvane://` URLs (the `corvane` command line tool, Finder's "Open
    // in Corvane", links) may arrive before launch has finished: queue them
    let url_inbox = corvane_core::app_url::AppUrlInbox::default();
    let url_sender = url_inbox.sender();
    app.on_open_urls(move |urls| {
        for url in urls {
            url_sender.send(url);
        }
    });

    app.run(move |cx| {
        phase(started, "platform ready");
        corvane_ui::theme::preseed_kit_theme(cx);
        gpui_kit::init(cx);
        phase(started, "gpui-kit initialised");

        // CORVANE_THEME=light|dark|high-contrast overrides the saved setting (dev convenience).
        let stored_theme = settings.theme;
        let theme_setting = match std::env::var("CORVANE_THEME").as_deref() {
            Ok("light") => ThemeSetting::Light,
            Ok("dark") => ThemeSetting::Dark,
            Ok("high-contrast") => ThemeSetting::HighContrast,
            _ => settings.theme,
        };
        // `101-high-contrast-theme` off: High Contrast shows as Dark
        let high_contrast = launch_flags.bool(corvane_core::flags::ids::HIGH_CONTRAST_THEME);
        // GHD `App.render`: the welcome flow is always drawn in the light theme
        let welcome_done = settings.welcome_completed;
        let shown_theme = if welcome_done {
            theme_setting
        } else {
            ThemeSetting::Light
        };
        APPLIED_THEME.with(|t| t.set(shown_theme));
        // View › Zoom: every size in corvane-ui scales by this factor
        // (CORVANE_ZOOM=<factor> overrides the saved one for a session)
        let zoom = std::env::var("CORVANE_ZOOM")
            .ok()
            .and_then(|z| z.parse::<f32>().ok())
            .unwrap_or(settings.window_zoom_factor);
        corvane_ui::theme::sizes::set_zoom_factor(zoom);
        corvane_ui::theme::set_mono_font(corvane_platform::fonts::ghd_monospace_family());
        info!(zoom, "window zoom factor");
        corvane_ui::init(cx, resolve_theme_with(shown_theme, high_contrast, cx));
        let sidebar_width = corvane_ui::theme::sizes::zpx(settings.sidebar_width);
        let state = Dispatcher::init(store, settings, flag_overrides, flags_env, cx);
        Dispatcher::load_custom_emoji(cx);
        Dispatcher::check_crash_reports(cx);
        // a notification click brings the (possibly hidden) window forward
        // and opens its dialog; installed before the first frame so a click
        // that launched Corvane is delivered too
        Dispatcher::listen_for_notification_clicks(focus_main_window, cx);
        let service_urls = url_inbox.sender();
        corvane_platform::services::register_open_in_corvane(move |path| {
            service_urls.send(corvane_core::app_url::open_local_repo_url(&path));
        });
        Dispatcher::listen_for_app_urls(url_inbox, focus_main_window, cx);
        {
            let s = state.read(cx);
            menus::install(
                cx,
                &s.editor_label(),
                &s.shell_label(),
                s.flags
                    .bool(corvane_core::flags::ids::RELEASE_NOTES_MENU_ITEM),
                s.flags
                    .bool(corvane_core::flags::ids::IMPORT_FROM_GITHUB_DESKTOP),
            );
        }
        phase(started, "theme, keymap, menus and state installed");

        // Settings › Appearance and Integrations feed back into the theme and
        // the "Open in …" menu labels; system appearance flips the System theme.
        // compared with the stored setting, so a CORVANE_THEME override holds
        // until the user picks a theme
        let mut last_theme = stored_theme;
        let mut chosen_theme = theme_setting;
        let mut last_welcome_done = welcome_done;
        // the menu bar and the theme also depend on flags (401, 101)
        let mut last_menu_key = {
            let s = state.read(cx);
            (
                s.editor_label(),
                s.shell_label(),
                s.flags
                    .bool(corvane_core::flags::ids::RELEASE_NOTES_MENU_ITEM),
                s.flags
                    .bool(corvane_core::flags::ids::IMPORT_FROM_GITHUB_DESKTOP),
            )
        };
        let mut last_high_contrast = high_contrast;
        corvane_ui::format::sync(&state.read(cx).settings);
        cx.observe(&state, move |state, cx| {
            Dispatcher::sync_crash_reports_setting(cx);
            // accounts or Settings › Notifications changed: (un)subscribe
            Dispatcher::sync_alive_subscriptions(cx);
            let (theme, welcome_done, menu_key, high_contrast) = {
                let s = state.read(cx);
                corvane_ui::format::sync(&s.settings);
                (
                    s.settings.theme,
                    s.settings.welcome_completed,
                    (
                        s.editor_label(),
                        s.shell_label(),
                        s.flags
                            .bool(corvane_core::flags::ids::RELEASE_NOTES_MENU_ITEM),
                        s.flags
                            .bool(corvane_core::flags::ids::IMPORT_FROM_GITHUB_DESKTOP),
                    ),
                    s.flags.bool(corvane_core::flags::ids::HIGH_CONTRAST_THEME),
                )
            };
            if menu_key != last_menu_key {
                last_menu_key = menu_key;
                menus::install(
                    cx,
                    &last_menu_key.0,
                    &last_menu_key.1,
                    last_menu_key.2,
                    last_menu_key.3,
                );
            }
            let theme_changed = theme != last_theme;
            if theme_changed {
                last_theme = theme;
                chosen_theme = theme;
            }
            let high_contrast_changed = high_contrast != last_high_contrast;
            last_high_contrast = high_contrast;
            if theme_changed || high_contrast_changed || welcome_done != last_welcome_done {
                last_welcome_done = welcome_done;
                apply_theme(
                    if welcome_done {
                        chosen_theme
                    } else {
                        ThemeSetting::Light
                    },
                    cx,
                );
            }
        })
        .detach();

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &Hide, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.on_action(|_: &InstallCli, cx| Dispatcher::install_cli(cx));
        cx.on_action(|_: &ImportFromGitHubDesktop, cx| {
            let enabled = corvane_core::AppState::global(cx)
                .read(cx)
                .flags
                .bool(corvane_core::flags::ids::IMPORT_FROM_GITHUB_DESKTOP);
            if enabled {
                Dispatcher::show_popup(Popup::ImportFromGitHubDesktop, cx)
            }
        });
        cx.on_action(|_: &AddLocalRepository, cx| {
            Dispatcher::show_popup(Popup::AddExistingRepository { path: None }, cx)
        });
        cx.on_action(|_: &NewRepository, cx| {
            Dispatcher::show_popup(Popup::CreateRepository { path: None }, cx)
        });
        cx.on_action(|_: &CloneRepository, cx| {
            Dispatcher::show_popup(Popup::CloneRepository { url: None }, cx)
        });
        // CORVANE_DEV_ACCOUNTS="login@api-base,…" adds token-less accounts to
        // this session only (dev/testing convenience for the account pickers).
        if let Ok(spec) = std::env::var("CORVANE_DEV_ACCOUNTS") {
            corvane_core::AppState::global(cx).update(cx, |s, cx| {
                for (ix, item) in spec.split(',').enumerate() {
                    if let Some((login, endpoint)) = item.split_once('@') {
                        s.accounts.push(corvane_core::Account {
                            endpoint: endpoint.to_string(),
                            id: 1_000_000 + ix as u64,
                            login: login.to_string(),
                            name: None,
                            avatar_url: None,
                            emails: Vec::new(),
                            scopes: Vec::new(),
                            plan: None,
                            private_primary_email: false,
                        });
                    }
                }
                cx.notify();
            });
        }
        // CORVANE_SNAPSHOT=<png> (build with `--features snapshots`): render the
        // window offscreen after CORVANE_SNAPSHOT_DELAY_MS (default 4000), save
        // it and quit - visual checks without screen-recording permission.
        #[cfg(feature = "snapshots")]
        if let Ok(path) = std::env::var("CORVANE_SNAPSHOT") {
            let delay = std::env::var("CORVANE_SNAPSHOT_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4000u64);
            cx.spawn(async move |cx: &mut AsyncApp| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(delay))
                    .await;
                // an unfocused window only renders when drawn: draw once so
                // views start their background work (diff highlighting), give
                // it time, then draw the frame that is saved
                cx.update(|cx| {
                    for handle in cx.windows() {
                        let _ = handle.update(cx, |_, window, cx| {
                            window.refresh();
                            window.draw(cx).clear(cx);
                        });
                    }
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                cx.update(|cx| {
                    for handle in cx.windows() {
                        let image = handle.update(cx, |_, window, cx| {
                            // draw now: an unfocused window gets no display-link
                            // frames, and render_to_image uses the last scene
                            window.refresh();
                            window.draw(cx).clear(cx);
                            window.render_to_image()
                        });
                        match image {
                            Ok(Ok(image)) => match image.save(&path) {
                                Ok(()) => info!(
                                    path,
                                    zoom = corvane_ui::theme::sizes::zoom_factor(),
                                    "snapshot saved"
                                ),
                                Err(err) => error!(?err, "could not save the snapshot"),
                            },
                            Ok(Err(err)) => error!(?err, "could not render the snapshot"),
                            Err(err) => error!(?err, "no window for the snapshot"),
                        }
                    }
                    cx.quit();
                });
            })
            .detach();
        }
        // CORVANE_CONTROL=<port> (build with `--features snapshots`): remote
        // control for the GitHub Desktop parity harness (`tools/parity`).
        #[cfg(feature = "snapshots")]
        if let Some(port) = std::env::var("CORVANE_CONTROL")
            .ok()
            .and_then(|p| p.parse().ok())
        {
            parity_control::start(port, open_dev_popup, cx);
        }
        // CORVANE_ADD_REPO=/path adds a repository at launch (dev/testing convenience).
        if let Ok(path) = std::env::var("CORVANE_ADD_REPO") {
            Dispatcher::add_repository(std::path::PathBuf::from(path), cx);
        }
        // `--open-repo <path>` (GHD `--cli-open`; the command line tool now
        // sends an `openLocalRepo` URL instead): select the repository, or
        // offer to add it.
        if let Some(path) = open_repo_argument(std::env::args()) {
            Dispatcher::open_local_repository(path, cx);
        }
        // CORVANE_CLONE="<url>|<path>" clones at launch (dev/testing convenience).
        if let Ok(spec) = std::env::var("CORVANE_CLONE")
            && let Some((url, path)) = spec.split_once('|')
        {
            Dispatcher::clone_repository(url.to_string(), std::path::PathBuf::from(path), None, cx);
        }
        // CORVANE_POPUP=<name> opens a dialog at launch (dev/testing convenience
        // for headless smoke runs; API-backed dialogs get sample data):
        //   preferences[:<tab>] (accounts, integrations, git, appearance, notifications,
        //   prompts, advanced, accessibility) | repository-settings | about | create
        //   clone | clone:<url>
        //   release-notes | move-to-applications | upstream-already-exists
        //   pr-review[:approved|:commented] (changes requested by default)
        //   pr-comment | pr-checks-failed
        //   pr-list (sample pull requests in the branch foldout's Pull Requests tab)
        //   tutorial:<step> (the repository becomes the tutorial repository, shown at
        //   <step>: pick-editor, create-branch, edit-file, make-commit, push-branch,
        //   open-pull-request, all-done, announced, paused)
        //   tutorial-create | tutorial-exit (the two tutorial dialogs; tutorial-create
        //   needs CORVANE_DEV_ACCOUNTS)
        //   crash-report (turns on "Save crash reports locally" and panics, so the
        //   next launch shows "Corvane quit unexpectedly last time")
        //   no-write-access (the repository becomes a read-only GitHub repository;
        //   add CORVANE_DEV_ACCOUNTS=login@https://api.github.com for the fork dialog)
        //   test-notifications (Help › Show Test Notifications: posts sample
        //   review / comment / checks-failed notifications; needs the .app bundle)
        //   notification-click:review|comment|checks-failed (what clicking such a
        //   notification does: its userInfo payload goes through the click handler)
        //   zoom-in | zoom-out | zoom-reset (View › Zoom, with the zoom overlay)
        //   sign-in (the GitHub.com sign-in dialog)
        //   alive:review|comment|checks-failed[:api] (an Alive event for the sample
        //   pull requests through the notification handler; sample data unless :api)
        //   update-available[:brew][:about|:notes] (a sample update in the ready /
        //   Homebrew state: the banner, plus About or the Release Notes with
        //   "Install and Restart")
        //   flags[:<search>] (Corvane › Flags…, with the search box prefilled)
        if let Ok(popup) = std::env::var("CORVANE_POPUP") {
            // Deferred so a `CORVANE_ADD_REPO` repository has been added and refreshed.
            cx.spawn(async move |cx: &mut AsyncApp| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                cx.update(|cx| {
                    open_dev_popup(&popup, cx);
                });
            })
            .detach();
        }
        cx.on_action(|_: &RemoveRepository, cx| {
            if let Some(id) = corvane_core::AppState::global(cx).read(cx).selected {
                Dispatcher::request_remove_repository(id, cx);
            }
        });
        cx.on_action(|_: &OpenFlags, cx| Dispatcher::open_flags(None, cx));
        cx.on_action(|_: &OpenSettings, cx| {
            Dispatcher::open_preferences(corvane_core::PreferencesTab::Accounts, cx)
        });
        cx.on_action(|_: &About, cx| {
            Dispatcher::show_popup(
                Popup::About {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                cx,
            )
        });
        let selected_path = |cx: &App| -> Option<(u64, std::path::PathBuf)> {
            let s = corvane_core::AppState::global(cx).read(cx);
            let repo = s.selected_repository()?;
            Some((repo.id, repo.path.clone()))
        };
        cx.on_action(move |_: &RepositorySettings, cx| {
            if let Some((id, _)) = selected_path(cx) {
                Dispatcher::open_repository_settings(
                    id,
                    corvane_core::RepositorySettingsTab::Remote,
                    cx,
                );
            }
        });
        cx.on_action(move |_: &OpenInEditor, cx| {
            if let Some((_, path)) = selected_path(cx) {
                Dispatcher::open_in_editor(path, cx);
            }
        });
        cx.on_action(move |_: &OpenInShell, cx| {
            if let Some((_, path)) = selected_path(cx) {
                Dispatcher::open_in_shell(&path, cx);
            }
        });
        cx.on_action(move |_: &ShowInFinder, cx| {
            if let Some((_, path)) = selected_path(cx) {
                Dispatcher::show_in_finder(&path, cx);
            }
        });
        cx.on_action(move |_: &OpenWith, cx| {
            if let Some((_, path)) = selected_path(cx) {
                Dispatcher::open_with(path, cx);
            }
        });
        cx.on_action(move |_: &ViewOnGitHub, cx| {
            if let Some((id, _)) = selected_path(cx) {
                Dispatcher::view_on_github(id, cx);
            }
        });
        cx.on_action(move |_: &CreateIssue, cx| {
            if let Some((id, _)) = selected_path(cx) {
                Dispatcher::create_issue(id, cx);
            }
        });
        cx.on_action(move |_: &CompareOnGitHub, cx| {
            if let Some((id, _)) = selected_path(cx) {
                Dispatcher::compare_on_github(id, cx);
            }
        });
        cx.on_action(move |_: &ViewBranchOnGitHub, cx| {
            if let Some((id, _)) = selected_path(cx) {
                Dispatcher::view_branch_on_github(id, cx);
            }
        });
        cx.on_action(move |_: &CreatePullRequest, cx| {
            if let Some((id, _)) = selected_path(cx) {
                Dispatcher::create_pull_request(id, cx);
            }
        });
        cx.on_action(move |_: &PreviewPullRequest, cx| {
            if let Some((id, _)) = selected_path(cx) {
                Dispatcher::start_pull_request(id, cx);
            }
        });
        // Help
        cx.on_action(|_: &ReportIssue, cx| {
            Dispatcher::open_url("https://github.com/wasi-master/corvane/issues/new", cx)
        });
        cx.on_action(|_: &ContactSupport, cx| {
            Dispatcher::open_url("https://github.com/wasi-master/corvane/discussions", cx)
        });
        cx.on_action(|_: &ShowReleaseNotes, cx| Dispatcher::show_release_notes(cx));
        // GHD test menu "Show notification" (`testShowNotification`)
        #[cfg(debug_assertions)]
        cx.on_action(|_: &ShowTestNotifications, cx| {
            if let Some(id) = corvane_core::AppState::global(cx).read(cx).selected {
                Dispatcher::show_popup(Popup::TestNotifications { repo: id }, cx);
            }
        });
        cx.on_action(|_: &ShowUserGuides, cx| {
            Dispatcher::open_url("https://docs.github.com/en/desktop", cx)
        });
        cx.on_action(|_: &ShowKeyboardShortcuts, cx| {
            Dispatcher::open_url(
                "https://docs.github.com/en/desktop/overview/github-desktop-keyboard-shortcuts",
                cx,
            )
        });
        cx.on_action(|_: &ShowLogs, cx| {
            let dir = corvane_platform::paths::logs_dir();
            let _ = std::fs::create_dir_all(&dir);
            cx.reveal_path(&dir);
        });
        // Window
        cx.on_action(|_: &CloseWindow, cx| {
            // GHD hides the window and keeps running; the Dock brings it back.
            #[cfg(target_os = "macos")]
            if let Some(handle) = cx.active_window() {
                handle
                    .update(cx, |_, window, cx| {
                        corvane_ui::native_window::hide_window(window, cx)
                    })
                    .ok();
            }
            #[cfg(not(target_os = "macos"))]
            cx.hide();
        });
        cx.on_action(|_: &BringAllToFront, cx| cx.activate(true));

        // Same size as the GitHub Desktop reference captures in .docs.
        let window_size = size(px(1367.), px(814.));
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("Corvane".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(9.), px(9.))),
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                window_size,
                cx,
            ))),
            window_min_size: Some(size(px(960.), px(660.))),
            app_id: Some(corvane_platform::BUNDLE_ID.into()),
            ..Default::default()
        };

        let workspace = match gpui_kit::open_window(options, cx, move |window, cx| {
            cx.new(|cx| Workspace::new(state, sidebar_width, window, cx))
        }) {
            Ok((_, workspace)) => {
                info!(
                    elapsed_ms = started.elapsed().as_millis(),
                    "main window opened"
                );
                workspace
            }
            Err(err) => {
                error!(?err, "failed to open main window");
                cx.quit();
                return;
            }
        };

        // System theme follows macOS light/dark switches (`supportsSystemThemeChanges`)
        // and, back in Corvane, the "Increase contrast" display option.
        if let Some(window) = cx.active_window() {
            let ws = workspace.clone();
            window
                .update(cx, |_, window, cx| {
                    ws.update(cx, |_, cx| {
                        cx.observe_window_activation(window, |_, window, cx| {
                            let theme = APPLIED_THEME.with(|t| t.get());
                            if window.is_window_active()
                                && theme == ThemeSetting::System
                                && resolve_theme(theme, cx).name
                                    != corvane_ui::theme::ActiveGhdTheme::ghd(&**cx).name
                            {
                                apply_theme(theme, cx);
                            }
                        })
                        .detach();
                    });
                    // the red close button hides the window like ⌘W (GHD
                    // `window.on('close')` → `hide()` unless quitting)
                    #[cfg(target_os = "macos")]
                    window.on_window_should_close(cx, |window, cx| {
                        corvane_ui::native_window::hide_window(window, cx);
                        false
                    });
                    window
                        .observe_window_appearance(|_, cx| {
                            let theme = APPLIED_THEME.with(|t| t.get());
                            if theme == ThemeSetting::System {
                                apply_theme(theme, cx);
                            }
                        })
                        .detach();
                })
                .ok();
        }

        // View / Window actions are global so the menu items stay enabled whatever has focus.
        let ws = workspace.clone();
        cx.on_action(move |_: &ShowChanges, cx| {
            ws.update(cx, |w, cx| w.set_section(Section::Changes, cx))
        });
        let ws = workspace.clone();
        cx.on_action(move |_: &ShowHistory, cx| {
            ws.update(cx, |w, cx| w.set_section(Section::History, cx))
        });
        let ws = workspace.clone();
        cx.on_action(move |_: &ToggleSection, cx| {
            ws.update(cx, |w, cx| {
                let next = match w.section() {
                    Section::Changes => Section::History,
                    Section::History => Section::Changes,
                };
                w.set_section(next, cx)
            })
        });
        let ws = workspace.clone();
        cx.on_action(move |_: &ShowRepositoryList, cx| {
            if let Some(window) = cx.active_window() {
                let ws = ws.clone();
                window
                    .update(cx, move |_, window, cx| {
                        ws.update(cx, |w, cx| w.show_repository_list(window, cx))
                    })
                    .ok();
            }
        });
        let ws = workspace.clone();
        cx.on_action(move |_: &ShowBranchesList, cx| {
            if let Some(window) = cx.active_window() {
                let ws = ws.clone();
                window
                    .update(cx, move |_, window, cx| {
                        ws.update(cx, |w, cx| w.show_branches_list(window, cx))
                    })
                    .ok();
            }
        });
        let ws = workspace.clone();
        cx.on_action(move |_: &ShowWorktreesList, cx| {
            if let Some(window) = cx.active_window() {
                let ws = ws.clone();
                window
                    .update(cx, move |_, window, cx| {
                        ws.update(cx, |w, cx| w.show_worktrees_list(window, cx))
                    })
                    .ok();
            }
        });
        let ws = workspace.clone();
        cx.on_action(move |_: &GoToSummary, cx| {
            if let Some(window) = cx.active_window() {
                let ws = ws.clone();
                window
                    .update(cx, move |_, window, cx| {
                        ws.update(cx, |w, cx| w.focus_commit_summary(window, cx))
                    })
                    .ok();
            }
        });
        let ws = workspace.clone();
        cx.on_action(move |_: &Find, cx| {
            if let Some(window) = cx.active_window() {
                let ws = ws.clone();
                window
                    .update(cx, move |_, window, cx| {
                        ws.update(cx, |w, cx| w.focus_filter(window, cx))
                    })
                    .ok();
            }
        });
        let ws = workspace.clone();
        cx.on_action(move |_: &ToggleChangesFilter, cx| {
            ws.update(cx, |w, cx| w.toggle_changes_filter(cx))
        });
        // View › Reset Zoom / Zoom In / Zoom Out (GHD `zoom(ZoomDirection)`)
        let ws = workspace.clone();
        cx.on_action(move |_: &ZoomIn, cx| ws.update(cx, |w, cx| w.zoom(1, cx)));
        let ws = workspace.clone();
        cx.on_action(move |_: &ZoomOut, cx| ws.update(cx, |w, cx| w.zoom(-1, cx)));
        let ws = workspace.clone();
        cx.on_action(move |_: &ResetZoom, cx| ws.update(cx, |w, cx| w.zoom(0, cx)));
        let ws = workspace.clone();
        cx.on_action(move |_: &CompareToBranch, cx| {
            if let Some(window) = cx.active_window() {
                let ws = ws.clone();
                window
                    .update(cx, move |_, window, cx| {
                        ws.update(cx, |w, cx| w.show_compare(window, cx))
                    })
                    .ok();
            }
        });
        // Branch menu
        let selected = |cx: &App| corvane_core::AppState::global(cx).read(cx).selected;
        cx.on_action(move |_: &NewBranch, cx| {
            if let Some(id) = selected(cx) {
                Dispatcher::show_popup(
                    Popup::CreateBranch {
                        repo: id,
                        target_sha: None,
                        initial_name: String::new(),
                    },
                    cx,
                );
            }
        });
        cx.on_action(move |_: &NewWorktree, cx| {
            if let Some(id) = selected(cx) {
                Dispatcher::show_popup(
                    Popup::AddWorktree {
                        repo: id,
                        initial_branch_name: None,
                        initial_worktree_name: None,
                    },
                    cx,
                );
            }
        });
        let current_branch = |cx: &App| -> Option<(u64, String)> {
            let s = corvane_core::AppState::global(cx).read(cx);
            let id = s.selected?;
            let name = s
                .repo_states
                .get(&id)?
                .info
                .as_ref()?
                .current_branch()?
                .name
                .clone();
            Some((id, name))
        };
        cx.on_action(move |_: &RenameBranch, cx| {
            if let Some((id, name)) = current_branch(cx) {
                Dispatcher::show_popup(Popup::RenameBranch { repo: id, name }, cx);
            }
        });
        cx.on_action(move |_: &DeleteBranch, cx| {
            if let Some((id, name)) = current_branch(cx) {
                Dispatcher::show_popup(Popup::DeleteBranch { repo: id, name }, cx);
            }
        });
        cx.on_action(move |_: &MergeIntoCurrentBranch, cx| {
            if let Some((id, _)) = current_branch(cx) {
                Dispatcher::show_popup(
                    Popup::MergeBranch {
                        repo: id,
                        squash: false,
                    },
                    cx,
                );
            }
        });
        cx.on_action(move |_: &SquashAndMergeIntoCurrentBranch, cx| {
            if let Some((id, _)) = current_branch(cx) {
                Dispatcher::show_popup(
                    Popup::MergeBranch {
                        repo: id,
                        squash: true,
                    },
                    cx,
                );
            }
        });
        cx.on_action(move |_: &Push, cx| {
            if let Some(id) = selected(cx) {
                Dispatcher::push(id, false, None, cx);
            }
        });
        cx.on_action(move |_: &Pull, cx| {
            if let Some(id) = selected(cx) {
                Dispatcher::pull(id, cx);
            }
        });
        cx.on_action(move |_: &Fetch, cx| {
            if let Some(id) = selected(cx) {
                Dispatcher::fetch(id, false, cx);
            }
        });
        Dispatcher::start_background_tasks(cx);
        Dispatcher::refresh_accounts(cx);
        // Alive subscriptions for pull request notifications (GHD AliveStore)
        Dispatcher::start_alive(cx);
        Dispatcher::start_pull_request_updater(cx);
        Dispatcher::start_commit_status_refresh(cx);
        // GHD `componentDidMount`: offer the move to /Applications
        Dispatcher::check_move_to_applications_folder(cx);
        // `checkForUpdates(true)` at launch and every four hours (release builds)
        Dispatcher::start_update_checks(cx);
        // on-demand packs installed earlier (extended grammars)
        Dispatcher::load_installed_packs(cx);
        cx.on_action(move |_: &RebaseCurrentBranch, cx| {
            if let Some((id, _)) = current_branch(cx) {
                Dispatcher::start_rebase_flow(id, cx);
            }
        });
        cx.on_action(move |_: &UpdateFromDefaultBranch, cx| {
            if let Some((id, _)) = current_branch(cx) {
                Dispatcher::update_from_default_branch(id, cx);
            }
        });
        cx.on_action(move |_: &StashAllChanges, cx| {
            if let Some((id, _)) = current_branch(cx) {
                Dispatcher::stash_all_changes(id, cx);
            }
        });
        cx.on_action(move |_: &ToggleStashedChanges, cx| {
            if let Some(id) = selected(cx) {
                Dispatcher::toggle_stash_view(id, cx);
            }
        });
        cx.on_action(move |_: &DiscardAllChanges, cx| {
            if let Some(id) = selected(cx) {
                let paths: Vec<String> = corvane_core::AppState::global(cx)
                    .read(cx)
                    .repo_states
                    .get(&id)
                    .and_then(|r| r.status.as_ref())
                    .map(|st| st.files.iter().map(|f| f.path.clone()).collect())
                    .unwrap_or_default();
                Dispatcher::request_discard_changes(id, paths, cx);
            }
        });
        cx.on_action(|_: &CloseFoldout, cx| {
            Dispatcher::close_foldout(cx);
            Dispatcher::close_popup(cx);
        });
        cx.on_action(|_: &Minimize, cx| {
            if let Some(window) = cx.active_window() {
                window
                    .update(cx, |_, window, _| window.minimize_window())
                    .ok();
            }
        });
        cx.on_action(|_: &Zoom, cx| {
            if let Some(window) = cx.active_window() {
                window.update(cx, |_, window, _| window.zoom_window()).ok();
            }
        });
        cx.on_action(|_: &ToggleFullScreen, cx| {
            if let Some(window) = cx.active_window() {
                window
                    .update(cx, |_, window, _| window.toggle_fullscreen())
                    .ok();
            }
        });
        cx.activate(true);
    });
}

thread_local! {
    /// The theme setting last applied (the saved one, or `CORVANE_THEME`).
    static APPLIED_THEME: std::cell::Cell<ThemeSetting> =
        const { std::cell::Cell::new(ThemeSetting::System) };
}

/// The palette for `setting` given the system appearance, Settings ›
/// Accessibility › Display › "Increase contrast" and the
/// `101-high-contrast-theme` flag (read from the app state).
fn resolve_theme(setting: ThemeSetting, cx: &App) -> corvane_ui::theme::GhdTheme {
    let high_contrast = corvane_core::AppState::try_global(cx).is_none_or(|s| {
        s.read(cx)
            .flags
            .bool(corvane_core::flags::ids::HIGH_CONTRAST_THEME)
    });
    resolve_theme_with(setting, high_contrast, cx)
}

/// `resolve_theme` before the app state exists: with `high_contrast` off a
/// High Contrast setting shows as Dark and "Increase contrast" is ignored.
fn resolve_theme_with(
    setting: ThemeSetting,
    high_contrast: bool,
    cx: &App,
) -> corvane_ui::theme::GhdTheme {
    let system_dark = matches!(
        cx.window_appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    );
    corvane_ui::theme::GhdTheme::for_setting(
        corvane_core::flags::effective_theme(setting, high_contrast),
        system_dark,
        high_contrast && corvane_platform::accessibility::increase_contrast(),
    )
}

/// GHD `focusWindow`: bring Corvane forward and show its window, even when
/// it was hidden with ⌘W.
fn focus_main_window(cx: &mut App) {
    cx.activate(true);
    #[cfg(target_os = "macos")]
    for handle in cx.windows() {
        handle
            .update(cx, |_, window, cx| {
                corvane_ui::native_window::show_window(window, cx)
            })
            .ok();
    }
}

/// `CORVANE_POPUP=<name>` (and the parity harness's `popup` hook): open a
/// dialog or sample state for visual checks. Names are listed in `main`.
fn open_dev_popup(popup: &str, cx: &mut App) {
    let selected = corvane_core::AppState::global(cx).read(cx).selected;
    match (popup, selected) {
        ("import-ghd", _) => Dispatcher::show_popup(Popup::ImportFromGitHubDesktop, cx),
        (other, _) if other == "flags" || other.starts_with("flags:") => {
            Dispatcher::open_flags(other.strip_prefix("flags:").map(str::to_string), cx)
        }
        (other, _) if other.starts_with("preferences") => {
            use corvane_core::PreferencesTab as Tab;
            let tab = match other.strip_prefix("preferences:") {
                Some("integrations") => Tab::Integrations,
                Some("git") => Tab::Git,
                Some("appearance") => Tab::Appearance,
                Some("notifications") => Tab::Notifications,
                Some("prompts") => Tab::Prompts,
                Some("advanced") => Tab::Advanced,
                Some("accessibility") => Tab::Accessibility,
                _ => Tab::Accounts,
            };
            Dispatcher::open_preferences(tab, cx)
        }
        ("repository-settings", Some(id)) => Dispatcher::open_repository_settings(
            id,
            corvane_core::RepositorySettingsTab::Remote,
            cx,
        ),
        ("about", _) => Dispatcher::show_popup(
            Popup::About {
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            cx,
        ),
        ("create", _) => Dispatcher::show_popup(Popup::CreateRepository { path: None }, cx),
        ("clone", _) => Dispatcher::show_popup(Popup::CloneRepository { url: None }, cx),
        // GHD `showFakeUpstreamAlreadyExists` (test UI components):
        // an in-memory fork of desktop/desktop whose `upstream`
        // points elsewhere
        ("upstream-already-exists", Some(id)) => {
            let parent = corvane_core::GitHubRepository {
                endpoint: "https://api.github.com".into(),
                owner: "desktop".into(),
                name: "desktop".into(),
                html_url: "https://github.com/desktop/desktop".into(),
                clone_url: "https://github.com/desktop/desktop.git".into(),
                default_branch: Some("development".into()),
                private: false,
                fork: false,
                parent: None,
                archived: false,
                permissions: None,
                allow_forking: None,
            };
            corvane_core::AppState::global(cx).update(cx, |s, _| {
                if let Some(r) = s.repositories.iter_mut().find(|r| r.id == id) {
                    let mut fork = parent.clone();
                    fork.owner = "octocat".into();
                    fork.fork = true;
                    fork.parent = Some(Box::new(parent));
                    r.github = Some(fork);
                }
            });
            Dispatcher::show_popup(
                Popup::UpstreamAlreadyExists {
                    repo: id,
                    existing_url: "https://github.com/someone-else/desktop.git".into(),
                },
                cx,
            )
        }
        // GHD test UI components: `MoveToApplicationsFolder`
        ("move-to-applications", _) => Dispatcher::show_popup(Popup::MoveToApplicationsFolder, cx),
        // GHD `showFakeReleaseNotes` (test UI components)
        ("release-notes", _) => {
            use corvane_core::release_notes::{parse_release_body, release_summary};
            let body = "Corvane now reads more of **GitHub Desktop**'s workflow: \
                    _Markdown_ with `inline code`, ~~webviews~~ and \
                    [links](https://github.com/wasi-master/corvane).\n\n\
                    - [New] Branch autocompletion in Add Worktree\n\
                    - [Improved] Clone resolves owner/name through the API\n\
                    - [Added] Commit message templates\n\
                    - [Fixed] Arrow keys scroll the changes list. Thanks @octocat!\n\
                    - [Fixed] Upstream remote conflicts are reported\n\
                    - [Removed] Stale menu items";
            Dispatcher::show_popup(
                Popup::ReleaseNotes {
                    summary: release_summary(
                        env!("CARGO_PKG_VERSION"),
                        Some(std::time::SystemTime::now()),
                        parse_release_body(body),
                    ),
                },
                cx,
            )
        }
        // GHD `TestNotifications`: the pull request notification
        // dialogs for the CORVANE_ADD_REPO repository
        (other, Some(id)) if other.starts_with("pr-review") => {
            use corvane_github::api::ApiPullRequestReviewState as State;
            let state = match other.strip_prefix("pr-review") {
                Some(":approved") => State::Approved,
                Some(":commented") => State::Commented,
                _ => State::ChangesRequested,
            };
            Dispatcher::show_popup(
                Popup::PullRequestReview {
                    repo: id,
                    pull_request: dev_samples::pull_request(id, cx),
                    review: dev_samples::review(state),
                    should_checkout_branch: true,
                    should_change_repository: false,
                },
                cx,
            )
        }
        (other, Some(id)) if other.starts_with("tutorial:") => {
            let step = corvane_core::tutorial::TutorialStep::parse(&other["tutorial:".len()..]);
            corvane_core::AppState::global(cx).update(cx, |s, cx| {
                if let Some(r) = s.repositories.iter_mut().find(|r| r.id == id) {
                    r.is_tutorial_repository = true;
                }
                s.tutorial_step_override = step;
                cx.notify();
            });
        }
        ("tutorial-create", _) => Dispatcher::show_create_tutorial_repository(cx),
        ("tutorial-exit", _) => Dispatcher::show_popup(Popup::ConfirmExitTutorial, cx),
        ("crash-report", _) => {
            Dispatcher::update_settings(cx, |s| s.save_crash_reports = true);
            Dispatcher::sync_crash_reports_setting(cx);
            panic!("CORVANE_POPUP=crash-report: deliberate crash");
        }
        ("no-write-access", Some(id)) => dev_samples::make_read_only(id, cx),
        (other, Some(id)) if other.starts_with("notification-click:") => {
            use corvane_core::notifications::TestNotificationType as Kind;
            let kind = match &other["notification-click:".len()..] {
                "comment" => Kind::PullRequestComment,
                "checks-failed" => Kind::ChecksFailed,
                _ => Kind::PullRequestReview,
            };
            let notification = corvane_core::samples::notification(kind, id, cx);
            if let Some(payload) = Dispatcher::notification_payload(&notification) {
                Dispatcher::notification_payload_clicked(&payload, cx);
            }
        }
        (other, _) if other.starts_with("update-available") => {
            let flags: Vec<&str> = other.split(':').skip(1).collect();
            Dispatcher::install_sample_update(flags.contains(&"brew"), cx);
            if flags.contains(&"about") {
                Dispatcher::show_popup(
                    Popup::About {
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                    cx,
                );
            } else if flags.contains(&"notes") {
                Dispatcher::show_update_release_notes(cx);
            }
        }
        ("zoom-in", _) => cx.dispatch_action(&ZoomIn),
        ("zoom-out", _) => cx.dispatch_action(&ZoomOut),
        ("zoom-reset", _) => cx.dispatch_action(&ResetZoom),
        // GHD `simulateAliveEvent`: an Alive event through the real handler
        (other, Some(id)) if other.starts_with("alive:") => {
            use corvane_core::notifications::TestNotificationType as Kind;
            let parts: Vec<&str> = other.split(':').collect();
            let kind = match parts.get(1).copied() {
                Some("comment") => Kind::PullRequestComment,
                Some("checks-failed") => Kind::ChecksFailed,
                _ => Kind::PullRequestReview,
            };
            let data = if parts.contains(&"api") {
                corvane_core::AliveEventData::Api
            } else {
                corvane_core::AliveEventData::Sample
            };
            dev_samples::install_pull_requests(id, cx);
            Dispatcher::simulate_alive_event(id, kind, data, cx);
        }
        // the sign-in dialog (device flow by default, browser flow link)
        ("sign-in", _) => Dispatcher::show_popup(Popup::SignIn { enterprise: false }, cx),
        ("sign-in-enterprise", _) => Dispatcher::show_popup(Popup::SignIn { enterprise: true }, cx),
        ("test-notifications", Some(id)) => {
            Dispatcher::show_popup(Popup::TestNotifications { repo: id }, cx)
        }
        ("pr-list", Some(id)) => {
            dev_samples::install_pull_requests(id, cx);
            Dispatcher::change_branches_tab(corvane_core::BranchesTab::PullRequests, cx);
            Dispatcher::toggle_foldout(corvane_core::Foldout::Branch, cx);
        }
        ("pr-comment", Some(id)) => Dispatcher::show_popup(
            Popup::PullRequestComment {
                repo: id,
                pull_request: dev_samples::pull_request(id, cx),
                comment: dev_samples::comment(),
                should_checkout_branch: true,
                should_change_repository: false,
            },
            cx,
        ),
        ("pr-checks-failed", Some(id)) => Dispatcher::show_popup(
            Popup::PullRequestChecksFailed {
                repo: id,
                pull_request: dev_samples::pull_request(id, cx),
                checks: dev_samples::failed_checks(),
                should_change_repository: false,
            },
            cx,
        ),
        // `clone:<url>` opens the URL tab pre-filled
        (other, _) if other.starts_with("clone:") => Dispatcher::show_popup(
            Popup::CloneRepository {
                url: Some(other["clone:".len()..].to_string()),
            },
            cx,
        ),
        _ => {}
    }
}

/// Settings › Appearance › Theme: swap the palette live (`ApplicationTheme`).
fn apply_theme(setting: ThemeSetting, cx: &mut App) {
    APPLIED_THEME.with(|t| t.set(setting));
    corvane_ui::theme::apply(resolve_theme(setting, cx), cx);
    for window in cx.windows() {
        window.update(cx, |_, window, _| window.refresh()).ok();
    }
}

fn phase(started: Instant, what: &str) {
    debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "startup: {what}"
    );
}

/// `--open-repo <path>` (or `--open-repo=<path>`) from the command line tool.
fn open_repo_argument(args: impl IntoIterator<Item = String>) -> Option<std::path::PathBuf> {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--open-repo" {
            return args.next().map(std::path::PathBuf::from);
        }
        if let Some(path) = arg.strip_prefix("--open-repo=") {
            return Some(std::path::PathBuf::from(path));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::open_repo_argument;

    // `gpui_kit::*` brings GPUI's `test` macro into scope; use the std one.
    #[::core::prelude::v1::test]
    fn open_repo_argument_forms() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            open_repo_argument(args(&["corvane", "--open-repo", "/tmp/repo"])),
            Some("/tmp/repo".into())
        );
        assert_eq!(
            open_repo_argument(args(&["corvane", "-psn_0_123", "--open-repo=/a b"])),
            Some("/a b".into())
        );
        assert_eq!(open_repo_argument(args(&["corvane"])), None);
        assert_eq!(open_repo_argument(args(&["corvane", "--open-repo"])), None);
    }
}
