//! Authorization code single-use storage (RFC 078: typed CodeHash).
//!
//! The plaintext code is never stored: we keep only a SHA-256 hex hash so
//! that a database leak does not let an attacker replay outstanding codes.
//! Codes are single-use; consumption flips the `consumed` flag inside the
//! same transaction that issues the access token.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::AuthorizationCodeRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::{CodeHash, ids::UserId};

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuthorizationCodeRow> {
    let auth_methods_json: String = row.get(11)?;
    let auth_methods = serde_json::from_str(&auth_methods_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(AuthorizationCodeRow {
        code_hash: CodeHash::from_stored(row.get(0)?),
        client_id: row.get::<_, String>(1)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        user_id: row.get::<_, String>(2)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        redirect_uri: row.get(3)?,
        scope: row.get(4)?,
        nonce: row.get(5)?,
        code_challenge: row.get(6)?,
        code_challenge_method: row.get(7)?,
        expires_at: row.get::<_, DateTime<Utc>>(8)?,
        consumed: row.get::<_, i64>(9)? != 0,
        created_at: row.get::<_, DateTime<Utc>>(10)?,
        auth_methods,
    })
}

pub async fn insert(db: &Database, row: &AuthorizationCodeRow) -> StoreResult<()> {
    let methods_json = serde_json::to_string(&row.auth_methods)?;
    let code_hash = row.code_hash.as_str().to_owned();
    let row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO auth_codes(code_hash, client_id, user_id, redirect_uri, scope, nonce, code_challenge, code_challenge_method, expires_at, consumed, created_at, auth_methods) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                code_hash,
                row.client_id.to_string(),
                row.user_id.to_string(),
                row.redirect_uri,
                row.scope,
                row.nonce,
                row.code_challenge,
                row.code_challenge_method,
                row.expires_at,
                row.consumed as i64,
                row.created_at,
                methods_json,
            ],
        )?;
        Ok(())
    }).await
}

/// Atomically consume an authorization code (RFC 079).
///
/// Enforcement is entirely inside the SQL statement:
///
/// ```sql
/// UPDATE auth_codes SET consumed = 1
///  WHERE code_hash = ?1 AND consumed = 0 AND expires_at > ?2
/// ```
///
/// rows-affected = 1 → the code was active and is now consumed; we
/// immediately SELECT the row in the same transaction and return it.
/// rows-affected = 0 → the code was unknown, already consumed, or expired;
/// all three cases surface identically as `StoreError::NotFound` so the
/// caller cannot distinguish them (preserves non-disclosure, RFC 079 P5).
///
/// The `now` parameter is the caller's clock snapshot; it is passed in so
/// that callers using `SharedClock` and tests using synthetic clocks agree
/// on the boundary.
pub async fn consume(
    db: &Database,
    code_hash: &CodeHash,
    now: DateTime<Utc>,
) -> StoreResult<AuthorizationCodeRow> {
    let hash_str = code_hash.as_str().to_owned();
    db.with_tx(move |tx| {
        // Atomic UPDATE: flips consumed only when the row is live.
        // rows_affected is the authoritative single-use arbiter.
        let rows_affected = tx.execute(
            "UPDATE auth_codes SET consumed = 1              WHERE code_hash = ?1 AND consumed = 0 AND expires_at > ?2",
            params![hash_str.as_str(), now],
        )?;
        if rows_affected == 0 {
            return Err(StoreError::NotFound);
        }
        // The row is now consumed; read it back within the same transaction.
        tx.query_row(
            "SELECT code_hash, client_id, user_id, redirect_uri, scope, nonce,              code_challenge, code_challenge_method, expires_at, consumed, created_at,              auth_methods FROM auth_codes WHERE code_hash = ?1",
            [hash_str.as_str()],
            map,
        )
        .map_err(StoreError::from)
    })
    .await
}

/// Mark all unconsumed auth codes for the given user as consumed. Called
/// when a user is disabled or soft-deleted so that any in-flight
/// authorization codes cannot be exchanged for tokens.
pub async fn invalidate_all_for_user(db: &Database, user_id: UserId) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE auth_codes SET consumed = 1 WHERE user_id = ?1 AND consumed = 0",
            [user_id.to_string()],
        )?;
        Ok(n)
    })
    .await
}

/// Periodic cleanup of expired entries. Called from a background task or on
/// admin demand; never required for correctness, just hygiene.
pub async fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute("DELETE FROM auth_codes WHERE expires_at < ?1", [Utc::now()])?;
        Ok(n)
    })
    .await
}

#[cfg(test)]
mod tests;
