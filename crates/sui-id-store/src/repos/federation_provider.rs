//! `federation_provider` repository — upstream OIDC IdP configurations
//! (RFC 004).
//!
//! Provider secrets are stored encrypted with the master key using
//! XChaCha20-Poly1305 (AAD = `"federation_provider.client_secret"`).

use crate::{
    Database,
    crypto::{self, MasterKey},
    errors::{StoreError, StoreResult},
    models::{FederationProviderRow, ProvisionMode, ValidationStatus},
    registry::{
        ActorRequirement, AttributeSpec, AuditEventKind, MustAttemptDescriptor, TargetRequirement,
    },
};
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::FederationProviderId;

pub const AAD: &[u8] = b"federation_provider.client_secret";

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<FederationProviderRow> {
    let id_str: String = row.get(0)?;
    let id = id_str.parse::<FederationProviderId>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let provision_mode_str: String = row.get(7)?;
    let provision_mode = ProvisionMode::try_parse(&provision_mode_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(FederationProviderRow {
        id,
        slug: row.get(1)?,
        display_name: row.get(2)?,
        issuer: row.get(3)?,
        client_id: row.get(4)?,
        client_secret_enc: row.get(5)?,
        scopes: row.get(6)?,
        provision_mode,
        enabled: row.get::<_, i64>(8)? != 0,
        allowed_origins: row.get(9)?,
        config_version: row.get(10)?,
        activation_generation: row.get(11)?,
        validation_status: ValidationStatus::parse(&row.get::<_, String>(12)?),
        evidence_config_version: row.get(13)?,
        evidence_activation_generation: row.get(14)?,
        evidence_policy_digest: row.get(15)?,
        evidence_observed_at: row.get(16)?,
        evidence_metadata_fingerprint: row.get(17)?,
        evidence_jwks_fingerprint: row.get(18)?,
        created_at: row.get(19)?,
        updated_at: row.get(20)?,
        id_token_algs: row.get(21)?,
    })
}

const SELECT: &str = "SELECT id, slug, display_name, issuer, client_id, client_secret_enc, \
     scopes, provision_mode, enabled, allowed_origins, \
     config_version, activation_generation, validation_status, \
     evidence_config_version, evidence_activation_generation, evidence_policy_digest, \
     evidence_observed_at, evidence_metadata_fingerprint, evidence_jwks_fingerprint, \
     created_at, updated_at, id_token_algs \
     FROM federation_provider";

/// List all providers ordered by slug.
pub async fn list(db: &Database) -> StoreResult<Vec<FederationProviderRow>> {
    db.with_read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY slug ASC"))?;
        let rows = stmt.query_map([], map)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::from)
    })
    .await
}

/// List only enabled providers (for the login screen).
pub async fn list_enabled(db: &Database) -> StoreResult<Vec<FederationProviderRow>> {
    db.with_read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE enabled = 1 ORDER BY slug ASC"))?;
        let rows = stmt.query_map([], map)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::from)
    })
    .await
}

/// Fetch a provider by id.
pub async fn get(db: &Database, id: FederationProviderId) -> StoreResult<FederationProviderRow> {
    let id_str = id.to_string();
    db.with_read(move |conn| {
        conn.prepare(&format!("{SELECT} WHERE id = ?1"))?
            .query_row([&id_str], map)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })
    })
    .await
}

/// Durable `enabled` re-read, on the caller's own transaction (RFC
/// 096-B1 stage 10b, F03's own authority recheck). `NotFound` if the
/// provider no longer exists at all — the same signal a disabled
/// provider's own absence would give, since both mean "this identity's
/// authority is gone."
pub fn is_enabled_within_tx(
    conn: &rusqlite::Connection,
    id: FederationProviderId,
) -> StoreResult<bool> {
    conn.query_row(
        "SELECT enabled FROM federation_provider WHERE id = ?1",
        [id.to_string()],
        |r| r.get::<_, i64>(0),
    )
    .map(|v| v != 0)
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
        other => StoreError::from(other),
    })
}

/// RFC 096-B2 S2b: the fresh, same-transaction re-read C23 and C25 build
/// their comparison and guard on — the row cannot change between this
/// read and either command's own guarded `UPDATE` because both run in
/// the same transaction.
pub fn get_within_tx(
    tx: &rusqlite::Connection,
    id: FederationProviderId,
) -> StoreResult<FederationProviderRow> {
    tx.prepare(&format!("{SELECT} WHERE id = ?1"))?
        .query_row([id.to_string()], map)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
}

/// Fetch a provider by slug.
pub async fn get_by_slug(db: &Database, slug: &str) -> StoreResult<FederationProviderRow> {
    let slug = slug.to_owned();
    db.with_read(move |conn| {
        conn.prepare(&format!("{SELECT} WHERE slug = ?1"))?
            .query_row([&slug], map)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })
    })
    .await
}

/// Decrypt the client secret stored in `client_secret_enc`.
/// Returns `None` if the provider has no secret (public client).
/// Call this outside any `with_conn` closure — crypto happens synchronously.
pub fn decrypt_secret(key: &MasterKey, row: &FederationProviderRow) -> StoreResult<Option<String>> {
    match &row.client_secret_enc {
        None => Ok(None),
        Some(enc) => {
            let plain = crypto::open(key, enc, AAD)?;
            Ok(Some(
                String::from_utf8(plain).map_err(|_| StoreError::Crypto)?,
            ))
        }
    }
}

/// Create a provider.  `client_secret_plain` is encrypted before writing.
///
/// Cycle J stage 10 (fix round): no production caller (`create_within_tx`,
/// below, is C16's own path -- its doc already says so). Kept behind
/// `test-support`: 21 callers across `sui-id-core`, `sui-id-store`'s own
/// test runners, and `sui-id`'s e2e suites, all building a provider row
/// to seed a fixture, none relying on this form's own sealing over
/// `create_within_tx`'s pre-sealed one.
#[cfg(any(test, feature = "test-support"))]
pub async fn create(
    db: &Database,
    row: &FederationProviderRow,
    client_secret_plain: Option<&str>,
) -> StoreResult<()> {
    // Encrypt before entering with_conn (crypto is sync, no borrow issues).
    let enc: Option<Vec<u8>> = match client_secret_plain {
        Some(s) => Some(crypto::seal(db.key(), s.as_bytes(), AAD)?),
        None => None,
    };
    let row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO federation_provider \
             (id, slug, display_name, issuer, client_id, client_secret_enc, \
              scopes, provision_mode, enabled, allowed_origins, created_at, updated_at, \
              id_token_algs) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                row.id.to_string(),
                row.slug,
                row.display_name,
                row.issuer,
                row.client_id,
                enc,
                row.scopes,
                row.provision_mode.as_str(),
                row.enabled as i64,
                row.allowed_origins,
                row.created_at,
                row.updated_at,
                // RFC 096-B2 S2a: written as the fixture states it, so a
                // fixture that records algorithms is not silently NULL.
                row.id_token_algs,
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

/// RFC 096-B2 S2a Part C: exactly what C16 (create) supplies. **No
/// `enabled` field exists here at all** — the production create path
/// has no way to insert an enabled row, a type fact rather than a
/// convention a reviewer must remember to check (`startup.rs:421` used
/// to copy `enabled` straight from config; that is now impossible to
/// express through this type, not merely discouraged). `client_secret_
/// enc` must already hold the sealed ciphertext (or `None`); `provision_
/// mode`, `allowed_origins` and `id_token_algs` are a validated
/// [`crate::federation_policy::FederationTrustPolicy`]'s own canonical
/// fields, never raw config strings.
#[derive(Debug, Clone)]
pub struct NewFederationProvider {
    pub id: FederationProviderId,
    pub slug: String,
    pub display_name: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret_enc: Option<Vec<u8>>,
    pub scopes: String,
    pub provision_mode: ProvisionMode,
    pub allowed_origins: String,
    pub id_token_algs: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create a provider inside an existing transaction (RFC 094 M2b wave 3,
/// C16). Does no cryptography, unlike [`create`] above, which still does
/// its own sealing for its many test-fixture callers. The one production
/// call site (`runtime/startup.rs`) goes through `commands::
/// create_federation_provider` instead, which seals the secret before
/// calling this. `enabled`, `config_version`, `activation_generation`,
/// `validation_status` and every evidence column are left to their own
/// schema `DEFAULT` (migrations 0037/0050) — `0`, `0`, `'requires_
/// review'`, and `NULL` respectively — since [`NewFederationProvider`]
/// has no way to name a different value for any of them.
pub fn create_within_tx(
    tx: &rusqlite::Transaction<'_>,
    row: &NewFederationProvider,
) -> StoreResult<()> {
    tx.execute(
        "INSERT INTO federation_provider \
         (id, slug, display_name, issuer, client_id, client_secret_enc, \
          scopes, provision_mode, allowed_origins, id_token_algs, created_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            row.id.to_string(),
            row.slug,
            row.display_name,
            row.issuer,
            row.client_id,
            row.client_secret_enc,
            row.scopes,
            row.provision_mode.as_str(),
            row.allowed_origins,
            row.id_token_algs,
            row.created_at,
            row.updated_at,
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

/// Disable a provider.
///
/// Cycle J stage 10 (RFC 094 M2b criterion 1/3): no production caller
/// anywhere (C17's own row already said so). Kept behind `test-support`
/// — not deleted, unlike `delete` (no caller at all, including tests) —
/// because test fixtures in *this* crate's `repos::federation_provider`
/// tests, `sui-id-core`'s `authn::federated_mfa` tests, and
/// `commands::tests::runner::{federation,login}` all call it to flip a
/// provider back to disabled mid-test.
///
/// RFC 096-B2 S2a: this used to also *enable* a provider (an `enabled:
/// bool` parameter). Migration 0051's own `CHECK` now refuses an enabled
/// row with no `validated` status, no `id_token_algs`, and no evidence
/// bound to its current version/generation — exactly what a bare
/// `UPDATE ... SET enabled = 1` can never supply. Measured, not assumed:
/// every real caller already passed `false` (a `grep -rn "set_enabled("`
/// across the whole workspace found exactly one call passing `true`,
/// `repos::federation_provider::tests::set_enabled_and_delete`, moved to
/// [`create_enabled_for_test`] below instead). Dropping the parameter
/// makes "this function cannot enable a provider" a type fact rather
/// than a convention every future caller must remember.
#[cfg(any(test, feature = "test-support"))]
pub async fn set_disabled(
    db: &Database,
    id: FederationProviderId,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let id_str = id.to_string();
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE federation_provider SET enabled = 0, updated_at = ?1 WHERE id = ?2",
            params![now, id_str],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

/// RFC 096-B2 S2a Part C: the one test-support way to create an enabled
/// provider. Migration 0051's `CHECK` requires an enabled row to be
/// `validated`, carry `id_token_algs`, and carry all four evidence
/// fields with `evidence_config_version`/`evidence_activation_generation`
/// equal to the row's own counters — the exact shape a real C17 enable
/// (S4) will produce. No production code writes any of that yet, so
/// this test-only builder writes it directly: `row`'s own
/// `config_version`/`activation_generation` (0 for a fresh fixture),
/// `row.id_token_algs` if set, else a default singleton `"RS256"`, and a
/// fixed, arbitrary 32-byte evidence digest/fingerprint pair — nothing
/// in this codebase reads evidence bytes as meaningful before S3/S4
/// exist, so there is no "real" value to put there yet, and a fixed one
/// is exactly as inert as a random one while being reproducible.
/// **It satisfies the `CHECK`, the same `INSERT` every other row goes
/// through — it does not bypass it.**
#[cfg(any(test, feature = "test-support"))]
pub async fn create_enabled_for_test(
    db: &Database,
    row: &FederationProviderRow,
    client_secret_plain: Option<&str>,
) -> StoreResult<()> {
    const FIXED_EVIDENCE_BYTES: [u8; 32] = [0xABu8; 32];
    let enc: Option<Vec<u8>> = match client_secret_plain {
        Some(s) => Some(crypto::seal(db.key(), s.as_bytes(), AAD)?),
        None => None,
    };
    let id_token_algs = row
        .id_token_algs
        .clone()
        .unwrap_or_else(|| "RS256".to_string());
    let row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO federation_provider \
             (id, slug, display_name, issuer, client_id, client_secret_enc, \
              scopes, provision_mode, enabled, allowed_origins, \
              config_version, activation_generation, validation_status, \
              evidence_config_version, evidence_activation_generation, \
              evidence_policy_digest, evidence_observed_at, \
              evidence_metadata_fingerprint, evidence_jwks_fingerprint, \
              id_token_algs, created_at, updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?10,?11,'validated', \
                      ?12,?13,?14,?15,?16,?17,?18,?19,?20)",
            params![
                row.id.to_string(),
                row.slug,
                row.display_name,
                row.issuer,
                row.client_id,
                enc,
                row.scopes,
                row.provision_mode.as_str(),
                row.allowed_origins,
                row.config_version,
                row.activation_generation,
                row.config_version,
                row.activation_generation,
                FIXED_EVIDENCE_BYTES.as_slice(),
                row.updated_at,
                FIXED_EVIDENCE_BYTES.as_slice(),
                FIXED_EVIDENCE_BYTES.as_slice(),
                id_token_algs,
                row.created_at,
                row.updated_at,
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

/// Delete a provider (cascades to federation_link rows).
///
/// Cycle J stage 10 (RFC 094 M2b criterion 1/3): no production caller
/// anywhere (C18's own row already said so) -- unlike `set_enabled`, its
/// one caller (`set_enabled_and_delete`, this file's own `tests.rs`) is
/// in-crate, so plain `#[cfg(test)]` is enough; no cross-crate
/// `test-support` reach is needed for this one.
#[cfg(test)]
pub async fn delete(db: &Database, id: FederationProviderId) -> StoreResult<()> {
    let id_str = id.to_string();
    db.with_conn(move |conn| {
        let n = conn.execute("DELETE FROM federation_provider WHERE id = ?1", [&id_str])?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

/// RFC 096-B2 S2b, C23: the guarded full-policy replacement `UPDATE`.
/// Exactly RFC 094 clause 9's guard (`WHERE id = ? AND config_version = ?
/// AND activation_generation = ?`), clause 4 (forced disabled/
/// requires_review, every evidence column cleared), and clause 2 (both
/// counters -- already checked-incremented by the caller, this function
/// only writes the two values given). Returns the number of rows the
/// `UPDATE` touched; the caller asserts exactly 1.
#[allow(clippy::too_many_arguments)]
pub fn replace_policy_within_tx(
    tx: &rusqlite::Transaction<'_>,
    id: FederationProviderId,
    issuer: &str,
    client_id: &str,
    client_secret_enc: Option<&[u8]>,
    scopes: &str,
    provision_mode: ProvisionMode,
    allowed_origins: &str,
    id_token_algs: &str,
    old_config_version: i64,
    new_config_version: i64,
    old_activation_generation: i64,
    new_activation_generation: i64,
    now: DateTime<Utc>,
) -> StoreResult<usize> {
    let n = tx.execute(
        "UPDATE federation_provider \
         SET issuer = ?1, client_id = ?2, client_secret_enc = ?3, scopes = ?4, \
             provision_mode = ?5, allowed_origins = ?6, id_token_algs = ?7, \
             config_version = ?8, activation_generation = ?9, \
             enabled = 0, validation_status = 'requires_review', \
             evidence_config_version = NULL, evidence_activation_generation = NULL, \
             evidence_policy_digest = NULL, evidence_observed_at = NULL, \
             evidence_metadata_fingerprint = NULL, evidence_jwks_fingerprint = NULL, \
             updated_at = ?10 \
         WHERE id = ?11 AND config_version = ?12 AND activation_generation = ?13",
        params![
            issuer,
            client_id,
            client_secret_enc,
            scopes,
            provision_mode.as_str(),
            allowed_origins,
            id_token_algs,
            new_config_version,
            new_activation_generation,
            now,
            id.to_string(),
            old_config_version,
            old_activation_generation,
        ],
    )?;
    Ok(n)
}

/// RFC 096-B2 S2b, C25 (and the shape C17's own disable, S4, will
/// reuse): disable an enabled row, checked-incrementing only
/// `activation_generation` (RFC 094 `:445-450`) -- `config_version` is
/// untouched, since the trust policy itself has not changed, only its
/// activation. Guarded on `enabled = 1` as well as the exact old
/// generation: a provider already disabled cannot be "disabled" a
/// second time by this function (the caller checks this first and
/// returns `StoreError::NoChange` without calling it at all, but the
/// guard is restated here too, the same defensive-insurance shape
/// `federation_mfa_pending::refresh_for_restart_within_tx`'s own doc
/// comment explains).
pub fn disable_removed_within_tx(
    tx: &rusqlite::Transaction<'_>,
    id: FederationProviderId,
    old_activation_generation: i64,
    new_activation_generation: i64,
    now: DateTime<Utc>,
) -> StoreResult<usize> {
    let n = tx.execute(
        "UPDATE federation_provider \
         SET enabled = 0, validation_status = 'requires_review', \
             activation_generation = ?1, \
             evidence_config_version = NULL, evidence_activation_generation = NULL, \
             evidence_policy_digest = NULL, evidence_observed_at = NULL, \
             evidence_metadata_fingerprint = NULL, evidence_jwks_fingerprint = NULL, \
             updated_at = ?2 \
         WHERE id = ?3 AND activation_generation = ?4 AND enabled = 1",
        params![
            new_activation_generation,
            now,
            id.to_string(),
            old_activation_generation,
        ],
    )?;
    Ok(n)
}

/// RFC 096-B2 S2b Part D, C26. `display_name` is not trust policy (RFC
/// 096 `:434`) — updates without disabling, checked-incrementing
/// nothing, invalidating nothing. The one-column counterpart to
/// migration 0051's full-policy `UPDATE` (`commands::
/// replace_federation_provider_policy`).
pub fn update_display_name_within_tx(
    tx: &rusqlite::Transaction<'_>,
    id: FederationProviderId,
    display_name: &str,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let n = tx.execute(
        "UPDATE federation_provider SET display_name = ?1, updated_at = ?2 WHERE id = ?3",
        params![display_name, now, id.to_string()],
    )?;
    if n == 0 {
        Err(StoreError::NotFound)
    } else {
        Ok(())
    }
}

// ── Audit event anchors (RFC 004) ────────────────────────────────────────────
//
// The actual audit::append calls happen in the binary-crate federation handlers
// which have access to the Database.  The string literals here anchor the
// audit-matrix CI gate so the gate can verify they exist in source.

/// Audit event emitted when a federated sign-in completes successfully.
pub const AUDIT_SIGNIN_SUCCESS: &str = "auth.federation.signin.success";

/// RFC 096-B1 stage 6b: the registry-declared descriptor for
/// `auth.federation.takeover_blocked` (an email collision detected and
/// blocked, P2), emitted via [`crate::repos::audit::emit_must_attempt`]
/// rather than a free-string `audit::append` call -- unlike the three
/// anchors above, this event has no separate string constant: the literal
/// lives once, in `name` below, which is itself what the audit-matrix gate
/// reads. `actor`/`target` both `None`: the request that
/// reaches this check has no verified local identity to attribute it to
/// (P2 fires before any local user is resolved from the upstream
/// identity), matching the pre-conversion call site's own `actor: None,
/// target: None`.
pub static AUDIT_TAKEOVER_BLOCKED_DESCRIPTOR: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationTakeoverBlocked,
    name: "auth.federation.takeover_blocked",
    actor: ActorRequirement::None,
    target: TargetRequirement::None,
    attributes: &[
        AttributeSpec {
            name: "provider_id",
            description: "internal id of the federation provider the colliding identity came from",
            required: true,
        },
        AttributeSpec {
            name: "reason",
            description: "bounded reason token; always \"email_collision\" for this event",
            required: true,
        },
    ],
};

/// RFC 096-B1 stage 6c, F05: a transport/token-level failure talking to the
/// upstream IdP, discovered *after* the attempt was found (and, once
/// claimed, while it is `exchanging`) -- discovery fetch, token exchange,
/// or token response parsing. `federated_start`'s own discovery failure (no
/// attempt row exists yet at that point) is unrelated to F05's state
/// machine, but shares this same descriptor: RFC 094 M2b criterion 2 R1
/// (cycle J stage 10) converted its call site too, from the bare
/// `audit::append` this doc used to describe, to a spawned
/// `emit_must_attempt` call using the exact `provider_id`/`reason`
/// attributes below -- the former separate `AUDIT_SIGNIN_UPSTREAM_FAILURE`
/// string constant is gone; this is now the one declared source of the
/// name, from both call sites. `actor`/`target` both `None`, matching
/// [`AUDIT_TAKEOVER_BLOCKED_DESCRIPTOR`]'s own reasoning: nothing about a
/// transport failure identifies a local actor.
pub static AUDIT_SIGNIN_UPSTREAM_FAILURE_DESCRIPTOR: MustAttemptDescriptor =
    MustAttemptDescriptor {
        kind: AuditEventKind::AuthFederationSigninUpstreamFailure,
        name: "auth.federation.signin.upstream_failure",
        actor: ActorRequirement::None,
        target: TargetRequirement::None,
        attributes: &[
            AttributeSpec {
                name: "provider_id",
                description: "internal id of the federation provider the attempt was for",
                required: true,
            },
            AttributeSpec {
                name: "reason",
                description: "bounded reason token naming which transport/token step failed",
                required: true,
            },
        ],
    };

/// RFC 096-B1 stage 6c, F05: the generic `link_only` denial (RFC 096
/// `:699-701`) for any unlinked upstream identity -- deliberately the same
/// event regardless of whether the identity's email collides with a local
/// account, since `:699` requires `link_only` to return one generic result
/// either way (`identity_resolution`'s own `LinkRequired` state, and
/// `tests/e2e/r096_b1_stage5.rs`'s `link_only_gives_the_same_result_
/// whether_or_not_the_email_collides`, already pin this on the live path).
pub static AUDIT_LINK_REQUIRED_DESCRIPTOR: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationLinkRequired,
    name: "auth.federation.link_required",
    actor: ActorRequirement::None,
    target: TargetRequirement::None,
    attributes: &[
        AttributeSpec {
            name: "provider_id",
            description: "internal id of the federation provider the unlinked identity came from",
            required: true,
        },
        AttributeSpec {
            name: "reason",
            description: "bounded reason token; always \"unlinked_identity\" for this event -- \
                           kept required rather than omitted for the same reason \
                           AUDIT_TAKEOVER_BLOCKED_DESCRIPTOR's own `reason` is, despite also \
                           never varying: a uniform shape across all four F05 descriptors",
            required: true,
        },
    ],
};

/// RFC 096-B1 stage 6c, F05: every other post-claim denial -- a
/// validation, mapping, disabled-provider, disabled-user, MFA-state-read,
/// or sign-in commit failure that is none of
/// [`AUDIT_SIGNIN_UPSTREAM_FAILURE_DESCRIPTOR`],
/// [`AUDIT_TAKEOVER_BLOCKED_DESCRIPTOR`] or
/// [`AUDIT_LINK_REQUIRED_DESCRIPTOR`]'s own, more specific case. The
/// `reason` attribute is what makes this one bucket diagnosable: see
/// `crates/sui-id/src/http/federation_attempt_terminalize.rs`'s own
/// enumeration of every call site and the `reason` token each one passes.
///
/// `actor`/`target` both `Optional`, unlike the three descriptors above:
/// this bucket spans call sites both before and after a local user is
/// resolved (every pre-link-resolution validation failure has no user_id
/// at all; `complete_federated_signin`'s own checks do) -- `Optional` is
/// the one `ActorRequirement`/`TargetRequirement` value that is correct at
/// every one of them, matching `MfaAdminReset`'s own precedent for a
/// sometimes-known actor.
pub static AUDIT_SIGNIN_DENIED_DESCRIPTOR: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationSigninDenied,
    name: "auth.federation.signin.denied",
    actor: ActorRequirement::Optional,
    target: TargetRequirement::Optional,
    attributes: &[
        AttributeSpec {
            name: "provider_id",
            description: "internal id of the federation provider the attempt was for",
            required: true,
        },
        AttributeSpec {
            name: "reason",
            description: "bounded reason token naming which post-claim check refused",
            required: true,
        },
    ],
};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
