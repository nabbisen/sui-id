use super::*;
use base64ct::{Base64UrlUnpadded, Encoding};

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
    let payload_json = if payload_json == "{}" {
        r#"{"exp":9999999999}"#.to_string()
    } else {
        payload_json.replacen('{', r#"{"exp":9999999999,"#, 1)
    };
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

/// A real digest, not a hand-picked-looking hex string -- `sha256_hex` of
/// the literal `"the-attempts-nonce"`.
fn digest_of(nonce: &str) -> String {
    sui_id_core::tokens::sha256_hex(nonce)
}

#[test]
fn an_absent_nonce_is_refused() {
    let claims = verified("{}");
    let expected = digest_of("anything");
    assert_eq!(validate_nonce(&claims, &expected), Err(NonceError::Missing));
}

#[test]
fn a_non_string_nonce_is_refused() {
    let p = r#"{"nonce":5}"#;
    let claims = verified(p);
    let expected = digest_of("anything");
    assert_eq!(
        validate_nonce(&claims, &expected),
        Err(NonceError::WrongType)
    );
}

#[test]
fn a_matching_nonce_digest_is_accepted() {
    let nonce = "the-attempts-nonce";
    let p = format!(r#"{{"nonce":"{nonce}"}}"#);
    let claims = verified(&p);
    let expected = digest_of(nonce);
    assert_eq!(validate_nonce(&claims, &expected), Ok(()));
}

#[test]
fn a_mismatched_nonce_digest_is_refused() {
    let p = r#"{"nonce":"the-attempts-nonce"}"#;
    let claims = verified(p);
    let expected = digest_of("a-different-nonce-entirely");
    assert_eq!(
        validate_nonce(&claims, &expected),
        Err(NonceError::Mismatch)
    );
}

#[test]
fn an_expected_digest_that_is_too_short_is_refused_as_malformed() {
    let p = r#"{"nonce":"the-attempts-nonce"}"#;
    let claims = verified(p);
    let short = digest_of("the-attempts-nonce");
    let short = &short[..63];
    assert_eq!(
        validate_nonce(&claims, short),
        Err(NonceError::ExpectedDigestMalformed)
    );
}

#[test]
fn an_expected_digest_with_a_non_hex_character_is_refused_as_malformed() {
    let p = r#"{"nonce":"the-attempts-nonce"}"#;
    let claims = verified(p);
    // 64 characters, one of them not hex.
    let bad = "g".repeat(64);
    assert_eq!(
        validate_nonce(&claims, &bad),
        Err(NonceError::ExpectedDigestMalformed)
    );
}

/// The malformed-expected-digest precondition is checked before the token's
/// own `nonce` claim is even read -- confirmed by using a payload with no
/// `nonce` claim at all and still getting the malformed-digest error, not
/// `Missing`.
#[test]
fn a_malformed_expected_digest_is_caught_even_when_the_token_has_no_nonce_at_all() {
    let claims = verified("{}");
    let bad = "too-short";
    assert_eq!(
        validate_nonce(&claims, bad),
        Err(NonceError::ExpectedDigestMalformed)
    );
}

/// Stage 6a's `first_duplicate_member` scan covers every top-level member,
/// before a `VerifiedIdTokenClaims` can exist at all -- a repeated `nonce`
/// never reaches this module in the first place. No second check is added
/// here; this asserts the existing one covers `nonce` specifically.
#[test]
fn a_repeated_nonce_member_is_refused_by_the_existing_stage_6a_scan() {
    let header_json = format!(r#"{{"alg":"RS256","kid":"{KID}"}}"#);
    let payload_json = r#"{"exp":9999999999,"nonce":"a","nonce":"b"}"#;
    let encoded_header = Base64UrlUnpadded::encode_string(header_json.as_bytes());
    let encoded_payload = Base64UrlUnpadded::encode_string(payload_json.as_bytes());
    let message = format!("{encoded_header}.{encoded_payload}");
    let key = jsonwebtoken::EncodingKey::from_rsa_der(&der(RSA_DER));
    let signature =
        jsonwebtoken::crypto::sign(message.as_bytes(), &key, jsonwebtoken::Algorithm::RS256)
            .expect("signing with a freshly generated key must succeed");
    let token = format!("{message}.{signature}");

    let result =
        crate::id_token::verify_id_token_against_jwks(&token, &["RS256".to_string()], &jwks());
    assert!(matches!(
        result,
        Err(crate::id_token::VerificationError::DuplicatePayloadMember(ref name)) if name == "nonce"
    ));
}
