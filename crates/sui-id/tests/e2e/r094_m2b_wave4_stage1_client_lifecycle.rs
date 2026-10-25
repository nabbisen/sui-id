//! RFC 094 M2b wave 4 stage 1 — C05 (disable client): an e2e assertion
//! through the real `sui-id-core` admin use-case layer, not the sealed
//! command directly. A handler reverted to a same-shaped, pre-sealing
//! wrapper still compiles and still passes a value-only e2e test — this
//! asserts the audit record too.

use super::common::*;

#[tokio::test]
async fn disable_client_through_the_admin_handler_revokes_tokens_and_is_audited() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let (client_id_str, _secret) = create_client(&state, &session).await;
    let client_id: sui_id_shared::ids::ClientId = client_id_str.parse().expect("parse client id");

    // Seed one active refresh token for this client.
    let token = sui_id_shared::RawRefreshToken::from_untrusted(uuid::Uuid::new_v4().to_string());
    let token_id = sui_id_shared::RefreshTokenId::generate();
    let row = sui_id_store::models::RefreshTokenRow {
        id: token_id.clone(),
        user_id: admin_id,
        client_id,
        scope: "openid".into(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        revoked_at: None,
        created_at: chrono::Utc::now(),
        auth_methods: vec![],
        family_id: sui_id_shared::FamilyId::root_of(&token_id),
    };
    sui_id_store::repos::refresh_tokens::insert(&state.db, &row, &token)
        .await
        .expect("seed refresh token");

    let actor = admin_actor_for(admin_id);
    sui_id_core::admin::set_client_disabled(
        &state.db,
        &state.clock,
        &actor,
        client_id,
        true,
        Some("spam reports".into()),
        &state.caches,
    )
    .await
    .expect("disable client through the admin handler");

    let stored = sui_id_store::repos::clients::get(&state.db, client_id)
        .await
        .expect("get client");
    assert!(stored.is_disabled, "client must be disabled");

    let active: i64 = state
        .db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM refresh_tokens WHERE client_id = ?1 AND revoked_at IS NULL",
                [client_id.to_string()],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");
    assert_eq!(active, 0, "the refresh token must have been revoked");

    let note: Option<String> = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'client.disable' ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("audit row");
    let note = note.expect("note");
    assert_eq!(
        sui_id_store::registry::note_field(&note, "refresh_tokens_revoked").as_deref(),
        Some("1"),
        "the audit record must report the revoke, not just the client row"
    );
}
