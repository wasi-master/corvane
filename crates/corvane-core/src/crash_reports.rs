//! Local crash reports at launch (Corvane addition; see
//! `corvane_platform::crash_reports`): with "Save crash reports locally" on,
//! the reports written since the previous launch open `CrashReportFound`.
//! The launch time is recorded on every start, so turning the setting on
//! never surfaces old reports.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui_kit::App;
use tracing::{debug, info};

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::Popup;

impl Dispatcher {
    /// At launch: look for reports newer than the previous launch, then
    /// record this launch.
    pub fn check_crash_reports(cx: &mut App) {
        let (enabled, since) = {
            let s = &Self::state(cx).read(cx).settings;
            (s.save_crash_reports, s.last_launched_at)
        };
        corvane_platform::crash_reports::set_enabled(enabled);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        debug!(enabled, ?since, now, "checking for crash reports");
        Self::update_settings(cx, |s| s.last_launched_at = Some(now));
        let (true, Some(since)) = (enabled, since) else {
            return;
        };
        let since = UNIX_EPOCH + Duration::from_secs(since);
        spawn_bg(
            cx,
            move || {
                corvane_platform::crash_reports::reports_since(
                    &corvane_platform::crash_reports::crashes_dir(),
                    &corvane_platform::crash_reports::diagnostic_reports_dir(),
                    since,
                )
            },
            |reports, cx| {
                if reports.is_empty() {
                    return;
                }
                info!(
                    count = reports.len(),
                    "found crash reports from the last session"
                );
                Self::show_popup(Popup::CrashReportFound { reports }, cx);
            },
        );
    }

    /// Settings › Advanced › "Save crash reports locally" takes effect at once.
    pub fn sync_crash_reports_setting(cx: &App) {
        corvane_platform::crash_reports::set_enabled(
            Self::state(cx).read(cx).settings.save_crash_reports,
        );
    }
}
