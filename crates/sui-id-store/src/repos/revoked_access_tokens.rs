//! Deny-list of access-token JTI values.

use crate::db::Database;
use crate::errors::StoreResult;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::{ClientId, UserId};

#[derive(Debug, Clone)]
pub struct RevokedAccessTokenRow {
    pub jti: String,
    pub revoked_at: DateTime<Utc>,
    pub exp: DateTime<Utc>,
    pub revoked_by_user: Option<UserId>,
    pub revoked_by_client: Option<ClientId>,
}

pub async fn insert(db: &Database, row: &RevokedAccessTokenRow) -> StoreResult<()> {
    let row = row.clone();
    db.with_conn(move |conn| {
        // ON CONFLICT DO NOTHING — RFC 7009 specifies revocation is
        // idempotent: revoking an already-revoked token must succeed
        // silently, so we should not error on duplicate insert.
        conn.execute(
            "INSERT INTO revoked_access_tokens \
             (jti, revoked_at, exp, revoked_by_user, revoked_by_client) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(jti) DO NOTHING",
            params![
                row.jti,
                row.revoked_at,
                row.exp,
                row.revoked_by_user.map(|u| u.to_string()),
                row.revoked_by_client.map(|c| c.to_string()),
            ],
        )?;
        Ok(())
    })
    .await
}

pub async fn is_revoked(db: &Database, jti: &str) -> StoreResult<bool> {
    let jti = jti.to_owned();
    db.with_conn(move |conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM revoked_access_tokens WHERE jti = ?1",
            [jti],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    })
    .await
}

/// Drop entries whose underlying token has expired anyway.
pub async fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM revoked_access_tokens WHERE exp < ?1",
            [Utc::now()],
        )?;
        Ok(n)
    })
    .await
}
