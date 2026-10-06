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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SparklineRange {
    /// Last 24 hours, 1-hour buckets, 24 points.
    Last24Hours,
    /// Last 7 days, 1-day buckets, 7 points.
    #[default]
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
mod tests;
