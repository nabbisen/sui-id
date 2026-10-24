//! RFC 094 M2b stage 2: the one property store-level tests cannot
//! exercise — a validation failure (unknown `tls_mode`) must leave the
//! pending row and `smtp_config` intact, because `apply` rejects it
//! *before* ever calling the sealed S08 command
//! (`sui_id_store::commands::apply_pending_settings_change`). The
//! command's own rollback/exactly-once/binding-failure evidence lives in
//! `sui-id-store/src/commands/tests/runner/pending_settings.rs`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use crate::actor::Actor;
use crate::time::system_clock;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::Role;

fn admin_actor_for(user_id: sui_id_shared::ids::UserId) -> crate::actor::AdminActor {
    Actor::from_session(user_id, Role::Admin, SessionId::new())
        .into_admin()
        .expect("admin actor")
}

fn sample_payload(tls_mode: &str) -> SmtpPasswordChangePayload {
    SmtpPasswordChangePayload {
        enabled: true,
        host: "smtp.example.com".into(),
        port: 587,
        tls_mode: tls_mode.into(),
        username: Some("relay-user".into()),
        password: "correct-secret".into(),
        from_address: "noreply@example.com".into(),
        from_name: None,
        base_url: "https://sui-id.example.com".into(),
    }
}

#[tokio::test]
async fn apply_with_an_unknown_tls_mode_leaves_the_pending_row_and_smtp_config_intact() {
    let db = Database::open_in_memory(MasterKey::generate()).expect("db");
    let clock = system_clock();
    let actor_id = sui_id_shared::ids::UserId::new();
    let actor = admin_actor_for(actor_id);
    let session_id = actor.session_id();

    let pending = create(
        &db,
        &actor,
        session_id,
        "smtp_password_update",
        &sample_payload("not-a-real-tls-mode"),
        "SMTP password: will be updated",
        "csrf-token",
        &clock,
    )
    .await
    .expect("create pending change");

    let err = apply(&db, pending.id, &actor, session_id, "csrf-token", &clock)
        .await
        .expect_err("an unknown tls_mode must be rejected");
    let message = format!("{err:?}");
    assert!(
        message.contains("unknown tls_mode"),
        "the error must name the field, not the neutral message: {message}"
    );

    // The pending row is still there -- not consumed by the failed
    // validation (this is RFC 094 M2b stage plan §1's defect, as a test:
    // before this stage, `apply` consumed the row before validating).
    let summary =
        sui_id_store::repos::pending_settings_change::get_summary(&db, pending.id, clock.now())
            .await
            .expect("the pending row must still exist");
    assert_eq!(summary, "SMTP password: will be updated");

    // smtp_config was never touched.
    assert!(
        sui_id_store::repos::smtp_config::get(&db)
            .await
            .expect("get smtp_config")
            .is_none(),
        "smtp_config must not have been written"
    );
}

#[tokio::test]
async fn apply_with_a_valid_payload_writes_smtp_config_end_to_end() {
    let db = Database::open_in_memory(MasterKey::generate()).expect("db");
    let clock = system_clock();
    let actor_id = sui_id_shared::ids::UserId::new();
    let actor = admin_actor_for(actor_id);
    let session_id = actor.session_id();

    let pending = create(
        &db,
        &actor,
        session_id,
        "smtp_password_update",
        &sample_payload("starttls"),
        "SMTP password: will be updated",
        "csrf-token",
        &clock,
    )
    .await
    .expect("create pending change");

    apply(&db, pending.id, &actor, session_id, "csrf-token", &clock)
        .await
        .expect("apply a valid payload");

    let stored = sui_id_store::repos::smtp_config::get(&db)
        .await
        .expect("get smtp_config")
        .expect("smtp_config row exists");
    assert_eq!(stored.host, "smtp.example.com");
    assert_eq!(stored.port, 587);
    assert_eq!(stored.tls_mode, sui_id_store::models::SmtpTlsMode::StartTls);

    // The pending row is gone (single-use).
    assert!(matches!(
        sui_id_store::repos::pending_settings_change::get_summary(&db, pending.id, clock.now())
            .await,
        Err(sui_id_store::StoreError::NotFound)
    ));
}

#[tokio::test]
async fn apply_with_the_wrong_actor_is_refused_with_the_neutral_message_and_leaves_the_row() {
    let db = Database::open_in_memory(MasterKey::generate()).expect("db");
    let clock = system_clock();
    let creator = admin_actor_for(sui_id_shared::ids::UserId::new());
    let impostor = admin_actor_for(sui_id_shared::ids::UserId::new());

    let pending = create(
        &db,
        &creator,
        creator.session_id(),
        "smtp_password_update",
        &sample_payload("starttls"),
        "SMTP password: will be updated",
        "csrf-token",
        &clock,
    )
    .await
    .expect("create pending change");

    let err = apply(
        &db,
        pending.id,
        &impostor,
        impostor.session_id(),
        "csrf-token",
        &clock,
    )
    .await
    .expect_err("a different actor/session must be refused");
    let message = format!("{err:?}");
    assert!(
        message.contains("expired or is no longer valid"),
        "a binding failure must use the same neutral message absence uses: {message}"
    );

    // The row must still exist -- Decision B: a binding failure must
    // not consume it.
    let summary =
        sui_id_store::repos::pending_settings_change::get_summary(&db, pending.id, clock.now())
            .await
            .expect("the pending row must still exist after a binding failure");
    assert_eq!(summary, "SMTP password: will be updated");
}
