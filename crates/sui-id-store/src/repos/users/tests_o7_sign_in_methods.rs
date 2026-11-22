//! `0.93.0` O7 §1/§3: [`other_sign_in_methods`]'s own four signals,
//! each proven present and absent, and `except_provider`'s exclusion
//! proven against a user whose *only* link is the excepted provider.

use super::*;
use crate::models::{
    CredentialRow, FederationLinkRow, FederationProviderRow, ProvisionMode,
    UserWebauthnCredentialRow, ValidationStatus,
};
use crate::{Database, crypto::MasterKey};
use sui_id_shared::ids::{FederationProviderId, WebauthnCredentialId};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

async fn a_user(db: &Database, source: crate::models::UserSource) -> UserId {
    let id = UserId::new();
    let now = chrono::Utc::now();
    create(
        db,
        &UserRow {
            id,
            username: format!("o7-{id}"),
            display_name: None,
            is_admin: false,
            role: crate::models::Role::User,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
            last_login_at: None,
            source: source.clone(),
            external_stable_id: None,
        },
    )
    .await
    .expect("create user");
    if source != crate::models::UserSource::Local {
        db.with_conn(move |c| {
            c.execute(
                "UPDATE users SET source = ?1 WHERE id = ?2",
                rusqlite::params![source.as_str(), id.to_string()],
            )?;
            Ok(())
        })
        .await
        .expect("set source");
    }
    id
}

async fn a_provider(db: &Database, slug: &str) -> FederationProviderId {
    let now = chrono::Utc::now();
    let row = FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: slug.into(),
        issuer: "https://idp.example".into(),
        client_id: "client".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::ProvisionOnFirstLogin,
        enabled: true,
        allowed_origins: String::new(),
        config_version: 0,
        activation_generation: 0,
        validation_status: ValidationStatus::RequiresReview,
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
    let id = row.id;
    crate::repos::federation_provider::create_enabled_for_test(db, &row, None)
        .await
        .expect("seed provider");
    id
}

async fn link(db: &Database, user_id: UserId, provider_id: FederationProviderId, sub: &str) {
    let now = chrono::Utc::now();
    crate::repos::federation_link::upsert(
        db,
        FederationLinkRow {
            user_id,
            provider_id,
            upstream_sub: sub.into(),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .expect("link");
}

#[tokio::test]
async fn nothing_present_is_all_false() {
    let db = fresh_db();
    let user = a_user(&db, crate::models::UserSource::Local).await;
    let provider = a_provider(&db, "o7-nothing").await;
    let m = other_sign_in_methods(&db, user, provider)
        .await
        .expect("read");
    assert_eq!(m, SignInMethods::default());
    assert!(!m.any());
}

#[tokio::test]
async fn password_only() {
    let db = fresh_db();
    let user = a_user(&db, crate::models::UserSource::Local).await;
    let provider = a_provider(&db, "o7-password").await;
    crate::repos::credentials::upsert(
        &db,
        &CredentialRow {
            user_id: user,
            password_hash: "hash-placeholder".into(),
            updated_at: chrono::Utc::now(),
        },
    )
    .await
    .expect("credential");
    let m = other_sign_in_methods(&db, user, provider)
        .await
        .expect("read");
    assert_eq!(
        m,
        SignInMethods {
            password: true,
            ..Default::default()
        }
    );
    assert!(m.any());
}

#[tokio::test]
async fn passkey_only() {
    let db = fresh_db();
    let user = a_user(&db, crate::models::UserSource::Local).await;
    let provider = a_provider(&db, "o7-passkey").await;
    crate::repos::user_webauthn_credentials::create(
        &db,
        &UserWebauthnCredentialRow {
            id: WebauthnCredentialId::new(),
            user_id: user,
            credential_id: vec![1u8, 2, 3],
            passkey_enc: Vec::new(),
            nickname: "a passkey".into(),
            created_at: chrono::Utc::now(),
            last_used_at: None,
        },
        b"passkey-json-placeholder",
    )
    .await
    .expect("seed passkey");
    let m = other_sign_in_methods(&db, user, provider)
        .await
        .expect("read");
    assert_eq!(
        m,
        SignInMethods {
            passkeys: true,
            ..Default::default()
        }
    );
}

#[tokio::test]
async fn directory_only() {
    let db = fresh_db();
    let user = a_user(&db, crate::models::UserSource::Ldap).await;
    let provider = a_provider(&db, "o7-directory").await;
    let m = other_sign_in_methods(&db, user, provider)
        .await
        .expect("read");
    assert_eq!(
        m,
        SignInMethods {
            directory: true,
            ..Default::default()
        }
    );
}

/// `except_provider` really excludes the link being removed (`0.93.0`
/// O7 §3): a user whose *only* link is this provider has `other_links
/// == 0`, and a second link to a different provider still counts.
#[tokio::test]
async fn except_provider_excludes_only_that_link() {
    let db = fresh_db();
    let user = a_user(&db, crate::models::UserSource::Local).await;
    let being_unlinked = a_provider(&db, "o7-being-unlinked").await;
    link(&db, user, being_unlinked, "sub-a").await;

    let only_this_link = other_sign_in_methods(&db, user, being_unlinked)
        .await
        .expect("read");
    assert_eq!(
        only_this_link,
        SignInMethods::default(),
        "the only link is the one being removed: other_links == 0"
    );

    let other_provider = a_provider(&db, "o7-another").await;
    link(&db, user, other_provider, "sub-b").await;
    let with_another_link = other_sign_in_methods(&db, user, being_unlinked)
        .await
        .expect("read");
    assert_eq!(
        with_another_link,
        SignInMethods {
            other_links: true,
            ..Default::default()
        },
        "a second, different link still counts"
    );
}

#[tokio::test]
async fn unknown_user_is_not_found() {
    let db = fresh_db();
    let provider = a_provider(&db, "o7-unknown").await;
    let result = other_sign_in_methods(&db, UserId::new(), provider).await;
    assert!(matches!(result, Err(StoreError::NotFound)));
}
