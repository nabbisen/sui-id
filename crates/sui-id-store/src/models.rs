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
    /// Optional email address. Added in migration 0012. Used by the
    /// setup wizard and admin user-creation form, and (in a future
    /// release) by the password-change notification and forgot-password
    /// reset flows once SMTP support lands. NULL = "we don't have one";
    /// userinfo simply omits the `email` claim in that case.
    pub email: Option<String>,
    /// Preferred UI locale, BCP-47 tag (e.g. "ja", "en"). NULL =
    /// no preference; the application falls back through Cookie /
    /// Accept-Language / server default. Added in migration 0016.
    pub preferred_lang: Option<String>,
    pub is_admin: bool,
    pub is_disabled: bool,
    pub is_deleted: bool,
    /// Stable per-user UUID handle for WebAuthn `user.id`. Backfilled
    /// at migration 0004 time for users created before that. Decoupled
    /// from `id` (sui-id `UserId`) on purpose so the relying-party
    /// handle can be rotated independently without breaking foreign
    /// keys.
    pub user_uuid: uuid::Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Consecutive failed password attempts since last success.
    /// Reset to 0 on a successful password verification.
    pub failed_login_count: i64,
    /// Earliest moment the account becomes eligible for password
    /// verification again. None means "not locked". A `Some(t)` with
    /// `t` already in the past represents a stale lock that has
    /// expired and will be cleared on the next sign-in.
    pub locked_until: Option<DateTime<Utc>>,
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
    /// Space-separated list of permitted scope values. Empty string means
    /// "no policy configured" — interpreted as "permit any scope" for
    /// backwards compatibility with rows created before migration 0002.
    pub allowed_scopes: String,
    /// Logout return URIs registered for this client. Independent of
    /// `redirect_uris`. An empty list triggers a fall-back to
    /// `redirect_uris` for backwards compatibility (with a deprecation
    /// log line) — see `sui_id_core::session::resolve_post_logout_uri`.
    pub post_logout_redirect_uris: Vec<String>,
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
    /// Snapshot of the originating session's authentication factors,
    /// taken at code issuance. Read at the token endpoint to populate
    /// the resulting ID token's `acr` / `amr`. We snapshot rather
    /// than dereferencing back to the session because the session
    /// can be revoked between authorize and token without affecting
    /// the validity of an already-issued auth code.
    pub auth_methods: Vec<sui_id_shared::AuthMethod>,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: SessionId,
    pub user_id: UserId,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    /// Which authentication factors were used to establish this
    /// session. Read at ID-token issuance to populate `acr` / `amr`.
    /// Stored as a JSON array of [`sui_id_shared::AuthMethod`] values
    /// in the database. An empty list represents a pre-migration
    /// session and is treated as single-factor by issuance code.
    pub auth_methods: Vec<sui_id_shared::AuthMethod>,
    /// Most recent moment at which this session re-proved a strong
    /// factor — set on initial login when MFA was used, and updated
    /// every time the user completes a step-up challenge. Used by
    /// `core::step_up::is_fresh` to decide whether a sensitive
    /// action can proceed without a fresh challenge. `None` means
    /// either the session was established with password-only
    /// authentication, or the row predates the v0.20.0 migration —
    /// either way the freshness check refuses.
    pub last_step_up_at: Option<DateTime<Utc>>,
    /// Most recent moment at which this session was presented in
    /// an authenticated request. Backs the v0.25.0 idle-session
    /// timeout — when `now - last_used_at >
    /// idle_session_timeout_secs` (and the timeout is non-zero),
    /// `session::resolve` revokes the row before returning. The
    /// column is `NULL` for rows from before migration 0018 and
    /// for sessions that have never been re-presented; the
    /// application treats `NULL` as "as old as `created_at`",
    /// which is the conservative choice.
    pub last_used_at: Option<DateTime<Utc>>,
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
    /// Snapshot of the originating session's factors, propagated
    /// from the auth code at issuance and forward through every
    /// rotation. The refreshed ID token reports the *original*
    /// authentication, never a synthetic re-evaluation.
    pub auth_methods: Vec<sui_id_shared::AuthMethod>,
    /// Identifier of the rotation family this refresh token belongs
    /// to. The first refresh token issued for a given session
    /// (initial authorization-code exchange) has `family_id == id`.
    /// Each subsequent rotation copies the parent's `family_id`
    /// onto the new row. If a revoked token is later replayed, we
    /// revoke every row in the same family — see the refresh-grant
    /// flow in `sui_id_core::authorize`.
    pub family_id: String,
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

#[derive(Debug, Clone)]
pub struct UserTotpRow {
    pub user_id: UserId,
    /// Sealed TOTP secret bytes (20 bytes when decrypted).
    pub secret_enc: Vec<u8>,
    pub enabled: bool,
    /// Sealed JSON array of Argon2id hashes (one per single-use recovery
    /// code). `None` if the user never generated recovery codes — which is
    /// a temporary state during initial enrolment.
    pub recovery_codes_enc: Option<Vec<u8>>,
    /// Most recently accepted RFC 6238 time step. Used to reject replays
    /// of the same 6-digit code within its 30-second window.
    pub last_used_step: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub confirmed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct LoginPendingMfaRow {
    pub id: sui_id_shared::ids::PendingMfaId,
    pub user_id: UserId,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct UserWebauthnCredentialRow {
    pub id: sui_id_shared::ids::WebauthnCredentialId,
    pub user_id: UserId,
    /// Raw credential id bytes as the authenticator returned them.
    pub credential_id: Vec<u8>,
    /// Sealed `webauthn_rs::prelude::Passkey` (JSON).
    pub passkey_enc: Vec<u8>,
    pub nickname: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct WebauthnPendingRow {
    pub id: sui_id_shared::ids::WebauthnPendingId,
    pub kind: WebauthnPendingKind,
    /// `None` for authentication ceremonies that started without a
    /// known user (e.g. discoverable-credential flows). Today we only
    /// drive authentication after the password step has identified the
    /// user, so this is always `Some` in practice.
    pub user_id: Option<UserId>,
    /// Opaque JSON the application layer hands back to webauthn-rs.
    pub state_json: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebauthnPendingKind {
    Register,
    Authenticate,
    /// A WebAuthn assertion ceremony driven from `/me/security/step-up`
    /// — the user is already signed in, and a successful finish
    /// stamps the session's `last_step_up_at` rather than minting a
    /// new session. Distinct from `Authenticate` so a pending row
    /// can never cross flows by accident. Stored as the literal
    /// string `step_up` in the `webauthn_pending.kind` column —
    /// migration 0013 widens the CHECK constraint to allow it.
    StepUp,
}

impl WebauthnPendingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::Authenticate => "authenticate",
            Self::StepUp => "step_up",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "register" => Some(Self::Register),
            "authenticate" => Some(Self::Authenticate),
            "step_up" => Some(Self::StepUp),
            _ => None,
        }
    }
}

// ---------- SMTP configuration (v0.22.0) ----------

/// Connection-level TLS mode for the SMTP submission relay.
///
/// `wasm-smtp` requires TLS at the API surface, so a true "plain"
/// option is not exposed here. The two values map directly to the
/// adapter's two connect helpers:
/// `TokioTlsTransport::connect_implicit_tls` for `Implicit`, and
/// `TokioPlainTransport::connect` followed by
/// `SmtpClient::connect_starttls` for `StartTls`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpTlsMode {
    /// TLS-from-the-start, port 465. The transport handshakes TLS
    /// before any SMTP byte is exchanged.
    Implicit,
    /// Plaintext greeting then upgrade in-place after STARTTLS,
    /// port 587. The transport switches to TLS mid-session.
    StartTls,
}

impl SmtpTlsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Implicit => "implicit",
            Self::StartTls => "starttls",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "implicit" => Some(Self::Implicit),
            "starttls" => Some(Self::StartTls),
            _ => None,
        }
    }
}

/// Singleton row in `smtp_config`. The primary key is hard-coded
/// to `'singleton'` — there is only ever one effective SMTP
/// configuration. See migration 0014 for full rationale.
#[derive(Debug, Clone)]
pub struct SmtpConfigRow {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub tls_mode: SmtpTlsMode,
    pub username: Option<String>,
    /// Sealed XChaCha20-Poly1305 ciphertext over the SMTP
    /// password. AAD = `b"smtp.password"`. `None` when the
    /// relay does not require authentication.
    pub password_enc: Option<Vec<u8>>,
    pub from_address: String,
    pub from_name: Option<String>,
    /// Public origin sui-id is reachable at, used for
    /// constructing absolute URLs in outgoing mail (e.g. the
    /// password-reset link).
    pub base_url: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ---------- Password reset tokens (v0.22.0) ----------

/// One row in `password_reset_tokens`. The plaintext token never
/// touches the database; only its SHA-256 hash. See migration 0015.
#[derive(Debug, Clone)]
pub struct PasswordResetTokenRow {
    pub id: sui_id_shared::ids::PasswordResetTokenId,
    pub user_id: sui_id_shared::ids::UserId,
    pub token_hash: Vec<u8>,
    pub issued_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub consumed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub requester_ip: Option<String>,
}

/// HIBP (Pwned Passwords) check operational mode. Stored as the
/// string `'off' | 'warn' | 'block'` in `server_settings.hibp_mode`
/// (CHECK-constrained), see migration 0017.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HibpMode {
    /// No check performed; no outbound request made. The right
    /// setting for offline / air-gapped deployments.
    Off,
    /// Check performed; a hit is recorded as an audit event but
    /// the password is accepted. Default at install time.
    Warn,
    /// Check performed; a hit refuses the password.
    Block,
}

impl HibpMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Warn => "warn",
            Self::Block => "block",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "off" => Some(Self::Off),
            "warn" => Some(Self::Warn),
            "block" => Some(Self::Block),
            _ => None,
        }
    }
}

impl Default for HibpMode {
    fn default() -> Self {
        Self::Warn
    }
}

// ---------- Server settings (v0.23.0) ----------

/// Singleton row in `server_settings`. Holds process-wide settings
/// that an admin should be able to change without restarting the
/// server.
#[derive(Debug, Clone)]
pub struct ServerSettingsRow {
    /// BCP-47 language tag used as a fallback when no per-user,
    /// cookie, or Accept-Language preference matches a supported
    /// locale. Validated at the application layer to be one of the
    /// known `Locale` tags.
    pub default_lang: String,
    /// Pwned Passwords check mode. See [`HibpMode`].
    pub hibp_mode: HibpMode,
    /// Idle-session-timeout, in seconds. `0` means the feature is
    /// disabled (sessions only expire at their `expires_at`).
    /// Application-validated to be in `[0, 30 * 86400]` so a
    /// fat-fingered value does not silently exceed the absolute
    /// session ceiling.
    pub idle_session_timeout_secs: i64,
    /// Maximum simultaneous active sessions per user. `0` means
    /// the cap is disabled. When non-zero, a new login that would
    /// exceed the cap evicts the oldest existing session (FIFO).
    /// Application-validated to be in `[0, 1000]`.
    pub max_concurrent_sessions: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
