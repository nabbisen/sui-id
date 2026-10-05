use super::*;
use chrono::{TimeZone, Utc};

fn ts(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
}

#[test]
fn en_date_formatting() {
    let dt = ts(2024, 5, 12, 14, 7);
    assert_eq!(en_fmt_date(dt), "12 May 2024");
    assert_eq!(en_fmt_date_time(dt), "12 May 2024 14:07");
}

#[test]
fn en_relative_formatting() {
    let now = ts(2024, 5, 12, 15, 0);
    assert_eq!(
        en_fmt_relative(ts(2024, 5, 12, 14, 59), now),
        "1 minute ago"
    );
    assert_eq!(
        en_fmt_relative(ts(2024, 5, 12, 14, 57), now),
        "3 minutes ago"
    );
    assert_eq!(en_fmt_relative(ts(2024, 5, 12, 12, 0), now), "3 hours ago");
    assert_eq!(en_fmt_relative(ts(2024, 5, 9, 15, 0), now), "3 days ago");
}
