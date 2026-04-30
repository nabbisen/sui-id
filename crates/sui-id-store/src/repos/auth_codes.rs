//! Authorization code single-use storage.
//!
//! The plaintext code is never stored: we keep only a SHA-256 hash so that
//! a database leak does not let an attacker replay outstanding codes. Codes
//! are single-use; consumption flips the `consumed` flag inside the same
//! transaction that issues the access token.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::AuthorizationCodeRow;
use chrono::{DateTime, Utc};
use rusqlite::params;

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuthorizationCodeRow> {
    let auth_methods_json: String = row.get(11)?;
    let auth_methods = serde_json::from_str(&auth_methods_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(AuthorizationCodeRow {
        code_hash: row.get(0)?,
        client_id: row
            .get::<_, String>(1)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?,
        user_id: row
            .get::<_, String>(2)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?,
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

pub fn insert(db: &Database, row: &AuthorizationCodeRow) -> StoreResult<()> {
    let methods_json = serde_json::to_string(&row.auth_methods)?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO auth_codes(code_hash, client_id, user_id, redirect_uri, scope, nonce, code_challenge, code_challenge_method, expires_at, consumed, created_at, auth_methods) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                row.code_hash,
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
    })
}

/// Atomically fetch and mark-as-consumed an authorization code. Returns
/// `NotFound` if the code does not exist, was already consumed, or has
/// expired.
pub fn consume(db: &Database, code_hash: &str) -> StoreResult<AuthorizationCodeRow> {
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        let row: AuthorizationCodeRow = tx
            .query_row(
                "SELECT code_hash, client_id, user_id, redirect_uri, scope, nonce, code_challenge, code_challenge_method, expires_at, consumed, created_at, auth_methods FROM auth_codes WHERE code_hash = ?1",
                [code_hash],
                map,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })?;
        if row.consumed || row.expires_at <= Utc::now() {
            return Err(StoreError::NotFound);
        }
        tx.execute("UPDATE auth_codes SET consumed = 1 WHERE code_hash = ?1", [code_hash])?;
        tx.commit()?;
        Ok(row)
    })
}

/// Periodic cleanup of expired entries. Called from a background task or on
/// admin demand; never required for correctness, just hygiene.
pub fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "DELETE FROM auth_codes WHERE expires_at < ?1",
            [Utc::now()],
        )?;
        Ok(n)
    })
}
