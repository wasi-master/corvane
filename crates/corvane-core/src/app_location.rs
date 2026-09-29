//! The Move to Applications prompt - GHD `ui/app.tsx` (`componentDidMount`
//! shows `PopupType.MoveToApplicationsFolder`), `dispatcher.ts`
//! (`moveToApplicationsFolder`, `setAskToMoveToApplicationsFolderSetting`) and
//! the Electron calls in `main-process/main.ts`
//! (`corvane_platform::app_location`).

use gpui_kit::App;
use tracing::info;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::Popup;

impl Dispatcher {
    /// At launch: offer the move when running from a bundle outside the
    /// Applications folders, unless the user said "don't ask again". Like
    /// GHD (`__DEV__ === false`), development builds never ask.
    pub fn check_move_to_applications_folder(cx: &mut App) {
        if cfg!(debug_assertions) {
            return;
        }
        if !Self::state(cx)
            .read(cx)
            .settings
            .ask_to_move_to_applications_folder
        {
            return;
        }
        let Some(bundle) = corvane_platform::app_location::running_bundle() else {
            return;
        };
        if corvane_platform::app_location::is_in_applications_folder(&bundle) {
            return;
        }
        info!(bundle = %bundle.display(), "offering to move to /Applications");
        Self::show_popup(Popup::MoveToApplicationsFolder, cx);
    }

    /// `setAskToMoveToApplicationsFolderSetting`.
    pub fn set_ask_to_move_to_applications_folder(ask: bool, cx: &mut App) {
        Self::update_settings(cx, |s| s.ask_to_move_to_applications_folder = ask);
    }

    /// "Move and Restart": move the bundle to /Applications, then quit and
    /// open it from there.
    pub fn move_to_applications_folder(cx: &mut App) {
        let Some(bundle) = corvane_platform::app_location::running_bundle() else {
            Self::show_error(
                "Could not move Corvane",
                "Corvane is not running from an app bundle.",
                cx,
            );
            return;
        };
        spawn_bg(
            cx,
            move || corvane_platform::app_location::move_to_applications_folder(&bundle),
            |result, cx| match result {
                Ok(dest) => {
                    info!(dest = %dest.display(), "moved to /Applications, restarting");
                    match corvane_platform::app_location::relaunch_after_exit(
                        &dest,
                        std::process::id(),
                    ) {
                        Ok(()) => cx.quit(),
                        Err(err) => Self::show_error(
                            "Corvane was moved but could not restart",
                            format!("Open it from {}. ({err})", dest.display()),
                            cx,
                        ),
                    }
                }
                Err(err) => Self::show_error("Could not move Corvane", err, cx),
            },
        );
    }
}

impl Dispatcher {
    /// `installDarwinCLI` (GHD `install-cli.ts`): link the bundle's
    /// `corvane` script into `/usr/local/bin`, asking for administrator
    /// rights when needed, then `CLIInstalled`; a failure is a plain error.
    pub fn install_cli(cx: &mut App) {
        let Some(packaged) = corvane_platform::cli::packaged_path() else {
            Self::show_error(
                "Could not install the command line tool",
                "The command line tool is only available when Corvane runs from Corvane.app.",
                cx,
            );
            return;
        };
        let installed = corvane_platform::cli::install_path();
        let target = installed.clone();
        spawn_bg(
            cx,
            move || corvane_platform::cli::install(&packaged, &target),
            move |result, cx| match result {
                Ok(()) => {
                    info!(path = %installed.display(), "installed the command line tool");
                    Self::show_popup(Popup::CLIInstalled { path: installed }, cx);
                }
                Err(message) => {
                    Self::show_error("Could not install the command line tool", message, cx)
                }
            },
        );
    }
}
