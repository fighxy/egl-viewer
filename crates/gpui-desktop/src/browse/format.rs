//! Display formatting for the details panel (ported from `ui/src/features/browse/format.ts`).

use chrono::TimeZone as _;

/// Human-readable file size, or an em dash when unknown.
pub fn format_bytes(bytes: Option<u64>) -> String {
    let Some(bytes) = bytes else {
        return "—".into();
    };
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let units = ["KB", "MB", "GB", "TB"];
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", units[unit])
    } else {
        format!("{} {}", value.round() as u64, units[unit])
    }
}

/// `width × height`, or an em dash when either side is missing.
pub fn format_dimensions(width: Option<u32>, height: Option<u32>) -> String {
    match (width, height) {
        (Some(w), Some(h)) => format!("{w} × {h}"),
        _ => "—".into(),
    }
}

/// `YYYY-MM-DD HH:MM` in the computer's time zone (D-174), or an em dash. Stored times are UTC
/// epoch milliseconds; the zone's rule for that date applies, daylight saving included.
pub fn format_date(ms: Option<i64>) -> String {
    let Some(ms) = ms else {
        return "—".into();
    };
    match chrono::Local.timestamp_millis_opt(ms) {
        chrono::LocalResult::Single(time) | chrono::LocalResult::Ambiguous(time, _) => {
            time.format(DATE_FORMAT).to_string()
        }
        chrono::LocalResult::None => "—".into(),
    }
}

const DATE_FORMAT: &str = "%Y-%m-%d %H:%M";

/// [`format_date`] in a fixed zone, for tests that must not depend on the machine's zone.
#[cfg(test)]
fn format_date_at(ms: i64, offset: chrono::FixedOffset) -> String {
    offset
        .timestamp_millis_opt(ms)
        .single()
        .map_or_else(|| "—".into(), |time| time.format(DATE_FORMAT).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_and_dimensions() {
        assert_eq!(format_bytes(None), "—");
        assert_eq!(format_bytes(Some(500)), "500 B");
        assert_eq!(format_bytes(Some(1536)), "1.5 KB");
        assert_eq!(format_dimensions(Some(100), Some(50)), "100 × 50");
        assert_eq!(format_dimensions(None, Some(50)), "—");
    }

    #[test]
    fn dates_show_in_the_local_zone() {
        assert_eq!(format_date(None), "—");
        // 2026-11-14 03:09:27 UTC, opened at 10:09 in Krasnoyarsk (UTC+7).
        let ms = 1_794_625_767_741;
        let utc = chrono::FixedOffset::east_opt(0).unwrap();
        let krasnoyarsk = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
        let new_york = chrono::FixedOffset::west_opt(5 * 3600).unwrap();
        assert_eq!(format_date_at(ms, utc), "2026-11-14 03:09");
        assert_eq!(format_date_at(ms, krasnoyarsk), "2026-11-14 10:09");
        assert_eq!(format_date_at(ms, new_york), "2026-11-13 22:09");
        // The app's own call uses the machine's zone: the same instant, its local wall time.
        let local = chrono::Local
            .timestamp_millis_opt(ms)
            .single()
            .unwrap()
            .format(DATE_FORMAT)
            .to_string();
        assert_eq!(format_date(Some(ms)), local);
    }
}
