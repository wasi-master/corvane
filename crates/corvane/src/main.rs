//! Corvane entry point: logging, persisted settings, GPUI application, window.

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
        menus::install(cx);
        let sidebar_width = px(settings.sidebar_width);
        let state = Dispatcher::init(store, settings, cx);
        phase(started, "theme, keymap, menus and state installed");

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
        cx.on_action(|_: &RemoveRepository, cx| {
            if let Some(id) = corvane_core::AppState::global(cx).read(cx).selected {
                Dispatcher::remove_repository(id, cx);
            }
        });

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

fn phase(started: Instant, what: &str) {
    debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "startup: {what}"
    );
}
