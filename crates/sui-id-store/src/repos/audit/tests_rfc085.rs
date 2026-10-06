//! RFC 085: audit atomicity tests.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use crate::{Database, crypto::MasterKey, models::AuditLogRow};
use chrono::Utc;

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

fn sample_row(action: &str) -> AuditLogRow {
    AuditLogRow {
        at: Utc::now(),
        actor: None,
        action: action.into(),
        target: None,
        result: "ok".into(),
        note: None,
    }
}

/// append_within_tx inside a committed transaction produces a row that
/// passes chain verification (RFC 085 P2 — committed path).
#[tokio::test]
async fn append_within_tx_commits_with_caller_transaction() {
    let db = fresh_db();
    let row = sample_row("admin.test_action");
    db.with_tx(move |tx| super::append_within_tx(tx, &row))
        .await
        .expect("append_within_tx");
    let report = super::verify_chain_tail(&db, 10).await.expect("verify");
    assert_eq!(report.checked, 1, "row must be in chain");
    assert!(report.broken_at_seq.is_none(), "chain must be unbroken");
}

/// If the caller's transaction rolls back, the audit row is NOT written —
/// atomic either-both-or-neither (RFC 085 P2 — rollback path).
#[tokio::test]
async fn append_within_tx_rolls_back_with_caller_transaction() {
    let db = fresh_db();
    let row = sample_row("admin.should_not_appear");
    let result = db
        .with_tx(move |tx| {
            super::append_within_tx(tx, &row)?;
            Err::<(), _>(crate::StoreError::Conflict) // force rollback
        })
        .await;
    assert!(result.is_err(), "with_tx must propagate the error");
    let recent = super::recent(&db, 10).await.expect("recent");
    assert!(
        recent.is_empty(),
        "rolled-back audit row must not appear in chain"
    );
}

/// append_within_tx rows integrate seamlessly with rows written via the
/// ordinary async append — chain integrity is preserved (RFC 085 P5).
#[tokio::test]
async fn append_within_tx_maintains_chain_with_prior_rows() {
    let db = fresh_db();
    super::append(&db, &sample_row("act.before"))
        .await
        .expect("append 1");
    super::append(&db, &sample_row("act.before2"))
        .await
        .expect("append 2");
    db.with_tx(move |tx| super::append_within_tx(tx, &sample_row("act.within")))
        .await
        .expect("within_tx");
    super::append(&db, &sample_row("act.after"))
        .await
        .expect("append 3");
    let report = super::verify_chain_tail(&db, 20).await.expect("verify");
    assert_eq!(report.checked, 4, "all four rows must be in chain");
    assert!(report.broken_at_seq.is_none(), "chain must be unbroken");
}
