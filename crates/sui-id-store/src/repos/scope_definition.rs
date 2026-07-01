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
use rusqlite::params;

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
    db.with_conn(|conn| {
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
    db.with_conn(move |conn| {
        conn.query_row(
            "SELECT name, requires_consent, is_default, created_at \
             FROM scope_definition WHERE name = ?1",
            [&name],
            map,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
    .await
}

/// Insert a new scope.  Returns `StoreError::Conflict` if the name already
/// exists.
pub async fn create(db: &Database, row: &ScopeDefinitionRow) -> StoreResult<()> {
    let row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO scope_definition \
             (name, requires_consent, is_default, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![
                row.name,
                row.requires_consent as i64,
                row.is_default as i64,
                row.created_at,
            ],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                StoreError::Conflict
            }
            other => StoreError::from(other),
        })?;
        Ok(())
    })
    .await
}

/// Delete a scope.  Callers should check that no active client uses the
/// scope before deleting.
pub async fn delete(db: &Database, name: &str) -> StoreResult<()> {
    let name = name.to_owned();
    db.with_conn(move |conn| {
        let n = conn.execute("DELETE FROM scope_definition WHERE name = ?1", [&name])?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

/// Return the set of scope names that have `requires_consent = 1`.
/// Used by the consent gate to decide whether a scope needs user approval.
pub async fn consented_names(db: &Database) -> StoreResult<std::collections::HashSet<String>> {
    db.with_conn(|conn| {
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
mod tests {
    use super::*;
    use crate::{Database, crypto::MasterKey};

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).unwrap()
    }

    #[tokio::test]
    async fn minimum_catalog_seeded_by_migration() {
        let db = fresh_db();
        let scopes = list(&db).await.unwrap();
        let names: Vec<&str> = scopes.iter().map(|s| s.name.as_str()).collect();
        for required in &["openid", "profile", "email", "offline_access"] {
            assert!(
                names.contains(required),
                "scope {required} missing from catalog"
            );
        }
    }

    #[tokio::test]
    async fn openid_does_not_require_consent() {
        let db = fresh_db();
        let openid = get(&db, "openid").await.unwrap();
        assert!(!openid.requires_consent, "openid must not require consent");
    }

    #[tokio::test]
    async fn create_and_delete_scope() {
        let db = fresh_db();
        let row = ScopeDefinitionRow {
            name: "custom:read".into(),
            requires_consent: true,
            is_default: false,
            created_at: chrono::Utc::now(),
        };
        create(&db, &row).await.unwrap();
        let fetched = get(&db, "custom:read").await.unwrap();
        assert_eq!(fetched.name, "custom:read");
        assert!(fetched.requires_consent);
        delete(&db, "custom:read").await.unwrap();
        assert!(matches!(
            get(&db, "custom:read").await,
            Err(StoreError::NotFound)
        ));
    }

    #[tokio::test]
    async fn consented_names_excludes_openid() {
        let db = fresh_db();
        let names = consented_names(&db).await.unwrap();
        assert!(
            !names.contains("openid"),
            "openid must not be in consented set"
        );
        assert!(
            names.contains("profile"),
            "profile must be in consented set"
        );
        assert!(names.contains("email"), "email must be in consented set");
    }
}
