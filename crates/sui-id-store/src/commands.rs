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
//! | [`U01`] (`users::create_within_tx` + `credentials::upsert_within_tx`) | Class-A with a closed result branch on *input*, not observed state: `user.create` / `user.create_warned_hibp` |
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
use crate::registry::{
    ActorRequirement, AttributeSpec, AuditAttributes, AuditBuildError, AuditClass, AuditEventKind,
    AuditResult, AuditTarget, AuthorizedCommandContext, ClassATx, EventDescriptor,
    SealedCommandEvent, TargetRequirement,
};
use chrono::{DateTime, Utc};
use sui_id_shared::{
    FamilyId, RefreshTokenHash,
    ids::{ClientId, SigningKeyId, UserId},
};

// ── K01 — signing-key rotation ──────────────────────────────────────────

static K01_ROTATED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::SigningKeyRotate,
    name: "signing_key.rotate",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "algorithm",
        description: "the new key's signing algorithm",
    }],
};

crate::declare_write_command! {
    /// K01 — signing-key rotation.
    command K01 = "K01" {
        // Key rotation is an ops/CLI/scheduled trigger, not an action a
        // logged-in user takes on their own session — no human actor is
        // ever the authority for it (K01_ROTATED's `actor: None` already
        // says the same thing about the event payload).
        system_principal: permitted;
        enum K01Event {
            Rotated { new_key: SigningKeyId, algorithm: String } => &K01_ROTATED,
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
        let Self::Rotated { algorithm, .. } = self;
        AuditAttributes::builder()
            .attribute("algorithm", algorithm.clone())
            .build()
    }
}

/// Run K01 (signing-key rotation) through the Class-A runner.
///
/// `private_key_plain` is sealed by the caller *before* this is called
/// (RFC 094: crypto work stays outside the transaction) — same contract as
/// [`crate::repos::signing_keys::rotate_atomic`].
pub async fn rotate_signing_key(
    db: &crate::Database,
    new_id: SigningKeyId,
    algorithm: String,
    private_key_sealed: Vec<u8>,
    public_key: Vec<u8>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<K01>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, K01>| {
        crate::repos::signing_keys::rotate_atomic_within_tx(
            tx.tx(),
            new_id,
            &algorithm,
            &private_key_sealed,
            &public_key,
        )?;
        Ok((
            (),
            K01Event::Rotated {
                new_key: new_id,
                algorithm,
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
        },
        AttributeSpec {
            name: "locked_for_secs",
            description: "the lock window's length in seconds, as computed by the caller's \
                backoff policy",
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

// ── U01 — create user (closed branch on input) ──────────────────────────

static U01_CREATE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserCreate,
    name: "user.create",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

static U01_CREATE_WARNED_HIBP: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserCreateWarnedHibp,
    name: "user.create_warned_hibp",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// U01 — admin create user, branching on the HIBP policy outcome
    /// decided by the *caller* before this runs (not observed inside the
    /// transaction) — the branch is closed over input, which is why this
    /// slice member is distinct from U22's closed branch over observed
    /// state.
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
            CreatedWarnedHibp { user_id: UserId } => &U01_CREATE_WARNED_HIBP,
        }
    }
}

impl SealedCommandEvent<U01> for U01Event {
    fn target(&self) -> Option<AuditTarget> {
        match self {
            Self::Created { user_id } | Self::CreatedWarnedHibp { user_id } => {
                Some(AuditTarget(user_id.to_string()))
            }
        }
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run U01 (admin create user) through the Class-A runner. `hibp_warned`
/// is the caller's already-decided branch (RFC 094: the branch is closed
/// over input, not re-derived here).
/// `admin` is the authorizing actor's `UserId` — the caller is
/// responsible for it genuinely coming from a verified authorization
/// decision (RFC 094 §"Class-A transaction seam"; see
/// `AuthorizedCommandContext::for_authorized_actor`'s own doc comment
/// for the full caller-discipline contract).
pub async fn create_user(
    db: &crate::Database,
    admin: UserId,
    user: crate::models::UserRow,
    credential: Option<crate::models::CredentialRow>,
    hibp_warned: bool,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U01>::for_authorized_actor(admin, None);
    let user_id = user.id;
    db.class_a(context, move |tx: &mut ClassATx<'_, U01>| {
        crate::repos::users::create_within_tx(tx.tx(), &user)?;
        if let Some(cred) = &credential {
            crate::repos::credentials::upsert_within_tx(tx.tx(), cred)?;
        }
        let event = if hibp_warned {
            U01Event::CreatedWarnedHibp { user_id }
        } else {
            U01Event::Created { user_id }
        };
        Ok(((), event))
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
    attributes: &[AttributeSpec {
        name: "reason",
        description: "operator-supplied reason for the disable, if given",
    }],
};

crate::declare_write_command! {
    /// U02 — admin disable user. Same `forbidden` reasoning as U01: the
    /// coverage matrix requires `admin user id` as the actor.
    command U02 = "U02" {
        system_principal: forbidden;
        enum U02Event {
            Disabled { user_id: UserId, reason: Option<String> } => &U02_DISABLE,
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
        let Self::Disabled { reason, .. } = self;
        let mut builder = AuditAttributes::builder();
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder.build()
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
    target: UserId,
    reason: Option<String>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U02>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U02>| {
        crate::repos::users::set_disabled_within_tx(tx.tx(), target, true)?;
        crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), target, chrono::Utc::now())?;
        crate::repos::refresh_tokens::revoke_all_for_user_within_tx(
            tx.tx(),
            target,
            chrono::Utc::now(),
        )?;
        crate::repos::auth_codes::invalidate_all_for_user_within_tx(tx.tx(), target)?;
        Ok((
            (),
            U02Event::Disabled {
                user_id: target,
                reason,
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
    attributes: &[],
};

crate::declare_write_command! {
    /// U03 — admin re-enable user.
    command U03 = "U03" {
        system_principal: forbidden;
        enum U03Event {
            Enabled { user_id: UserId } => &U03_ENABLE,
        }
    }
}

impl SealedCommandEvent<U03> for U03Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Enabled { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run U03 (admin re-enable user) through the Class-A runner. Unlike
/// U02, re-enabling does not revoke anything — the coverage matrix
/// carries no note field for this row, matching the pre-conversion
/// behavior of `set_user_disabled(disabled: false)`.
pub async fn enable_user(
    db: &crate::Database,
    admin: UserId,
    target: UserId,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U03>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U03>| {
        crate::repos::users::set_disabled_within_tx(tx.tx(), target, false)?;
        Ok(((), U03Event::Enabled { user_id: target }))
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
    attributes: &[AttributeSpec {
        name: "reason",
        description: "operator-supplied reason for the deletion, if given",
    }],
};

crate::declare_write_command! {
    /// U04 — admin soft-delete user.
    command U04 = "U04" {
        system_principal: forbidden;
        enum U04Event {
            Deleted { user_id: UserId, reason: Option<String> } => &U04_DELETE,
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
        let Self::Deleted { reason, .. } = self;
        let mut builder = AuditAttributes::builder();
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder.build()
    }
}

/// Run U04 (admin soft-delete user) through the Class-A runner. Same
/// revocation-bundling reasoning as U02.
pub async fn delete_user(
    db: &crate::Database,
    admin: UserId,
    target: UserId,
    reason: Option<String>,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U04>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U04>| {
        crate::repos::users::soft_delete_within_tx(tx.tx(), target)?;
        crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), target, chrono::Utc::now())?;
        crate::repos::refresh_tokens::revoke_all_for_user_within_tx(
            tx.tx(),
            target,
            chrono::Utc::now(),
        )?;
        crate::repos::auth_codes::invalidate_all_for_user_within_tx(tx.tx(), target)?;
        Ok((
            (),
            U04Event::Deleted {
                user_id: target,
                reason,
            },
        ))
    })
    .await
}

// ── U05 — admin role change ──────────────────────────────────────────
//
// **Schema not previously specified — flagged for review.** Unlike
// U01-U04, `docs/src/reference/audit-coverage-matrix.md` carries no row
// for `user.role_change`; it says only "will be added when the
// role-change handler is converted to Class A atomicity" (RFC 085). No
// production code emits this event today either — `users_set_role`
// (`sui-id/src/http/handlers/admin/users.rs`) calls
// `sui_id_store::repos::users::set_role` directly, with no audit call at
// all. The event name (`user.role_change`) and actor/target shape (admin
// user id / target user id) come from `command-inventory.md:64`; the
// `old_role`/`new_role` attribute pair is this implementation's own
// proposal, chosen because a role-change record with neither value is
// not reconstructable from the rest of the audit log. Treat this
// descriptor as provisional until confirmed.
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
        },
        AttributeSpec {
            name: "new_role",
            description: "the user's role after the change",
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

// ── U06 — admin password reset ───────────────────────────────────────

static U06_RESET_PASSWORD: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserResetPassword,
    name: "user.reset_password",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// U06 — admin password reset. Same `forbidden` reasoning as U01-U05:
    /// the coverage matrix requires `admin user id` as the actor.
    command U06 = "U06" {
        system_principal: forbidden;
        enum U06Event {
            Reset { user_id: UserId } => &U06_RESET_PASSWORD,
        }
    }
}

impl SealedCommandEvent<U06> for U06Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Reset { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run U06 (admin password reset) through the Class-A runner. `credential`
/// is the already-hashed replacement row — password hashing and the HIBP
/// breach check are network/CPU-bound work that stays outside the
/// transaction, same contract as U01's HIBP branch and K01's sealed key
/// material. Revokes the target's sessions, refresh tokens, and in-flight
/// auth codes in the same transaction as the credential swap and the
/// audit append, same reasoning as U02/U04: a password reset is exactly
/// the kind of operation where "credential changed but old sessions
/// survived the crash between the two calls" is the failure this RFC
/// exists to close.
pub async fn reset_user_password(
    db: &crate::Database,
    admin: UserId,
    target: UserId,
    credential: crate::models::CredentialRow,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U06>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U06>| {
        crate::repos::credentials::upsert_within_tx(tx.tx(), &credential)?;
        crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), target, chrono::Utc::now())?;
        crate::repos::refresh_tokens::revoke_all_for_user_within_tx(
            tx.tx(),
            target,
            chrono::Utc::now(),
        )?;
        crate::repos::auth_codes::invalidate_all_for_user_within_tx(tx.tx(), target)?;
        Ok(((), U06Event::Reset { user_id: target }))
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
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "totp",
            description: "whether a TOTP enrollment was removed (\"removed\" or \"absent\")",
        },
        AttributeSpec {
            name: "passkeys",
            description: "number of WebAuthn credentials removed",
        },
        AttributeSpec {
            name: "reason",
            description: "operator-supplied reason for the reset, if given",
        },
    ],
};

crate::declare_write_command! {
    /// U07 — admin MFA reset. Same `forbidden` reasoning as U01-U06: the
    /// coverage matrix requires `admin user id` as the actor.
    command U07 = "U07" {
        system_principal: forbidden;
        enum U07Event {
            Reset {
                user_id: UserId,
                totp_removed: bool,
                passkeys_removed: usize,
                reason: Option<String>,
            } => &U07_ADMIN_RESET,
        }
    }
}

impl SealedCommandEvent<U07> for U07Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Reset { user_id, .. } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        let Self::Reset {
            totp_removed,
            passkeys_removed,
            reason,
            ..
        } = self;
        let mut builder = AuditAttributes::builder()
            .attribute("totp", if *totp_removed { "removed" } else { "absent" })
            .attribute("passkeys", passkeys_removed.to_string());
        if let Some(r) = reason {
            builder = builder.attribute("reason", r.clone());
        }
        builder.build()
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
    target: UserId,
    reason: Option<String>,
) -> StoreResult<crate::registry::Audited<(bool, usize)>> {
    let context = AuthorizedCommandContext::<U07>::for_authorized_actor(admin, None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U07>| {
        // Existence probe only -- the role itself is unused here. Also
        // deliberately excludes soft-deleted users (`is_deleted = 0` in
        // get_role_within_tx's WHERE clause): resetting MFA on a deleted
        // account is meaningless, so a soft-deleted target now returns
        // NotFound where the pre-conversion `users::get`-based check
        // would have let it through. See this function's own doc comment.
        crate::repos::users::get_role_within_tx(tx.tx(), target)?;

        let totp_removed = crate::repos::user_totp::delete_within_tx(tx.tx(), target)?;

        let creds =
            crate::repos::user_webauthn_credentials::list_for_user_within_tx(tx.tx(), target)?;
        let passkeys_removed = creds.len();
        for c in &creds {
            crate::repos::user_webauthn_credentials::delete_within_tx(tx.tx(), c.id, target)?;
        }

        let event = U07Event::Reset {
            user_id: target,
            totp_removed,
            passkeys_removed,
            reason,
        };
        Ok(((totp_removed, passkeys_removed), event))
    })
    .await
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
// U08 covers `users::admin_unlock` only. `clear_lockout` — the automatic
// counter reset on a successful post-failure login — is a distinct
// command (U24, Class P, no audit event), not an alternate branch of
// this one; the two share byte-identical SQL but opposite security
// meanings, and conflating them was the second half of the blocking
// finding. Nothing here touches `clear_lockout`.

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
        },
        AttributeSpec {
            name: "refresh_tokens_revoked",
            description: "count of refresh tokens revoked by this change",
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
            ..
        } = self;
        AuditAttributes::builder()
            .attribute("sessions_revoked", sessions_revoked.to_string())
            .attribute("refresh_tokens_revoked", refresh_tokens_revoked.to_string())
            .build()
    }
}

/// Run U09 (self-service password change) through the Class-A runner.
/// `credential` is the already-hashed replacement row — same
/// outside-the-transaction contract as U01/U06. When `revoke_others` is
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
        let (sessions_revoked, refresh_tokens_revoked) = if revoke_others {
            let now = chrono::Utc::now();
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
    attributes: &[],
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
            Completed { user_id: UserId } => &U10_RESET_COMPLETED,
        }
    }
}

impl SealedCommandEvent<U10> for U10Event {
    fn target(&self) -> Option<AuditTarget> {
        let Self::Completed { user_id } = self;
        Some(AuditTarget(user_id.to_string()))
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

/// Run U10 (forgot-password completion) through the Class-A runner:
/// credential swap, token consume, and session/refresh-token revocation
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
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U10>::for_system_actor(None);
    db.class_a(context, move |tx: &mut ClassATx<'_, U10>| {
        crate::repos::credentials::upsert_within_tx(tx.tx(), &credential)?;
        crate::repos::password_reset_tokens::mark_consumed_within_tx(
            tx.tx(),
            token_id,
            consumed_at,
        )?;
        crate::repos::sessions::revoke_all_for_user_within_tx(tx.tx(), user_id, consumed_at)?;
        crate::repos::refresh_tokens::revoke_all_for_user_within_tx(tx.tx(), user_id, consumed_at)?;
        Ok(((), U10Event::Completed { user_id }))
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
        },
        AttributeSpec {
            name: "family_revoked_count",
            description: "how many still-active family members were revoked in this sweep",
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

// ── U30 — session creation (Protocol; proves no Audited<T> path) ───────

/// Run U30 (session creation) through the `Protocol` runner. No event, no
/// audit row is possible here by construction — there is no
/// `WriteTx<Protocol>` method that produces `Audited<T>`. See
/// `tests/compile_fail/protocol_cannot_construct_audited.rs` for the
/// negative proof.
pub async fn insert_session(
    db: &crate::Database,
    session: crate::models::SessionRow,
) -> StoreResult<()> {
    db.protocol(move |write| crate::repos::sessions::insert_within_tx(write.tx(), &session))
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

#[cfg(test)]
mod tests;
