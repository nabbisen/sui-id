use super::*;
use chrono::{TimeZone, Utc};

fn ts(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
}

#[test]
fn zh_hans_date_formatting() {
    let dt = ts(2024, 5, 12, 14, 7);
    assert_eq!(zh_hans_fmt_date(dt), "2024年5月12日");
    assert_eq!(zh_hans_fmt_date_time(dt), "2024年5月12日 14:07");
}

#[test]
fn zh_hans_relative_formatting() {
    let now = ts(2024, 5, 12, 15, 0);
    assert_eq!(
        zh_hans_fmt_relative(ts(2024, 5, 12, 14, 57), now),
        "3 分钟前"
    );
    assert_eq!(
        zh_hans_fmt_relative(ts(2024, 5, 12, 12, 0), now),
        "3 小时前"
    );
    assert_eq!(zh_hans_fmt_relative(ts(2024, 5, 9, 15, 0), now), "3 天前");
}
