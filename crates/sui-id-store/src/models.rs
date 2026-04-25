//! Internal domain row types.
//!
//! These mirror the DB schema closely. They are the input/output type for
//! the repository functions in [`crate::repos`]. The distinction from the
//! public API DTOs in `sui-id-shared::api` is deliberate: storage and wire
//! formats evolve independently.

use chrono::{DateTime, Utc};
use sui_id_shared::ids::{ClientId, SessionId, SigningKeyId, UserId};

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: UserId,
    pub username: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
    pub is_disabled: bool,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CredentialRow {
    pub user_id: UserId,
    pub password_hash: String,
    pub must_change: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ClientRow {
    pub id: ClientId,
    pub name: String,
    pub confidential: bool,
    pub secret_hash: Option<String>,
    pub redirect_uris: Vec<String>,
    pub is_disabled: bool,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AuthorizationCodeRow {
    pub code_hash: String,
    pub client_id: ClientId,
    pub user_id: UserId,
    pub redirect_uri: String,
    pub scope: String,
    pub nonce: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub expires_at: DateTime<Utc>,
    pub consumed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: SessionId,
    pub user_id: UserId,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRow {
    pub id: String,
    pub token_plain: Option<String>, // populated only at issuance
    pub user_id: UserId,
    pub client_id: ClientId,
    pub scope: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SigningKeyRow {
    pub id: SigningKeyId,
    pub algorithm: String,
    /// Sealed bytes of the private key (XChaCha20-Poly1305 of raw 32 bytes).
    pub private_key_enc: Vec<u8>,
    pub public_key: Vec<u8>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub rotated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct AuditLogRow {
    pub at: DateTime<Utc>,
    pub actor: Option<UserId>,
    pub action: String,
    pub target: Option<String>,
    pub result: String,
    pub note: Option<String>,
}
