//! Append-only audit log.
//!
//! By design this module exposes only `append` and read operations: the
//! codebase never updates or deletes audit rows. Operators who need to comply
//! with retention windows can issue manual SQL with appropriate review.

use crate::db::Database;
use crate::errors::StoreResult;
use crate::models::AuditLogRow;
use chrono::{DateTime, Utc};
use rusqlite::params;

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditLogRow> {
    Ok(AuditLogRow {
        at: row.get::<_, DateTime<Utc>>(0)?,
        actor: row
            .get::<_, Option<String>>(1)?
            .map(|s| s.parse())
            .transpose()
            .map_err(|e: uuid::Error| {
                rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
            })?,
        action: row.get(2)?,
        target: row.get(3)?,
        result: row.get(4)?,
        note: row.get(5)?,
    })
}

pub fn append(db: &Database, row: &AuditLogRow) -> StoreResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO audit_log(at, actor, action, target, result, note) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                row.at,
                row.actor.map(|u| u.to_string()),
                row.action,
                row.target,
                row.result,
                row.note,
            ],
        )?;
        Ok(())
    })
}

pub fn recent(db: &Database, limit: i64) -> StoreResult<Vec<AuditLogRow>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT at, actor, action, target, result, note FROM audit_log ORDER BY seq DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}
