use super::*;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::TimeZone;

use crate::cache_freshness::{ActivationGeneration, ProviderVersion};
use crate::id_token::VerifiedIdTokenClaims;
use crate::identity_claims::validate_identity_claims;
use crate::optional_claims::validate_optional_claims;
use sui_id_shared::ids::FederationProviderId;

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

fn verified(payload_json: &str) -> VerifiedIdTokenClaims {
    let token = sign_raw_payload(payload_json);
    crate::id_token::verify_id_token_against_jwks(&token, &["RS256".to_string()], &jwks())
        .expect("a genuinely signed, structurally valid token must verify")
}

fn provider() -> CacheKey {
    CacheKey::new(
        FederationProviderId::new(),
        ProviderVersion(3),
        ActivationGeneration(1),
    )
}

#[test]
fn the_capability_carries_every_field_it_was_built_from() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}","email":"user@example.com","email_verified":true,"preferred_username":"alice","name":"Alice Example"}}"#
    );
    let claims = verified(&p);
    let identity = validate_identity_claims(&claims, ISSUER, CLIENT_ID).expect("valid identity");
    let optional = validate_optional_claims(&claims).expect("valid optional claims");
    let key = provider();
    let when = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    let capability = construct_identity_capability(&identity, &optional, key, when);

    assert_eq!(capability.provider(), key);
    assert_eq!(capability.sub(), "user-1");
    assert_eq!(capability.verified_email(), Some("user@example.com"));
    assert_eq!(capability.preferred_username(), Some("alice"));
    assert_eq!(capability.name(), Some("Alice Example"));
    assert_eq!(capability.validated_at(), when);
}

#[test]
fn an_unverified_email_does_not_reach_the_capability() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}","email":"user@example.com","email_verified":false}}"#
    );
    let claims = verified(&p);
    let identity = validate_identity_claims(&claims, ISSUER, CLIENT_ID).expect("valid identity");
    let optional = validate_optional_claims(&claims).expect("valid optional claims");
    let when = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    let capability = construct_identity_capability(&identity, &optional, provider(), when);

    assert_eq!(capability.verified_email(), None);
}

/// RFC 096 `:707`: "sub" never appears in a log or metric label. The hand-
/// written `Debug` impl redacts it; this proves the redaction marker is
/// what prints, not the real value.
#[test]
fn debug_redacts_sub() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"a-very-distinctive-subject-value","aud":"{CLIENT_ID}"}}"#
    );
    let claims = verified(&p);
    let identity = validate_identity_claims(&claims, ISSUER, CLIENT_ID).expect("valid identity");
    let optional = validate_optional_claims(&claims).expect("valid optional claims");
    let when = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    let capability = construct_identity_capability(&identity, &optional, provider(), when);
    let debug = format!("{capability:?}");

    assert!(!debug.contains("a-very-distinctive-subject-value"));
}

/// Same clause, for `verified_email`.
#[test]
fn debug_redacts_verified_email() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}","email":"a-distinctive-mailbox@example.com","email_verified":true}}"#
    );
    let claims = verified(&p);
    let identity = validate_identity_claims(&claims, ISSUER, CLIENT_ID).expect("valid identity");
    let optional = validate_optional_claims(&claims).expect("valid optional claims");
    let when = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    let capability = construct_identity_capability(&identity, &optional, provider(), when);
    let debug = format!("{capability:?}");

    assert!(!debug.contains("a-distinctive-mailbox"));
}

/// The display hints are not named in RFC 096 `:707`'s forbidden list, so
/// they print plainly -- confirming the redaction above is selective, not
/// a blanket "hide everything" that would also mask a real bug in which
/// field holds what.
#[test]
fn debug_prints_the_display_hints_plainly() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}","preferred_username":"a-distinctive-username"}}"#
    );
    let claims = verified(&p);
    let identity = validate_identity_claims(&claims, ISSUER, CLIENT_ID).expect("valid identity");
    let optional = validate_optional_claims(&claims).expect("valid optional claims");
    let when = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    let capability = construct_identity_capability(&identity, &optional, provider(), when);
    let debug = format!("{capability:?}");

    assert!(debug.contains("a-distinctive-username"));
}

/// RFC 096 `:687-689`: "no raw token, nonce, or upstream access token".
/// `construct_identity_capability`'s parameter types already make this
/// structural -- see the module doc comment -- this is the runtime
/// demonstration against the real pipeline: a token carrying a distinctive
/// `nonce` is run all the way through, and the resulting capability's
/// `Debug` output is searched for it.
#[test]
fn the_capability_holds_no_trace_of_the_tokens_nonce() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}","nonce":"a-very-distinctive-nonce-value"}}"#
    );
    let claims = verified(&p);
    // The raw claims do carry the nonce -- confirms the fixture is
    // meaningful, not vacuously true because nothing ever held it.
    assert_eq!(claims.nonce(), Some("a-very-distinctive-nonce-value"));

    let identity = validate_identity_claims(&claims, ISSUER, CLIENT_ID).expect("valid identity");
    let optional = validate_optional_claims(&claims).expect("valid optional claims");
    let when = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    let capability = construct_identity_capability(&identity, &optional, provider(), when);
    let debug = format!("{capability:?}");

    assert!(!debug.contains("a-very-distinctive-nonce-value"));
}
