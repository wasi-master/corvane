//! Corvane entry point: logging, persisted settings, GPUI application, window.

mod assets;
mod logging;
mod menus;

use std::sync::Arc;
use std::time::Instant;

use corvane_core::{Dispatcher, StoreExt, ThemeSetting};
use corvane_ui::actions::{AddLocalRepository, Hide, HideOthers, Quit, RemoveRepository, ShowAll};
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
        cx.on_action(|_: &AddLocalRepository, cx| Dispatcher::prompt_add_repository(cx));
        // CORVANE_ADD_REPO=/path adds a repository at launch (dev/testing convenience).
        if let Ok(path) = std::env::var("CORVANE_ADD_REPO") {
            Dispatcher::add_repository(std::path::PathBuf::from(path), cx);
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

        match gpui_kit::open_window(options, cx, move |window, cx| {
            cx.new(|cx| Workspace::new(state, sidebar_width, window, cx))
        }) {
            Ok(_) => info!(
                elapsed_ms = started.elapsed().as_millis(),
                "main window opened"
            ),
            Err(err) => {
                error!(?err, "failed to open main window");
                cx.quit();
            }
        }
        cx.activate(true);
    });
}

fn phase(started: Instant, what: &str) {
    debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "startup: {what}"
    );
}
