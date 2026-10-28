//! Milliseconds since the Unix epoch and UTC dates, without a date library.

use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn now_ms() -> i64 {
    system_ms(SystemTime::now()).unwrap_or(0)
}

pub(crate) fn system_ms(time: SystemTime) -> Option<i64> {
    let since = time.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(since.as_millis()).ok()
}

/// `YYYY-MM-DD` of a timestamp in UTC.
pub(crate) fn utc_date(ms: i64) -> String {
    let (year, month, day) = civil_from_days(ms.div_euclid(86_400_000));
    format!("{year:04}-{month:02}-{day:02}")
}

/// `YYYY-MM-DD HH-MM-SS` in UTC, safe in a file name.
pub fn utc_file_stamp(ms: i64) -> String {
    let seconds = ms.div_euclid(1000).rem_euclid(86_400);
    format!(
        "{} {:02}-{:02}-{:02}",
        utc_date(ms),
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 to a proleptic Gregorian date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_dates() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(951_782_400_000), "2000-02-29");
        assert_eq!(utc_date(1_793_145_600_000 - 1), "2026-10-27");
        assert_eq!(utc_date(1_793_145_600_000), "2026-10-28");
        assert_eq!(utc_date(-1), "1969-12-31");
        assert_eq!(
            utc_file_stamp(1_793_145_600_000 + 37_325_000),
            "2026-10-28 10-22-05"
        );
    }
}
