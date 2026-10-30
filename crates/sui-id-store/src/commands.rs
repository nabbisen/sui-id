//! RFC 094 M2a Stage 1 — the proving slice.
//!
//! Five commands, chosen (per the Stage 1 scope review, 2026-08-28) to
//! cover every distinct shape the registry foundation must prove, not
//! picked by count:
//!
//! | Command | Proves |
//! |---|---|
//! | [`K01`] (`signing_keys::rotate_atomic_within_tx`) | Class-A runner, simple case: one command, one event |
//! | [`U22`] (`users::record_login_failure_within_tx`) | Conditional Class-A: `auth.login.failure` **or** `auth.lockout`, exhaustively |
//! | [`U01`] (`users::create_within_tx`) | Class-A, one event: `user.create` (it once branched on input into `user.create_warned_hibp`, retired by RFC 115 D9) |
//! | `u30_protocol_insert` | `Database::protocol` — proves it **cannot** construct `Audited<T>` |
//! | `o01_operational_enqueue` | `Database::operational` — the fourth runner exists and is distinct from `protocol` |
//!
//! Not in the slice, per the same review: `X` (bootstrap/migrations) and
//! `I` (internal primitives) — different lifecycle and not a top-level
//! capability, respectively.
//!
//! None of this was wired into a production call site as of Stage 1. RFC
//! 094 §"Multiple implementation steps" separates the registry foundation
//! from the conversion waves; this module was foundation-only through
//! Stage 2. **U22 is the first exception**: the conversion waves' first
//! item wires `record_login_failure` into `authn::session::
//! login_with_mfa`'s wrong-password branch (`sui-id-core`), replacing the
//! two-call, non-atomic pattern that command exists to fix. K01, U01, and
//! the Protocol/Operational proofs remain unwired.

use crate::StoreResult;
use crate::errors::StoreError;
use crate::registry::{
    ActorRequirement, AttributeSpec, AuditAttributes, AuditBuildError, AuditClass, AuditEventKind,
    AuditResult, AuditTarget, AuthorizedCommandContext, ClassATx, EventDescriptor,
    SealedCommandEvent, TargetRequirement,
};
use chrono::{DateTime, Utc};
use sui_id_shared::{
    FamilyId, RefreshTokenHash,
    ids::{
        ClientId, FederationLoginAttemptId, FederationProviderId, PendingChangeId,
        RegistrationTokenId, SessionId, SigningKeyId, UserId,
    },
};

// ── RFC 102 B4 — the gated action carries its authorization ─────────────

/// How recently a step-up counts as fresh (RFC 102 B4). The same window
/// the HTTP gate uses; `sui-id-core` re-exports it.
pub const STEP_UP_FRESHNESS_SECS: i64 = 300;

/// The `step_up` attribute every gated command's descriptor requires.
const STEP_UP_ATTRIBUTE: AttributeSpec = AttributeSpec {
    name: "step_up",
    description: "what authorized the action: \"fresh:<method>:<seconds>\", \"not_required:no_second_factor\", or \"not_applicable:system_principal\" (the CLI only)",
    required: true,
};

/// The only `step_up` value a system-principal entry records.
const STEP_UP_NOT_APPLICABLE: &str = "not_applicable:system_principal";

/// What a session-bound gated command found about its acting session, read
/// inside its own transaction (RFC 102 B4, M2). It has no
/// `not_applicable` form: only an entry that takes no session, and runs as
/// the system principal, records that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStepUpEvidence {
    /// The session stepped up with `method` `age_secs` seconds ago, within
    /// [`STEP_UP_FRESHNESS_SECS`].
    Fresh { method: String, age_secs: i64 },
    /// The acting user has no second factor, so no step-up was possible.
    NotRequired,
}

impl SessionStepUpEvidence {
    /// The attribute value.
    pub fn as_attribute(&self) -> String {
        match self {
            Self::Fresh { method, age_secs } => format!("fresh:{method}:{age_secs}"),
            Self::NotRequired => "not_required:no_second_factor".to_owned(),
        }
    }
}

/// Re-read the acting session inside a gated command's transaction and
/// derive its step-up evidence. The session must be live and the actor's
/// (otherwise `StepUpRequired`). A user with no second factor is
/// `NotRequired`; a user with one must have stepped up within
/// [`STEP_UP_FRESHNESS_SECS`] of `now`, or the command rolls back with
/// `StepUpRequired` (freshness lapsed between the gate and the commit).
fn session_step_up_evidence_within_tx(
    conn: &rusqlite::Connection,
    session_id: sui_id_shared::ids::SessionId,
    actor: UserId,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<SessionStepUpEvidence> {
    crate::repos::sessions::require_live_session_within_tx(conn, session_id, actor, now).map_err(
        |e| match e {
            crate::StoreError::NotFound => crate::StoreError::StepUpRequired,
            other => other,
        },
    )?;
    let has_second_factor: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM user_totp WHERE user_id = ?1 AND enabled = 1) \
         OR EXISTS(SELECT 1 FROM user_webauthn_credentials WHERE user_id = ?1)",
        [actor.to_string()],
        |r| r.get(0),
    )?;
    if !has_second_factor {
        return Ok(SessionStepUpEvidence::NotRequired);
    }
    let (last, method): (Option<chrono::DateTime<chrono::Utc>>, Option<String>) = conn.query_row(
        "SELECT last_step_up_at, last_step_up_method FROM sessions WHERE id = ?1",
        [session_id.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    match last {
        Some(at) if at <= now && (now - at).num_seconds() <= STEP_UP_FRESHNESS_SECS => {
            Ok(SessionStepUpEvidence::Fresh {
                // A step-up recorded before RFC 102 stage 3 has no method.
                method: method.unwrap_or_else(|| "unknown".to_owned()),
                age_secs: (now - at).num_seconds(),
            })
        }
        _ => Err(crate::StoreError::StepUpRequired),
    }
}

// ── K01 — signing-key rotation ──────────────────────────────────────────

static K01_ROTATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SigningKeyRotate,
    name: "signing_key.rotate",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "algorithm",
            description: "the new key's signing algorithm",
            required: false,
        },
        AttributeSpec {
            name: "reason",
            description: "operator-supplied reason for the rotation, if given",
            required: false,
        },
        STEP_UP_ATTRIBUTE,
    ],
};

crate::declare_write_command! {
    /// K01 — signing-key rotation. Its caller is the administrator's web
    /// rotation (`signing_keys_rotate`, gated by an admin session and a
    /// step-up), so the administrator is the actor; no CLI or scheduled
    /// rotation exists (RFC 102 stage 7).
    command K01 = "K01" {
        system_principal: forbidden;
        enum K01Event {
            Rotated {
                new_key: SigningKeyId,
                algorithm: String,
                reason: Option<String>,
                step_up: SessionStepUpEvidence,
            } => &K01_ROTATED,
        }
    }
}

impl SealedCommandEvent<K01> for K01Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Rotated { new_key, .. } = self;
        Some(AuditTarget(new_key.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Rotated {
            algorithm,
            reason,
            step_up,
            ..
        } = self;
        let mut builder = AuditAttributes::builder().attribute("algorithm", algorithm.clone());
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder.attribute("step_up", step_up.as_attribute()).build()
    }
}

/// Run K01 (signing-key rotation) through the Class-A runner, as the
/// administrator `admin` acting on `session_id` (RFC 102 B4: the step-up
/// evidence is computed from that session inside the transaction, judged
/// at the caller's `now`; RFC 102 stage 8: one clock for every command).
///
/// `private_key_sealed` is sealed by the caller *before* this is called
/// (RFC 094: crypto work stays outside the transaction).
#[allow(clippy::too_many_arguments)]
pub async fn rotate_signing_key(
    db: &crate::Database,
    admin: UserId,
    session_id: sui_id_shared::ids::SessionId,
    new_id: SigningKeyId,
    algorithm: String,
    private_key_sealed: Vec<u8>,
    public_key: Vec<u8>,
    reason: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<K01>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, K01>| {
        let step_up = session_step_up_evidence_within_tx(tx.tx(), session_id, admin, now)?;
        crate::repos::signing_keys::rotate_atomic_within_tx(
            tx.tx(),
            new_id,
            &algorithm,
            &private_key_sealed,
            &public_key,
            now,
        )?;
        Ok((
            (),
            K01Event::Rotated {
                new_key: new_id,
                algorithm,
                reason,
                step_up,
            },
        ))
    })
    .await
}

// ── U22 — login failure / lockout (closed branches) ─────────────────────

static U22_FAILURE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthLoginFailure,
    name: "auth.login.failure",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "count",
        description: "failed-login counter value after this attempt",
        required: false,
    }],
};

static U22_LOCKOUT: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthLockout,
    name: "auth.lockout",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "count",
            description: "failed-login counter value that crossed the threshold",
            required: false,
        },
        AttributeSpec {
            name: "locked_for_secs",
            description: "the lock window's length in seconds, as computed by the caller's \
                backoff policy",
            required: false,
        },
    ],
};

crate::declare_write_command! {
    /// U22 — record login failure, with the threshold-crossing branch.
    command U22 = "U22" {
        // Recording a login failure runs in response to an unauthenticated
        // request; there is no authorizing human actor to consume a
        // decision from (U22_FAILURE/U22_LOCKOUT's `actor: None` says the
        // same thing about the event payload — the target user is not the
        // authority for their own failure being recorded).
        system_principal: permitted;
        enum U22Event {
            Failure { user_id: UserId, count: i64 } => &U22_FAILURE,
            Lockout { user_id: UserId, count: i64, locked_for_secs: i64 } => &U22_LOCKOUT,
        }
    }
}

impl SealedCommandEvent<U22> for U22Event {
    fn target(&self) -> Option<AuditTarget> {
        match self {
            Self::Failure { user_id, .. } | Self::Lockout { user_id, .. } => {
                Some(AuditTarget(user_id.to_string()))
            }
        }
    }

    fn result(&self) -> AuditResult {
        // Both branches are the command *succeeding at its job* (recording
        // the failure) — `AuditResult` describes the audit record's own
        // outcome, not whether the login attempt itself succeeded.
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        match self {
            Self::Failure { count, .. } => AuditAttributes::builder()
                .attribute("count", count.to_string())
                .build(),
            Self::Lockout {
                count,
                locked_for_secs,
                ..
            } => AuditAttributes::builder()
                .attribute("count", count.to_string())
                .attribute("locked_for_secs", locked_for_secs.to_string())
                .build(),
        }
    }
}

/// Run U22 (record login failure, closed branch on threshold) through the
/// Class-A runner.
///
/// `lock_window_for_count` is the caller's lockout-backoff policy (e.g.
/// `authn::session::lockout_backoff`, which stays in `sui-id-core` — this
/// module doesn't reimplement domain policy). Called *inside* the
/// transaction with the freshly-incremented count, so the branch is
/// decided from the same guarded read the counter update used — this is
/// the atomic replacement for the current two-call, non-atomic pattern in
/// `authn::session::verify_password_login` (bump, then a second
/// best-effort call to stamp the lock if crossed).
pub async fn record_login_failure(
    db: &crate::Database,
    user_id: UserId,
    lock_window_for_count: impl Fn(i64) -> Option<chrono::TimeDelta> + Send + 'static,
) -> StoreResult<crate::registry::Audited<i64>> {
    let context = AuthorizedCommandContext::<U22>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U22>| {
        let new_count =
            crate::repos::users::record_login_failure_within_tx(tx.tx(), user_id, None)?;
        let event = match lock_window_for_count(new_count) {
            Some(window) => {
                let lock_until = chrono::Utc::now() + window;
                tx.tx().execute(
                    "UPDATE users SET locked_until = ?1 WHERE id = ?2",
                    rusqlite::params![lock_until, user_id.to_string()],
                )?;
                U22Event::Lockout {
                    user_id,
                    count: new_count,
                    locked_for_secs: window.num_seconds(),
                }
            }
            None => U22Event::Failure {
                user_id,
                count: new_count,
            },
        };
        Ok((new_count, event))
    })
    .await
}

// ── U01 — create user ───────────────────────────────────────────────────

static U01_CREATE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserCreate,
    name: "user.create",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// U01 — admin create user. **It creates no credential** (RFC 115 D4):
    /// nobody but the account holder ever chooses a password, so this
    /// command has no parameter that could carry one. The account is
    /// activated through an administrator-issued recovery link (U37).
    command U01 = "U01" {
        // Settled 2026-09-08, not provisional: this is "admin create
        // user", the coverage matrix requires `admin user id` as its
        // actor, and the descriptors above say `Required`. Forbidding
        // the system-principal adapter is what makes an admin-attributed
        // audit row for this command enforced rather than merely
        // documented — `for_system_actor::<U01>()` is now a compile
        // error (see `tests/compile_fail/
        // admin_command_forbidden_cannot_use_system_actor.rs`), and
        // `create_user` below uses `for_authorized_actor` instead.
        system_principal: forbidden;
        enum U01Event {
            Created { user_id: UserId } => &U01_CREATE,
        }
    }
}

impl SealedCommandEvent<U01> for U01Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Created { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run U01 (admin create user) through the Class-A runner. The user is
/// created **without a credential**: there is no parameter for one (RFC 115
/// D4), so a password at creation is a compile error, not a review finding.
/// `admin` is the authorizing actor's `UserId` — the caller is
/// responsible for it genuinely coming from a verified authorization
/// decision (RFC 094 §"Class-A transaction seam"; see
/// `AuthorizedCommandContext::for_authorized_actor`'s own doc comment
/// for the full caller-discipline contract).
pub async fn create_user(
    db: &crate::Database,
    admin: UserId,
    user: crate::models::UserRow,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U01>::for_authorized_actor(admin, None);
    let user_id = user.id;
    db.class_a(context, move |tx: &mut ClassATx<'_, U01>| {
        crate::repos::users::create_within_tx(tx.tx(), &user)?;
        Ok(((), U01Event::Created { user_id }))
    })
    .await
}

// ── U02 — disable user ───────────────────────────────────────────────

static U02_DISABLE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserDisable,
    name: "user.disable",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "reason",
            description: "operator-supplied reason for the disable, if given",
            required: false,
        },
        STEP_UP_ATTRIBUTE,
    ],
};

crate::declare_write_command! {
    /// U02 — admin disable user. Same `forbidden` reasoning as U01: the
    /// coverage matrix requires `admin user id` as the actor.
    command U02 = "U02" {
        system_principal: forbidden;
        enum U02Event {
            Disabled {
                user_id: UserId,
                reason: Option<String>,
                step_up: SessionStepUpEvidence,
            } => &U02_DISABLE,
        }
    }
}

impl SealedCommandEvent<U02> for U02Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Disabled { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Disabled {
            reason, step_up, ..
        } = self;
        let mut builder = AuditAttributes::builder();
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder.attribute("step_up", step_up.as_attribute()).build()
    }
}

/// Run U02 (admin disable user) through the Class-A runner. Revokes the
/// target's sessions, refresh tokens, and in-flight auth codes in the
/// same transaction as the `is_disabled` flip and the audit append —
/// previously three separate best-effort calls after an unguarded
/// mutation (RFC 094's motivating pattern).
pub async fn disable_user(
    db: &crate::Database,
    admin: UserId,
    session_id: sui_id_shared::ids::SessionId,
    target: UserId,
    reason: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U02>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U02>| {
        let step_up = session_step_up_evidence_within_tx(tx.tx(), session_id, admin, now)?;
        crate::repos::users::set_disabled_within_tx(tx.tx(), target, true)?;
        crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), target, now)?;
        crate::repos::refresh_tokens::revoke_all_for_user_within_tx(tx.tx(), target, now)?;
        crate::repos::auth_codes::invalidate_all_for_user_within_tx(tx.tx(), target)?;
        // RFC 103 D3: a disabled user's recovery links stop working.
        crate::repos::password_reset_tokens::revoke_outstanding_for_user_within_tx(
            tx.tx(),
            target,
            None,
            now,
        )?;
        Ok((
            (),
            U02Event::Disabled {
                user_id: target,
                reason,
                step_up,
            },
        ))
    })
    .await
}

// ── U03 — enable user ────────────────────────────────────────────────

static U03_ENABLE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserEnable,
    name: "user.enable",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[STEP_UP_ATTRIBUTE],
};

crate::declare_write_command! {
    /// U03 — admin re-enable user.
    command U03 = "U03" {
        system_principal: forbidden;
        enum U03Event {
            Enabled { user_id: UserId, step_up: SessionStepUpEvidence } => &U03_ENABLE,
        }
    }
}

impl SealedCommandEvent<U03> for U03Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Enabled { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Enabled { step_up, .. } = self;
        AuditAttributes::builder()
            .attribute("step_up", step_up.as_attribute())
            .build()
    }
}

/// Run U03 (admin re-enable user) through the Class-A runner. Unlike
/// U02, re-enabling does not revoke anything — the coverage matrix
/// carries no note field for this row, matching the pre-conversion
/// behavior of `set_user_disabled(disabled: false)`.
pub async fn enable_user(
    db: &crate::Database,
    admin: UserId,
    session_id: sui_id_shared::ids::SessionId,
    target: UserId,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U03>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U03>| {
        let step_up = session_step_up_evidence_within_tx(tx.tx(), session_id, admin, now)?;
        crate::repos::users::set_disabled_within_tx(tx.tx(), target, false)?;
        Ok((
            (),
            U03Event::Enabled {
                user_id: target,
                step_up,
            },
        ))
    })
    .await
}

// ── U04 — soft-delete user ───────────────────────────────────────────

static U04_DELETE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserDelete,
    name: "user.delete",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "reason",
            description: "operator-supplied reason for the deletion, if given",
            required: false,
        },
        STEP_UP_ATTRIBUTE,
    ],
};

crate::declare_write_command! {
    /// U04 — admin soft-delete user.
    command U04 = "U04" {
        system_principal: forbidden;
        enum U04Event {
            Deleted {
                user_id: UserId,
                reason: Option<String>,
                step_up: SessionStepUpEvidence,
            } => &U04_DELETE,
        }
    }
}

impl SealedCommandEvent<U04> for U04Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Deleted { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Deleted {
            reason, step_up, ..
        } = self;
        let mut builder = AuditAttributes::builder();
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder.attribute("step_up", step_up.as_attribute()).build()
    }
}

/// Run U04 (admin soft-delete user) through the Class-A runner. Same
/// revocation-bundling reasoning as U02.
pub async fn delete_user(
    db: &crate::Database,
    admin: UserId,
    session_id: sui_id_shared::ids::SessionId,
    target: UserId,
    reason: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U04>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U04>| {
        let step_up = session_step_up_evidence_within_tx(tx.tx(), session_id, admin, now)?;
        crate::repos::users::soft_delete_within_tx(tx.tx(), target)?;
        crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), target, now)?;
        crate::repos::refresh_tokens::revoke_all_for_user_within_tx(tx.tx(), target, now)?;
        crate::repos::auth_codes::invalidate_all_for_user_within_tx(tx.tx(), target)?;
        // RFC 103 D3: a deleted user's recovery links stop working.
        crate::repos::password_reset_tokens::revoke_outstanding_for_user_within_tx(
            tx.tx(),
            target,
            None,
            now,
        )?;
        Ok((
            (),
            U04Event::Deleted {
                user_id: target,
                reason,
                step_up,
            },
        ))
    })
    .await
}

// ── U05 — admin role change ──────────────────────────────────────────
//
// The matrix row for `user.role_change` and this descriptor agree: admin user
// id as actor, target user id as target, `old_role` and `new_role` as
// attributes, Class A. `users_set_role` (`sui-id/src/http/handlers/admin/users.rs`)
// runs this command through `commands::change_user_role`, so the mutation and
// its audit row commit in one transaction. The attribute pair was this
// implementation's proposal: a role-change record with neither value is not
// reconstructable from the rest of the audit log.
static U05_ROLE_CHANGE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserRoleChange,
    name: "user.role_change",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "old_role",
            description: "the user's role before the change",
            required: false,
        },
        AttributeSpec {
            name: "new_role",
            description: "the user's role after the change",
            required: false,
        },
    ],
};

crate::declare_write_command! {
    /// U05 — admin role change.
    command U05 = "U05" {
        system_principal: forbidden;
        enum U05Event {
            RoleChanged {
                user_id: UserId,
                old_role: crate::models::Role,
                new_role: crate::models::Role,
            } => &U05_ROLE_CHANGE,
        }
    }
}

impl SealedCommandEvent<U05> for U05Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::RoleChanged { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::RoleChanged {
            old_role, new_role, ..
        } = self;
        AuditAttributes::builder()
            .attribute("old_role", old_role.as_str())
            .attribute("new_role", new_role.as_str())
            .build()
    }
}

/// Run U05 (admin role change) through the Class-A runner.
///
/// The last-admin guard is split per RFC 094's own vocabulary
/// (`migration-checklist.md`'s per-command checklist): the *authorization*
/// check ("is the caller allowed to change roles at all") is non-racy and
/// stays in `sui-id-core`, before this is called. The *last-admin count*
/// is racy state — concurrent role changes can both observe "2 admins
/// left" and both demote — so it is re-read and re-checked here, inside
/// the same transaction that performs the demotion, using `old_role` read
/// from this transaction rather than a value the caller resolved earlier.
/// This function does not call `sui-id-core`'s `authz` module directly:
/// `sui-id-store` cannot depend on `sui-id-core` (RFC 094's standing
/// constraint), so the specific invariant ("would this demotion leave
/// zero admins") is inlined here rather than routed through the general
/// decision table. `StoreError::Conflict` on the guard failing is
/// intentionally generic — the friendly, localized error message is
/// still produced by the pre-check in `sui-id-core`; this is the rare
/// race the pre-check cannot close.
pub async fn change_user_role(
    db: &crate::Database,
    admin: UserId,
    target: UserId,
    new_role: crate::models::Role,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U05>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U05>| {
        let old_role = crate::repos::users::get_role_within_tx(tx.tx(), target)?;
        if old_role.is_admin() && !new_role.is_admin() {
            let admins = crate::repos::users::count_admins_within_tx(tx.tx())?;
            if admins <= 1 {
                return Err(crate::StoreError::Conflict);
            }
        }
        crate::repos::users::set_role_within_tx(tx.tx(), target, new_role)?;
        Ok((
            (),
            U05Event::RoleChanged {
                user_id: target,
                old_role,
                new_role,
            },
        ))
    })
    .await
}

// ── U07 — admin MFA reset ────────────────────────────────────────────
//
// Event name settled 2026-09-09 (`.git-exclude/reviewed/
// 094-wave-b-u06-dispatch-u07-blocked-2026-09-09.md` §2): keep
// `mfa.admin_reset`, the name production has emitted since RFC 060, over
// `command-inventory.md`'s original `user.reset_mfa` — four deployed
// operator alerting queries (one under an explicit "Alert on this"
// callout) depend on the shipped name; nothing depended on the other one,
// because nothing ever emitted it.

static U07_ADMIN_RESET: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::MfaAdminReset,
    name: "mfa.admin_reset",
    class: AuditClass::Atomic,
    // Required on the web path; absent when the operator CLI resets a user
    // who has no administrator left to act (RFC 103 D12).
    actor: ActorRequirement::Optional,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "totp",
            description: "whether a TOTP enrollment was removed (\"removed\" or \"absent\")",
            required: false,
        },
        AttributeSpec {
            name: "passkeys",
            description: "number of WebAuthn credentials removed",
            required: false,
        },
        AttributeSpec {
            name: "reason",
            description: "operator-supplied reason for the reset, if given",
            required: false,
        },
        AttributeSpec {
            name: "via",
            description: "\"cli\" when `sui-id admin reset-mfa` issued the reset; absent for the web path",
            required: false,
        },
        STEP_UP_ATTRIBUTE,
    ],
};

crate::declare_write_command! {
    /// U07 — MFA reset. The web path runs as the signed-in administrator
    /// (`admin_reset_mfa`). RFC 103 D12 adds the operator CLI
    /// (`operator_reset_mfa`), whose authority is the host's master key,
    /// not a session: it has no actor, so U07 permits the system principal.
    command U07 = "U07" {
        system_principal: permitted;
        enum U07Event {
            /// The web reset: an administrator's session, with its step-up
            /// evidence.
            Reset {
                user_id: UserId,
                totp_removed: bool,
                passkeys_removed: usize,
                reason: Option<String>,
                step_up: SessionStepUpEvidence,
            } => &U07_ADMIN_RESET,
            /// The operator CLI: no session, the system principal. The only
            /// event in this crate that records `not_applicable`.
            OperatorReset {
                user_id: UserId,
                totp_removed: bool,
                passkeys_removed: usize,
                reason: String,
            } => &U07_ADMIN_RESET,
        }
    }
}

impl SealedCommandEvent<U07> for U07Event {
    fn target(&self) -> Option<AuditTarget> {
        let (Self::Reset { user_id, .. } | Self::OperatorReset { user_id, .. }) = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let removed = |totp: bool, passkeys: usize| {
            AuditAttributes::builder()
                .attribute("totp", if totp { "removed" } else { "absent" })
                .attribute("passkeys", passkeys.to_string())
        };
        match self {
            Self::Reset {
                totp_removed,
                passkeys_removed,
                reason,
                step_up,
                ..
            } => {
                let mut builder = removed(*totp_removed, *passkeys_removed);
                if let Some(r) = reason {
                    builder = builder.attribute("reason", r.clone());
                }
                builder.attribute("step_up", step_up.as_attribute()).build()
            }
            Self::OperatorReset {
                totp_removed,
                passkeys_removed,
                reason,
                ..
            } => removed(*totp_removed, *passkeys_removed)
                .attribute("reason", reason.clone())
                .attribute("via", "cli")
                .attribute("step_up", STEP_UP_NOT_APPLICABLE)
                .build(),
        }
    }
}

/// Run U07 (admin MFA reset) through the Class-A runner: reads the
/// target's current MFA factors, removes all of them, and appends
/// `mfa.admin_reset` in one transaction — replacing the previous
/// unguarded delete-then-delete-then-fire-and-forget-append sequence
/// (which also bypassed this crate's own `AuthorizedCommandContext`
/// entirely, appending via a raw `AuditLogRow` with no descriptor
/// backing it at all).
///
/// The existence check the caller used to do outside any transaction
/// (`users::get(db, target)`, discarding the row, purely to distinguish
/// "no such user" from "user with nothing to reset") is folded into the
/// transaction via `get_role_within_tx` — reused here only as a cheap
/// existence probe, its returned role is not otherwise used.
///
/// **Behavior change, deliberate, not inherited from the helper's other
/// caller.** `users::get` (the pre-conversion check) does not filter on
/// `is_deleted`, so a reset against a soft-deleted user used to succeed.
/// `get_role_within_tx` (U05's helper, reused here) does filter on
/// `is_deleted = 0`, so the same call now returns `NotFound`. Found in
/// review (2026-09-09) — the query shapes are not equivalent, and this
/// candidate had described them as if they were. Keeping the stricter
/// behavior: resetting MFA on a deleted account is meaningless, and
/// rejecting it is more defensible than silently proceeding. See
/// `u07_reset_of_soft_deleted_user_returns_not_found` for the proof.
///
/// Sessions are deliberately **not** revoked (matches the pre-conversion
/// behavior): an MFA reset restores login capability rather than forcing
/// a logout, so an operator who wants both runs `user.disable` /
/// `user.enable` as well, same as before this conversion.
pub async fn admin_reset_mfa(
    db: &crate::Database,
    admin: UserId,
    session_id: sui_id_shared::ids::SessionId,
    target: UserId,
    reason: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<crate::registry::Audited<(bool, usize)>> {
    let context = AuthorizedCommandContext::<U07>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U07>| {
        let step_up = session_step_up_evidence_within_tx(tx.tx(), session_id, admin, now)?;
        let (totp_removed, passkeys_removed) = remove_mfa_within_tx(tx, target)?;
        Ok((
            (totp_removed, passkeys_removed),
            U07Event::Reset {
                user_id: target,
                totp_removed,
                passkeys_removed,
                reason,
                step_up,
            },
        ))
    })
    .await
}

/// Run U07 for the operator CLI, `sui-id admin reset-mfa` (RFC 103 D12):
/// the same removal as [`admin_reset_mfa`], with no actor, `via = cli` and
/// `step_up = not_applicable:system_principal`. It takes no session, so no
/// step-up evidence exists to compute; only the CLI adapter calls it, and
/// the web path must never reach the system principal.
pub async fn operator_reset_mfa(
    db: &crate::Database,
    target: UserId,
    reason: String,
) -> StoreResult<crate::registry::Audited<(bool, usize)>> {
    let context = AuthorizedCommandContext::<U07>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U07>| {
        let (totp_removed, passkeys_removed) = remove_mfa_within_tx(tx, target)?;
        Ok((
            (totp_removed, passkeys_removed),
            U07Event::OperatorReset {
                user_id: target,
                totp_removed,
                passkeys_removed,
                reason,
            },
        ))
    })
    .await
}

fn remove_mfa_within_tx(tx: &mut ClassATx<'_, U07>, target: UserId) -> StoreResult<(bool, usize)> {
    // Existence probe only -- the role itself is unused here. Also
    // deliberately excludes soft-deleted users (`is_deleted = 0` in
    // get_role_within_tx's WHERE clause): resetting MFA on a deleted
    // account is meaningless, so a soft-deleted target now returns
    // NotFound where the pre-conversion `users::get`-based check
    // would have let it through. See `admin_reset_mfa`'s doc comment.
    crate::repos::users::get_role_within_tx(tx.tx(), target)?;

    let totp_removed = crate::repos::user_totp::delete_within_tx(tx.tx(), target)?;

    let creds = crate::repos::user_webauthn_credentials::list_for_user_within_tx(tx.tx(), target)?;
    let passkeys_removed = creds.len();
    for c in &creds {
        crate::repos::user_webauthn_credentials::delete_within_tx(tx.tx(), c.id, target)?;
    }

    Ok((totp_removed, passkeys_removed))
}

// ── U08 — CLI operator unlock ────────────────────────────────────────
//
// `system_principal: permitted`, `ActorRequirement::None` — decided
// 2026-09-09 (`.git-exclude/reviewed/
// 094-wave-b-scoping-u08-blocked-2026-09-09.md` §4(a), applied in
// `migration-checklist.md`). The only caller is `sui-id admin
// unlock-user` (`cli.rs::run_admin_unlock_user`), whose authority is
// possession of the master key and filesystem access, not an
// authenticated user — it cannot supply an actor, so unlike U01-U07 this
// command uses the sealed system-authority adapter (`for_system_actor`),
// the same mechanism K01 and T04 use for their own non-human callers.
//
// U08 covers `users::admin_unlock` only. The automatic counter reset on a
// successful sign-in is a distinct thing, not an alternate branch of this
// command; the two share near-identical SQL but opposite security meanings,
// and conflating them was the second half of the blocking finding. (Written
// when that reset was `users::clear_lockout`; that function now has no
// production caller. The resets that run are L01, L02 and L03, and U09 and
// U10 clear the password lockout only, RFC 118.)

static U08_UNLOCK: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AdminUserUnlock,
    name: "admin.user.unlock",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// U08 — CLI operator unlock.
    command U08 = "U08" {
        system_principal: permitted;
        enum U08Event {
            Unlocked { user_id: UserId } => &U08_UNLOCK,
        }
    }
}

impl SealedCommandEvent<U08> for U08Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Unlocked { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run U08 (CLI operator unlock) through the Class-A runner: the failure
/// counter and lock timestamp reset, and the `admin.user.unlock` audit
/// append, now commit in one transaction — replacing the previous
/// unguarded `users::admin_unlock` followed by a fire-and-forget raw
/// `audit::append` call in `cli.rs` that bypassed this crate's registry
/// entirely (no descriptor, no `AuthorizedCommandContext`).
///
/// The coverage matrix's declared note fields for this row are empty —
/// the pre-conversion CLI code embedded the looked-up username in a
/// free-text note (`"unlocked via command line for username={username}"`),
/// which this drops: the target column already records the unlocked
/// user's id, and the matrix specifies no note field for this event.
/// Disclosed rather than silently kept or silently dropped.
pub async fn admin_unlock_user(
    db: &crate::Database,
    target: UserId,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U08>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U08>| {
        crate::repos::users::admin_unlock_within_tx(tx.tx(), target)?;
        Ok(((), U08Event::Unlocked { user_id: target }))
    })
    .await
}

// ── U09 — self password change ───────────────────────────────────────

static U09_CHANGED_SELF: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthPasswordChangedSelf,
    name: "auth.password.changed_self",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "sessions_revoked",
            description: "count of other sessions revoked by this change",
            required: false,
        },
        AttributeSpec {
            name: "refresh_tokens_revoked",
            description: "count of refresh tokens revoked by this change",
            required: false,
        },
        AttributeSpec {
            name: "lockout_cleared",
            description: "the password failure count this change cleared, present only when a counter was non-zero or a lock was live (RFC 118 D5); the second-factor lock and count are never cleared",
            required: false,
        },
    ],
};

crate::declare_write_command! {
    /// U09 — self-service password change. `forbidden`: this is always a
    /// specific authenticated user acting on their own account, never a
    /// system/CLI caller — same reasoning as U01-U08, just a self-service
    /// actor rather than an admin one. `for_authorized_actor` doesn't
    /// distinguish the two; both are "a verified authorization decision
    /// for this `UserId`" (see its own doc comment).
    command U09 = "U09" {
        system_principal: forbidden;
        enum U09Event {
            Changed {
                user_id: UserId,
                sessions_revoked: usize,
                refresh_tokens_revoked: usize,
                lockout_cleared: Option<i64>,
            } => &U09_CHANGED_SELF,
        }
    }
}

impl SealedCommandEvent<U09> for U09Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Changed { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Changed {
            sessions_revoked,
            refresh_tokens_revoked,
            lockout_cleared,
            ..
        } = self;
        let mut builder = AuditAttributes::builder()
            .attribute("sessions_revoked", sessions_revoked.to_string())
            .attribute("refresh_tokens_revoked", refresh_tokens_revoked.to_string());
        if let Some(count) = lockout_cleared {
            builder = builder.attribute("lockout_cleared", count.to_string());
        }
        builder.build()
    }
}

/// Run U09 (self-service password change) through the Class-A runner.
/// `credential` is the already-hashed replacement row — same
/// outside-the-transaction contract as U01. When `revoke_others` is
/// false, the sweep counts are always `(0, 0)`: this mirrors the
/// pre-conversion behavior exactly rather than emitting a zero-effort
/// audit note that implies a sweep was attempted and found nothing.
pub async fn change_password_self(
    db: &crate::Database,
    user_id: UserId,
    credential: crate::models::CredentialRow,
    keep_current_session: Option<sui_id_shared::ids::SessionId>,
    revoke_others: bool,
) -> StoreResult<crate::registry::Audited<(usize, usize)>> {
    let context = AuthorizedCommandContext::<U09>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U09>| {
        crate::repos::credentials::upsert_within_tx(tx.tx(), &credential)?;
        let now = chrono::Utc::now();
        // RFC 118 D1, D3: the password lockout does not outlive the credential
        // change that made it moot; the second-factor lock is not touched.
        let lockout = crate::repos::users::clear_password_lockout_within_tx(tx.tx(), user_id, now)?;
        // RFC 103 D3: a password change invalidates outstanding reset links,
        // whether or not the other sessions are being swept.
        crate::repos::password_reset_tokens::revoke_outstanding_for_user_within_tx(
            tx.tx(),
            user_id,
            None,
            now,
        )?;
        let (sessions_revoked, refresh_tokens_revoked) = if revoke_others {
            let sessions_revoked = match keep_current_session {
                Some(keep) => crate::repos::sessions::revoke_all_for_user_except_within_tx(
                    tx.tx(),
                    user_id,
                    keep,
                    now,
                )?,
                None => {
                    crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), user_id, now)?
                }
            };
            let refresh_tokens_revoked =
                crate::repos::refresh_tokens::revoke_all_for_user_within_tx(tx.tx(), user_id, now)?;
            (sessions_revoked, refresh_tokens_revoked)
        } else {
            (0, 0)
        };
        Ok((
            (sessions_revoked, refresh_tokens_revoked),
            U09Event::Changed {
                user_id,
                sessions_revoked,
                refresh_tokens_revoked,
                lockout_cleared: lockout.attribute(),
            },
        ))
    })
    .await
}

// ── U10 — forgot-password completion ─────────────────────────────────

static U10_RESET_COMPLETED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthPasswordResetCompleted,
    name: "auth.password.reset_completed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "origin",
            description: "where the consumed token came from: \"email\" (forgot-password), \"web\" (an administrator) or \"cli\" (the operator)",
            required: false,
        },
        AttributeSpec {
            name: "hibp",
            description: "\"warned\" when the new password appears in a known breach and the deployment's HIBP mode is `warn` (RFC 115 D9); absent otherwise",
            required: false,
        },
        AttributeSpec {
            name: "lockout_cleared",
            description: "the password failure count this change cleared, present only when a counter was non-zero or a lock was live (RFC 118 D5); the second-factor lock and count are never cleared",
            required: false,
        },
    ],
};

crate::declare_write_command! {
    /// U10 — forgot-password (token-based) completion. `permitted`,
    /// `ActorRequirement::None`: the presenter is authorized by
    /// possession of the one-time reset token, not by an authenticated
    /// session — there is no `UserId` a verified authorization decision
    /// could name, the same shape as T04's refresh-token presenter, not
    /// U01-U09's authenticated actor.
    command U10 = "U10" {
        system_principal: permitted;
        enum U10Event {
            Completed {
                user_id: UserId,
                origin: crate::models::ResetTokenOrigin,
                hibp_warned: bool,
                lockout_cleared: Option<i64>,
            } => &U10_RESET_COMPLETED,
        }
    }
}

impl SealedCommandEvent<U10> for U10Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Completed { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Completed {
            origin,
            hibp_warned,
            lockout_cleared,
            ..
        } = self;
        let mut builder = AuditAttributes::builder().attribute("origin", origin.as_str());
        if *hibp_warned {
            builder = builder.attribute("hibp", "warned");
        }
        if let Some(count) = lockout_cleared {
            builder = builder.attribute("lockout_cleared", count.to_string());
        }
        builder.build()
    }
}

/// Run U10 (forgot-password completion) through the Class-A runner.
/// `hibp_warned` is the caller's already-decided outcome of the breach check
/// on the new password in `warn` mode (RFC 115 D9): the event records it, so
/// a breached password the deployment chose to allow is no longer silent.
///
/// The credential swap, token consume, and session/refresh-token revocation
/// were already atomic before this candidate (a raw `db.with_tx` block
/// in `forgot_password.rs`) — RFC 094's actual gap here was the audit
/// event, appended separately and afterward via `events::emit`, whose
/// own doc comment says failure "does not propagate." This folds that
/// append into the same transaction as everything else, using the
/// registry's sealed capability instead of a raw `with_tx` closure.
pub async fn consume_and_reset_password(
    db: &crate::Database,
    user_id: UserId,
    token_id: sui_id_shared::ids::PasswordResetTokenId,
    credential: crate::models::CredentialRow,
    consumed_at: chrono::DateTime<chrono::Utc>,
    hibp_warned: bool,
) -> StoreResult<crate::registry::Audited<crate::repos::users::PasswordLockoutCleared>> {
    let context = AuthorizedCommandContext::<U10>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U10>| {
        // RFC 103 D13: consume the token once (guarded), and re-read the
        // user inside this transaction. Anything that made the token
        // ineligible since it was read — consumed, expired, the user
        // disabled, deleted or not a local account — rolls back with
        // `NotFound` and writes no credential.
        crate::repos::password_reset_tokens::mark_consumed_within_tx(
            tx.tx(),
            token_id,
            consumed_at,
        )?;
        if !crate::repos::users::is_active_local_within_tx(tx.tx(), user_id)? {
            return Err(crate::StoreError::NotFound);
        }
        // RFC 103 U10 `origin`: read from the token this completion consumed.
        let origin = crate::repos::password_reset_tokens::origin_within_tx(tx.tx(), token_id)?;
        crate::repos::credentials::upsert_within_tx(tx.tx(), &credential)?;
        // RFC 118 D1, D3, D4: clear the password lockout in this transaction,
        // and keep what was found, read before the clear, for the completion
        // response. A lock the second-factor lockout may have set stays.
        let lockout =
            crate::repos::users::clear_password_lockout_within_tx(tx.tx(), user_id, consumed_at)?;
        crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), user_id, consumed_at)?;
        crate::repos::refresh_tokens::revoke_all_for_user_within_tx(tx.tx(), user_id, consumed_at)?;
        // RFC 103 D3: completing one link invalidates the user's other
        // outstanding links.
        crate::repos::password_reset_tokens::revoke_outstanding_for_user_within_tx(
            tx.tx(),
            user_id,
            Some(token_id),
            consumed_at,
        )?;
        Ok((
            lockout,
            U10Event::Completed {
                user_id,
                origin,
                hibp_warned,
                lockout_cleared: lockout.attribute(),
            },
        ))
    })
    .await
}

// ── U37 — issue a recovery link (RFC 103 D2, D3, D5, D6, D8, D9) ────────

static U37_ISSUED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserRecoveryLinkIssued,
    name: "user.recovery_link.issued",
    class: AuditClass::Atomic,
    // The issuing administrator on the web; none for the operator CLI.
    actor: ActorRequirement::Optional,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "reason",
            description: "the operator's recorded reason for issuing the link",
            required: false,
        },
        AttributeSpec {
            name: "via",
            description: "\"web\" (an administrator) or \"cli\" (the operator)",
            required: false,
        },
        AttributeSpec {
            name: "expires_at",
            description: "when the link stops working (RFC 3339, UTC)",
            required: false,
        },
        AttributeSpec {
            name: "invalidated",
            description: "how many of the target's outstanding links this issuance invalidated",
            required: false,
        },
        AttributeSpec {
            name: "provisioning",
            description: "\"1\" when the link was issued for a just-created account and counted against the provisioning ceiling (RFC 115 D11); absent otherwise",
            required: false,
        },
        STEP_UP_ATTRIBUTE,
    ],
};

/// Most recovery links one administrator, or the operator CLI, may issue in
/// a rolling hour (RFC 103 D8).
pub const RECOVERY_LINKS_PER_HOUR: i64 = 5;

/// Most *provisioning* links one administrator, or the operator CLI, may
/// issue in a rolling hour (RFC 115 D11). A link is provisioning when U37
/// reads its target, inside its own transaction, as a local non-administrator
/// account that has never held a credential, never signed in and never had a
/// token of any kind. Provisioning is not free: a bulk import is possible and
/// a runaway is not. The number is an implementer's choice, larger than
/// [`RECOVERY_LINKS_PER_HOUR`] and small enough that an unattended loop is
/// stopped within minutes; the architect may set it.
pub const PROVISIONING_LINKS_PER_HOUR: i64 = 50;

// "Its own, larger ceiling" (RFC 115 D11), enforced at compile time: a ceiling
// that fell back to the ordinary five would make bulk provisioning impossible
// again, and this fails the build instead of a test.
const _: () = assert!(PROVISIONING_LINKS_PER_HOUR > RECOVERY_LINKS_PER_HOUR);

crate::declare_write_command! {
    /// U37 — issue a recovery link. Two entries: the administrator's web
    /// operation (`issue_recovery_link_as_admin`, an actor and a session)
    /// and the operator CLI (`issue_recovery_link_as_operator`, the system
    /// principal, no session), so U37 permits the system principal, as U07
    /// does. The event never carries the token or its hash.
    command U37 = "U37" {
        system_principal: permitted;
        enum U37Event {
            /// Issued by an administrator on the web, with the session's
            /// step-up evidence.
            Issued {
                target: UserId,
                reason: String,
                expires_at: chrono::DateTime<chrono::Utc>,
                invalidated: usize,
                provisioning: bool,
                step_up: SessionStepUpEvidence,
            } => &U37_ISSUED,
            /// Issued by the operator CLI: no session, the system
            /// principal. The only U37 event that records `not_applicable`.
            OperatorIssued {
                target: UserId,
                reason: String,
                expires_at: chrono::DateTime<chrono::Utc>,
                invalidated: usize,
                provisioning: bool,
            } => &U37_ISSUED,
        }
    }
}

impl SealedCommandEvent<U37> for U37Event {
    fn target(&self) -> Option<AuditTarget> {
        let (Self::Issued { target, .. } | Self::OperatorIssued { target, .. }) = self;
        Some(AuditTarget(target.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let expires = |at: &chrono::DateTime<chrono::Utc>| {
            at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        };
        match self {
            Self::Issued {
                reason,
                expires_at,
                invalidated,
                provisioning,
                step_up,
                ..
            } => {
                let builder = AuditAttributes::builder()
                    .attribute("reason", reason.clone())
                    .attribute("via", "web")
                    .attribute("expires_at", expires(expires_at))
                    .attribute("invalidated", invalidated.to_string());
                let builder = if *provisioning {
                    builder.attribute("provisioning", "1")
                } else {
                    builder
                };
                builder.attribute("step_up", step_up.as_attribute()).build()
            }
            Self::OperatorIssued {
                reason,
                expires_at,
                invalidated,
                provisioning,
                ..
            } => {
                let builder = AuditAttributes::builder()
                    .attribute("reason", reason.clone())
                    .attribute("via", "cli")
                    .attribute("expires_at", expires(expires_at))
                    .attribute("invalidated", invalidated.to_string());
                let builder = if *provisioning {
                    builder.attribute("provisioning", "1")
                } else {
                    builder
                };
                builder.attribute("step_up", STEP_UP_NOT_APPLICABLE).build()
            }
        }
    }
}

/// What U37 issued. The plaintext token is not here: it never reaches the
/// store, only its hash does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryLinkGrant {
    pub token_id: sui_id_shared::ids::PasswordResetTokenId,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// How many of the target's outstanding links this issuance invalidated.
    pub invalidated: usize,
}

fn refuse(why: crate::errors::RecoveryRefusal) -> crate::StoreError {
    crate::StoreError::RecoveryRefused(why)
}

/// The longest recovery reason U37 accepts, in characters, after trimming.
/// The confirm screen's field carries the same bound. It is not the only
/// bound: the reason is also capped at the audit attribute's byte limit
/// (`registry::MAX_ATTRIBUTE_VALUE_BYTES`, 512), because 200 characters of a
/// script with three-byte characters (Japanese, Chinese) is 600 bytes and
/// would otherwise reach that limit and fail as a generic storage error.
pub const RECOVERY_REASON_MAX_CHARS: usize = 200;

/// RFC 103 stage 3 ruling 3: the reason is free text written into a
/// `key=value` audit note whose values are not escaped, and attribute values
/// are bounded at 512 bytes. So it is trimmed, must not be empty, must not
/// exceed [`RECOVERY_REASON_MAX_CHARS`] characters or the attribute byte
/// bound (whichever is reached first), and must not contain an
/// ASCII control character (a newline, tab or escape). Each is a typed
/// refusal, raised before any write, so an over-long reason never reaches the
/// attribute bound and fails as a generic storage error. Both U37 entries
/// call this, so every caller gets the same rule.
fn validated_recovery_reason(reason: &str) -> StoreResult<String> {
    use crate::errors::RecoveryRefusal as Why;
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(refuse(Why::ReasonRequired));
    }
    if reason.chars().count() > RECOVERY_REASON_MAX_CHARS
        || reason.len() > crate::registry::MAX_ATTRIBUTE_VALUE_BYTES
    {
        return Err(refuse(Why::ReasonTooLong));
    }
    if reason.chars().any(|c| c.is_ascii_control()) {
        return Err(refuse(Why::ReasonHasControlCharacters));
    }
    Ok(reason.to_owned())
}

/// RFC 103 D5, read inside U37's transaction: who a recovery link may be
/// issued for. `admin_issuer` is `Some` on the web (which also refuses the
/// issuer and any administrator) and `None` on the CLI (whose authority is
/// not an administrator session).
fn check_recovery_target(
    conn: &rusqlite::Connection,
    target: UserId,
    admin_issuer: Option<UserId>,
) -> StoreResult<crate::repos::users::RecoveryTarget> {
    use crate::errors::RecoveryRefusal as Why;
    let t = crate::repos::users::recovery_target_within_tx(conn, target).map_err(|e| match e {
        crate::StoreError::NotFound => refuse(Why::TargetUnknown),
        other => other,
    })?;
    if let Some(admin) = admin_issuer {
        if admin == target {
            return Err(refuse(Why::TargetIsSelf));
        }
        // RFC 115 D10: an administrator target is refused only when it is a
        // live account. One that has never held a credential and never
        // signed in is being activated, not captured.
        if t.role.is_admin() && !t.never_activated {
            return Err(refuse(Why::TargetIsAdmin));
        }
    }
    if t.is_deleted {
        return Err(refuse(Why::TargetDeleted));
    }
    if t.is_disabled {
        return Err(refuse(Why::TargetDisabled));
    }
    if !t.source.is_local() {
        return Err(refuse(Why::TargetNonLocal));
    }
    Ok(t)
}

/// RFC 115 D11: is a link for `t` a *provisioning* link? Decided from the
/// target's state read in this same transaction, never from anything the
/// caller says: a local, **non-administrator** account that has never held a
/// credential, never signed in, and has never had a token of any kind. A live
/// account can therefore never qualify (it has a credential), an account that
/// already had a link can never qualify a second time, and an administrator
/// is never exempt: an unthrottled stream of new administrator accounts is the
/// persistence primitive a stolen session with a fresh step-up would want.
/// Creating a user to obtain an unthrottled issuance yields a link only to the
/// account just created.
fn is_provisioning(t: &crate::repos::users::RecoveryTarget) -> bool {
    t.source.is_local() && !t.role.is_admin() && t.never_activated && !t.has_any_token
}

/// Revoke the target's outstanding links and insert the new one: the shared
/// tail of both U37 entries.
#[allow(clippy::too_many_arguments)]
fn revoke_and_insert_recovery_token(
    conn: &rusqlite::Connection,
    target: UserId,
    token_hash: Vec<u8>,
    via: crate::models::ResetTokenOrigin,
    issued_by: Option<UserId>,
    provisioning: bool,
    expires_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<RecoveryLinkGrant> {
    let invalidated = crate::repos::password_reset_tokens::revoke_outstanding_for_user_within_tx(
        conn, target, None, now,
    )?;
    let token_id = sui_id_shared::ids::PasswordResetTokenId::new();
    crate::repos::password_reset_tokens::insert_with_within_tx(
        conn,
        &crate::models::PasswordResetTokenRow {
            id: token_id,
            user_id: target,
            token_hash,
            issued_at: now,
            expires_at,
            consumed_at: None,
            requester_ip: None,
            issued_via: via,
            issued_by,
            revoked_at: None,
        },
        provisioning,
    )?;
    Ok(RecoveryLinkGrant {
        token_id,
        expires_at,
        invalidated,
    })
}

/// Run U37 for an administrator on the web (RFC 103 D5, D6, D8, D9). In one
/// transaction: the acting session's step-up evidence is computed and must
/// be `fresh` (D6: an administrator with no second factor is refused with
/// `StepUpRequired`, as is a lapsed step-up); the target is re-read and
/// refused if it is the issuer, an administrator, deleted, disabled or not
/// local (D5); the issuer's hourly limit is counted from the database (D8);
/// then every outstanding link of the target is revoked (D3), the new
/// token's hash is inserted, and `user.recovery_link.issued` is written.
/// Every refusal writes nothing.
///
/// `token_hash` is the SHA-256 of a token generated outside the transaction;
/// the plaintext never reaches this function.
#[allow(clippy::too_many_arguments)]
pub async fn issue_recovery_link_as_admin(
    db: &crate::Database,
    admin: UserId,
    session_id: sui_id_shared::ids::SessionId,
    target: UserId,
    token_hash: Vec<u8>,
    reason: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<crate::registry::Audited<RecoveryLinkGrant>> {
    use crate::errors::RecoveryRefusal as Why;
    let context = AuthorizedCommandContext::<U37>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U37>| {
        let reason = validated_recovery_reason(&reason)?;
        let step_up = session_step_up_evidence_within_tx(tx.tx(), session_id, admin, now)?;
        if !matches!(step_up, SessionStepUpEvidence::Fresh { .. }) {
            // D6: `not_required` is refused for this operation.
            return Err(crate::StoreError::StepUpRequired);
        }
        let t = check_recovery_target(tx.tx(), target, Some(admin))?;
        // RFC 115 D11: which throttle applies is decided here, from `t`.
        let provisioning = is_provisioning(&t);
        let (ceiling, since) = (
            if provisioning {
                PROVISIONING_LINKS_PER_HOUR
            } else {
                RECOVERY_LINKS_PER_HOUR
            },
            now - chrono::TimeDelta::hours(1),
        );
        let issued = crate::repos::password_reset_tokens::count_issued_by_within_tx(
            tx.tx(),
            admin,
            since,
            provisioning,
        )?;
        if issued >= ceiling {
            return Err(refuse(Why::Throttled));
        }
        let grant = revoke_and_insert_recovery_token(
            tx.tx(),
            target,
            token_hash,
            crate::models::ResetTokenOrigin::Web,
            Some(admin),
            provisioning,
            expires_at,
            now,
        )?;
        Ok((
            grant,
            U37Event::Issued {
                target,
                reason,
                expires_at,
                invalidated: grant.invalidated,
                provisioning,
                step_up,
            },
        ))
    })
    .await
}

/// Run U37 for the operator CLI (`sui-id admin issue-recovery-link`, RFC 103
/// D12 precedent): the system principal, no session, no actor, and
/// `step_up = not_applicable:system_principal`. The same target refusals as
/// the web entry **except** the administrator and self refusals, because its
/// authority is not an administrator session (D5). D8's CLI limit is counted
/// from the database (`issued_via = 'cli'` in the last hour), because the CLI
/// is a separate process.
pub async fn issue_recovery_link_as_operator(
    db: &crate::Database,
    target: UserId,
    token_hash: Vec<u8>,
    reason: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<crate::registry::Audited<RecoveryLinkGrant>> {
    use crate::errors::RecoveryRefusal as Why;
    let context = AuthorizedCommandContext::<U37>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U37>| {
        let reason = validated_recovery_reason(&reason)?;
        let t = check_recovery_target(tx.tx(), target, None)?;
        let provisioning = is_provisioning(&t);
        let ceiling = if provisioning {
            PROVISIONING_LINKS_PER_HOUR
        } else {
            RECOVERY_LINKS_PER_HOUR
        };
        let issued = crate::repos::password_reset_tokens::count_issued_via_cli_within_tx(
            tx.tx(),
            now - chrono::TimeDelta::hours(1),
            provisioning,
        )?;
        if issued >= ceiling {
            return Err(refuse(Why::Throttled));
        }
        let grant = revoke_and_insert_recovery_token(
            tx.tx(),
            target,
            token_hash,
            crate::models::ResetTokenOrigin::Cli,
            None,
            provisioning,
            expires_at,
            now,
        )?;
        Ok((
            grant,
            U37Event::OperatorIssued {
                target,
                reason,
                expires_at,
                invalidated: grant.invalidated,
                provisioning,
            },
        ))
    })
    .await
}

// ── T04 — refresh-token rotation / reuse revocation (closed branches) ───

static T04_ROTATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::RefreshRotated,
    name: "auth.refresh.rotated",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "family_id",
        description: "the rotation family the presented token belonged to",
        required: false,
    }],
};

static T04_THEFT_DETECTED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::RefreshTheftDetected,
    name: "auth.refresh.theft_detected",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "family_id",
            description: "the rotation family that was revoked",
            required: false,
        },
        AttributeSpec {
            name: "family_revoked_count",
            description: "how many still-active family members were revoked in this sweep",
            required: false,
        },
    ],
};

crate::declare_write_command! {
    /// T04 — refresh-token rotation, with the reuse/theft branch.
    command T04 = "T04" {
        // A refresh-token presenter is not an authorizing human actor for
        // this command any more than a login attempt is for U22 — the
        // token's own validity is the authority, checked inside the
        // transaction (T04_ROTATED/T04_THEFT_DETECTED's `actor: None`
        // says the same thing about the event payload).
        system_principal: permitted;
        enum T04Event {
            Rotated { user_id: UserId, family_id: FamilyId } => &T04_ROTATED,
            TheftDetected { user_id: UserId, family_id: FamilyId, family_revoked: i64 } => &T04_THEFT_DETECTED,
        }
    }
}

impl SealedCommandEvent<T04> for T04Event {
    fn target(&self) -> Option<AuditTarget> {
        match self {
            Self::Rotated { user_id, .. } | Self::TheftDetected { user_id, .. } => {
                Some(AuditTarget(user_id.to_string()))
            }
        }
    }

    fn result(&self) -> AuditResult {
        // Theft detection is this command *succeeding at its job*
        // (recording and closing the family) — see U22's identical
        // reasoning for why this isn't `AuditResult::Failure`.
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        match self {
            Self::Rotated { family_id, .. } => AuditAttributes::builder()
                .attribute("family_id", family_id.as_str().to_string())
                .build(),
            Self::TheftDetected {
                family_id,
                family_revoked,
                ..
            } => AuditAttributes::builder()
                .attribute("family_id", family_id.as_str().to_string())
                .attribute("family_revoked_count", family_revoked.to_string())
                .build(),
        }
    }
}

/// What T04 committed, for the caller to act on. Both variants are a
/// *successful* command execution (both commit); which one occurred is
/// the caller's business, not an error — matching how `begin_rotation`'s
/// `RotationLookup` already drew this line before RFC 094.
pub enum T04Outcome {
    /// The presented token was the family's active one; it is now
    /// revoked and `successor` (already inserted, in the same family) is
    /// this call's replacement. `raw_token` is the successor's plaintext
    /// — safe to expose to the caller now, and only now, because this
    /// variant is only ever constructed after commit.
    Rotated {
        successor: crate::models::RefreshTokenRow,
        raw_token: sui_id_shared::RawRefreshToken,
    },
    /// The presented token was already revoked (a prior rotation, or a
    /// concurrent winner). The whole family is now revoked, including
    /// `prepared`'s never-used successor (dropped here, zeroizing its raw
    /// token — see `PreparedRefreshToken`'s doc comment).
    TheftDetected { family_revoked: i64 },
}

/// Run T04 (refresh-token rotation, closed branch on reuse) through the
/// Class-A runner.
///
/// `prepared` must already hold the successor to use on the `Rotated`
/// branch — RFC 094 requires the raw token, its hash, and its sealed
/// ciphertext to be computed *before* this transaction opens (`repos::
/// refresh_tokens::prepare_refresh_token`), so the only fallible work
/// inside the transaction is database work. On the `TheftDetected`
/// branch, `prepared` is dropped whole without ever being inserted —
/// its raw token zeroizes via `RawRefreshToken`'s own `Drop`.
pub async fn rotate_refresh_token(
    db: &crate::Database,
    presented_hash: RefreshTokenHash,
    expected_client: ClientId,
    now: DateTime<Utc>,
    prepared: crate::repos::refresh_tokens::PreparedRefreshToken,
) -> StoreResult<crate::registry::Audited<T04Outcome>> {
    let context = AuthorizedCommandContext::<T04>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, T04>| {
        let rotation = crate::repos::refresh_tokens::begin_rotation_within_tx(
            tx.tx(),
            &presented_hash,
            &expected_client,
            now,
        )?;
        match rotation {
            crate::repos::refresh_tokens::RotationLookup::RotatedHere(row) => {
                crate::repos::refresh_tokens::insert_prepared_within_tx(tx.tx(), &prepared)?;
                let event = T04Event::Rotated {
                    user_id: row.user_id,
                    family_id: row.family_id,
                };
                Ok((
                    T04Outcome::Rotated {
                        successor: prepared.row,
                        raw_token: prepared.raw_token,
                    },
                    event,
                ))
            }
            crate::repos::refresh_tokens::RotationLookup::ReuseDetected {
                row,
                family_revoked,
            } => {
                // `prepared` is dropped here, unused — its raw token
                // zeroizes via `Drop`, not returned to any caller.
                let family_revoked = family_revoked as i64;
                let event = T04Event::TheftDetected {
                    user_id: row.user_id,
                    family_id: row.family_id,
                    family_revoked,
                };
                Ok((T04Outcome::TheftDetected { family_revoked }, event))
            }
            crate::repos::refresh_tokens::RotationLookup::Expired(_)
            | crate::repos::refresh_tokens::RotationLookup::Unknown => {
                Err(crate::StoreError::NotFound)
            }
        }
    })
    .await
}

// ── T09 — initial root-family refresh-token issuance (Protocol) ────────

/// Run T09 (initial refresh-token issuance) through the `Protocol`
/// runner. No event, no audit row is possible here by construction —
/// same reasoning as U30/O01: initial issuance is high-frequency protocol
/// state, and `T04` (this module) is the only path to `Audited<T>` for
/// refresh tokens. See `tests/compile_fail/` for the crate-wide version
/// of this claim; this command's own share of it is that nothing in this
/// function's body can reach `Database::class_a`.
pub async fn insert_initial_refresh_token(
    db: &crate::Database,
    prepared: crate::repos::refresh_tokens::PreparedRefreshToken,
) -> StoreResult<()> {
    db.protocol(move |write| {
        crate::repos::refresh_tokens::insert_prepared_within_tx(write.tx(), &prepared)
    })
    .await
}

// ── L01 — password sign-in (RFC 102 Part A) ─────────────────────────────

static L01_SUCCESS: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthLoginSuccess,
    name: "auth.login.success",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "evicted",
            description: "older sessions revoked to keep the user within the concurrent-session cap",
            required: false,
        },
        AttributeSpec {
            name: "source",
            description: "slug of the user source that authenticated a directory sign-in (L03); absent for a local password (L01)",
            required: false,
        },
        AttributeSpec {
            name: "stable_id",
            description: "the directory stable id of a directory sign-in (L03), truncated to 255 bytes; absent for L01",
            required: false,
        },
    ],
};

crate::declare_write_command! {
    /// L01 — a successful password sign-in for a user with no second
    /// factor. The actor is the user whose password was just verified,
    /// outside this transaction, by `sui-id-core`.
    command L01 = "L01" {
        system_principal: forbidden;
        enum L01Event {
            Success { user_id: UserId, evicted: i64 } => &L01_SUCCESS,
        }
    }
}

impl SealedCommandEvent<L01> for L01Event {
    fn target(&self) -> Option<AuditTarget> {
        match self {
            Self::Success { user_id, .. } => Some(AuditTarget(user_id.to_string())),
        }
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        match self {
            Self::Success { evicted, .. } => AuditAttributes::builder()
                .attribute("evicted", evicted.to_string())
                .build(),
        }
    }
}

/// Run L01: in one transaction, re-read the user as active and unlocked,
/// clear the failure counter and any stale lock, set `last_login_at`,
/// insert `session`, and revoke the oldest sessions over the
/// concurrent-session cap. Returns how many sessions were evicted. A user
/// who is no longer active rolls back with `NotFound`.
pub async fn sign_in_with_password(
    db: &crate::Database,
    session: crate::models::SessionRow,
) -> StoreResult<crate::registry::Audited<i64>> {
    let user_id = session.user_id;
    let context = AuthorizedCommandContext::<L01>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, L01>| {
        let now = session.created_at;
        crate::repos::users::record_password_login_within_tx(tx.tx(), user_id, now)?;
        crate::repos::sessions::insert_within_tx(tx.tx(), &session)?;
        let evicted = evict_over_cap_within_tx(tx.tx(), user_id, now)?;
        Ok((evicted, L01Event::Success { user_id, evicted }))
    })
    .await
}

// ── L03 — directory sign-in (RFC 102 Part A) ────────────────────────────

crate::declare_write_command! {
    /// L03 — a successful sign-in through an external user source, for a
    /// user with no second factor. It shares `auth.login.success` with L01
    /// and adds the `source` attribute. The actor is the shadow user whose
    /// directory password a user source just verified.
    command L03 = "L03" {
        system_principal: forbidden;
        enum L03Event {
            Success { user_id: UserId, source: String, stable_id: String, evicted: i64 } => &L01_SUCCESS,
        }
    }
}

impl SealedCommandEvent<L03> for L03Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Success { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Success {
            source,
            stable_id,
            evicted,
            ..
        } = self;
        AuditAttributes::builder()
            .attribute("evicted", evicted.to_string())
            .attribute("source", source.clone())
            .attribute(
                "stable_id",
                truncate_utf8(stable_id, EXTERNAL_ID_ATTRIBUTE_BYTES),
            )
            .build()
    }
}

/// Run L03. In one transaction: upsert the shadow row (creating it with
/// `user_id`, or refreshing the display fields of the row that already has
/// that id), re-read the user as active and unlocked, reset the password
/// counter and stale lock, set `last_login_at`, insert the session and evict
/// over-cap sessions. `session.user_id` must be `user_id`. Any refusal rolls
/// back with `NotFound`.
pub async fn sign_in_from_directory(
    db: &crate::Database,
    shadow: crate::repos::users::LdapShadowData,
    source_slug: String,
    session: crate::models::SessionRow,
) -> StoreResult<crate::registry::Audited<i64>> {
    let user_id = session.user_id;
    let context = AuthorizedCommandContext::<L03>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, L03>| {
        let now = session.created_at;
        crate::repos::users::upsert_ldap_shadow_within_tx(tx.tx(), &shadow, user_id, now)?;
        crate::repos::users::record_password_login_within_tx(tx.tx(), user_id, now)?;
        crate::repos::sessions::insert_within_tx(tx.tx(), &session)?;
        let evicted = evict_over_cap_within_tx(tx.tx(), user_id, now)?;
        Ok((
            evicted,
            L03Event::Success {
                user_id,
                source: source_slug,
                stable_id: shadow.external_stable_id.clone(),
                evicted,
            },
        ))
    })
    .await
}

/// Byte bound for an upstream identifier recorded as an audit attribute: a
/// directory stable id (a DN is allowed) or a federation `sub`. Well under
/// the registry's per-value limit, so an unusually long identifier is
/// recorded truncated rather than failing the sign-in.
pub const EXTERNAL_ID_ATTRIBUTE_BYTES: usize = 255;

/// `value` cut to at most `max` bytes, on a character boundary.
pub fn truncate_utf8(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_owned();
    }
    let mut end = max;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

// ── L04 — federated sign-in (RFC 102 Part A) ────────────────────────────

static L04_SUCCESS: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthFederationSigninSuccess,
    name: "auth.federation.signin.success",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "provider",
            description: "slug of the upstream federation provider",
            required: false,
        },
        AttributeSpec {
            name: "evicted",
            description: "older sessions revoked to keep the user within the concurrent-session cap",
            required: false,
        },
    ],
};

crate::declare_write_command! {
    /// L04 — a successful federated sign-in (the shipped path), for a user
    /// with no second factor. The actor is the user linked to the upstream
    /// identity the callback just verified.
    command L04 = "L04" {
        system_principal: forbidden;
        enum L04Event {
            Success { user_id: UserId, provider: String, evicted: i64 } => &L04_SUCCESS,
        }
    }
}

impl SealedCommandEvent<L04> for L04Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Success { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Success {
            provider, evicted, ..
        } = self;
        AuditAttributes::builder()
            .attribute("provider", provider.clone())
            .attribute("evicted", evicted.to_string())
            .build()
    }
}

/// Run L04. In one transaction (RFC 094 `:548`, amended 2026-10-15 — F01
/// folded in, not a separate Protocol transaction): the guarded
/// `federation_login_attempt` `exchanging -> completed` transition, the
/// enabled-provider and same-active-link/user authority checks F01 named
/// but the legacy handler never ran at commit time, re-read the user as
/// active (not disabled, not deleted), set `last_login_at`, insert the
/// session, and evict over-cap sessions — all or nothing. Before this
/// amendment a crash between the attempt's own completion and the
/// session insert could leave an attempt `completed` with no session, or
/// a session with the attempt still `exchanging`; one transaction now
/// either commits every one of these writes or none of them.
///
/// **Authority, re-read here rather than trusted from the caller's own,
/// now-stale read**: `authority_drift_within_tx` (shared with F03/F06) —
/// provider still enabled, the link still exists for this exact
/// `(user_id, provider_id)` pair — is the fresh recheck the legacy
/// handler's own `provider.enabled` check could not be, since that
/// variable is read once, long before the upstream round trip, and never
/// re-fetched before this commit. Its third check (`users::
/// is_active_within_tx`) duplicates this function's own guarded
/// `record_federated_login_within_tx`, deliberately: the handler's own
/// early user-disabled/deleted check (`complete_federated_signin`) is a
/// cheap UX filter *before* the OIDC exchange even starts, not a
/// substitute for the durable, atomic recheck this transaction performs
/// at the only point that actually matters — immediately before the
/// session it would authorise commits. Any drift found here rolls back
/// the whole transaction as `StoreError::Conflict`, the same outcome
/// shape the attempt-completion guard already uses.
///
/// **This changes which error a disabled/deleted user rolls back with**,
/// stated here rather than left for a reader to assume "unchanged":
/// before this amendment, `record_federated_login_within_tx`'s own
/// guarded `UPDATE` (`WHERE is_disabled = 0 AND is_deleted = 0`) was the
/// only place that caught it, and a zero-rows-affected result there is
/// `NotFound`. Now `authority_drift_within_tx` runs first and reaches
/// the identical user row with `users::is_active_within_tx` — a plain
/// `SELECT`, not a guarded `UPDATE` — which returns `Ok(false)` for a
/// disabled/deleted *flag*, turned into `Err(Conflict)` here, the same
/// outcome every other authority-drift case and the attempt-completion
/// guard already produce. Only a user row absent from the table
/// entirely (not merely flagged — `is_active_within_tx`'s own `Err(
/// NotFound)` for that case) still reaches this function as `NotFound`,
/// propagated untouched before `Conflict` is ever considered. The
/// handler's own caller already treats every non-`NotFound` error here
/// identically (the same redirect; only the log level differs) and
/// needed no change to keep doing so — but a caller that specifically
/// branches on `NotFound` to mean "disabled or deleted" would now be
/// wrong, which is exactly the distinction worth stating rather than
/// letting the type alone imply.
///
/// No `upstream_sub` parameter: RFC 096 `:714-715`'s review (stage 7's own
/// carried-in item) removed the `sub` audit attribute this event used to
/// carry truncated — the upstream subject already lives durably on the
/// `federation_link` row, so the audit trail stays reconstructible from
/// internal ids alone without it here either, and there is nothing left
/// for a caller to pass.
pub async fn sign_in_federated(
    db: &crate::Database,
    attempt_id: FederationLoginAttemptId,
    provider_id: FederationProviderId,
    provider_slug: String,
    session: crate::models::SessionRow,
) -> StoreResult<crate::registry::Audited<i64>> {
    let user_id = session.user_id;
    let context = AuthorizedCommandContext::<L04>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, L04>| {
        let now = session.created_at;
        crate::repos::federation_login_attempt::complete_within_tx(tx.tx(), attempt_id)?;
        if authority_drift_within_tx(tx.tx(), provider_id, user_id)?.is_some() {
            return Err(StoreError::Conflict);
        }
        crate::repos::users::record_federated_login_within_tx(tx.tx(), user_id, now)?;
        crate::repos::sessions::insert_within_tx(tx.tx(), &session)?;
        let evicted = evict_over_cap_within_tx(tx.tx(), user_id, now)?;
        Ok((
            evicted,
            L04Event::Success {
                user_id,
                provider: provider_slug,
                evicted,
            },
        ))
    })
    .await
}

// ── F04 — first federated provisioning (RFC 096-B1 stage 7) ────────────

static F04_PROVISIONED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthFederationProvisioned,
    name: "auth.federation.provisioned",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "provider_id",
            description: "internal id of the upstream federation provider",
            required: false,
        },
        AttributeSpec {
            name: "mode",
            description: "the provision mode that ran this command -- always \"provision_on_first_login\", the only mode F04 ever runs under",
            required: false,
        },
    ],
};

crate::declare_write_command! {
    /// F04 — first federated provisioning (RFC 096 `:706-711`, amended RFC
    /// 094). The actor is the newly created user; `:714-715` is why its
    /// attributes carry only internal ids and the fixed mode literal,
    /// never the upstream `sub` or email that identified them.
    command F04 = "F04" {
        system_principal: forbidden;
        enum F04Event {
            Provisioned { user_id: UserId, provider_id: FederationProviderId } => &F04_PROVISIONED,
        }
    }
}

impl SealedCommandEvent<F04> for F04Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Provisioned { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Provisioned { provider_id, .. } = self;
        AuditAttributes::builder()
            .attribute("provider_id", provider_id.to_string())
            .attribute("mode", "provision_on_first_login")
            .build()
    }
}

/// Run F04. In one transaction: complete the attempt (guarded `exchanging
/// -> completed`), create the passwordless non-admin user, create exactly
/// one provider/sub link, insert the `[Fed]` session and evict over-cap
/// sessions. Every write re-asks, inside this transaction, the question
/// the handler already asked outside it:
///
/// - **the attempt is still `exchanging`** — [`crate::repos::
///   federation_login_attempt::complete_within_tx`]; a lost race (F05 beat
///   this transaction to `failed`, or a concurrent commit already reached
///   `completed`) is `StoreError::Conflict`.
/// - **the username and the normalized email are each still free** —
///   [`crate::repos::users::create_within_tx`]'s own `UNIQUE` constraints
///   on `username` and `email_normalized` (migrations 0001, 0020/0022).
///   Enforced by the insert itself, not a duplicate `SELECT` the insert
///   could still race past — a constraint violation is `StoreError::
///   Conflict`.
/// - **no link to this upstream identity exists yet** —
///   [`crate::repos::federation_link::insert_within_tx`]'s own `UNIQUE
///   (provider_id, upstream_sub)` (migration for `federation_link`). Also
///   a plain insert, never an upsert, for the same reason: an upsert would
///   absorb the exact race this recheck exists to catch. A constraint
///   violation is `StoreError::Conflict`.
///
/// `user`, `link` and `session` must already agree on the same freshly
/// generated `UserId` — the caller resolves that id, the username, and
/// the link's upstream identity before opening this transaction; nothing
/// in here invents or overrides any of them.
pub async fn provision_federated_user(
    db: &crate::Database,
    attempt_id: FederationLoginAttemptId,
    provider_id: FederationProviderId,
    user: crate::models::UserRow,
    link: crate::models::FederationLinkRow,
    session: crate::models::SessionRow,
) -> StoreResult<crate::registry::Audited<i64>> {
    let user_id = session.user_id;
    let context = AuthorizedCommandContext::<F04>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, F04>| {
        let now = session.created_at;
        crate::repos::federation_login_attempt::complete_within_tx(tx.tx(), attempt_id)?;
        crate::repos::users::create_within_tx(tx.tx(), &user)?;
        crate::repos::federation_link::insert_within_tx(tx.tx(), &link)?;
        crate::repos::sessions::insert_within_tx(tx.tx(), &session)?;
        let evicted = evict_over_cap_within_tx(tx.tx(), user_id, now)?;
        Ok((
            evicted,
            F04Event::Provisioned {
                user_id,
                provider_id,
            },
        ))
    })
    .await
}

// ── F02 — existing federated link, local MFA required (Protocol) ───────

/// Run F02 (RFC 094 `:549`, RFC 096 `:719-722`) through the `Protocol`
/// runner. No event, no audit row is possible here by construction — the
/// same reasoning as T09 (`insert_initial_refresh_token`, above): F02 is
/// Class P (`contracts/write-commands.toml`), and a Class-P command has no
/// path to `Audited<T>` by construction (`registry.rs`'s own
/// "Non-atomic runners" section). The Class-B observation that replaces
/// the legacy `auth.login.password_ok_mfa_required` append on this branch
/// (RFC 096-B1 stage 9, item 7) is emitted by the caller, through
/// `repos::audit::emit_must_attempt` and
/// `repos::federation_mfa_pending::AUDIT_MFA_REQUIRED_DESCRIPTOR` — outside
/// this transaction, the same way every other `emit_must_attempt` call
/// site in this codebase sits outside its own Class-A/Class-P boundary
/// (RFC 094 A6: a failure to log never blocks the write this stage's own
/// two writes already committed).
///
/// Exactly two writes, both or neither: the guarded attempt completion
/// (`federation_login_attempt::complete_within_tx`, the same function F04
/// already uses) and exactly one `federation_mfa_pending` row
/// (`federation_mfa_pending::insert_within_tx`). `pending.user_id` and
/// `pending.provider_id` must already agree with `attempt_id`'s own
/// resolved identity — the caller resolves all three before opening this
/// transaction, the same division `provision_federated_user` (F04) already
/// uses for `attempt_id`/`provider_id`/`user`/`link`/`session`.
///
/// **The "active link" recheck is the row's own `FOREIGN KEY`,** atomic
/// with the insert (migration 0047's fix round: `FOREIGN KEY (user_id,
/// provider_id) REFERENCES federation_link`) — not a separate `SELECT`
/// this write could race past. A lost race on either write (the attempt
/// no longer `exchanging`, or the link gone) is `StoreError::Conflict`.
///
/// **The provider-enabled/version/generation recheck and the "active
/// user" recheck are not repeated here.** Both already run, fresh, in the
/// caller's own request before this function is reached:
/// `federated_callback`'s single `provider.enabled` check plus `claim`'s
/// own provider-triple match (RFC 096 `:597-598`) cover the provider;
/// `complete_federated_signin`'s own re-read of the user
/// (`!is_disabled && !is_deleted`) covers the user, a few lines before the
/// branch that calls this function. Neither is this stage's own new
/// mechanism — both are named here, not reimplemented, so the recheck
/// this stage *does* add (the link `FOREIGN KEY`) is not mistaken for
/// covering all three.
///
/// Builds and seals the row itself from its raw pieces, rather than
/// taking a pre-built `FederationMfaPendingRow` — the same "crypto is
/// sync, seal before entering `with_conn`" shape
/// `federation_login_attempt::insert` already uses, and it keeps
/// `continuation_sealed`'s AAD construction (`build_aad`, five columns,
/// all resolved here) out of the caller's crate entirely: the HTTP
/// handler never needs to know the sealed column exists, only that this
/// function returns the row's id and its own `expires_at` for the cookie
/// it does need to set. RFC 096-B1 stage 12b: `expires_at` is returned
/// alongside the id — not read a second time by the caller — because a
/// restart reuses the *existing* row's own value (the next paragraph's
/// own point), and the cookie's `max_age` must reflect that, not a fresh
/// `PENDING_LIFETIME_SECS` the row itself was never granted.
/// `now` is sampled once by the caller and used for both `created_at` and
/// `expires_at` here, not read twice — the schema's own `expires_at =
/// created_at + 300s` `CHECK` would refuse a row straddling a clock tick
/// between two reads.
/// RFC 096-B1 stage 11b, RFC 096 `:756`: "a restart preserves count/
/// status; it grants no reset." Before this stage, every call inserted
/// a brand-new row unconditionally — a federated sign-in restarted
/// while an existing continuation sat at `failure_count = 4` produced a
/// *second* row at `0`, with its own independent five-guess budget, as
/// many times as the upstream round trip was repeated. This function
/// now durably re-reads whatever "active" row already exists for
/// `user_id` (`federation_mfa_pending::find_active_by_user_id_within_tx`,
/// `status IN ('pending', 'exhausted')`) before deciding whether to
/// insert at all.
///
/// Three outcomes, in order of how "sticky" the existing row is:
///
/// 1. **`Exhausted`** — the cap the row already names. Returned
///    unchanged; nothing is written at all. This is the case the
///    dispatch asks to be most careful with, and it is the only one
///    with zero write path: there is no `UPDATE` for a sixth restart to
///    race, because none runs.
/// 2. **`Pending`, still live** (`expires_at > now`) — the restart case
///    the dispatch's own example describes. Reused:
///    `refresh_for_restart_within_tx` updates the three per-attempt
///    artifacts a new upstream round trip actually produces (bindings,
///    next target, re-sealed continuation) and nothing else —
///    `failure_count`/`status`/`created_at`/`expires_at` are not in its
///    own `SET` list, so this stage does not widen the 300-second
///    lifetime to compensate either, as the dispatch asked. A live
///    `federation_webauthn_ceremony` row, if one exists, is deleted
///    first (fix round, R1): its own composite foreign key binds
///    `provider_id`/`provider_config_version`/
///    `provider_activation_generation` to this row's *current* values
///    with `NO ACTION` on update, and `refresh_for_restart_within_tx`
///    is about to change exactly those three — "retry requires F06" is
///    this design's existing answer to an in-flight ceremony that
///    cannot survive what happens next, applied here unconditionally
///    rather than only when the triple actually changes.
/// 3. **`Pending`, but already expired by wall clock** — not a budget
///    worth protecting (F03's own expiry check already refuses to
///    count a proof against a row in this state, the same way it would
///    a fresh one), and *not* reusable either: handing back a
///    continuation that is already dead would make every restart after
///    a plain 300-second timeout permanently useless, not safe. Durably
///    invalidated here (`invalidate_within_tx`, the same guarded
///    transition F03 already uses for the identical reason), then
///    treated as if no active row existed.
///
/// **Why `completed`/`invalidated` rows are invisible to this check
/// entirely**, rather than also staying sticky: `:756`'s own sentence
/// sits in a paragraph about the row's *brute-force*-relevant state —
/// the five-guess cap and what restarting it should not be able to do.
/// A `completed` row already granted its session; nothing is left to
/// protect by refusing the same user a fresh continuation on their
/// next, unrelated federated sign-in. An `invalidated` row ended for a
/// reason that is not a wrong guess at all (authority drift, or this
/// same wall-clock expiry) — treating *that* as permanently sticky
/// would turn an ordinary timeout into a de-facto lockout no RFC text
/// asks for. Only `pending` (mid-budget) and `exhausted` (budget spent)
/// are the states a restart must not be able to escape, and migration
/// `0048`'s own partial index enforces exactly that pair, not all four
/// statuses — see its own header for the reasoning restated against the
/// schema.
#[allow(clippy::too_many_arguments)]
pub async fn issue_federated_mfa_pending(
    db: &crate::Database,
    attempt_id: FederationLoginAttemptId,
    provider_id: FederationProviderId,
    provider_config_version: i64,
    provider_activation_generation: i64,
    user_id: UserId,
    next_path: Option<String>,
    browser_binding_sha256: [u8; 32],
    csrf_binding_sha256: [u8; 32],
    now: DateTime<Utc>,
) -> StoreResult<(sui_id_shared::ids::FederationMfaPendingId, DateTime<Utc>)> {
    // `Database` is a cheap `Clone` handle (unlike `MasterKey`): the
    // closure below must be `'static`, and which row gets sealed (the
    // existing one, on reuse, or a freshly generated one) is not known
    // until the durable read inside the transaction runs — so unlike
    // this function's own pre-stage-11b shape, sealing cannot happen
    // before `db.protocol` is entered at all.
    let db_for_tx = db.clone();
    db.protocol(
        move |write| -> StoreResult<(sui_id_shared::ids::FederationMfaPendingId, DateTime<Utc>)> {
            let tx = write.tx();
            crate::repos::federation_login_attempt::complete_within_tx(tx, attempt_id)?;

            if let Some(existing) =
                crate::repos::federation_mfa_pending::find_active_by_user_id_within_tx(tx, user_id)?
            {
                if existing.status == crate::models::FederationMfaPendingStatus::Exhausted {
                    return Ok((existing.id, existing.expires_at));
                }
                if existing.expires_at > now {
                    // RFC 096-B1 stage 11b fix round, R1: the ceremony
                    // table's own composite `FOREIGN KEY` has `ON
                    // DELETE CASCADE` and no `ON UPDATE` clause (0047
                    // `:240-249`), so `NO ACTION` governs the three
                    // columns `refresh_for_restart_within_tx` is about
                    // to change (`provider_id`, `provider_config_version`,
                    // `provider_activation_generation`) — a live
                    // ceremony still referencing this row's *old*
                    // identity triple refuses that `UPDATE` outright,
                    // measured directly: `FOREIGN KEY constraint
                    // failed`, extended code 787, not a clean outcome.
                    // A restart re-issuing the continuation is the same
                    // "retry requires F06" rule this design already
                    // applies to a wrong WebAuthn completion
                    // (`reject_or_exhaust`) and to F06 itself replacing
                    // its own prior ceremony — an in-flight challenge
                    // bound to the *previous* round has no claim on
                    // surviving this one, so it is consumed
                    // unconditionally here too, not only when the
                    // identity triple actually changes: `provider_
                    // config_version`/`provider_activation_generation`
                    // are fixed at `(0, 0)` today (stage 10a's own
                    // measurement) and would otherwise make this fix
                    // invisible until RFC 096-B2/M2b makes them real.
                    // `ON UPDATE CASCADE` was considered and rejected:
                    // the copied triple exists so a ceremony cannot
                    // outlive the authority it was issued under, and
                    // cascading would let it silently follow a changed
                    // one instead of being invalidated by it.
                    crate::repos::federation_webauthn_ceremony::delete_by_pending_id_within_tx(
                        tx,
                        existing.id,
                    )?;
                    let aad = crate::repos::federation_mfa_pending::build_aad(
                        existing.id,
                        provider_id,
                        provider_config_version,
                        provider_activation_generation,
                        user_id,
                    );
                    let continuation_sealed = crate::crypto::seal(db_for_tx.key(), &[], &aad)?;
                    let refreshed =
                        crate::repos::federation_mfa_pending::refresh_for_restart_within_tx(
                            tx,
                            existing.id,
                            provider_id,
                            provider_config_version,
                            provider_activation_generation,
                            user_id,
                            &continuation_sealed,
                            next_path.as_deref(),
                            browser_binding_sha256,
                            csrf_binding_sha256,
                        )?;
                    if !refreshed {
                        return Err(StoreError::Conflict);
                    }
                    return Ok((existing.id, existing.expires_at));
                }
                // Expired by wall clock, still `pending` in the database:
                // dead, not reusable, and must not go on blocking the fresh
                // row below (migration 0048's own partial index would
                // refuse a second `pending`/`exhausted` row for this
                // `user_id` otherwise).
                let invalidated =
                    crate::repos::federation_mfa_pending::invalidate_within_tx(tx, existing.id)?;
                if !invalidated {
                    return Err(StoreError::Conflict);
                }
            }

            let id = sui_id_shared::ids::FederationMfaPendingId::new();
            let aad = crate::repos::federation_mfa_pending::build_aad(
                id,
                provider_id,
                provider_config_version,
                provider_activation_generation,
                user_id,
            );
            let continuation_sealed = crate::crypto::seal(db_for_tx.key(), &[], &aad)?;
            let pending = crate::models::FederationMfaPendingRow {
                id,
                provider_id,
                provider_config_version,
                provider_activation_generation,
                user_id,
                continuation_sealed: Some(continuation_sealed),
                next_path,
                browser_binding_sha256,
                csrf_binding_sha256,
                failure_count: 0,
                status: crate::models::FederationMfaPendingStatus::Pending,
                created_at: now,
                expires_at: now
                    + chrono::Duration::seconds(
                        crate::repos::federation_mfa_pending::PENDING_LIFETIME_SECS,
                    ),
            };
            crate::repos::federation_mfa_pending::insert_within_tx(tx, &pending)?;
            Ok((id, pending.expires_at))
        },
    )
    .await
}

// ── F03 — submit/complete federated local MFA (Protocol) ───────────────

use crate::repos::federation_mfa_pending::{
    FailureIncrement, InvalidatedReason, MfaRejectionReason,
};

/// What stage 10a's `FederatedMfaVerifier` already checked, reduced to
/// the plain primitives F03 needs — RFC 096 `:784-789`'s F03 seam:
/// `sui-id-store` cannot name `sui_id_core::federated_mfa`'s own
/// candidate types, so the promotion logic (also `sui-id-core`, beside
/// the verifier) converts its sealed `ValidTotp`/`ValidRecovery`/
/// `BoundRejected` into exactly this before calling F03. `None` is for
/// the one case the verifier itself already decided without a proof at
/// all: `FederatedMfaOutcome::Expired`. F03 re-derives expiry durably on
/// every call regardless of which variant is passed (RFC 096 `:792-793`:
/// "authoritative consumption remains inside F03"), so `None` reaches
/// exactly the same `Invalidated` outcome the verifier's own read-only
/// check already anticipated — there is no proof to launder into one of
/// the other three outcomes in this case, and inventing one would be
/// pretending a submission was evaluated when it was not.
#[derive(Debug)]
pub enum F03Proof {
    /// A TOTP code the verifier already matched to `step`, strictly
    /// after the row's own `last_used_step` at verification time —
    /// re-checked here against the *current* durable value, inside the
    /// same transaction that then advances it, so there is no gap for a
    /// concurrent consumer of the same step.
    Totp { step: i64 },
    /// A recovery code the verifier already matched; `expected_sealed`
    /// is the blob it matched against and `new_sealed` is that blob
    /// with the matched hash removed — re-checked that `expected_sealed`
    /// is still the row's current blob before swapping it in, same
    /// reason as `Totp`.
    Recovery {
        expected_sealed: Vec<u8>,
        new_sealed: Vec<u8>,
    },
    /// RFC 096-B1 stage 11: a WebAuthn assertion the verifier's own
    /// `verify_webauthn` entry point already matched to a live
    /// ceremony and credential — `ceremony_id`/`credential_id` are
    /// what F03 durably re-checks still identify the *same* ceremony/
    /// credential (RFC 096 `:794-798`'s own binding, re-derived rather
    /// than trusted from the verifier's earlier, non-authoritative
    /// read); `expected_sealed`/`new_sealed` are the credential's own
    /// passkey blob before/after `Passkey::update_credential` advanced
    /// its signature counter — the same compare-and-swap pair
    /// `Recovery` already commits for the recovery-code blob, applied
    /// here to `user_webauthn_credentials.passkey_enc` instead.
    Webauthn {
        ceremony_id: sui_id_shared::ids::FederationWebauthnCeremonyId,
        credential_id: sui_id_shared::ids::WebauthnCredentialId,
        expected_sealed: Vec<u8>,
        new_sealed: Vec<u8>,
    },
    /// The verifier bound the submission to the row but the proof itself
    /// was wrong, malformed, unsupported, or substituted a different
    /// method/ceremony — RFC 096 `:741-743`. `reason` is the verifier's
    /// own closed selection, already converted to this crate's plain
    /// enum by the caller.
    Rejected { reason: MfaRejectionReason },
}

/// F03's own four guarded outcomes, RFC 096 `:812-818`.
#[derive(Debug)]
pub enum F03Outcome {
    /// Method-specific anti-replay/pending consumption, link
    /// observation/bookkeeping, and `[Fed, local_method]` session/cap
    /// already committed. `session` is the row as inserted;
    /// `evicted` is how many older sessions the cap eviction revoked.
    Promoted {
        session: crate::models::SessionRow,
        evicted: i64,
        /// `"totp"`, `"recovery_code"`, or (RFC 096-B1 stage 11)
        /// `"webauthn"` — the same vocabulary L02's own
        /// `auth.mfa.success` attribute uses, not the RFC 8176 `amr` token.
        method: &'static str,
    },
    /// A wrong/unsupported proof, counted to 1–4. The row is still
    /// `pending`.
    RejectedStillPending {
        failure_count: i64,
        reason: MfaRejectionReason,
    },
    /// The fifth wrong/unsupported proof. The row is now `exhausted`.
    AttemptsExhausted { reason: MfaRejectionReason },
    /// Authority drift or expiry, found by F03's own durable recheck —
    /// independent of whatever proof (if any) was submitted. The row is
    /// now `invalidated`.
    Invalidated { reason: InvalidatedReason },
}

/// F03 itself. One Protocol transaction: durably re-reads the row,
/// opens `continuation_sealed` with the row's own current values as AAD
/// (stage 9's tamper check, exercised here for the first time — RFC
/// 096-B1 stage 10b dispatch `:64-70`), re-checks authority (provider
/// enabled, link still exists, user active, factor enabled) and expiry
/// independently of
/// whatever the verifier found earlier, then evaluates `proof` only if
/// every one of those holds. Each binding (browser/CSRF cookie) is
/// **not** re-compared here — that is the verifier's own job, done once,
/// already (RFC 096-B1 stage 10b dispatch `:72-74`); F03 trusts its
/// caller reached it only through a successful `verify()`, exactly the
/// way the source-scope test (`r096_b1_stage10b_f03_scope.rs`) is the
/// only thing that makes that true, the type system cannot (RFC 096
/// `:784-789`).
///
/// Returns `Err(StoreError::NotFound)`, with no audit emission at all,
/// if the row cannot be durably re-read (an unknown id, or one whose
/// parent link was removed and cascaded it away) — there is no actor or
/// target to attribute an event to once the row itself is gone, so this
/// is folded into the same indistinguishable-failure shape the verifier
/// already uses for "no such row," not one of the four tracked outcomes.
pub async fn complete_federated_mfa(
    db: &crate::Database,
    pending_id: sui_id_shared::ids::FederationMfaPendingId,
    proof: Option<F03Proof>,
    now: DateTime<Utc>,
) -> StoreResult<F03Outcome> {
    // `Database` is a cheap `Clone` handle (unlike `MasterKey`, which
    // deliberately is not): the closure below must be `'static`, so it
    // owns its own handle rather than borrowing `db`, and reads the key
    // back off it once the row's own current AAD is known durably,
    // inside the same transaction that read it.
    let db_for_tx = db.clone();
    let outcome = db
        .protocol(move |write| -> StoreResult<F03Outcome> {
            let tx = write.tx();
            let row = crate::repos::federation_mfa_pending::get_within_tx(tx, pending_id)?
                .ok_or(StoreError::NotFound)?;

            // The tamper check stage 9 sealed for: a row edited in place
            // out of band (e.g. `user_id` repointed by a direct `UPDATE`
            // bypassing this crate's own write paths) fails to open.
            // Only meaningful while the row is still `pending` — a
            // terminal row's own `continuation_sealed` is `None` by the
            // table's own `CHECK` (stage 10a's R1), and this function
            // only reaches a terminal row via the early-return branches
            // below, before this open would run.
            if row.status == crate::models::FederationMfaPendingStatus::Pending {
                let sealed = row.continuation_sealed.as_deref().ok_or_else(|| {
                    StoreError::Integrity("pending row has no continuation".into())
                })?;
                let aad = crate::repos::federation_mfa_pending::build_aad(
                    row.id,
                    row.provider_id,
                    row.provider_config_version,
                    row.provider_activation_generation,
                    row.user_id,
                );
                crate::crypto::open(db_for_tx.key(), sealed, &aad)
                    .map_err(|_| StoreError::Integrity("continuation failed to open".into()))?;
            }

            if row.status != crate::models::FederationMfaPendingStatus::Pending {
                return Err(StoreError::Conflict);
            }
            if row.expires_at <= now {
                let invalidated =
                    crate::repos::federation_mfa_pending::invalidate_within_tx(tx, pending_id)?;
                if !invalidated {
                    return Err(StoreError::Conflict);
                }
                return Ok(F03Outcome::Invalidated {
                    reason: InvalidatedReason::Expired,
                });
            }
            if let Some(reason) = authority_drift_within_tx(tx, row.provider_id, row.user_id)? {
                let invalidated =
                    crate::repos::federation_mfa_pending::invalidate_within_tx(tx, pending_id)?;
                if !invalidated {
                    return Err(StoreError::Conflict);
                }
                return Ok(F03Outcome::Invalidated { reason });
            }
            // Resolved once, used only by the Totp/Recovery/Rejected(
            // Totp|Recovery|Malformed) arms below — RFC 096-B1 stage 11
            // narrows what was, through stage 10b, an unconditional
            // requirement for *every* submission (there was no other
            // method to require it for). A `Webauthn`/`MethodMismatch`
            // submission needs no TOTP row at all; requiring one would
            // wrongly invalidate a user whose only enrolled second
            // factor is a passkey.
            let totp_row =
                crate::repos::user_totp::get_within_tx(tx, row.user_id)?.filter(|t| t.enabled);

            match proof {
                None => {
                    // The verifier already found this expired; the
                    // durable recheck above did not (a clock cannot run
                    // backward), so there is nothing left to do but
                    // report the same mismatch as a store-level
                    // integrity problem rather than silently no-op.
                    Err(StoreError::Integrity(
                        "F03 called with no proof on a row the durable recheck still finds pending"
                            .into(),
                    ))
                }
                Some(F03Proof::Rejected { reason }) => {
                    if matches!(
                        reason,
                        MfaRejectionReason::Totp
                            | MfaRejectionReason::Recovery
                            | MfaRejectionReason::Malformed
                    ) && totp_row.is_none()
                    {
                        let invalidated =
                            crate::repos::federation_mfa_pending::invalidate_within_tx(
                                tx, pending_id,
                            )?;
                        if !invalidated {
                            return Err(StoreError::Conflict);
                        }
                        return Ok(F03Outcome::Invalidated {
                            reason: InvalidatedReason::FactorChanged,
                        });
                    }
                    reject_or_exhaust(tx, pending_id, reason)
                }
                Some(F03Proof::Totp { step }) => {
                    let Some(totp_row) = totp_row else {
                        let invalidated =
                            crate::repos::federation_mfa_pending::invalidate_within_tx(
                                tx, pending_id,
                            )?;
                        if !invalidated {
                            return Err(StoreError::Conflict);
                        }
                        return Ok(F03Outcome::Invalidated {
                            reason: InvalidatedReason::FactorChanged,
                        });
                    };
                    if step <= totp_row.last_used_step {
                        // Stale/replayed: functionally a wrong proof.
                        reject_or_exhaust(tx, pending_id, MfaRejectionReason::Totp)
                    } else if crate::repos::federation_mfa_pending::complete_within_tx(
                        tx, pending_id,
                    )? {
                        crate::repos::user_totp::advance_last_used_step_within_tx(
                            tx,
                            row.user_id,
                            step,
                        )?;
                        promote(tx, &row, now, sui_id_shared::AuthMethod::Totp)
                    } else {
                        Err(StoreError::Conflict)
                    }
                }
                Some(F03Proof::Recovery {
                    expected_sealed,
                    new_sealed,
                }) => {
                    let Some(totp_row) = totp_row else {
                        let invalidated =
                            crate::repos::federation_mfa_pending::invalidate_within_tx(
                                tx, pending_id,
                            )?;
                        if !invalidated {
                            return Err(StoreError::Conflict);
                        }
                        return Ok(F03Outcome::Invalidated {
                            reason: InvalidatedReason::FactorChanged,
                        });
                    };
                    if totp_row.recovery_codes_enc.as_deref() != Some(expected_sealed.as_slice()) {
                        // Stale/already consumed: functionally wrong.
                        reject_or_exhaust(tx, pending_id, MfaRejectionReason::Recovery)
                    } else if crate::repos::federation_mfa_pending::complete_within_tx(
                        tx, pending_id,
                    )? {
                        crate::repos::user_totp::swap_recovery_codes_within_tx(
                            tx,
                            row.user_id,
                            &expected_sealed,
                            &new_sealed,
                        )?;
                        promote(tx, &row, now, sui_id_shared::AuthMethod::RecoveryCode)
                    } else {
                        Err(StoreError::Conflict)
                    }
                }
                Some(F03Proof::Webauthn {
                    ceremony_id,
                    credential_id,
                    expected_sealed,
                    new_sealed,
                }) => {
                    // RFC 096 `:798-801`'s own re-check, durable: the
                    // ceremony the candidate was built against must
                    // still be *the* ceremony bound to this row, and
                    // still unexpired by its own, narrower expiry — not
                    // the verifier's earlier, non-authoritative read.
                    // A concurrent F06 call that replaced or outlived
                    // it since `verify_webauthn` ran is this, not a
                    // wrong proof: the dispatch's own `CeremonyInvalid`
                    // description, "bound to something other than
                    // what was presented."
                    let ceremony =
                        crate::repos::federation_webauthn_ceremony::find_by_pending_id_within_tx(
                            tx, pending_id,
                        )?;
                    let ceremony_ok = ceremony
                        .as_ref()
                        .is_some_and(|c| c.id == ceremony_id && c.expires_at > now);
                    if !ceremony_ok {
                        let invalidated =
                            crate::repos::federation_mfa_pending::invalidate_within_tx(
                                tx, pending_id,
                            )?;
                        if !invalidated {
                            return Err(StoreError::Conflict);
                        }
                        return Ok(F03Outcome::Invalidated {
                            reason: InvalidatedReason::CeremonyInvalid,
                        });
                    }
                    // The credential itself (not merely its blob) must
                    // still exist — the WebAuthn analogue of the
                    // TOTP-row-must-exist check above, for the one
                    // credential this candidate names. Read before
                    // `complete_within_tx` runs, same as TOTP's own
                    // `last_used_step`/Recovery's own blob check: a
                    // mismatched-but-present blob below is a wrong
                    // proof (counted), decided *before* the row's
                    // status flips, not after.
                    let current =
                        crate::repos::user_webauthn_credentials::current_passkey_enc_within_tx(
                            tx,
                            credential_id,
                        )?;
                    let Some(current) = current else {
                        let invalidated =
                            crate::repos::federation_mfa_pending::invalidate_within_tx(
                                tx, pending_id,
                            )?;
                        if !invalidated {
                            return Err(StoreError::Conflict);
                        }
                        return Ok(F03Outcome::Invalidated {
                            reason: InvalidatedReason::FactorChanged,
                        });
                    };
                    if current != expected_sealed {
                        // Stale/already consumed: functionally wrong,
                        // the same treatment Recovery's own blob
                        // mismatch already gets.
                        reject_or_exhaust(tx, pending_id, MfaRejectionReason::Webauthn)
                    } else if crate::repos::federation_mfa_pending::complete_within_tx(
                        tx, pending_id,
                    )? {
                        crate::repos::user_webauthn_credentials::swap_passkey_within_tx(
                            tx,
                            credential_id,
                            &expected_sealed,
                            &new_sealed,
                            now,
                        )?;
                        crate::repos::federation_webauthn_ceremony::consume_within_tx(
                            tx,
                            ceremony_id,
                        )?;
                        promote(tx, &row, now, sui_id_shared::AuthMethod::Webauthn)
                    } else {
                        Err(StoreError::Conflict)
                    }
                }
            }
        })
        .await?;

    emit_f03_event(db, &outcome, now).await;
    Ok(outcome)
}

/// Shared between F03 (above) and F06 (below): RFC 096 `:740-741`'s
/// three authority-drift checks that neither command's own identity
/// columns durably re-prove on their own — provider enabled, link still
/// exists, user active. Expiry is the fourth RFC 096 names in the same
/// breath but is deliberately not here: both callers already hold
/// `now` and the row's own `expires_at` from their own read, so
/// checking it here would mean threading one more parameter for a
/// comparison the caller can already make directly.
///
/// `Ok(None)` is authority intact. `Ok(Some(reason))` is for the
/// caller to decide what drift means for its own outcome type: F03
/// terminalizes the row to `invalidated` with it; F06 (RFC 096 `:822-
/// 824`, "grants no authentication authority by itself") has no
/// `Invalidated` outcome of its own to report one through, and simply
/// refuses the write.
fn authority_drift_within_tx(
    tx: &rusqlite::Transaction<'_>,
    provider_id: sui_id_shared::ids::FederationProviderId,
    user_id: UserId,
) -> StoreResult<Option<InvalidatedReason>> {
    if !crate::repos::federation_provider::is_enabled_within_tx(tx, provider_id)? {
        return Ok(Some(InvalidatedReason::ProviderDisabled));
    }
    if !crate::repos::federation_link::exists_within_tx(tx, user_id, provider_id)? {
        return Ok(Some(InvalidatedReason::LinkChanged));
    }
    if !crate::repos::users::is_active_within_tx(tx, user_id)? {
        return Ok(Some(InvalidatedReason::UserInactive));
    }
    Ok(None)
}

// ── F06 — begin/replace the federated WebAuthn ceremony (Protocol) ─────

/// How long a federated WebAuthn ceremony outlives its own creation,
/// before `:794-798`'s "expiry no later than the pending row" caps it
/// down further. 60 seconds — the typical browser-side WebAuthn
/// ceremony timeout, not the parent row's own 300-second budget
/// (`federation_mfa_pending::PENDING_LIFETIME_SECS`): in practice the
/// pending row is already partway through its own 300 seconds by the
/// time F06 runs (F02 created it first), so this is almost always the
/// tighter of the two caps — stated here rather than left for a reader
/// to notice only the `expires_at <= pending_expires_at` `CHECK` ever
/// binds.
const FEDERATED_WEBAUTHN_CEREMONY_LIFETIME_SECS: i64 = 60;

/// F06 itself, RFC 096 `:822-824` plus `contracts/write-commands.toml`'s
/// own row: "require pending F02 row count 0–4 and exact unexpired
/// provider/version/generation/link/user; atomically replace at most
/// one `FederatedLogin` ceremony bound to parent/RP/origin/challenge
/// and capped expiry." Class P: grants no authentication/session
/// authority by itself, and — unlike F03 — has no `Invalidated`
/// outcome to report a guard failure through, since it never
/// terminalizes the pending row; every guard failure is simply
/// `Err(StoreError::Conflict)`, the refusal shape every other guarded
/// write in this crate's federation tables already uses.
///
/// `ceremony_state_sealed` arrives already sealed — this function's
/// caller is `sui-id-core` (RFC 096-B1 stage 11 dispatch: "webauthn-rs
/// is a `sui-id-core` dependency, not `sui-id-store`'s"), which builds
/// the `webauthn_rs::prelude::PasskeyAuthentication` state and seals it
/// with `sui_id_store::crypto::seal`/`db.key()` directly (both already
/// `pub`, the same route `communication::mail::outbox` uses from the
/// same crate) before calling this. This function never opens it: F06
/// only needs to *store* the bytes, and the one thing that would ever
/// need to open them durably — F03's own tamper/freshness recheck — is
/// F03's job, not this one's.
///
/// The durably re-read pending row's own
/// `provider_id`/`provider_config_version`/`provider_activation_generation`/
/// `user_id`/`expires_at` are what get written into the new ceremony
/// row's composite-FK columns — not whatever the caller assumed when it
/// built `ceremony_state_sealed`'s AAD. RFC 096-B2/M2b's own durable
/// provider version/generation columns do not exist yet (stage 10a's
/// own verifier doc states this directly: every provider's version/
/// generation is fixed at `(0, 0)` today), so there is currently no way
/// for those two values to actually drift between the caller's own
/// read and this transaction; `user_id`/`provider_id` are immutable for
/// a given pending row's lifetime. The caller/durable-read mismatch
/// this would otherwise create is therefore a latent gap, not a live
/// one — named here rather than silently assumed away, the same
/// position stage 10a already took on the identical limitation.
pub async fn begin_federation_webauthn_ceremony(
    db: &crate::Database,
    pending_id: sui_id_shared::ids::FederationMfaPendingId,
    ceremony_state_sealed: Vec<u8>,
    rp_id: String,
    origin: String,
    now: DateTime<Utc>,
) -> StoreResult<sui_id_shared::ids::FederationWebauthnCeremonyId> {
    db.protocol(move |write| {
        let tx = write.tx();
        let row = crate::repos::federation_mfa_pending::get_within_tx(tx, pending_id)?
            .ok_or(StoreError::NotFound)?;
        if row.status != crate::models::FederationMfaPendingStatus::Pending {
            return Err(StoreError::Conflict);
        }
        if row.expires_at <= now {
            return Err(StoreError::Conflict);
        }
        if authority_drift_within_tx(tx, row.provider_id, row.user_id)?.is_some() {
            return Err(StoreError::Conflict);
        }
        if !crate::repos::user_webauthn_credentials::has_any_within_tx(tx, row.user_id)? {
            return Err(StoreError::Conflict);
        }
        let id = sui_id_shared::ids::FederationWebauthnCeremonyId::new();
        let expires_at = std::cmp::min(
            now + chrono::Duration::seconds(FEDERATED_WEBAUTHN_CEREMONY_LIFETIME_SECS),
            row.expires_at,
        );
        let ceremony = crate::models::FederationWebauthnCeremonyRow {
            id,
            pending_id: row.id,
            provider_id: row.provider_id,
            provider_config_version: row.provider_config_version,
            provider_activation_generation: row.provider_activation_generation,
            user_id: row.user_id,
            ceremony_state_sealed,
            rp_id,
            origin,
            created_at: now,
            pending_expires_at: row.expires_at,
            expires_at,
        };
        crate::repos::federation_webauthn_ceremony::insert_or_replace_within_tx(tx, &ceremony)?;
        Ok(id)
    })
    .await
}

/// The shared tail of [`complete_federated_mfa`]'s `Promoted` branch —
/// link observation/bookkeeping, session insert, cap eviction — common
/// to both methods once the row's own guarded `status -> completed`
/// write and the method-specific anti-replay consumption have both
/// already succeeded.
fn promote(
    tx: &rusqlite::Transaction<'_>,
    row: &crate::models::FederationMfaPendingRow,
    now: DateTime<Utc>,
    method: sui_id_shared::AuthMethod,
) -> StoreResult<F03Outcome> {
    crate::repos::federation_link::touch_last_seen_within_tx(
        tx,
        row.user_id,
        row.provider_id,
        now,
    )?;
    crate::repos::users::record_federated_login_within_tx(tx, row.user_id, now)?;
    let session = crate::models::SessionRow {
        id: sui_id_shared::ids::SessionId::new(),
        user_id: row.user_id,
        expires_at: now + chrono::Duration::hours(12),
        created_at: now,
        revoked_at: None,
        auth_methods: vec![sui_id_shared::AuthMethod::Fed, method],
        last_step_up_at: (method != sui_id_shared::AuthMethod::RecoveryCode).then_some(now),
        last_used_at: None,
    };
    crate::repos::sessions::insert_within_tx(tx, &session)?;
    // The same vocabulary `SecondFactorProof::method()` (L02) already
    // writes here — "totp"/"recovery_code", not the RFC 8176 `amr`
    // token (`as_amr()` gives "otp" for both, which would collapse
    // the distinction this column exists to keep).
    let method_str = match method {
        sui_id_shared::AuthMethod::Totp => "totp",
        sui_id_shared::AuthMethod::RecoveryCode => "recovery_code",
        sui_id_shared::AuthMethod::Webauthn => "webauthn",
        _ => unreachable!("promote is only called with Totp, RecoveryCode, or Webauthn"),
    };
    if method != sui_id_shared::AuthMethod::RecoveryCode {
        crate::repos::sessions::set_step_up_method_within_tx(tx, session.id, method_str)?;
    }
    let evicted = evict_over_cap_within_tx(tx, row.user_id, now)?;
    Ok(F03Outcome::Promoted {
        session,
        evicted,
        method: method_str,
    })
}

/// The shared guarded-increment step for a wrong/unsupported proof,
/// whichever method it came from.
///
/// RFC 096 `:767-768`/`:798-801`: a ceremony bound to this row is
/// deleted here in two cases — a wrong WebAuthn completion "consumes
/// only that failed ceremony" regardless of whether this attempt also
/// exhausts the row, and reaching exactly the fifth failure (by *any*
/// reason) "deletes bound method ceremonies" outright, since
/// exhaustion ends every retry path at once, not just WebAuthn's.
/// [`federation_webauthn_ceremony::delete_by_pending_id_within_tx`]'s
/// own doc is why one unconditional, idempotent call covers both:
/// there is at most one ceremony per row, so neither case can
/// over-reach by deleting one that was not theirs.
fn reject_or_exhaust(
    tx: &rusqlite::Transaction<'_>,
    pending_id: sui_id_shared::ids::FederationMfaPendingId,
    reason: MfaRejectionReason,
) -> StoreResult<F03Outcome> {
    let outcome =
        match crate::repos::federation_mfa_pending::increment_failure_within_tx(tx, pending_id)? {
            Some(FailureIncrement::StillPending { failure_count }) => {
                F03Outcome::RejectedStillPending {
                    failure_count,
                    reason,
                }
            }
            Some(FailureIncrement::Exhausted) => F03Outcome::AttemptsExhausted { reason },
            None => return Err(StoreError::Conflict),
        };
    if reason == MfaRejectionReason::Webauthn
        || matches!(outcome, F03Outcome::AttemptsExhausted { .. })
    {
        crate::repos::federation_webauthn_ceremony::delete_by_pending_id_within_tx(tx, pending_id)?;
    }
    Ok(outcome)
}

async fn emit_f03_event(db: &crate::Database, outcome: &F03Outcome, at: DateTime<Utc>) {
    use crate::repos::audit::emit_must_attempt;
    use crate::repos::federation_mfa_pending::{
        AUDIT_MFA_EXHAUSTED_DESCRIPTOR, AUDIT_MFA_INVALIDATED_DESCRIPTOR,
        AUDIT_MFA_REJECTED_DESCRIPTOR, AUDIT_MFA_SUCCESS_DESCRIPTOR,
    };
    match outcome {
        F03Outcome::Promoted {
            session,
            evicted,
            method,
        } => {
            let attributes = AuditAttributes::builder()
                .attribute("method", *method)
                .attribute("evicted", evicted.to_string())
                .build();
            let _ = emit_must_attempt(
                db,
                &AUDIT_MFA_SUCCESS_DESCRIPTOR,
                at,
                Some(session.user_id),
                Some(AuditTarget(session.user_id.to_string())),
                "ok",
                attributes,
            )
            .await;
        }
        F03Outcome::RejectedStillPending { reason, .. } => {
            let attributes = AuditAttributes::builder()
                .attribute("reason", reason.as_str())
                .build();
            let _ = emit_must_attempt(
                db,
                &AUDIT_MFA_REJECTED_DESCRIPTOR,
                at,
                None,
                None,
                "denied",
                attributes,
            )
            .await;
        }
        F03Outcome::AttemptsExhausted { reason } => {
            let attributes = AuditAttributes::builder()
                .attribute("reason", reason.as_str())
                .build();
            let _ = emit_must_attempt(
                db,
                &AUDIT_MFA_EXHAUSTED_DESCRIPTOR,
                at,
                None,
                None,
                "denied",
                attributes,
            )
            .await;
        }
        F03Outcome::Invalidated { reason } => {
            let attributes = AuditAttributes::builder()
                .attribute("reason", reason.as_str())
                .build();
            let _ = emit_must_attempt(
                db,
                &AUDIT_MFA_INVALIDATED_DESCRIPTOR,
                at,
                None,
                None,
                "denied",
                attributes,
            )
            .await;
        }
    }
}

// ── L02 — second-factor sign-in (RFC 102 Part A) ────────────────────────

static L02_SUCCESS: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthMfaSuccess,
    name: "auth.mfa.success",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "method",
            description: "the second factor: \"totp\", \"recovery_code\" or \"webauthn\"",
            required: false,
        },
        AttributeSpec {
            name: "evicted",
            description: "older sessions revoked to keep the user within the concurrent-session cap",
            required: false,
        },
    ],
};

crate::declare_write_command! {
    /// L02 — a completed second-factor sign-in. The actor is the user whose
    /// password produced the pending row and whose factor `sui-id-core`
    /// verified outside this transaction.
    command L02 = "L02" {
        system_principal: forbidden;
        enum L02Event {
            Success { user_id: UserId, method: &'static str, evicted: i64 } => &L02_SUCCESS,
        }
    }
}

impl SealedCommandEvent<L02> for L02Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Success { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Success {
            method, evicted, ..
        } = self;
        AuditAttributes::builder()
            .attribute("method", *method)
            .attribute("evicted", evicted.to_string())
            .build()
    }
}

/// The second factor L02 commits, with what its guard compares.
pub enum SecondFactorProof {
    /// A TOTP code matched `step`; the stored step must still be below it.
    Totp { step: i64 },
    /// A recovery code matched in the blob `expected_sealed`; the blob is
    /// replaced by `new_sealed` (the same codes without that one) only if
    /// it is unchanged.
    RecoveryCode {
        expected_sealed: Vec<u8>,
        new_sealed: Vec<u8>,
    },
    /// A WebAuthn assertion was verified (its counter update is U29's).
    Webauthn,
}

impl SecondFactorProof {
    fn method(&self) -> &'static str {
        match self {
            Self::Totp { .. } => "totp",
            Self::RecoveryCode { .. } => "recovery_code",
            Self::Webauthn => "webauthn",
        }
    }
}

/// Run L02. In one transaction: consume the pending row (it must exist,
/// belong to the session's user and be unexpired), apply the factor's
/// guard, re-read the user as active and unlocked, reset the password
/// counter, stale lock and second-factor failure count, set
/// `last_login_at`, insert `session` with freshness decided **by method**
/// (TOTP and WebAuthn are fresh; a recovery code is not, RFC 102 N5), and
/// evict over-cap sessions. Any guard that loses rolls back with
/// `NotFound`.
pub async fn complete_second_factor(
    db: &crate::Database,
    pending_id: sui_id_shared::ids::PendingMfaId,
    mut session: crate::models::SessionRow,
    proof: SecondFactorProof,
) -> StoreResult<crate::registry::Audited<crate::models::SessionRow>> {
    let user_id = session.user_id;
    let context = AuthorizedCommandContext::<L02>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, L02>| {
        let now = session.created_at;
        crate::repos::login_pending_mfa::consume_within_tx(tx.tx(), pending_id, user_id, now)?;
        match &proof {
            SecondFactorProof::Totp { step } => {
                crate::repos::user_totp::advance_last_used_step_within_tx(tx.tx(), user_id, *step)?
            }
            SecondFactorProof::RecoveryCode {
                expected_sealed,
                new_sealed,
            } => crate::repos::user_totp::swap_recovery_codes_within_tx(
                tx.tx(),
                user_id,
                expected_sealed,
                new_sealed,
            )?,
            SecondFactorProof::Webauthn => {}
        }
        crate::repos::users::record_second_factor_login_within_tx(tx.tx(), user_id, now)?;
        let method = proof.method();
        let fresh = !matches!(proof, SecondFactorProof::RecoveryCode { .. });
        session.last_step_up_at = fresh.then_some(now);
        crate::repos::sessions::insert_within_tx(tx.tx(), &session)?;
        if fresh {
            crate::repos::sessions::set_step_up_method_within_tx(tx.tx(), session.id, method)?;
        }
        let evicted = evict_over_cap_within_tx(tx.tx(), user_id, now)?;
        Ok((
            session,
            L02Event::Success {
                user_id,
                method,
                evicted,
            },
        ))
    })
    .await
}

// ── L07 — second-factor failure (RFC 102 A8) ────────────────────────────

static L07_FAILURE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthMfaFailure,
    name: "auth.mfa.failure",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "count",
        description: "consecutive wrong second factors for this user, after this one",
        required: false,
    }],
};

static L07_LOCKOUT: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthMfaLockout,
    name: "auth.mfa.lockout",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "count",
            description: "consecutive wrong second factors that reached the lockout threshold",
            required: false,
        },
        AttributeSpec {
            name: "locked_for_secs",
            description: "length of the account lock, from U22's backoff",
            required: false,
        },
    ],
};

/// Consecutive wrong second factors at which every pending-MFA row for the
/// user is deleted and the account is locked (RFC 102 A8).
pub const MFA_FAILURE_LOCKOUT_THRESHOLD: i64 = 5;

crate::declare_write_command! {
    /// L07 — a wrong second factor at sign-in, counted on the user. The
    /// actor is the user whose password produced the pending row.
    command L07 = "L07" {
        system_principal: forbidden;
        enum L07Event {
            Failure { user_id: UserId, count: i64 } => &L07_FAILURE,
            Lockout { user_id: UserId, count: i64, locked_for_secs: i64 } => &L07_LOCKOUT,
        }
    }
}

impl SealedCommandEvent<L07> for L07Event {
    fn target(&self) -> Option<AuditTarget> {
        match self {
            Self::Failure { user_id, .. } | Self::Lockout { user_id, .. } => {
                Some(AuditTarget(user_id.to_string()))
            }
        }
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        match self {
            Self::Failure { count, .. } => AuditAttributes::builder()
                .attribute("count", count.to_string())
                .build(),
            Self::Lockout {
                count,
                locked_for_secs,
                ..
            } => AuditAttributes::builder()
                .attribute("count", count.to_string())
                .attribute("locked_for_secs", locked_for_secs.to_string())
                .build(),
        }
    }
}

/// What L07 did: the new count, and whether the account was locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondFactorFailureOutcome {
    pub count: i64,
    pub locked: bool,
}

/// Run L07: count one wrong second factor on `user_id`. From
/// [`MFA_FAILURE_LOCKOUT_THRESHOLD`] on, delete every pending-MFA row for
/// the user and lock the account for `lock_window_for_count(count)`
/// (U22's backoff, supplied by `sui-id-core`).
pub async fn record_second_factor_failure(
    db: &crate::Database,
    user_id: UserId,
    lock_window_for_count: impl Fn(i64) -> Option<chrono::TimeDelta> + Send + 'static,
) -> StoreResult<crate::registry::Audited<SecondFactorFailureOutcome>> {
    let context = AuthorizedCommandContext::<L07>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, L07>| {
        let count = crate::repos::users::increment_mfa_failure_within_tx(tx.tx(), user_id)?;
        if count >= MFA_FAILURE_LOCKOUT_THRESHOLD {
            crate::repos::login_pending_mfa::delete_all_for_user_within_tx(tx.tx(), user_id)?;
            // The threshold always locks, even where the backoff curve
            // would not yet (it starts at 3 password failures).
            let window = lock_window_for_count(count).unwrap_or(chrono::TimeDelta::seconds(30));
            tx.tx().execute(
                "UPDATE users SET locked_until = ?1 WHERE id = ?2",
                rusqlite::params![chrono::Utc::now() + window, user_id.to_string()],
            )?;
            Ok((
                SecondFactorFailureOutcome {
                    count,
                    locked: true,
                },
                L07Event::Lockout {
                    user_id,
                    count,
                    locked_for_secs: window.num_seconds(),
                },
            ))
        } else {
            Ok((
                SecondFactorFailureOutcome {
                    count,
                    locked: false,
                },
                L07Event::Failure { user_id, count },
            ))
        }
    })
    .await
}

/// Revoke the user's oldest active sessions until the count is within
/// `server_settings.max_concurrent_sessions` (0 means no cap). Runs on the
/// caller's transaction, after the new session is inserted, so a committed
/// sign-in never leaves the user over the cap.
fn evict_over_cap_within_tx(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<i64> {
    let cap = match crate::repos::server_settings::get_within_tx(tx) {
        Ok(settings) => settings.max_concurrent_sessions,
        // No settings row yet (before setup has written one): no cap.
        Err(crate::StoreError::NotFound) => 0,
        Err(other) => return Err(other),
    };
    if cap <= 0 {
        return Ok(0);
    }
    let count = crate::repos::sessions::count_active_for_user_within_tx(tx, user_id, now)?;
    if count <= cap {
        return Ok(0);
    }
    let oldest =
        crate::repos::sessions::oldest_active_for_user_within_tx(tx, user_id, now, count - cap)?;
    let mut evicted = 0;
    for old in oldest {
        evicted += crate::repos::sessions::revoke_within_tx(tx, old.id, now)? as i64;
    }
    Ok(evicted)
}

// ── L05 — step-up success (RFC 102 Part B) ──────────────────────────────

static L05_SUCCESS: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthStepUpSuccess,
    name: "auth.step_up.success",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "method",
            description: "the factor that satisfied the step-up: \"totp\" or \"webauthn\"",
            required: false,
        },
        AttributeSpec {
            name: "gate",
            description: "the sanitised return_to path the step-up was for, truncated to 256 bytes",
            required: false,
        },
    ],
};

/// Byte bound for L05's `gate` attribute (RFC 102's L05 row).
pub const STEP_UP_GATE_ATTRIBUTE_BYTES: usize = 256;

crate::declare_write_command! {
    /// L05 — a successful step-up on a signed-in session. The actor is the
    /// session's user, whose factor `sui-id-core` verified outside this
    /// transaction.
    command L05 = "L05" {
        system_principal: forbidden;
        enum L05Event {
            Success { user_id: UserId, method: &'static str, gate: String } => &L05_SUCCESS,
        }
    }
}

impl SealedCommandEvent<L05> for L05Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Success { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Success { method, gate, .. } = self;
        AuditAttributes::builder()
            .attribute("method", *method)
            .attribute("gate", truncate_utf8(gate, STEP_UP_GATE_ATTRIBUTE_BYTES))
            .build()
    }
}

/// The factor L05 consumes, with what its guard compares.
pub enum StepUpProof {
    /// A TOTP code matched `step`; the stored step must still be below it.
    Totp { step: i64 },
    /// A WebAuthn assertion was verified against the `StepUp` ceremony
    /// `pending_id`, which L05 deletes.
    Webauthn {
        pending_id: sui_id_shared::ids::WebauthnPendingId,
    },
}

/// Run L05. In one transaction: re-read the session (exists, belongs to
/// `user_id`, unrevoked, unexpired, user active); consume the factor (the
/// guarded TOTP step advance, or the guarded delete of this user's `StepUp`
/// ceremony); set `last_step_up_at` and `last_step_up_method`; reset the
/// session's step-up failure count; and write `auth.step_up.success`. Any
/// guard that loses rolls back with `NotFound`.
pub async fn complete_step_up(
    db: &crate::Database,
    user_id: UserId,
    session_id: sui_id_shared::ids::SessionId,
    proof: StepUpProof,
    gate: String,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<L05>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, L05>| {
        crate::repos::sessions::require_live_session_within_tx(tx.tx(), session_id, user_id, now)?;
        let method = match proof {
            StepUpProof::Totp { step } => {
                crate::repos::user_totp::advance_last_used_step_within_tx(tx.tx(), user_id, step)?;
                "totp"
            }
            StepUpProof::Webauthn { pending_id } => {
                crate::repos::webauthn_pending::consume_step_up_within_tx(
                    tx.tx(),
                    pending_id,
                    user_id,
                    now,
                )?;
                "webauthn"
            }
        };
        crate::repos::sessions::record_step_up_within_tx(tx.tx(), session_id, method, now)?;
        Ok((
            (),
            L05Event::Success {
                user_id,
                method,
                gate,
            },
        ))
    })
    .await
}

// ── L06 — step-up failure (RFC 102 Part B) ──────────────────────────────

static L06_FAILURE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthStepUpFailure,
    name: "auth.step_up.failure",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "count",
        description: "consecutive failed step-up attempts on this session, after this one",
        required: false,
    }],
};

static L06_SESSION_REVOKED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthStepUpSessionRevoked,
    name: "auth.step_up.session_revoked",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "count",
        description: "consecutive failed step-up attempts that reached the revocation threshold",
        required: false,
    }],
};

/// Consecutive step-up failures on one session at which the session is
/// revoked (RFC 102 B2).
pub const STEP_UP_FAILURE_REVOCATION_THRESHOLD: i64 = 5;

crate::declare_write_command! {
    /// L06 — a failed step-up re-authentication, counted on the session;
    /// the threshold-crossing branch revokes that session. The actor is the
    /// signed-in user whose session made the attempt.
    command L06 = "L06" {
        system_principal: forbidden;
        enum L06Event {
            Failure { user_id: UserId, count: i64 } => &L06_FAILURE,
            SessionRevoked { user_id: UserId, count: i64 } => &L06_SESSION_REVOKED,
        }
    }
}

impl SealedCommandEvent<L06> for L06Event {
    fn target(&self) -> Option<AuditTarget> {
        match self {
            Self::Failure { user_id, .. } | Self::SessionRevoked { user_id, .. } => {
                Some(AuditTarget(user_id.to_string()))
            }
        }
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        match self {
            Self::Failure { count, .. } | Self::SessionRevoked { count, .. } => {
                AuditAttributes::builder()
                    .attribute("count", count.to_string())
                    .build()
            }
        }
    }
}

/// What L06 did: the new count, and whether the session was revoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepUpFailureOutcome {
    pub count: i64,
    pub session_revoked: bool,
}

/// Run L06: count one failed step-up on `session_id`, and revoke that
/// session when the count reaches [`STEP_UP_FAILURE_REVOCATION_THRESHOLD`].
/// The session must be unrevoked and belong to `user_id`; otherwise the
/// transaction rolls back with `NotFound`.
pub async fn record_step_up_failure(
    db: &crate::Database,
    user_id: UserId,
    session_id: sui_id_shared::ids::SessionId,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<crate::registry::Audited<StepUpFailureOutcome>> {
    let context = AuthorizedCommandContext::<L06>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, L06>| {
        let count = crate::repos::sessions::increment_step_up_failure_within_tx(
            tx.tx(),
            session_id,
            user_id,
        )?;
        if count >= STEP_UP_FAILURE_REVOCATION_THRESHOLD {
            crate::repos::sessions::revoke_within_tx(tx.tx(), session_id, now)?;
            Ok((
                StepUpFailureOutcome {
                    count,
                    session_revoked: true,
                },
                L06Event::SessionRevoked { user_id, count },
            ))
        } else {
            Ok((
                StepUpFailureOutcome {
                    count,
                    session_revoked: false,
                },
                L06Event::Failure { user_id, count },
            ))
        }
    })
    .await
}

// ── U12, U14, U15 — adding a second factor (RFC 102 B7) ─────────────────

/// One event for every factor addition; the three commands share it and
/// the `method` attribute says which factor.
static MFA_FACTOR_ADDED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthMfaFactorAdded,
    name: "auth.mfa.factor_added",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "method",
        description: "the factor added: `totp`, `recovery_codes` or `webauthn`",
        required: false,
    }],
};

/// The factor an [`MFA_FACTOR_ADDED`] event records. A closed set, so the
/// attribute is bounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddedFactor {
    Totp,
    RecoveryCodes,
    Webauthn,
}

impl AddedFactor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Totp => "totp",
            Self::RecoveryCodes => "recovery_codes",
            Self::Webauthn => "webauthn",
        }
    }
}

fn factor_added_attributes(method: AddedFactor) -> Result<AuditAttributes, AuditBuildError> {
    AuditAttributes::builder()
        .attribute("method", method.as_str())
        .build()
}

crate::declare_write_command! {
    /// U12 — confirm TOTP enrolment: enable it, store the sealed recovery
    /// codes and advance the replay cursor, with `auth.mfa.factor_added`.
    command U12 = "U12" {
        system_principal: forbidden;
        enum U12Event {
            Added { user_id: UserId } => &MFA_FACTOR_ADDED,
        }
    }
}

impl SealedCommandEvent<U12> for U12Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Added { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }
    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        factor_added_attributes(AddedFactor::Totp)
    }
}

/// Run U12. `sealed_recovery_codes` comes from
/// `repos::user_totp::seal_recovery_codes`; `last_used_step` is the step
/// the confirmation code verified against.
pub async fn confirm_totp_enrollment(
    db: &crate::Database,
    user_id: UserId,
    sealed_recovery_codes: Vec<u8>,
    last_used_step: i64,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U12>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U12>| {
        crate::repos::user_totp::confirm_with_recovery_within_tx(
            tx.tx(),
            user_id,
            &sealed_recovery_codes,
            last_used_step,
        )?;
        Ok(((), U12Event::Added { user_id }))
    })
    .await
}

crate::declare_write_command! {
    /// U14 — regenerate recovery codes, with `auth.mfa.factor_added`.
    command U14 = "U14" {
        system_principal: forbidden;
        enum U14Event {
            Added { user_id: UserId } => &MFA_FACTOR_ADDED,
        }
    }
}

impl SealedCommandEvent<U14> for U14Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Added { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }
    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        factor_added_attributes(AddedFactor::RecoveryCodes)
    }
}

/// Run U14 with an already-sealed blob.
pub async fn regenerate_recovery_codes(
    db: &crate::Database,
    user_id: UserId,
    sealed_recovery_codes: Vec<u8>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U14>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U14>| {
        crate::repos::user_totp::set_recovery_codes_within_tx(
            tx.tx(),
            user_id,
            &sealed_recovery_codes,
        )?;
        Ok(((), U14Event::Added { user_id }))
    })
    .await
}

crate::declare_write_command! {
    /// U15 — register a passkey, with `auth.mfa.factor_added`.
    command U15 = "U15" {
        system_principal: forbidden;
        enum U15Event {
            Added { user_id: UserId } => &MFA_FACTOR_ADDED,
        }
    }
}

impl SealedCommandEvent<U15> for U15Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Added { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }
    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        factor_added_attributes(AddedFactor::Webauthn)
    }
}

/// Run U15. `row.passkey_enc` must already hold the sealed passkey
/// (`repos::user_webauthn_credentials::seal_passkey`).
pub async fn register_passkey(
    db: &crate::Database,
    row: crate::models::UserWebauthnCredentialRow,
) -> StoreResult<crate::registry::Audited<()>> {
    let user_id = row.user_id;
    let context = AuthorizedCommandContext::<U15>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U15>| {
        crate::repos::user_webauthn_credentials::create_within_tx(tx.tx(), &row)?;
        Ok(((), U15Event::Added { user_id }))
    })
    .await
}

// ── U13, U16 — removing a second factor (cycle I stage 2) ───────────────
//
// `auth.mfa.factor_removed` is one event shared by the two commands, the
// same shape `auth.mfa.factor_added` already uses for U12/U14/U15
// (stage plan §A1): the addition side treats TOTP and passkey additions
// as one event with a `method` attribute, so the removal side must too,
// or the two halves of the same fact stop lining up in the log.
// `RecoveryCodes` has no removal counterpart — nothing removes recovery
// codes, only regenerates them (U14) — so the enum is narrower than
// `AddedFactor`.
//
// Measured 2026-10-30 (cycle I stage 1): before this stage, U13 and U16
// each had exactly one caller, a self-service HTTP handler, which
// appended a best-effort `mfa.disable` / `webauthn.credential.delete`
// row *after* a silent core-layer call. Both are retired; see the
// `CHANGELOG`.

static MFA_FACTOR_REMOVED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthMfaFactorRemoved,
    name: "auth.mfa.factor_removed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "method",
        description: "the factor removed: `totp` or `webauthn`",
        required: false,
    }],
};

/// The factor an [`MFA_FACTOR_REMOVED`] event records. Narrower than
/// [`AddedFactor`]: recovery codes are regenerated (U14), never removed
/// on their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovedFactor {
    Totp,
    Webauthn,
}

impl RemovedFactor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Totp => "totp",
            Self::Webauthn => "webauthn",
        }
    }
}

fn factor_removed_attributes(method: RemovedFactor) -> Result<AuditAttributes, AuditBuildError> {
    AuditAttributes::builder()
        .attribute("method", method.as_str())
        .build()
}

crate::declare_write_command! {
    /// U13 — disable TOTP (self-service), with `auth.mfa.factor_removed`.
    command U13 = "U13" {
        system_principal: forbidden;
        enum U13Event {
            Removed { user_id: UserId } => &MFA_FACTOR_REMOVED,
        }
    }
}

impl SealedCommandEvent<U13> for U13Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Removed { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }
    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        factor_removed_attributes(RemovedFactor::Totp)
    }
}

/// Run U13 (disable TOTP). `user_totp::delete_within_tx` does not error
/// when there is nothing to remove (RFC 094 U07's own precedent: absence
/// is routine for an MFA reset) — this command's own caller always has
/// an enabled factor to remove (the self-service page only shows the
/// "disable" action when one exists), so that absence is reported as
/// `NotFound` here instead, matching the pre-conversion
/// `user_totp::delete`'s own behaviour exactly.
pub async fn disable_totp(
    db: &crate::Database,
    user_id: UserId,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U13>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U13>| {
        let removed = crate::repos::user_totp::delete_within_tx(tx.tx(), user_id)?;
        if !removed {
            return Err(StoreError::NotFound);
        }
        Ok(((), U13Event::Removed { user_id }))
    })
    .await
}

crate::declare_write_command! {
    /// U16 — delete a passkey (self-service), with `auth.mfa.factor_removed`.
    command U16 = "U16" {
        system_principal: forbidden;
        enum U16Event {
            Removed { user_id: UserId } => &MFA_FACTOR_REMOVED,
        }
    }
}

impl SealedCommandEvent<U16> for U16Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Removed { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }
    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        factor_removed_attributes(RemovedFactor::Webauthn)
    }
}

/// Run U16 (delete a passkey). `user_webauthn_credentials::delete_within_tx`
/// already errors `NotFound` on nothing removed, matching the
/// pre-conversion `user_webauthn_credentials::delete`'s own behaviour.
pub async fn delete_passkey(
    db: &crate::Database,
    user_id: UserId,
    credential_id: sui_id_shared::ids::WebauthnCredentialId,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U16>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U16>| {
        crate::repos::user_webauthn_credentials::delete_within_tx(tx.tx(), credential_id, user_id)?;
        Ok(((), U16Event::Removed { user_id }))
    })
    .await
}

// ── U17 — renaming a passkey (cycle I stage 2) ───────────────────────────
//
// Measured 2026-10-30: no event existed under any name before this
// stage — the handler called the repo's bare `UPDATE` directly, with no
// core-layer function and no append anywhere.

static MFA_FACTOR_RENAMED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthMfaFactorRenamed,
    name: "auth.mfa.factor_renamed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "method",
        description: "the factor renamed — always `webauthn` today; only passkeys carry a nickname",
        required: false,
    }],
};

crate::declare_write_command! {
    /// U17 — rename a passkey (self-service), with `auth.mfa.factor_renamed`.
    command U17 = "U17" {
        system_principal: forbidden;
        enum U17Event {
            Renamed { user_id: UserId } => &MFA_FACTOR_RENAMED,
        }
    }
}

impl SealedCommandEvent<U17> for U17Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Renamed { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }
    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder()
            .attribute("method", RemovedFactor::Webauthn.as_str())
            .build()
    }
}

/// Run U17 (rename a passkey). `update_nickname_within_tx` errors
/// `NotFound` on nothing renamed — the pre-conversion `update_nickname`
/// did not (a bare `UPDATE`, row count discarded); sealing adds the
/// check rather than silently succeeding against a nonexistent or
/// foreign credential id.
pub async fn rename_passkey(
    db: &crate::Database,
    user_id: UserId,
    credential_id: String,
    new_nickname: String,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U17>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U17>| {
        crate::repos::user_webauthn_credentials::update_nickname_within_tx(
            tx.tx(),
            &credential_id,
            user_id,
            &new_nickname,
        )?;
        Ok(((), U17Event::Renamed { user_id }))
    })
    .await
}

// ── U18 — revoke one session (cycle I stage 2) ───────────────────────────
//
// Measured 2026-10-30: all three call sites were silent. One has no
// human actor (idle-timeout auto-revoke, inside session resolution),
// which is why this command permits the system principal, like U08/U22 —
// but unlike those, the *other* two call sites do have one, so the
// system path is reached only from the one caller that truly has none,
// not from every caller (stage plan §A2 names the same shape for S10's
// own decision, resolved there, not here).

static U18_SESSION_REVOKED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthSessionRevoked,
    name: "auth.session.revoked",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Optional,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "trigger",
        description: "why the session was revoked: `idle_timeout`, `self_signout` or `self_service`",
        required: true,
    }],
};

/// Which of [`U18_SESSION_REVOKED`]'s three call sites revoked the
/// session. A closed set, so the attribute is bounded — the same shape
/// [`RemovedFactor`] gives `method`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRevokeTrigger {
    /// `sui-id-core/src/authn/session.rs`'s idle-timeout enforcement,
    /// inside session resolution. No human actor.
    IdleTimeout,
    /// The operator's own `/admin/logout`.
    SelfSignout,
    /// The user's own `/me/security/sessions/{id}/revoke`.
    SelfService,
}

impl SessionRevokeTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IdleTimeout => "idle_timeout",
            Self::SelfSignout => "self_signout",
            Self::SelfService => "self_service",
        }
    }
}

crate::declare_write_command! {
    /// U18 — revoke one session, with `auth.session.revoked`.
    command U18 = "U18" {
        system_principal: permitted;
        enum U18Event {
            Revoked { user_id: UserId, trigger: SessionRevokeTrigger } => &U18_SESSION_REVOKED,
        }
    }
}

impl SealedCommandEvent<U18> for U18Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Revoked { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }
    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Revoked { trigger, .. } = self;
        AuditAttributes::builder()
            .attribute("trigger", trigger.as_str())
            .build()
    }
}

/// Run U18 for a caller with a human actor — self sign-out
/// (`trigger: SelfSignout`) or self-service single-session revoke
/// (`trigger: SelfService`). `sessions::revoke_within_tx` is silently
/// idempotent (0 rows changed if already revoked) rather than erroring,
/// matching the pre-conversion best-effort callers' own tolerance of a
/// session that is already gone.
pub async fn revoke_session(
    db: &crate::Database,
    actor: UserId,
    user_id: UserId,
    session_id: SessionId,
    trigger: SessionRevokeTrigger,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U18>::for_authorized_actor(actor, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U18>| {
        let _ = crate::repos::sessions::revoke_within_tx(tx.tx(), session_id, now)?;
        Ok(((), U18Event::Revoked { user_id, trigger }))
    })
    .await
}

/// Run U18 for the one caller with no human actor: idle-timeout
/// auto-revocation during session resolution. `trigger` is always
/// [`SessionRevokeTrigger::IdleTimeout`] — fixed here, not a parameter,
/// so no other caller can reach the system principal through this
/// function (the same discipline U07's `operator_reset_mfa` uses).
pub async fn revoke_session_idle_timeout(
    db: &crate::Database,
    user_id: UserId,
    session_id: SessionId,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U18>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U18>| {
        let _ = crate::repos::sessions::revoke_within_tx(tx.tx(), session_id, now)?;
        Ok((
            (),
            U18Event::Revoked {
                user_id,
                trigger: SessionRevokeTrigger::IdleTimeout,
            },
        ))
    })
    .await
}

// ── U19 — revoke all sessions / force logout (cycle I stage 2, amended
//    stage 2b) ───────────────────────────────────────────────────────────
//
// Measured 2026-10-30: `revoke_all_for_user` (RP-initiated logout) was
// silent; `revoke_all_for_user_except` (self-service "sign out other
// sessions") appended the best-effort `auth.sessions.bulk_revoke_self`.
// One event covers both surfaces now; that name is retired (`CHANGELOG`).
//
// RP-initiated logout can identify the user one of two ways — a validated
// session, or `id_token_hint` alone — and the end-session handler is the
// only place that knows which one ran. Only the session-backed path has a
// provable actor, so (like U18) this command permits the system
// principal, and gains the same two-entry-function split: no caller can
// reach the unattributed path with an actor, or the attributed path
// without one. Self-service always holds a validated session.

static U19_SESSIONS_REVOKED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthSessionsRevoked,
    name: "auth.sessions.revoked",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Optional,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "trigger",
            description: "`rp_initiated_logout` or `self_service`",
            required: true,
        },
        AttributeSpec {
            name: "count",
            description: "number of sessions revoked",
            required: true,
        },
        AttributeSpec {
            name: "identified_by",
            description: "how the user was identified: `session` or `id_token_hint`",
            required: true,
        },
    ],
};

/// Which of [`U19_SESSIONS_REVOKED`]'s two surfaces revoked the sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionsRevokeTrigger {
    /// RP-initiated OIDC logout (`GET /oidc/logout`), via
    /// `sui_id_core::session::logout_user`.
    RpInitiatedLogout,
    /// The user's own "sign out other sessions" button.
    SelfService,
}

impl SessionsRevokeTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RpInitiatedLogout => "rp_initiated_logout",
            Self::SelfService => "self_service",
        }
    }
}

/// How the identity behind a U19 revocation was established. A closed
/// set, the same shape [`SessionsRevokeTrigger`] already gives `trigger` —
/// so the distinction is in the row, not only in the actor column being
/// empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionsIdentifiedBy {
    /// A validated session: self-service, or RP-initiated logout via the
    /// session-cookie fallback.
    Session,
    /// `id_token_hint` alone, with no session involved.
    IdTokenHint,
}

impl SessionsIdentifiedBy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::IdTokenHint => "id_token_hint",
        }
    }
}

crate::declare_write_command! {
    /// U19 — revoke all sessions for a user, with `auth.sessions.revoked`.
    command U19 = "U19" {
        system_principal: permitted;
        enum U19Event {
            Revoked { user_id: UserId, trigger: SessionsRevokeTrigger, count: usize, identified_by: SessionsIdentifiedBy } => &U19_SESSIONS_REVOKED,
        }
    }
}

impl SealedCommandEvent<U19> for U19Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Revoked { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }
    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Revoked {
            trigger,
            count,
            identified_by,
            ..
        } = self;
        AuditAttributes::builder()
            .attribute("trigger", trigger.as_str())
            .attribute("count", count.to_string())
            .attribute("identified_by", identified_by.as_str())
            .build()
    }
}

/// Run U19 for RP-initiated logout when identity was established by a
/// validated session (the session-cookie fallback). The actor is
/// `user_id`.
pub async fn revoke_all_sessions_for_user_as_user(
    db: &crate::Database,
    user_id: UserId,
    trigger: SessionsRevokeTrigger,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<usize>> {
    let context = AuthorizedCommandContext::<U19>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U19>| {
        let count = crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), user_id, now)?;
        Ok((
            count,
            U19Event::Revoked {
                user_id,
                trigger,
                count,
                identified_by: SessionsIdentifiedBy::Session,
            },
        ))
    })
    .await
}

/// Run U19 for RP-initiated logout when identity was established by
/// `id_token_hint` alone, with no session. The system principal —
/// `identified_by` is fixed here, not a parameter, so no caller can reach
/// an actor through this function (the same discipline U18's
/// `revoke_session_idle_timeout` uses).
pub async fn revoke_all_sessions_for_user_unattributed(
    db: &crate::Database,
    user_id: UserId,
    trigger: SessionsRevokeTrigger,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<usize>> {
    let context = AuthorizedCommandContext::<U19>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U19>| {
        let count = crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), user_id, now)?;
        Ok((
            count,
            U19Event::Revoked {
                user_id,
                trigger,
                count,
                identified_by: SessionsIdentifiedBy::IdTokenHint,
            },
        ))
    })
    .await
}

/// Run U19 for the self-service "sign out other sessions" surface:
/// revoke every session for `user_id` except `keep`. Unchanged from stage
/// 2 — this caller always holds a validated `SessionContext`, so the
/// actor is proven and `identified_by` is fixed to [`SessionsIdentifiedBy::Session`].
pub async fn revoke_other_sessions_for_user(
    db: &crate::Database,
    user_id: UserId,
    keep: SessionId,
    trigger: SessionsRevokeTrigger,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<usize>> {
    let context = AuthorizedCommandContext::<U19>::for_authorized_actor(user_id, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U19>| {
        let count = crate::repos::sessions::revoke_all_for_user_except_within_tx(
            tx.tx(),
            user_id,
            keep,
            now,
        )?;
        Ok((
            count,
            U19Event::Revoked {
                user_id,
                trigger,
                count,
                identified_by: SessionsIdentifiedBy::Session,
            },
        ))
    })
    .await
}

// ── C15 — dynamic client registration ────────────────────────────────────

static C15_DYNAMIC_REGISTER: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientDynamicRegister,
    name: "client.dynamic_register",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "client_name",
        description: "the registered client's self-reported name",
        // Always populated in practice (the handler validates a non-empty
        // `client_name` before calling this command), but `required` here
        // is RFC 102 stage 7's step-up invariant's vocabulary (B4: every
        // step-up-gated descriptor's `step_up` attribute, and no other
        // descriptor requires anything else) -- not a general-purpose
        // presence guarantee `AuditAttributesBuilder::build` enforces.
        required: false,
    }],
};

crate::declare_write_command! {
    /// C15 — RFC 7591 dynamic client registration. `permitted`,
    /// `ActorRequirement::None`: the caller is authorized by possession of
    /// the registration token consumed inside this same transaction, not
    /// by an authenticated session — the same shape as U10's reset-token
    /// presenter, not U01-U15's authenticated actor.
    command C15 = "C15" {
        system_principal: permitted;
        enum C15Event {
            Registered { client_id: ClientId, client_name: String } => &C15_DYNAMIC_REGISTER,
        }
    }
}

impl SealedCommandEvent<C15> for C15Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Registered { client_id, .. } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Registered { client_name, .. } = self;
        AuditAttributes::builder()
            .attribute("client_name", client_name.clone())
            .build()
    }
}

/// Run C15 (dynamic client registration) through the Class-A runner.
///
/// **The caller must validate the request body before calling this.** The
/// token consumption inside this transaction is guarded against reuse, but
/// it is not guarded against a body that turns out malformed — that
/// ordering (validate first, consume second) is the handler's
/// responsibility, not this function's, because validation needs no
/// database access and holding a write transaction open across it would
/// only widen the lock window for no benefit (RFC 094 M2a, 2026-10-02).
///
/// Returns `Err(StoreError::NotFound)` when the token is invalid, expired,
/// revoked, or exhausted — nothing is written, and the caller maps this to
/// its own "invalid token" response, the same convention
/// `consume_and_reset_password` (U10) already established for its
/// token-presenter shape.
///
/// On success: consumes the token, creates the client row, and stamps its
/// `registered_via`, all in one transaction. `clients::create_within_tx`
/// leaves `registered_via` at its table default (RFC 094 C11's own reason
/// to exist as a command distinct from C15's `create` write), so the
/// `set_registered_via_within_tx` call below is not redundant with the row
/// just inserted — it is the only place that column is ever actually set
/// for this path.
///
/// **Two statements, one transaction, and that is the registered shape.**
/// `contracts/write-commands.toml` declares C15's mutation surface as
/// *"guarded `client_registration_token::consume`, `clients::create`,
/// `set_registered_via` in one RFC 094 transaction"*, and G17 checks it.
/// RFC 095 `:362-366` reads differently — it says the insert writes the
/// source *"in one statement"* and that separate `set_registered_via`
/// writes are *"forbidden in the dynamic path"*. **The code is not being
/// changed to match that sentence**; the two-call shape is the RFC 094
/// Stage-0 classification, atomicity is preserved by the transaction, and
/// `create_leaves_registered_via_at_the_table_default` exists to stop
/// exactly that "fix". The RFC sentence is what is out of step, and
/// correcting an Accepted RFC's text is not the implementation's call.
pub async fn register_client_dynamically(
    db: &crate::Database,
    token_hash: String,
    row: crate::models::ClientRow,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<C15>::for_system_actor(None);
    let client_id = row.id;
    let client_name = row.name.clone();
    let via = row.registered_via.clone();
    db.class_a(context, move |tx: &mut ClassATx<'_, C15>| {
        let consumed =
            crate::repos::client_registration_token::consume_within_tx(tx.tx(), &token_hash, now)?;
        if !consumed {
            return Err(crate::StoreError::NotFound);
        }
        crate::repos::clients::create_within_tx(tx.tx(), &row)?;
        crate::repos::clients::set_registered_via_within_tx(tx.tx(), client_id, via, now)?;
        Ok((
            (),
            C15Event::Registered {
                client_id,
                client_name,
            },
        ))
    })
    .await
}

// ── O01 — enqueue email (Operational) ────────────────────────────────────

/// Run O01 (enqueue email) through the `Operational` runner. Same
/// no-`Audited<T>`-path property as `Protocol`.
pub async fn enqueue_email(
    db: &crate::Database,
    row: crate::models::EmailOutboxRow,
) -> StoreResult<()> {
    db.operational(move |write| crate::repos::email_outbox::enqueue_within_tx(write.tx(), &row))
        .await
}

// ── S07 — create pending settings change ─────────────────────────────────
//
// RFC 094 M2b stage 1. Before this stage, `sui_id_core::settings::
// pending_change::create` called `pending_settings_change::insert` (which
// commits on its own) and then a best-effort `let _ = audit::append(...)`
// -- a sensitive settings change could be stored with no record of it if
// the append failed. The mutation and the audit lived in different
// crates with nothing between them; this command is the seam that joins
// them, which the three remaining M2b waves copy.

static S07_CREATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SettingsPendingChangeCreated,
    name: "settings.pending_change.created",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "intent",
        description: "the caller-supplied intent string naming what kind of \
            settings change this is (e.g. \"smtp_password_change\")",
        required: true,
    }],
};

crate::declare_write_command! {
    /// S07 — create a pending settings change (RFC 090, RFC 094 M2b stage
    /// 1). Its only caller, `pending_change::create`, requires
    /// `&AdminActor` -- only an administrator reaches this, so the actor
    /// is required, matching `K01`'s reasoning for the same shape.
    command S07 = "S07" {
        system_principal: forbidden;
        enum S07Event {
            Created {
                id: PendingChangeId,
                intent: String,
            } => &S07_CREATED,
        }
    }
}

impl SealedCommandEvent<S07> for S07Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Created { id, .. } = self;
        Some(AuditTarget(id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Created { intent, .. } = self;
        AuditAttributes::builder()
            .attribute("intent", intent.clone())
            .build()
    }
}

/// Run S07 (create a pending settings change) through the Class-A runner.
///
/// `row` is built by the caller before this is called: its payload is
/// already AES-GCM-sealed (RFC 094 keeps crypto outside the transaction),
/// and `row.id`/`row.intent`/`row.summary`/`row.expires_at` are already
/// everything `PendingChange` (the core-level return type) needs -- none
/// of it depends on the insert having committed. So this command returns
/// `()`, not a value: the K01 template's `(value, event)` shape did not
/// force a change here, and the caller already has what it needs from
/// the `row` it built.
///
/// Only `intent` becomes an attribute, not `summary`: `summary` is
/// caller-supplied free text meant for the confirm page, and `AuditAttributes`
/// is a closed, bounded vocabulary, not a place for arbitrary text to
/// accumulate (dispatch §2). The row's id is already the event's target,
/// so it is not duplicated into an attribute.
pub async fn create_pending_settings_change(
    db: &crate::Database,
    admin: UserId,
    row: crate::repos::pending_settings_change::PendingSettingsChangeRow,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<S07>::for_authorized_actor(admin, None);
    let id = row.id;
    let intent = row.intent.clone();
    db.class_a(context, move |tx: &mut ClassATx<'_, S07>| {
        crate::repos::pending_settings_change::insert_within_tx(tx.tx(), &row)?;
        Ok(((), S07Event::Created { id, intent }))
    })
    .await
}

// ── S09 — cancel pending settings change ──────────────────────────────────

static S09_CANCELLED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SettingsPendingChangeCancelled,
    name: "settings.pending_change.cancelled",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// S09 — cancel a pending settings change (RFC 090, RFC 094 M2b
    /// stage 1). Same actor shape as S07 -- `pending_change::cancel`
    /// also requires `&AdminActor`. Unlike S07, measured: no HTTP route
    /// calls `pending_change::cancel` today (there is no
    /// `/admin/settings/*/cancel` handler). `Required` still matches who
    /// *can* reach the call, which is what the type signature commits
    /// to regardless of whether a route exists yet -- the same
    /// foundation-before-wiring shape this module's own header describes
    /// for K01/U01 at M2a.
    command S09 = "S09" {
        system_principal: forbidden;
        enum S09Event {
            Cancelled {
                id: PendingChangeId,
            } => &S09_CANCELLED,
        }
    }
}

impl SealedCommandEvent<S09> for S09Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Cancelled { id } = self;
        Some(AuditTarget(id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run S09 (cancel a pending settings change) through the Class-A runner.
///
/// No `intent` attribute, unlike S07: cancel does not read the row before
/// deleting it (the pre-stage `cancel` never did either), so intent is
/// not known without an extra lookup this stage does not add. Idempotent,
/// like the pre-stage `cancel`: a missing row (already applied,
/// cancelled, expired, or purged) is a correct outcome, not an error, and
/// still emits the cancellation event -- no behaviour change for a
/// caller that succeeds (dispatch §4).
pub async fn cancel_pending_settings_change(
    db: &crate::Database,
    admin: UserId,
    id: PendingChangeId,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<S09>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, S09>| {
        crate::repos::pending_settings_change::cancel_within_tx(tx.tx(), id)?;
        Ok(((), S09Event::Cancelled { id }))
    })
    .await
}

// ── S08 — apply pending settings change ──────────────────────────────────
//
// RFC 094 M2b stage 2. Before this stage, `apply` consumed the pending
// row (committing), then appended a best-effort audit row, then returned
// the decrypted payload for the caller to write `smtp_config` itself --
// three commits for one logical operation, and a failure after the first
// left the row consumed with nothing changed (stage plan §A). This
// command is the restructure: the caller validates and seals the payload
// *before* calling in (RFC 094 keeps crypto outside the transaction),
// and this one transaction re-reads the row, checks the binding, deletes
// it only if the binding matches, writes `smtp_config`, and emits the
// event -- success and denial alike.

static S08_APPLIED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SettingsPendingChangeApplied,
    name: "settings.pending_change.applied",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "intent",
        description: "the pending change's own intent string",
        required: true,
    }],
};

static S08_BINDING_FAILED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SettingsPendingChangeBindingFailed,
    name: "settings.pending_change.binding_failed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "intent",
        description: "the pending change's own intent string",
        required: true,
    }],
};

crate::declare_write_command! {
    /// S08 — apply a pending settings change (RFC 090, RFC 094 M2b stage
    /// 2). Same actor shape as S07/S09: `pending_change::apply` requires
    /// `&AdminActor`.
    ///
    /// Two event variants, not one, per Decision B (stage plan §3): a
    /// binding failure must commit a denial, not roll back as a
    /// transaction error, so it cannot share `Applied`'s arm.
    command S08 = "S08" {
        system_principal: forbidden;
        enum S08Event {
            Applied {
                id: PendingChangeId,
                intent: String,
            } => &S08_APPLIED,
            BindingFailed {
                id: PendingChangeId,
                intent: String,
            } => &S08_BINDING_FAILED,
        }
    }
}

impl SealedCommandEvent<S08> for S08Event {
    fn target(&self) -> Option<AuditTarget> {
        let (Self::Applied { id, .. } | Self::BindingFailed { id, .. }) = self;
        Some(AuditTarget(id.to_string()))
    }

    fn result(&self) -> AuditResult {
        match self {
            Self::Applied { .. } => AuditResult::Ok,
            Self::BindingFailed { .. } => AuditResult::Failure,
        }
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let (Self::Applied { intent, .. } | Self::BindingFailed { intent, .. }) = self;
        AuditAttributes::builder()
            .attribute("intent", intent.clone())
            .build()
    }
}

/// What the caller learns from S08: whether the pending change was
/// actually applied, or refused on a binding mismatch. The caller maps
/// `BindingFailed` to the same neutral "expired or no longer valid"
/// message absence already uses (stage plan §3: non-disclosure, for
/// binding failure and for absence alike) -- this type exists so that
/// mapping happens once, at the one call site, rather than the command
/// returning an `Err` that would be indistinguishable from a real
/// transaction failure.
pub enum ApplyPendingSettingsChangeOutcome {
    Applied,
    BindingFailed,
}

/// Run S08 (apply a pending settings change) through the Class-A runner.
///
/// `new_smtp_config` is built by the caller *before* this is called: the
/// payload has already been decrypted, validated (`tls_mode`), and its
/// password re-sealed under `smtp_config`'s own AAD -- none of that is
/// crypto or validation this command repeats or re-checks. This
/// transaction's only job is the part that must be atomic: re-read the
/// pending row, decide `Applied` vs `BindingFailed`, and -- on
/// `Applied` only -- delete the row and write `smtp_config`, together.
#[allow(clippy::too_many_arguments)]
pub async fn apply_pending_settings_change(
    db: &crate::Database,
    admin: UserId,
    id: PendingChangeId,
    session_id: sui_id_shared::ids::SessionId,
    csrf_token: String,
    now: DateTime<Utc>,
    new_smtp_config: crate::models::SmtpConfigRow,
) -> StoreResult<crate::registry::Audited<ApplyPendingSettingsChangeOutcome>> {
    let context = AuthorizedCommandContext::<S08>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, S08>| {
        let attempt = crate::repos::pending_settings_change::apply_within_tx(
            tx.tx(),
            id,
            session_id,
            admin,
            &csrf_token,
            now,
        )?;
        match attempt {
            crate::repos::pending_settings_change::ApplyAttempt::Applied { intent } => {
                crate::repos::smtp_config::upsert_within_tx(tx.tx(), &new_smtp_config)?;
                Ok((
                    ApplyPendingSettingsChangeOutcome::Applied,
                    S08Event::Applied { id, intent },
                ))
            }
            crate::repos::pending_settings_change::ApplyAttempt::BindingFailed { intent } => Ok((
                ApplyPendingSettingsChangeOutcome::BindingFailed,
                S08Event::BindingFailed { id, intent },
            )),
        }
    })
    .await
}

// ── S01 — change default language ────────────────────────────────────────
//
// RFC 094 M2b stage 3. The stage plan originally called this wave "sealing
// what is already atomic" -- wrong (dispatch §0): production calls the
// *bare* `update_default_lang` (no audit at all), not the already-atomic
// `change_default_lang` the plan traced. This command now serves both the
// admin settings page and the setup wizard, which turned out to need the
// same `ActorRequirement` -- the wizard's `CurrentAdmin` extractor is the
// same one the settings page uses, so by the time either calls in, a real
// admin session already exists. The old bare/atomic pair in
// `server_settings.rs` is deleted; this is the one path now.

static S01_CHANGED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SettingsDefaultLanguageChanged,
    name: "settings.default_language.changed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::None,
    attributes: &[
        AttributeSpec {
            name: "old",
            description: "the previous BCP-47 language tag",
            required: true,
        },
        AttributeSpec {
            name: "new",
            description: "the new BCP-47 language tag",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// S01 — change the server default UI language. Administrator
    /// action through `&AdminActor`, from either the settings page or
    /// the setup wizard.
    command S01 = "S01" {
        system_principal: forbidden;
        enum S01Event {
            Changed { old: String, new: String } => &S01_CHANGED,
        }
    }
}

impl SealedCommandEvent<S01> for S01Event {
    fn target(&self) -> Option<AuditTarget> {
        None
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Changed { old, new } = self;
        AuditAttributes::builder()
            .attribute("old", old.clone())
            .attribute("new", new.clone())
            .build()
    }
}

/// Run S01 (change default language) through the Class-A runner.
pub async fn change_default_lang(
    db: &crate::Database,
    admin: UserId,
    lang: String,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<(String, String)>> {
    let context = AuthorizedCommandContext::<S01>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, S01>| {
        let old =
            crate::repos::server_settings::update_default_lang_within_tx(tx.tx(), &lang, now)?;
        Ok((
            (old.clone(), lang.clone()),
            S01Event::Changed { old, new: lang },
        ))
    })
    .await
}

// ── S02 — change HIBP mode ───────────────────────────────────────────────
//
// RFC 094 M2b stage 3. **No defect closed**: the setup wizard's
// `change_hibp_mode` was already atomic, and the admin settings page has
// no HIBP control at all -- `update_hibp_mode`, the row's named
// mutation surface, had zero production callers (measured). Sealed
// anyway because the inventory lists it and M2b requires every Class-A
// row converted (dispatch §1); the old bare/atomic pair is deleted and
// the setup wizard now calls this command, for the same reason S01's
// does.

static S02_CHANGED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SettingsHibpModeChanged,
    name: "settings.hibp_mode.changed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::None,
    attributes: &[
        AttributeSpec {
            name: "old",
            description: "the previous HIBP mode",
            required: true,
        },
        AttributeSpec {
            name: "new",
            description: "the new HIBP mode",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// S02 — change the server-wide HIBP check mode. Only the setup
    /// wizard calls this today; the admin settings page has no control
    /// for it.
    command S02 = "S02" {
        system_principal: forbidden;
        enum S02Event {
            Changed {
                old: crate::models::HibpMode,
                new: crate::models::HibpMode,
            } => &S02_CHANGED,
        }
    }
}

impl SealedCommandEvent<S02> for S02Event {
    fn target(&self) -> Option<AuditTarget> {
        None
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Changed { old, new } = self;
        AuditAttributes::builder()
            .attribute("old", old.as_str())
            .attribute("new", new.as_str())
            .build()
    }
}

/// Run S02 (change HIBP mode) through the Class-A runner.
pub async fn change_hibp_mode(
    db: &crate::Database,
    admin: UserId,
    mode: crate::models::HibpMode,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<(crate::models::HibpMode, crate::models::HibpMode)>> {
    let context = AuthorizedCommandContext::<S02>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, S02>| {
        let old = crate::repos::server_settings::update_hibp_mode_within_tx(tx.tx(), mode, now)?;
        Ok(((old, mode), S02Event::Changed { old, new: mode }))
    })
    .await
}

// ── S03 — change idle session timeout ────────────────────────────────────
//
// RFC 094 M2b stage 3. Named in the inventory since M2a; measured, never
// emitted anywhere before this stage -- the admin settings page's
// `update_idle_session_timeout` wrote the value with no audit append at
// all (dispatch §0).

static S03_CHANGED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SettingsIdleTimeoutChanged,
    name: "settings.idle_timeout.changed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::None,
    attributes: &[
        AttributeSpec {
            name: "old",
            description: "the previous idle-session-timeout value, in seconds",
            required: true,
        },
        AttributeSpec {
            name: "new",
            description: "the new idle-session-timeout value, in seconds",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// S03 — change the idle-session-timeout value. Administrator
    /// action through `&AdminActor`.
    command S03 = "S03" {
        system_principal: forbidden;
        enum S03Event {
            Changed { old: i64, new: i64 } => &S03_CHANGED,
        }
    }
}

impl SealedCommandEvent<S03> for S03Event {
    fn target(&self) -> Option<AuditTarget> {
        None
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Changed { old, new } = self;
        AuditAttributes::builder()
            .attribute("old", old.to_string())
            .attribute("new", new.to_string())
            .build()
    }
}

/// Run S03 (change idle session timeout) through the Class-A runner.
pub async fn change_idle_session_timeout(
    db: &crate::Database,
    admin: UserId,
    secs: i64,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<(i64, i64)>> {
    let context = AuthorizedCommandContext::<S03>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, S03>| {
        let old = crate::repos::server_settings::update_idle_session_timeout_within_tx(
            tx.tx(),
            secs,
            now,
        )?;
        Ok(((old, secs), S03Event::Changed { old, new: secs }))
    })
    .await
}

// ── S04 — change max concurrent sessions ─────────────────────────────────
//
// RFC 094 M2b stage 3. Same "named but never emitted" shape as S03
// (dispatch §0).

static S04_CHANGED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SettingsMaxSessionsChanged,
    name: "settings.max_sessions.changed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::None,
    attributes: &[
        AttributeSpec {
            name: "old",
            description: "the previous concurrent-session cap",
            required: true,
        },
        AttributeSpec {
            name: "new",
            description: "the new concurrent-session cap",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// S04 — change the concurrent-session cap. Administrator action
    /// through `&AdminActor`.
    command S04 = "S04" {
        system_principal: forbidden;
        enum S04Event {
            Changed { old: i64, new: i64 } => &S04_CHANGED,
        }
    }
}

impl SealedCommandEvent<S04> for S04Event {
    fn target(&self) -> Option<AuditTarget> {
        None
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Changed { old, new } = self;
        AuditAttributes::builder()
            .attribute("old", old.to_string())
            .attribute("new", new.to_string())
            .build()
    }
}

/// Run S04 (change max concurrent sessions) through the Class-A runner.
pub async fn change_max_concurrent_sessions(
    db: &crate::Database,
    admin: UserId,
    cap: i64,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<(i64, i64)>> {
    let context = AuthorizedCommandContext::<S04>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, S04>| {
        let old = crate::repos::server_settings::update_max_concurrent_sessions_within_tx(
            tx.tx(),
            cap,
            now,
        )?;
        Ok(((old, cap), S04Event::Changed { old, new: cap }))
    })
    .await
}

// ── S05 — rotate metrics token ───────────────────────────────────────────
//
// RFC 094 M2b stage 3. Same "named but never emitted" shape as S03/S04,
// plus a different actor shape: the only caller is `sui-id admin
// rotate-metrics-token`, which opens the database directly with the
// master key and has no authenticated `UserId` at all -- the same
// non-human-caller shape U08 uses (`for_system_actor`,
// `ActorRequirement::None`, `system_principal: permitted`). **No
// attributes at all**: the dispatch is explicit that neither the new nor
// the old hash may become an audit attribute, and there is nothing else
// non-secret left to record about a token rotation.

static S05_ROTATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SettingsMetricsTokenRotated,
    name: "settings.metrics_token.rotated",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::None,
    attributes: &[],
};

crate::declare_write_command! {
    /// S05 — rotate the `/metrics` endpoint's bearer token. CLI-only; no
    /// authenticated actor exists for this call.
    command S05 = "S05" {
        system_principal: permitted;
        enum S05Event {
            Rotated => &S05_ROTATED,
        }
    }
}

impl SealedCommandEvent<S05> for S05Event {
    fn target(&self) -> Option<AuditTarget> {
        None
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run S05 (rotate metrics token) through the Class-A runner. `hash` is
/// `None` to clear the token (disables bearer-token auth on `/metrics`).
pub async fn rotate_metrics_token(
    db: &crate::Database,
    hash: Option<String>,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<S05>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, S05>| {
        crate::repos::server_settings::update_metrics_token_hash_within_tx(
            tx.tx(),
            hash.as_deref(),
            now,
        )?;
        Ok(((), S05Event::Rotated))
    })
    .await
}

// ── S06 — change SMTP configuration (direct save) ────────────────────────
//
// RFC 094 M2b stage 3. The direct (no-pending-change) settings-save path
// -- the password-change path through a pending change is S08's,
// converted in stage 2, and untouched here. Before this stage,
// `smtp_config::upsert` committed and a separate best-effort
// `audit::append` followed; this reuses `upsert_within_tx`, which stage
// 2 already added for S08's own use. Attributes mirror the pre-stage
// best-effort note's fields exactly (`enabled`, `host`, `port`,
// `tls_mode`) -- never the password, which this path's caller never
// reads back out of the row it just built, let alone hands to this
// command.

static S06_CHANGED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AuthSmtpConfigChanged,
    name: "auth.smtp_config.changed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::None,
    attributes: &[
        AttributeSpec {
            name: "enabled",
            description: "whether the SMTP relay is enabled",
            required: true,
        },
        AttributeSpec {
            name: "host",
            description: "the SMTP relay host",
            required: true,
        },
        AttributeSpec {
            name: "port",
            description: "the SMTP relay port",
            required: true,
        },
        AttributeSpec {
            name: "tls_mode",
            description: "the SMTP relay's TLS mode",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// S06 — change the SMTP configuration via the direct (no secret
    /// change) save path. Administrator action through `&AdminActor`.
    command S06 = "S06" {
        system_principal: forbidden;
        enum S06Event {
            Changed {
                enabled: bool,
                host: String,
                port: u16,
                tls_mode: crate::models::SmtpTlsMode,
            } => &S06_CHANGED,
        }
    }
}

impl SealedCommandEvent<S06> for S06Event {
    fn target(&self) -> Option<AuditTarget> {
        None
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Changed {
            enabled,
            host,
            port,
            tls_mode,
        } = self;
        AuditAttributes::builder()
            .attribute("enabled", enabled.to_string())
            .attribute("host", host.clone())
            .attribute("port", port.to_string())
            .attribute("tls_mode", tls_mode.as_str())
            .build()
    }
}

/// Run S06 (change SMTP configuration, direct save) through the Class-A
/// runner. `new_smtp_config` is built by the caller before this is
/// called, same discipline as S08's own `new_smtp_config` parameter.
pub async fn change_smtp_config(
    db: &crate::Database,
    admin: UserId,
    new_smtp_config: crate::models::SmtpConfigRow,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<S06>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, S06>| {
        crate::repos::smtp_config::upsert_within_tx(tx.tx(), &new_smtp_config)?;
        Ok((
            (),
            S06Event::Changed {
                enabled: new_smtp_config.enabled,
                host: new_smtp_config.host.clone(),
                port: new_smtp_config.port,
                tls_mode: new_smtp_config.tls_mode,
            },
        ))
    })
    .await
}

// ── C05 — disable client ─────────────────────────────────────────────────
//
// RFC 094 M2b wave 4 stage 1. Two writes, one transaction: the client's
// `is_disabled` flag and `refresh_tokens::revoke_all_for_client_within_tx`
// -- the count it returns becomes this event's `refresh_tokens_revoked`
// attribute, which an incident reconstruction wants and which is only
// knowable inside the transaction that did the revoking.

static C05_DISABLED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientDisable,
    name: "client.disable",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "reason",
            description: "operator-supplied reason for the disable, if given",
            required: false,
        },
        AttributeSpec {
            name: "refresh_tokens_revoked",
            description: "count of refresh tokens revoked as part of the disable",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// C05 — disable a client. Administrator action through
    /// `&AdminActor`.
    command C05 = "C05" {
        system_principal: forbidden;
        enum C05Event {
            Disabled {
                client_id: ClientId,
                reason: Option<String>,
                refresh_tokens_revoked: usize,
            } => &C05_DISABLED,
        }
    }
}

impl SealedCommandEvent<C05> for C05Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Disabled { client_id, .. } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Disabled {
            reason,
            refresh_tokens_revoked,
            ..
        } = self;
        let mut builder = AuditAttributes::builder();
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder
            .attribute("refresh_tokens_revoked", refresh_tokens_revoked.to_string())
            .build()
    }
}

/// Run C05 (disable a client) through the Class-A runner: the
/// `is_disabled` flag and the refresh-token revoke now commit together,
/// with the audit event.
pub async fn disable_client(
    db: &crate::Database,
    admin: UserId,
    target: ClientId,
    reason: Option<String>,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<C05>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C05>| {
        crate::repos::clients::set_disabled_within_tx(tx.tx(), target, true, now)?;
        let refresh_tokens_revoked =
            crate::repos::refresh_tokens::revoke_all_for_client_within_tx(tx.tx(), target, now)?;
        Ok((
            (),
            C05Event::Disabled {
                client_id: target,
                reason,
                refresh_tokens_revoked,
            },
        ))
    })
    .await
}

// ── C21 — enable client ──────────────────────────────────────────────────
//
// RFC 094 M2b wave 4 stage 1. One write only -- enabling revokes nothing,
// and must not start a revoke call the way disable and delete do.

static C21_ENABLED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientEnable,
    name: "client.enable",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// C21 — enable a client. Administrator action through
    /// `&AdminActor`.
    command C21 = "C21" {
        system_principal: forbidden;
        enum C21Event {
            Enabled { client_id: ClientId } => &C21_ENABLED,
        }
    }
}

impl SealedCommandEvent<C21> for C21Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Enabled { client_id } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run C21 (enable a client) through the Class-A runner.
pub async fn enable_client(
    db: &crate::Database,
    admin: UserId,
    target: ClientId,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<C21>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C21>| {
        crate::repos::clients::set_disabled_within_tx(tx.tx(), target, false, now)?;
        Ok(((), C21Event::Enabled { client_id: target }))
    })
    .await
}

// ── C06 — delete client (soft) ───────────────────────────────────────────
//
// RFC 094 M2b wave 4 stage 1. Same two-write shape as C05: the soft
// delete and the refresh-token revoke, one transaction, with the revoked
// count as this event's attribute.

static C06_DELETED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientDelete,
    name: "client.delete",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "reason",
            description: "operator-supplied reason for the delete, if given",
            required: false,
        },
        AttributeSpec {
            name: "refresh_tokens_revoked",
            description: "count of refresh tokens revoked as part of the delete",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// C06 — soft-delete a client. Administrator action through
    /// `&AdminActor`.
    command C06 = "C06" {
        system_principal: forbidden;
        enum C06Event {
            Deleted {
                client_id: ClientId,
                reason: Option<String>,
                refresh_tokens_revoked: usize,
            } => &C06_DELETED,
        }
    }
}

impl SealedCommandEvent<C06> for C06Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Deleted { client_id, .. } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Deleted {
            reason,
            refresh_tokens_revoked,
            ..
        } = self;
        let mut builder = AuditAttributes::builder();
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder
            .attribute("refresh_tokens_revoked", refresh_tokens_revoked.to_string())
            .build()
    }
}

/// Run C06 (soft-delete a client) through the Class-A runner: the
/// soft-delete flags and the refresh-token revoke now commit together,
/// with the audit event.
pub async fn delete_client(
    db: &crate::Database,
    admin: UserId,
    target: ClientId,
    reason: Option<String>,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<C06>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C06>| {
        crate::repos::clients::soft_delete_within_tx(tx.tx(), target, now)?;
        let refresh_tokens_revoked =
            crate::repos::refresh_tokens::revoke_all_for_client_within_tx(tx.tx(), target, now)?;
        Ok((
            (),
            C06Event::Deleted {
                client_id: target,
                reason,
                refresh_tokens_revoked,
            },
        ))
    })
    .await
}

// ── C03 — the client-update form (C03, C04, C09) ─────────────────────────
//
// RFC 094 M2b wave 4 stage 2. One admin form submits three fields --
// allowed scopes, post-logout redirect URIs, and consent policy -- that
// were three independent commits before this stage: a failure partway
// through left some fields changed and others not, with no single
// record of what the form actually did. `Database::class_a` is generic
// over exactly one `CommandSpec` and its closure returns exactly one
// `(T, C::Event)` -- confirmed by reading its body (one `append_within_tx`
// call, fed by one `C::descriptor(&event)`) -- so three separate sealed
// commands (three separate `class_a` calls) cannot share one transaction;
// each opens and commits its own. The three writes and one audit record
// below are therefore one sealed command, carrying the inventory's first
// id, C03. C04 and C09 are this event's sibling rows, not separate
// commands -- see `contracts/write-commands.toml`'s note on C03, C04 and
// C09.

static C03_FORM_UPDATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientUpdateForm,
    name: "client.update_form",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "allowed_scopes_count",
            description: "count of scope tokens in the new allowed_scopes value",
            required: true,
        },
        AttributeSpec {
            name: "post_logout_redirect_uris_count",
            description: "count of URIs in the new post_logout_redirect_uris value",
            required: true,
        },
        AttributeSpec {
            name: "consent_policy",
            description: "the new consent policy value",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// C03 — the client-update form's three coordinated fields.
    /// Administrator action through `&AdminActor`. C04 and C09 are this
    /// command's sibling inventory rows (see the module-level comment
    /// above this block), not separate `declare_write_command!`
    /// invocations.
    command C03 = "C03" {
        system_principal: forbidden;
        enum C03Event {
            FormUpdated {
                client_id: ClientId,
                allowed_scopes_count: usize,
                post_logout_redirect_uris_count: usize,
                consent_policy: crate::models::ConsentPolicy,
            } => &C03_FORM_UPDATED,
        }
    }
}

impl SealedCommandEvent<C03> for C03Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::FormUpdated { client_id, .. } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::FormUpdated {
            allowed_scopes_count,
            post_logout_redirect_uris_count,
            consent_policy,
            ..
        } = self;
        AuditAttributes::builder()
            .attribute("allowed_scopes_count", allowed_scopes_count.to_string())
            .attribute(
                "post_logout_redirect_uris_count",
                post_logout_redirect_uris_count.to_string(),
            )
            .attribute("consent_policy", consent_policy.as_str())
            .build()
    }
}

/// Run C03 (the client-update form) through the Class-A runner: the
/// three field writes and the audit event now commit together.
/// `allowed_scopes` and `post_logout_redirect_uris` are validated by the
/// caller (`sui-id-core`'s `update_client_form`) before this runs --
/// this function trusts its input, the same division stage 2's S08
/// established for a sealed command's own validation boundary.
pub async fn update_client_form(
    db: &crate::Database,
    admin: UserId,
    target: ClientId,
    allowed_scopes: String,
    post_logout_redirect_uris: Vec<String>,
    consent_policy: crate::models::ConsentPolicy,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let allowed_scopes_count = allowed_scopes.split_whitespace().count();
    let post_logout_redirect_uris_count = post_logout_redirect_uris.len();
    let context = AuthorizedCommandContext::<C03>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C03>| {
        crate::repos::clients::set_allowed_scopes_within_tx(tx.tx(), target, &allowed_scopes, now)?;
        crate::repos::clients::set_post_logout_redirect_uris_within_tx(
            tx.tx(),
            target,
            &post_logout_redirect_uris,
            now,
        )?;
        crate::repos::clients::update_consent_policy_within_tx(
            tx.tx(),
            target,
            consent_policy.clone(),
            now,
        )?;
        Ok((
            (),
            C03Event::FormUpdated {
                client_id: target,
                allowed_scopes_count,
                post_logout_redirect_uris_count,
                consent_policy,
            },
        ))
    })
    .await
}

// ── C10 — client application-identity ────────────────────────────────────
//
// RFC 140 stage 1. Administrator action through `&AdminActor`. A wave-1
// shape before this round: the inventory declared `client.
// app_identity_changed` and nothing emitted it -- `update_app_identity`
// (the write this seals) had no caller anywhere in `crates/`. `logo_uri`
// is not a parameter here -- stage 2, RFC 140 §5 undecided -- and the
// existing stored value is read fresh inside this transaction and
// passed through unchanged.

static C10_APP_IDENTITY_CHANGED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientAppIdentityChanged,
    name: "client.app_identity_changed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "homepage_uri_changed",
            description: "whether homepage_uri differs from its prior stored value",
            required: true,
        },
        AttributeSpec {
            name: "privacy_policy_uri_changed",
            description: "whether privacy_policy_uri differs from its prior stored value",
            required: true,
        },
        AttributeSpec {
            name: "tos_uri_changed",
            description: "whether tos_uri differs from its prior stored value",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// C10 — update a client's application-identity links. Administrator
    /// action through `&AdminActor`.
    command C10 = "C10" {
        system_principal: forbidden;
        enum C10Event {
            Changed {
                client_id: ClientId,
                homepage_uri_changed: bool,
                privacy_policy_uri_changed: bool,
                tos_uri_changed: bool,
            } => &C10_APP_IDENTITY_CHANGED,
        }
    }
}

impl SealedCommandEvent<C10> for C10Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Changed { client_id, .. } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Changed {
            homepage_uri_changed,
            privacy_policy_uri_changed,
            tos_uri_changed,
            ..
        } = self;
        AuditAttributes::builder()
            .attribute("homepage_uri_changed", homepage_uri_changed.to_string())
            .attribute(
                "privacy_policy_uri_changed",
                privacy_policy_uri_changed.to_string(),
            )
            .attribute("tos_uri_changed", tos_uri_changed.to_string())
            .build()
    }
}

/// Run C10 (update a client's application-identity links) through the
/// Class-A runner. Reads the client's current `logo_uri` and the three
/// managed fields inside this same transaction — never from a value the
/// caller fetched earlier — both to preserve `logo_uri` unchanged (stage
/// 1 does not manage it) and to compute which of the three managed
/// fields actually changed, the same freshness discipline RFC 140
/// stage 1 §1 just applied to wave 3's `origins_before`.
pub async fn update_client_app_identity(
    db: &crate::Database,
    admin: UserId,
    target: ClientId,
    homepage_uri: Option<String>,
    privacy_policy_uri: Option<String>,
    tos_uri: Option<String>,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<C10>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C10>| {
        let existing = crate::repos::clients::get_app_identity_within_tx(tx.tx(), target)?;
        let homepage_uri_changed = homepage_uri != existing.homepage_uri;
        let privacy_policy_uri_changed = privacy_policy_uri != existing.privacy_policy_uri;
        let tos_uri_changed = tos_uri != existing.tos_uri;
        crate::repos::clients::update_app_identity_within_tx(
            tx.tx(),
            target,
            existing.logo_uri,
            homepage_uri,
            privacy_policy_uri,
            tos_uri,
            now,
        )?;
        Ok((
            (),
            C10Event::Changed {
                client_id: target,
                homepage_uri_changed,
                privacy_policy_uri_changed,
                tos_uri_changed,
            },
        ))
    })
    .await
}

// ── C01 — create client ──────────────────────────────────────────────────
//
// RFC 094 M2b wave 4 stage 3. One write, target is the new client's id.

static C01_CREATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientCreate,
    name: "client.create",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// C01 — create a client. Administrator action through
    /// `&AdminActor`.
    command C01 = "C01" {
        system_principal: forbidden;
        enum C01Event {
            Created { client_id: ClientId } => &C01_CREATED,
        }
    }
}

impl SealedCommandEvent<C01> for C01Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Created { client_id } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run C01 (create a client) through the Class-A runner. `row` is
/// already fully built by the caller (`sui-id-core`'s `create_client`,
/// which also generates and hashes the secret before this runs — the
/// same validate/hash-outside-the-transaction boundary stage 2's S08
/// established).
pub async fn create_client(
    db: &crate::Database,
    admin: UserId,
    row: crate::models::ClientRow,
) -> StoreResult<crate::registry::Audited<()>> {
    let client_id = row.id;
    let context = AuthorizedCommandContext::<C01>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C01>| {
        crate::repos::clients::create_within_tx(tx.tx(), &row)?;
        Ok(((), C01Event::Created { client_id }))
    })
    .await
}

// ── C02 — update client basic info ───────────────────────────────────────
//
// RFC 094 M2b wave 4 stage 3. Name and redirect URIs only; the
// client-update form's other three fields are C03 (stage 2).

static C02_UPDATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientUpdateBasic,
    name: "client.update",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// C02 — update a client's name and redirect URIs. Administrator
    /// action through `&AdminActor`.
    command C02 = "C02" {
        system_principal: forbidden;
        enum C02Event {
            Updated { client_id: ClientId } => &C02_UPDATED,
        }
    }
}

impl SealedCommandEvent<C02> for C02Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Updated { client_id } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run C02 (update a client's basic info) through the Class-A runner.
pub async fn update_client_basic(
    db: &crate::Database,
    admin: UserId,
    target: ClientId,
    name: String,
    redirect_uris: Vec<String>,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<C02>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C02>| {
        crate::repos::clients::update_basic_within_tx(
            tx.tx(),
            target,
            Some(&name),
            Some(&redirect_uris),
            now,
        )?;
        Ok(((), C02Event::Updated { client_id: target }))
    })
    .await
}

// ── C07 — rotate client secret ───────────────────────────────────────────
//
// RFC 094 M2b wave 4 stage 3. Never carries the new secret or its hash.

static C07_ROTATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientRotateSecret,
    name: "client.rotate_secret",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "reason",
        description: "operator-supplied reason for the rotation, if given",
        required: false,
    }],
};

crate::declare_write_command! {
    /// C07 — rotate a confidential client's secret. Administrator
    /// action through `&AdminActor`.
    command C07 = "C07" {
        system_principal: forbidden;
        enum C07Event {
            Rotated {
                client_id: ClientId,
                reason: Option<String>,
            } => &C07_ROTATED,
        }
    }
}

impl SealedCommandEvent<C07> for C07Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Rotated { client_id, .. } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Rotated { reason, .. } = self;
        let mut builder = AuditAttributes::builder();
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder.build()
    }
}

/// Run C07 (rotate a client secret) through the Class-A runner.
/// `new_hash` is already computed by the caller (`sui-id-core`'s
/// `rotate_client_secret`, which generates and hashes the new secret
/// before this runs) — never the plaintext secret, and never recorded
/// as an attribute.
pub async fn rotate_client_secret(
    db: &crate::Database,
    admin: UserId,
    target: ClientId,
    new_hash: String,
    reason: Option<String>,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<C07>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C07>| {
        crate::repos::clients::set_secret_hash_within_tx(tx.tx(), target, Some(&new_hash), now)?;
        Ok((
            (),
            C07Event::Rotated {
                client_id: target,
                reason,
            },
        ))
    })
    .await
}

// ── K03 — delete signing key ─────────────────────────────────────────────
//
// RFC 094 M2b wave 4 stage 3. `Conflict` (active key) and `NotFound`
// (missing key) both refuse before any write; neither reaches
// `class_a`, so this event only ever records a successful delete.

static K03_DELETED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SigningKeyDelete,
    name: "signing_key.delete",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "reason",
        description: "operator-supplied reason for the deletion, if given",
        required: false,
    }],
};

crate::declare_write_command! {
    /// K03 — hard-delete a retired signing key. Administrator action
    /// through `&AdminActor`.
    command K03 = "K03" {
        system_principal: forbidden;
        enum K03Event {
            Deleted {
                key_id: SigningKeyId,
                reason: Option<String>,
            } => &K03_DELETED,
        }
    }
}

impl SealedCommandEvent<K03> for K03Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Deleted { key_id, .. } = self;
        Some(AuditTarget(key_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Deleted { reason, .. } = self;
        let mut builder = AuditAttributes::builder();
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder.build()
    }
}

/// Run K03 (hard-delete a retired signing key) through the Class-A
/// runner. RFC 094 M2b wave 4 stage 4: stage 3's `now: DateTime<Utc>`
/// parameter (accepted only for signature uniformity with that stage's
/// other four commands, and otherwise unused — the delete touches no
/// timestamp column, and the audit row's own `at` is never
/// caller-controlled; `build_audit_row` calls `chrono::Utc::now()`
/// directly) is removed. A parameter that provably does nothing makes
/// every future caller work out whether it matters.
pub async fn delete_signing_key(
    db: &crate::Database,
    admin: UserId,
    target: SigningKeyId,
    reason: Option<String>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<K03>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, K03>| {
        crate::repos::signing_keys::delete_within_tx(tx.tx(), target)?;
        Ok((
            (),
            K03Event::Deleted {
                key_id: target,
                reason,
            },
        ))
    })
    .await
}

// ── C08 — set a dev-mode client's secret hash ────────────────────────────
//
// RFC 094 M2b wave 4 stage 3. A wave-1 shape, not a wave-4 one: the
// bare write emitted no event at all before this stage, and its only
// caller is dev-mode seeding, not an administrator action. A distinct
// event name from C07's `client.rotate_secret` — see `registry.rs`'s
// `ClientDevSecretSet` doc comment for why two sealed commands may not
// share one name, measured against `scripts/rust_registry.py`.

static C08_DEV_SECRET_SET: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientDevSecretSet,
    name: "client.dev_secret_set",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// C08 — patch a dev-mode-seeded client's secret hash to an
    /// operator-supplied, predictable value. Runs under the synthetic
    /// admin actor dev-mode seeding already constructs for C01.
    command C08 = "C08" {
        system_principal: forbidden;
        enum C08Event {
            Set { client_id: ClientId } => &C08_DEV_SECRET_SET,
        }
    }
}

impl SealedCommandEvent<C08> for C08Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Set { client_id } = self;
        Some(AuditTarget(client_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run C08 (patch a dev-mode client's secret hash) through the Class-A
/// runner. `new_hash` is already computed by the caller (dev-mode
/// seeding hashes the operator-supplied secret before this runs) —
/// never the plaintext secret, and never recorded as an attribute.
pub async fn set_dev_client_secret(
    db: &crate::Database,
    admin: UserId,
    target: ClientId,
    new_hash: String,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<C08>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C08>| {
        crate::repos::clients::set_dev_secret_hash_within_tx(tx.tx(), target, Some(&new_hash))?;
        Ok(((), C08Event::Set { client_id: target }))
    })
    .await
}

// ── C22 — revoke a client-registration token ─────────────────────────────
//
// RFC 094 M2b wave 4 stage 4. CLI-only, no authenticated actor -- the
// operator's authority is possession of the master key and filesystem
// access, the same shape U08 and K01 use.

static C22_REVOKED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::ClientRegistrationTokenRevoked,
    name: "client.registration_token.revoked",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// C22 — CLI operator revokes a client-registration token.
    command C22 = "C22" {
        system_principal: permitted;
        enum C22Event {
            Revoked { token_id: RegistrationTokenId } => &C22_REVOKED,
        }
    }
}

impl SealedCommandEvent<C22> for C22Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Revoked { token_id } = self;
        Some(AuditTarget(token_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run C22 (revoke a client-registration token) through the Class-A
/// runner: the `revoked_at` write and the `client.registration_token.revoked`
/// audit append now commit in one transaction, replacing a bare
/// `client_registration_token::revoke` that had no caller and no event
/// at all.
pub async fn revoke_registration_token(
    db: &crate::Database,
    target: RegistrationTokenId,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<C22>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C22>| {
        crate::repos::client_registration_token::revoke_within_tx(tx.tx(), target, now)?;
        Ok(((), C22Event::Revoked { token_id: target }))
    })
    .await
}

// ── C16 — federation provider create ─────────────────────────────────────
//
// RFC 094 M2b wave 3. Startup provisioning (`runtime/startup.rs`), not an
// administrator action: a `[[federation_providers]]` config block absent
// from the database is created. No session -- `system_principal:
// permitted`, the same shape C22 uses, since the operator's authority is
// the config file and the master key, not an authenticated actor. Ruled
// 2026-10-25: RFC 096-B2's `activation_generation`/review-required
// semantics are not implemented by this seal -- it seals `create` as it
// stands today, and 096-B2 extends this sealed command rather than
// converting a bare one.

static C16_CREATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::FederationProviderCreate,
    name: "federation.provider.created",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "slug",
            description: "the provider's configured slug",
            required: true,
        },
        AttributeSpec {
            name: "issuer",
            description: "the provider's configured issuer URL",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// C16 — startup provisioning creates a federation provider absent
    /// from the database.
    command C16 = "C16" {
        system_principal: permitted;
        enum C16Event {
            Created {
                provider_id: FederationProviderId,
                slug: String,
                issuer: String,
            } => &C16_CREATED,
        }
    }
}

impl SealedCommandEvent<C16> for C16Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Created { provider_id, .. } = self;
        Some(AuditTarget(provider_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Created { slug, issuer, .. } = self;
        AuditAttributes::builder()
            .attribute("slug", slug.clone())
            .attribute("issuer", issuer.clone())
            .build()
    }
}

/// Run C16 (create a federation provider) through the Class-A runner.
/// `client_secret_plain` is sealed here, inside the command, before the
/// row reaches [`crate::repos::federation_provider::create_within_tx`] —
/// never passed to that function in plaintext, and never recorded as an
/// attribute (only `slug` and `issuer` are). Callers must leave
/// `row.client_secret_enc` as `None`; this command fills it.
pub async fn create_federation_provider(
    db: &crate::Database,
    mut row: crate::models::FederationProviderRow,
    client_secret_plain: Option<String>,
) -> StoreResult<crate::registry::Audited<()>> {
    let db_for_tx = db.clone();
    let context = AuthorizedCommandContext::<C16>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C16>| {
        row.client_secret_enc = match &client_secret_plain {
            Some(s) => Some(crate::crypto::seal(
                db_for_tx.key(),
                s.as_bytes(),
                crate::repos::federation_provider::AAD,
            )?),
            None => None,
        };
        crate::repos::federation_provider::create_within_tx(tx.tx(), &row)?;
        Ok((
            (),
            C16Event::Created {
                provider_id: row.id,
                slug: row.slug.clone(),
                issuer: row.issuer.clone(),
            },
        ))
    })
    .await
}

// ── C24 — federation provider allowed-origins change ─────────────────────
//
// RFC 094 M2b wave 3. Not in the inventory before this round -- a live
// Class-A-shaped write (RFC 134 step 7a) with no id at all. Startup
// provisioning reconciles a configured `allowed_origins` list against the
// stored one and writes over it when they differ; same no-session shape
// as C16.

static C24_ORIGINS_CHANGED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::FederationProviderOriginsChanged,
    name: "federation.provider.allowed_origins_changed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "origins_before",
            description: "count of allowed origins before the change",
            required: true,
        },
        AttributeSpec {
            name: "origins_after",
            description: "count of allowed origins after the change",
            required: true,
        },
    ],
};

crate::declare_write_command! {
    /// C24 — startup provisioning reconciles a federation provider's
    /// configured `allowed_origins` against the stored value.
    command C24 = "C24" {
        system_principal: permitted;
        enum C24Event {
            Changed {
                provider_id: FederationProviderId,
                origins_before: usize,
                origins_after: usize,
            } => &C24_ORIGINS_CHANGED,
        }
    }
}

impl SealedCommandEvent<C24> for C24Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Changed { provider_id, .. } = self;
        Some(AuditTarget(provider_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Changed {
            origins_before,
            origins_after,
            ..
        } = self;
        AuditAttributes::builder()
            .attribute("origins_before", origins_before.to_string())
            .attribute("origins_after", origins_after.to_string())
            .build()
    }
}

/// Run C24 (reconcile a provider's `allowed_origins`) through the
/// Class-A runner. RFC 140 stage 1 §1: `origins_before` is read inside
/// this same transaction, by `update_allowed_origins_within_tx` itself,
/// immediately before its `UPDATE` — not supplied by the caller. The
/// wave 3 round took it as a parameter instead, which let a Class-A
/// record carry an attribute the transaction never verified; fixed here
/// before the signature was published.
pub async fn update_federation_provider_allowed_origins(
    db: &crate::Database,
    target: FederationProviderId,
    allowed_origins: String,
    now: DateTime<Utc>,
) -> StoreResult<crate::registry::Audited<()>> {
    let origins_after = allowed_origins.split_whitespace().count();
    let context = AuthorizedCommandContext::<C24>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, C24>| {
        let origins_before = crate::repos::federation_provider::update_allowed_origins_within_tx(
            tx.tx(),
            target,
            &allowed_origins,
            now,
        )?;
        Ok((
            (),
            C24Event::Changed {
                provider_id: target,
                origins_before,
                origins_after,
            },
        ))
    })
    .await
}

#[cfg(test)]
mod tests;
