//! `federation_provider` repository — upstream OIDC IdP configurations
//! (RFC 004).
//!
//! Provider secrets are stored encrypted with the master key using
//! XChaCha20-Poly1305 (AAD = `"federation_provider.client_secret"`).

use crate::{
    Database,
    crypto::{self, MasterKey},
    errors::{StoreError, StoreResult},
    models::{FederationProviderRow, ProvisionMode},
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
    Ok(FederationProviderRow {
        id,
        slug: row.get(1)?,
        display_name: row.get(2)?,
        issuer: row.get(3)?,
        client_id: row.get(4)?,
        client_secret_enc: row.get(5)?,
        scopes: row.get(6)?,
        provision_mode: ProvisionMode::parse(&row.get::<_, String>(7)?),
        enabled: row.get::<_, i64>(8)? != 0,
        allowed_origins: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

const SELECT: &str = "SELECT id, slug, display_name, issuer, client_id, client_secret_enc, \
     scopes, provision_mode, enabled, allowed_origins, created_at, updated_at \
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
              scopes, provision_mode, enabled, allowed_origins, created_at, updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
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

/// Update `enabled` flag.
pub async fn set_enabled(
    db: &Database,
    id: FederationProviderId,
    enabled: bool,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let id_str = id.to_string();
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE federation_provider SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![enabled as i64, now, id_str],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

/// Update `allowed_origins` only (RFC 134 step 7a). Deliberately not a
/// general `update`: one column, so a future caller reconciling the origin
/// set at boot cannot reach `issuer`, `client_id` or any other
/// config-managed field by accident.
pub async fn update_allowed_origins(
    db: &Database,
    id: FederationProviderId,
    allowed_origins: &str,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let id_str = id.to_string();
    let allowed_origins = allowed_origins.to_owned();
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE federation_provider SET allowed_origins = ?1, updated_at = ?2 WHERE id = ?3",
            params![allowed_origins, now, id_str],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

/// Delete a provider (cascades to federation_link rows).
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

// ── Audit event anchors (RFC 004) ────────────────────────────────────────────
//
// The actual audit::append calls happen in the binary-crate federation handlers
// which have access to the Database.  The string literals here anchor the
// audit-matrix CI gate so the gate can verify they exist in source.

/// Audit event emitted when a federated sign-in completes successfully.
pub const AUDIT_SIGNIN_SUCCESS: &str = "auth.federation.signin.success";
/// Audit event emitted when the upstream IdP returns an error during callback.
pub const AUDIT_SIGNIN_UPSTREAM_FAILURE: &str = "auth.federation.signin.upstream_failure";
/// Audit event emitted when a federation link is created (first sign-in or link flow).
pub const AUDIT_LINK_CREATED: &str = "auth.federation.link.created";

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
/// or token response parsing. `federated_start`'s own discovery failure
/// (no attempt row exists yet at that point) is unrelated and keeps using
/// `AUDIT_SIGNIN_UPSTREAM_FAILURE`'s existing best-effort `audit::append`
/// path; this descriptor is for the post-claim case F05 owns.
/// `actor`/`target` both `None`, matching
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
