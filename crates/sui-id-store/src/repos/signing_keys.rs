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
