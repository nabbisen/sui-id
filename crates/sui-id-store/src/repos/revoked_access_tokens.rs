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

pub fn insert(db: &Database, row: &RevokedAccessTokenRow) -> StoreResult<()> {
    db.with_conn(|conn| {
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
}

pub fn is_revoked(db: &Database, jti: &str) -> StoreResult<bool> {
    db.with_conn(|conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM revoked_access_tokens WHERE jti = ?1",
            [jti],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    })
}

/// Drop entries whose underlying token has expired anyway.
pub fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "DELETE FROM revoked_access_tokens WHERE exp < ?1",
            [Utc::now()],
        )?;
        Ok(n)
    })
}
