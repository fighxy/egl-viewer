//! Display formatting for the details panel (ported from `ui/src/features/browse/format.ts`).

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

/// Medium date + short time in the en locale, or an em dash.
pub fn format_date(ms: Option<i64>) -> String {
    let Some(ms) = ms else {
        return "—".into();
    };
    // chrono is not a workspace dep; format a stable UTC-ish local string from epoch ms.
    let secs = ms / 1000;
    let datetime = chrono_lite(secs);
    datetime.unwrap_or_else(|| "—".into())
}

/// Minimal UTC formatting without pulling chrono: YYYY-MM-DD HH:MM.
fn chrono_lite(secs: i64) -> Option<String> {
    let days = secs.div_euclid(86_400);
    let day_secs = secs.rem_euclid(86_400) as u32;
    let (y, m, d) = civil_from_days(days)?;
    let hh = day_secs / 3600;
    let mm = (day_secs % 3600) / 60;
    Some(format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}"))
}

/// Howard Hinnant civil_from_days (UTC).
fn civil_from_days(z: i64) -> Option<(i32, u32, u32)> {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Some((y as i32, m, d))
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
    fn dates_round_trip_epoch() {
        assert_eq!(format_date(None), "—");
        let s = format_date(Some(0));
        assert!(s.starts_with("1970-01-01"), "{s}");
    }
}
