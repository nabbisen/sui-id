//! Short-lived in-flight WebAuthn ceremonies.
//!
//! Each row matches one outstanding `start_passkey_registration` or
//! `start_passkey_authentication` call. The application layer hands
//! the row id back to the browser as a cookie; on completion the
//! browser POSTs back, we look the row up, hand the state JSON to
//! webauthn-rs, and delete the row.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::{WebauthnPendingKind, WebauthnPendingRow};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use sui_id_shared::ids::{UserId, WebauthnPendingId};

const SELECT: &str = "SELECT id, kind, user_id, state_json, expires_at, created_at \
                      FROM webauthn_pending";

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<WebauthnPendingRow> {
    let id_str: String = row.get(0)?;
    let kind_str: String = row.get(1)?;
    let user_id_str: Option<String> = row.get(2)?;
    Ok(WebauthnPendingRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        kind: WebauthnPendingKind::parse(&kind_str).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                "unknown webauthn_pending kind".into(),
            )
        })?,
        user_id: user_id_str
            .map(|s| {
                s.parse::<UserId>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .transpose()?,
        state_json: row.get(3)?,
        expires_at: row.get::<_, DateTime<Utc>>(4)?,
        created_at: row.get::<_, DateTime<Utc>>(5)?,
    })
}

pub async fn insert(db: &Database, row: &WebauthnPendingRow) -> StoreResult<()> {
    let row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO webauthn_pending(id, kind, user_id, state_json, expires_at, created_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                row.id.to_string(),
                row.kind.as_str(),
                row.user_id.map(|u| u.to_string()),
                row.state_json,
                row.expires_at,
                row.created_at,
            ],
        )?;
        Ok(())
    })
    .await
}

pub async fn get(db: &Database, id: WebauthnPendingId) -> StoreResult<Option<WebauthnPendingRow>> {
    db.with_conn(move |conn| {
        Ok(conn
            .query_row(&format!("{SELECT} WHERE id = ?1"), [id.to_string()], map)
            .optional()?)
    })
    .await
}

pub async fn delete(db: &Database, id: WebauthnPendingId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM webauthn_pending WHERE id = ?1",
            [id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Hygiene: drop expired ceremonies. Called from the GC task.
pub async fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM webauthn_pending WHERE expires_at < ?1",
            [Utc::now()],
        )?;
        Ok(n)
    })
    .await
}
