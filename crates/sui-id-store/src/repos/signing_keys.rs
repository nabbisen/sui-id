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
pub async fn insert_with_plaintext(
    db: &Database,
    id: SigningKeyId,
    algorithm: &str,
    private_key_plain: &[u8],
    public_key: &[u8],
    is_active: bool,
) -> StoreResult<SigningKeyRow> {
    let sealed = seal(db.key(), private_key_plain, AAD)?;
    let now = Utc::now();
    let algorithm = algorithm.to_owned();
    let public_key = public_key.to_vec();
    let algorithm_copy = algorithm.clone();
    let public_key_copy = public_key.clone();
    db.with_conn(move |conn| {
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
    }).await?;
    Ok(SigningKeyRow {
        id,
        algorithm: algorithm_copy,
        private_key_enc: vec![],
        public_key: public_key_copy,
        is_active,
        created_at: now,
        rotated_at: None,
    })
}

/// AAD used when sealing / opening signing key private bytes. Exposed so the
/// rotation path in `sui-id-core::admin` can seal the key *before* entering
/// the DB transaction.
pub const SIGNING_KEY_AAD: &[u8] = AAD;

/// Insert a signing key using already-sealed private key bytes, directly on
/// a `Transaction`. Used by the rotation path (retire-then-insert in one tx).
pub fn insert_sealed_on_conn(
    tx: &rusqlite::Transaction<'_>,
    id: SigningKeyId,
    algorithm: &str,
    private_key_sealed: &[u8],
    public_key: &[u8],
    is_active: bool,
) -> StoreResult<()> {
    let now = Utc::now();
    tx.execute(
        "INSERT INTO signing_keys(id, algorithm, private_key_enc, public_key, is_active, created_at, rotated_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![
            id.to_string(),
            algorithm,
            private_key_sealed,
            public_key,
            is_active as i64,
            now,
        ],
    )?;
    Ok(())
}
///
/// When more than one row has `is_active = 1` (which can briefly happen
/// during a rotation transaction), the **most recently created** one wins.
/// This is the key newly issued tokens should be signed with.
pub async fn active(db: &Database) -> StoreResult<SigningKeyRow> {
    db.with_conn(move |conn| {
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
    }).await
}

/// Mark a key as retired: set `is_active = 0` and stamp `rotated_at`.
/// The row is **not** deleted — its public half stays in JWKS so that
/// already-issued tokens can still be verified during the grace window.
/// Returns `NotFound` if no key has the given id.
pub async fn retire(db: &Database, id: SigningKeyId) -> StoreResult<()> {
    db.with_conn(move |conn| {
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
    }).await
}

/// Hard-delete a signing key row. Only permitted for already-retired keys
/// — deleting an active key would break newly-minted token verification.
/// Returns `Conflict` for an active row, `NotFound` for a missing one.
pub async fn delete(db: &Database, id: SigningKeyId) -> StoreResult<()> {
    db.with_conn(move |conn| {
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
    }).await
}

/// Decrypt the private key bytes for a row.
pub async fn unseal_private(db: &Database, row: &SigningKeyRow) -> StoreResult<Vec<u8>> {
    open(db.key(), &row.private_key_enc, AAD)
}

/// All currently active and recently retired keys, useful for JWKS.
pub async fn list_published(db: &Database) -> StoreResult<Vec<SigningKeyRow>> {
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, algorithm, private_key_enc, public_key, is_active, created_at, rotated_at FROM signing_keys \
             ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map([], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }).await
}

/// Active signing keys only (is_active = 1). Used by the JWKS cache
/// to populate the verification cache on startup and after key rotation.
pub async fn list_active(db: &Database) -> StoreResult<Vec<SigningKeyRow>> {
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, algorithm, private_key_enc, public_key, is_active, created_at, rotated_at              FROM signing_keys WHERE is_active = 1 ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map([], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }).await
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

/// Retire the currently active key (if any) and insert a new one, atomically.
///
/// This is the correct rotation order when the partial unique index
/// `idx_signing_keys_single_active` is present (migration 0021): the index
/// allows at most one `is_active = 1` row, so the old insert-then-retire
/// order would briefly create two active rows and violate the constraint.
///
/// Both steps execute inside a single transaction; external readers never
/// observe the zero-active-keys gap between them.
///
/// `private_key_plain` is sealed with the master key before the INSERT.
pub async fn rotate_atomic(
    db: &Database,
    new_id: SigningKeyId,
    algorithm: &str,
    private_key_plain: &[u8],
    public_key: &[u8],
) -> StoreResult<SigningKeyRow> {
    // Seal outside the transaction so crypto work is not inside the mutex.
    let sealed = seal(db.key(), private_key_plain, AAD)?;
    let pk_vec = public_key.to_vec();
    let pk_vec_ret = pk_vec.clone();
    let algorithm_owned = algorithm.to_owned();
    let algorithm_ret = algorithm_owned.clone();
    let now = Utc::now();
    db.with_tx(move |tx| {
        // Step 1: retire any currently active key.
        tx.execute(
            "UPDATE signing_keys SET is_active = 0, rotated_at = ?1 WHERE is_active = 1",
            params![now],
        )?;
        // Step 2: insert the new active key.
        insert_sealed_on_conn(tx, new_id, algorithm_owned.as_str(), &sealed, &pk_vec, true)?;
        Ok(())
    }).await?;
    Ok(SigningKeyRow {
        id: new_id,
        algorithm: algorithm_ret,
        private_key_enc: vec![],
        public_key: pk_vec_ret,
        is_active: true,
        created_at: now,
        rotated_at: None,
    })
}
