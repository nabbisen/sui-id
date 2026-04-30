//! Structured security events.
//!
//! Two things have always been at risk of drifting apart in sui-id:
//! the `tracing` log line written when something authentication-
//! relevant happens, and the `audit_log` row written for the same
//! event. They were two separate calls in two adjacent places, easy
//! to keep in sync at first and easy to let drift over time.
//!
//! This module makes them a single call. [`emit`] takes a typed
//! [`SecurityEvent`], writes a structured `tracing::info!` with the
//! event's fields, *and* appends a row to `audit_log` with the same
//! shape. Adding a new kind of event is a single match-arm here, not
//! a hunt through five handlers.
//!
//! ## Why one module instead of two?
//!
//! Operators consume the audit log (after the fact, for compliance)
//! and the structured tracing stream (live, in a SIEM) for *almost*
//! the same information. A login failure should be visible in both,
//! with the same fields. Routing both through one type ensures that.
//!
//! ## Conventions
//!
//! - Event names use dotted lowercase, e.g. `auth.login.success`,
//!   `auth.mfa.failure`. The first segment is always the rough
//!   subsystem (`auth`, `oauth`, `client`, `user`, `webauthn`,
//!   `mfa`, `signing_key`).
//! - Fields stick to a small vocabulary so SIEM queries stay
//!   uniform: `actor` (UserId), `target` (free-form id),
//!   `client_ip`, `client_id`, `request_id`. Add new fields only
//!   if a search will benefit.
//! - The `result` is one of `ok` / `failure` / `skipped` /
//!   `inactive` / `active`. Never free text.

use crate::time::SharedClock;
use sui_id_shared::ids::UserId;
use sui_id_store::repos::audit;
use sui_id_store::Database;

/// Outcome flag — kept narrow on purpose so SIEM queries can pivot
/// reliably.
#[derive(Debug, Clone, Copy)]
pub enum Outcome {
    Ok,
    Failure,
    Skipped,
    /// Used by introspection: the token is recognised and currently active.
    Active,
    /// Used by introspection: the token is unknown, expired, or not
    /// for this client.
    Inactive,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failure => "failure",
            Self::Skipped => "skipped",
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

/// Typed security event. Construct one of these and pass it to
/// [`emit`]; the function writes the structured log line and the
/// audit-log row in one go.
///
/// Each variant carries the fields specific to that event. Fields
/// shared across all variants — `actor`, `request_id`, `client_ip` —
/// are not on the variant; they go in [`Context`] which the caller
/// wraps around the event.
#[derive(Debug, Clone)]
pub enum SecurityEvent {
    LoginPasswordSuccess {
        user_id: UserId,
        username: String,
    },
    LoginPasswordFailure {
        username: String,
        reason: &'static str,
    },
    LoginPasswordOkMfaRequired {
        user_id: UserId,
    },
    MfaSuccess {
        user_id: UserId,
        method: &'static str, // "totp" | "recovery_code" | "webauthn"
    },
    MfaFailure {
        user_id: UserId,
        reason: &'static str,
    },
    SessionRevoked {
        user_id: UserId,
        reason: &'static str,
    },
    AdminMfaReset {
        actor: UserId,
        target_user: UserId,
        totp_removed: bool,
        passkeys_removed: usize,
    },
    AuthorizeIssued {
        user_id: UserId,
        client_id: String,
        scope: String,
    },
    AuthorizeRejected {
        client_id: Option<String>,
        reason: &'static str,
    },
    TokenIssued {
        user_id: UserId,
        client_id: String,
        grant_type: &'static str,
    },
    TokenRefreshed {
        user_id: UserId,
        client_id: String,
    },
    TokenIntrospected {
        client_id: String,
        outcome: Outcome,
        kind: Option<&'static str>,
    },
    TokenRevoked {
        client_id: String,
        kind: Option<&'static str>,
    },
    Logout {
        user_id: UserId,
    },
}

impl SecurityEvent {
    /// Stable event name. Operators set up SIEM queries and audit-log
    /// alerts against these strings — do not rename one without
    /// running a deprecation cycle.
    pub fn name(&self) -> &'static str {
        match self {
            Self::LoginPasswordSuccess { .. } => "auth.login.success",
            Self::LoginPasswordFailure { .. } => "auth.login.failure",
            Self::LoginPasswordOkMfaRequired { .. } => "auth.login.password_ok_mfa_required",
            Self::MfaSuccess { .. } => "auth.mfa.success",
            Self::MfaFailure { .. } => "auth.mfa.failure",
            Self::SessionRevoked { .. } => "auth.session.revoked",
            Self::AdminMfaReset { .. } => "mfa.admin_reset",
            Self::AuthorizeIssued { .. } => "oauth.authorize.issued",
            Self::AuthorizeRejected { .. } => "oauth.authorize.rejected",
            Self::TokenIssued { .. } => "oauth.token.issued",
            Self::TokenRefreshed { .. } => "oauth.token.refreshed",
            Self::TokenIntrospected { .. } => "oauth.token.introspected",
            Self::TokenRevoked { .. } => "oauth.token.revoked",
            Self::Logout { .. } => "auth.logout",
        }
    }

    /// Whom the action targets, if anyone. Used as the audit-log
    /// `target` column.
    fn target(&self) -> Option<String> {
        match self {
            Self::LoginPasswordSuccess { user_id, .. }
            | Self::LoginPasswordOkMfaRequired { user_id }
            | Self::MfaSuccess { user_id, .. }
            | Self::MfaFailure { user_id, .. }
            | Self::SessionRevoked { user_id, .. }
            | Self::Logout { user_id } => Some(user_id.to_string()),
            Self::LoginPasswordFailure { username, .. } => Some(username.clone()),
            Self::AdminMfaReset { target_user, .. } => Some(target_user.to_string()),
            Self::AuthorizeIssued {
                user_id, client_id, ..
            }
            | Self::TokenIssued {
                user_id, client_id, ..
            }
            | Self::TokenRefreshed { user_id, client_id } => {
                Some(format!("{user_id}:{client_id}"))
            }
            Self::AuthorizeRejected { client_id, .. } => client_id.clone(),
            Self::TokenIntrospected { client_id, .. } | Self::TokenRevoked { client_id, .. } => {
                Some(client_id.clone())
            }
        }
    }

    fn outcome(&self) -> Outcome {
        match self {
            Self::LoginPasswordSuccess { .. }
            | Self::LoginPasswordOkMfaRequired { .. }
            | Self::MfaSuccess { .. }
            | Self::SessionRevoked { .. }
            | Self::AdminMfaReset { .. }
            | Self::AuthorizeIssued { .. }
            | Self::TokenIssued { .. }
            | Self::TokenRefreshed { .. }
            | Self::TokenRevoked { .. }
            | Self::Logout { .. } => Outcome::Ok,
            Self::LoginPasswordFailure { .. }
            | Self::MfaFailure { .. }
            | Self::AuthorizeRejected { .. } => Outcome::Failure,
            Self::TokenIntrospected { outcome, .. } => *outcome,
        }
    }

    /// A short note describing the event, suitable for the audit log
    /// `note` column. Free text is fine here because operators read
    /// it; SIEM queries should pivot on the event `name` instead.
    fn note(&self) -> Option<String> {
        match self {
            Self::LoginPasswordFailure { reason, .. } | Self::MfaFailure { reason, .. } => {
                Some((*reason).into())
            }
            Self::SessionRevoked { reason, .. } => Some((*reason).into()),
            Self::MfaSuccess { method, .. } => Some((*method).into()),
            Self::AuthorizeIssued { scope, .. } => Some(scope.clone()),
            Self::AuthorizeRejected { reason, .. } => Some((*reason).into()),
            Self::TokenIssued { grant_type, .. } => Some((*grant_type).into()),
            Self::TokenIntrospected { kind, .. } | Self::TokenRevoked { kind, .. } => {
                kind.map(|k| k.into())
            }
            Self::AdminMfaReset {
                totp_removed,
                passkeys_removed,
                ..
            } => Some(format!(
                "totp={} passkeys={}",
                if *totp_removed { "removed" } else { "absent" },
                passkeys_removed
            )),
            _ => None,
        }
    }
}

/// Per-request context. The HTTP layer fills this in once at the
/// start of a request; handler code passes it (along with the event)
/// to [`emit`]. Cloneable on purpose so a handler can keep one
/// `Context` and emit several events from it.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Whoever performed the action, if known. For unauthenticated
    /// flows (e.g. failed login attempts), this is None.
    pub actor: Option<UserId>,
    /// Best-effort client IP, post-`trusted_proxies` resolution.
    pub client_ip: Option<String>,
    /// X-Request-Id, propagated from middleware.
    pub request_id: Option<String>,
}

impl Context {
    pub fn anonymous() -> Self {
        Self::default()
    }
    pub fn with_actor(mut self, actor: UserId) -> Self {
        self.actor = Some(actor);
        self
    }
    pub fn with_client_ip(mut self, ip: impl Into<String>) -> Self {
        self.client_ip = Some(ip.into());
        self
    }
}

/// Emit a security event.
///
/// Writes:
/// 1. A structured `tracing::info!` (or `warn!` for failures) at
///    the call site, with the event name as the `event` field and
///    the variant's data as additional fields.
/// 2. An `audit_log` row with `actor`, `action` (=event name),
///    `target`, `result`, and `note`.
///
/// Either side of (1) and (2) failing logs a warning but does not
/// propagate — observability work must never break the request.
pub fn emit(db: &Database, clock: &SharedClock, ctx: &Context, event: SecurityEvent) {
    let name = event.name();
    let outcome = event.outcome();
    let target = event.target();
    let note = event.note();
    let actor = ctx.actor;

    // Structured tracing line. Fields beyond actor/target/result/note
    // are pulled from the variant by string-formatting — kept simple
    // because the audit-log row is the canonical record; the tracing
    // line is for live consumption.
    let actor_str = actor.map(|a| a.to_string()).unwrap_or_else(|| "-".into());
    let target_str = target.clone().unwrap_or_else(|| "-".into());
    let note_str = note.clone().unwrap_or_default();
    let request_id = ctx.request_id.clone().unwrap_or_default();
    let client_ip = ctx.client_ip.clone().unwrap_or_default();
    match outcome {
        Outcome::Failure => {
            tracing::warn!(
                event = name,
                actor = %actor_str,
                target = %target_str,
                result = outcome.as_str(),
                note = %note_str,
                request_id = %request_id,
                client_ip = %client_ip,
                "security event"
            );
        }
        _ => {
            tracing::info!(
                event = name,
                actor = %actor_str,
                target = %target_str,
                result = outcome.as_str(),
                note = %note_str,
                request_id = %request_id,
                client_ip = %client_ip,
                "security event"
            );
        }
    }

    // Audit-log row.
    if let Err(e) = audit::append(
        db,
        &sui_id_store::models::AuditLogRow {
            at: clock.now(),
            actor,
            action: name.into(),
            target,
            result: outcome.as_str().into(),
            note,
        },
    ) {
        // Don't fail the caller; the tracing line above is still
        // there as a fallback record.
        tracing::warn!(
            error = %e,
            event = name,
            "failed to append audit-log row for security event"
        );
    }
}
