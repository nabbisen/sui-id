use super::*;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Utc;

use crate::cache_freshness::{ActivationGeneration, CacheKey, ProviderVersion};
use crate::identity_capability::construct_identity_capability;
use crate::identity_claims::validate_identity_claims;
use crate::optional_claims::validate_optional_claims;
use sui_id_shared::ids::FederationProviderId;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{FederationLinkRow, FederationProviderRow, Role, UserRow, UserSource};
use sui_id_store::repos::{federation_link, federation_provider, users};

// Same throwaway RSA fixture key already committed at
// `identity_capability/tests.rs` and four other 096-A test files.
const RSA_DER: &str = "MIIEogIBAAKCAQEArLOyRn5+sfSg9jNccpJTXiSiLtW/IJPvHZq81XO8v3U+gOPYVKDSNXy26GLAaZgNdB7KOIQsNnmw/OYtxlkBAV/vS0zRwLI5qwNIfnVp0SQMPkJgXSWbAkL/hVnmb6mgobevlke/2lBGEyO7RAnFfEpzATb666Avbf+BMK0+uL96OhgYKY5nqa6U3rohiMr+Fx8uOBdk2GbFn6IWXhsmK9+QVCY3YT40OP/WhjDtTSO04Cf66p+nib4mBqdOQvSokGhbYMnCEvEUkXWu4maOW0dB03CaNDmV86kovn024CSjnERgMHOsRdMtuFm0yQRpVlGRkFYX8OY5A/8gHmoIgwIDAQABAoIBAASKxkbX47TO7NRrwh+4FHuyaQVsyFGQ/jLK050uNqOD51D2Svjhwg0IirIIRx+OxKMDoLFQKWXrpeMZ71iY82TrGF8XhQdESLa5igOXjuDvSaxYUKFdMGX5/w0lBx1RtBKnbbnSeCEZED9HfDLuZe0G801NpMZjnOVUFN6l98zmYxpPVgZAG1BqzXCRquGOu/PszDZ07aDCC6zRZxCqmi+mYsAL0N4K2J8yy/HgV1kQ8lzilDKL0EPMNtyNzf+QBV7q6kl9Ob4uoHLHYwX1F9D4XoVhaXv/Uu0fTdG/qCiZC7woCmqjEf/AJiuJaaBEi2w1PychP9EKZxHtNQ/5qwUCgYEA25g/vCzxkQrJ6pMlHTwIQmGHMVRLM60MmJ+jdOUSc9gHIslz8T1gLc9RwwhPQyCYEiBIZ1qrUoFWQMfyixQk5WZ+ydN0icAoF0s0g0qKQNQDnG2RMD0R620KBzLEDzBKbmWB8U+Y49JKzGoV4Cx+kRrCOZR/ZVfzN3qOAmyT06UCgYEAyVVJEtnS5rZNkZwj4QgdpdpOX7+10l5trpE+d/mYhNeFRkhEUe8ibiyxiQb9lB9DZ4sXgMfAyHGc8bh+02gPPwlVV2jRqb6h7FO69tN+yVaKcY5NA72ERuJvL7oXQ2xIj5XMX5QrQKxJS055uGjj+a1iCjmschrozQUESh7iEwcCgYAnqmKo3P1tk6NRae7kTvm28+L1uCI1XWbPEtb1wIMKxdTUJct5ofqDi9VbA1894t9VNtudP7V+m7o2zWc0VBkuDsuMLVP5peoX+w+rP4WlnCZi1S/KpN1dxz5uem8Lx09Kja9hJV2amVvFfMwiyCa8kzbOK9KvPanDNbH9Ihu5uQKBgA4gE65k5e0V4T9UCxhgr2PReyowkxsdUOisfAuC0XaQgGM78r8k3e+I5zPL78KSpvH+yjlYymfFwNMctJk0dc1gZEJrsjoMi+O+xCFJGV4a2j+5UiHvC/bFMDPTBIrQcA7S3bHe/WHeNI46BUQw572+smAxR64BwU+RCIoCvK3FAoGAMaVCgqKomZMB1hasz+vflPG+W0FukaQ0f1pjAYSRDlahRdlYNVrKpB+dto+BXLgpRHNn0YoJFU7ZtMPl1XtmYawF4WPu5Alp7UT7OwXIuk2CZpbtL28pxuA6zWkKKrvBpqv8KEOSO8ah2pVfybCilFtw8O9DICaiGrj/G0UcF4E=";
const RSA_N: &str = "rLOyRn5-sfSg9jNccpJTXiSiLtW_IJPvHZq81XO8v3U-gOPYVKDSNXy26GLAaZgNdB7KOIQsNnmw_OYtxlkBAV_vS0zRwLI5qwNIfnVp0SQMPkJgXSWbAkL_hVnmb6mgobevlke_2lBGEyO7RAnFfEpzATb666Avbf-BMK0-uL96OhgYKY5nqa6U3rohiMr-Fx8uOBdk2GbFn6IWXhsmK9-QVCY3YT40OP_WhjDtTSO04Cf66p-nib4mBqdOQvSokGhbYMnCEvEUkXWu4maOW0dB03CaNDmV86kovn024CSjnERgMHOsRdMtuFm0yQRpVlGRkFYX8OY5A_8gHmoIgw";
const RSA_E: &str = "AQAB";
const KID: &str = "test-key";
const ISSUER: &str = "https://idp.example.com";
const CLIENT_ID: &str = "this-rp-client-id";

fn der(b64_std: &str) -> Vec<u8> {
    base64ct::Base64::decode_vec(b64_std).expect("valid standard base64 DER")
}

fn jwks() -> crate::jwks::Jwks {
    crate::jwks::Jwks {
        keys: vec![serde_json::json!({"kty": "RSA", "kid": KID, "n": RSA_N, "e": RSA_E})],
    }
}

fn sign_raw_payload(payload_json: &str) -> String {
    let header_json = format!(r#"{{"alg":"RS256","kid":"{KID}"}}"#);
    let payload_json = payload_json.replacen('{', r#"{"exp":9999999999,"#, 1);
    let encoded_header = Base64UrlUnpadded::encode_string(header_json.as_bytes());
    let encoded_payload = Base64UrlUnpadded::encode_string(payload_json.as_bytes());
    let message = format!("{encoded_header}.{encoded_payload}");
    let key = jsonwebtoken::EncodingKey::from_rsa_der(&der(RSA_DER));
    let signature =
        jsonwebtoken::crypto::sign(message.as_bytes(), &key, jsonwebtoken::Algorithm::RS256)
            .expect("signing with a freshly generated key must succeed");
    format!("{message}.{signature}")
}

/// Builds a real, sealed `IdentityCapability` the same way stage 4's own
/// pipeline would, for `provider_id` and the given `sub`/email shape — not
/// a hand-built stand-in, since the type has no public constructor other
/// than the real validation chain.
fn capability_for(
    provider_id: FederationProviderId,
    sub: &str,
    email_json: &str,
) -> IdentityCapability {
    let payload = format!(r#"{{"iss":"{ISSUER}","sub":"{sub}","aud":"{CLIENT_ID}"{email_json}}}"#);
    let token = sign_raw_payload(&payload);
    let claims =
        crate::id_token::verify_id_token_against_jwks(&token, &["RS256".to_string()], &jwks())
            .expect("a genuinely signed, structurally valid token must verify");
    let identity = validate_identity_claims(&claims, ISSUER, CLIENT_ID).expect("valid identity");
    let optional = validate_optional_claims(&claims).expect("valid optional claims");
    let key = CacheKey::new(provider_id, ProviderVersion(1), ActivationGeneration(1));
    construct_identity_capability(&identity, &optional, key, Utc::now())
}

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

async fn seed_provider(db: &Database, provision_mode: ProvisionMode) -> FederationProviderId {
    let row = FederationProviderRow {
        id: FederationProviderId::new(),
        slug: "test-provider".to_owned(),
        display_name: "Test Provider".to_owned(),
        issuer: ISSUER.to_owned(),
        client_id: CLIENT_ID.to_owned(),
        client_secret_enc: None,
        scopes: "openid email".to_owned(),
        provision_mode,
        enabled: true,
        allowed_origins: String::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let id = row.id;
    federation_provider::create(db, &row, None).await.unwrap();
    id
}

async fn seed_local_user(db: &Database, email: &str) -> sui_id_shared::ids::UserId {
    let now = Utc::now();
    let row = UserRow {
        id: sui_id_shared::ids::UserId::new(),
        username: "existing-local-user".into(),
        display_name: None,
        is_admin: false,
        role: Role::User,
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
        email: Some(email.to_owned()),
        preferred_lang: None,
        email_normalized: None,
        email_verified_at: None,
    };
    let id = row.id;
    users::create(db, &row).await.unwrap();
    id
}

async fn seed_link(
    db: &Database,
    provider_id: FederationProviderId,
    sub: &str,
    user_id: sui_id_shared::ids::UserId,
) {
    let now = Utc::now();
    federation_link::upsert(
        db,
        FederationLinkRow {
            user_id,
            provider_id,
            upstream_sub: sub.to_owned(),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn an_existing_link_resolves_without_needing_email() {
    let db = open_test_db();
    let provider_id = seed_provider(&db, ProvisionMode::LinkOnly).await;
    let user_id = seed_local_user(&db, "owner@example.com").await;
    seed_link(&db, provider_id, "returning-sub", user_id).await;
    // No email claim at all -- RFC 096 :695-696: an existing link
    // authenticates without it.
    let capability = capability_for(provider_id, "returning-sub", "");

    let resolution = resolve_verified_identity(&db, &capability, ProvisionMode::LinkOnly)
        .await
        .unwrap();

    assert_eq!(resolution, IdentityResolution::ExistingLink { user_id });
}

#[tokio::test]
async fn link_only_with_no_link_and_no_collision_requires_a_link() {
    let db = open_test_db();
    let provider_id = seed_provider(&db, ProvisionMode::LinkOnly).await;
    let capability = capability_for(
        provider_id,
        "new-sub",
        r#","email":"never-seen@example.com","email_verified":true"#,
    );

    let resolution = resolve_verified_identity(&db, &capability, ProvisionMode::LinkOnly)
        .await
        .unwrap();

    assert_eq!(resolution, IdentityResolution::LinkRequired);
}

/// The generic-result property (dispatch item 4): `link_only`'s result for
/// an unlinked identity must be the *same* state whether or not its email
/// would collide with a local account -- collision denial is specific to
/// `provision_on_first_login` (RFC 096 `:699`, `:707-708`). This test
/// drives the exact scenario that would be `DeniedCollision` under the
/// other mode and confirms `link_only` still returns `LinkRequired`.
#[tokio::test]
async fn link_only_returns_the_same_generic_result_even_when_the_email_would_collide() {
    let db = open_test_db();
    let provider_id = seed_provider(&db, ProvisionMode::LinkOnly).await;
    seed_local_user(&db, "collides@example.com").await;
    let colliding = capability_for(
        provider_id,
        "sub-a",
        r#","email":"collides@example.com","email_verified":true"#,
    );
    let non_colliding = capability_for(
        provider_id,
        "sub-b",
        r#","email":"never-seen@example.com","email_verified":true"#,
    );

    let a = resolve_verified_identity(&db, &colliding, ProvisionMode::LinkOnly)
        .await
        .unwrap();
    let b = resolve_verified_identity(&db, &non_colliding, ProvisionMode::LinkOnly)
        .await
        .unwrap();

    assert_eq!(a, IdentityResolution::LinkRequired);
    assert_eq!(b, IdentityResolution::LinkRequired);
    assert_eq!(
        a, b,
        "link_only must not distinguish a collision from any other unlinked identity"
    );
}

#[tokio::test]
async fn provision_on_first_login_with_no_collision_is_eligible() {
    let db = open_test_db();
    let provider_id = seed_provider(&db, ProvisionMode::ProvisionOnFirstLogin).await;
    let capability = capability_for(
        provider_id,
        "new-sub",
        r#","email":"never-seen@example.com","email_verified":true"#,
    );

    let resolution =
        resolve_verified_identity(&db, &capability, ProvisionMode::ProvisionOnFirstLogin)
            .await
            .unwrap();

    assert_eq!(resolution, IdentityResolution::ProvisionEligible);
}

/// The takeover-collision test (dispatch item 3): a local user already
/// holds the normalized email, and the result is a denial that does
/// *not* auto-link -- no `user_id`, no link row, just the state.
///
/// **Also item 6's real proof, not a separate assertion on
/// `normalize_email` in isolation.** The seeded user's email and the
/// resolved capability's email differ in case; `resolve_verified_identity`
/// still reports `DeniedCollision`, which only happens if the collision
/// check normalizes with the same function the insert path used. It holds
/// doubly: `users::create` (`repos/users.rs:97`) recomputes
/// `email_normalized` from the stored value with `sui_id_shared::
/// normalize_email`, the same function `resolve_verified_identity` calls
/// on the claimed email, so this test pins insert-side and lookup-side
/// normalization agreeing, not merely that the function is
/// case-insensitive on its own.
#[tokio::test]
async fn provision_on_first_login_denies_as_collision_and_does_not_auto_link() {
    let db = open_test_db();
    let provider_id = seed_provider(&db, ProvisionMode::ProvisionOnFirstLogin).await;
    let existing_user = seed_local_user(&db, "Collides@Example.com").await;
    // Different case than the stored value, to prove this goes through
    // normalization rather than an exact-string match.
    let capability = capability_for(
        provider_id,
        "new-sub",
        r#","email":"collides@example.com","email_verified":true"#,
    );

    let resolution =
        resolve_verified_identity(&db, &capability, ProvisionMode::ProvisionOnFirstLogin)
            .await
            .unwrap();

    assert_eq!(resolution, IdentityResolution::DeniedCollision);
    // No link was created as a side effect of resolving.
    let link = federation_link::find_by_sub(&db, provider_id, "new-sub")
        .await
        .unwrap();
    assert!(link.is_none(), "resolution must not auto-link");
    let _ = existing_user;
}

#[tokio::test]
async fn provision_on_first_login_with_no_email_claim_is_denied_unverified() {
    let db = open_test_db();
    let provider_id = seed_provider(&db, ProvisionMode::ProvisionOnFirstLogin).await;
    let capability = capability_for(provider_id, "new-sub", "");

    let resolution =
        resolve_verified_identity(&db, &capability, ProvisionMode::ProvisionOnFirstLogin)
            .await
            .unwrap();

    assert_eq!(resolution, IdentityResolution::DeniedUnverified);
}

#[tokio::test]
async fn provision_on_first_login_with_an_unverified_email_is_denied_unverified() {
    let db = open_test_db();
    let provider_id = seed_provider(&db, ProvisionMode::ProvisionOnFirstLogin).await;
    let capability = capability_for(
        provider_id,
        "new-sub",
        r#","email":"present@example.com","email_verified":false"#,
    );

    let resolution =
        resolve_verified_identity(&db, &capability, ProvisionMode::ProvisionOnFirstLogin)
            .await
            .unwrap();

    assert_eq!(resolution, IdentityResolution::DeniedUnverified);
}
