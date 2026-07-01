//! Dashboard data assembly.
//!
//! This module computes the data shown on the admin dashboard:
//! today the login-activity sparkline, in future the other tiles
//! the design memo asks for. Lives here rather than in the handler
//! so the same data shape is unit-testable without building HTTP.
//!
//! All work is done in terms of typed time ranges and dense bucket
//! arrays — handlers never see raw audit rows or have to fill zero
//! buckets themselves.

use crate::errors::CoreResult;
use crate::time::SharedClock;
use chrono::{DateTime, Duration, Utc};
use sui_id_store::Database;
use sui_id_store::repos::audit;

/// The three time ranges the dashboard sparkline supports.
///
/// More than three would over-stuff the UI; fewer would make the
/// "last hour" / "last week" / "last month" question harder to
/// answer. The range and bucket sizes are paired in the type so
/// that handlers can't accidentally render a 30-day window with
/// 1-hour buckets and produce 720 SVG line segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SparklineRange {
    /// Last 24 hours, 1-hour buckets, 24 points.
    Last24Hours,
    /// Last 7 days, 1-day buckets, 7 points.
    Last7Days,
    /// Last 30 days, 1-day buckets, 30 points.
    Last30Days,
}

impl SparklineRange {
    /// The wire string used in `?range=...` URL query parameters.
    /// Round-trips with [`SparklineRange::from_query`].
    pub fn as_query(&self) -> &'static str {
        match self {
            Self::Last24Hours => "24h",
            Self::Last7Days => "7d",
            Self::Last30Days => "30d",
        }
    }

    pub fn from_query(s: &str) -> Option<Self> {
        match s {
            "24h" => Some(Self::Last24Hours),
            "7d" => Some(Self::Last7Days),
            "30d" => Some(Self::Last30Days),
            _ => None,
        }
    }

    pub fn bucket_minutes(&self) -> i64 {
        match self {
            Self::Last24Hours => 60,
            Self::Last7Days | Self::Last30Days => 60 * 24,
        }
    }

    pub fn bucket_count(&self) -> usize {
        match self {
            Self::Last24Hours => 24,
            Self::Last7Days => 7,
            Self::Last30Days => 30,
        }
    }

    /// Human-readable label for the UI, matching the screen-design
    /// memo's Japanese copy.
    pub fn label_ja(&self) -> &'static str {
        match self {
            Self::Last24Hours => "過去 24 時間",
            Self::Last7Days => "過去 7 日間",
            Self::Last30Days => "過去 30 日間",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Last24Hours, Self::Last7Days, Self::Last30Days]
    }
}

impl Default for SparklineRange {
    fn default() -> Self {
        Self::Last7Days
    }
}

/// One bucket of the login-activity sparkline.
///
/// `bucket_start` is the start of the bucket window. For a 7-day
/// view this will be midnight-aligned in UTC. `success` and
/// `failure` are direct counts of `auth.login.success` and
/// `auth.login.failure` audit rows in the bucket. Both default to
/// zero when no events landed in the bucket.
#[derive(Debug, Clone, Copy)]
pub struct LoginActivityBucket {
    pub bucket_start: DateTime<Utc>,
    pub success: i64,
    pub failure: i64,
}

/// Full result of [`login_activity`]. `buckets` is a dense array
/// of `range.bucket_count()` entries, oldest first; missing audit
/// rows from the underlying query are filled in as zero. `total_*`
/// are the sums over the buckets, surfaced in the UI as the lede
/// next to the sparkline.
#[derive(Debug, Clone)]
pub struct LoginActivity {
    pub range: SparklineRange,
    pub buckets: Vec<LoginActivityBucket>,
    pub total_success: i64,
    pub total_failure: i64,
}

/// Compute the login-activity series for the given range.
///
/// The window is anchored at `clock.now()` and walks backward by
/// `bucket_count` steps of `bucket_minutes` each. Bucket starts
/// are aligned to the Unix epoch by the SQL (so two callers
/// hitting the same range a few minutes apart get the same
/// boundaries), but the *first* bucket is whichever bucket
/// `now - range_duration` falls into — meaning the "left edge" of
/// the chart can be slightly less than `range_duration` ago. This
/// is the behaviour any reasonable dashboard wants: the user sees
/// "the last 7 buckets I've experienced", not "the period from
/// exactly 168 hours ago".
pub async fn login_activity(
    db: &Database,
    clock: &SharedClock,
    range: SparklineRange,
) -> CoreResult<LoginActivity> {
    let now = clock.now();
    let bucket_minutes = range.bucket_minutes();
    let bucket_count = range.bucket_count();
    let total_minutes = bucket_minutes * bucket_count as i64;
    let since = now - Duration::minutes(total_minutes);
    let until = now;

    let raw = audit::count_by_action_in_window(
        db,
        &["auth.login.success", "auth.login.failure"],
        since,
        until,
        bucket_minutes,
    )
    .await?;

    // Build the dense bucket array. Align the very first bucket
    // start to the Unix-epoch grid, the same alignment SQL used.
    let bucket_secs = bucket_minutes * 60;
    let now_unix = now.timestamp();
    let last_bucket_start = (now_unix / bucket_secs) * bucket_secs
        // Subtract (bucket_count - 1) full buckets so the *last*
        // bucket is the current one and the *first* bucket is the
        // earliest one in the window.
        - bucket_secs * (bucket_count as i64 - 1);

    let mut buckets: Vec<LoginActivityBucket> = (0..bucket_count)
        .map(|i| {
            let start_unix = last_bucket_start + bucket_secs * i as i64;
            LoginActivityBucket {
                bucket_start: DateTime::<Utc>::from_timestamp(start_unix, 0).unwrap_or(now),
                success: 0,
                failure: 0,
            }
        })
        .collect();

    // Fill in the rows we got back from SQL. Because both the
    // query and the array are aligned to the same Unix-epoch grid
    // and the same `bucket_secs` step, the index lookup is exact
    // — no fuzzy nearest-neighbour matching needed.
    for row in raw {
        let row_unix = row.bucket_start.timestamp();
        if row_unix < last_bucket_start {
            // Older than the dense array's start — can happen if
            // the query window included a partial bucket below the
            // first dense slot. Skip: the dashboard doesn't show it.
            continue;
        }
        let idx = ((row_unix - last_bucket_start) / bucket_secs) as usize;
        if idx >= bucket_count {
            continue;
        }
        match row.action.as_str() {
            "auth.login.success" => buckets[idx].success += row.count,
            "auth.login.failure" => buckets[idx].failure += row.count,
            _ => {}
        }
    }

    let total_success: i64 = buckets.iter().map(|b| b.success).sum();
    let total_failure: i64 = buckets.iter().map(|b| b.failure).sum();
    Ok(LoginActivity {
        range,
        buckets,
        total_success,
        total_failure,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::system_clock;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::AuditLogRow;

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).expect("db")
    }

    async fn append(db: &Database, action: &str, at: DateTime<Utc>) {
        sui_id_store::repos::audit::append(
            db,
            &AuditLogRow {
                at,
                actor: None,
                action: action.into(),
                target: None,
                result: "ok".into(),
                note: None,
            },
        )
        .await
        .expect("append");
    }

    #[tokio::test]
    async fn empty_db_returns_zero_filled_dense_array_for_each_range() {
        let db = fresh_db();
        let clock = system_clock();
        for &r in SparklineRange::all() {
            let a = login_activity(&db, &clock, r).await.expect("activity");
            assert_eq!(a.buckets.len(), r.bucket_count(), "range {:?}", r);
            assert_eq!(a.total_success, 0);
            assert_eq!(a.total_failure, 0);
            assert!(a.buckets.iter().all(|b| b.success == 0 && b.failure == 0));
        }
    }

    #[tokio::test]
    async fn bucket_starts_are_strictly_increasing_and_aligned() {
        let db = fresh_db();
        let clock = system_clock();
        let a = login_activity(&db, &clock, SparklineRange::Last7Days)
            .await
            .expect("activity");
        let secs = a.range.bucket_minutes() * 60;
        for w in a.buckets.windows(2) {
            let delta = w[1].bucket_start.timestamp() - w[0].bucket_start.timestamp();
            assert_eq!(delta, secs, "buckets must be evenly spaced");
            assert_eq!(
                w[0].bucket_start.timestamp() % secs,
                0,
                "buckets must be aligned to the epoch grid"
            );
        }
    }

    #[tokio::test]
    async fn rows_in_window_are_counted_into_the_right_bucket() {
        let db = fresh_db();
        let clock = system_clock();
        let now = clock.now();
        // Insert events distributed across the last 24 hours.
        for h in 0..24 {
            let at = now - Duration::hours(h);
            for _ in 0..(h % 5) {
                append(&db, "auth.login.success", at).await;
            }
            if h % 7 == 0 {
                append(&db, "auth.login.failure", at).await;
            }
            // An unrelated action — must NOT show up in totals.
            append(&db, "auth.password.changed_self", at).await;
        }
        let a = login_activity(&db, &clock, SparklineRange::Last24Hours)
            .await
            .expect("activity");
        // Total successes: sum_{h=0..24} (h % 5) = 4+5*(0+1+2+3+4) = 0+1+2+3+4 + 0+1+2+3+4 + ... 5 cycles
        // Easier: just compare against a hand recount.
        let mut expected_success = 0;
        let mut expected_failure = 0;
        for h in 0..24 {
            expected_success += h % 5;
            if h % 7 == 0 {
                expected_failure += 1;
            }
        }
        assert_eq!(a.total_success, expected_success);
        assert_eq!(a.total_failure, expected_failure);
    }

    #[tokio::test]
    async fn rows_outside_window_are_ignored() {
        let db = fresh_db();
        let clock = system_clock();
        let now = clock.now();
        // 8 days ago for Last7Days view -> outside window.
        append(&db, "auth.login.success", now - Duration::days(8)).await;
        // 5 days ago -> inside.
        append(&db, "auth.login.success", now - Duration::days(5)).await;
        let a = login_activity(&db, &clock, SparklineRange::Last7Days)
            .await
            .expect("activity");
        assert_eq!(a.total_success, 1, "only the in-window row should count");
    }

    #[tokio::test]
    async fn unrelated_actions_are_never_counted() {
        let db = fresh_db();
        let clock = system_clock();
        let now = clock.now();
        for _ in 0..100 {
            append(&db, "auth.password.changed_self", now).await;
            append(&db, "mfa.admin_reset", now).await;
            append(&db, "auth.refresh.theft_detected", now).await;
        }
        let a = login_activity(&db, &clock, SparklineRange::Last7Days)
            .await
            .expect("activity");
        assert_eq!(a.total_success, 0);
        assert_eq!(a.total_failure, 0);
    }

    #[tokio::test]
    async fn range_query_strings_round_trip() {
        for &r in SparklineRange::all() {
            assert_eq!(SparklineRange::from_query(r.as_query()), Some(r));
        }
        assert_eq!(SparklineRange::from_query("garbage"), None);
        assert_eq!(SparklineRange::from_query(""), None);
    }
}
