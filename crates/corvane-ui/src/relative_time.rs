//! GHD `RelativeTime`: "just now", "5 minutes ago", "2 hours ago", "3 days ago"…

use std::time::SystemTime;

pub fn relative(from: SystemTime) -> String {
    // Settings › Appearance › "Prefer absolute dates over relative".
    if crate::format::prefer_absolute_dates() {
        return crate::format::format_date(from);
    }
    let now = SystemTime::now();
    let past_ms = match now.duration_since(from) {
        Ok(d) => d.as_millis() as f64,
        // `getRelativeTimeInfoFromDate`: more than a minute ahead shows the date
        Err(e) if e.duration().as_secs() > 60 => return crate::format::format_date(from),
        Err(_) => 0.0,
    };
    if past_ms < 60_000.0 {
        return "just now".to_string();
    }
    format_relative_past(past_ms)
}

/// GHD `formatRelative` (lib/format-relative.ts, after github/time-elements):
/// each unit is the rounded previous one, worded like
/// `Intl.RelativeTimeFormat('en-US', { numeric: 'auto' })`.
fn format_relative_past(ms: f64) -> String {
    let sec = (ms / 1000.0).round();
    let min = (sec / 60.0).round();
    let hr = (min / 60.0).round();
    let day = (hr / 24.0).round();
    let month = (day / 30.0).round();
    let year = (month / 12.0).round();
    let (n, unit) = if sec < 45.0 {
        (sec, "second")
    } else if min < 45.0 {
        (min, "minute")
    } else if hr < 24.0 {
        (hr, "hour")
    } else if day < 30.0 {
        (day, "day")
    } else if month < 18.0 {
        (month, "month")
    } else {
        (year, "year")
    };
    let n = n as u64;
    match (n, unit) {
        (0, "second") => "now".to_string(),
        (0, other) => format!("this {other}"),
        (1, "day") => "yesterday".to_string(),
        (1, "month") => "last month".to_string(),
        (1, "year") => "last year".to_string(),
        (1, other) => format!("1 {other} ago"),
        (n, other) => format!("{n} {other}s ago"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn buckets() {
        let now = SystemTime::now();
        assert_eq!(relative(now), "just now");
        assert_eq!(relative(now - Duration::from_secs(90)), "2 minutes ago");
        assert_eq!(relative(now - Duration::from_secs(7200)), "2 hours ago");
        assert_eq!(
            relative(now - Duration::from_secs(86_400 * 3)),
            "3 days ago"
        );
    }

    #[test]
    fn rounds_and_words_like_intl_numeric_auto() {
        let day = 86_400_000.0;
        assert_eq!(format_relative_past(27.6 * day), "28 days ago");
        assert_eq!(format_relative_past(day), "yesterday");
        assert_eq!(format_relative_past(42.0 * day), "last month");
        assert_eq!(format_relative_past(58.0 * day), "2 months ago");
        assert_eq!(format_relative_past(44.0 * 60_000.0), "44 minutes ago");
        assert_eq!(format_relative_past(50.0 * 60_000.0), "1 hour ago");
        assert_eq!(format_relative_past(600.0 * day), "2 years ago");
    }
}
