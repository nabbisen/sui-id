//! RFC 103 T10 (`0.93.0` O7 §0, defence in depth): `login_with_mfa`
//! refuses a user with a federation link, whatever `source` says. The
//! only way such a user can hold a password at all is the recovery-link
//! defect the store-side checks close — this test constructs the
//! password row directly, bypassing that path entirely, to prove the
//! backstop holds independent of it.

use super::*;
use crate::password::hash_password;
use crate::time::system_clock;
use chrono::Utc;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{
    CredentialRow, FederationLinkRow, FederationProviderRow, ProvisionMode,
};
use sui_id_store::repos::{credentials, federation_link, federation_provider, users};
use sui_id_store::{Database, models::UserSource};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

async fn make_local_user(db: &Database, username: &str) -> UserId {
    let id = UserId::new();
    let now = Utc::now();
    users::create(
        db,
        &sui_id_store::models::UserRow {
            id,
            username: username.into(),
            display_name: None,
            is_admin: false,
            role: sui_id_store::models::Role::User,
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: UserSource::Local,
            external_stable_id: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("user");
    id
}

async fn link_to_a_provider(db: &Database, user_id: UserId) {
    let now = Utc::now();
    let provider = FederationProviderRow {
        id: sui_id_shared::ids::FederationProviderId::new(),
        slug: "t10-login-provider".into(),
        display_name: "T10 Login Provider".into(),
        issuer: "https://idp.example".into(),
        client_id: "client".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::ProvisionOnFirstLogin,
        enabled: true,
        allowed_origins: String::new(),
        config_version: 0,
        activation_generation: 0,
        validation_status: sui_id_store::models::ValidationStatus::RequiresReview,
        evidence_config_version: None,
        evidence_activation_generation: None,
        evidence_policy_digest: None,
        evidence_observed_at: None,
        evidence_metadata_fingerprint: None,
        evidence_jwks_fingerprint: None,
        created_at: now,
        updated_at: now,
        id_token_algs: None,
    };
    let provider_id = provider.id;
    federation_provider::create_enabled_for_test(db, &provider, None)
        .await
        .expect("seed provider");
    federation_link::upsert(
        db,
        FederationLinkRow {
            user_id,
            provider_id,
            upstream_sub: "t10-login-sub".into(),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .expect("link");
}

#[tokio::test]
async fn password_sign_in_refuses_a_linked_federated_user_with_a_password_row() {
    let db = fresh_db();
    let clock = system_clock();
    let username = "t10-federated";
    let password = "a-long-enough-password-123";
    let user_id = make_local_user(&db, username).await;
    // Constructed directly: this is the exact shape the recovery-link
    // defect would have produced, built here without going through it,
    // so this test proves the backstop independent of that fix.
    credentials::upsert(
        &db,
        &CredentialRow {
            user_id,
            password_hash: hash_password(password).await.expect("hash"),
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("credential");
    link_to_a_provider(&db, user_id).await;

    let result = login_with_mfa(
        &db,
        &clock,
        username,
        password,
        24 * 60 * 60,
        SessionAudience::Any,
    )
    .await;
    assert!(
        matches!(result, Err(CoreError::InvalidCredentials)),
        "a linked federated user's own password is refused"
    );
}

/// The mirror case: the same user, same password, with the link removed
/// — ordinary password sign-in succeeds, so the refusal above is keyed
/// on the link, not on anything else about this fixture.
#[tokio::test]
async fn password_sign_in_succeeds_once_unlinked() {
    let db = fresh_db();
    let clock = system_clock();
    let username = "t10-unlinked";
    let password = "a-long-enough-password-123";
    let user_id = make_local_user(&db, username).await;
    credentials::upsert(
        &db,
        &CredentialRow {
            user_id,
            password_hash: hash_password(password).await.expect("hash"),
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("credential");

    let result = login_with_mfa(
        &db,
        &clock,
        username,
        password,
        24 * 60 * 60,
        SessionAudience::Any,
    )
    .await;
    assert!(
        matches!(result, Ok(LoginOutcome::SessionEstablished(_))),
        "an unlinked user's own password succeeds"
    );
}
