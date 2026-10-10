use super::*;
use base64ct::{Base64UrlUnpadded, Encoding};
use sha2::{Digest, Sha256};
use sui_id_shared::ids::FederationProviderId;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{FederationProviderRow, ProvisionMode};
use sui_id_store::repos::{federation_login_attempt, federation_provider};

// Duplicated from `nonce_claim/tests.rs` (and four other 096-A stages'
// test files before it) rather than shared -- the established convention
// in this project for this exact fixture, not an oversight.
const RSA_DER: &str = "MIIEogIBAAKCAQEArLOyRn5+sfSg9jNccpJTXiSiLtW/IJPvHZq81XO8v3U+gOPYVKDSNXy26GLAaZgNdB7KOIQsNnmw/OYtxlkBAV/vS0zRwLI5qwNIfnVp0SQMPkJgXSWbAkL/hVnmb6mgobevlke/2lBGEyO7RAnFfEpzATb666Avbf+BMK0+uL96OhgYKY5nqa6U3rohiMr+Fx8uOBdk2GbFn6IWXhsmK9+QVCY3YT40OP/WhjDtTSO04Cf66p+nib4mBqdOQvSokGhbYMnCEvEUkXWu4maOW0dB03CaNDmV86kovn024CSjnERgMHOsRdMtuFm0yQRpVlGRkFYX8OY5A/8gHmoIgwIDAQABAoIBAASKxkbX47TO7NRrwh+4FHuyaQVsyFGQ/jLK050uNqOD51D2Svjhwg0IirIIRx+OxKMDoLFQKWXrpeMZ71iY82TrGF8XhQdESLa5igOXjuDvSaxYUKFdMGX5/w0lBx1RtBKnbbnSeCEZED9HfDLuZe0G801NpMZjnOVUFN6l98zmYxpPVgZAG1BqzXCRquGOu/PszDZ07aDCC6zRZxCqmi+mYsAL0N4K2J8yy/HgV1kQ8lzilDKL0EPMNtyNzf+QBV7q6kl9Ob4uoHLHYwX1F9D4XoVhaXv/Uu0fTdG/qCiZC7woCmqjEf/AJiuJaaBEi2w1PychP9EKZxHtNQ/5qwUCgYEA25g/vCzxkQrJ6pMlHTwIQmGHMVRLM60MmJ+jdOUSc9gHIslz8T1gLc9RwwhPQyCYEiBIZ1qrUoFWQMfyixQk5WZ+ydN0icAoF0s0g0qKQNQDnG2RMD0R620KBzLEDzBKbmWB8U+Y49JKzGoV4Cx+kRrCOZR/ZVfzN3qOAmyT06UCgYEAyVVJEtnS5rZNkZwj4QgdpdpOX7+10l5trpE+d/mYhNeFRkhEUe8ibiyxiQb9lB9DZ4sXgMfAyHGc8bh+02gPPwlVV2jRqb6h7FO69tN+yVaKcY5NA72ERuJvL7oXQ2xIj5XMX5QrQKxJS055uGjj+a1iCjmschrozQUESh7iEwcCgYAnqmKo3P1tk6NRae7kTvm28+L1uCI1XWbPEtb1wIMKxdTUJct5ofqDi9VbA1894t9VNtudP7V+m7o2zWc0VBkuDsuMLVP5peoX+w+rP4WlnCZi1S/KpN1dxz5uem8Lx09Kja9hJV2amVvFfMwiyCa8kzbOK9KvPanDNbH9Ihu5uQKBgA4gE65k5e0V4T9UCxhgr2PReyowkxsdUOisfAuC0XaQgGM78r8k3e+I5zPL78KSpvH+yjlYymfFwNMctJk0dc1gZEJrsjoMi+O+xCFJGV4a2j+5UiHvC/bFMDPTBIrQcA7S3bHe/WHeNI46BUQw572+smAxR64BwU+RCIoCvK3FAoGAMaVCgqKomZMB1hasz+vflPG+W0FukaQ0f1pjAYSRDlahRdlYNVrKpB+dto+BXLgpRHNn0YoJFU7ZtMPl1XtmYawF4WPu5Alp7UT7OwXIuk2CZpbtL28pxuA6zWkKKrvBpqv8KEOSO8ah2pVfybCilFtw8O9DICaiGrj/G0UcF4E=";
const RSA_N: &str = "rLOyRn5-sfSg9jNccpJTXiSiLtW_IJPvHZq81XO8v3U-gOPYVKDSNXy26GLAaZgNdB7KOIQsNnmw_OYtxlkBAV_vS0zRwLI5qwNIfnVp0SQMPkJgXSWbAkL_hVnmb6mgobevlke_2lBGEyO7RAnFfEpzATb666Avbf-BMK0-uL96OhgYKY5nqa6U3rohiMr-Fx8uOBdk2GbFn6IWXhsmK9-QVCY3YT40OP_WhjDtTSO04Cf66p-nib4mBqdOQvSokGhbYMnCEvEUkXWu4maOW0dB03CaNDmV86kovn024CSjnERgMHOsRdMtuFm0yQRpVlGRkFYX8OY5A_8gHmoIgw";
const RSA_E: &str = "AQAB";
const KID: &str = "test-key";

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

fn verified_with_nonce(nonce: &str) -> VerifiedIdTokenClaims {
    let token = sign_raw_payload(&format!(r#"{{"nonce":"{nonce}"}}"#));
    crate::id_token::verify_id_token_against_jwks(&token, &["RS256".to_string()], &jwks())
        .expect("a genuinely signed, structurally valid token must verify")
}

fn sha256_bytes(s: &str) -> [u8; 32] {
    Sha256::digest(s.as_bytes()).into()
}

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

async fn seed_provider(db: &Database) -> FederationProviderId {
    let row = FederationProviderRow {
        id: FederationProviderId::new(),
        slug: "test-provider".to_owned(),
        display_name: "Test Provider".to_owned(),
        issuer: "https://idp.example".to_owned(),
        client_id: "client-1".to_owned(),
        client_secret_enc: None,
        scopes: "openid email".to_owned(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: true,
        allowed_origins: String::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let id = row.id;
    federation_provider::create(db, &row, None).await.unwrap();
    id
}

fn fixed_now() -> DateTime<Utc> {
    "2026-10-10T12:00:00Z".parse().unwrap()
}

async fn stored_status(db: &Database, id: sui_id_shared::ids::FederationLoginAttemptId) -> String {
    let id_str = id.to_string();
    db.with_read(move |read| {
        Ok(read
            .prepare("SELECT status FROM federation_login_attempt WHERE id = ?1")?
            .query_row([id_str], |r| r.get(0))?)
    })
    .await
    .unwrap()
}

async fn insert_attempt_with_nonce(
    db: &Database,
    provider_id: FederationProviderId,
    nonce: &str,
    now: DateTime<Utc>,
) -> FederationLoginAttemptRow {
    federation_login_attempt::insert(
        db,
        provider_id,
        1,
        1,
        [1u8; 32],
        sha256_bytes(nonce),
        [3u8; 32],
        b"the-pkce-verifier",
        "https://rp.example/cb".to_owned(),
        None,
        now,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn a_matching_nonce_claims_and_consumes() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt_with_nonce(&db, provider_id, "the-real-nonce", now).await;
    let claims = verified_with_nonce("the-real-nonce");

    let result =
        claim_and_consume_nonce(&db, attempt.id, &claims, now + chrono::Duration::seconds(1)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn a_mismatched_nonce_fails_consumption_but_the_claim_already_landed() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt_with_nonce(&db, provider_id, "the-real-nonce", now).await;
    let claims = verified_with_nonce("a-different-nonce");

    let result =
        claim_and_consume_nonce(&db, attempt.id, &claims, now + chrono::Duration::seconds(1)).await;
    assert!(matches!(result, Err(ClaimAndNonceError::Nonce(_))));

    // The module's own documented decision: a nonce mismatch does not roll
    // the claim back. The row must already be `exchanging`.
    assert_eq!(stored_status(&db, attempt.id).await, "exchanging");
}

#[tokio::test]
async fn a_claim_failure_is_reported_as_claim_not_nonce() {
    let db = open_test_db();
    let claims = verified_with_nonce("irrelevant");
    let result = claim_and_consume_nonce(
        &db,
        sui_id_shared::ids::FederationLoginAttemptId::new(),
        &claims,
        fixed_now(),
    )
    .await;
    assert!(matches!(
        result,
        Err(ClaimAndNonceError::Claim(StoreError::NotFound))
    ));
}

/// Stage 3, item 5's boundary decision: `hex_lower` must produce exactly
/// what `sui_id_core::tokens::sha256_hex` would produce for the same
/// nonce, so the two sides of `validate_nonce`'s comparison are never
/// case-mismatched.
#[test]
fn hex_lower_matches_sha256_hex() {
    let nonce = "some-nonce-value";
    let digest = sha256_bytes(nonce);
    assert_eq!(hex_lower(&digest), sui_id_core::tokens::sha256_hex(nonce));
}
