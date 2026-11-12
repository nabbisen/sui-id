//! Data-transfer objects exchanged on the JSON management API.
//!
//! These are deliberately separated from internal domain types: the wire
//! format is a stability boundary that may evolve at a different pace from
//! storage schemas.

use crate::ids::{ClientId, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------- setup ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStatusDto {
    pub initialized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInitialAdminResponse {
    pub user_id: UserId,
    pub username: String,
}

// ---------- session / login ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoAmIResponse {
    pub user_id: UserId,
    pub username: String,
    pub is_admin: bool,
    pub display_name: Option<String>,
}

// ---------- users ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: UserId,
    pub username: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
    pub is_disabled: bool,
    pub is_deleted: bool,
    /// True if the user has any active MFA factor — TOTP enrolment or
    /// at least one registered passkey. Drives whether the admin user
    /// list shows a "Reset MFA" action.
    #[serde(default)]
    pub mfa_enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserListResponse {
    pub users: Vec<UserSummary>,
}

// ---------- clients ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateClientRequest {
    pub name: String,
    pub redirect_uris: Vec<String>,
    /// If true, a confidential client (gets a secret). Otherwise public (PKCE only).
    #[serde(default = "default_true")]
    pub confidential: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSummary {
    pub id: ClientId,
    pub name: String,
    pub redirect_uris: Vec<String>,
    /// Space-separated list of permitted scopes. Empty string means
    /// "no policy configured" — interpreted as "permit any scope" for
    /// backwards compatibility with rows created before migration 0002.
    #[serde(default)]
    pub allowed_scopes: String,
    /// RP-initiated logout return URIs. Empty list falls back to
    /// `redirect_uris` at logout time (legacy compatibility).
    #[serde(default)]
    pub post_logout_redirect_uris: Vec<String>,
    pub confidential: bool,
    pub is_disabled: bool,
    pub is_deleted: bool,
    /// Consent policy as a string tag (RFC 038): "none", "first_time", "always".
    #[serde(default)]
    pub consent_policy: String,
    /// Registration source as a string tag (RFC 065): "admin", "dynamic".
    /// RFC 136 D2: drives the self-registered marker in the admin UI.
    #[serde(default)]
    pub registered_via: String,
    /// RFC 095 M3 stage 4a §5: `None` drives the "unrecorded" marker in
    /// the admin UI. Carries the wire value (`"none"` /
    /// `"client_secret_basic"` / `"client_secret_post"`, or an unknown
    /// stored value — see `auth_method_unrecognized`) when recorded; not
    /// rendered today, but kept a real value rather than a `bool` so a
    /// future renderer does not need to widen this type to show it.
    #[serde(default)]
    pub token_endpoint_auth_method: Option<String>,
    /// RFC 095 M3 stage 4b §0.2: `true` when the column holds a value
    /// outside the closed set (`sui_id_store::models::TokenEndpointAuthMethod::Unknown`)
    /// — present, but not what `validate_auth_method` would ever have
    /// written, which only happens if something changed the database from
    /// outside. Drives its own, more alarming marker than "unrecorded":
    /// a value nobody can parse is worth more attention than one nobody
    /// recorded, not less.
    #[serde(default)]
    pub auth_method_unrecognized: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientListResponse {
    pub clients: Vec<ClientSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateClientRequest {
    pub name: Option<String>,
    pub redirect_uris: Option<Vec<String>>,
}

// ---------- signing keys ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningKeySummary {
    pub id: crate::ids::SigningKeyId,
    pub algorithm: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub rotated_at: Option<DateTime<Utc>>,
}

// ---------- federation providers (RFC 096-B2 S5) ----------

/// One row of the `/admin/federation-providers` list page. Deliberately
/// omits every trust field (`client_id`, `client_secret_enc`, `scopes`,
/// `allowed_origins`, `id_token_algs`, and anything digest/fingerprint-
/// shaped) — RFC 096 `:380`: the admin UI may enable or disable a
/// provider, but may not mutate, or even display as editable, its trust
/// policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationProviderSummary {
    pub id: crate::ids::FederationProviderId,
    pub slug: String,
    pub display_name: String,
    pub issuer: String,
    pub enabled: bool,
    /// `true` for `ValidationStatus::RequiresReview`, `false` for
    /// `Validated` — a plain bool so this crate need not depend on
    /// `sui-id-store`'s model type.
    pub requires_review: bool,
    /// The provider's own `evidence_observed_at` — when it was last
    /// checked and (if the check passed) enabled. `None` means never.
    pub last_checked_at: Option<DateTime<Utc>>,
    pub has_linked_users: bool,
    /// `false` when no `[[federation_providers]]` config block names
    /// this provider's slug (the C25 case) — the list row then offers
    /// no Enable action, since it could only fail.
    pub is_configured: bool,
}

// ---------- audit ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntryDto {
    pub at: DateTime<Utc>,
    pub actor: Option<UserId>,
    pub action: String,
    pub target: Option<String>,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
