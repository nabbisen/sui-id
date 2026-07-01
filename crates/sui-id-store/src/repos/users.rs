//! User CRUD.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::UserRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::UserId;

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserRow> {
    let user_uuid_str: String = row.get(8)?;
    // Migration 0004 added `user_uuid` with `DEFAULT ''` and immediately
    // backfilled all existing rows. In a correctly-migrated database this
    // string is never empty. If it were empty (e.g. from a direct SQL write
    // that bypassed the application), `Uuid::parse_str("")` would return an
    // opaque conversion error that would surface as a 500 with no actionable
    // message. We handle it explicitly: emit a warning and use `Uuid::nil()`
    // so that the row can still be read (the user can log in; WebAuthn will
    // fail for this row until the operator repairs the value). A future
    // migration will add `CHECK (user_uuid <> '')` once the safe parent-table
    // rebuild strategy is available.
    let user_uuid = if user_uuid_str.is_empty() {
        tracing::warn!(
            "users row has empty user_uuid — using nil UUID as fallback; \
             repair with: UPDATE users SET user_uuid = lower(hex(randomblob(4))) || '-' || \
             lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || \
             substr('89ab', 1+(abs(random())%4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || \
             lower(hex(randomblob(6))) WHERE user_uuid = ''"
        );
        uuid::Uuid::nil()
    } else {
        uuid::Uuid::parse_str(&user_uuid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
        })?
    };
    Ok(UserRow {
        id: row.get::<_, String>(0)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        is_admin: row.get::<_, i64>(3)? != 0,
        // RFC 071: read role column; fall back to is_admin for rows
        // that predate migration 0027 (role will be '' or NULL in that case).
        role: row
            .get::<_, Option<String>>(15)?
            .as_deref()
            .and_then(crate::models::Role::from_db_str)
            .unwrap_or_else(|| {
                if row.get::<_, i64>(3).unwrap_or(0) != 0 {
                    crate::models::Role::Admin
                } else {
                    crate::models::Role::User
                }
            }),
        is_disabled: row.get::<_, i64>(4)? != 0,
        is_deleted: row.get::<_, i64>(5)? != 0,
        user_uuid,
        created_at: row.get::<_, DateTime<Utc>>(6)?,
        updated_at: row.get::<_, DateTime<Utc>>(7)?,
        failed_login_count: row.get::<_, i64>(9)?,
        locked_until: row.get::<_, Option<DateTime<Utc>>>(10)?,
        email: row.get::<_, Option<String>>(11)?,
        preferred_lang: row.get::<_, Option<String>>(12)?,
        email_normalized: row.get::<_, Option<String>>(13)?,
        email_verified_at: row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(14)?,
        last_login_at: row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(16)?,
    })
}

const SELECT_USER: &str = "SELECT id, username, display_name, is_admin, is_disabled, \
                           is_deleted, created_at, updated_at, user_uuid, \
                           failed_login_count, locked_until, email, preferred_lang, \
                           email_normalized, email_verified_at, role, last_login_at \
                           FROM users";

pub async fn create(db: &Database, user: &UserRow) -> StoreResult<()> {
    let email_normalized = user.email.as_deref().map(sui_id_shared::normalize_email);
    let user = user.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO users(id, username, display_name, is_admin, role, is_disabled, is_deleted, \
                                created_at, updated_at, user_uuid, \
                                failed_login_count, locked_until, email, preferred_lang, \
                                email_normalized) \
             VALUES(?1, ?2, ?3, ?4, ?15, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                user.id.to_string(),
                user.username,
                user.display_name,
                user.is_admin as i64,
                user.is_disabled as i64,
                user.is_deleted as i64,
                user.created_at,
                user.updated_at,
                user.user_uuid.to_string(),
                user.failed_login_count,
                user.locked_until,
                user.email,
                user.preferred_lang,
                email_normalized,
                user.role.as_str(),
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
    }).await
}

/// Update a user's preferred UI language. `lang` is a BCP-47 tag
/// or `None` to clear ("no preference"). Application-layer
/// validation should ensure the tag is one of `Locale::ALL` before
/// calling.
pub async fn set_preferred_lang(
    db: &Database,
    id: UserId,
    lang: Option<&str>,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let lang = lang.map(str::to_owned);
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE users SET preferred_lang = ?1, updated_at = ?2 WHERE id = ?3",
            params![lang, now, id.to_string()],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

pub async fn get(db: &Database, id: UserId) -> StoreResult<UserRow> {
    db.with_conn(move |conn| {
        conn.query_row(
            &format!("{SELECT_USER} WHERE id = ?1"),
            [id.to_string()],
            map_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
    .await
}

pub async fn find_by_username(db: &Database, username: &str) -> StoreResult<UserRow> {
    let username = username.to_owned();
    db.with_conn(move |conn| {
        conn.query_row(
            &format!("{SELECT_USER} WHERE username = ?1"),
            [username],
            map_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
    .await
}

/// Find a user by normalised email address. The caller should pass a
/// value produced by `sui_id_shared::normalize_email`; this function
/// operates on the `email_normalized` index column (migration 0020)
/// so the lookup is O(log n) and case-insensitive.
///
/// Returns `Ok(None)` when no user matches.
pub async fn find_by_email_normalized(
    db: &Database,
    normalized: &str,
) -> StoreResult<Option<UserRow>> {
    let normalized = normalized.to_owned();
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&format!("{SELECT_USER} WHERE email_normalized = ?1"))?;
        let res = stmt.query_row([normalized], map_row);
        match res {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    })
    .await
}

/// Find a user by email address, normalising the input automatically.
/// Thin wrapper over `find_by_email_normalized` for callers that hold
/// the original-case address.
pub async fn find_by_email(db: &Database, email: &str) -> StoreResult<Option<UserRow>> {
    find_by_email_normalized(db, &sui_id_shared::normalize_email(email)).await
}

/// Like `get` but returns `Ok(None)` instead of `Err(NotFound)`.
/// Convenience for callers (notably the post-password-reset
/// notification path) that legitimately want to no-op on a missing
/// row instead of treating it as an error.
pub async fn find_by_id_opt(db: &Database, id: UserId) -> StoreResult<Option<UserRow>> {
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&format!("{SELECT_USER} WHERE id = ?1"))?;
        let res = stmt.query_row([id.to_string()], map_row);
        match res {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    })
    .await
}

pub async fn list(db: &Database) -> StoreResult<Vec<UserRow>> {
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&format!("{SELECT_USER} ORDER BY created_at ASC"))?;
        let rows = stmt
            .query_map([], map_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// Toggle the `is_disabled` flag (suspend / un-suspend).
pub async fn set_disabled(db: &Database, id: UserId, disabled: bool) -> StoreResult<()> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE users SET is_disabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![disabled as i64, Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Soft-delete a user. Hard delete is intentionally not exposed at this
/// layer: it would orphan audit-log references.
pub async fn soft_delete(db: &Database, id: UserId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE users SET is_deleted = 1, is_disabled = 1, updated_at = ?1 WHERE id = ?2",
            params![Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Increment the user's consecutive-failure counter and (when the
/// caller decides the lock window applies) stamp `locked_until`.
/// Returns the new failure count.
///
/// `lock_until` is the wall-clock time before which the account is
/// refused. `None` means "increment the counter but don't lock yet"
/// — used at low failure counts where we want to count but not yet
/// punish. The decision is intentionally outside this function so
/// that the `sui_id_core` layer can choose the backoff curve.
pub async fn record_login_failure(
    db: &Database,
    id: UserId,
    lock_until: Option<DateTime<Utc>>,
) -> StoreResult<i64> {
    db.with_conn(move |conn| {
        let tx = conn.unchecked_transaction()?;
        let count: i64 = tx
            .query_row(
                "SELECT failed_login_count FROM users WHERE id = ?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })?;
        let new_count = count + 1;
        tx.execute(
            "UPDATE users SET failed_login_count = ?1, locked_until = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_count, lock_until, Utc::now(), id.to_string()],
        )?;
        tx.commit()?;
        Ok(new_count)
    }).await
}

/// Reset the user's failure counter and clear any active lock.
/// Called on a successful password verification.
pub async fn clear_lockout(db: &Database, id: UserId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE users SET failed_login_count = 0, locked_until = NULL, updated_at = ?1 WHERE id = ?2",
            params![Utc::now(), id.to_string()],
        )?;
        Ok(())
    }).await
}

/// Admin-initiated unlock: reset both fields without requiring a
/// successful password check. Used by `sui-id admin unlock-user`.
pub async fn admin_unlock(db: &Database, id: UserId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE users SET failed_login_count = 0, locked_until = NULL, updated_at = ?1 WHERE id = ?2",
            params![Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }).await
}

/// Update a user's email address. Writes both `email` (original case)
/// and `email_normalized` (via `sui_id_shared::normalize_email`) in
/// the same statement so the two columns stay in sync.
///
/// Pass `None` to clear the email from the account.
pub async fn update_email(
    db: &Database,
    id: UserId,
    email: Option<&str>,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let normalized = email.map(sui_id_shared::normalize_email);
    let email = email.map(str::to_owned);
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE users SET email = ?1, email_normalized = ?2, updated_at = ?3 WHERE id = ?4",
            params![email, normalized, now, id.to_string()],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

/// Batch-resolve user IDs to usernames (RFC 043 dashboard).
/// Unknown IDs are silently skipped.
pub async fn resolve_usernames(
    db: &Database,
    ids: &[UserId],
) -> StoreResult<std::collections::HashMap<UserId, String>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "SELECT id, username FROM users WHERE id IN ({})",
        placeholders.join(", ")
    );
    let id_strings: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<rusqlite::types::Value> = id_strings
            .iter()
            .map(|s| rusqlite::types::Value::Text(s.clone()))
            .collect();
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let id_str: String = row.get(0)?;
                let username: String = row.get(1)?;
                Ok((id_str, username))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut map = std::collections::HashMap::new();
        for (id_str, username) in rows {
            if let Ok(uid) = id_str.parse::<UserId>() {
                map.insert(uid, username);
            }
        }
        Ok(map)
    })
    .await
}

/// RFC 073: Count admin users (role = 'admin' OR is_admin = 1, since the
/// role column may not yet exist) who have NEITHER an enabled TOTP secret
/// NOR any WebAuthn credential. A user with no MFA factor at all is the
/// one we want to surface in the dashboard action items.
pub async fn count_admins_without_mfa(db: &Database) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users u \
             WHERE u.is_admin = 1 \
               AND u.is_disabled = 0 \
               AND u.is_deleted = 0 \
               AND NOT EXISTS (\
                   SELECT 1 FROM user_totp t \
                   WHERE t.user_id = u.id AND t.enabled = 1\
               ) \
               AND NOT EXISTS (\
                   SELECT 1 FROM user_webauthn_credentials c \
                   WHERE c.user_id = u.id\
               )",
            [],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    })
    .await
}

/// RFC 073: True if the user has at least one MFA factor enrolled
/// (enabled TOTP secret OR any WebAuthn credential).
pub async fn has_mfa(db: &Database, user_id: &UserId) -> StoreResult<bool> {
    let uid = user_id.to_string();
    db.with_conn(move |conn| {
        let n: i64 = conn.query_row(
            "SELECT \
                (SELECT COUNT(*) FROM user_totp WHERE user_id = ?1 AND enabled = 1) \
              + (SELECT COUNT(*) FROM user_webauthn_credentials WHERE user_id = ?1)",
            params![uid],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    })
    .await
}

/// RFC 071: Change a user's role. Writes both `role` (new) and `is_admin`
/// (compat shim) so old code that still reads `is_admin` continues to work
/// until migration 0029 drops the column.
pub async fn set_role(
    db: &Database,
    user_id: &UserId,
    role: crate::models::Role,
) -> StoreResult<()> {
    let uid = user_id.to_string();
    let role_str = role.as_str().to_owned();
    let is_admin_val = role.is_admin() as i64;
    db.with_conn(move |conn| {
        let rows = conn.execute(
            "UPDATE users SET role = ?2, is_admin = ?3, updated_at = datetime('now') \
             WHERE id = ?1 AND is_deleted = 0",
            params![uid, role_str, is_admin_val],
        )?;
        if rows == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// RFC 071: Count non-deleted users whose role = 'admin'.
/// Used by the last-admin safeguard before a demotion is permitted.
pub async fn count_admins(db: &Database) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_deleted = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    })
    .await
}

/// RFC 074: record a successful login timestamp.
/// Called by the session-creation path after password / passkey / TOTP
/// verification succeeds. Best-effort — a failed write must never abort login.
pub async fn set_last_login(
    db: &Database,
    user_id: &UserId,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<()> {
    let uid = user_id.to_string();
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE users SET last_login_at = ?2 WHERE id = ?1",
            rusqlite::params![uid, now],
        )?;
        Ok(())
    })
    .await
}
