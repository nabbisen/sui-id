//! Refresh token storage.
//!
//! The plaintext token is sealed with the master key before insertion. On
//! lookup we decrypt and compare in constant time. Plaintext tokens are
//! returned to the API only at issuance.

use crate::crypto::{open, seal};
use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::RefreshTokenRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::{ClientId, UserId};

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<RefreshTokenRow> {
    Ok(RefreshTokenRow {
        id: row.get(0)?,
        token_plain: None,
        user_id: row
            .get::<_, String>(2)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?,
        client_id: row
            .get::<_, String>(3)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e)))?,
        scope: row.get(4)?,
        expires_at: row.get::<_, DateTime<Utc>>(5)?,
        revoked_at: row.get::<_, Option<DateTime<Utc>>>(6)?,
        created_at: row.get::<_, DateTime<Utc>>(7)?,
    })
}

const AAD: &[u8] = b"sui-id/refresh_token/v1";

/// Insert a new refresh token row. The plaintext token is taken from
/// `row.token_plain`; the caller is responsible for generating it.
pub fn insert(db: &Database, row: &RefreshTokenRow) -> StoreResult<()> {
    let plain = row
        .token_plain
        .as_deref()
        .ok_or_else(|| StoreError::Integrity("refresh token: missing plaintext on insert".into()))?;
    let sealed = seal(db.key(), plain.as_bytes(), AAD)?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO refresh_tokens(id, token_enc, user_id, client_id, scope, expires_at, revoked_at, created_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.id,
                sealed,
                row.user_id.to_string(),
                row.client_id.to_string(),
                row.scope,
                row.expires_at,
                row.revoked_at,
                row.created_at,
            ],
        )?;
        Ok(())
    })
}

/// Look up a token row by plaintext value. Performs constant-time comparison
/// of the decrypted bytes.
pub fn find_active(db: &Database, plaintext: &str) -> StoreResult<RefreshTokenRow> {
    use subtle::ConstantTimeEq;

    let now = Utc::now();
    let candidates: Vec<(RefreshTokenRow, Vec<u8>)> = db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, token_enc, user_id, client_id, scope, expires_at, revoked_at, created_at FROM refresh_tokens \
             WHERE revoked_at IS NULL AND expires_at > ?1",
        )?;
        let rows = stmt
            .query_map([now], |r| {
                let row = map(r)?;
                let enc: Vec<u8> = r.get(1)?;
                Ok((row, enc))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })?;

    let pt = plaintext.as_bytes();
    for (row, enc) in candidates {
        // Decryption itself authenticates the ciphertext; if it succeeds we
        // know the bytes were stored by us. We then constant-time compare to
        // the supplied plaintext to avoid timing oracles.
        if let Ok(opened) = open(db.key(), &enc, AAD) {
            if opened.ct_eq(pt).into() {
                return Ok(row);
            }
        }
    }
    Err(StoreError::NotFound)
}

pub fn revoke(db: &Database, id: &str) -> StoreResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), id],
        )?;
        Ok(())
    })
}

pub fn revoke_all_for_user(db: &Database, user_id: UserId) -> StoreResult<usize> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), user_id.to_string()],
        )?;
        Ok(n)
    })
}

pub fn revoke_all_for_client(db: &Database, client_id: ClientId) -> StoreResult<usize> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE client_id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), client_id.to_string()],
        )?;
        Ok(n)
    })
}

/// Delete revoked-and-old or expired refresh tokens. Hygiene only — expired
/// or revoked tokens are already excluded from `find_active`.
pub fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "DELETE FROM refresh_tokens WHERE expires_at < ?1 OR revoked_at IS NOT NULL",
            [Utc::now()],
        )?;
        Ok(n)
    })
}
