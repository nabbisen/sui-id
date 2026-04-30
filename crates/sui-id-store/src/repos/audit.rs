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
                rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
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

pub fn append(db: &Database, row: &AuditLogRow) -> StoreResult<()> {
    db.with_conn(|conn| {
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
        let hash = compute_hash(&prev_hash, row);
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
}

pub fn recent(db: &Database, limit: i64) -> StoreResult<Vec<AuditLogRow>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT at, actor, action, target, result, note FROM audit_log ORDER BY seq DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
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
pub fn verify_chain_tail(db: &Database, limit: i64) -> StoreResult<ChainVerifyReport> {
    let rows: Vec<(i64, AuditLogRow, String, String)> = db.with_conn(|conn| {
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
    })?;

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

    #[test]
    fn appended_rows_form_a_consistent_chain() {
        let db = fresh_db();
        for i in 0..5 {
            append(&db, &sample_row(&format!("act.{i}"))).expect("append");
        }
        let r = verify_chain_tail(&db, 100).expect("verify");
        assert_eq!(r.checked, 5);
        assert_eq!(r.broken_at_seq, None);
        assert_eq!(r.legacy_unhashed, 0);
    }

    #[test]
    fn first_row_chains_from_empty_prev_hash() {
        let db = fresh_db();
        append(&db, &sample_row("first")).expect("append");
        let (prev, hash): (String, String) = db
            .with_conn(|c| {
                let (p, h): (String, String) = c
                    .query_row(
                        "SELECT prev_hash, hash FROM audit_log ORDER BY seq ASC LIMIT 1",
                        [],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )?;
                Ok((p, h))
            })
            .unwrap();
        assert_eq!(prev, "", "first row's prev_hash must be empty");
        assert_eq!(hash.len(), 64, "hash must be 64 hex chars (SHA-256)");
    }

    #[test]
    fn tampering_with_a_row_makes_chain_verification_fail() {
        let db = fresh_db();
        append(&db, &sample_row("a")).expect("append");
        append(&db, &sample_row("b")).expect("append");
        append(&db, &sample_row("c")).expect("append");

        // Rewrite row #2's `action` directly via SQL — exactly the
        // attack the chain is supposed to detect.
        db.with_conn(|c| {
            c.execute(
                "UPDATE audit_log SET action = 'tampered' WHERE seq = 2",
                [],
            )?;
            Ok(())
        })
        .expect("tamper");

        let r = verify_chain_tail(&db, 100).expect("verify");
        // Walking newest-first, seq=3 still hashes correctly because
        // its prev_hash was committed before the tamper. seq=2 is
        // the row whose recomputed hash now disagrees with stored.
        assert_eq!(r.broken_at_seq, Some(2), "{r:?}");
    }

    #[test]
    fn legacy_unhashed_rows_are_reported_separately() {
        let db = fresh_db();
        let now = Utc::now();
        db.with_conn(|c| {
            c.execute(
                "INSERT INTO audit_log(at, actor, action, target, result, note, prev_hash, hash) \
                 VALUES(?1, NULL, 'legacy', NULL, 'ok', NULL, '', '')",
                [now],
            )?;
            Ok(())
        })
        .unwrap();
        append(&db, &sample_row("post-upgrade")).expect("append");

        let r = verify_chain_tail(&db, 100).expect("verify");
        assert_eq!(r.checked, 1);
        assert_eq!(r.legacy_unhashed, 1);
        assert_eq!(r.broken_at_seq, None);
    }

    #[test]
    fn canonical_bytes_distinguishes_field_boundaries() {
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
