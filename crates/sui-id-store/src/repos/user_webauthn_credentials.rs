//! Per-user WebAuthn (passkey) credentials.
//!
//! `passkey_enc` holds a serialised `webauthn_rs::prelude::Passkey`
//! sealed under the master key. The struct is opaque to sui-id and only
//! re-interpreted by webauthn-rs at authentication time. We seal it
//! because it contains the public key, signature counter, and other
//! per-credential state — none of which is, on its own, secret enough
//! to be high-value, but the conservative default is to encrypt
//! everything we can.

use crate::crypto::{open, seal};
use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::UserWebauthnCredentialRow;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use sui_id_shared::ids::{UserId, WebauthnCredentialId};

const AAD: &[u8] = b"sui-id/user_webauthn_credentials/passkey/v1";

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserWebauthnCredentialRow> {
    let id_str: String = row.get(0)?;
    let user_id_str: String = row.get(1)?;
    Ok(UserWebauthnCredentialRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        user_id: user_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        credential_id: row.get(2)?,
        passkey_enc: row.get(3)?,
        nickname: row.get(4)?,
        created_at: row.get::<_, DateTime<Utc>>(5)?,
        last_used_at: row.get::<_, Option<DateTime<Utc>>>(6)?,
    })
}

const SELECT: &str = "SELECT id, user_id, credential_id, passkey_enc, nickname, \
                      created_at, last_used_at FROM user_webauthn_credentials";

pub fn list_for_user(db: &Database, user_id: UserId) -> StoreResult<Vec<UserWebauthnCredentialRow>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE user_id = ?1 ORDER BY created_at ASC"
        ))?;
        let rows = stmt
            .query_map([user_id.to_string()], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

pub fn count_for_user(db: &Database, user_id: UserId) -> StoreResult<usize> {
    db.with_conn(|conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_webauthn_credentials WHERE user_id = ?1",
            [user_id.to_string()],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    })
}

pub fn find_by_credential_id(
    db: &Database,
    credential_id: &[u8],
) -> StoreResult<Option<UserWebauthnCredentialRow>> {
    db.with_conn(|conn| {
        Ok(conn
            .query_row(
                &format!("{SELECT} WHERE credential_id = ?1"),
                [credential_id],
                map,
            )
            .optional()?)
    })
}

pub fn create(
    db: &Database,
    row: &UserWebauthnCredentialRow,
    passkey_json_plain: &[u8],
) -> StoreResult<()> {
    let enc = seal(db.key(), passkey_json_plain, AAD)?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at, last_used_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.id.to_string(),
                row.user_id.to_string(),
                row.credential_id,
                enc,
                row.nickname,
                row.created_at,
                row.last_used_at,
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
    })
}

pub fn decrypt_passkey(db: &Database, row: &UserWebauthnCredentialRow) -> StoreResult<Vec<u8>> {
    Ok(open(db.key(), &row.passkey_enc, AAD)?)
}

/// Replace the sealed passkey blob (used after authentication when the
/// signature counter advances).
pub fn update_passkey(
    db: &Database,
    id: WebauthnCredentialId,
    passkey_json_plain: &[u8],
) -> StoreResult<()> {
    let enc = seal(db.key(), passkey_json_plain, AAD)?;
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE user_webauthn_credentials SET passkey_enc = ?1, last_used_at = ?2 WHERE id = ?3",
            params![enc, Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
}

pub fn delete(db: &Database, id: WebauthnCredentialId, user_id: UserId) -> StoreResult<()> {
    // Scoped to user_id so a stray id can't delete another user's
    // credential even via a server-side path.
    db.with_conn(|conn| {
        let n = conn.execute(
            "DELETE FROM user_webauthn_credentials WHERE id = ?1 AND user_id = ?2",
            params![id.to_string(), user_id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
}

/// Re-seal every `passkey_enc` row under `new_key`. Used by
/// master-key rotation. Runs inside the caller's transaction —
/// the function does not commit.
pub fn reseal_all(
    tx: &rusqlite::Transaction<'_>,
    old_key: &crate::crypto::MasterKey,
    new_key: &crate::crypto::MasterKey,
) -> StoreResult<u64> {
    let mut stmt = tx.prepare(
        "SELECT id, passkey_enc FROM user_webauthn_credentials",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let enc: Vec<u8> = row.get(1)?;
            Ok((id, enc))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    let mut count = 0u64;
    for (id, enc) in rows {
        let plain = crate::crypto::open(old_key, &enc, AAD)?;
        let resealed = crate::crypto::seal(new_key, &plain, AAD)?;
        tx.execute(
            "UPDATE user_webauthn_credentials SET passkey_enc = ?1 WHERE id = ?2",
            rusqlite::params![resealed, id],
        )?;
        count += 1;
    }
    Ok(count)
}
