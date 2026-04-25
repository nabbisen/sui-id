//! Credential storage. The password hash is opaque (Argon2id PHC format) and
//! is therefore stored as TEXT, not encrypted again.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::CredentialRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::UserId;

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<CredentialRow> {
    Ok(CredentialRow {
        user_id: row
            .get::<_, String>(0)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
        password_hash: row.get(1)?,
        must_change: row.get::<_, i64>(2)? != 0,
        updated_at: row.get::<_, DateTime<Utc>>(3)?,
    })
}

pub fn upsert(db: &Database, cred: &CredentialRow) -> StoreResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO credentials(user_id, password_hash, must_change, updated_at) \
             VALUES(?1, ?2, ?3, ?4) \
             ON CONFLICT(user_id) DO UPDATE SET \
                 password_hash = excluded.password_hash, \
                 must_change = excluded.must_change, \
                 updated_at = excluded.updated_at",
            params![
                cred.user_id.to_string(),
                cred.password_hash,
                cred.must_change as i64,
                cred.updated_at,
            ],
        )?;
        Ok(())
    })
}

pub fn get(db: &Database, user_id: UserId) -> StoreResult<CredentialRow> {
    db.with_conn(|conn| {
        conn.query_row(
            "SELECT user_id, password_hash, must_change, updated_at FROM credentials WHERE user_id = ?1",
            [user_id.to_string()],
            map,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
}
