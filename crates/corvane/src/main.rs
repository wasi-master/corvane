//! Corvane entry point: logging, persisted settings, GPUI application, window.

mod askpass;
mod assets;
mod logging;
mod menus;

use std::sync::Arc;
use std::time::Instant;

use corvane_core::{Dispatcher, Popup, Section, StoreExt, ThemeSetting};
use corvane_ui::actions::*;
use corvane_ui::workspace::Workspace;
use gpui_kit::*;
use tracing::{debug, error, info};

fn main() {
    // `GIT_ASKPASS` runs this same binary; answer git and exit before touching GPUI.
    if std::env::var_os("CORVANE_ASKPASS").is_some() {
        askpass::run();
    }
    let started = Instant::now();
    let _log_guard = logging::init();
    info!(version = env!("CARGO_PKG_VERSION"), "starting corvane");
    phase(started, "logging initialised");

    let store = match corvane_store::Store::open_in(corvane_platform::paths::app_support_dir()) {
        Ok(store) => Arc::new(store),
        Err(err) => {
            error!(
                ?err,
                "could not open settings store; falling back to a temporary one"
            );
            let tmp = std::env::temp_dir().join("corvane-fallback");
            Arc::new(corvane_store::Store::open_in(tmp).expect("temporary store"))
        }
    };
    let settings = store.settings().unwrap_or_default();
    phase(started, "store opened");

    let app = gpui_kit::application().with_assets(assets::Assets);
    phase(started, "application created");

    app.run(move |cx| {
        phase(started, "platform ready");
        corvane_ui::theme::preseed_kit_theme(cx);
        gpui_kit::init(cx);
        phase(started, "gpui-kit initialised");

        // CORVANE_THEME=light|dark overrides the saved setting (dev convenience).
        let theme_setting = match std::env::var("CORVANE_THEME").as_deref() {
            Ok("light") => ThemeSetting::Light,
            Ok("dark") => ThemeSetting::Dark,
            _ => settings.theme,
        };
        let theme = match theme_setting {
            ThemeSetting::Light => corvane_ui::theme::Appearance::Light,
            ThemeSetting::Dark => corvane_ui::theme::Appearance::Dark,
            ThemeSetting::System => match cx.window_appearance() {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => {
                    corvane_ui::theme::Appearance::Dark
                }
                _ => corvane_ui::theme::Appearance::Light,
            },
        };
        corvane_ui::init(cx, theme);
        let sidebar_width = px(settings.sidebar_width);
        let state = Dispatcher::init(store, settings, cx);
        {
            let s = state.read(cx);
            menus::install(cx, &s.editor_label(), &s.shell_label());
        }
        phase(started, "theme, keymap, menus and state installed");

        // Settings › Appearance and Integrations feed back into the theme and
        // the "Open in …" menu labels; system appearance flips the System theme.
        let mut last_theme = theme_setting;
        let mut last_labels = {
            let s = state.read(cx);
            (s.editor_label(), s.shell_label())
        };
        corvane_ui::format::sync(&state.read(cx).settings);
        cx.observe(&state, move |state, cx| {
            let (theme, labels) = {
                let s = state.read(cx);
                corvane_ui::format::sync(&s.settings);
                (s.settings.theme, (s.editor_label(), s.shell_label()))
            };
            if labels != last_labels {
                last_labels = labels;
                menus::install(cx, &last_labels.0, &last_labels.1);
            }
            if theme != last_theme {
                last_theme = theme;
                apply_theme(theme, cx);
            }
        })
        .detach();

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &Hide, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.on_action(|_: &AddLocalRepository, cx| {
            Dispatcher::show_popup(Popup::AddExistingRepository { path: None }, cx)
        });
        cx.on_action(|_: &NewRepository, cx| {
            Dispatcher::show_popup(Popup::CreateRepository { path: None }, cx)
        });
        cx.on_action(|_: &CloneRepository, cx| {
            Dispatcher::show_popup(Popup::CloneRepository { url: None }, cx)
        });
        // CORVANE_ADD_REPO=/path adds a repository at launch (dev/testing convenience).
        if let Ok(path) = std::env::var("CORVANE_ADD_REPO") {
            Dispatcher::add_repository(std::path::PathBuf::from(path), cx);
        }
        // CORVANE_CLONE="<url>|<path>" clones at launch (dev/testing convenience).
        if let Ok(spec) = std::env::var("CORVANE_CLONE")
            && let Some((url, path)) = spec.split_once('|')
        {
            Dispatcher::clone_repository(url.to_string(), std::path::PathBuf::from(path), cx);
        }
        // CORVANE_POPUP=preferences|repository-settings|about|create|clone opens a dialog at
        // launch (dev/testing convenience for headless smoke runs).
        if let Ok(popup) = std::env::var("CORVANE_POPUP") {
            // Deferred so a `CORVANE_ADD_REPO` repository has been added and refreshed.
            cx.spawn(async move |cx: &mut AsyncApp| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                cx.update(|cx| {
                    let selected = corvane_core::AppState::global(cx).read(cx).selected;
                    match (popup.as_str(), selected) {
                        ("preferences", _) => {
                            Dispatcher::open_preferences(corvane_core::PreferencesTab::Accounts, cx)
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
                        ("create", _) => {
                            Dispatcher::show_popup(Popup::CreateRepository { path: None }, cx)
                        }
                        ("clone", _) => {
                            Dispatcher::show_popup(Popup::CloneRepository { url: None }, cx)
                        }
                        _ => {}
                    }
                });
            })
            .detach();
        }
        cx.on_action(|_: &RemoveRepository, cx| {
            if let Some(id) = corvane_core::AppState::global(cx).read(cx).selected {
                Dispatcher::request_remove_repository(id, cx);
            }
        });
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
        // Help
        cx.on_action(|_: &ReportIssue, cx| {
            Dispatcher::open_url("https://github.com/wasi-master/corvane/issues/new", cx)
        });
        cx.on_action(|_: &ContactSupport, cx| {
            Dispatcher::open_url("https://github.com/wasi-master/corvane/discussions", cx)
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
            // GHD keeps running with the window closed; GPUI has no per-window
            // hide, so the app hides (⌘H) and comes back from the Dock.
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
            app_id: Some("com.wasimaster.corvane".into()),
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

        // System theme follows macOS light/dark switches (`supportsSystemThemeChanges`).
        if let Some(window) = cx.active_window() {
            window
                .update(cx, |_, window, _cx| {
                    window
                        .observe_window_appearance(|_, cx| {
                            let theme = corvane_core::AppState::global(cx).read(cx).settings.theme;
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
                Dispatcher::show_popup(Popup::AddWorktree { repo: id }, cx);
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

/// Settings › Appearance › Theme: swap the palette live (`ApplicationTheme`).
fn apply_theme(setting: ThemeSetting, cx: &mut App) {
    let appearance = match setting {
        ThemeSetting::Light => corvane_ui::theme::Appearance::Light,
        ThemeSetting::Dark => corvane_ui::theme::Appearance::Dark,
        ThemeSetting::System => match cx.window_appearance() {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => {
                corvane_ui::theme::Appearance::Dark
            }
            _ => corvane_ui::theme::Appearance::Light,
        },
    };
    corvane_ui::theme::apply(corvane_ui::theme::GhdTheme::for_appearance(appearance), cx);
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
