//! Internal domain row types.
//!
//! These mirror the DB schema closely. They are the input/output type for
//! the repository functions in [`crate::repos`]. The distinction from the
//! public API DTOs in `sui-id-shared::api` is deliberate: storage and wire
//! formats evolve independently.

use chrono::{DateTime, Utc};
use sui_id_shared::ids::{ClientId, EmailOutboxId, SessionId, SigningKeyId, UserId};
use sui_id_shared::{CodeHash, FamilyId, RefreshTokenId};

/// Administrative access level for a user account (RFC 071, migration 0027).
///
/// The three values correspond to the `role` column's check constraint.
/// `is_admin` (the old boolean column) is still written in parallel until
/// migration 0029 drops it; read paths use `role` exclusively after v0.59.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Full administrative capability — can read and mutate all admin state.
    Admin,
    /// Read-only access to all admin surfaces; cannot mutate any state.
    Auditor,
    /// End-user self-service only (`/me/*` routes).
    User,
}

impl Role {
    /// Parse from the database string value.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Self::Admin),
            "auditor" => Some(Self::Auditor),
            "user" => Some(Self::User),
            _ => None,
        }
    }
    /// Database / API string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Auditor => "auditor",
            Self::User => "user",
        }
    }
    /// True if the role permits administrative READ access (admin or auditor).
    pub fn can_read_admin(self) -> bool {
        matches!(self, Self::Admin | Self::Auditor)
    }
    /// True if the role permits administrative WRITE access (admin only).
    pub fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Source of a user's identity (RFC 005).
pub enum UserSource {
    /// Credentials stored locally in the `credentials` table.
    #[default]
    Local,
    /// Authenticated via a read-only LDAP bind; no local password stored.
    Ldap,
}

impl UserSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Ldap => "ldap",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "local" => Some(Self::Local),
            "ldap" => Some(Self::Ldap),
            _ => None,
        }
    }
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: UserId,
    pub username: String,
    pub display_name: Option<String>,
    /// Original-case email address. Added in migration 0012.
    /// NULL = "we don't have one"; userinfo omits the `email` claim in
    /// that case. The original case is preserved so the UI can display
    /// what the user typed at registration.
    pub email: Option<String>,
    /// Case-folded form of `email` (lower + trim). Added in migration
    /// 0020. Used for all uniqueness checks and lookup-by-email paths
    /// (including forgot-password) so that "Alice@Example.com" and
    /// "alice@example.com" resolve to the same account.
    pub email_normalized: Option<String>,
    /// Timestamp at which the email address was confirmed. NULL for all
    /// users until an email-verification flow ships (future RFC). Present
    /// now so userinfo can honestly report `email_verified: false`.
    pub email_verified_at: Option<DateTime<Utc>>,
    /// Preferred UI locale, BCP-47 tag (e.g. "ja", "en"). NULL =
    /// no preference; the application falls back through Cookie /
    /// Accept-Language / server default. Added in migration 0016.
    pub preferred_lang: Option<String>,
    /// Legacy boolean admin flag — kept in sync with `role` until migration
    /// 0029 drops this column. New code must read `role` instead.
    pub is_admin: bool,
    /// RFC 071 role (migration 0027). Authoritative source of admin access.
    pub role: Role,
    pub is_disabled: bool,
    pub is_deleted: bool,
    /// RFC 074: timestamp of the user's most recent successful login
    /// (migration 0030). NULL until first login after this column was added.
    pub last_login_at: Option<DateTime<Utc>>,
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
    /// RFC 005: source of this user's identity.
    /// `'local'` — password stored in `credentials` table.
    /// `'ldap'`  — authenticated via LDAP bind; no `credentials` row.
    pub source: UserSource,
    /// RFC 005: opaque stable identifier from the external source
    /// (objectGUID, entryUUID, DN).  `None` for `source='local'` users.
    pub external_stable_id: Option<String>,
}

// ── Federation (RFC 004) ────────────────────────────────────────────────────

/// Upstream provisioning policy for a federation provider.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProvisionMode {
    /// User must authenticate locally to create the link.
    #[default]
    LinkOnly,
    /// Create a password-less local user on first sign-in (gated on
    /// `email_verified = true`; otherwise held for admin approval).
    ProvisionOnFirstLogin,
}

impl ProvisionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LinkOnly => "link_only",
            Self::ProvisionOnFirstLogin => "provision_on_first_login",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "provision_on_first_login" => Self::ProvisionOnFirstLogin,
            _ => Self::LinkOnly,
        }
    }
}

/// Upstream OIDC identity-provider configuration (RFC 004).
#[derive(Debug, Clone)]
pub struct FederationProviderRow {
    pub id: sui_id_shared::ids::FederationProviderId,
    pub slug: String,
    pub display_name: String,
    pub issuer: String,
    pub client_id: String,
    /// XChaCha20-Poly1305 ciphertext of the client secret.
    /// `None` for public clients (no secret).
    pub client_secret_enc: Option<Vec<u8>>,
    /// Space-separated requested scopes (e.g. `"openid email"`).
    pub scopes: String,
    pub provision_mode: ProvisionMode,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// A confirmed link between a local user and an upstream identity (RFC 004).
#[derive(Debug, Clone)]
pub struct FederationLinkRow {
    pub user_id: sui_id_shared::ids::UserId,
    pub provider_id: sui_id_shared::ids::FederationProviderId,
    /// The `sub` claim from the upstream ID token — the mapping key (P1).
    pub upstream_sub: String,
    /// Last-seen upstream email; metadata only — never used for mapping (P1).
    pub upstream_email: Option<String>,
    pub linked_at: chrono::DateTime<chrono::Utc>,
    pub last_seen_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct CredentialRow {
    pub user_id: UserId,
    pub password_hash: String,
    pub must_change: bool,
    pub updated_at: DateTime<Utc>,
}

/// Per-client consent screen policy (RFC 038).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConsentPolicy {
    /// No consent screen — first-party default. Existing behaviour.
    #[default]
    None,
    /// Show consent on first authorization; skip if prior grant covers
    /// the requested scopes.
    FirstTime,
    /// Always show the consent screen regardless of stored grants.
    Always,
}

impl ConsentPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::FirstTime => "first_time",
            Self::Always => "always",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "first_time" => Self::FirstTime,
            "always" => Self::Always,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// How a client was registered (RFC 008).
pub enum RegistrationSource {
    /// Registered by an operator via the admin panel (default).
    #[default]
    Admin,
    /// Self-registered via RFC 7591 dynamic client registration.
    Dynamic,
}

impl RegistrationSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Dynamic => "dynamic",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "dynamic" => Self::Dynamic,
            _ => Self::Admin,
        }
    }
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
    /// Per-client consent policy (RFC 038).
    pub consent_policy: ConsentPolicy,
    /// RFC 008: how the client was registered.
    pub registered_via: RegistrationSource,
    /// RFC 008: application identity URLs (validated HTTPS, never fetched).
    pub logo_uri: Option<String>,
    pub homepage_uri: Option<String>,
    pub privacy_policy_uri: Option<String>,
    pub tos_uri: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A stored user consent record (RFC 038).
#[derive(Debug, Clone)]
pub struct UserConsentRow {
    pub user_id: UserId,
    pub client_id: ClientId,
    /// Space-separated granted scope tokens.
    pub granted_scopes: String,
    pub granted_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AuthorizationCodeRow {
    /// SHA-256 hex digest of the code plaintext. The plaintext is never
    /// stored; a DB leak cannot expose outstanding codes for replay.
    pub code_hash: CodeHash,
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
    /// Opaque row identifier (16-byte CSPRNG, base64url-encoded).
    pub id: RefreshTokenId,
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
    pub family_id: FamilyId,
}

/// The result of issuing a new refresh token.
///
/// Separates the stored row (which never carries the plaintext) from the
/// single-use plaintext token returned to the caller at issuance time.
/// After `insert`, the caller is responsible for delivering `token` to
/// the client and then letting it drop (zeroize on drop).
#[derive(Debug)]
pub struct IssuedRefreshToken {
    /// The row as it was written to the database (no plaintext field).
    pub row: RefreshTokenRow,
    /// The plaintext token to hand to the client. Redacted from `Debug`.
    pub token: sui_id_shared::RawRefreshToken,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HibpMode {
    /// No check performed; no outbound request made. The right
    /// setting for offline / air-gapped deployments.
    Off,
    /// Check performed; a hit is recorded as an audit event but
    /// the password is accepted. Default at install time.
    #[default]
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
    /// Hashed bearer token for the `/metrics` endpoint (RFC 006).
    /// `None` means no token has been generated; the bearer-token auth
    /// path is not available until the operator runs
    /// `sui-id admin rotate-metrics-token`.
    pub metrics_token_hash: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ── Email outbox (RFC 001) ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailOutboxState {
    Queued,
    Sending,
    Sent,
    Failed,
}

impl EmailOutboxState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for EmailOutboxState {
    type Err = crate::errors::StoreError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(Self::Queued),
            "sending" => Ok(Self::Sending),
            "sent" => Ok(Self::Sent),
            "failed" => Ok(Self::Failed),
            other => Err(crate::errors::StoreError::InvalidData(format!(
                "unknown outbox state: {other:?}"
            ))),
        }
    }
}

/// A row in the `email_outbox` table. Both `recipient_enc` and `payload_enc`
/// are AAD-bound ciphertext sealed under the master key.
#[derive(Debug, Clone)]
pub struct EmailOutboxRow {
    pub id: EmailOutboxId,
    pub state: EmailOutboxState,
    /// Stable template identifier, e.g. `"forgot_password"`.
    pub template: String,
    pub recipient_enc: Vec<u8>,
    pub payload_enc: Vec<u8>,
    pub attempt_count: i64,
    pub next_attempt_at: DateTime<Utc>,
    pub last_error: Option<String>,
    /// BCP-47 locale tag resolved at enqueue time from the recipient's
    /// `preferred_lang`. `None` means "fall back to server default".
    pub locale: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
