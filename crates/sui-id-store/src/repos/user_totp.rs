//! TOTP enrolment and lookup.
//!
//! Two encrypted columns: `secret_enc` (the TOTP secret) and
//! `recovery_codes_enc` (a JSON array of Argon2id hashes). Both are
//! sealed under the master key by the application layer using the same
//! XChaCha20-Poly1305 helper as every other encrypted column. The
//! plaintext does not appear in the row.

use crate::crypto::{open, seal};
use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::UserTotpRow;
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use sui_id_shared::ids::UserId;

const AAD: &[u8] = b"sui-id/user_totp/v1";
const RECOVERY_AAD: &[u8] = b"sui-id/user_totp/recovery/v1";

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserTotpRow> {
    let id_str: String = row.get(0)?;
    Ok(UserTotpRow {
        user_id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        secret_enc: row.get(1)?,
        enabled: row.get::<_, i64>(2)? != 0,
        recovery_codes_enc: row.get(3)?,
        last_used_step: row.get(4)?,
        created_at: row.get::<_, DateTime<Utc>>(5)?,
        confirmed_at: row.get::<_, Option<DateTime<Utc>>>(6)?,
    })
}

const SELECT: &str = "SELECT user_id, secret_enc, enabled, recovery_codes_enc, \
                      last_used_step, created_at, confirmed_at FROM user_totp";

pub async fn get(db: &Database, user_id: UserId) -> StoreResult<Option<UserTotpRow>> {
    db.with_conn(move |conn| {
        Ok(conn
            .query_row(
                &format!("{SELECT} WHERE user_id = ?1"),
                [user_id.to_string()],
                map,
            )
            .optional()?)
    })
    .await
}

/// Insert (or replace) a TOTP enrolment for the user. Used by both the
/// initial unconfirmed insert and by the "regenerate secret" path.
pub async fn upsert_pending(
    db: &Database,
    user_id: UserId,
    plaintext_secret: &[u8],
) -> StoreResult<()> {
    let enc = seal(db.key(), plaintext_secret, AAD)?;
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT OR REPLACE INTO user_totp \
             (user_id, secret_enc, enabled, recovery_codes_enc, last_used_step, created_at, confirmed_at) \
             VALUES (?1, ?2, 0, NULL, 0, ?3, NULL)",
            params![user_id.to_string(), enc, Utc::now()],
        )?;
        Ok(())
    }).await
}

/// Decrypt and return the raw TOTP secret. Caller must zero the buffer.
pub async fn decrypt_secret(db: &Database, row: &UserTotpRow) -> StoreResult<Vec<u8>> {
    open(db.key(), &row.secret_enc, AAD)
}

/// Mark the enrolment as confirmed and store the (already-hashed) recovery
/// codes JSON. Atomic — the user is only "MFA enabled" once both fields
/// are written.
pub async fn confirm_with_recovery(
    db: &Database,
    user_id: UserId,
    recovery_codes_json_plain: &[u8],
) -> StoreResult<()> {
    let enc = seal(db.key(), recovery_codes_json_plain, RECOVERY_AAD)?;
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE user_totp SET enabled = 1, recovery_codes_enc = ?1, confirmed_at = ?2 \
             WHERE user_id = ?3",
            params![enc, Utc::now(), user_id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Decrypt the recovery codes JSON. Returns `None` when the user has
/// never confirmed enrolment.
pub async fn decrypt_recovery_codes(
    db: &Database,
    row: &UserTotpRow,
) -> StoreResult<Option<Vec<u8>>> {
    match &row.recovery_codes_enc {
        Some(blob) => Ok(Some(open(db.key(), blob, RECOVERY_AAD)?)),
        None => Ok(None),
    }
}

/// Replace the recovery codes blob (used when the user regenerates them
/// after MFA is already enabled).
pub async fn set_recovery_codes(
    db: &Database,
    user_id: UserId,
    recovery_codes_json_plain: &[u8],
) -> StoreResult<()> {
    let enc = seal(db.key(), recovery_codes_json_plain, RECOVERY_AAD)?;
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE user_totp SET recovery_codes_enc = ?1 WHERE user_id = ?2",
            params![enc, user_id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Update the replay-defence cursor. Should be called immediately after
/// a successful TOTP code verification.
pub async fn set_last_used_step(db: &Database, user_id: UserId, step: i64) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE user_totp SET last_used_step = ?1 WHERE user_id = ?2",
            params![step, user_id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Disable TOTP for the user (delete the row entirely). Used by the
/// admin "disable MFA" action and by the user's own profile page.
pub async fn delete(db: &Database, user_id: UserId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM user_totp WHERE user_id = ?1",
            [user_id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Re-seal both `secret_enc` and `recovery_codes_enc` columns
/// under `new_key`. Returns `(secrets, recovery)` counts. Used
/// by master-key rotation; does not commit.
pub fn reseal_all(
    tx: &rusqlite::Transaction<'_>,
    old_key: &crate::crypto::MasterKey,
    new_key: &crate::crypto::MasterKey,
) -> StoreResult<(u64, u64)> {
    let mut stmt = tx.prepare("SELECT user_id, secret_enc, recovery_codes_enc FROM user_totp")?;
    let rows = stmt
        .query_map([], |row| {
            let user_id: String = row.get(0)?;
            let secret: Vec<u8> = row.get(1)?;
            let recovery: Option<Vec<u8>> = row.get(2)?;
            Ok((user_id, secret, recovery))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    let mut secrets = 0u64;
    let mut recoveries = 0u64;
    for (user_id, sec_enc, rec_enc) in rows {
        let sec_plain = crate::crypto::open(old_key, &sec_enc, AAD)?;
        let sec_resealed = crate::crypto::seal(new_key, &sec_plain, AAD)?;
        let rec_resealed = match rec_enc {
            Some(rec) => {
                let plain = crate::crypto::open(old_key, &rec, RECOVERY_AAD)?;
                let r = crate::crypto::seal(new_key, &plain, RECOVERY_AAD)?;
                recoveries += 1;
                Some(r)
            }
            None => None,
        };
        tx.execute(
            "UPDATE user_totp SET secret_enc = ?1, recovery_codes_enc = ?2 WHERE user_id = ?3",
            rusqlite::params![sec_resealed, rec_resealed, user_id],
        )?;
        secrets += 1;
    }
    Ok((secrets, recoveries))
}
