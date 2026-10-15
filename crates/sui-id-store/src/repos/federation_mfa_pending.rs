//! RFC 096-B1 stage 9: F02, the first writer of `federation_mfa_pending`
//! (migration 0047). RFC 096 `:719-722`: an existing, already-linked
//! federated identity whose local account requires MFA enters this row
//! rather than a session.
//!
//! ## AAD binding
//!
//! `continuation_sealed` is XChaCha20-Poly1305 sealed with AAD binding
//! `id`, `provider_id`, `provider_config_version`,
//! `provider_activation_generation` and `user_id` — the same
//! reconstruct-from-the-row convention `federation_login_attempt`'s own
//! `pkce_verifier_sealed` uses (migration 0046's comment).
//!
//! **What the plaintext is, and why it is empty.** `:721-722` lists what
//! the challenge must preserve: `primary_method=Fed`, provider/link
//! identity, next target, creation/expiry, and one-time consumption.
//! Migration 0047's own table gives every one of those its own plain
//! column already — `primary_method`, the provider triple and `user_id`,
//! `next_path`, `created_at`/`expires_at`, and `status`. There is nothing
//! left for a sealed payload to *store*. What it still buys, with zero
//! plaintext bytes, is answer 2 of the dispatch's three: a promotion (stage
//! 10, F03) must *open* this column with the row's own current values as
//! AAD before it may proceed, so a row edited in place out of band —
//! `user_id` repointed to a different victim by a direct `UPDATE` that
//! bypasses this crate's own write paths, for instance — fails to open and
//! cannot be promoted. That is a tamper check on the row's own identity,
//! not storage, and an empty plaintext says so as plainly as a non-empty
//! one would: XChaCha20-Poly1305 authenticates zero-length plaintext the
//! same way it authenticates any other length, and `tests.rs` proves it
//! fails to open after mutating any one of the five bound columns.
//!
//! ## Why two binding digests, not one written twice
//!
//! `browser_binding_sha256` and `csrf_binding_sha256` are both 32-byte
//! digests of a cookie-held opaque value, both compared the same way
//! (constant-time, at consumption — stage 10). That similarity is not
//! redundancy: the two values reach a consuming request through
//! different channels, and the channel is what each one is actually
//! checking.
//!
//! `browser_binding_sha256`'s raw value arrives **automatically**, as an
//! ordinary cookie the browser attaches to every request in its jar —
//! what it proves is "the same user agent that received this cookie is
//! presenting it now," independent of what page or form sent the
//! request. `csrf_binding_sha256`'s raw value must instead arrive **in
//! the request body**: `HttpOnly` keeps script from reading its cookie,
//! so the only way the value reaches a submission is a hidden form field
//! `/admin/login/mfa` renders server-side from the cookie it already
//! holds — which a cross-site POST cannot reproduce even if the cookie
//! itself rides along automatically the way the browser binding's does.
//! Collapsing the two into one column would keep the first property and
//! lose the second.
//!
//! ## Clock
//!
//! Same convention as `federation_login_attempt::insert`:
//! [`insert_within_tx`] takes an already-sealed row, built from one
//! `DateTime<Utc>` reading by its caller (`commands::issue_federated_mfa_pending`),
//! not two separate `now()` calls — the schema's own `expires_at =
//! created_at + 300s` `CHECK` would refuse a row straddling a clock tick
//! between two reads.

use crate::errors::{StoreError, StoreResult};
use crate::models::FederationMfaPendingRow;
use crate::registry::{
    ActorRequirement, AttributeSpec, AuditEventKind, MustAttemptDescriptor, TargetRequirement,
};
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use sui_id_shared::ids::{FederationMfaPendingId, FederationProviderId, UserId};

const SELECT_COLUMNS: &str = "id, provider_id, provider_config_version, \
    provider_activation_generation, user_id, continuation_sealed, next_path, \
    browser_binding_sha256, csrf_binding_sha256, failure_count, status, created_at, expires_at";

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FederationMfaPendingRow> {
    let id_str: String = row.get(0)?;
    let provider_id_str: String = row.get(1)?;
    let user_id_str: String = row.get(4)?;
    let browser_binding_sha256: Vec<u8> = row.get(7)?;
    let csrf_binding_sha256: Vec<u8> = row.get(8)?;
    let status_str: String = row.get(10)?;
    let fixed = |v: Vec<u8>, col: usize| -> rusqlite::Result<[u8; 32]> {
        v.try_into().map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                col,
                rusqlite::types::Type::Blob,
                "not 32 bytes".into(),
            )
        })
    };
    Ok(FederationMfaPendingRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_id: provider_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_config_version: row.get(2)?,
        provider_activation_generation: row.get(3)?,
        user_id: user_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?,
        continuation_sealed: row.get(5)?,
        next_path: row.get(6)?,
        browser_binding_sha256: fixed(browser_binding_sha256, 7)?,
        csrf_binding_sha256: fixed(csrf_binding_sha256, 8)?,
        failure_count: row.get(9)?,
        status: status_str.parse().map_err(|_: StoreError| {
            rusqlite::Error::FromSqlConversionFailure(
                10,
                rusqlite::types::Type::Text,
                "unknown federation_mfa_pending status".into(),
            )
        })?,
        created_at: row.get(11)?,
        expires_at: row.get(12)?,
    })
}

/// RFC 096 `:732-733`: 300 seconds from `created_at`.
pub const PENDING_LIFETIME_SECS: i64 = 300;

/// The AAD bound into `continuation_sealed` (module doc). NUL-separated,
/// the same safety argument as `federation_login_attempt::build_aad`: a
/// UUID string and a decimal integer can each contain no NUL byte, so no
/// two distinct `(id, provider_id, version, generation, user_id)` tuples
/// collide on the same byte string.
pub fn build_aad(
    id: FederationMfaPendingId,
    provider_id: FederationProviderId,
    provider_config_version: i64,
    provider_activation_generation: i64,
    user_id: UserId,
) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(id.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_id.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_config_version.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_activation_generation.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(user_id.to_string().as_bytes());
    aad
}

/// Insert exactly one `pending` `federation_mfa_pending` row, on the
/// caller's own transaction. `status` and `failure_count` are named
/// explicitly now (stage 10a: `models::FederationMfaPendingRow`'s own
/// doc explains why) — always `'pending'`/`0`, the only state this
/// function ever constructs a row in. `primary_method` is still left to
/// the table's own `DEFAULT` ('fed'): nothing reads it back yet, and the
/// table's own `CHECK` pins it to that one value regardless.
///
/// A `FOREIGN KEY (user_id, provider_id) REFERENCES federation_link`
/// violation here (migration 0047's fix round) is this stage's "active
/// link" recheck, atomic with the insert itself — not a separate `SELECT`
/// this write could race past. Reported as [`StoreError::Conflict`], the
/// same variant every other constraint-violation recheck in this crate's
/// federation tables already uses.
pub fn insert_within_tx(
    conn: &rusqlite::Connection,
    row: &FederationMfaPendingRow,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO federation_mfa_pending( \
            id, provider_id, provider_config_version, provider_activation_generation, user_id, \
            continuation_sealed, next_path, browser_binding_sha256, csrf_binding_sha256, \
            failure_count, status, created_at, expires_at \
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            row.id.to_string(),
            row.provider_id.to_string(),
            row.provider_config_version,
            row.provider_activation_generation,
            row.user_id.to_string(),
            row.continuation_sealed,
            row.next_path,
            row.browser_binding_sha256.as_slice(),
            row.csrf_binding_sha256.as_slice(),
            row.failure_count,
            row.status.as_str(),
            row.created_at,
            row.expires_at,
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

/// RFC 096-B1 stage 10a: the first read of a `federation_mfa_pending` row
/// by its own id — the `FederatedMfaVerifier`'s binding gate needs the
/// full row (bindings, status, failure_count, expiry) to validate a
/// submission against. Returns `Ok(None)` for an unknown id rather than
/// an error: "no such row" and "a row that fails every other check" must
/// be indistinguishable to the verifier's own caller, and folding a
/// missing row into the same `Option::None` the verifier already has to
/// handle for other refusals is what makes that natural rather than
/// another case to remember.
pub async fn find_by_id(
    db: &crate::Database,
    id: FederationMfaPendingId,
) -> StoreResult<Option<FederationMfaPendingRow>> {
    let id_str = id.to_string();
    db.with_read(move |read| {
        match read
            .prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM federation_mfa_pending WHERE id = ?1"
            ))?
            .query_row([id_str], map_row)
        {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StoreError::from(e)),
        }
    })
    .await
}

/// Same as [`find_by_id`], on the caller's own transaction (RFC 096-B1
/// stage 10b, F03's own durable, authoritative re-read — the row
/// `verify`'s own earlier, read-only check is never trusted a second
/// time; F03 re-derives status/count/expiry/authority itself, inside
/// the same transaction that then writes, so there is no gap between
/// this read and F03's own guarded write for a concurrent F03 call to
/// land in).
pub fn get_within_tx(
    conn: &rusqlite::Connection,
    id: FederationMfaPendingId,
) -> StoreResult<Option<FederationMfaPendingRow>> {
    match conn
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM federation_mfa_pending WHERE id = ?1"
        ))?
        .query_row([id.to_string()], map_row)
    {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(StoreError::from(e)),
    }
}

/// What [`increment_failure_within_tx`] found and did — RFC 096 `:765-771`'s
/// own two cases for a wrong proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureIncrement {
    /// New count 1–4: still pending.
    StillPending { failure_count: i64 },
    /// New count exactly 5: terminalized to `exhausted`,
    /// `continuation_sealed` cleared by the same guarded `UPDATE`.
    Exhausted,
}

/// RFC 096 `:765-767`: "a wrong proof guarded on `status=pending`,
/// `failure_count < 5`, and unexpired unchanged authority increments by
/// one. New count 1–4 returns `RejectedStillPending`; new count exactly
/// 5 sets `status=exhausted`, destroys the continuation... and returns
/// `AttemptsExhausted`." One guarded `UPDATE`, not a read then a
/// separate write: the `WHERE status = 'pending' AND failure_count < 5`
/// clause is what makes "no sixth increment exists" true under a
/// concurrent second F03 call racing the same row (RFC 096-B1 stage 10b
/// dispatch `:55-58`'s equality-boundary test), not merely under a
/// single caller. `Ok(None)` means the guard did not hold — the row was
/// not `pending` or count was already 5 by the time this call's own
/// transaction ran; the caller re-reads to learn why (expiry/authority
/// drift already caught it, another call already exhausted it, or it
/// already promoted).
///
/// Deleting bound method ceremonies on exhaustion (RFC 096 `:768`) is
/// not done here: no code path creates a `federation_webauthn_ceremony`
/// row yet (F06, stage 11), so there is nothing to delete — stated
/// rather than silently omitted.
pub fn increment_failure_within_tx(
    conn: &rusqlite::Connection,
    id: FederationMfaPendingId,
) -> StoreResult<Option<FailureIncrement>> {
    let result = conn.query_row(
        "UPDATE federation_mfa_pending \
         SET failure_count = failure_count + 1, \
             status = CASE WHEN failure_count + 1 >= 5 THEN 'exhausted' ELSE status END, \
             continuation_sealed = CASE WHEN failure_count + 1 >= 5 THEN NULL ELSE continuation_sealed END \
         WHERE id = ?1 AND status = 'pending' AND failure_count < 5 \
         RETURNING failure_count",
        [id.to_string()],
        |r| r.get::<_, i64>(0),
    );
    match result {
        Ok(new_count) if new_count >= 5 => Ok(Some(FailureIncrement::Exhausted)),
        Ok(new_count) => Ok(Some(FailureIncrement::StillPending {
            failure_count: new_count,
        })),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(StoreError::from(e)),
    }
}

/// RFC 096 `:769-770`: "A correct proof may promote only while count is
/// 0–4." The same guard shape as [`increment_failure_within_tx`] —
/// `WHERE status = 'pending' AND failure_count < 5` — so the equality
/// boundary binds both directions identically: whichever of a wrong or
/// a correct proof's own guarded `UPDATE` serializes first wins, and
/// the loser's guard fails here exactly as it would have failed above.
/// `Ok(false)` means the guard did not hold; the caller re-reads to
/// learn why, the same as [`increment_failure_within_tx`].
pub fn complete_within_tx(
    conn: &rusqlite::Connection,
    id: FederationMfaPendingId,
) -> StoreResult<bool> {
    let n = conn.execute(
        "UPDATE federation_mfa_pending SET status = 'completed', continuation_sealed = NULL \
         WHERE id = ?1 AND status = 'pending' AND failure_count < 5",
        [id.to_string()],
    )?;
    Ok(n == 1)
}

/// RFC 096 `:740-741`, `:816`: "`Invalidated` terminalizes on authority
/// drift/expiry." Guarded on `status = 'pending'` alone — unlike the two
/// above, authority/expiry are checked by the caller *before* this is
/// reached (they are not part of the wrong/correct-proof equality
/// boundary; a drifted/expired row is invalidated regardless of
/// `failure_count`), so the only race this one guards against is a
/// concurrent call that already terminalized the row some other way.
pub fn invalidate_within_tx(
    conn: &rusqlite::Connection,
    id: FederationMfaPendingId,
) -> StoreResult<bool> {
    let n = conn.execute(
        "UPDATE federation_mfa_pending SET status = 'invalidated', continuation_sealed = NULL \
         WHERE id = ?1 AND status = 'pending'",
        [id.to_string()],
    )?;
    Ok(n == 1)
}

/// RFC 096 `:734`: terminal rows are purged after 24 hours. **Not filtered
/// on `status`**, the same choice `federation_login_attempt::purge_expired`
/// already made and for the same reason restated for this table rather
/// than assumed to carry over: `:734`'s sentence is about terminal rows,
/// but a row abandoned mid-challenge (a user who closes the tab at the
/// TOTP prompt) is never terminal either, and `expires_at` here is
/// `created_at + 300s` — a 24-hour cutoff past that is generous for an
/// abandoned row too, so one `DELETE ... WHERE expires_at < ?1` reaches
/// both kinds the same way `federation_login_attempt`'s own purge reaches
/// `pending`/`exchanging` rows this old.
///
/// `federation_webauthn_ceremony` needs no purge of its own: its composite
/// `FOREIGN KEY ... ON DELETE CASCADE` (migration 0047's fix round) means
/// this `DELETE` collects any ceremony whose parent it removes, and the
/// same `FOREIGN KEY` makes a ceremony with no live parent unrepresentable
/// in the first place (`tests_rfc096_b1_stage8.rs`'s own
/// `a_ceremony_without_a_parent_pending_row_cannot_be_stored` already
/// proves the second half; this module's own tests prove the first).
pub async fn purge_expired(db: &crate::Database, now: DateTime<Utc>) -> StoreResult<usize> {
    let cutoff = now - Duration::hours(24);
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM federation_mfa_pending WHERE expires_at < ?1",
            [cutoff],
        )?;
        Ok(n)
    })
    .await
}

/// RFC 096-B1 stage 9: the registry-declared Class-B observation that a
/// federated sign-in stopped at the local second factor. Replaces the
/// legacy, factually wrong `auth.login.password_ok_mfa_required` append
/// this branch used to make (no password was presented; for an F04-
/// provisioned user there is no password at all — RFC 096 `:723-724`
/// "never substitutes `Pwd`"). `actor`/`target` are both `Required`: unlike
/// `federation_provider.rs`'s own must-attempt descriptors (P2/F05, fired
/// before or without a resolved local account), this event only fires for
/// an existing, already-linked user — `user_id` is always known.
pub static AUDIT_MFA_REQUIRED_DESCRIPTOR: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationMfaRequired,
    name: "auth.federation.mfa_required",
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "provider_id",
        description: "internal id of the federation provider this sign-in was for",
        required: false,
    }],
};

/// RFC 096 `:826-828`'s closed reason vocabulary for
/// `auth.federation.mfa_rejected`/`auth.federation.mfa_exhausted`. A
/// plain, store-level enum rather than `sui_id_core::federated_mfa::
/// RejectedReason` — F03 lives in `sui-id-store`, which cannot name a
/// `sui-id-core` type (RFC 096 `:784-789`, the F03 seam) — but it is
/// still closed: the only caller today is stage 10b's own promotion
/// logic, converting exhaustively from the verifier's own closed enum,
/// so nothing upstream of that conversion can select a value this one
/// does not already mirror. `Webauthn` and `MethodMismatch` are
/// unreachable this stage, the same way the verifier's own
/// `RejectedReason::Webauthn`/`MethodMismatch` are (stage 10a's own
/// doc) — F06 (stage 11) is what gives either a producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaRejectionReason {
    Totp,
    Recovery,
    Webauthn,
    Malformed,
    MethodMismatch,
}

impl MfaRejectionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Totp => "totp",
            Self::Recovery => "recovery",
            Self::Webauthn => "webauthn",
            Self::Malformed => "malformed",
            Self::MethodMismatch => "method_mismatch",
        }
    }
}

/// RFC 096 `:830-831`'s closed reason vocabulary for
/// `auth.federation.mfa_invalidated`. Three of the seven are
/// unreachable this stage, named rather than silently unused:
/// `ProviderChanged` needs RFC 096-B2/M2b's durable provider version/
/// generation columns (stage 10a's own verifier already states this
/// limitation; F03 inherits it). `CeremonyInvalid` needs F06's ceremony
/// table (stage 11). `LinkChanged` has no path at all with the current
/// schema: the only way a link changes out from under a pending row is
/// `federation_mfa_pending`'s own `FOREIGN KEY ... REFERENCES
/// federation_link ON DELETE CASCADE` (migration 0047) firing on an
/// unlink — which deletes the *row itself*, not just the link, so F03's
/// own durable re-read finds no row at all (`StoreError::NotFound`,
/// outside the four outcomes entirely) rather than a row it could
/// invalidate with this reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidatedReason {
    Expired,
    ProviderChanged,
    ProviderDisabled,
    LinkChanged,
    UserInactive,
    FactorChanged,
    CeremonyInvalid,
}

impl InvalidatedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Expired => "expired",
            Self::ProviderChanged => "provider_changed",
            Self::ProviderDisabled => "provider_disabled",
            Self::LinkChanged => "link_changed",
            Self::UserInactive => "user_inactive",
            Self::FactorChanged => "factor_changed",
            Self::CeremonyInvalid => "ceremony_invalid",
        }
    }
}

/// RFC 096-B1 stage 10b, F03: a wrong/unsupported proof against a still-
/// pending row, 1–4. `:826-828`.
pub static AUDIT_MFA_REJECTED_DESCRIPTOR: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationMfaRejected,
    name: "auth.federation.mfa_rejected",
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "reason",
        description: "which closed proof-rejection reason this was: totp, recovery, webauthn, malformed, or method_mismatch",
        required: true,
    }],
};

/// RFC 096-B1 stage 10b, F03: the fifth wrong proof — the row
/// terminalizes to `exhausted` in the same transaction. `:826-828`.
pub static AUDIT_MFA_EXHAUSTED_DESCRIPTOR: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationMfaExhausted,
    name: "auth.federation.mfa_exhausted",
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "attempt_limit",
        description: "the fixed cap reached (always 5 — RFC 096 :731)",
        required: true,
    }],
};

/// RFC 096-B1 stage 10b, F03: authority drift or expiry terminalizes the
/// row without counting a wrong proof. `:826-831`.
pub static AUDIT_MFA_INVALIDATED_DESCRIPTOR: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationMfaInvalidated,
    name: "auth.federation.mfa_invalidated",
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "reason",
        description: "which closed invalidation reason this was: expired, provider_changed, provider_disabled, link_changed, user_inactive, factor_changed, or ceremony_invalid",
        required: true,
    }],
};

/// RFC 096-B1 stage 10b fix round, R1: F03's `Promoted` outcome, its own
/// name rather than a reuse of `auth.federation.signin.success` (L04/F01
/// — no second factor needed). The same split the local path already
/// has between `auth.login.success` (L01/L03) and `auth.mfa.success`
/// (L02): the class is a property of the guarantee (Class A cannot be
/// missing if the mutation happened; Class B can), so one name cannot
/// carry both without leaving an operator unable to tell which assurance
/// a row carries. `:816-817` separates "post-commit Class-B sign-in
/// success" from F03's own observations without naming the success
/// event — naming it is this fix, not an RFC amendment.
pub static AUDIT_MFA_SUCCESS_DESCRIPTOR: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationMfaSuccess,
    name: "auth.federation.mfa_success",
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "method",
            description: "the second factor: \"totp\" or \"recovery_code\"",
            required: false,
        },
        AttributeSpec {
            name: "evicted",
            description: "older sessions revoked to keep the user within the concurrent-session cap",
            required: false,
        },
    ],
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
