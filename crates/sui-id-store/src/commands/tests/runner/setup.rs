//! Cycle I stage 4, S10 — first-run setup becomes one transaction.
//!
//! Before this stage the append already existed and propagated
//! (`setup.rs`'s own `.await?`), but as the fifth of five writes, the
//! first four each committing on their own. A failure in the fifth left
//! an operator told setup failed when it had not. One transaction now:
//! user, credential, the conditional signing-key bootstrap, and the
//! initialized flag commit with the event, or none of them do.
//!
//! RFC 120 D5/D6's named first-run exemption: `ActorRequirement::None`,
//! `system_principal: permitted`. The target (the created admin's user
//! id) carries what the actor column would otherwise duplicate.

use super::*;

fn sealed_signing_key(db: &Database) -> crate::commands::InitialSigningKeyMaterial {
    crate::commands::InitialSigningKeyMaterial {
        id: SigningKeyId::new(),
        algorithm: "EdDSA",
        private_key_sealed: crate::repos::signing_keys::seal_private_key(
            db,
            b"initial-admin-test-signing-key-bytes",
        )
        .expect("seal"),
        public_key: b"test-public-key".to_vec(),
    }
}

fn initial_admin(username: &str) -> (UserRow, crate::models::CredentialRow) {
    let mut user = a_user();
    user.username = username.to_owned();
    user.is_admin = true;
    user.role = crate::models::Role::Admin;
    let cred = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "$argon2id$placeholder$for-the-test".into(),
        updated_at: Utc::now(),
    };
    (user, cred)
}

async fn last_event(db: &Database) -> crate::models::AuditLogRow {
    repos::audit::recent(db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row")
}

#[tokio::test]
async fn s10_headless_setup_creates_admin_credential_key_and_marks_initialized() {
    let db = fresh_db();
    let (user, cred) = initial_admin("first-admin");
    let user_id = user.id;
    let before = audit_rows(&db).await;

    let audited = crate::commands::complete_initial_setup(
        &db,
        user,
        cred,
        sealed_signing_key(&db),
        crate::commands::SetupAuthorizedBy::Filesystem,
        Utc::now(),
    )
    .await
    .expect("setup");
    audited.into_inner();

    assert!(
        repos::state::is_initialized(&db).expect("state read"),
        "the initialized flag must be set"
    );
    repos::users::get(&db, user_id)
        .await
        .expect("the admin user must exist");
    repos::credentials::get(&db, user_id)
        .await
        .expect("the credential must exist");
    repos::signing_keys::active(&db)
        .await
        .expect("a signing key must now be active");

    let event = last_event(&db).await;
    assert_eq!(event.action, "admin.setup.completed");
    assert_eq!(
        event.actor, None,
        "RFC 120: no authenticated actor at first run"
    );
    assert_eq!(event.target.as_deref(), Some(user_id.to_string().as_str()));
    assert_eq!(event.note.as_deref(), Some("authorized_by=filesystem"));
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S10");
}

#[tokio::test]
async fn s10_web_wizard_setup_records_authorized_by_setup_token() {
    let db = fresh_db();
    let (user, cred) = initial_admin("first-admin");

    crate::commands::complete_initial_setup(
        &db,
        user,
        cred,
        sealed_signing_key(&db),
        crate::commands::SetupAuthorizedBy::SetupToken,
        Utc::now(),
    )
    .await
    .expect("setup");

    assert_eq!(
        last_event(&db).await.note.as_deref(),
        Some("authorized_by=setup_token")
    );
}

#[tokio::test]
async fn s10_does_not_duplicate_an_already_active_signing_key() {
    let db = fresh_db();
    // Seed an active key before setup runs, the way a restored or
    // pre-provisioned database might already have one.
    let existing = repos::signing_keys::insert_with_plaintext(
        &db,
        SigningKeyId::new(),
        "EdDSA",
        b"pre-existing-key",
        b"pre-existing-pub",
        true,
    )
    .await
    .expect("seed an active key");

    let (user, cred) = initial_admin("first-admin");
    crate::commands::complete_initial_setup(
        &db,
        user,
        cred,
        sealed_signing_key(&db),
        crate::commands::SetupAuthorizedBy::Filesystem,
        Utc::now(),
    )
    .await
    .expect("setup");

    let active_keys = repos::signing_keys::list_active(&db)
        .await
        .expect("list active");
    assert_eq!(
        active_keys.len(),
        1,
        "setup must not mint a second active key"
    );
    assert_eq!(
        active_keys[0].id, existing.id,
        "the pre-existing key must remain the active one"
    );
}

#[tokio::test]
async fn s10_against_an_already_initialized_system_writes_no_record() {
    let db = fresh_db();
    let (first, first_cred) = initial_admin("first-admin");
    crate::commands::complete_initial_setup(
        &db,
        first,
        first_cred,
        sealed_signing_key(&db),
        crate::commands::SetupAuthorizedBy::Filesystem,
        Utc::now(),
    )
    .await
    .expect("first setup");

    let before = audit_rows(&db).await;
    let (second, second_cred) = initial_admin("second-admin");
    let result = crate::commands::complete_initial_setup(
        &db,
        second,
        second_cred,
        sealed_signing_key(&db),
        crate::commands::SetupAuthorizedBy::Filesystem,
        Utc::now(),
    )
    .await;
    assert!(
        matches!(result, Err(StoreError::AlreadyInitialized)),
        "expected AlreadyInitialized"
    );
    assert_eq!(audit_rows(&db).await, before, "no second record written");
    assert_eq!(
        repos::state::user_count(&db).expect("user count"),
        1,
        "the second admin must not have been created"
    );
}

/// The dispatch's own required test: two concurrent first-run submissions
/// must not both pass. SQLite's single-writer transaction serializes the
/// two `complete_initial_setup` calls; which one wins is the scheduler's
/// to decide (stage 3's own `tokio::join!` precedent — read the outcome
/// off whichever actually won rather than assuming it), but exactly one
/// must win and exactly one admin and one audit row must exist
/// afterwards.
#[tokio::test]
async fn s10_concurrent_setup_attempts_only_one_succeeds() {
    let db = fresh_db();
    let (a, a_cred) = initial_admin("admin-a");
    let (b, b_cred) = initial_admin("admin-b");
    let now = Utc::now();

    let (first, second) = tokio::join!(
        crate::commands::complete_initial_setup(
            &db,
            a,
            a_cred,
            sealed_signing_key(&db),
            crate::commands::SetupAuthorizedBy::Filesystem,
            now,
        ),
        crate::commands::complete_initial_setup(
            &db,
            b,
            b_cred,
            sealed_signing_key(&db),
            crate::commands::SetupAuthorizedBy::SetupToken,
            now,
        ),
    );

    let first = first.map(|_| ());
    let second = second.map(|_| ());
    match (first, second) {
        (Ok(()), Err(StoreError::AlreadyInitialized)) => {}
        (Err(StoreError::AlreadyInitialized), Ok(())) => {}
        other => panic!(
            "expected exactly one winner (Ok, AlreadyInitialized) in either order, got {other:?}"
        ),
    }

    assert_eq!(
        repos::state::user_count(&db).expect("user count"),
        1,
        "exactly one admin must exist"
    );
    assert_eq!(audit_rows(&db).await, 1, "exactly one audit row must exist");
}

#[tokio::test]
async fn s10_injected_failure_before_append_leaves_no_partial_state() {
    let db = fresh_db();
    let (user, cred) = initial_admin("first-admin");
    let user_id = user.id;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::complete_initial_setup(
        &db,
        user,
        cred,
        sealed_signing_key(&db),
        crate::commands::SetupAuthorizedBy::Filesystem,
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        !repos::state::is_initialized(&db).expect("state read"),
        "the rollback must leave the system not initialized"
    );
    assert!(
        matches!(
            repos::users::get(&db, user_id).await,
            Err(StoreError::NotFound)
        ),
        "the rollback must leave no admin user"
    );
    assert!(
        matches!(
            repos::credentials::get(&db, user_id).await,
            Err(StoreError::NotFound)
        ),
        "the rollback must leave no credential"
    );
    assert!(
        repos::signing_keys::active(&db).await.is_err(),
        "the rollback must leave no active signing key"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit, "no event");
    record_rollback_coverage("S10");
}
