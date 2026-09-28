//! GHD `RelativeTime`: "just now", "5 minutes ago", "2 hours ago", "3 days ago"…

use std::time::SystemTime;

pub fn relative(from: SystemTime) -> String {
    let secs = SystemTime::now()
        .duration_since(from)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let plural = |n: u64, unit: &str| {
        if n == 1 {
            format!("1 {unit} ago")
        } else {
            format!("{n} {unit}s ago")
        }
    };
    match secs {
        0..=59 => "just now".to_string(),
        60..=3599 => plural(secs / 60, "minute"),
        3600..=86_399 => plural(secs / 3600, "hour"),
        86_400..=2_591_999 => plural(secs / 86_400, "day"),
        2_592_000..=31_535_999 => plural(secs / 2_592_000, "month"),
        _ => plural(secs / 31_536_000, "year"),
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
        assert_eq!(relative(now - Duration::from_secs(90)), "1 minute ago");
        assert_eq!(relative(now - Duration::from_secs(7200)), "2 hours ago");
        assert_eq!(
            relative(now - Duration::from_secs(86_400 * 3)),
            "3 days ago"
        );
    }
}
