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
pub struct CreateInitialAdminRequest {
    /// Setup token printed once on stderr at first start.
    pub setup_token: String,
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInitialAdminResponse {
    pub user_id: UserId,
    pub username: String,
}

// ---------- session / login ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoAmIResponse {
    pub user_id: UserId,
    pub username: String,
    pub is_admin: bool,
    pub display_name: Option<String>,
}

// ---------- users ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: UserId,
    pub username: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
    pub is_disabled: bool,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserListResponse {
    pub users: Vec<UserSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
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
pub struct CreateClientResponse {
    pub client_id: ClientId,
    pub client_secret: Option<String>,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub confidential: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSummary {
    pub id: ClientId,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub confidential: bool,
    pub is_disabled: bool,
    pub is_deleted: bool,
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
