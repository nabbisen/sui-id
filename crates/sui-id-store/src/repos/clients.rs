//! Client (relying party) CRUD.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::ClientRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::ClientId;

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClientRow> {
    let uris_json: String = row.get(4)?;
    let redirect_uris: Vec<String> = serde_json::from_str(&uris_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(ClientRow {
        id: row
            .get::<_, String>(0)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
        name: row.get(1)?,
        confidential: row.get::<_, i64>(2)? != 0,
        secret_hash: row.get(3)?,
        redirect_uris,
        is_disabled: row.get::<_, i64>(5)? != 0,
        is_deleted: row.get::<_, i64>(6)? != 0,
        created_at: row.get::<_, DateTime<Utc>>(7)?,
        updated_at: row.get::<_, DateTime<Utc>>(8)?,
    })
}

const SELECT: &str = "SELECT id, name, confidential, secret_hash, redirect_uris, is_disabled, is_deleted, created_at, updated_at FROM clients";

pub fn create(db: &Database, c: &ClientRow) -> StoreResult<()> {
    let uris = serde_json::to_string(&c.redirect_uris)?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO clients(id, name, confidential, secret_hash, redirect_uris, is_disabled, is_deleted, created_at, updated_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                c.id.to_string(),
                c.name,
                c.confidential as i64,
                c.secret_hash,
                uris,
                c.is_disabled as i64,
                c.is_deleted as i64,
                c.created_at,
                c.updated_at,
            ],
        )?;
        Ok(())
    })
}

pub fn get(db: &Database, id: ClientId) -> StoreResult<ClientRow> {
    db.with_conn(|conn| {
        conn.query_row(
            &format!("{SELECT} WHERE id = ?1"),
            [id.to_string()],
            map,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
}

pub fn list(db: &Database) -> StoreResult<Vec<ClientRow>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY created_at ASC"))?;
        let rows = stmt
            .query_map([], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

pub fn update_basic(
    db: &Database,
    id: ClientId,
    name: Option<&str>,
    redirect_uris: Option<&[String]>,
) -> StoreResult<()> {
    db.with_conn(|conn| {
        // Read current row to merge new values.
        let current: ClientRow = conn.query_row(
            &format!("{SELECT} WHERE id = ?1"),
            [id.to_string()],
            map,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })?;
        let new_name = name.unwrap_or(&current.name);
        let new_uris = redirect_uris.map(<[String]>::to_vec).unwrap_or(current.redirect_uris.clone());
        let uris_json = serde_json::to_string(&new_uris)?;
        conn.execute(
            "UPDATE clients SET name = ?1, redirect_uris = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_name, uris_json, Utc::now(), id.to_string()],
        )?;
        Ok(())
    })
}

pub fn set_disabled(db: &Database, id: ClientId, disabled: bool) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE clients SET is_disabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![disabled as i64, Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
}

pub fn soft_delete(db: &Database, id: ClientId) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE clients SET is_deleted = 1, is_disabled = 1, updated_at = ?1 WHERE id = ?2",
            params![Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
}
