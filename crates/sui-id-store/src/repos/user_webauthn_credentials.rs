//! Per-user WebAuthn (passkey) credentials.
//!
//! `passkey_enc` holds a serialised `webauthn_rs::prelude::Passkey`
//! sealed under the master key. The struct is opaque to sui-id and only
//! re-interpreted by webauthn-rs at authentication time. We seal it
//! because it contains the public key, signature counter, and other
//! per-credential state — none of which is, on its own, secret enough
//! to be high-value, but the conservative default is to encrypt
//! everything we can.

use crate::crypto::{open, seal};
use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::UserWebauthnCredentialRow;
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use sui_id_shared::ids::{UserId, WebauthnCredentialId};

const AAD: &[u8] = b"sui-id/user_webauthn_credentials/passkey/v1";

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserWebauthnCredentialRow> {
    let id_str: String = row.get(0)?;
    let user_id_str: String = row.get(1)?;
    Ok(UserWebauthnCredentialRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        user_id: user_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        credential_id: row.get(2)?,
        passkey_enc: row.get(3)?,
        nickname: row.get(4)?,
        created_at: row.get::<_, DateTime<Utc>>(5)?,
        last_used_at: row.get::<_, Option<DateTime<Utc>>>(6)?,
    })
}

const SELECT: &str = "SELECT id, user_id, credential_id, passkey_enc, nickname, \
                      created_at, last_used_at FROM user_webauthn_credentials";

pub async fn list_for_user(
    db: &Database,
    user_id: UserId,
) -> StoreResult<Vec<UserWebauthnCredentialRow>> {
    db.with_read(move |conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE user_id = ?1 ORDER BY created_at ASC"
        ))?;
        let rows = stmt
            .query_map([user_id.to_string()], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// Same as [`list_for_user`], for a caller that already holds a
/// transaction (RFC 094 U07: the sealed Class-A capability).
pub fn list_for_user_within_tx(
    conn: &rusqlite::Connection,
    user_id: UserId,
) -> StoreResult<Vec<UserWebauthnCredentialRow>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT} WHERE user_id = ?1 ORDER BY created_at ASC"
    ))?;
    let rows = stmt
        .query_map([user_id.to_string()], map)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub async fn count_for_user(db: &Database, user_id: UserId) -> StoreResult<usize> {
    db.with_read(move |conn| {
        let n: i64 = conn
            .prepare("SELECT COUNT(*) FROM user_webauthn_credentials WHERE user_id = ?1")?
            .query_row([user_id.to_string()], |r| r.get(0))?;
        Ok(n as usize)
    })
    .await
}

pub async fn find_by_credential_id(
    db: &Database,
    credential_id: &[u8],
) -> StoreResult<Option<UserWebauthnCredentialRow>> {
    let credential_id = credential_id.to_vec();
    db.with_read(move |conn| {
        Ok(conn
            .prepare(&format!("{SELECT} WHERE credential_id = ?1"))?
            .query_row([credential_id], map)
            .optional()?)
    })
    .await
}

/// RFC 096-B1 stage 11, F03's WebAuthn branch: one read that answers
/// both of F03's own questions about the credential the candidate
/// names, the same two-in-one shape the TOTP branch's own `totp_row`
/// read already answers for TOTP ("is there a row at all" and "what is
/// its current state"). `Ok(None)` is the WebAuthn analogue of TOTP's
/// own `FactorChanged`: the passkey was deleted entirely in the window
/// between `verify_webauthn` and F03's own transaction. `Ok(Some(blob))`
/// lets the caller compare against `expected_sealed` itself — a
/// mismatch there is "wrong proof" (counted), not `FactorChanged`,
/// mirroring Recovery's own blob-mismatch-is-just-wrong-proof
/// convention rather than TOTP's row-existence one.
pub fn current_passkey_enc_within_tx(
    conn: &rusqlite::Connection,
    id: WebauthnCredentialId,
) -> StoreResult<Option<Vec<u8>>> {
    match conn.query_row(
        "SELECT passkey_enc FROM user_webauthn_credentials WHERE id = ?1",
        [id.to_string()],
        |r| r.get(0),
    ) {
        Ok(blob) => Ok(Some(blob)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(StoreError::from(e)),
    }
}

/// RFC 096-B1 stage 11, F06: "require ... at least one WebAuthn
/// credential enrolled" before issuing a ceremony — the durable,
/// transaction-scoped twin of [`count_for_user`], mirroring
/// `authn::webauthn::start_authentication`'s own
/// `if creds.is_empty() { return Err(...) }` guard for the *local*
/// path (RFC 096-B1 stage 11 dispatch `:64`: "reuse the shape, do not
/// re-derive it").
pub fn has_any_within_tx(conn: &rusqlite::Connection, user_id: UserId) -> StoreResult<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM user_webauthn_credentials WHERE user_id = ?1",
        [user_id.to_string()],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub async fn create(
    db: &Database,
    row: &UserWebauthnCredentialRow,
    passkey_json_plain: &[u8],
) -> StoreResult<()> {
    let enc = seal(db.key(), passkey_json_plain, AAD)?;
    let row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at, last_used_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.id.to_string(),
                row.user_id.to_string(),
                row.credential_id,
                enc,
                row.nickname,
                row.created_at,
                row.last_used_at,
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

/// Seal a serialised passkey for storage, before a transaction is opened.
pub fn seal_passkey(db: &Database, passkey_json_plain: &[u8]) -> StoreResult<Vec<u8>> {
    seal(db.key(), passkey_json_plain, AAD)
}

/// [`create`] inside a caller-owned transaction, with the passkey already
/// sealed into `row.passkey_enc`.
pub fn create_within_tx(
    tx: &rusqlite::Transaction<'_>,
    row: &UserWebauthnCredentialRow,
) -> StoreResult<()> {
    tx.execute(
        "INSERT INTO user_webauthn_credentials \
         (id, user_id, credential_id, passkey_enc, nickname, created_at, last_used_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row.id.to_string(),
            row.user_id.to_string(),
            row.credential_id,
            row.passkey_enc,
            row.nickname,
            row.created_at,
            row.last_used_at,
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
}

pub async fn decrypt_passkey(
    db: &Database,
    row: &UserWebauthnCredentialRow,
) -> StoreResult<Vec<u8>> {
    open(db.key(), &row.passkey_enc, AAD)
}

/// Replace the sealed passkey blob (used after authentication when the
/// signature counter advances).
pub async fn update_passkey(
    db: &Database,
    id: WebauthnCredentialId,
    passkey_json_plain: &[u8],
) -> StoreResult<()> {
    let enc = seal(db.key(), passkey_json_plain, AAD)?;
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE user_webauthn_credentials SET passkey_enc = ?1, last_used_at = ?2 WHERE id = ?3",
            params![enc, Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }).await
}

/// RFC 096-B1 stage 11, F03's WebAuthn branch: the guarded compare-and-
/// swap on `passkey_enc`, the same shape
/// [`user_totp::swap_recovery_codes_within_tx`](super::user_totp::swap_recovery_codes_within_tx)
/// already uses for the recovery-code blob — `expected_sealed` is the
/// blob the verifier's own WebAuthn entry point read and matched the
/// completion against (its `ValidWebAuthn` candidate's own field).
/// `WHERE id = ... AND passkey_enc = ...` is a second, redundant guard
/// by the time F03 calls this: its own caller already compares
/// [`current_passkey_enc_within_tx`]'s read against `expected_sealed`
/// before reaching this point (mismatch there is a wrong proof, counted
/// via `reject_or_exhaust`, resolved *before* `complete_within_tx` runs
/// — not after, the way `complete_within_tx`'s own guard would have
/// rolled back a too-late failure here instead). Kept anyway, the same
/// defense-in-depth posture `complete_within_tx`'s own
/// `WHERE status = 'pending'` clause already takes even though its
/// callers also check status first.
pub fn swap_passkey_within_tx(
    conn: &rusqlite::Connection,
    id: WebauthnCredentialId,
    expected_sealed: &[u8],
    new_sealed: &[u8],
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let n = conn.execute(
        "UPDATE user_webauthn_credentials SET passkey_enc = ?1, last_used_at = ?2 \
         WHERE id = ?3 AND passkey_enc = ?4",
        params![new_sealed, now, id.to_string(), expected_sealed],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

pub async fn delete(db: &Database, id: WebauthnCredentialId, user_id: UserId) -> StoreResult<()> {
    // Scoped to user_id so a stray id can't delete another user's
    // credential even via a server-side path.
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM user_webauthn_credentials WHERE id = ?1 AND user_id = ?2",
            params![id.to_string(), user_id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Same as [`delete`], for a caller that already holds a transaction
/// (RFC 094 U07: the sealed Class-A capability). Unlike
/// [`user_totp::delete_within_tx`](super::user_totp::delete_within_tx),
/// `NotFound` here stays an error: every id this is called with comes
/// from [`list_for_user_within_tx`] read inside the same transaction, so
/// its absence would mean a genuine race, not a routine "nothing to
/// remove."
pub fn delete_within_tx(
    conn: &rusqlite::Connection,
    id: WebauthnCredentialId,
    user_id: UserId,
) -> StoreResult<()> {
    let n = conn.execute(
        "DELETE FROM user_webauthn_credentials WHERE id = ?1 AND user_id = ?2",
        params![id.to_string(), user_id.to_string()],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// Re-seal every `passkey_enc` row under `new_key`. Used by
/// master-key rotation. Runs inside the caller's transaction —
/// the function does not commit.
pub fn reseal_all(
    tx: &rusqlite::Transaction<'_>,
    old_key: &crate::crypto::MasterKey,
    new_key: &crate::crypto::MasterKey,
) -> StoreResult<u64> {
    let mut stmt = tx.prepare("SELECT id, passkey_enc FROM user_webauthn_credentials")?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let enc: Vec<u8> = row.get(1)?;
            Ok((id, enc))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    let mut count = 0u64;
    for (id, enc) in rows {
        let plain = crate::crypto::open(old_key, &enc, AAD)?;
        let resealed = crate::crypto::seal(new_key, &plain, AAD)?;
        tx.execute(
            "UPDATE user_webauthn_credentials SET passkey_enc = ?1 WHERE id = ?2",
            rusqlite::params![resealed, id],
        )?;
        count += 1;
    }
    Ok(count)
}

/// Rename a passkey credential (RFC 040). The `user_id` predicate
/// ensures a user can only rename their own credentials.
pub async fn update_nickname(
    db: &Database,
    credential_id: &str,
    user_id: UserId,
    new_nickname: &str,
) -> StoreResult<()> {
    let cid = credential_id.to_string();
    let nick = new_nickname.to_string();
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE user_webauthn_credentials \
             SET nickname = ?1 \
             WHERE id = ?2 AND user_id = ?3",
            rusqlite::params![nick, cid, user_id.to_string()],
        )?;
        Ok(())
    })
    .await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "user_webauthn_credentials/tests.rs"]
mod tests;
