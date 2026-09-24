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
