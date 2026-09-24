//! `password_reset_tokens` table — see migration 0015.
//!
//! The plaintext token is never stored. Callers compute SHA-256
//! and pass the hash bytes to `find_by_hash` / `consume`.

use crate::{
    Database, StoreError, StoreResult,
    models::{PasswordResetTokenRow, ResetTokenOrigin},
};
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::{PasswordResetTokenId, UserId};

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PasswordResetTokenRow> {
    let id_str: String = row.get(0)?;
    let user_id_str: String = row.get(1)?;
    Ok(PasswordResetTokenRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        user_id: user_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        token_hash: row.get(2)?,
        issued_at: row.get::<_, DateTime<Utc>>(3)?,
        expires_at: row.get::<_, DateTime<Utc>>(4)?,
        consumed_at: row.get::<_, Option<DateTime<Utc>>>(5)?,
        requester_ip: row.get(6)?,
        issued_via: {
            let s: String = row.get(7)?;
            ResetTokenOrigin::parse(&s).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    "unknown password_reset_tokens.issued_via".into(),
                )
            })?
        },
        issued_by: row
            .get::<_, Option<String>>(8)?
            .map(|s| {
                s.parse().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        8,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .transpose()?,
        revoked_at: row.get::<_, Option<DateTime<Utc>>>(9)?,
    })
}

const SELECT_COLUMNS: &str = "id, user_id, token_hash, issued_at, expires_at, consumed_at, \
     requester_ip, issued_via, issued_by, revoked_at";

pub async fn insert(db: &Database, row: &PasswordResetTokenRow) -> StoreResult<()> {
    let row = row.clone();
    db.with_conn(move |conn| insert_within_tx(conn, &row)).await
}

/// [`insert`] on a caller-held connection or transaction (RFC 103 U37).
pub fn insert_within_tx(
    conn: &rusqlite::Connection,
    row: &PasswordResetTokenRow,
) -> StoreResult<()> {
    insert_with_within_tx(conn, row, false)
}

/// Insert `row`, marking it as a *provisioning* link (RFC 115 D11) or not.
/// Only U37 passes `true`, from the target's state read in its own
/// transaction.
pub fn insert_with_within_tx(
    conn: &rusqlite::Connection,
    row: &PasswordResetTokenRow,
    provisioning: bool,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO password_reset_tokens(id, user_id, token_hash, issued_at, \
                                             expires_at, consumed_at, requester_ip, \
                                             issued_via, issued_by, revoked_at, provisioning) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            row.id.to_string(),
            row.user_id.to_string(),
            row.token_hash,
            row.issued_at,
            row.expires_at,
            row.consumed_at,
            row.requester_ip,
            row.issued_via.as_str(),
            row.issued_by.map(|u| u.to_string()),
            row.revoked_at,
            provisioning,
        ],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::Conflict
        }
        other => StoreError::from(other),
    })?;
    Ok(())
}

/// Look up a token row by its hashed value. Returns `None` if no
/// row matches. The caller is responsible for checking that the row
/// is unconsumed and not expired before honouring it.
pub async fn find_by_hash(
    db: &Database,
    token_hash: &[u8],
) -> StoreResult<Option<PasswordResetTokenRow>> {
    let token_hash = token_hash.to_vec();
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM password_reset_tokens WHERE token_hash = ?1"
        ))?;
        let res = stmt.query_row([token_hash], map_row);
        match res {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    })
    .await
}

/// Mark a token row as consumed. Idempotent in practice: the
/// caller checks `consumed_at IS NULL` before calling, and the
/// row continues to exist post-consume so a replay attempt sees
/// "consumed" rather than "missing".
pub async fn mark_consumed(
    db: &Database,
    id: PasswordResetTokenId,
    consumed_at: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE password_reset_tokens SET consumed_at = ?1 WHERE id = ?2",
            params![consumed_at, id.to_string()],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

/// Same as [`mark_consumed`] but runs inside a caller-owned transaction.
pub fn mark_consumed_within_tx(
    tx: &rusqlite::Transaction<'_>,
    id: PasswordResetTokenId,
    consumed_at: DateTime<Utc>,
) -> StoreResult<()> {
    // RFC 103 D13: consume exactly once. A token already consumed, revoked
    // (D3) or expired by `consumed_at` changes no row, and the caller's
    // transaction rolls back. Without the guard two concurrent completions
    // of one token both commit.
    let n = tx.execute(
        "UPDATE password_reset_tokens SET consumed_at = ?1 \
         WHERE id = ?2 AND consumed_at IS NULL AND revoked_at IS NULL AND expires_at > ?1",
        params![consumed_at, id.to_string()],
    )?;
    if n == 0 {
        Err(StoreError::NotFound)
    } else {
        Ok(())
    }
}

/// Periodic cleanup helper: delete rows whose `expires_at` is in
/// the past *and* are unconsumed (or consumed long enough ago that
/// keeping them adds no value). The cutoff comes from the caller so
/// the test suite can drive deterministic clocks.
pub async fn delete_expired(db: &Database, before: DateTime<Utc>) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM password_reset_tokens WHERE expires_at < ?1",
            params![before],
        )?;
        Ok(n)
    })
    .await
}

/// RFC 103 D3: invalidate the user's outstanding tokens (unconsumed,
/// unrevoked, unexpired at `now`) by setting `revoked_at`, on the caller's
/// transaction. `except` spares one token: U10 keeps the token it is
/// consuming. Returns how many were revoked.
pub fn revoke_outstanding_for_user_within_tx(
    conn: &rusqlite::Connection,
    user_id: UserId,
    except: Option<PasswordResetTokenId>,
    now: DateTime<Utc>,
) -> StoreResult<usize> {
    let except = except.map(|id| id.to_string());
    Ok(conn.execute(
        "UPDATE password_reset_tokens SET revoked_at = ?1 \
         WHERE user_id = ?2 AND consumed_at IS NULL AND revoked_at IS NULL \
           AND expires_at > ?1 AND (?3 IS NULL OR id != ?3)",
        params![now, user_id.to_string(), except],
    )?)
}

/// RFC 103 D8: how many tokens `admin` issued on the web since `since`,
/// counting every state (used, revoked and expired tokens count too).
/// `provisioning` selects which throttle is being counted (RFC 115 D11): the
/// ordinary links (`false`, the five an hour) or the provisioning ones
/// (`true`, their own ceiling). The two never share a count.
pub fn count_issued_by_within_tx(
    conn: &rusqlite::Connection,
    admin: UserId,
    since: DateTime<Utc>,
    provisioning: bool,
) -> StoreResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM password_reset_tokens \
         WHERE issued_by = ?1 AND issued_via = 'web' AND issued_at > ?2 AND provisioning = ?3",
        params![admin.to_string(), since, provisioning],
        |row| row.get(0),
    )?)
}

/// RFC 103 D8: how many tokens the operator CLI issued since `since`. The
/// CLI is a separate process, so the count comes from the database.
/// `provisioning` as in [`count_issued_by_within_tx`].
pub fn count_issued_via_cli_within_tx(
    conn: &rusqlite::Connection,
    since: DateTime<Utc>,
    provisioning: bool,
) -> StoreResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM password_reset_tokens \
         WHERE issued_via = 'cli' AND issued_at > ?1 AND provisioning = ?2",
        params![since, provisioning],
        |row| row.get(0),
    )?)
}

/// The origin of token `id`, read on the caller's transaction (RFC 103 U10's
/// `origin`). `NotFound` if there is no such row.
pub fn origin_within_tx(
    conn: &rusqlite::Connection,
    id: PasswordResetTokenId,
) -> StoreResult<ResetTokenOrigin> {
    let via: String = conn
        .query_row(
            "SELECT issued_via FROM password_reset_tokens WHERE id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })?;
    ResetTokenOrigin::parse(&via)
        .ok_or_else(|| StoreError::Integrity(format!("unknown issued_via {via:?}")))
}

/// Count outstanding (unconsumed, unrevoked, unexpired) reset tokens for a
/// user. The forgot-password rate limit can use this to refuse
/// "issue another token" beyond a small ceiling, regardless of IP.
pub async fn count_active_for_user(
    db: &Database,
    user_id: UserId,
    now: DateTime<Utc>,
) -> StoreResult<i64> {
    db.with_conn(move |conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM password_reset_tokens \
             WHERE user_id = ?1 AND consumed_at IS NULL AND revoked_at IS NULL \
               AND expires_at > ?2",
            params![user_id.to_string(), now],
            |row| row.get(0),
        )?;
        Ok(n)
    })
    .await
}

/// RFC 073: Count password-reset tokens that have been issued but not
/// consumed and have not expired. A high number means many resets
/// requested but not completed — possibly worth admin attention.
pub async fn count_outstanding(
    db: &Database,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<usize> {
    let now_str = now.to_rfc3339();
    db.with_conn(move |conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM password_reset_tokens \
             WHERE consumed_at IS NULL AND revoked_at IS NULL AND expires_at > ?1",
            params![now_str],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    })
    .await
}
