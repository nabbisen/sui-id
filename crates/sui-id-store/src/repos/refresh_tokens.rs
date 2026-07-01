//! Refresh token storage (RFC 078: typed identifiers and RawRefreshToken).
//!
//! The plaintext token is sealed with the master key before insertion. On
//! lookup we first try the indexed `token_hash` path (O(log n)), falling
//! back to the legacy full-decrypt scan only for rows whose `token_hash`
//! is NULL (pre-migration 0019 rows not yet backfilled). The fallback is
//! removed once the backfill completes and a follow-up migration marks the
//! column NOT NULL. Plaintext tokens are returned to the API only at
//! issuance.

use crate::crypto::{open, seal};
use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::RefreshTokenRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::{
    FamilyId, RawRefreshToken, RefreshTokenHash, RefreshTokenId,
    ids::{ClientId, UserId},
};

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<RefreshTokenRow> {
    let auth_methods_json: String = row.get(8)?;
    let auth_methods = serde_json::from_str(&auth_methods_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(RefreshTokenRow {
        id: RefreshTokenId::from_stored(row.get::<_, String>(0)?),
        user_id: row.get::<_, String>(2)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        client_id: row.get::<_, String>(3)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
        })?,
        scope: row.get(4)?,
        expires_at: row.get::<_, DateTime<Utc>>(5)?,
        revoked_at: row.get::<_, Option<DateTime<Utc>>>(6)?,
        created_at: row.get::<_, DateTime<Utc>>(7)?,
        auth_methods,
        family_id: FamilyId::from_stored(row.get::<_, String>(9)?),
    })
}

const AAD: &[u8] = b"sui-id/refresh_token/v1";

/// Insert a new refresh token row.
///
/// The `token` is the plaintext value that the client will receive; it is
/// sealed with the master key before writing. The `RefreshTokenRow` itself
/// carries no plaintext field — the plaintext is only held by the caller's
/// `RawRefreshToken` value until it is delivered to the client.
pub async fn insert(
    db: &Database,
    row: &RefreshTokenRow,
    token: &RawRefreshToken,
) -> StoreResult<()> {
    let sealed = seal(db.key(), token.expose().as_bytes(), AAD)?;
    let hash = RefreshTokenHash::of(token).as_bytes().to_vec();
    let methods_json = serde_json::to_string(&row.auth_methods)?;
    let id = row.id.as_str().to_owned();
    let family_id = row.family_id.as_str().to_owned();
    let row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO refresh_tokens(id, token_enc, token_hash, user_id, client_id, scope, \
             expires_at, revoked_at, created_at, auth_methods, family_id) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                sealed,
                hash,
                row.user_id.to_string(),
                row.client_id.to_string(),
                row.scope,
                row.expires_at,
                row.revoked_at,
                row.created_at,
                methods_json,
                family_id,
            ],
        )?;
        Ok(())
    })
    .await
}

/// Look up an active token row by plaintext value.
///
/// Fast path: use the `token_hash` index for O(log n) lookup. This covers
/// all rows inserted at or after migration 0019.
///
/// Fallback path: for rows whose `token_hash` IS NULL (pre-migration rows
/// not yet backfilled by the startup task), decrypt each candidate and
/// compare in constant time. This fallback is removed once the backfill
/// is complete and a follow-up migration ensures the column is NOT NULL.
pub async fn find_active(db: &Database, token: &RawRefreshToken) -> StoreResult<RefreshTokenRow> {
    use subtle::ConstantTimeEq;

    let now = Utc::now();
    let hash = RefreshTokenHash::of(token).as_bytes().to_vec();
    // Copy the plaintext bytes for use in the fallback closure.
    let pt_bytes: Vec<u8> = token.expose().as_bytes().to_vec();

    // --- fast path: indexed lookup ---
    let fast: Option<RefreshTokenRow> = db
        .with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, token_enc, user_id, client_id, scope, expires_at, revoked_at, \
             created_at, auth_methods, family_id \
             FROM refresh_tokens \
             WHERE token_hash = ?1 AND revoked_at IS NULL AND expires_at > ?2",
            )?;
            match stmt.query_row(params![hash, now], map) {
                Ok(row) => Ok(Some(row)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(StoreError::from(e)),
            }
        })
        .await?;
    if let Some(row) = fast {
        return Ok(row);
    }

    // --- fallback: decrypt-scan for NULL token_hash rows (backfill pending) ---
    let candidates: Vec<(RefreshTokenRow, Vec<u8>)> = db
        .with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, token_enc, user_id, client_id, scope, expires_at, revoked_at, \
             created_at, auth_methods, family_id \
             FROM refresh_tokens \
             WHERE token_hash IS NULL AND revoked_at IS NULL AND expires_at > ?1",
            )?;
            let rows = stmt
                .query_map([now], |r| {
                    let row = map(r)?;
                    let enc: Vec<u8> = r.get(1)?;
                    Ok((row, enc))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;

    for (row, enc) in candidates {
        if let Ok(opened) = open(db.key(), &enc, AAD)
            && opened.ct_eq(&pt_bytes).into()
        {
            return Ok(row);
        }
    }
    Err(StoreError::NotFound)
}

pub async fn revoke(db: &Database, id: &RefreshTokenId) -> StoreResult<()> {
    let id = id.as_str().to_owned();
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), id],
        )?;
        Ok(())
    })
    .await
}

pub async fn revoke_all_for_user(db: &Database, user_id: UserId) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), user_id.to_string()],
        )?;
        Ok(n)
    })
    .await
}

/// Same as [`revoke_all_for_user`] but runs inside a caller-owned
/// transaction, so it participates in the caller's atomicity boundary.
pub fn revoke_all_for_user_within_tx(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<usize> {
    let n = tx.execute(
        "UPDATE refresh_tokens SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL",
        params![now, user_id.to_string()],
    )?;
    Ok(n)
}

pub async fn revoke_all_for_client(db: &Database, client_id: ClientId) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE client_id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), client_id.to_string()],
        )?;
        Ok(n)
    })
    .await
}

/// Delete expired refresh tokens. Retains revoked-but-unexpired rows so
/// that the theft-detection path (`find_any`) can still fire on replays
/// within the original token's lifetime.
///
/// The previous behaviour (`WHERE expires_at < ?1 OR revoked_at IS NOT
/// NULL`) was incorrect: it deleted revoked rows immediately, which meant
/// a rotated token could be garbage-collected before a replay attempt
/// reached the theft-detection branch.
pub async fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM refresh_tokens WHERE expires_at < ?1",
            [Utc::now()],
        )?;
        Ok(n)
    })
    .await
}

/// Find a refresh token row by plaintext, *including* revoked rows
/// that haven't been purged yet. Used by the theft-detection path
/// in the refresh-grant flow: a token presented at the token
/// endpoint that decrypts to a row with `revoked_at != NULL` is
/// almost certainly an attacker replaying a captured-and-already-
/// rotated token, so the caller revokes the whole family.
///
/// Returns `NotFound` for tokens that genuinely don't exist (typo,
/// or rows already purged by `purge_expired`). Returns the row
/// regardless of `revoked_at` and `expires_at` when found.
///
/// Fast path: indexed lookup via `token_hash` (no revocation /
/// expiry filter applied so that the caller sees the `revoked_at`
/// field and can trigger theft detection). Fallback: decrypt-scan
/// for NULL-hash rows not yet backfilled.
pub async fn find_any(db: &Database, token: &RawRefreshToken) -> StoreResult<RefreshTokenRow> {
    use subtle::ConstantTimeEq;

    let hash = RefreshTokenHash::of(token).as_bytes().to_vec();
    let pt_bytes: Vec<u8> = token.expose().as_bytes().to_vec();

    // --- fast path: indexed lookup (includes revoked rows) ---
    let fast: Option<RefreshTokenRow> = db
        .with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, token_enc, user_id, client_id, scope, expires_at, revoked_at, \
             created_at, auth_methods, family_id \
             FROM refresh_tokens \
             WHERE token_hash = ?1",
            )?;
            match stmt.query_row([hash], map) {
                Ok(row) => Ok(Some(row)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(StoreError::from(e)),
            }
        })
        .await?;
    if let Some(row) = fast {
        return Ok(row);
    }

    // --- fallback: decrypt-scan for NULL token_hash rows (backfill pending) ---
    let candidates: Vec<(RefreshTokenRow, Vec<u8>)> = db
        .with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, token_enc, user_id, client_id, scope, expires_at, revoked_at, \
             created_at, auth_methods, family_id \
             FROM refresh_tokens \
             WHERE token_hash IS NULL",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    let row = map(r)?;
                    let enc: Vec<u8> = r.get(1)?;
                    Ok((row, enc))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;

    for (row, enc) in candidates {
        if let Ok(opened) = open(db.key(), &enc, AAD)
            && opened.ct_eq(&pt_bytes).into()
        {
            return Ok(row);
        }
    }
    Err(StoreError::NotFound)
}

/// Revoke every active row in the given rotation family. Returns
/// the number of rows updated. Idempotent: rows already revoked
/// are not re-revoked.
pub async fn revoke_family(db: &Database, family_id: &FamilyId) -> StoreResult<usize> {
    let family_id = family_id.as_str().to_owned();
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE family_id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), family_id],
        )?;
        Ok(n)
    })
    .await
}

/// The outcome of an atomic refresh-token rotation attempt (RFC 080).
///
/// Every concurrent exchange of the same token hash receives exactly one
/// variant.  The caller matches on this to implement the grant path and
/// the theft-detection path; no raw revoke/find calls are needed in core.
#[derive(Debug)]
pub enum RotationLookup {
    /// This caller won the race: the row was active and is now atomically
    /// revoked by this call.  Proceed to issue the successor token.
    RotatedHere(RefreshTokenRow),
    /// The row exists but was already revoked — either by a prior rotation
    /// or by a concurrent winner of this exact exchange.  This is the
    /// theft-detection signal: family revocation has ALREADY been performed
    /// inside the same transaction, so the family is fully revoked by the
    /// time this variant is returned.  `family_revoked` is the number of
    /// previously-active rows that were revoked in that sweep (useful for
    /// the audit note).
    ReuseDetected {
        row: RefreshTokenRow,
        family_revoked: usize,
    },
    /// The row exists, has not been revoked, but its `expires_at` is ≤ `now`.
    Expired(RefreshTokenRow),
    /// No row with this token hash exists (or the row was already purged).
    Unknown,
}

/// Atomically attempt to rotate a refresh token (RFC 080).
///
/// Implements the rows-affected arbitration that closes the TOCTOU window
/// in the previous read-check-revoke-insert sequence:
///
/// 1. SELECT the row by hash (no filters) inside a transaction.
/// 2. If absent → `Unknown`.
/// 3. If `expires_at ≤ now` and `revoked_at IS NULL` → `Expired`.
/// 4. `UPDATE SET revoked_at = now WHERE id = ? AND revoked_at IS NULL`:
///    - rows_affected = 1 → this call won; return `RotatedHere`.
///    - rows_affected = 0 → already revoked (concurrent winner or prior
///      rotation); immediately `UPDATE … WHERE family_id = ? AND revoked_at
///      IS NULL` to revoke all remaining family members, then return
///      `ReuseDetected`.
///
/// The `expected_client` guard runs before step 4: if the presented token
/// belongs to a different client than the requester, the function returns
/// `Err(StoreError::Conflict)` without mutating any state — matching the
/// pre-existing behaviour where a client mismatch is rejected before any
/// revocation.
///
/// Security properties guaranteed by this function (RFC 080 §Security):
/// - **P1**: exactly one concurrent caller receives `RotatedHere`.
/// - **P2**: every concurrent loser receives `ReuseDetected` and the
///   family is fully revoked before that variant reaches the caller.
/// - **P3**: after any rotation chain, the per-family active-token count
///   is ≤ 1.
pub async fn begin_rotation(
    db: &Database,
    token_hash: &RefreshTokenHash,
    expected_client: &sui_id_shared::ids::ClientId,
    now: DateTime<Utc>,
) -> StoreResult<RotationLookup> {
    let hash_bytes = token_hash.as_bytes().to_vec();
    let expected_client_str = expected_client.to_string();

    db.with_tx(move |tx| {
        // Step 1: fetch the row by hash (including revoked / expired rows so
        // we can distinguish every outcome).
        let row: Option<RefreshTokenRow> = match tx.query_row(
            "SELECT id, token_enc, user_id, client_id, scope, expires_at, revoked_at,              created_at, auth_methods, family_id              FROM refresh_tokens WHERE token_hash = ?1",
            [hash_bytes.as_slice()],
            map,
        ) {
            Ok(r) => Some(r),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(StoreError::from(e)),
        };

        // Step 2: unknown token.
        let row = match row {
            None => return Ok(RotationLookup::Unknown),
            Some(r) => r,
        };

        // Client-binding check: reject without mutating if the token belongs
        // to a different client than the one presenting it.  Matches pre-RFC
        // behaviour where mismatch is rejected before any state change.
        if row.client_id.to_string() != expected_client_str {
            return Err(StoreError::Conflict);
        }

        // Step 3: expired-but-not-yet-revoked.
        if row.expires_at <= now && row.revoked_at.is_none() {
            return Ok(RotationLookup::Expired(row));
        }

        // Step 4: atomic arbitration via rows-affected guard.
        let id_str = row.id.as_str().to_owned();
        let family_str = row.family_id.as_str().to_owned();

        let affected = tx.execute(
            "UPDATE refresh_tokens SET revoked_at = ?1              WHERE id = ?2 AND revoked_at IS NULL",
            params![now, id_str],
        )?;

        if affected == 1 {
            // This caller won the race.
            Ok(RotationLookup::RotatedHere(row))
        } else {
            // Already revoked (row.revoked_at was Some, or a concurrent winner
            // just beat us).  Revoke all remaining family members in the same
            // transaction so the family is fully closed before we return.
            let family_revoked = tx.execute(
                "UPDATE refresh_tokens SET revoked_at = ?1                  WHERE family_id = ?2 AND revoked_at IS NULL",
                params![now, family_str],
            )?;
            Ok(RotationLookup::ReuseDetected {
                row,
                family_revoked,
            })
        }
    })
    .await
}

/// Re-seal every `token_enc` row under `new_key`. Used by
/// master-key rotation. Runs inside the caller's transaction —
/// the function does not commit.
pub fn reseal_all(
    tx: &rusqlite::Transaction<'_>,
    old_key: &crate::crypto::MasterKey,
    new_key: &crate::crypto::MasterKey,
) -> StoreResult<u64> {
    let mut stmt = tx.prepare("SELECT id, token_enc FROM refresh_tokens")?;
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
            "UPDATE refresh_tokens SET token_enc = ?1 WHERE id = ?2",
            rusqlite::params![resealed, id],
        )?;
        count += 1;
    }
    Ok(count)
}

/// Backfill `token_hash` for rows that predate migration 0019 (where the
/// column did not yet exist). Called once at startup from a
/// `tokio::spawn` task; the system is correct before backfill completes
/// because `find_active` / `find_any` fall back to the decrypt-scan path
/// for rows with `token_hash IS NULL`.
///
/// Error policy: if a row's `token_enc` does not decrypt (e.g. from a
/// partial key-rotation), the row is skipped with a warning. The row
/// remains un-backfilled and will continue to be covered by the fallback
/// scan, which also fails to decrypt it and skips it silently — so the
/// behaviour is unchanged.
///
/// Returns the number of rows successfully backfilled.
pub async fn backfill_token_hashes(db: &Database) -> StoreResult<usize> {
    let rows_to_fill: Vec<(String, Vec<u8>)> = db
        .with_conn(move |conn| {
            let mut stmt =
                conn.prepare("SELECT id, token_enc FROM refresh_tokens WHERE token_hash IS NULL")?;
            let rows = stmt
                .query_map([], |r| {
                    let id: String = r.get(0)?;
                    let enc: Vec<u8> = r.get(1)?;
                    Ok((id, enc))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;

    let mut count = 0usize;
    for (id, enc) in rows_to_fill {
        let id_for_log = id.clone();
        let plain = match open(db.key(), &enc, AAD) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    id = %id_for_log,
                    error = %e,
                    "refresh_token backfill: failed to decrypt row; skipping"
                );
                continue;
            }
        };
        let plain_str = match std::str::from_utf8(&plain) {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!(id = %id_for_log, "refresh_token backfill: non-UTF-8 plaintext; skipping");
                continue;
            }
        };
        // Compute hash via the same path as insert: wrap plaintext in
        // RawRefreshToken::from_untrusted, then RefreshTokenHash::of.
        let hash = RefreshTokenHash::of(&RawRefreshToken::from_untrusted(plain_str.to_owned()))
            .as_bytes()
            .to_vec();
        match db
            .with_conn(move |conn| {
                conn.execute(
                "UPDATE refresh_tokens SET token_hash = ?1 WHERE id = ?2 AND token_hash IS NULL",
                rusqlite::params![hash, id],
            )?;
                Ok(())
            })
            .await
        {
            Ok(()) => count += 1,
            Err(e) => {
                tracing::warn!(id = %id_for_log, error = %e, "refresh_token backfill: write failed; skipping");
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests;
