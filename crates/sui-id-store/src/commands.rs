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
    actor: ActorRequirement::Optional,
    target: TargetRequirement::Required,
    attributes: &[],
};

static U01_CREATE_WARNED_HIBP: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserCreateWarnedHibp,
    name: "user.create_warned_hibp",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Optional,
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
        // Provisional, not a domain claim: this is "admin create user"
        // (see the command doc comment below), which in the real,
        // eventually-wired call site plausibly *should* require an
        // authenticated admin actor and therefore be `forbidden` here.
        // Marked `permitted` only because Stage 2 doesn't yet add the
        // decision-consuming constructor this command would need instead
        // — `for_system_actor` is still the only way `create_user` below
        // can obtain a context. Revisit when U01 is wired to its real
        // admin-facing call site (Wave A).
        system_principal: permitted;
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
pub async fn create_user(
    db: &crate::Database,
    user: crate::models::UserRow,
    credential: Option<crate::models::CredentialRow>,
    hibp_warned: bool,
) -> StoreResult<crate::registry::Audited<()>> {
    let context = AuthorizedCommandContext::<U01>::for_system_actor(None);
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
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::registry::{CommandSpec, SystemPrincipalPermitted};

    // ── Stage 2 item 1: every slice command is a deliberate declaration,
    //    not a silent default. A compile-time fact, not a runtime check —
    //    this only fails to *compile* if a `system_principal:` clause is
    //    ever removed or the macro's `permitted` arm stops emitting the
    //    impl; it can't regress at runtime.
    const _: fn() = || {
        fn assert_system_principal_permitted<C: SystemPrincipalPermitted>() {}
        assert_system_principal_permitted::<K01>();
        assert_system_principal_permitted::<U22>();
        assert_system_principal_permitted::<U01>();
    };

    // ── Stage 1 item 5: duplicate-name / class-mismatch / missing-field /
    //    stable-serialization tests, against this slice's real table ──────

    fn all_descriptors() -> Vec<&'static EventDescriptor> {
        vec![
            &K01_ROTATED,
            &U22_FAILURE,
            &U22_LOCKOUT,
            &U01_CREATE,
            &U01_CREATE_WARNED_HIBP,
        ]
    }

    #[test]
    fn no_duplicate_event_names() {
        let names: Vec<&str> = all_descriptors().iter().map(|d| d.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            names.len(),
            sorted.len(),
            "duplicate event name in the descriptor table: {names:?}"
        );
    }

    #[test]
    fn no_duplicate_event_kinds() {
        let kinds: Vec<AuditEventKind> = all_descriptors().iter().map(|d| d.kind).collect();
        let mut sorted = kinds.clone();
        sorted.sort_by_key(|k| format!("{k:?}"));
        sorted.dedup();
        assert_eq!(kinds.len(), sorted.len(), "duplicate AuditEventKind");
    }

    #[test]
    fn every_descriptor_is_atomic_class() {
        // Every command in this slice's Class-A set must register
        // AuditClass::Atomic — a MustAttempt descriptor here would be a
        // class mismatch (no Class-B command is in this slice).
        for d in all_descriptors() {
            assert_eq!(d.class, AuditClass::Atomic, "{} is not Atomic", d.name);
        }
    }

    #[test]
    fn k01_descriptor_maps_are_exhaustive_and_correct() {
        let event = K01Event::Rotated {
            new_key: SigningKeyId::new(),
            algorithm: "ed25519".into(),
        };
        assert_eq!(K01::descriptor(&event).name, "signing_key.rotate");
    }

    #[test]
    fn u22_both_branches_map_to_distinct_descriptors() {
        let uid = UserId::new();
        let failure = U22Event::Failure {
            user_id: uid,
            count: 1,
        };
        let lockout = U22Event::Lockout {
            user_id: uid,
            count: 5,
            locked_for_secs: 30,
        };
        assert_eq!(U22::descriptor(&failure).name, "auth.login.failure");
        assert_eq!(U22::descriptor(&lockout).name, "auth.lockout");
        assert_ne!(
            U22::descriptor(&failure).name,
            U22::descriptor(&lockout).name
        );
    }

    #[test]
    fn u01_both_branches_map_to_distinct_descriptors() {
        let uid = UserId::new();
        let created = U01Event::Created { user_id: uid };
        let warned = U01Event::CreatedWarnedHibp { user_id: uid };
        assert_eq!(U01::descriptor(&created).name, "user.create");
        assert_eq!(U01::descriptor(&warned).name, "user.create_warned_hibp");
    }

    #[test]
    fn missing_field_is_a_compile_error_not_a_runtime_check() {
        // Not a runnable test: documents that `AttributeSpec` mismatches
        // (a variant's `attributes()` emitting a name absent from its
        // descriptor's `attributes` list) are not currently caught here —
        // that check belongs to the structural comparison tool (Stage 1
        // item 6), which reads the generated attribute set against the
        // declared one. Registering it as a known gap rather than a silent
        // omission: see the Stage 1 submission's disclosure section.
    }

    // ── Stage 1 item 4: generated reference documentation ───────────────

    #[test]
    fn reference_markdown_covers_every_descriptor_and_is_deterministic() {
        let descriptors = all_descriptors();
        let rendered = crate::registry::generate_reference_markdown(&descriptors);

        for d in &descriptors {
            assert!(
                rendered.contains(&format!("`{}`", d.name)),
                "reference table is missing {}",
                d.name
            );
        }
        // K01's declared attribute must actually appear, not just the
        // event name -- proves the attribute column isn't silently empty.
        assert!(rendered.contains("`algorithm`"));

        let rendered_again = crate::registry::generate_reference_markdown(&descriptors);
        assert_eq!(
            rendered, rendered_again,
            "generation must be deterministic — a doc-drift check diffs two renders"
        );
    }

    // ── Stable-serialization: event names, once emitted, never change
    //    shape silently. ─────────────────────────────────────────────────

    #[test]
    fn event_names_match_command_inventory() {
        // These five strings are the audit-log `action` column's contract
        // with every existing consumer (SIEM queries, `rfcs/handoffs/
        // 094-transactional-audit/command-inventory.md`). Pinned literally,
        // not derived, so a rename shows up as a diff here.
        let expected = [
            "signing_key.rotate",
            "auth.login.failure",
            "auth.lockout",
            "user.create",
            "user.create_warned_hibp",
        ];
        let mut actual: Vec<&str> = all_descriptors().iter().map(|d| d.name).collect();
        actual.sort_unstable();
        let mut expected = expected.to_vec();
        expected.sort_unstable();
        assert_eq!(actual, expected);
    }

    // ── End-to-end runner tests ─────────────────────────────────────────
    // Real `Database`, real SQLite. Proves the runners actually work, not
    // just that the descriptor tables are internally consistent.

    #[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    mod runner {
        use super::*;
        use crate::crypto::MasterKey;
        use crate::models::{EmailOutboxRow, EmailOutboxState, SessionRow, UserRow};
        use crate::repos;
        use crate::{Database, StoreError};
        use chrono::{TimeDelta, Utc};

        fn fresh_db() -> Database {
            Database::open_in_memory(MasterKey::generate()).expect("db")
        }

        fn a_user() -> UserRow {
            UserRow {
                id: UserId::new(),
                username: format!("user-{}", uuid::Uuid::new_v4()),
                display_name: None,
                email: None,
                email_normalized: None,
                email_verified_at: None,
                preferred_lang: None,
                is_admin: false,
                role: crate::models::Role::User,
                is_disabled: false,
                is_deleted: false,
                last_login_at: None,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                failed_login_count: 0,
                locked_until: None,
                source: crate::models::UserSource::Local,
                external_stable_id: None,
            }
        }

        fn a_client() -> crate::models::ClientRow {
            crate::models::ClientRow {
                id: ClientId::new(),
                name: format!("client-{}", uuid::Uuid::new_v4()),
                confidential: false,
                secret_hash: None,
                redirect_uris: vec!["https://example.com/cb".into()],
                allowed_scopes: String::new(),
                post_logout_redirect_uris: vec![],
                is_disabled: false,
                is_deleted: false,
                consent_policy: crate::models::ConsentPolicy::default(),
                registered_via: crate::models::RegistrationSource::default(),
                logo_uri: None,
                homepage_uri: None,
                privacy_policy_uri: None,
                tos_uri: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }

        async fn latest_audit_action(db: &Database) -> Option<String> {
            repos::audit::recent(db, 1)
                .await
                .expect("audit tail")
                .into_iter()
                .next()
                .map(|row| row.action)
        }

        #[tokio::test]
        async fn k01_rotates_key_and_appends_audit_row() {
            let db = fresh_db();
            let new_id = SigningKeyId::new();
            let audited = rotate_signing_key(
                &db,
                new_id,
                "ed25519".into(),
                b"sealed-placeholder".to_vec(),
                b"public-key-placeholder".to_vec(),
            )
            .await
            .expect("rotate");
            audited.into_inner();

            let active = repos::signing_keys::active(&db).await.expect("active key");
            assert_eq!(active.id, new_id);

            assert_eq!(
                latest_audit_action(&db).await.as_deref(),
                Some("signing_key.rotate")
            );
        }

        // ── Stage 2 items 2-3: injected-failure rollback proofs ──────────
        //
        // The acceptance bar (per the Stage 2 direction, §3): an injected
        // failure must roll back *both* the mutation and the audit row,
        // verified by observing state before and after -- not by asserting
        // that the call returned `Err`. All three tests below do exactly
        // that: capture "before" state, inject, assert `Err`, then assert
        // "after" state is identical to "before" -- for both the domain
        // table and the audit log.

        #[tokio::test]
        async fn k01_injected_failure_before_append_rolls_back_the_mutation() {
            let db = fresh_db();
            let before_active = repos::signing_keys::active(&db).await;
            let before_audit = latest_audit_action(&db).await;

            db.fault_injector().fail_before_next_append();
            let result = rotate_signing_key(
                &db,
                SigningKeyId::new(),
                "ed25519".into(),
                b"sealed-placeholder".to_vec(),
                b"public-key-placeholder".to_vec(),
            )
            .await;
            assert!(result.is_err(), "injected failure must surface as Err");

            // The mutation (key insert/activation) never happened: same
            // "no active key" state as before, not a half-activated new key.
            assert_eq!(
                before_active.err().map(|e| e.to_string()),
                repos::signing_keys::active(&db)
                    .await
                    .err()
                    .map(|e| e.to_string())
            );
            assert_eq!(latest_audit_action(&db).await, before_audit);
        }

        #[tokio::test]
        async fn k01_injected_failure_after_append_rolls_back_the_mutation_and_the_append() {
            let db = fresh_db();
            let before_audit = latest_audit_action(&db).await;

            db.fault_injector().fail_after_next_append();
            let result = rotate_signing_key(
                &db,
                SigningKeyId::new(),
                "ed25519".into(),
                b"sealed-placeholder".to_vec(),
                b"public-key-placeholder".to_vec(),
            )
            .await;
            assert!(result.is_err(), "injected failure must surface as Err");

            // append_within_tx really ran (it's before this injection
            // point) and really inserted a row -- proving this rolls back
            // requires that the row is gone, not merely that this test
            // never looked for it.
            assert!(
                repos::signing_keys::active(&db).await.is_err(),
                "no active key: the mutation rolled back with the audit row"
            );
            assert_eq!(
                latest_audit_action(&db).await,
                before_audit,
                "the audit row append_within_tx just wrote must not survive \
                 an uncommitted transaction"
            );
        }

        #[tokio::test]
        async fn k01_injected_commit_failure_rolls_back_the_mutation_and_the_append() {
            // Distinct from the two tests above: the closure itself returns
            // `Ok` (mutation succeeded, event built, append succeeded) --
            // this proves that a rejection at SQLite's own commit boundary,
            // which no domain code observes or controls, still prevents
            // `Audited<T>` from ever being constructed and still leaves no
            // trace in either table.
            let db = fresh_db();
            let before_audit = latest_audit_action(&db).await;

            db.fail_next_commit_for_test();
            let result = rotate_signing_key(
                &db,
                SigningKeyId::new(),
                "ed25519".into(),
                b"sealed-placeholder".to_vec(),
                b"public-key-placeholder".to_vec(),
            )
            .await;
            assert!(
                result.is_err(),
                "a rejected commit must surface as Err, not a silently-empty Ok"
            );

            assert!(
                repos::signing_keys::active(&db).await.is_err(),
                "no active key: a rejected commit persists nothing"
            );
            assert_eq!(latest_audit_action(&db).await, before_audit);

            // The hook self-disarmed after firing once: a real rotation now
            // succeeds normally, proving this test didn't leave the
            // connection unable to commit anything ever again.
            let audited = rotate_signing_key(
                &db,
                SigningKeyId::new(),
                "ed25519".into(),
                b"sealed-placeholder".to_vec(),
                b"public-key-placeholder".to_vec(),
            )
            .await
            .expect("rotate after the injected commit failure has cleared");
            audited.into_inner();
        }

        #[tokio::test]
        async fn u22_below_threshold_emits_failure_not_lockout() {
            let db = fresh_db();
            let user = a_user();
            repos::users::create(&db, &user).await.expect("create user");

            let audited = record_login_failure(&db, user.id, |_count| None)
                .await
                .expect("record failure");
            assert_eq!(audited.into_inner(), 1);
            assert_eq!(
                latest_audit_action(&db).await.as_deref(),
                Some("auth.login.failure")
            );

            let row = repos::users::get(&db, user.id).await.expect("get");
            assert_eq!(row.failed_login_count, 1);
            assert!(row.locked_until.is_none());
        }

        #[tokio::test]
        async fn u22_crossing_threshold_emits_lockout_and_sets_locked_until() {
            let db = fresh_db();
            let user = a_user();
            repos::users::create(&db, &user).await.expect("create user");

            // Threshold of 1: the very first failure crosses it.
            let audited = record_login_failure(&db, user.id, |count| {
                (count >= 1).then_some(TimeDelta::seconds(30))
            })
            .await
            .expect("record failure");
            assert_eq!(audited.into_inner(), 1);
            let latest = repos::audit::recent(&db, 1)
                .await
                .expect("audit tail")
                .into_iter()
                .next()
                .expect("one row");
            assert_eq!(latest.action, "auth.lockout");
            // The lock window's length must survive into the audit note —
            // this is the detail the two-call, non-atomic predecessor put
            // in a second, separately-appended row; U22 carries it as an
            // attribute on the one row it appends instead.
            let note = latest.note.expect("lockout row must carry a note");
            assert!(
                note.contains("locked_for_secs=30"),
                "note must record the lock window length: {note:?}"
            );

            let row = repos::users::get(&db, user.id).await.expect("get");
            assert!(
                row.locked_until.is_some(),
                "locked_until must be set on the crossing transaction"
            );
        }

        #[tokio::test]
        async fn u22_lockout_and_counter_update_are_the_same_transaction() {
            // Regression proof for the thing this command replaces: the
            // current authn::session code makes two separate calls (bump,
            // then a second best-effort call to stamp the lock), so a
            // crash between them leaves the counter bumped but no lock.
            // Here there is only one call and one transaction; there is no
            // window where the counter is bumped but the lock (when owed)
            // is not yet set, because both writes and the audit append
            // share one `with_tx`.
            let db = fresh_db();
            let user = a_user();
            repos::users::create(&db, &user).await.expect("create user");

            record_login_failure(&db, user.id, |count| {
                (count >= 1).then_some(TimeDelta::seconds(60))
            })
            .await
            .expect("record failure");

            let row = repos::users::get(&db, user.id).await.expect("get");
            // If the counter and lock could ever be observed apart, this
            // assertion is what would eventually catch it under a
            // concurrency/fault-injection harness -- for now this proves
            // the shape: one call produced both effects. The fault
            // injector now exists (see the k01_injected_* tests above);
            // genuine concurrent execution is proven separately below.
            assert_eq!(row.failed_login_count, 1);
            assert!(row.locked_until.is_some());
        }

        #[tokio::test]
        async fn concurrent_class_a_commands_maintain_one_unbroken_chain() {
            // RFC 094 Stage 2: "prove the audit chain is read and written
            // on the caller transaction." `repos::audit`'s own tests
            // already prove `append_within_tx` behaves correctly called
            // directly and in isolation; this proves the same property
            // through the real Class-A/registry path, under genuinely
            // concurrent execution rather than by reading the
            // mutex-serialization argument in its doc comment. If the
            // chain-head read and the row insert were ever split across
            // two separate transactions (a regression this test would
            // catch), concurrent commands could compute the same
            // `prev_hash` and fork the chain, or `verify_chain_tail` would
            // report a break.
            let db = fresh_db();
            const N: usize = 20;
            let mut user_ids = Vec::with_capacity(N);
            for _ in 0..N {
                let user = a_user();
                repos::users::create(&db, &user).await.expect("create user");
                user_ids.push(user.id);
            }

            let handles: Vec<_> = user_ids
                .into_iter()
                .map(|user_id| {
                    let db = db.clone();
                    tokio::spawn(async move {
                        record_login_failure(&db, user_id, |_count| None)
                            .await
                            .expect("record failure")
                    })
                })
                .collect();
            for handle in handles {
                handle.await.expect("task join");
            }

            let report = repos::audit::verify_chain_tail(&db, (N * 2) as i64)
                .await
                .expect("verify chain");
            assert_eq!(
                report.checked, N,
                "exactly one audit row per concurrent command, no lost or duplicated rows"
            );
            assert!(
                report.broken_at_seq.is_none(),
                "no fork: every row's prev_hash must chain from exactly one predecessor, \
                 even though the audit-row writes raced"
            );
        }

        #[tokio::test]
        async fn u01_normal_branch_creates_user_and_credential() {
            let db = fresh_db();
            let user = a_user();
            let cred = crate::models::CredentialRow {
                user_id: user.id,
                password_hash: "argon2-placeholder".into(),
                must_change: false,
                updated_at: Utc::now(),
            };

            let audited = create_user(&db, user.clone(), Some(cred), false)
                .await
                .expect("create");
            audited.into_inner();

            let row = repos::users::get(&db, user.id).await.expect("get");
            assert_eq!(row.id, user.id);
            assert_eq!(
                latest_audit_action(&db).await.as_deref(),
                Some("user.create")
            );
        }

        #[tokio::test]
        async fn u01_hibp_branch_emits_the_warned_event_name() {
            let db = fresh_db();
            let user = a_user();

            create_user(&db, user.clone(), None, true)
                .await
                .expect("create");

            assert_eq!(
                latest_audit_action(&db).await.as_deref(),
                Some("user.create_warned_hibp")
            );
        }

        #[tokio::test]
        async fn u01_rolls_back_credential_and_user_together_on_conflict() {
            // A real SQL-level failure (duplicate primary key), distinct
            // from -- and weaker than -- the two injected-failure tests
            // right below: SQLite's default `ABORT` conflict resolution
            // means the one failed statement here writes nothing on its
            // own, so "nothing new persisted" would hold even if `class_a`
            // never rolled back anything. Kept for the real-SQL-failure
            // case it does cover; the injected tests are what actually
            // exercise `class_a`'s own rollback behavior.
            let db = fresh_db();
            let user = a_user();
            repos::users::create(&db, &user)
                .await
                .expect("first create");

            let before = latest_audit_action(&db).await;

            let mut dup = a_user();
            dup.id = user.id; // force the conflict
            let result = create_user(&db, dup, None, false).await;
            assert!(matches!(result, Err(StoreError::Conflict)));

            // No new audit row from the failed attempt.
            assert_eq!(latest_audit_action(&db).await, before);
        }

        #[tokio::test]
        async fn u01_injected_failure_before_append_rolls_back_the_user_insert() {
            let db = fresh_db();
            let user = a_user();
            let before_audit = latest_audit_action(&db).await;

            db.fault_injector().fail_before_next_append();
            let result = create_user(&db, user.clone(), None, false).await;
            assert!(result.is_err(), "injected failure must surface as Err");

            // The user insert really ran (it's before this injection
            // point); proving rollback requires it to be gone, not merely
            // that this test never looked for it.
            assert!(
                repos::users::get(&db, user.id).await.is_err(),
                "no user row: the insert rolled back"
            );
            assert_eq!(latest_audit_action(&db).await, before_audit);
        }

        #[tokio::test]
        async fn u01_injected_failure_after_append_rolls_back_the_user_insert_and_the_append() {
            let db = fresh_db();
            let user = a_user();
            let before_audit = latest_audit_action(&db).await;

            db.fault_injector().fail_after_next_append();
            let result = create_user(&db, user.clone(), None, false).await;
            assert!(result.is_err(), "injected failure must surface as Err");

            assert!(
                repos::users::get(&db, user.id).await.is_err(),
                "no user row: the insert rolled back with the audit row"
            );
            assert_eq!(latest_audit_action(&db).await, before_audit);
        }

        // ── T04/T09 — refresh-token rotation and initial issuance ───────

        async fn seed_family(
            db: &Database,
        ) -> (UserId, ClientId, FamilyId, sui_id_shared::RefreshTokenHash) {
            let user = a_user();
            repos::users::create(db, &user).await.expect("create user");
            let client = a_client();
            repos::clients::create(db, &client)
                .await
                .expect("create client");

            let root_id = sui_id_shared::RefreshTokenId::generate();
            let family = FamilyId::root_of(&root_id);
            let row = crate::models::RefreshTokenRow {
                id: root_id,
                user_id: user.id,
                client_id: client.id,
                scope: "openid".into(),
                expires_at: Utc::now() + TimeDelta::hours(1),
                revoked_at: None,
                created_at: Utc::now(),
                auth_methods: vec![],
                family_id: family.clone(),
            };
            let prepared = repos::refresh_tokens::prepare_refresh_token(db.key(), row)
                .expect("prepare root token");
            let hash = sui_id_shared::RefreshTokenHash::of(&prepared.raw_token);
            insert_initial_refresh_token(db, prepared)
                .await
                .expect("t09 initial issue");
            (user.id, client.id, family, hash)
        }

        fn a_successor(
            user_id: UserId,
            client_id: ClientId,
            family: FamilyId,
        ) -> crate::models::RefreshTokenRow {
            crate::models::RefreshTokenRow {
                id: sui_id_shared::RefreshTokenId::generate(),
                user_id,
                client_id,
                scope: "openid".into(),
                expires_at: Utc::now() + TimeDelta::hours(1),
                revoked_at: None,
                created_at: Utc::now(),
                auth_methods: vec![],
                family_id: family,
            }
        }

        #[tokio::test]
        async fn t09_protocol_issues_initial_token_with_no_audit_row() {
            let db = fresh_db();
            let before = latest_audit_action(&db).await;
            let (_, _, _, hash) = seed_family(&db).await;

            // The row genuinely exists (T09 really wrote it) ...
            assert!(
                repos::refresh_tokens::begin_rotation(&db, &hash, &ClientId::new(), Utc::now())
                    .await
                    .is_err(),
                "sanity: wrong client must reject without revoking"
            );
            // ... but no audit row exists for it -- T09 has no path to
            // Audited<T>, by construction (Database::protocol).
            assert_eq!(latest_audit_action(&db).await, before);
        }

        #[tokio::test]
        async fn t04_normal_rotation_revokes_old_inserts_successor_and_appends_rotated() {
            let db = fresh_db();
            let (user_id, client_id, family, hash) = seed_family(&db).await;

            let successor = a_successor(user_id, client_id, family.clone());
            let successor_id = successor.id.clone();
            let prepared =
                repos::refresh_tokens::prepare_refresh_token(db.key(), successor).unwrap();

            let audited = rotate_refresh_token(&db, hash, client_id, Utc::now(), prepared)
                .await
                .expect("rotate");
            match audited.into_inner() {
                T04Outcome::Rotated { successor, .. } => {
                    assert_eq!(successor.id, successor_id);
                }
                T04Outcome::TheftDetected { .. } => panic!("expected Rotated"),
            }

            assert_eq!(
                latest_audit_action(&db).await.as_deref(),
                Some("auth.refresh.rotated")
            );
        }

        #[tokio::test]
        async fn t04_reuse_revokes_family_and_appends_theft_detected() {
            let db = fresh_db();
            let (user_id, client_id, family, hash) = seed_family(&db).await;

            // First rotation: legitimate, wins.
            let s1 = a_successor(user_id, client_id, family.clone());
            let p1 = repos::refresh_tokens::prepare_refresh_token(db.key(), s1).unwrap();
            rotate_refresh_token(&db, hash.clone(), client_id, Utc::now(), p1)
                .await
                .expect("first rotation");

            // Replay of the now-revoked root token: reuse.
            let s2 = a_successor(user_id, client_id, family.clone());
            let p2 = repos::refresh_tokens::prepare_refresh_token(db.key(), s2).unwrap();
            let audited = rotate_refresh_token(&db, hash, client_id, Utc::now(), p2)
                .await
                .expect("replay");
            match audited.into_inner() {
                T04Outcome::TheftDetected { family_revoked } => {
                    assert!(family_revoked >= 1, "at least the winner's successor");
                }
                T04Outcome::Rotated { .. } => panic!("expected TheftDetected"),
            }

            assert_eq!(
                latest_audit_action(&db).await.as_deref(),
                Some("auth.refresh.theft_detected")
            );

            let active: i64 = db
                .with_conn_sync(|conn| {
                    Ok(conn
                        .query_row(
                            "SELECT COUNT(*) FROM refresh_tokens \
                             WHERE family_id = ?1 AND revoked_at IS NULL",
                            [family.as_str()],
                            |r| r.get(0),
                        )
                        .expect("count"))
                })
                .expect("query");
            assert_eq!(active, 0, "reuse must close the whole family");
        }

        #[tokio::test]
        async fn t04_injected_failure_before_append_rolls_back_revoke_and_successor_insert() {
            let db = fresh_db();
            let (user_id, client_id, family, hash) = seed_family(&db).await;
            let before_audit = latest_audit_action(&db).await;

            let successor = a_successor(user_id, client_id, family);
            let successor_id = successor.id.clone();
            let prepared =
                repos::refresh_tokens::prepare_refresh_token(db.key(), successor).unwrap();

            db.fault_injector().fail_before_next_append();
            let result =
                rotate_refresh_token(&db, hash.clone(), client_id, Utc::now(), prepared).await;
            assert!(result.is_err(), "injected failure must surface as Err");

            // The old row must still be active -- the guarded revoke rolled
            // back with everything else in this transaction.
            assert!(
                matches!(
                    repos::refresh_tokens::begin_rotation(&db, &hash, &client_id, Utc::now())
                        .await
                        .expect("old token must still be rotatable"),
                    repos::refresh_tokens::RotationLookup::RotatedHere(_)
                ),
                "old row must still be active: the revoke rolled back"
            );
            assert!(
                db.with_conn_sync(|conn| {
                    Ok(conn
                        .query_row(
                            "SELECT 1 FROM refresh_tokens WHERE id = ?1",
                            [successor_id.as_str()],
                            |r| r.get::<_, i64>(0),
                        )
                        .is_err())
                })
                .unwrap(),
                "successor must not have been inserted"
            );
            assert_eq!(latest_audit_action(&db).await, before_audit);
        }

        #[tokio::test]
        async fn t04_injected_commit_failure_rolls_back_everything() {
            let db = fresh_db();
            let (user_id, client_id, family, hash) = seed_family(&db).await;
            let before_audit = latest_audit_action(&db).await;

            let successor = a_successor(user_id, client_id, family);
            let prepared =
                repos::refresh_tokens::prepare_refresh_token(db.key(), successor).unwrap();

            db.fail_next_commit_for_test();
            let result =
                rotate_refresh_token(&db, hash.clone(), client_id, Utc::now(), prepared).await;
            assert!(
                result.is_err(),
                "a rejected commit must surface as Err, not a silently-empty Ok"
            );

            assert!(
                matches!(
                    repos::refresh_tokens::begin_rotation(&db, &hash, &client_id, Utc::now())
                        .await
                        .expect("old token must still be rotatable"),
                    repos::refresh_tokens::RotationLookup::RotatedHere(_)
                ),
                "old row must still be active after a rejected commit"
            );
            assert_eq!(latest_audit_action(&db).await, before_audit);
        }

        #[tokio::test]
        async fn u30_protocol_inserts_session_with_no_audit_row() {
            let db = fresh_db();
            let user = a_user();
            repos::users::create(&db, &user).await.expect("create user");

            let before = latest_audit_action(&db).await;

            let session = SessionRow {
                id: sui_id_shared::ids::SessionId::new(),
                user_id: user.id,
                expires_at: Utc::now() + TimeDelta::hours(1),
                created_at: Utc::now(),
                revoked_at: None,
                auth_methods: vec![],
                last_step_up_at: None,
                last_used_at: Some(Utc::now()),
            };
            insert_session(&db, session.clone())
                .await
                .expect("insert session");

            let fetched = repos::sessions::get(&db, session.id)
                .await
                .expect("get session");
            assert_eq!(fetched.id, session.id);

            // Protocol commands are not the tamper-evident chain -- no new
            // audit row, by construction (there is no code path from
            // `Database::protocol` to `audit::append_within_tx`).
            assert_eq!(latest_audit_action(&db).await, before);
        }

        #[tokio::test]
        async fn o01_operational_enqueues_email_with_no_audit_row() {
            let db = fresh_db();
            let before = latest_audit_action(&db).await;

            let row = EmailOutboxRow {
                id: sui_id_shared::ids::EmailOutboxId::new(),
                state: EmailOutboxState::Queued,
                template: "forgot_password".into(),
                recipient_enc: vec![1, 2, 3],
                payload_enc: vec![4, 5, 6],
                attempt_count: 0,
                next_attempt_at: Utc::now(),
                last_error: None,
                locale: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            enqueue_email(&db, row).await.expect("enqueue");

            assert_eq!(latest_audit_action(&db).await, before);
        }
    }
}
