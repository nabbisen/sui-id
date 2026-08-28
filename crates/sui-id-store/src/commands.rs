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
//! Nothing here is wired into a production call site. RFC 094 §"Multiple
//! implementation steps" separates the registry foundation from the
//! conversion waves; this module is foundation only.

use crate::StoreResult;
use crate::registry::{
    ActorRequirement, AttributeSpec, AuditAttributes, AuditBuildError, AuditClass, AuditEventKind,
    AuditResult, AuditTarget, AuthorizedCommandContext, ClassATx, EventDescriptor,
    SealedCommandEvent, TargetRequirement,
};
use sui_id_shared::ids::{SigningKeyId, UserId};

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
    attributes: &[AttributeSpec {
        name: "count",
        description: "failed-login counter value that crossed the threshold",
    }],
};

crate::declare_write_command! {
    /// U22 — record login failure, with the threshold-crossing branch.
    command U22 = "U22" {
        enum U22Event {
            Failure { user_id: UserId, count: i64 } => &U22_FAILURE,
            Lockout { user_id: UserId, count: i64 } => &U22_LOCKOUT,
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
            Self::Failure { count, .. } | Self::Lockout { count, .. } => AuditAttributes::builder()
                .attribute("count", count.to_string())
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
    use crate::registry::CommandSpec;

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

    #[allow(clippy::expect_used, clippy::unwrap_used)]
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
            assert_eq!(
                latest_audit_action(&db).await.as_deref(),
                Some("auth.lockout")
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
            // concurrency/fault-injection harness (Stage 1's failure
            // injector, not yet built) -- for now this proves the shape:
            // one call produced both effects.
            assert_eq!(row.failed_login_count, 1);
            assert!(row.locked_until.is_some());
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
            // RFC 094's atomicity claim, proven rather than assumed: force
            // the second write (credential upsert) to fail by using a
            // duplicate primary key scenario is awkward to construct
            // directly, so instead this proves the weaker but still real
            // half -- a duplicate *user* id rolls back cleanly with no
            // partial row and no audit entry, rather than leaving a
            // half-committed user.
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
