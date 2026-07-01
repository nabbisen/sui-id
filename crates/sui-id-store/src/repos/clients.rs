//! Client (relying party) CRUD.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::ClientRow;
use crate::repos::json_util::require_valid_json;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::ClientId;

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClientRow> {
    let uris_json: String = row.get(4)?;
    let redirect_uris: Vec<String> = serde_json::from_str(&uris_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let allowed_scopes: String = row.get(7)?;
    let post_logout_json: String = row.get(8)?;
    let post_logout_redirect_uris: Vec<String> =
        serde_json::from_str(&post_logout_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
        })?;
    let consent_policy_str: String = row.get(11).unwrap_or_else(|_| "none".to_string());
    let registered_via_str: String = row.get(12).unwrap_or_else(|_| "admin".to_string());
    Ok(ClientRow {
        id: row.get::<_, String>(0)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        confidential: row.get::<_, i64>(2)? != 0,
        secret_hash: row.get(3)?,
        redirect_uris,
        allowed_scopes,
        post_logout_redirect_uris,
        is_disabled: row.get::<_, i64>(5)? != 0,
        is_deleted: row.get::<_, i64>(6)? != 0,
        consent_policy: crate::models::ConsentPolicy::parse(&consent_policy_str),
        registered_via: crate::models::RegistrationSource::parse(&registered_via_str),
        logo_uri: row.get(13)?,
        homepage_uri: row.get(14)?,
        privacy_policy_uri: row.get(15)?,
        tos_uri: row.get(16)?,
        created_at: row.get::<_, DateTime<Utc>>(9)?,
        updated_at: row.get::<_, DateTime<Utc>>(10)?,
    })
}

// Column order in SELECT: id, name, confidential, secret_hash, redirect_uris,
//                         is_disabled, is_deleted, allowed_scopes,
//                         post_logout_redirect_uris, created_at, updated_at.
const SELECT: &str = "SELECT id, name, confidential, secret_hash, redirect_uris, \
                      is_disabled, is_deleted, allowed_scopes, \
                      post_logout_redirect_uris, created_at, updated_at, consent_policy, \
                      registered_via, logo_uri, homepage_uri, privacy_policy_uri, tos_uri \
                      FROM clients";

pub async fn create(db: &Database, c: &ClientRow) -> StoreResult<()> {
    let uris = serde_json::to_string(&c.redirect_uris)?;
    let post_logout = serde_json::to_string(&c.post_logout_redirect_uris)?;
    // Pre-condition: validate that the serialised JSON round-trips correctly
    // before writing, so a future read cannot encounter corrupt JSON.
    require_valid_json::<Vec<String>>(&uris, "clients.redirect_uris")?;
    require_valid_json::<Vec<String>>(&post_logout, "clients.post_logout_redirect_uris")?;
    let c = c.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO clients(id, name, confidential, secret_hash, redirect_uris, \
                                 is_disabled, is_deleted, allowed_scopes, \
                                 post_logout_redirect_uris, created_at, updated_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                c.id.to_string(),
                c.name,
                c.confidential as i64,
                c.secret_hash,
                uris,
                c.is_disabled as i64,
                c.is_deleted as i64,
                c.allowed_scopes,
                post_logout,
                c.created_at,
                c.updated_at,
            ],
        )?;
        Ok(())
    })
    .await
}

pub async fn get(db: &Database, id: ClientId) -> StoreResult<ClientRow> {
    db.with_conn(move |conn| {
        conn.query_row(&format!("{SELECT} WHERE id = ?1"), [id.to_string()], map)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })
    })
    .await
}

pub async fn list(db: &Database) -> StoreResult<Vec<ClientRow>> {
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY created_at ASC"))?;
        let rows = stmt.query_map([], map)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

pub async fn update_basic(
    db: &Database,
    id: ClientId,
    name: Option<&str>,
    redirect_uris: Option<&[String]>,
) -> StoreResult<()> {
    let name = name.map(str::to_owned);
    let redirect_uris = redirect_uris.map(<[_]>::to_vec);
    db.with_conn(move |conn| {
        // Read current row to merge new values.
        let current: ClientRow = conn
            .query_row(&format!("{SELECT} WHERE id = ?1"), [id.to_string()], map)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })?;
        let new_name = name.as_deref().unwrap_or(&current.name);
        let new_uris = redirect_uris.unwrap_or(current.redirect_uris.clone());
        let uris_json = serde_json::to_string(&new_uris)?;
        require_valid_json::<Vec<String>>(&uris_json, "clients.redirect_uris")?;
        conn.execute(
            "UPDATE clients SET name = ?1, redirect_uris = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_name, uris_json, Utc::now(), id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Replace the `allowed_scopes` policy for a client.
pub async fn set_allowed_scopes(db: &Database, id: ClientId, scopes: &str) -> StoreResult<()> {
    let scopes = scopes.to_owned();
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE clients SET allowed_scopes = ?1, updated_at = ?2 WHERE id = ?3",
            params![scopes, Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Replace the `post_logout_redirect_uris` list for a client.
pub async fn set_post_logout_redirect_uris(
    db: &Database,
    id: ClientId,
    uris: &[String],
) -> StoreResult<()> {
    let json = serde_json::to_string(uris)?;
    require_valid_json::<Vec<String>>(&json, "clients.post_logout_redirect_uris")?;
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE clients SET post_logout_redirect_uris = ?1, updated_at = ?2 WHERE id = ?3",
            params![json, Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

pub async fn set_disabled(db: &Database, id: ClientId, disabled: bool) -> StoreResult<()> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE clients SET is_disabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![disabled as i64, Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

pub async fn soft_delete(db: &Database, id: ClientId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE clients SET is_deleted = 1, is_disabled = 1, updated_at = ?1 WHERE id = ?2",
            params![Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Patch a client's `secret_hash` to a caller-supplied value.
/// Used **only** by dev mode to give a confidential client a
/// predictable secret instead of the auto-generated one. Not
/// exposed in the production HTTP path.
pub async fn set_dev_secret_hash(
    db: &Database,
    id: ClientId,
    new_hash: Option<&str>,
) -> StoreResult<()> {
    let new_hash = new_hash.map(str::to_owned);
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE clients SET secret_hash = ?1 WHERE id = ?2",
            params![new_hash, id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Update the consent policy for a client (RFC 038).
pub async fn update_consent_policy(
    db: &Database,
    id: ClientId,
    policy: crate::models::ConsentPolicy,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE clients SET consent_policy = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![policy.as_str(), now, id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Update the client secret hash (RFC 047 — secret rotation).
pub async fn set_secret_hash(
    db: &Database,
    id: ClientId,
    hash: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<()> {
    let h = hash.map(str::to_owned);
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE clients SET secret_hash = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![h, now, id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// RFC 008: update a client's application-identity fields.
///
/// Validates that any non-None URI is HTTPS (or http://localhost).  Returns
/// `StoreError::InvalidData` if any URL fails the check (P6).
pub async fn update_app_identity(
    db: &Database,
    id: ClientId,
    logo_uri: Option<String>,
    homepage_uri: Option<String>,
    privacy_policy_uri: Option<String>,
    tos_uri: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<()> {
    for uri in [&logo_uri, &homepage_uri, &privacy_policy_uri, &tos_uri]
        .into_iter()
        .flatten()
    {
        if !is_valid_app_uri(uri) {
            return Err(StoreError::InvalidData(format!(
                "application-identity URI must be HTTPS (or http://localhost): {uri}"
            )));
        }
    }
    let id_str = id.to_string();
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE clients SET logo_uri = ?1, homepage_uri = ?2, \
             privacy_policy_uri = ?3, tos_uri = ?4, updated_at = ?5 WHERE id = ?6",
            params![
                logo_uri,
                homepage_uri,
                privacy_policy_uri,
                tos_uri,
                now,
                id_str
            ],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

/// Update the `registered_via` field on a client row.
pub async fn set_registered_via(
    db: &Database,
    id: ClientId,
    via: crate::models::RegistrationSource,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<()> {
    let id_str = id.to_string();
    let via_str = via.as_str().to_owned();
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE clients SET registered_via = ?1, updated_at = ?2 WHERE id = ?3",
            params![via_str, now, id_str],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

/// Returns true when the URI is HTTPS or http://localhost (P6).
pub fn is_valid_app_uri(uri: &str) -> bool {
    if uri.starts_with("https://") {
        return true;
    }
    // Allow http://localhost for development registrations.
    if uri.starts_with("http://localhost") || uri.starts_with("http://127.0.0.1") {
        return true;
    }
    false
}
