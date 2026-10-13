//! RFC 096-B1 stage 2: durable attempt start. RFC 096 `:583-589`, the
//! `pending` row only — claiming it (`pending -> exchanging`) is stage 3.
//!
//! ## AAD binding
//!
//! `pkce_verifier_sealed` is XChaCha20-Poly1305 sealed with AAD binding
//! `id`, `provider_id`, `provider_config_version` and
//! `provider_activation_generation` (RFC 096 `:586-587`). All four are
//! already columns on this same row, so [`build_aad`] reconstructs the AAD
//! from them rather than storing it a second time. Altering any one of the
//! four after the fact — a different attempt id, a different provider, or
//! a stale config version/activation generation — changes the AAD and makes
//! the sealed verifier fail to open; see `tests.rs` for the test proving
//! this for each of the four independently, which is the actual point of
//! binding them at all.
//!
//! ## Clock
//!
//! No `SharedClock` here: `sui-id-core` (where `SharedClock` lives) depends
//! on `sui-id-store`, not the other way around, so this crate cannot name
//! that type. [`insert`] takes an already-sampled `now: DateTime<Utc>`, the
//! same convention every other timestamp-taking function in this crate
//! already uses (e.g. `sessions::count_active_for_user`). The caller reads
//! the clock exactly once and derives both `created_at` and `expires_at`
//! from that one reading — see the module's own "clock regression" note in
//! the stage 2 package for why this is where that discipline lives.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::{FederationLoginAttemptRow, FederationLoginAttemptStatus};
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use sui_id_shared::ids::{FederationLoginAttemptId, FederationProviderId};

const SELECT_COLUMNS: &str = "id, provider_id, provider_config_version, \
    provider_activation_generation, state_sha256, nonce_sha256, browser_binding_sha256, \
    pkce_verifier_sealed, exact_redirect_uri, next_path, created_at, expires_at, status, \
    claimed_at";

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FederationLoginAttemptRow> {
    let id_str: String = row.get(0)?;
    let provider_id_str: String = row.get(1)?;
    let state_sha256: Vec<u8> = row.get(4)?;
    let nonce_sha256: Vec<u8> = row.get(5)?;
    let browser_binding_sha256: Vec<u8> = row.get(6)?;
    let status_str: String = row.get(12)?;
    let fixed = |v: Vec<u8>, col: usize| -> rusqlite::Result<[u8; 32]> {
        v.try_into().map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                col,
                rusqlite::types::Type::Blob,
                "not 32 bytes".into(),
            )
        })
    };
    Ok(FederationLoginAttemptRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_id: provider_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_config_version: row.get(2)?,
        provider_activation_generation: row.get(3)?,
        state_sha256: fixed(state_sha256, 4)?,
        nonce_sha256: fixed(nonce_sha256, 5)?,
        browser_binding_sha256: fixed(browser_binding_sha256, 6)?,
        pkce_verifier_sealed: row.get(7)?,
        exact_redirect_uri: row.get(8)?,
        next_path: row.get(9)?,
        created_at: row.get(10)?,
        expires_at: row.get(11)?,
        status: status_str.parse().map_err(|_: StoreError| {
            rusqlite::Error::FromSqlConversionFailure(
                12,
                rusqlite::types::Type::Text,
                "unknown federation_login_attempt status".into(),
            )
        })?,
        claimed_at: row.get(13)?,
    })
}

/// RFC 096 `:588-589`: 600 seconds from `created_at`.
pub const ATTEMPT_LIFETIME_SECS: i64 = 600;

/// The AAD bound into `pkce_verifier_sealed` (module doc). NUL-separated:
/// safe because neither a UUID string (hex and hyphens only) nor a decimal
/// integer can contain a NUL byte, so no two distinct
/// `(id, provider_id, version, generation)` tuples can produce the same
/// byte string.
fn build_aad(
    id: FederationLoginAttemptId,
    provider_id: FederationProviderId,
    provider_config_version: i64,
    provider_activation_generation: i64,
) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(id.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_id.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_config_version.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_activation_generation.to_string().as_bytes());
    aad
}

/// Create a `pending` `federation_login_attempt` row. `pkce_verifier_plain`
/// is sealed before the write; the plaintext is never stored.
///
/// `now` is sampled by the caller exactly once and used for both
/// `created_at` and `expires_at = now + 600s` — this function does not read
/// a clock of its own, so there is nothing for it to disagree with.
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    db: &Database,
    provider_id: FederationProviderId,
    provider_config_version: i64,
    provider_activation_generation: i64,
    state_sha256: [u8; 32],
    nonce_sha256: [u8; 32],
    browser_binding_sha256: [u8; 32],
    pkce_verifier_plain: &[u8],
    exact_redirect_uri: String,
    next_path: Option<String>,
    now: DateTime<Utc>,
) -> StoreResult<FederationLoginAttemptRow> {
    let id = FederationLoginAttemptId::new();
    let aad = build_aad(
        id,
        provider_id,
        provider_config_version,
        provider_activation_generation,
    );
    // Crypto is sync; sealing before entering `with_conn` avoids holding a
    // connection across it (same shape as `federation_provider::create`).
    let pkce_verifier_sealed = crate::crypto::seal(db.key(), pkce_verifier_plain, &aad)?;

    let row = FederationLoginAttemptRow {
        id,
        provider_id,
        provider_config_version,
        provider_activation_generation,
        state_sha256,
        nonce_sha256,
        browser_binding_sha256,
        pkce_verifier_sealed,
        exact_redirect_uri,
        next_path,
        created_at: now,
        expires_at: now + Duration::seconds(ATTEMPT_LIFETIME_SECS),
        status: FederationLoginAttemptStatus::Pending,
        claimed_at: None,
    };

    let insert_row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO federation_login_attempt( \
                id, provider_id, provider_config_version, provider_activation_generation, \
                state_sha256, nonce_sha256, browser_binding_sha256, pkce_verifier_sealed, \
                exact_redirect_uri, next_path, created_at, expires_at, status, claimed_at \
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                insert_row.id.to_string(),
                insert_row.provider_id.to_string(),
                insert_row.provider_config_version,
                insert_row.provider_activation_generation,
                insert_row.state_sha256.as_slice(),
                insert_row.nonce_sha256.as_slice(),
                insert_row.browser_binding_sha256.as_slice(),
                insert_row.pkce_verifier_sealed,
                insert_row.exact_redirect_uri,
                insert_row.next_path,
                insert_row.created_at,
                insert_row.expires_at,
                insert_row.status.as_str(),
                insert_row.claimed_at,
            ],
        )?;
        Ok(())
    })
    .await?;

    Ok(row)
}

/// RFC 096-B1 stage 4: look up an attempt by its `state_sha256` — the only
/// value the callback actually has (the OAuth `state` parameter, echoed
/// back by the upstream; the caller hashes it before calling this). Read
/// path, not a write: this is the first caller of `federation_login_attempt`
/// that only reads, so it goes through [`crate::ReadConn`] like every other
/// read site in this crate, not `with_conn`.
pub async fn find_by_state_sha256(
    db: &Database,
    state_sha256: [u8; 32],
) -> StoreResult<FederationLoginAttemptRow> {
    db.with_read(move |read| {
        read.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM federation_login_attempt WHERE state_sha256 = ?1"
        ))?
        .query_row([state_sha256.as_slice()], map_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
    .await
}

/// RFC 096-B1 stage 3/6c: claim a `pending` attempt, `pending -> exchanging`,
/// single-use (RFC 096 `:590-592`, `:597-598`).
///
/// **Why this is a read, then a guarded check, then a conditional `UPDATE`
/// — not one statement folding every condition into the `WHERE` clause.**
/// `status`, `expires_at` and `created_at` are read once because the
/// dispatch asks for *distinguishable* refusals (`NotFound` / `Conflict` /
/// [`StoreError::AttemptExpired`] / [`StoreError::ClockRegression`] /
/// [`StoreError::AttemptProviderMismatch`] /
/// [`StoreError::AttemptBrowserBindingMismatch`]), and a single
/// `UPDATE ... WHERE status = 'pending' AND expires_at > ?3` would collapse
/// all of "never existed," "already claimed," and "expired" into one
/// indistinguishable zero-rows-affected outcome — correct for
/// `login_pending_mfa::consume_within_tx`'s single generic outcome, wrong
/// here. `expires_at`, `created_at`, `provider_id`,
/// `provider_config_version`, `provider_activation_generation` and
/// `browser_binding_sha256` are all immutable once inserted (never written
/// anywhere outside `insert`), so reading them separately from the one
/// thing that *does* change concurrently — `status` — introduces no race:
/// only the final `UPDATE`'s own `WHERE status = 'pending'` clause needs to
/// be atomic, because `status` is the one value two concurrent callers
/// could disagree about.
///
/// **The affected-row count is the authority for the claim itself**, exactly
/// as the dispatch states: if the `UPDATE` affects zero rows, this caller
/// did not win, whether because the row was never `pending` or because
/// another caller's `UPDATE` landed first — both report
/// [`StoreError::Conflict`], the same variant the read-time "not pending"
/// check above also uses, since both mean the same thing to a caller: this
/// attempt is not available to claim, right now, by you.
///
/// **RFC 096 `:597-598`'s match set, checked here rather than by a separate
/// post-claim step.** `expected_provider_id`/`expected_provider_config_
/// version`/`expected_provider_activation_generation` are the values the
/// caller already resolved (from the state cookie, independently of this
/// row) before calling; a mismatch against the row's own stored values
/// means this attempt does not belong to the provider/trust-generation the
/// caller is proceeding with, and the claim must refuse before it mutates
/// anything. `presented_browser_binding_sha256` is compared to the row's
/// own `browser_binding_sha256` in constant time
/// (`subtle::ConstantTimeEq`) — `:598`'s own requirement — and the one
/// variant covers both "wrong" and "absent" (the caller passes some 32-byte
/// value either way; see [`StoreError::AttemptBrowserBindingMismatch`]).
/// `ct_eq` on fixed-size 32-byte digests does not leak length, since both
/// sides are always exactly 32 bytes — there is no length to leak.
#[allow(clippy::too_many_arguments)]
pub async fn claim(
    db: &Database,
    id: FederationLoginAttemptId,
    now_at_claim: DateTime<Utc>,
    expected_provider_id: FederationProviderId,
    expected_provider_config_version: i64,
    expected_provider_activation_generation: i64,
    presented_browser_binding_sha256: [u8; 32],
) -> StoreResult<FederationLoginAttemptRow> {
    let id_str = id.to_string();
    let row = db
        .with_read({
            let id_str = id_str.clone();
            move |read| {
                read.prepare(&format!(
                    "SELECT {SELECT_COLUMNS} FROM federation_login_attempt WHERE id = ?1"
                ))?
                .query_row([id_str], map_row)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                    other => StoreError::from(other),
                })
            }
        })
        .await?;

    if row.status != FederationLoginAttemptStatus::Pending {
        return Err(StoreError::Conflict);
    }
    // Clock regression checked before expiry, and independently of it:
    // `expires_at <= now_at_claim` alone would be the wrong test under a
    // backward clock (RFC 096 `:588-589`) -- it makes the window look *less*
    // elapsed, not more, so a regressed clock could pass it when it must
    // not.
    if now_at_claim < row.created_at {
        return Err(StoreError::ClockRegression);
    }
    if row.expires_at <= now_at_claim {
        return Err(StoreError::AttemptExpired);
    }
    if row.provider_id != expected_provider_id
        || row.provider_config_version != expected_provider_config_version
        || row.provider_activation_generation != expected_provider_activation_generation
    {
        return Err(StoreError::AttemptProviderMismatch);
    }
    {
        use subtle::ConstantTimeEq;
        let binding_ok: bool = row
            .browser_binding_sha256
            .ct_eq(&presented_browser_binding_sha256)
            .into();
        if !binding_ok {
            return Err(StoreError::AttemptBrowserBindingMismatch);
        }
    }

    let affected = db
        .with_conn({
            let id_str = id_str.clone();
            move |conn| {
                Ok(conn.execute(
                    "UPDATE federation_login_attempt SET status = 'exchanging', claimed_at = ?1 \
                     WHERE id = ?2 AND status = 'pending'",
                    params![now_at_claim, id_str],
                )?)
            }
        })
        .await?;
    if affected == 0 {
        return Err(StoreError::Conflict);
    }

    // The tamper check (RFC 096-B1 stage 3, item 3): reconstruct the AAD
    // from the row's own four bound columns and open the sealed verifier.
    // If any of the four has changed since `insert` sealed it, this fails
    // -- the check this stage gets for free, not a hand-written field
    // comparison. The plaintext itself is not needed by this stage (nonce
    // consumption uses `nonce_sha256`, not the PKCE verifier) and is
    // dropped rather than returned: stage 4's token exchange is what needs
    // it, and presuming its shape here is exactly the kind of thing stage 2
    // already declined to do one layer down.
    let aad = build_aad(
        row.id,
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
    );
    drop(crate::crypto::open(
        db.key(),
        &row.pkce_verifier_sealed,
        &aad,
    )?);

    Ok(FederationLoginAttemptRow {
        status: FederationLoginAttemptStatus::Exchanging,
        claimed_at: Some(now_at_claim),
        ..row
    })
}

/// RFC 096-B1 stage 6a, RFC 096 `:604-605`: hygiene, called from the GC
/// task.
///
/// **`now` is a parameter, not read inside — checked against a seventh
/// precedent the dispatch's own survey did not name.** `sessions`,
/// `auth_codes`, `refresh_tokens`, `login_pending_mfa`, `webauthn_pending`
/// and `revoked_access_tokens` all read `Utc::now()` inside the function.
/// But `pending_settings_change::purge_expired(db, now: DateTime<Utc>)`
/// does not — it takes `now` as a parameter, the same convention this
/// module's own `insert`/`claim` already use. That seventh precedent, not
/// the six, is the one this function matches, for a reason specific to
/// this dispatch rather than a style preference: it asks for "an
/// `expires_at = now - 24h` exact-boundary case... in the shape stage 3's
/// claim boundary got," and stage 3's boundary test is deterministic only
/// because `claim` takes `now_at_claim` as a parameter. A function that
/// reads its own clock cannot be pinned to a boundary with microsecond
/// precision from a test that runs on a separate clock read, milliseconds
/// later — `gc.rs`'s own call site samples `Utc::now()` once, same
/// discipline as `insert`/`claim`'s callers.
///
/// **The predicate, derived against the actual columns, not asserted.**
/// This table has no terminal-transition timestamp — `status` changes in
/// place with no "when" recorded — so "24 hours after becoming terminal"
/// cannot be expressed directly. What the table *does* give: `expires_at`,
/// fixed at `created_at + 600s` by [`insert`] and never written again by
/// anything in this file, for every status alike. `expires_at < now - 24h`
/// is therefore a uniform rule reachable from the one timestamp that
/// exists on every row, regardless of `status`:
///
/// - `completed`/`failed`: deleted roughly 24h after the attempt's own
///   600-second window closed — not exactly 24h after the transition (no
///   such timestamp exists to measure from), but within 10 minutes of it,
///   which is the closest this schema can state the RFC's rule.
/// - `pending`/`exchanging` still around 24h later: **caught by the same
///   rule**, deliberately — RFC 096 `:587-588` already calls
///   `expires_at <= now` "expired" regardless of `status`, so a `pending`
///   or `exchanging` row this old is expired by that definition and
///   leaving it around indefinitely is not an option (the dispatch's own
///   framing). The predicate does not inspect `status` at all, so it
///   reaches every row the same way, including these.
///
/// **Boundary, matching the six neighbours' own strict `<`:**
/// `expires_at < now - 24h` — a row at *exactly* `now - 24h` survives this
/// sweep (not yet "after 24 hours", `:605`'s own wording), and is deleted
/// the next time this runs once the clock has moved past it even by one
/// tick. Mirrors `claim`'s own `expires_at <= now_at_claim` being the
/// *expired* test while this function's boundary is the *retention* test
/// — different questions, so a different inequality is not an
/// inconsistency between them.
pub async fn purge_expired(db: &Database, now: DateTime<Utc>) -> StoreResult<usize> {
    let cutoff = now - Duration::hours(24);
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM federation_login_attempt WHERE expires_at < ?1",
            [cutoff],
        )?;
        Ok(n)
    })
    .await
}

/// RFC 096-B1 stage 6c, F05: guarded `pending|exchanging -> failed`,
/// one-time (RFC 096 `:601-605`). No link, user, session or last-seen
/// mutation — that is the property F05's own manifest row names, and this
/// function's entire body is the one `UPDATE`, so there is nothing else in
/// it that could touch any of those.
///
/// One statement, unlike [`claim`]: F05 does not need [`claim`]'s
/// distinguishable refusals (`NotFound` vs "already terminal" vs
/// "expired"). Every state this `WHERE` excludes — `completed`, already
/// `failed`, or no such row at all — means the same thing to a caller
/// here: this attempt cannot be moved to `failed` by this call, right now.
/// `affected == 0` reports that uniformly as [`StoreError::Conflict`].
///
/// `now` exists only to satisfy the table's own `CHECK` — *"exactly one of
/// 'pending and never claimed' or 'non-pending and claimed' is a valid
/// row"* (migration 0046's own comment) — for the direct `pending ->
/// failed` transition (RFC 096 `:603-604`'s error-response path, which
/// never calls [`claim`] at all). `COALESCE(claimed_at, ?1)` leaves an
/// already-`exchanging` row's real claim time untouched and only fills the
/// column in for a row moving straight from `pending`.
pub async fn terminalize_failed(
    db: &Database,
    id: FederationLoginAttemptId,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let id_str = id.to_string();
    let affected = db
        .with_conn(move |conn| {
            Ok(conn.execute(
                "UPDATE federation_login_attempt \
                 SET status = 'failed', claimed_at = COALESCE(claimed_at, ?1) \
                 WHERE id = ?2 AND status IN ('pending', 'exchanging')",
                params![now, id_str],
            )?)
        })
        .await?;
    if affected == 0 {
        return Err(StoreError::Conflict);
    }
    Ok(())
}

/// RFC 096-B1 stage 7, F04: guarded `exchanging -> completed`, on the
/// caller's own transaction. Unlike [`terminalize_failed`], this is never
/// reached from `pending` directly — [`claim`] is the only path into
/// `exchanging`, and `claimed_at` is already set by the time this runs, so
/// there is no `now`/`COALESCE` here: the column is left exactly as
/// `claim` set it, and the table's own `CHECK` (claimed_at agrees with
/// status) is already satisfied before this `UPDATE` runs.
///
/// `affected == 0` means the row is not currently `exchanging` — already
/// `completed` (a second commit attempt for the same attempt), already
/// `failed` (F05 raced ahead of this transaction), or no such row at all.
/// Every one of those is the same fact to this call: this attempt cannot be
/// moved to `completed` by it, right now. Reported uniformly as
/// [`StoreError::Conflict`], the same shape `terminalize_failed` already
/// uses for its own "cannot be moved" cases.
pub fn complete_within_tx(
    conn: &rusqlite::Connection,
    id: FederationLoginAttemptId,
) -> StoreResult<()> {
    let affected = conn.execute(
        "UPDATE federation_login_attempt SET status = 'completed' \
         WHERE id = ?1 AND status = 'exchanging'",
        params![id.to_string()],
    )?;
    if affected == 0 {
        return Err(StoreError::Conflict);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
