//! Settings › Appearance › Formatting (GHD `models/formatting-preferences.ts`,
//! `lib/format-date.ts`, `lib/format-number.ts`): the date-fns patterns GHD
//! offers, rendered in local time, plus the number separators.

use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use corvane_core::Settings;

#[derive(Clone, Debug)]
struct Prefs {
    date_format: String,
    time_format: String,
    thousands: String,
    decimal: String,
    prefer_absolute_dates: bool,
}

static PREFS: RwLock<Option<Prefs>> = RwLock::new(None);

/// Mirror the persisted settings (called whenever settings change).
pub fn sync(settings: &Settings) {
    let (thousands, decimal) = split_number_format(&settings.number_format);
    if let Ok(mut p) = PREFS.write() {
        *p = Some(Prefs {
            date_format: settings.date_format.clone(),
            time_format: settings.time_format.clone(),
            thousands,
            decimal,
            prefer_absolute_dates: settings.prefer_absolute_dates,
        });
    }
}

fn prefs() -> Prefs {
    PREFS
        .read()
        .ok()
        .and_then(|p| p.clone())
        .unwrap_or_else(|| Prefs {
            date_format: corvane_core::DEFAULT_DATE_FORMAT.into(),
            time_format: corvane_core::DEFAULT_TIME_FORMAT.into(),
            thousands: ",".into(),
            decimal: ".".into(),
            prefer_absolute_dates: false,
        })
}

/// `preferAbsoluteDates`
pub fn prefer_absolute_dates() -> bool {
    prefs().prefer_absolute_dates
}

/// `numberFormatFromKey`: `"<thousands>|<decimal>"`.
pub fn split_number_format(key: &str) -> (String, String) {
    match key.split_once('|') {
        Some((t, d)) => (t.to_string(), d.to_string()),
        None => (",".into(), ".".into()),
    }
}

/// GHD `dateFormats` (pattern, example for 21 Jan 2025).
pub const DATE_FORMATS: [&str; 16] = [
    "MMM d, yyyy",
    "MMMM do, yyyy",
    "MM/dd/yyyy",
    "dd/MM/yyyy",
    "dd-MM-yyyy",
    "dd.MM.yyyy",
    "yyyy/MM/dd",
    "yyyy-MM-dd",
    "yyyy.MM.dd",
    "MM/dd/yy",
    "dd/MM/yy",
    "dd-MM-yy",
    "dd.MM.yy",
    "yy/MM/dd",
    "yy-MM-dd",
    "yy.MM.dd",
];

/// GHD `timeFormats`.
pub const TIME_FORMATS: [&str; 8] = [
    "HH:mm:ss",
    "HH.mm.ss",
    "HH:mm",
    "HH.mm",
    "h:mm:ss aaa",
    "h.mm.ss aaa",
    "h:mm aaa",
    "h.mm aaa",
];

/// GHD `numberFormats` as `"<thousands>|<decimal>"` keys.
pub const NUMBER_FORMATS: [&str; 6] = ["|.", "|,", ",|.", ".|,", " |.", " |,"];

/// The preview date GHD formats the select examples with.
const PREVIEW: LocalTime = LocalTime {
    year: 2025,
    month: 1,
    day: 21,
    hour: 15,
    minute: 4,
    second: 5,
};

pub fn date_example(pattern: &str) -> String {
    format_pattern(pattern, &PREVIEW)
}

pub fn time_example(pattern: &str) -> String {
    format_pattern(pattern, &PREVIEW)
}

/// `formatNumber(1234567.89, format)` for the select.
pub fn number_example(key: &str) -> String {
    let (t, d) = split_number_format(key);
    format_number_with(1_234_567.89, &t, &d)
}

/// Broken-down local time.
#[derive(Clone, Copy, Debug)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// Local wall-clock time via `localtime_r`.
pub fn local_time(at: SystemTime) -> LocalTime {
    let secs = at
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // SAFETY: `localtime_r` fills the caller-provided `tm`; both pointers are valid.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        let t: libc::time_t = secs as libc::time_t;
        libc::localtime_r(&t, &mut tm);
        LocalTime {
            year: tm.tm_year + 1900,
            month: (tm.tm_mon + 1) as u32,
            day: tm.tm_mday as u32,
            hour: tm.tm_hour as u32,
            minute: tm.tm_min as u32,
            second: tm.tm_sec as u32,
        }
    }
}

/// Date in the user's date format.
pub fn format_date(at: SystemTime) -> String {
    format_pattern(&prefs().date_format, &local_time(at))
}

/// Date and time in the user's formats.
pub fn format_date_time(at: SystemTime) -> String {
    let p = prefs();
    let lt = local_time(at);
    format!(
        "{} {}",
        format_pattern(&p.date_format, &lt),
        format_pattern(&p.time_format, &lt)
    )
}

/// Integer with the user's thousands separator.
pub fn format_count(value: u64) -> String {
    let p = prefs();
    format_number_with(value as f64, &p.thousands, &p.decimal)
}

/// GHD `formatNumber`: plain decimal expansion with configurable separators.
pub fn format_number_with(value: f64, thousands: &str, decimal: &str) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let negative = value < 0.;
    let text = format!("{}", value.abs());
    let (int_part, dec_part) = match text.split_once('.') {
        Some((i, d)) => (i.to_string(), Some(d.to_string())),
        None => (text, None),
    };
    let mut grouped = String::new();
    let digits: Vec<char> = int_part.chars().collect();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push_str(thousands);
        }
        grouped.push(*c);
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&grouped);
    if let Some(d) = dec_part {
        out.push_str(decimal);
        out.push_str(&d);
    }
    out
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn ordinal(day: u32) -> String {
    let suffix = match (day % 10, day % 100) {
        (1, n) if n != 11 => "st",
        (2, n) if n != 12 => "nd",
        (3, n) if n != 13 => "rd",
        _ => "th",
    };
    format!("{day}{suffix}")
}

/// The subset of date-fns tokens GHD's patterns use.
pub fn format_pattern(pattern: &str, t: &LocalTime) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if !c.is_ascii_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        let mut run = 1;
        while i + run < chars.len() && chars[i + run] == c {
            run += 1;
        }
        let hour12 = if t.hour.is_multiple_of(12) {
            12
        } else {
            t.hour % 12
        };
        match (c, run) {
            ('y', 4) => out.push_str(&format!("{:04}", t.year)),
            ('y', _) => out.push_str(&format!("{:02}", t.year % 100)),
            ('M', 4) => out.push_str(MONTHS[(t.month as usize - 1).min(11)]),
            ('M', 3) => out.push_str(&MONTHS[(t.month as usize - 1).min(11)][..3]),
            ('M', 2) => out.push_str(&format!("{:02}", t.month)),
            ('M', _) => out.push_str(&t.month.to_string()),
            ('d', 2) => out.push_str(&format!("{:02}", t.day)),
            ('d', _) => {
                // `do` = ordinal day
                if i + 1 < chars.len() && chars[i + 1] == 'o' {
                    out.push_str(&ordinal(t.day));
                    i += 2;
                    continue;
                }
                out.push_str(&t.day.to_string());
            }
            ('H', 2) => out.push_str(&format!("{:02}", t.hour)),
            ('H', _) => out.push_str(&t.hour.to_string()),
            ('h', 2) => out.push_str(&format!("{hour12:02}")),
            ('h', _) => out.push_str(&hour12.to_string()),
            ('m', _) => out.push_str(&format!("{:02}", t.minute)),
            ('s', _) => out.push_str(&format!("{:02}", t.second)),
            ('a', n) => {
                let am = t.hour < 12;
                out.push_str(match n {
                    3.. => {
                        if am {
                            "am"
                        } else {
                            "pm"
                        }
                    }
                    _ => {
                        if am {
                            "AM"
                        } else {
                            "PM"
                        }
                    }
                });
            }
            _ => {
                for _ in 0..run {
                    out.push(c);
                }
            }
        }
        i += run;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_match_ghd_examples() {
        assert_eq!(date_example("MMM d, yyyy"), "Jan 21, 2025");
        assert_eq!(date_example("MMMM do, yyyy"), "January 21st, 2025");
        assert_eq!(date_example("dd.MM.yy"), "21.01.25");
        assert_eq!(time_example("h:mm aaa"), "3:04 pm");
        assert_eq!(time_example("HH:mm:ss"), "15:04:05");
    }

    #[test]
    fn numbers_group_thousands() {
        assert_eq!(number_example(",|."), "1,234,567.89");
        assert_eq!(number_example(".|,"), "1.234.567,89");
        assert_eq!(number_example("|."), "1234567.89");
        assert_eq!(format_number_with(-42.0, ",", "."), "-42");
    }
}

/// GHD `formatBytes(bytes, decimals, fixed = true)`: `1.50 KiB`.
pub fn format_bytes(bytes: i64, decimals: usize) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let magnitude = bytes.unsigned_abs() as f64;
    let unit = if magnitude < 1. {
        0
    } else {
        (magnitude.log2() / 10.).floor().min(4.) as usize
    };
    let value = bytes as f64 / 1024f64.powi(unit as i32);
    format!("{value:.decimals$} {}", UNITS[unit])
}

#[cfg(test)]
mod byte_tests {
    use super::format_bytes;

    #[test]
    fn formats_like_ghd() {
        assert_eq!(format_bytes(0, 0), "0 B");
        assert_eq!(format_bytes(1023, 0), "1023 B");
        assert_eq!(format_bytes(1536, 2), "1.50 KiB");
        assert_eq!(format_bytes(-2048, 1), "-2.0 KiB");
        assert_eq!(format_bytes(5 * 1024 * 1024, 2), "5.00 MiB");
    }
}
