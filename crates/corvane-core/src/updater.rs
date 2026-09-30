//! Update state and flow - GHD `ui/lib/update-store.ts` (`UpdateStatus`,
//! `checkForUpdates`, `quitAndInstallUpdate`), the 4-hour timer and
//! `isUpdateAvailableBannerVisible` in `ui/app.tsx`, on top of
//! `corvane_platform::updater`.
//!
//! GHD lets Squirrel download the update as soon as one is found and shows
//! the banner once it is ready; Corvane does the same (download + minisign
//! verification in the background), then "Install and Restart" swaps the
//! bundle on request. Deviations: one release (the latest) feeds the release
//! notes, not every release since the running version; a Homebrew install
//! is told to `brew upgrade corvane` instead of being swapped; checks run
//! only in release builds unless `CORVANE_UPDATE_CHECK=1` (debug builds also
//! honour `CORVANE_UPDATE_INSTALL=1`: install as soon as the update is ready).
//! The launch and four-hourly checks can be switched off
//! (`506-no-automatic-update-checks`; GHD always checks).

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use corvane_platform::updater::{self, ReleaseInfo, UpdateError};
use gpui_kit::{App, AsyncApp};
use tracing::{error, info, warn};

use crate::dispatcher::Dispatcher;
use crate::release_notes::{ReleaseSummary, release_summary};
use crate::remote::spawn_bg;
use crate::state::Popup;

/// GHD `UpdateCheckInterval`.
const CHECK_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);

/// A release newer than the running version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub version: String,
    pub html_url: String,
    /// The release notes, as the `ReleaseNotes` dialog shows them.
    pub summary: ReleaseSummary,
}

impl AvailableUpdate {
    /// `heading_kinds` is `504-release-notes-heading-kinds`.
    fn from_release(release: &ReleaseInfo, heading_kinds: bool) -> Self {
        let published = release
            .published_at
            .as_deref()
            .and_then(corvane_models::parse_iso8601);
        let entries = crate::release_notes::parse_release_body_with(
            release.body.as_deref().unwrap_or_default(),
            heading_kinds,
        );
        Self {
            version: release.version.clone(),
            html_url: release.html_url.clone(),
            summary: release_summary(&release.version, published, entries),
        }
    }
}

/// GHD `UpdateStatus`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateStatus {
    /// `UpdateNotChecked`
    #[default]
    NotChecked,
    /// `CheckingForUpdates`
    Checking,
    /// `UpdateAvailable`: found, downloading and verifying.
    Downloading {
        update: AvailableUpdate,
        received: u64,
        total: Option<u64>,
    },
    /// `UpdateNotAvailable`
    NotAvailable,
    /// `UpdateReady`: verified zip on disk, "Install and Restart" swaps it in.
    Ready {
        update: AvailableUpdate,
        zip: PathBuf,
    },
    /// Corvane: the bundle belongs to the Homebrew cask, `brew upgrade`
    /// installs the update.
    AvailableViaHomebrew { update: AvailableUpdate },
    /// `InstallingUpdate`: the bundle swap is running; quits when done.
    Installing,
}

impl UpdateStatus {
    /// The release an "update available" banner is about.
    pub fn available(&self) -> Option<&AvailableUpdate> {
        match self {
            UpdateStatus::Ready { update, .. } | UpdateStatus::AvailableViaHomebrew { update } => {
                Some(update)
            }
            _ => None,
        }
    }

    /// GHD `canCheckForUpdates` button state: only when idle.
    pub fn can_check(&self) -> bool {
        matches!(self, UpdateStatus::NotChecked | UpdateStatus::NotAvailable)
    }
}

/// `IUpdateState` + the banner flag.
#[derive(Clone, Debug, Default)]
pub struct UpdateState {
    pub status: UpdateStatus,
    /// `lastSuccessfulCheck`
    pub last_successful_check: Option<SystemTime>,
    /// `isUpdateAvailableBannerVisible`: set when an update becomes ready,
    /// cleared by the banner's ✕ for the rest of the session.
    pub banner_visible: bool,
    /// Whether the check in flight was started from About (errors are shown)
    /// or the timer (errors are logged).
    pub user_initiated: bool,
    /// Ignore results of superseded checks.
    pub nonce: u64,
}

/// Updates run in release builds, and in debug builds with
/// `CORVANE_UPDATE_CHECK=1` (GHD: never on the development channel).
pub fn updates_enabled() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("CORVANE_UPDATE_CHECK").is_some()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Dispatcher {
    /// At launch: restore the last check time, drop the `.old` bundle a
    /// previous update left, then check after a jittered delay and every
    /// four hours (`setInterval(() => this.checkForUpdates(true), …)`).
    pub fn start_update_checks(cx: &mut App) {
        let last = Self::state(cx)
            .read(cx)
            .settings
            .last_successful_update_check
            .map(|secs| UNIX_EPOCH + Duration::from_secs(secs));
        Self::state(cx).update(cx, |s, _| s.update.last_successful_check = last);
        if let Some(bundle) = corvane_platform::app_location::running_bundle() {
            cx.background_executor()
                .spawn(async move {
                    let _ = updater::remove_old_bundle(&bundle);
                })
                .detach();
        }
        if !updates_enabled() {
            info!("update checks are off in this build");
            return;
        }
        if updater::public_key_is_placeholder() {
            warn!("built without CORVANE_UPDATE_PUBLIC_KEY: updates will fail verification");
        }
        // a few seconds' jitter so a fleet of launches does not hit the API
        // at once; `CORVANE_UPDATE_CHECK=1` checks right away
        let jitter = if cfg!(debug_assertions) {
            Duration::from_secs(2)
        } else {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            Duration::from_secs(15 + u64::from(nanos % 45))
        };
        cx.spawn(async move |cx: &mut AsyncApp| {
            cx.background_executor().timer(jitter).await;
            loop {
                cx.update(|cx| {
                    // `506-no-automatic-update-checks`: About's Check for
                    // Updates still works
                    let off = Self::state(cx)
                        .read(cx)
                        .flags
                        .bool(crate::flags::ids::NO_AUTOMATIC_UPDATE_CHECKS);
                    if !off {
                        Self::check_for_updates(false, cx);
                    }
                });
                cx.background_executor().timer(CHECK_INTERVAL).await;
            }
        })
        .detach();
    }

    /// `checkForUpdates`: ask the feed; download and verify a newer release.
    /// A check while an update is downloading or ready does nothing (GHD
    /// returns early on `UpdateReady`).
    pub fn check_for_updates(user_initiated: bool, cx: &mut App) {
        let nonce = {
            let state = Self::state(cx);
            let s = state.read(cx);
            if !s.update.status.can_check() {
                return;
            }
            state.update(cx, |s, cx| {
                s.update.nonce += 1;
                s.update.status = UpdateStatus::Checking;
                s.update.user_initiated = user_initiated;
                cx.notify();
                s.update.nonce
            })
        };
        let version = env!("CARGO_PKG_VERSION");
        spawn_bg(
            cx,
            move || updater::check_latest(version),
            move |result, cx| Self::update_check_finished(nonce, user_initiated, result, cx),
        );
    }

    fn update_check_finished(
        nonce: u64,
        user_initiated: bool,
        result: Result<Option<ReleaseInfo>, UpdateError>,
        cx: &mut App,
    ) {
        let state = Self::state(cx);
        if state.read(cx).update.nonce != nonce {
            return;
        }
        match result {
            Ok(None) => {
                info!("no update available");
                Self::touch_last_update_check(cx);
                state.update(cx, |s, cx| {
                    s.update.status = UpdateStatus::NotAvailable;
                    cx.notify();
                });
            }
            Ok(Some(release)) => {
                info!(version = %release.version, "update available");
                Self::touch_last_update_check(cx);
                let update = AvailableUpdate::from_release(&release, Self::heading_kinds(cx));
                let homebrew = corvane_platform::app_location::running_bundle()
                    .is_some_and(|b| updater::is_homebrew_install(&b));
                if homebrew {
                    state.update(cx, |s, cx| {
                        s.update.status = UpdateStatus::AvailableViaHomebrew { update };
                        s.update.banner_visible = true;
                        cx.notify();
                    });
                    return;
                }
                state.update(cx, |s, cx| {
                    s.update.status = UpdateStatus::Downloading {
                        update: update.clone(),
                        received: 0,
                        total: Some(release.zip_size).filter(|s| *s > 0),
                    };
                    cx.notify();
                });
                Self::download_update(nonce, release, update, cx);
            }
            Err(err) => {
                Self::update_failed(nonce, user_initiated, "check for updates", err, cx);
            }
        }
    }

    /// Download the zip and its signature, verify, then `Ready`.
    fn download_update(nonce: u64, release: ReleaseInfo, update: AvailableUpdate, cx: &mut App) {
        let state = Self::state(cx);
        let (tx, rx) = async_channel::unbounded::<(u64, Option<u64>)>();
        // progress feed for About's "Downloading update…"
        cx.spawn({
            let state = state.clone();
            async move |cx: &mut AsyncApp| {
                while let Ok((received, total)) = rx.recv().await {
                    state.update(cx, |s, cx| {
                        if s.update.nonce == nonce
                            && let UpdateStatus::Downloading {
                                received: r,
                                total: t,
                                ..
                            } = &mut s.update.status
                        {
                            *r = received;
                            if total.is_some() {
                                *t = total;
                            }
                            cx.notify();
                        }
                    });
                }
            }
        })
        .detach();
        let user_initiated = state.read(cx).update.user_initiated;
        spawn_bg(
            cx,
            move || {
                let mut last_sent = 0u64;
                let mut progress = |received: u64, total: Option<u64>| {
                    // at most one message per 512 KB
                    if received == 0 || received - last_sent >= 512 * 1024 {
                        last_sent = received;
                        let _ = tx.try_send((received, total));
                    }
                };
                let zip = updater::download(&release, &mut progress)?;
                let _ = tx.try_send((release.zip_size.max(last_sent), None));
                updater::verify(&zip)?;
                Ok::<PathBuf, UpdateError>(zip)
            },
            move |result, cx| {
                let state = Self::state(cx);
                if state.read(cx).update.nonce != nonce {
                    return;
                }
                match result {
                    Ok(zip) => {
                        info!(zip = %zip.display(), "update downloaded and verified");
                        state.update(cx, |s, cx| {
                            s.update.status = UpdateStatus::Ready { update, zip };
                            s.update.banner_visible = true;
                            cx.notify();
                        });
                        // CORVANE_UPDATE_INSTALL=1: install as soon as the update
                        // is ready (headless test of the swap + relaunch)
                        if cfg!(debug_assertions)
                            && std::env::var_os("CORVANE_UPDATE_INSTALL").is_some()
                        {
                            Self::install_update(cx);
                        }
                    }
                    Err(err) => {
                        Self::update_failed(nonce, user_initiated, "download the update", err, cx)
                    }
                }
            },
        );
    }

    /// `504-release-notes-heading-kinds`
    fn heading_kinds(cx: &App) -> bool {
        Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::RELEASE_NOTES_HEADING_KINDS)
    }

    /// GHD `onAutoUpdaterError`: back to `UpdateNotAvailable`; user-initiated
    /// checks show the error (`postError`), background ones only log it
    /// (`503-quiet-background-update-errors`).
    fn update_failed(nonce: u64, user_initiated: bool, what: &str, err: UpdateError, cx: &mut App) {
        error!(%err, "could not {what}");
        Self::state(cx).update(cx, |s, cx| {
            if s.update.nonce == nonce {
                s.update.status = UpdateStatus::NotAvailable;
                cx.notify();
            }
        });
        // `503-quiet-background-update-errors` off: post them as GHD does
        let quiet = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::QUIET_BACKGROUND_UPDATE_ERRORS);
        if user_initiated || !quiet {
            Self::show_error(
                "Could not check for updates",
                format!("Corvane could not {what}: {err}"),
                cx,
            );
        }
    }

    /// `touchLastChecked`
    fn touch_last_update_check(cx: &mut App) {
        let now = SystemTime::now();
        Self::state(cx).update(cx, |s, _| s.update.last_successful_check = Some(now));
        Self::update_settings(cx, |s| s.last_successful_update_check = Some(now_secs()));
    }

    /// The banner's ✕ (`setUpdateBannerVisibility(false)`).
    pub fn dismiss_update_banner(cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if s.update.banner_visible {
                s.update.banner_visible = false;
                cx.notify();
            }
        });
    }

    /// "what's new": the new release's notes; the dialog offers "Install and
    /// Restart" while the update is ready.
    pub fn show_update_release_notes(cx: &mut App) {
        let summary = Self::state(cx)
            .read(cx)
            .update
            .status
            .available()
            .map(|u| u.summary.clone());
        match summary {
            Some(summary) => Self::show_popup(Popup::ReleaseNotes { summary }, cx),
            // GHD: without release notes, open the releases page
            None => Self::open_url(crate::release_notes::RELEASE_NOTES_URL, cx),
        }
    }

    /// `quitAndInstallUpdate`: swap the bundle, relaunch it after this
    /// process exits, quit.
    pub fn install_update(cx: &mut App) {
        let state = Self::state(cx);
        let zip = match &state.read(cx).update.status {
            UpdateStatus::Ready { zip, .. } => zip.clone(),
            _ => return,
        };
        let Some(bundle) = corvane_platform::app_location::running_bundle() else {
            Self::show_error(
                "Could not install the update",
                "Corvane is not running from an app bundle.",
                cx,
            );
            return;
        };
        Self::close_popup(cx);
        state.update(cx, |s, cx| {
            s.update.status = UpdateStatus::Installing;
            cx.notify();
        });
        let pid = std::process::id();
        spawn_bg(
            cx,
            {
                let bundle = bundle.clone();
                move || {
                    updater::install(&zip, &bundle)?;
                    corvane_platform::app_location::relaunch_after_exit(&bundle, pid)
                        .map_err(UpdateError::Install)
                }
            },
            move |result, cx| match result {
                Ok(()) => {
                    info!(bundle = %bundle.display(), "update installed, restarting");
                    cx.quit();
                }
                Err(err) => {
                    error!(%err, "could not install the update");
                    Self::state(cx).update(cx, |s, cx| {
                        s.update.status = UpdateStatus::NotAvailable;
                        s.update.banner_visible = false;
                        cx.notify();
                    });
                    Self::show_error(
                        "Could not install the update",
                        format!(
                            "{err}\n\nDownload the release from {} and replace Corvane.app by hand.",
                            crate::release_notes::RELEASE_NOTES_URL
                        ),
                        cx,
                    );
                }
            },
        );
    }

    /// `CORVANE_POPUP=update-available[:brew]`: a sample update in the ready
    /// (or Homebrew) state so the banner, About and Release Notes can be seen
    /// without a release feed.
    pub fn install_sample_update(homebrew: bool, cx: &mut App) {
        let running = env!("CARGO_PKG_VERSION");
        let version = bump_patch(running);
        let body = format!(
            "Corvane {version} keeps up with GitHub Desktop.\n\n\
             - [New] Self-updater: verified downloads, install and restart\n\
             - [Improved] Release notes open from the update banner\n\
             - [Fixed] Banner icons follow the theme. Thanks @octocat!"
        );
        let release = ReleaseInfo {
            version: version.clone(),
            tag: format!("v{version}"),
            name: Some(format!("Corvane {version}")),
            body: Some(body),
            published_at: None,
            html_url: format!("https://github.com/wasi-master/corvane/releases/tag/v{version}"),
            zip_name: format!("Corvane-{version}-macos-universal.zip"),
            zip_url: String::new(),
            zip_size: 0,
            signature_url: String::new(),
        };
        let mut update = AvailableUpdate::from_release(&release, Self::heading_kinds(cx));
        update.summary.date_published = Some(SystemTime::now());
        Self::state(cx).update(cx, |s, cx| {
            s.update.status = if homebrew {
                UpdateStatus::AvailableViaHomebrew { update }
            } else {
                UpdateStatus::Ready {
                    update,
                    zip: updater::updates_dir().join(release.zip_name.clone()),
                }
            };
            s.update.last_successful_check = Some(SystemTime::now());
            s.update.banner_visible = true;
            cx.notify();
        });
    }
}

/// `0.1.0` → `0.1.1` (sample data).
fn bump_patch(version: &str) -> String {
    let mut parts: Vec<u64> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    while parts.len() < 3 {
        parts.push(0);
    }
    parts[2] += 1;
    parts
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_version_bumps_the_patch() {
        assert_eq!(bump_patch("0.1.0"), "0.1.1");
        assert_eq!(bump_patch("1.2"), "1.2.1");
    }

    #[test]
    fn only_ready_states_carry_a_release() {
        assert!(UpdateStatus::NotChecked.available().is_none());
        assert!(UpdateStatus::NotChecked.can_check());
        assert!(!UpdateStatus::Checking.can_check());
        assert!(!UpdateStatus::Installing.can_check());
    }
}
