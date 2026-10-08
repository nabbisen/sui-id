use super::*;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::TimeZone;

/// PKCS#1 DER, generated once for this module's fixtures; signs nothing
/// outside this file. Same key-material convention as `identity_claims`'s.
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

/// Signs a raw, caller-chosen payload string -- not a `Serialize` struct --
/// the same technique `identity_claims`'s tests use, for the same reason:
/// a malformed `iat` shape has to be put into the payload bytes directly.
fn sign_raw_payload(payload_json: &str) -> String {
    let header_json = format!(r#"{{"alg":"RS256","kid":"{KID}"}}"#);
    let encoded_header = Base64UrlUnpadded::encode_string(header_json.as_bytes());
    let encoded_payload = Base64UrlUnpadded::encode_string(payload_json.as_bytes());
    let message = format!("{encoded_header}.{encoded_payload}");
    let key = jsonwebtoken::EncodingKey::from_rsa_der(&der(RSA_DER));
    let signature =
        jsonwebtoken::crypto::sign(message.as_bytes(), &key, jsonwebtoken::Algorithm::RS256)
            .expect("signing with a freshly generated key must succeed");
    format!("{message}.{signature}")
}

/// Signs and runs `payload_json` through the real `verify_id_token_against_jwks`
/// pipeline -- the only way to obtain a `VerifiedIdTokenClaims`, since its
/// fields are private.
fn verified(payload_json: &str) -> VerifiedIdTokenClaims {
    let token = sign_raw_payload(payload_json);
    crate::id_token::verify_id_token_against_jwks(&token, &["RS256".to_string()], &jwks())
        .expect("a genuinely signed, structurally valid token must verify")
}

fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
}

/// A future `exp` far enough out that none of these tests' own clock
/// arithmetic ever brushes against it.
fn far_future_exp() -> i64 {
    jsonwebtoken::get_current_timestamp() as i64 + 10_000_000
}

#[test]
fn an_iat_within_both_bounds_is_accepted() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let now = at(2026, 1, 1, 12, 30, 0);
    let iat = created_at.timestamp();
    let p = format!(r#"{{"exp":{},"iat":{iat}}}"#, far_future_exp());
    assert!(validate_iat(&verified(&p), created_at, now).is_ok());
}

#[test]
fn a_missing_iat_is_refused() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let now = created_at;
    let p = format!(r#"{{"exp":{}}}"#, far_future_exp());
    assert_eq!(
        validate_iat(&verified(&p), created_at, now),
        Err(TimeClaimsError::Missing)
    );
}

#[test]
fn a_string_iat_is_refused() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let p = format!(
        r#"{{"exp":{},"iat":"{}"}}"#,
        far_future_exp(),
        created_at.timestamp()
    );
    assert_eq!(
        validate_iat(&verified(&p), created_at, created_at),
        Err(TimeClaimsError::WrongType)
    );
}

#[test]
fn a_float_iat_is_refused_even_with_a_zero_fraction() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let p = format!(
        r#"{{"exp":{},"iat":{:.1}}}"#,
        far_future_exp(),
        created_at.timestamp() as f64
    );
    assert_eq!(
        validate_iat(&verified(&p), created_at, created_at),
        Err(TimeClaimsError::NotAnInteger)
    );
}

#[test]
fn a_negative_iat_is_refused() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let p = format!(r#"{{"exp":{},"iat":-5}}"#, far_future_exp());
    assert_eq!(
        validate_iat(&verified(&p), created_at, created_at),
        Err(TimeClaimsError::Negative)
    );
}

/// `numeric_date` is shared with `id_token.rs`; its own
/// `an_exp_within_i64_but_past_chronos_range_is_refused` is what actually
/// exercises `chrono::DateTime::from_timestamp`'s range check specifically
/// -- `u64::MAX` here is rejected earlier, by `i64::try_from` failing.
#[test]
fn an_iat_past_u64_is_refused() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let p = format!(r#"{{"exp":{},"iat":{}}}"#, far_future_exp(), u64::MAX);
    assert_eq!(
        validate_iat(&verified(&p), created_at, created_at),
        Err(TimeClaimsError::OutOfRange)
    );
}

/// RFC 096 `:676`: the substitution defence. A token minted before this
/// attempt started -- even one whose signature, issuer, audience and
/// expiry are all otherwise perfect -- must be refused.
#[test]
fn an_iat_from_before_this_attempt_started_is_refused_as_a_substitution_attempt() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let now = created_at;
    // An hour before this attempt existed: a token from an earlier attempt.
    let old_iat = (created_at - chrono::Duration::hours(1)).timestamp();
    let p = format!(r#"{{"exp":{},"iat":{old_iat}}}"#, far_future_exp());
    assert_eq!(
        validate_iat(&verified(&p), created_at, now),
        Err(TimeClaimsError::TooOld)
    );
}

#[test]
fn iat_boundary_created_at_minus_60_is_accepted_minus_61_is_refused() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let now = created_at;

    let iat_60 = created_at.timestamp() - 60;
    let p60 = format!(r#"{{"exp":{},"iat":{iat_60}}}"#, far_future_exp());
    assert!(validate_iat(&verified(&p60), created_at, now).is_ok());

    let iat_61 = created_at.timestamp() - 61;
    let p61 = format!(r#"{{"exp":{},"iat":{iat_61}}}"#, far_future_exp());
    assert_eq!(
        validate_iat(&verified(&p61), created_at, now),
        Err(TimeClaimsError::TooOld)
    );
}

#[test]
fn an_iat_after_now_beyond_the_skew_is_refused_as_too_new() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let now = created_at;
    let future_iat = (now + chrono::Duration::hours(1)).timestamp();
    let p = format!(r#"{{"exp":{},"iat":{future_iat}}}"#, far_future_exp());
    assert_eq!(
        validate_iat(&verified(&p), created_at, now),
        Err(TimeClaimsError::TooNew)
    );
}

#[test]
fn iat_boundary_now_plus_60_is_accepted_plus_61_is_refused() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let now = created_at;

    let iat_60 = now.timestamp() + 60;
    let p60 = format!(r#"{{"exp":{},"iat":{iat_60}}}"#, far_future_exp());
    assert!(validate_iat(&verified(&p60), created_at, now).is_ok());

    let iat_61 = now.timestamp() + 61;
    let p61 = format!(r#"{{"exp":{},"iat":{iat_61}}}"#, far_future_exp());
    assert_eq!(
        validate_iat(&verified(&p61), created_at, now),
        Err(TimeClaimsError::TooNew)
    );
}

/// Duplicates are already handled -- stage 6a's `first_duplicate_member`
/// covers every top-level payload member, `iat` included, inside
/// `verify_id_token_against_jwks` itself, before a `VerifiedIdTokenClaims`
/// (and so before `validate_iat`) can even exist. This does not add a
/// second check; it asserts the existing one reaches `iat`.
#[test]
fn a_repeated_iat_member_is_refused_by_the_existing_stage_6a_scan() {
    let created_at = at(2026, 1, 1, 12, 0, 0);
    let iat = created_at.timestamp();
    let p = format!(
        r#"{{"exp":{},"iat":{iat},"iat":{}}}"#,
        far_future_exp(),
        iat + 1
    );
    let token = sign_raw_payload(&p);
    let result =
        crate::id_token::verify_id_token_against_jwks(&token, &["RS256".to_string()], &jwks());
    assert!(matches!(
        result,
        Err(crate::id_token::VerificationError::DuplicatePayloadMember(ref name)) if name == "iat"
    ));
}
