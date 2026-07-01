//! `client_registration_token` repository — initial access tokens for
//! RFC 7591 dynamic client registration (RFC 008, P4/P5).
//!
//! Tokens are stored hashed (SHA-256 hex).  Each token has a `max_uses`
//! cap (0 = unlimited) and an optional `expires_at` TTL.  `used_count` is
//! incremented atomically when a registration succeeds.  Tokens are
//! revoked by the operator via the admin panel or CLI.
//!
//! The constant-time comparison happens at the call site (the handler),
//! not here — this repo only does CRUD and consumption tracking.

use crate::{Database, StoreResult, errors::StoreError};
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::RegistrationTokenId;

#[derive(Debug, Clone)]
pub struct RegistrationTokenRow {
    pub id: RegistrationTokenId,
    /// SHA-256 hex of the raw bearer token.
    pub token_hash: String,
    /// 0 = unlimited.
    pub max_uses: i64,
    pub used_count: i64,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<RegistrationTokenRow> {
    Ok(RegistrationTokenRow {
        id: row.get::<_, String>(0)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        token_hash: row.get(1)?,
        max_uses: row.get(2)?,
        used_count: row.get(3)?,
        expires_at: row.get(4)?,
        revoked_at: row.get(5)?,
        note: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

const SELECT: &str = "SELECT id, token_hash, max_uses, used_count, expires_at, revoked_at, \
     note, created_at, updated_at FROM client_registration_token";

/// Insert a new token.
pub async fn create(db: &Database, row: &RegistrationTokenRow) -> StoreResult<()> {
    let row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO client_registration_token \
             (id, token_hash, max_uses, used_count, expires_at, revoked_at, \
              note, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.id.to_string(),
                row.token_hash,
                row.max_uses,
                row.used_count,
                row.expires_at,
                row.revoked_at,
                row.note,
                row.created_at,
                row.updated_at,
            ],
        )?;
        Ok(())
    })
    .await
}

/// Fetch all tokens ordered by creation date, newest first.
pub async fn list(db: &Database) -> StoreResult<Vec<RegistrationTokenRow>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY created_at DESC"))?;
        let rows = stmt.query_map([], map)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::from)
    })
    .await
}

/// Fetch a single token by id.
pub async fn get(db: &Database, id: RegistrationTokenId) -> StoreResult<RegistrationTokenRow> {
    let id_str = id.to_string();
    db.with_conn(move |conn| {
        conn.query_row(&format!("{SELECT} WHERE id = ?1"), [&id_str], map)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })
    })
    .await
}

/// Find a token by its hash.  Returns `None` if not found (rather than
/// `NotFound`) so the caller can make a constant-time decision without
/// branching on error.
pub async fn find_by_hash(
    db: &Database,
    token_hash: &str,
) -> StoreResult<Option<RegistrationTokenRow>> {
    let hash = token_hash.to_owned();
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE token_hash = ?1"))?;
        let mut rows = stmt.query_map([&hash], map)?;
        match rows.next() {
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(StoreError::from(e)),
            None => Ok(None),
        }
    })
    .await
}

/// Atomically check that the token is valid and consume one use.
///
/// Returns `Ok(true)` if the token was valid and the use was recorded.
/// Returns `Ok(false)` if the token is invalid (expired, revoked, or
/// max_uses exhausted).  Never returns `NotFound` — callers should treat
/// an unknown hash the same as an exhausted token (constant-time, P5).
pub async fn consume(db: &Database, token_hash: &str, now: DateTime<Utc>) -> StoreResult<bool> {
    let hash = token_hash.to_owned();
    db.with_tx(move |tx| {
        // Read
        let row_opt: Option<RegistrationTokenRow> = {
            let mut stmt = tx.prepare(&format!("{SELECT} WHERE token_hash = ?1"))?;
            let mut rows = stmt.query_map([&hash], map)?;
            match rows.next() {
                Some(Ok(r)) => Some(r),
                Some(Err(e)) => return Err(StoreError::from(e)),
                None => None,
            }
        };
        let row = match row_opt {
            None => return Ok(false),
            Some(r) => r,
        };
        // Validate
        if row.revoked_at.is_some() {
            return Ok(false);
        }
        if row.expires_at.is_some_and(|exp| now > exp) {
            return Ok(false);
        }
        if row.max_uses > 0 && row.used_count >= row.max_uses {
            return Ok(false);
        }
        // Consume
        tx.execute(
            "UPDATE client_registration_token \
             SET used_count = used_count + 1, updated_at = ?1 WHERE id = ?2",
            params![now, row.id.to_string()],
        )?;
        Ok(true)
    })
    .await
}

/// Revoke a token immediately.  Returns `NotFound` if the id is unknown.
pub async fn revoke(db: &Database, id: RegistrationTokenId, now: DateTime<Utc>) -> StoreResult<()> {
    let id_str = id.to_string();
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE client_registration_token \
             SET revoked_at = ?1, updated_at = ?2 WHERE id = ?3",
            params![now, now, id_str],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{Database, crypto::MasterKey};

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).unwrap()
    }

    fn sample_row(max_uses: i64, expires_at: Option<DateTime<Utc>>) -> RegistrationTokenRow {
        let now = chrono::Utc::now();
        RegistrationTokenRow {
            id: RegistrationTokenId::new(),
            token_hash: "abc123".into(),
            max_uses,
            used_count: 0,
            expires_at,
            revoked_at: None,
            note: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn consume_valid_token_returns_true() {
        let db = fresh_db();
        create(&db, &sample_row(0, None)).await.unwrap();
        let ok = consume(&db, "abc123", chrono::Utc::now()).await.unwrap();
        assert!(ok);
        let row = find_by_hash(&db, "abc123").await.unwrap().unwrap();
        assert_eq!(row.used_count, 1);
    }

    #[tokio::test]
    async fn consume_unknown_token_returns_false() {
        let db = fresh_db();
        let ok = consume(&db, "nosuchtoken", chrono::Utc::now())
            .await
            .unwrap();
        assert!(!ok, "unknown token must return false");
    }

    #[tokio::test]
    async fn consume_exhausted_token_returns_false() {
        let db = fresh_db();
        create(&db, &sample_row(1, None)).await.unwrap();
        assert!(consume(&db, "abc123", chrono::Utc::now()).await.unwrap());
        // Second use: should be rejected (max_uses = 1)
        assert!(!consume(&db, "abc123", chrono::Utc::now()).await.unwrap());
    }

    #[tokio::test]
    async fn revoke_blocks_consume() {
        let db = fresh_db();
        let row = sample_row(0, None);
        let id = row.id;
        create(&db, &row).await.unwrap();
        revoke(&db, id, chrono::Utc::now()).await.unwrap();
        assert!(!consume(&db, "abc123", chrono::Utc::now()).await.unwrap());
    }
}
