//! Append-only audit log with a tamper-evident hash chain.
//!
//! Each row's `hash` is `SHA-256(prev_hash || canonical_row_bytes)`.
//! The `prev_hash` of row N+1 is the `hash` of row N. Rewriting or
//! deleting row N without also fixing up **row N+1's `prev_hash`** is what
//! makes the tamper detectable (RFC 125): [`verify_chain_tail`] checks that
//! linkage — row N+1's `prev_hash` really is row N's `hash` — and that `seq`
//! is contiguous, not merely that each row's own `hash` column is a correct
//! function of that row's own `prev_hash` and content. The one thing this
//! cannot detect, inherent to any forward hash chain and not a defect: the
//! **current newest row**, before anything is appended after it, has no
//! successor yet to record its hash, so it can be rewritten (with its own
//! hash recomputed from its own unchanged `prev_hash`) without leaving a
//! trace *at that moment*. The moment a further row is appended, that
//! window closes for good — its `prev_hash` now fixes the value forever.
//!
//! A row whose `hash` column is empty is a pre-v0.17.0 legacy row and is
//! skipped rather than linkage-checked — its `prev_hash` is a migration
//! default, not data. **Legacy rows are therefore held to a rule of their
//! own, closed 2026-09-30 (RFC 125 stage 2): once a hashed row has been
//! seen walking the chain, a later row cannot legitimately be legacy.**
//! Hashing turned on once and never off, so blanking a hashed row's `hash`
//! and relinking its successor to make it look like the legacy boundary —
//! which used to take that row out of verification in two writes, raising
//! only `legacy_unhashed` and nothing else — is now exactly what this rule
//! catches, at the demoted row itself.
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
use rusqlite::OptionalExtension;
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
    db.with_read(move |conn| {
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
    db.with_read(move |conn| {
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
    /// Rows examined that participate in the chain (excludes
    /// `legacy_unhashed`, includes every row a break could be reported at).
    pub checked: usize,
    /// The `seq` of the first row (walking oldest to newest) found to be
    /// wrong in any of the ways this covers: its own `hash` does not match
    /// `SHA-256(prev_hash || content)`; its `prev_hash` does not match the
    /// previous row's actual `hash`; a `seq` gap precedes it; or it is the
    /// oldest row in the whole table (no predecessor exists at all) but its
    /// own `seq` is not `1`. `None` means none of those held for any row
    /// examined, **within the window this call was given** — see
    /// [`verify_chain_tail`]'s own doc for what "examined" covers at the
    /// window's edge.
    pub broken_at_seq: Option<i64>,
    /// Rows that were skipped because they predate the v0.17.0
    /// migration (their `hash` column is empty). These don't
    /// indicate tampering — they were never hashed in the first
    /// place. Reported for transparency.
    pub legacy_unhashed: usize,
}

/// Walk the most-recent `limit` audit rows and verify them **as a chain**:
/// each row's `prev_hash` must equal the previous row's `hash`, `seq` must be
/// contiguous, and each row's own `hash` must still be
/// `SHA-256(prev_hash || canonical_bytes)`. Stops at the first row (oldest to
/// newest) that fails any of those and reports its `seq`.
///
/// **The window's edge is checked, not assumed (RFC 125 D3).** The oldest row
/// the `LIMIT` returns has no predecessor *within that result set*, so this
/// reads one row further back (`seq` one less than the oldest one returned)
/// to learn what its `prev_hash` actually had to be, rather than treating
/// that boundary as verified when it was never compared to anything. If no
/// such row exists, the oldest row returned must genuinely be the table's
/// first row ever (`seq == 1`) for this to be legitimate (D4) — a survivor of
/// a truncated head has no predecessor either, but is not `seq == 1`, and
/// that is exactly what this catches.
///
/// **What one call cannot tell you.** It covers the returned window plus the
/// one boundary row behind it — not rows older than that, and not the
/// current newest row's own vulnerability to a rewrite with no successor yet
/// to catch it (module doc above). Intended to be called once at startup
/// with a cheap limit (a few thousand rows): more than enough to catch a
/// recent tampering attempt, cheap enough to not noticeably extend boot time.
pub async fn verify_chain_tail(db: &Database, limit: i64) -> StoreResult<ChainVerifyReport> {
    // (seq, row, prev_hash, hash) per row in the window; and the one row
    // older than the window (seq, hash), if any (RFC 125 D3).
    type ChainRows = Vec<(i64, AuditLogRow, String, String)>;
    type ChainBoundary = Option<(i64, String)>;

    let (rows, boundary): (ChainRows, ChainBoundary) = db
        .with_read(move |conn| {
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
            // RFC 125 D3: one row older than the oldest one returned, so the
            // window's edge is checked against its true predecessor instead
            // of silently treated as verified.
            let boundary = match collected.last() {
                Some((oldest_seq, ..)) => conn
                    .prepare(
                        "SELECT seq, hash FROM audit_log WHERE seq < ?1 \
                         ORDER BY seq DESC LIMIT 1",
                    )?
                    .query_row([oldest_seq], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?,
                None => None,
            };
            Ok((collected, boundary))
        })
        .await?;

    let mut report = ChainVerifyReport {
        checked: 0,
        broken_at_seq: None,
        legacy_unhashed: 0,
    };

    // Walking oldest to newest, `expected_prev` and `expected_seq` are what
    // the row about to be examined must match, given everything already
    // verified — starting from the boundary row read above, if one exists.
    // A predecessor whose own `hash` is empty (a legacy row) sets
    // `expected_prev` to `""`, which is exactly what a legitimate row
    // starting the hashed portion of the log has, so genesis-after-legacy
    // and genesis-at-the-very-first-row share one comparison, not two.
    let mut expected_prev = boundary.as_ref().map_or(String::new(), |(_, h)| h.clone());
    let mut expected_seq = boundary.as_ref().map(|(seq, _)| seq + 1);
    let no_boundary = boundary.is_none();
    // RFC 125 stage 2: legacy rows are a contiguous prefix — hashing turned
    // on once and never off, so a genuine legacy row can never follow a
    // genuine hashed one. `seen_hashed` starts true if the boundary row
    // (one older than the window) was itself hashed, so the window's very
    // first row is held to the same rule as every row after it.
    let mut seen_hashed = boundary.as_ref().is_some_and(|(_, h)| !h.is_empty());

    for (i, (seq, row, prev, hash)) in rows.iter().rev().enumerate() {
        // D4: when nothing precedes this row at all, it is only legitimate
        // if it is genuinely the table's first row. A row surviving a
        // truncated head has no predecessor either, but was not `seq == 1`.
        if i == 0 && no_boundary && *seq != 1 {
            report.broken_at_seq = Some(*seq);
            return Ok(report);
        }
        // D2: `seq` must be contiguous. A gap is a deleted row (middle or
        // head) that the linkage check below might otherwise not catch on
        // its own if the surviving rows happen to still agree pairwise.
        if let Some(exp_seq) = expected_seq
            && *seq != exp_seq
        {
            report.broken_at_seq = Some(*seq);
            return Ok(report);
        }
        expected_seq = Some(seq + 1);

        if hash.is_empty() {
            if seen_hashed {
                // A hashed row cannot legitimately be followed by a legacy
                // one. Blanking row `seq`'s `hash` and relinking its
                // successor to treat it as a legacy boundary is exactly
                // what this catches (RFC 125 stage 2).
                report.broken_at_seq = Some(*seq);
                return Ok(report);
            }
            // Pre-v0.17.0 row: not part of the chain, and its `prev_hash` is
            // a migration default, not data — nothing to compare it to.
            report.legacy_unhashed += 1;
            expected_prev = String::new();
            continue;
        }
        seen_hashed = true;
        report.checked += 1;

        // D1: row N's `prev_hash` must be row N-1's actual `hash` — not
        // merely a value this row's own hash formula happens to agree with.
        if *prev != expected_prev {
            report.broken_at_seq = Some(*seq);
            return Ok(report);
        }
        let computed = compute_hash(prev, row);
        if computed != *hash {
            report.broken_at_seq = Some(*seq);
            return Ok(report);
        }
        expected_prev = hash.clone();
    }
    Ok(report)
}

#[cfg(test)]
mod tests;

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
    db.with_read(move |conn| {
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

/// RFC 103 5b: the signed-in user's own most recent recovery-link event —
/// `user.recovery_link.issued` (the target is always the user this reads
/// for; the actor, if any, is the issuing administrator) or
/// `auth.password.reset_completed` — for the account page's summary line.
/// `None` if the user has neither. The two action names are those U37 and
/// U10 actually write; nothing here derives them from the registry, so a
/// rename of either must update this query too (as G13 requires for the
/// registry side).
pub async fn most_recent_recovery_event_for_user(
    db: &Database,
    user_id: sui_id_shared::ids::UserId,
) -> StoreResult<Option<AuditLogRow>> {
    use rusqlite::OptionalExtension;
    let uid = user_id.to_string();
    db.with_read(move |conn| {
        Ok(conn
            .prepare(
                "SELECT at, actor, action, target, result, note FROM audit_log \
                 WHERE target = ?1 \
                   AND action IN ('user.recovery_link.issued', 'auth.password.reset_completed') \
                 ORDER BY seq DESC LIMIT 1",
            )?
            .query_row([uid], map)
            .optional()?)
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
    db.with_read(move |conn| {
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
    "mfa.admin_reset",
    "user.recovery_link.issued",
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
    db.with_read(move |conn| {
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
mod tests_rfc085;
