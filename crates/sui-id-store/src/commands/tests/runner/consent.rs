//! Cycle I stage 3 — U20 (revoke a consent grant and its refresh
//! tokens) and U21 (grant/update consent). Both were silent before this
//! stage: `user_consent::revoke_with_tokens` and `user_consent::upsert`
//! had no event at all.

use super::*;

async fn seed_user_and_client(db: &Database) -> (UserId, ClientId) {
    let user = a_user();
    repos::users::create(db, &user).await.expect("create user");
    let client = a_client();
    repos::clients::create(db, &client)
        .await
        .expect("create client");
    (user.id, client.id)
}

async fn seed_consent(db: &Database, user_id: UserId, client_id: ClientId, scopes: &str) {
    let scopes = scopes.to_owned();
    db.with_conn(move |conn| {
        repos::user_consent::upsert_within_tx(conn, user_id, client_id, &scopes, Utc::now())
            .map(|_| ())
    })
    .await
    .expect("seed consent");
}

async fn seed_refresh_token(db: &Database, user_id: UserId, client_id: ClientId) {
    let id = sui_id_shared::RefreshTokenId::generate();
    let row = crate::models::RefreshTokenRow {
        id: id.clone(),
        user_id,
        client_id,
        scope: "openid".into(),
        expires_at: Utc::now() + TimeDelta::hours(1),
        revoked_at: None,
        created_at: Utc::now(),
        auth_methods: vec![],
        family_id: sui_id_shared::FamilyId::root_of(&id),
    };
    let token = sui_id_shared::RawRefreshToken::from_untrusted(uuid::Uuid::new_v4().to_string());
    repos::refresh_tokens::insert(db, &row, &token)
        .await
        .expect("seed refresh token");
}

async fn refresh_token_count(db: &Database, user_id: UserId, client_id: ClientId) -> i64 {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = ?1 AND client_id = ?2",
            [user_id.to_string(), client_id.to_string()],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("count")
}

async fn last_event(db: &Database) -> crate::models::AuditLogRow {
    repos::audit::recent(db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row")
}

// ── U20 — revoke a consent grant and its refresh tokens ─────────────────

#[tokio::test]
async fn u20_revokes_the_grant_and_every_refresh_token_for_the_client() {
    let db = fresh_db();
    let (user_id, client_id) = seed_user_and_client(&db).await;
    seed_consent(&db, user_id, client_id, "openid profile").await;
    seed_refresh_token(&db, user_id, client_id).await;
    seed_refresh_token(&db, user_id, client_id).await;
    let before = audit_rows(&db).await;

    crate::commands::revoke_consent(&db, user_id, client_id)
        .await
        .expect("revoke consent")
        .into_inner();

    assert!(
        repos::user_consent::get(&db, user_id, client_id)
            .await
            .expect("get")
            .is_none(),
        "the grant must be gone"
    );
    assert_eq!(
        refresh_token_count(&db, user_id, client_id).await,
        0,
        "every refresh token for this client must be gone too"
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "auth.consent.revoked");
    assert_eq!(event.actor, Some(user_id));
    assert_eq!(event.note, None, "no attributes");
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("U20");
}

#[tokio::test]
async fn u20_with_no_refresh_tokens_still_revokes_the_grant() {
    let db = fresh_db();
    let (user_id, client_id) = seed_user_and_client(&db).await;
    seed_consent(&db, user_id, client_id, "openid").await;

    crate::commands::revoke_consent(&db, user_id, client_id)
        .await
        .expect("revoke consent, no refresh tokens held")
        .into_inner();

    assert!(
        repos::user_consent::get(&db, user_id, client_id)
            .await
            .expect("get")
            .is_none(),
        "zero refresh tokens is normal, not a reason to fail the revoke"
    );
}

#[tokio::test]
async fn u20_against_a_nonexistent_grant_is_notfound_and_writes_no_record() {
    let db = fresh_db();
    let (user_id, client_id) = seed_user_and_client(&db).await;
    let before = audit_rows(&db).await;

    let result = crate::commands::revoke_consent(&db, user_id, client_id).await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "there was no grant to revoke"
    );
    assert_eq!(
        audit_rows(&db).await,
        before,
        "must not write a record claiming a grant was revoked"
    );
}

#[tokio::test]
async fn u20_injected_failure_before_append_leaves_the_grant_and_tokens_live() {
    let db = fresh_db();
    let (user_id, client_id) = seed_user_and_client(&db).await;
    seed_consent(&db, user_id, client_id, "openid").await;
    seed_refresh_token(&db, user_id, client_id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::revoke_consent(&db, user_id, client_id).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        repos::user_consent::get(&db, user_id, client_id)
            .await
            .expect("get")
            .is_some(),
        "the revoke rolled back: the grant is still live"
    );
    assert_eq!(
        refresh_token_count(&db, user_id, client_id).await,
        1,
        "the revoke rolled back: the refresh token is still live"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit, "no event");
    record_rollback_coverage("U20");
}

// ── U21 — grant/update consent ───────────────────────────────────────────

#[tokio::test]
async fn u21_a_first_approval_is_granted_and_stores_the_scopes() {
    let db = fresh_db();
    let (user_id, client_id) = seed_user_and_client(&db).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::grant_consent(
        &db,
        user_id,
        client_id,
        "openid profile".into(),
        Utc::now(),
    )
    .await
    .expect("grant consent");
    assert_eq!(
        audited.into_inner(),
        repos::user_consent::ConsentGrantOutcome::Granted
    );

    let row = repos::user_consent::get(&db, user_id, client_id)
        .await
        .expect("get")
        .expect("grant exists");
    assert_eq!(row.granted_scopes, "openid profile");

    let event = last_event(&db).await;
    assert_eq!(event.action, "auth.consent.granted");
    assert_eq!(event.actor, Some(user_id));
    assert_eq!(event.note.as_deref(), Some("change=granted"));
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("U21");
}

#[tokio::test]
async fn u21_a_scope_change_to_an_existing_grant_is_recorded_as_updated() {
    let db = fresh_db();
    let (user_id, client_id) = seed_user_and_client(&db).await;
    seed_consent(&db, user_id, client_id, "openid").await;

    let audited = crate::commands::grant_consent(
        &db,
        user_id,
        client_id,
        "openid profile email".into(),
        Utc::now(),
    )
    .await
    .expect("update consent");
    assert_eq!(
        audited.into_inner(),
        repos::user_consent::ConsentGrantOutcome::Updated,
        "a grant already existed for this client"
    );

    let row = repos::user_consent::get(&db, user_id, client_id)
        .await
        .expect("get")
        .expect("grant exists");
    assert_eq!(row.granted_scopes, "openid profile email");

    let event = last_event(&db).await;
    assert_eq!(event.note.as_deref(), Some("change=updated"));
}

#[tokio::test]
async fn u21_injected_failure_before_append_leaves_no_grant_stored() {
    let db = fresh_db();
    let (user_id, client_id) = seed_user_and_client(&db).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result =
        crate::commands::grant_consent(&db, user_id, client_id, "openid".into(), Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        repos::user_consent::get(&db, user_id, client_id)
            .await
            .expect("get")
            .is_none(),
        "the grant rolled back: nothing was stored"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit, "no event");
    record_rollback_coverage("U21");
}
