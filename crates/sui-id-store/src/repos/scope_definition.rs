//! `scope_definition` repository — the deployment's declared scope catalog
//! (RFC 008).
//!
//! The minimum catalog (`openid`, `profile`, `email`, `offline_access`) is
//! seeded by migration 0036.  Operators extend via the admin scope-catalog
//! page.  `requires_consent` controls whether the consent screen shows this
//! scope; `is_default` marks scopes that are always implied when a client
//! requests `openid`.

use crate::{Database, StoreResult, errors::StoreError};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct ScopeDefinitionRow {
    pub name: String,
    pub requires_consent: bool,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScopeDefinitionRow> {
    Ok(ScopeDefinitionRow {
        name: row.get(0)?,
        requires_consent: row.get::<_, i64>(1)? != 0,
        is_default: row.get::<_, i64>(2)? != 0,
        created_at: row.get(3)?,
    })
}

/// List all scope definitions ordered by name.
pub async fn list(db: &Database) -> StoreResult<Vec<ScopeDefinitionRow>> {
    db.with_read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT name, requires_consent, is_default, created_at \
             FROM scope_definition ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], map)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::from)
    })
    .await
}

/// Fetch a single scope by name.
pub async fn get(db: &Database, name: &str) -> StoreResult<ScopeDefinitionRow> {
    let name = name.to_owned();
    db.with_read(move |conn| {
        conn.prepare(
            "SELECT name, requires_consent, is_default, created_at \
             FROM scope_definition WHERE name = ?1",
        )?
        .query_row([&name], map)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
    .await
}

/// Return the set of scope names that have `requires_consent = 1`.
/// Used by the consent gate to decide whether a scope needs user approval.
pub async fn consented_names(db: &Database) -> StoreResult<std::collections::HashSet<String>> {
    db.with_read(|conn| {
        let mut stmt =
            conn.prepare("SELECT name FROM scope_definition WHERE requires_consent = 1")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<std::collections::HashSet<_>>>()
            .map_err(StoreError::from)
    })
    .await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
