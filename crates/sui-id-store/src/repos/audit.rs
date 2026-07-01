//! Append-only audit log with a tamper-evident hash chain.
//!
//! Each row's `hash` is `SHA-256(prev_hash || canonical_row_bytes)`.
//! The `prev_hash` of row N+1 is the `hash` of row N. To rewrite or
//! delete row N you must recompute every subsequent row's hash —
//! something an attacker with raw SQL access can do, but they
//! cannot do it without leaving any trace because there's no point
//! at which the legitimate code path produces a chain that doesn't
//! match the canonical recipe. A startup-time verifier walking the
//! tail of the log catches the mismatch.
//!
//! The hashes are not signed by any external party — that's an
//! orthogonal extension (RFC 3161 timestamping or a notary service)
//! that we'll add when there's a concrete operator need. Local
//! detection is enough for "DB-only access" attackers, which is by
//! far the more common attack model for a self-hosted IdP.
//!
//! By design this module exposes only `append` and read operations:
//! the codebase never updates or deletes audit rows.

use crate::db::Database;
use crate::errors::StoreResult;
use crate::models::AuditLogRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sha2::{Digest, Sha256};

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditLogRow> {
    Ok(AuditLogRow {
        at: row.get::<_, DateTime<Utc>>(0)?,
        actor: row
            .get::<_, Option<String>>(1)?
            .map(|s| s.parse())
            .transpose()
            .map_err(|e: uuid::Error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        action: row.get(2)?,
        target: row.get(3)?,
        result: row.get(4)?,
        note: row.get(5)?,
    })
}

/// Canonical byte serialisation of a row for hashing. Length-
/// prefixed UTF-8 fields so that two rows that "happen" to share a
/// concatenated representation can't collide. The format is opaque
/// to the rest of the world; if we ever change it we bump
/// migration version and document the break in the verifier.
fn canonical_bytes(row: &AuditLogRow) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);
    write_field(&mut buf, row.at.to_rfc3339().as_bytes());
    write_field(
        &mut buf,
        row.actor
            .map(|u| u.to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    write_field(&mut buf, row.action.as_bytes());
    write_field(&mut buf, row.target.as_deref().unwrap_or("").as_bytes());
    write_field(&mut buf, row.result.as_bytes());
    write_field(&mut buf, row.note.as_deref().unwrap_or("").as_bytes());
    buf
}

fn write_field(buf: &mut Vec<u8>, field: &[u8]) {
    let len = field.len() as u64;
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(field);
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// SHA-256 of `prev_hash_hex || canonical_bytes(row)`. The
/// `prev_hash` is hashed as raw hex bytes — the chain head (very
/// first row) uses an empty `prev_hash`, so its hash is just
/// `SHA-256("" || canonical_bytes(row))`.
fn compute_hash(prev_hash_hex: &str, row: &AuditLogRow) -> String {
    let mut h = Sha256::new();
    h.update(prev_hash_hex.as_bytes());
    h.update(canonical_bytes(row));
    hex_lower(h.finalize().as_slice())
}

pub async fn append(db: &Database, row: &AuditLogRow) -> StoreResult<()> {
    // RFC 006: increment the audit counter unconditionally (no-op when metrics
    // are disabled — global_metrics() returns None).
    if let Some(m) = crate::global_metrics() {
        m.audit_appended();
    }
    let row = row.clone();
    db.with_conn(move |conn| {
        let tx = conn.unchecked_transaction()?;
        // Read the latest hash inside the transaction so concurrent
        // appends serialise into a single chain.
        let prev_hash: String = tx
            .query_row(
                "SELECT COALESCE((SELECT hash FROM audit_log ORDER BY seq DESC LIMIT 1), '')",
                [],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let hash = compute_hash(&prev_hash, &row);
        tx.execute(
            "INSERT INTO audit_log(at, actor, action, target, result, note, prev_hash, hash) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.at,
                row.actor.map(|u| u.to_string()),
                row.action,
                row.target,
                row.result,
                row.note,
                prev_hash,
                hash,
            ],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await
}

/// Append an audit row *within an existing transaction* (RFC 085 Class-A atomicity).
///
/// Unlike [`append`], which opens its own transaction, this function runs inside
/// the caller's transaction so that the state change and the audit record commit
/// atomically — or neither does. Use this for Class-A operations (user
/// disable/delete/role-change, client mutations, signing-key rollover, etc.) where
/// a committed state change without an audit record is a correctness defect.
///
/// The hash-chain invariant is maintained: this function reads the current chain
/// head and writes the new row with the computed hash inside the same transaction.
/// Because `Database` serialises all writes behind a single mutex, this is
/// race-free.
///
/// # Atomicity guarantee
///
/// If the caller's transaction rolls back for any reason, neither the state change
/// nor the audit row reaches the database.  This is the intended fail-safe: an
/// audit subsystem failure becomes an operation failure, not a silent gap (RFC 085
/// §Security P2).
pub fn append_within_tx(tx: &rusqlite::Transaction<'_>, row: &AuditLogRow) -> StoreResult<()> {
    let prev_hash: String = tx
        .query_row(
            "SELECT COALESCE((SELECT hash FROM audit_log ORDER BY seq DESC LIMIT 1), '')",
            [],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let hash = compute_hash(&prev_hash, row);
    tx.execute(
        "INSERT INTO audit_log(at, actor, action, target, result, note, prev_hash, hash)          VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            row.at,
            row.actor.map(|u| u.to_string()),
            row.action,
            row.target,
            row.result,
            row.note,
            prev_hash,
            hash,
        ],
    )?;
    Ok(())
}

pub async fn recent(db: &Database, limit: i64) -> StoreResult<Vec<AuditLogRow>> {
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT at, actor, action, target, result, note FROM audit_log ORDER BY seq DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }).await
}

/// Fetch recent audit rows, optionally filtered by event-name prefix.
///
/// `filter` is matched as `action LIKE '<filter>%'`. An empty or None
/// filter is equivalent to calling [`recent`].
pub async fn recent_filtered(
    db: &Database,
    limit: i64,
    filter: Option<String>,
) -> StoreResult<Vec<AuditLogRow>> {
    db.with_conn(move |conn| {
        match filter.as_deref().filter(|s| !s.is_empty()) {
            None => {
                let mut stmt = conn.prepare(
                    "SELECT at, actor, action, target, result, note                      FROM audit_log ORDER BY seq DESC LIMIT ?1",
                )?;
                let rows = stmt.query_map([limit], map)?.collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            }
            Some(prefix) => {
                let pattern = format!("{prefix}%");
                let mut stmt = conn.prepare(
                    "SELECT at, actor, action, target, result, note                      FROM audit_log WHERE action LIKE ?2 ORDER BY seq DESC LIMIT ?1",
                )?;
                let rows = stmt
                    .query_map(rusqlite::params![limit, pattern], map)?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            }
        }
    }).await
}
/// Result of a tail-verification pass.
#[derive(Debug, Clone)]
pub struct ChainVerifyReport {
    /// Rows examined that participate in the chain.
    pub checked: usize,
    /// Sequence number of the first row whose stored hash disagrees
    /// with the recomputation. `None` means every checked row
    /// hashes correctly.
    pub broken_at_seq: Option<i64>,
    /// Rows that were skipped because they predate the v0.17.0
    /// migration (their `hash` column is empty). These don't
    /// indicate tampering — they were never hashed in the first
    /// place. Reported for transparency.
    pub legacy_unhashed: usize,
}

/// Walk the most-recent `limit` audit rows and verify each row's
/// hash matches `SHA-256(prev_hash || canonical_bytes)`. Stops at
/// the first mismatch and reports the offending row's `seq`.
///
/// Intended to be called once at startup with a cheap limit (a few
/// thousand rows): more than enough to catch a recent tampering
/// attempt, cheap enough to not noticeably extend boot time.
pub async fn verify_chain_tail(db: &Database, limit: i64) -> StoreResult<ChainVerifyReport> {
    let rows: Vec<(i64, AuditLogRow, String, String)> = db
        .with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT seq, at, actor, action, target, result, note, prev_hash, hash \
             FROM audit_log ORDER BY seq DESC LIMIT ?1",
            )?;
            let collected = stmt
                .query_map([limit], |r| {
                    let seq: i64 = r.get(0)?;
                    let row = AuditLogRow {
                        at: r.get(1)?,
                        actor: r
                            .get::<_, Option<String>>(2)?
                            .map(|s| s.parse())
                            .transpose()
                            .map_err(|e: uuid::Error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    2,
                                    rusqlite::types::Type::Text,
                                    Box::new(e),
                                )
                            })?,
                        action: r.get(3)?,
                        target: r.get(4)?,
                        result: r.get(5)?,
                        note: r.get(6)?,
                    };
                    let prev: String = r.get(7)?;
                    let hash: String = r.get(8)?;
                    Ok((seq, row, prev, hash))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(collected)
        })
        .await?;

    let mut report = ChainVerifyReport {
        checked: 0,
        broken_at_seq: None,
        legacy_unhashed: 0,
    };
    for (seq, row, prev, hash) in &rows {
        if hash.is_empty() {
            // Pre-v0.17.0 row: not part of the chain.
            report.legacy_unhashed += 1;
            continue;
        }
        report.checked += 1;
        let computed = compute_hash(prev, row);
        if computed != *hash {
            report.broken_at_seq = Some(*seq);
            return Ok(report);
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::crypto::MasterKey;
    use sui_id_shared::ids::UserId;

    fn fresh_db() -> Database {
        let key = MasterKey::generate();
        Database::open_in_memory(key).expect("db")
    }

    fn sample_row(action: &str) -> AuditLogRow {
        AuditLogRow {
            at: Utc::now(),
            actor: Some(UserId::new()),
            action: action.into(),
            target: Some("target-x".into()),
            result: "ok".into(),
            note: None,
        }
    }

    #[tokio::test]
    async fn appended_rows_form_a_consistent_chain() {
        let db = fresh_db();
        for i in 0..5 {
            append(&db, &sample_row(&format!("act.{i}")))
                .await
                .expect("append");
        }
        let r = verify_chain_tail(&db, 100).await.expect("verify");
        assert_eq!(r.checked, 5);
        assert_eq!(r.broken_at_seq, None);
        assert_eq!(r.legacy_unhashed, 0);
    }

    #[tokio::test]
    async fn first_row_chains_from_empty_prev_hash() {
        let db = fresh_db();
        append(&db, &sample_row("first")).await.expect("append");
        let (prev, hash): (String, String) = db
            .with_conn(|c| {
                let (p, h): (String, String) = c.query_row(
                    "SELECT prev_hash, hash FROM audit_log ORDER BY seq ASC LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                Ok((p, h))
            })
            .await
            .unwrap();
        assert_eq!(prev, "", "first row's prev_hash must be empty");
        assert_eq!(hash.len(), 64, "hash must be 64 hex chars (SHA-256)");
    }

    #[tokio::test]
    async fn tampering_with_a_row_makes_chain_verification_fail() {
        let db = fresh_db();
        append(&db, &sample_row("a")).await.expect("append");
        append(&db, &sample_row("b")).await.expect("append");
        append(&db, &sample_row("c")).await.expect("append");

        // Rewrite row #2's `action` directly via SQL — exactly the
        // attack the chain is supposed to detect.
        db.with_conn(|c| {
            c.execute("UPDATE audit_log SET action = 'tampered' WHERE seq = 2", [])?;
            Ok(())
        })
        .await
        .expect("tamper");

        let r = verify_chain_tail(&db, 100).await.expect("verify");
        // Walking newest-first, seq=3 still hashes correctly because
        // its prev_hash was committed before the tamper. seq=2 is
        // the row whose recomputed hash now disagrees with stored.
        assert_eq!(r.broken_at_seq, Some(2), "{r:?}");
    }

    #[tokio::test]
    async fn legacy_unhashed_rows_are_reported_separately() {
        let db = fresh_db();
        let now = Utc::now();
        db.with_conn(move |c| {
            c.execute(
                "INSERT INTO audit_log(at, actor, action, target, result, note, prev_hash, hash) \
                 VALUES(?1, NULL, 'legacy', NULL, 'ok', NULL, '', '')",
                [now],
            )?;
            Ok(())
        })
        .await
        .unwrap();
        append(&db, &sample_row("post-upgrade"))
            .await
            .expect("append");

        let r = verify_chain_tail(&db, 100).await.expect("verify");
        assert_eq!(r.checked, 1);
        assert_eq!(r.legacy_unhashed, 1);
        assert_eq!(r.broken_at_seq, None);
    }

    #[tokio::test]
    async fn canonical_bytes_distinguishes_field_boundaries() {
        // Length-prefix protects against a row {"a", "bc"} hashing
        // the same as a row {"ab", "c"}.
        let at = Utc::now();
        let r1 = AuditLogRow {
            at,
            actor: None,
            action: "a".into(),
            target: Some("bc".into()),
            result: "ok".into(),
            note: None,
        };
        let r2 = AuditLogRow {
            at,
            actor: None,
            action: "ab".into(),
            target: Some("c".into()),
            result: "ok".into(),
            note: None,
        };
        assert_ne!(canonical_bytes(&r1), canonical_bytes(&r2));
    }
}

/// Most recent audit rows where the given user is either the actor
/// or the target. Newest-first. Used by `/me/security` to surface a
/// user-scoped activity timeline without exposing other users' rows.
///
/// `target` matches by string equality on the `target` column —
/// most events that concern a user record the user's UUID there
/// (lockout, MFA reset, theft detection, …). `actor` matches the
/// `actor` UUID column. The OR of the two captures both
/// "things this user did" and "things done to this user".
pub async fn recent_for_user(
    db: &Database,
    user_id: sui_id_shared::ids::UserId,
    limit: i64,
) -> StoreResult<Vec<AuditLogRow>> {
    let uid = user_id.to_string();
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT at, actor, action, target, result, note FROM audit_log \
             WHERE actor = ?1 OR target = ?1 \
             ORDER BY seq DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![uid, limit], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// One bucket of a counted audit-action time series.
///
/// `bucket_start` is the inclusive start of the bucket window (in
/// UTC). `action` is the audit action name (e.g. `auth.login.success`).
/// `count` is how many rows in `audit_log` matched the action and
/// fell inside the bucket. Buckets with zero hits are *not* returned
/// — callers fill those in client-side, since an empty SELECT row
/// from SQLite is usually cheaper to synthesise than to LEFT JOIN
/// against a generated calendar.
#[derive(Debug, Clone)]
pub struct ActionCountBucket {
    pub bucket_start: chrono::DateTime<chrono::Utc>,
    pub action: String,
    pub count: i64,
}

/// Count audit-log rows matching any of the given `actions`,
/// occurring in `[since, until)`, grouped into time buckets of
/// `bucket_minutes` minutes.
///
/// Used by the dashboard sparkline: caller passes
/// `["auth.login.success", "auth.login.failure"]` and a 7-day window
/// in 24*60-minute buckets, gets back up to `7 * 2 = 14` rows,
/// fills the missing combinations with zeros, and feeds the result
/// into an SVG renderer.
///
/// SQLite alignment: buckets are aligned to the Unix epoch — for any
/// fixed `bucket_minutes`, two queries with different `since` values
/// will produce buckets at the same absolute time boundaries, so the
/// dashboard's 7d view shows the same per-day points whether you
/// open it at 09:00 or 17:00. The `at` column in `audit_log` is
/// stored as ISO-8601 text (chrono's default), so the alignment uses
/// `unixepoch()` to convert to a numeric.
///
/// Performance: with the v0.20.2 composite index on
/// `audit_log (at, action)`, this query is a range scan over the
/// `at` window with an `IN (...)` filter on `action`, and a final
/// GROUP BY on the bucket expression. For a busy IdP with millions
/// of audit rows the query is bounded by the *width of the window*,
/// not the size of the table.
pub async fn count_by_action_in_window(
    db: &Database,
    actions: &[&str],
    since: chrono::DateTime<chrono::Utc>,
    until: chrono::DateTime<chrono::Utc>,
    bucket_minutes: i64,
) -> StoreResult<Vec<ActionCountBucket>> {
    if actions.is_empty() || bucket_minutes <= 0 || until <= since {
        return Ok(Vec::new());
    }
    let bucket_seconds = bucket_minutes * 60;
    // Build a parameter placeholder list of the right shape:
    // ?3, ?4, ?5, … one per action. Indices ?1 / ?2 are reserved
    // for since / until.
    let action_placeholders: Vec<String> =
        (0..actions.len()).map(|i| format!("?{}", i + 3)).collect();
    let action_list = action_placeholders.join(", ");
    let sql = format!(
        "SELECT
             (CAST(unixepoch(at) AS INTEGER) / {bucket_seconds}) * {bucket_seconds} AS bucket_unix,
             action,
             COUNT(*) AS n
         FROM audit_log
         WHERE at >= ?1 AND at < ?2
           AND action IN ({action_list})
         GROUP BY bucket_unix, action
         ORDER BY bucket_unix ASC, action ASC"
    );
    let actions: Vec<String> = actions.iter().map(|s| s.to_string()).collect();
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&sql)?;
        // rusqlite's params! macro doesn't take a slice directly;
        // we build a Vec<&dyn ToSql> by hand.
        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(2 + actions.len());
        params.push(&since);
        params.push(&until);
        for a in &actions {
            params.push(a as &dyn rusqlite::ToSql);
        }
        let rows = stmt
            .query_map(params.as_slice(), |row| {
                let bucket_unix: i64 = row.get(0)?;
                let action: String = row.get(1)?;
                let count: i64 = row.get(2)?;
                let bucket_start = chrono::DateTime::<chrono::Utc>::from_timestamp(bucket_unix, 0)
                    .ok_or_else(|| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Integer,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "bucket_unix out of range",
                            )),
                        )
                    })?;
                Ok(ActionCountBucket {
                    bucket_start,
                    action,
                    count,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// Action prefix patterns that are surfaced on the admin dashboard
/// as "recent important events" (RFC 043).
pub const DASHBOARD_IMPORTANT_PREFIXES: &[&str] = &[
    "user.create",
    "user.disable",
    "user.delete",
    "user.reset_password",
    "user.reset_mfa",
    "client.create",
    "client.delete",
    "client.rotate_secret",
    "signing_key.rotate",
    "signing_key.delete",
    "auth.lockout",
    "auth.refresh.theft_detected",
    "admin.master_key.rotated",
];

/// Fetch the `n` most-recent audit rows whose `action` starts with any
/// of [`DASHBOARD_IMPORTANT_PREFIXES`]. Used by the admin dashboard.
pub async fn recent_important(db: &Database, n: usize) -> StoreResult<Vec<AuditLogRow>> {
    let n_i64 = n as i64;
    let clauses: Vec<String> = (0..DASHBOARD_IMPORTANT_PREFIXES.len())
        .map(|i| format!("action LIKE ?{}", i + 2))
        .collect();
    let sql = format!(
        "SELECT at, actor, action, target, result, note \
         FROM audit_log WHERE {} \
         ORDER BY seq DESC LIMIT ?1",
        clauses.join(" OR ")
    );
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&sql)?;
        let mut params: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::Integer(n_i64)];
        for p in DASHBOARD_IMPORTANT_PREFIXES {
            params.push(rusqlite::types::Value::Text(format!("{p}%")));
        }
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), map)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    })
    .await
}

#[cfg(test)]
mod tests_rfc085 {
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
}
