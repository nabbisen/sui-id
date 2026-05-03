//! Signing key storage. Private key bytes are sealed with the master key.

use crate::crypto::{open, seal};
use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::SigningKeyRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::SigningKeyId;

const AAD: &[u8] = b"sui-id/signing_key/v1";

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<SigningKeyRow> {
    Ok(SigningKeyRow {
        id: row
            .get::<_, String>(0)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
        algorithm: row.get(1)?,
        private_key_enc: row.get(2)?,
        public_key: row.get(3)?,
        is_active: row.get::<_, i64>(4)? != 0,
        created_at: row.get::<_, DateTime<Utc>>(5)?,
        rotated_at: row.get::<_, Option<DateTime<Utc>>>(6)?,
    })
}

/// Insert a new signing key. Pass the *plaintext* private key bytes in
/// `private_key_plain`; this function seals them before INSERT.
pub fn insert_with_plaintext(
    db: &Database,
    id: SigningKeyId,
    algorithm: &str,
    private_key_plain: &[u8],
    public_key: &[u8],
    is_active: bool,
) -> StoreResult<SigningKeyRow> {
    let sealed = seal(db.key(), private_key_plain, AAD)?;
    let now = Utc::now();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO signing_keys(id, algorithm, private_key_enc, public_key, is_active, created_at, rotated_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![
                id.to_string(),
                algorithm,
                sealed,
                public_key,
                is_active as i64,
                now,
            ],
        )?;
        Ok(())
    })?;
    Ok(SigningKeyRow {
        id,
        algorithm: algorithm.to_owned(),
        private_key_enc: vec![],
        public_key: public_key.to_vec(),
        is_active,
        created_at: now,
        rotated_at: None,
    })
}

/// Get the currently active signing key, or `NotFound` if none exists.
///
/// When more than one row has `is_active = 1` (which can briefly happen
/// during a rotation transaction), the **most recently created** one wins.
/// This is the key newly issued tokens should be signed with.
pub fn active(db: &Database) -> StoreResult<SigningKeyRow> {
    db.with_conn(|conn| {
        conn.query_row(
            "SELECT id, algorithm, private_key_enc, public_key, is_active, created_at, rotated_at FROM signing_keys \
             WHERE is_active = 1 ORDER BY created_at DESC LIMIT 1",
            [],
            map,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
}

/// Mark a key as retired: set `is_active = 0` and stamp `rotated_at`.
/// The row is **not** deleted — its public half stays in JWKS so that
/// already-issued tokens can still be verified during the grace window.
/// Returns `NotFound` if no key has the given id.
pub fn retire(db: &Database, id: SigningKeyId) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE signing_keys SET is_active = 0, rotated_at = ?1 \
             WHERE id = ?2 AND is_active = 1",
            rusqlite::params![Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            // Either the id doesn't exist or it was already retired. We
            // distinguish by a probe; the caller usually only cares that
            // the row is now in the retired state.
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM signing_keys WHERE id = ?1",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if exists == 0 {
                return Err(StoreError::NotFound);
            }
        }
        Ok(())
    })
}

/// Hard-delete a signing key row. Only permitted for already-retired keys
/// — deleting an active key would break newly-minted token verification.
/// Returns `Conflict` for an active row, `NotFound` for a missing one.
pub fn delete(db: &Database, id: SigningKeyId) -> StoreResult<()> {
    db.with_conn(|conn| {
        let row: Option<i64> = conn
            .query_row(
                "SELECT is_active FROM signing_keys WHERE id = ?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .ok();
        match row {
            None => Err(StoreError::NotFound),
            Some(1) => Err(StoreError::Conflict),
            Some(_) => {
                conn.execute(
                    "DELETE FROM signing_keys WHERE id = ?1",
                    [id.to_string()],
                )?;
                Ok(())
            }
        }
    })
}

/// Decrypt the private key bytes for a row.
pub fn unseal_private(db: &Database, row: &SigningKeyRow) -> StoreResult<Vec<u8>> {
    open(db.key(), &row.private_key_enc, AAD)
}

/// All currently active and recently retired keys, useful for JWKS.
pub fn list_published(db: &Database) -> StoreResult<Vec<SigningKeyRow>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, algorithm, private_key_enc, public_key, is_active, created_at, rotated_at FROM signing_keys \
             ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map([], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

/// Re-seal every `private_key_enc` row under `new_key`. Used by
/// master-key rotation. Runs inside the caller's transaction —
/// the function does not commit.
pub fn reseal_all(
    tx: &rusqlite::Transaction<'_>,
    old_key: &crate::crypto::MasterKey,
    new_key: &crate::crypto::MasterKey,
) -> StoreResult<u64> {
    let mut stmt = tx.prepare("SELECT id, private_key_enc FROM signing_keys")?;
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
            "UPDATE signing_keys SET private_key_enc = ?1 WHERE id = ?2",
            rusqlite::params![resealed, id],
        )?;
        count += 1;
    }
    Ok(count)
}
