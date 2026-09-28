//! Corvane entry point: logging, persisted settings, GPUI application, window.

mod assets;
mod logging;
mod menus;

use corvane_core::ThemeSetting;
use corvane_ui::actions::{Hide, HideOthers, Quit, ShowAll};
use corvane_ui::workspace::Workspace;
use gpui_kit::*;
use tracing::{error, info};

fn main() {
    let _log_guard = logging::init();
    info!(version = env!("CARGO_PKG_VERSION"), "starting corvane");

    let store = match corvane_store::Store::open_default() {
        Ok(store) => Some(store),
        Err(err) => {
            error!(?err, "could not open settings store; running with defaults");
            None
        }
    };
    let settings = store
        .as_ref()
        .and_then(|s| s.settings().ok())
        .unwrap_or_default();

    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);

            let theme = match settings.theme {
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

            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_action(|_: &Hide, cx| cx.hide());
            cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
            cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());

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

            let sidebar_width = px(settings.sidebar_width);
            match gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| Workspace::new(sidebar_width, window, cx))
            }) {
                Ok(_) => {}
                Err(err) => {
                    error!(?err, "failed to open main window");
                    cx.quit();
                }
            }
            cx.activate(true);
        });
}
