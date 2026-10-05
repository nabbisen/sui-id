//! Locale-aware date, time, and count formatters (RFC 002 § B).
//!
//! This module defines the [`Formatters`] struct and two shared helper
//! functions used by multiple locale implementations. The per-locale
//! constants (`FORMATTERS_EN`, `FORMATTERS_JA`, `FORMATTERS_ZH_HANS`, …)
//! live in the corresponding file under `locale/`.
//!
//! ## Design
//!
//! Functions are plain `fn` pointers rather than closures so the
//! struct can be `&'static`. No ICU dependency — all patterns are
//! hand-written to keep the binary lean and the logic auditable.
//!
//! ## Timestamp rendering policy (from RFC 017 § 4)
//!
//! - **Admin UI** (audit log, session list): absolute timestamps only.
//!   Operators need exact times; relative timestamps are ambiguous
//!   across time zones.
//! - **End-user UI** (`/me/security` "last used"): relative timestamps
//!   are acceptable and preferred for readability.
//!
//! Both rendering modes are available; the view layer chooses.

use chrono::{DateTime, Timelike, Utc};

/// Locale-aware formatting functions for dates, times, and counts.
///
/// Obtain via [`crate::Locale::formatters`].
pub struct Formatters {
    /// Date only: e.g. "2024年5月12日" or "12 May 2024".
    pub fmt_date: fn(DateTime<Utc>) -> String,
    /// Time only (24 h): e.g. "14:07".
    pub fmt_time: fn(DateTime<Utc>) -> String,
    /// Date + time: e.g. "2024年5月12日 14:07" or "12 May 2024 14:07".
    pub fmt_date_time: fn(DateTime<Utc>) -> String,
    /// Relative time from `now`: e.g. "3 時間前" or "3 hours ago".
    /// `now` is passed in so callers can use a mock clock in tests.
    pub fmt_relative: fn(at: DateTime<Utc>, now: DateTime<Utc>) -> String,
    /// Locale-appropriate number with thousands separator: e.g. "1,234".
    pub fmt_count: fn(u64) -> String,
}

// ── Shared helpers used by locale/ files ─────────────────────────────────────

/// Shared 24-hour time formatter: "HH:MM".
pub(crate) fn fmt_time_shared(dt: DateTime<Utc>) -> String {
    format!("{:02}:{:02}", dt.hour(), dt.minute())
}

/// Shared thousands-separator count formatter: 1,234,567.
pub(crate) fn fmt_count_shared(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        let remaining = s.len() - i;
        if i > 0 && remaining.is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
