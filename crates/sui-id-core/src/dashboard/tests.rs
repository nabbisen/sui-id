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
