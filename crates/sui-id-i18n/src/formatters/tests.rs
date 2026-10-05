use super::*;
use chrono::{TimeZone, Utc};

fn ts(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
}

#[test]
fn fmt_time_zero_pads() {
    assert_eq!(fmt_time_shared(ts(2024, 1, 1, 9, 5)), "09:05");
    assert_eq!(fmt_time_shared(ts(2024, 1, 1, 0, 0)), "00:00");
}

#[test]
fn fmt_count_thousands() {
    assert_eq!(fmt_count_shared(0), "0");
    assert_eq!(fmt_count_shared(999), "999");
    assert_eq!(fmt_count_shared(1000), "1,000");
    assert_eq!(fmt_count_shared(1_234_567), "1,234,567");
}
