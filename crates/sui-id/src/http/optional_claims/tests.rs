use super::*;
use base64ct::{Base64UrlUnpadded, Encoding};

/// Same key material and signing technique as `identity_claims`'s and
/// `time_claims`'s test modules -- a local copy per module, not a shared
/// helper, matching the project's established convention.
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

/// Injects a far-future `exp` the same way `identity_claims`'s and
/// `time_claims`'s helpers do, so every payload literal below can omit it.
fn sign_raw_payload(payload_json: &str) -> String {
    let header_json = format!(r#"{{"alg":"RS256","kid":"{KID}"}}"#);
    // An empty object needs no trailing comma; every other payload here
    // opens with at least one member already, same as `identity_claims`'s
    // and `time_claims`'s copies of this helper assume.
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

// ---- email -------------------------------------------------------------

#[test]
fn an_absent_email_is_not_an_error() {
    assert_eq!(validate_email(&verified("{}")), Ok(None));
}

#[test]
fn a_non_string_email_is_refused() {
    let p = r#"{"email":5}"#;
    assert_eq!(
        validate_email(&verified(p)),
        Err(OptionalClaimsError::EmailWrongType)
    );
}

#[test]
fn an_email_over_254_bytes_is_refused() {
    let local = "a".repeat(250);
    let p = format!(r#"{{"email":"{local}@example.com"}}"#);
    assert!(local.len() + "@example.com".len() > 254);
    assert_eq!(
        validate_email(&verified(&p)),
        Err(OptionalClaimsError::EmailTooLong)
    );
}

#[test]
fn an_email_with_no_at_sign_is_refused() {
    let p = r#"{"email":"not-an-email"}"#;
    assert_eq!(
        validate_email(&verified(p)),
        Err(OptionalClaimsError::EmailNotMailboxShaped)
    );
}

#[test]
fn an_email_with_two_at_signs_is_refused() {
    let p = r#"{"email":"a@b@example.com"}"#;
    assert_eq!(
        validate_email(&verified(p)),
        Err(OptionalClaimsError::EmailNotMailboxShaped)
    );
}

/// Stage 6c-fix, ruled by the architect: a dotless domain is accepted,
/// deliberately -- `email` is metadata only and never a lookup key, so
/// refusing it would fail a real login (`user@intranet`, `user@localhost`
/// from an enterprise or internal IdP) over a claim that grants no
/// authority. See `is_mailbox_shaped`'s doc comment.
#[test]
fn an_email_with_a_dotless_domain_is_accepted() {
    let p = r#"{"email":"user@localhost"}"#;
    assert_eq!(
        validate_email(&verified(p)),
        Ok(Some("user@localhost".to_string()))
    );
}

#[test]
fn an_email_with_a_leading_dot_in_the_domain_is_refused() {
    let p = r#"{"email":"user@.example.com"}"#;
    assert_eq!(
        validate_email(&verified(p)),
        Err(OptionalClaimsError::EmailNotMailboxShaped)
    );
}

#[test]
fn an_email_with_consecutive_dots_in_the_domain_is_refused() {
    let p = r#"{"email":"user@a..b"}"#;
    assert_eq!(
        validate_email(&verified(p)),
        Err(OptionalClaimsError::EmailNotMailboxShaped)
    );
}

#[test]
fn a_well_formed_email_is_accepted() {
    let p = r#"{"email":"user@example.com"}"#;
    assert_eq!(
        validate_email(&verified(p)),
        Ok(Some("user@example.com".to_string()))
    );
}

/// Stage 6c-fix, item 1: the rejecting-side tests alone cannot detect a
/// bound that is one too tight (the architect's own mutation -- 254 → 253
/// -- killed nothing). This is the accepting side, at exactly the limit:
/// 242 `a`s + `@` + `example.com` (11 bytes) = 254 bytes exactly.
#[test]
fn an_email_of_exactly_254_bytes_is_accepted() {
    let local = "a".repeat(242);
    let email = format!("{local}@example.com");
    assert_eq!(email.len(), 254);
    let p = format!(r#"{{"email":"{email}"}}"#);
    assert_eq!(validate_email(&verified(&p)), Ok(Some(email)));
}

// ---- email_verified ------------------------------------------------------

#[test]
fn an_absent_email_verified_is_false_not_an_error() {
    assert_eq!(validate_email_verified(&verified("{}")), Ok(false));
}

#[test]
fn an_email_verified_of_true_is_accepted() {
    let p = r#"{"email_verified":true}"#;
    assert_eq!(validate_email_verified(&verified(p)), Ok(true));
}

#[test]
fn an_email_verified_of_false_is_accepted() {
    let p = r#"{"email_verified":false}"#;
    assert_eq!(validate_email_verified(&verified(p)), Ok(false));
}

#[test]
fn a_string_email_verified_is_refused_not_treated_as_falsy() {
    let p = r#"{"email_verified":"true"}"#;
    assert_eq!(
        validate_email_verified(&verified(p)),
        Err(OptionalClaimsError::EmailVerifiedWrongType)
    );
}

#[test]
fn a_numeric_email_verified_is_refused() {
    let p = r#"{"email_verified":1}"#;
    assert_eq!(
        validate_email_verified(&verified(p)),
        Err(OptionalClaimsError::EmailVerifiedWrongType)
    );
}

// ---- preferred_username --------------------------------------------------

#[test]
fn an_absent_preferred_username_is_not_an_error() {
    assert_eq!(validate_preferred_username(&verified("{}")), Ok(None));
}

#[test]
fn a_non_string_preferred_username_is_refused() {
    let p = r#"{"preferred_username":5}"#;
    assert_eq!(
        validate_preferred_username(&verified(p)),
        Err(OptionalClaimsError::PreferredUsernameWrongType)
    );
}

/// 129 ASCII scalars: 129 bytes (under the 512-byte bound), 129 scalars
/// (over the 128-scalar bound) -- the only direction in which a bound can
/// fail alone, for the reason `validate_bounded_display_string`'s doc
/// comment proves: `max_bytes == 4 * max_scalars` here, so a byte-bound
/// violation is mathematically impossible without the scalar bound already
/// having been violated too.
#[test]
fn a_preferred_username_with_129_ascii_scalars_fails_the_scalar_bound_only() {
    let s = "a".repeat(129);
    assert_eq!(s.len(), 129);
    let p = format!(r#"{{"preferred_username":"{s}"}}"#);
    assert_eq!(
        validate_preferred_username(&verified(&p)),
        Err(OptionalClaimsError::PreferredUsernameTooManyScalars)
    );
}

/// 200 three-byte scalars: 600 bytes (over 512) and 200 scalars (also over
/// 128) -- both bounds are violated, and the byte check firing first is
/// what proves it is checked before the scalar check, not merely present.
#[test]
fn a_preferred_username_with_200_three_byte_scalars_fails_the_byte_bound_first() {
    let s = "\u{4e2d}".repeat(200);
    assert_eq!(s.len(), 600);
    assert_eq!(s.chars().count(), 200);
    let p = format!(r#"{{"preferred_username":"{s}"}}"#);
    assert_eq!(
        validate_preferred_username(&verified(&p)),
        Err(OptionalClaimsError::PreferredUsernameTooManyBytes)
    );
}

#[test]
fn a_preferred_username_with_a_control_character_is_refused() {
    // `\u0007` is a literal JSON escape here, not an embedded raw byte --
    // JSON forbids an unescaped control character inside a string.
    let p = r#"{"preferred_username":"a\u0007b"}"#;
    assert_eq!(
        validate_preferred_username(&verified(p)),
        Err(OptionalClaimsError::PreferredUsernameHasControlOrBidi)
    );
}

#[test]
fn a_preferred_username_with_a_bidi_override_is_refused() {
    let p = "{\"preferred_username\":\"a\u{202E}b\"}";
    assert_eq!(
        validate_preferred_username(&verified(p)),
        Err(OptionalClaimsError::PreferredUsernameHasControlOrBidi)
    );
}

#[test]
fn a_preferred_username_with_a_bidi_isolate_is_refused() {
    let p = "{\"preferred_username\":\"a\u{2066}b\"}";
    assert_eq!(
        validate_preferred_username(&verified(p)),
        Err(OptionalClaimsError::PreferredUsernameHasControlOrBidi)
    );
}

#[test]
fn a_well_formed_preferred_username_is_accepted() {
    let p = r#"{"preferred_username":"alice"}"#;
    assert_eq!(
        validate_preferred_username(&verified(p)),
        Ok(Some("alice".to_string()))
    );
}

/// Stage 6c-fix, item 1: the scalar bound's accepting side, at exactly 128
/// plain-ASCII scalars (128 bytes too, well under the byte bound).
#[test]
fn a_preferred_username_of_exactly_128_ascii_scalars_is_accepted() {
    let s = "a".repeat(128);
    let p = format!(r#"{{"preferred_username":"{s}"}}"#);
    assert_eq!(validate_preferred_username(&verified(&p)), Ok(Some(s)));
}

/// The byte bound's accepting side, at exactly 512 bytes: 128 four-byte
/// scalars is *also* exactly 128 scalars, sitting at both limits (`512 ==
/// 4 * 128`) without exceeding either -- see `validate_bounded_display_string`'s
/// doc comment for why these two numbers coincide for this claim.
#[test]
fn a_preferred_username_of_exactly_512_bytes_is_accepted() {
    let s = "\u{1F600}".repeat(128);
    assert_eq!(s.len(), 512);
    assert_eq!(s.chars().count(), 128);
    let p = format!(r#"{{"preferred_username":"{s}"}}"#);
    assert_eq!(validate_preferred_username(&verified(&p)), Ok(Some(s)));
}

// ---- name ----------------------------------------------------------------

#[test]
fn an_absent_name_is_not_an_error() {
    assert_eq!(validate_name(&verified("{}")), Ok(None));
}

#[test]
fn a_non_string_name_is_refused() {
    let p = r#"{"name":5}"#;
    assert_eq!(
        validate_name(&verified(p)),
        Err(OptionalClaimsError::NameWrongType)
    );
}

/// 257 ASCII scalars: 257 bytes (under 1024), 257 scalars (over 256) --
/// the scalar-only direction, same reasoning as `preferred_username`'s.
#[test]
fn a_name_with_257_ascii_scalars_fails_the_scalar_bound_only() {
    let s = "a".repeat(257);
    let p = format!(r#"{{"name":"{s}"}}"#);
    assert_eq!(
        validate_name(&verified(&p)),
        Err(OptionalClaimsError::NameTooManyScalars)
    );
}

/// 300 four-byte scalars (the UTF-8 maximum per scalar): 1200 bytes (over
/// 1024) and 300 scalars (also over 256) -- both-violate, byte check first.
#[test]
fn a_name_with_300_four_byte_scalars_fails_the_byte_bound_first() {
    let s = "\u{1F600}".repeat(300);
    assert_eq!(s.len(), 1200);
    assert_eq!(s.chars().count(), 300);
    let p = format!(r#"{{"name":"{s}"}}"#);
    assert_eq!(
        validate_name(&verified(&p)),
        Err(OptionalClaimsError::NameTooManyBytes)
    );
}

#[test]
fn a_name_with_a_control_character_is_refused() {
    let p = r#"{"name":"a\u0007b"}"#;
    assert_eq!(
        validate_name(&verified(p)),
        Err(OptionalClaimsError::NameHasControlOrBidi)
    );
}

#[test]
fn a_well_formed_name_is_accepted() {
    let p = r#"{"name":"Alice Example"}"#;
    assert_eq!(
        validate_name(&verified(p)),
        Ok(Some("Alice Example".to_string()))
    );
}

/// Stage 6c-fix, item 1: the scalar bound's accepting side, at exactly 256
/// plain-ASCII scalars.
#[test]
fn a_name_of_exactly_256_ascii_scalars_is_accepted() {
    let s = "a".repeat(256);
    let p = format!(r#"{{"name":"{s}"}}"#);
    assert_eq!(validate_name(&verified(&p)), Ok(Some(s)));
}

/// The byte bound's accepting side, at exactly 1024 bytes: 256 four-byte
/// scalars, exactly at both limits (`1024 == 4 * 256`).
#[test]
fn a_name_of_exactly_1024_bytes_is_accepted() {
    let s = "\u{1F600}".repeat(256);
    assert_eq!(s.len(), 1024);
    assert_eq!(s.chars().count(), 256);
    let p = format!(r#"{{"name":"{s}"}}"#);
    assert_eq!(validate_name(&verified(&p)), Ok(Some(s)));
}

// ---- amr -------------------------------------------------------------

#[test]
fn an_absent_amr_is_not_an_error() {
    assert_eq!(validate_amr(&verified("{}")), Ok(()));
}

#[test]
fn a_non_array_amr_is_refused() {
    let p = r#"{"amr":"pwd"}"#;
    assert_eq!(
        validate_amr(&verified(p)),
        Err(OptionalClaimsError::AmrWrongType)
    );
}

#[test]
fn an_amr_with_17_entries_is_refused() {
    let entries: Vec<String> = (0..17).map(|i| format!("\"m{i}\"")).collect();
    let p = format!(r#"{{"amr":[{}]}}"#, entries.join(","));
    assert_eq!(
        validate_amr(&verified(&p)),
        Err(OptionalClaimsError::AmrTooMany)
    );
}

#[test]
fn an_amr_entry_that_is_not_a_string_is_refused() {
    let p = r#"{"amr":["pwd",5]}"#;
    assert_eq!(
        validate_amr(&verified(p)),
        Err(OptionalClaimsError::AmrElementWrongType)
    );
}

#[test]
fn an_empty_amr_entry_is_refused() {
    let p = r#"{"amr":[""]}"#;
    assert_eq!(
        validate_amr(&verified(p)),
        Err(OptionalClaimsError::AmrElementLength)
    );
}

#[test]
fn an_amr_entry_over_64_bytes_is_refused() {
    let s = "a".repeat(65);
    let p = format!(r#"{{"amr":["{s}"]}}"#);
    assert_eq!(
        validate_amr(&verified(&p)),
        Err(OptionalClaimsError::AmrElementLength)
    );
}

#[test]
fn an_amr_entry_with_a_space_is_refused_as_not_visible_ascii() {
    let p = r#"{"amr":["p w d"]}"#;
    assert_eq!(
        validate_amr(&verified(p)),
        Err(OptionalClaimsError::AmrElementNotVisibleAscii)
    );
}

#[test]
fn a_repeated_amr_entry_is_refused() {
    let p = r#"{"amr":["pwd","pwd"]}"#;
    assert_eq!(
        validate_amr(&verified(p)),
        Err(OptionalClaimsError::AmrDuplicate)
    );
}

#[test]
fn a_well_formed_amr_is_accepted() {
    let p = r#"{"amr":["pwd","otp"]}"#;
    assert_eq!(validate_amr(&verified(p)), Ok(()));
}

/// Stage 6c-fix, item 1: the count bound's accepting side, at exactly 16
/// unique entries.
#[test]
fn an_amr_of_exactly_16_entries_is_accepted() {
    let entries: Vec<String> = (0..16).map(|i| format!("\"m{i}\"")).collect();
    let p = format!(r#"{{"amr":[{}]}}"#, entries.join(","));
    assert_eq!(validate_amr(&verified(&p)), Ok(()));
}

/// The element-length bound's accepting side, at exactly 64 bytes.
#[test]
fn an_amr_entry_of_exactly_64_bytes_is_accepted() {
    let s = "a".repeat(64);
    let p = format!(r#"{{"amr":["{s}"]}}"#);
    assert_eq!(validate_amr(&verified(&p)), Ok(()));
}

// ---- acr -------------------------------------------------------------

#[test]
fn an_absent_acr_is_not_an_error() {
    assert_eq!(validate_acr(&verified("{}")), Ok(()));
}

#[test]
fn a_non_string_acr_is_refused() {
    let p = r#"{"acr":5}"#;
    assert_eq!(
        validate_acr(&verified(p)),
        Err(OptionalClaimsError::AcrWrongType)
    );
}

#[test]
fn an_empty_acr_is_refused() {
    let p = r#"{"acr":""}"#;
    assert_eq!(
        validate_acr(&verified(p)),
        Err(OptionalClaimsError::AcrLength)
    );
}

#[test]
fn an_acr_over_256_bytes_is_refused() {
    let s = "a".repeat(257);
    let p = format!(r#"{{"acr":"{s}"}}"#);
    assert_eq!(
        validate_acr(&verified(&p)),
        Err(OptionalClaimsError::AcrLength)
    );
}

#[test]
fn an_acr_with_a_space_is_refused_as_not_visible_ascii() {
    let p = r#"{"acr":"urn:x a"}"#;
    assert_eq!(
        validate_acr(&verified(p)),
        Err(OptionalClaimsError::AcrNotVisibleAscii)
    );
}

#[test]
fn a_well_formed_acr_is_accepted() {
    let p = r#"{"acr":"urn:mace:incommon:iap:silver"}"#;
    assert_eq!(validate_acr(&verified(p)), Ok(()));
}

/// Stage 6c-fix, item 1: the length bound's accepting side, at exactly 256
/// bytes.
#[test]
fn an_acr_of_exactly_256_bytes_is_accepted() {
    let s = "a".repeat(256);
    let p = format!(r#"{{"acr":"{s}"}}"#);
    assert_eq!(validate_acr(&verified(&p)), Ok(()));
}

// ---- auth_time -------------------------------------------------------

#[test]
fn an_absent_auth_time_is_not_an_error() {
    assert_eq!(validate_auth_time(&verified("{}")), Ok(()));
}

#[test]
fn a_string_auth_time_is_refused() {
    let p = r#"{"auth_time":"1700000000"}"#;
    assert_eq!(
        validate_auth_time(&verified(p)),
        Err(OptionalClaimsError::AuthTimeWrongType)
    );
}

#[test]
fn a_float_auth_time_is_refused() {
    let p = r#"{"auth_time":1700000000.5}"#;
    assert_eq!(
        validate_auth_time(&verified(p)),
        Err(OptionalClaimsError::AuthTimeNotAnInteger)
    );
}

#[test]
fn a_negative_auth_time_is_refused() {
    let p = r#"{"auth_time":-5}"#;
    assert_eq!(
        validate_auth_time(&verified(p)),
        Err(OptionalClaimsError::AuthTimeNegative)
    );
}

#[test]
fn an_auth_time_past_u64_is_refused() {
    let p = format!(r#"{{"auth_time":{}}}"#, u64::MAX);
    assert_eq!(
        validate_auth_time(&verified(&p)),
        Err(OptionalClaimsError::AuthTimeOutOfRange)
    );
}

#[test]
fn a_well_formed_auth_time_is_accepted() {
    let p = r#"{"auth_time":1700000000}"#;
    assert_eq!(validate_auth_time(&verified(p)), Ok(()));
}

// ---- at_hash -----------------------------------------------------------

#[test]
fn an_absent_at_hash_is_not_an_error() {
    assert_eq!(validate_at_hash(&verified("{}")), Ok(()));
}

#[test]
fn a_non_string_at_hash_is_refused() {
    let p = r#"{"at_hash":5}"#;
    assert_eq!(
        validate_at_hash(&verified(p)),
        Err(OptionalClaimsError::AtHashWrongType)
    );
}

#[test]
fn an_empty_at_hash_is_refused() {
    let p = r#"{"at_hash":""}"#;
    assert_eq!(
        validate_at_hash(&verified(p)),
        Err(OptionalClaimsError::AtHashLength)
    );
}

#[test]
fn an_at_hash_over_256_bytes_is_refused() {
    let s = "a".repeat(257);
    let p = format!(r#"{{"at_hash":"{s}"}}"#);
    assert_eq!(
        validate_at_hash(&verified(&p)),
        Err(OptionalClaimsError::AtHashLength)
    );
}

#[test]
fn an_at_hash_with_a_space_is_refused_as_not_visible_ascii() {
    let p = r#"{"at_hash":"a b"}"#;
    assert_eq!(
        validate_at_hash(&verified(p)),
        Err(OptionalClaimsError::AtHashNotVisibleAscii)
    );
}

#[test]
fn a_well_formed_at_hash_is_accepted() {
    let p = r#"{"at_hash":"77QmUPtjPfzWtF2AnpK9RQ"}"#;
    assert_eq!(validate_at_hash(&verified(p)), Ok(()));
}

/// Stage 6c-fix, item 1: the length bound's accepting side, at exactly 256
/// bytes.
#[test]
fn an_at_hash_of_exactly_256_bytes_is_accepted() {
    let s = "a".repeat(256);
    let p = format!(r#"{{"at_hash":"{s}"}}"#);
    assert_eq!(validate_at_hash(&verified(&p)), Ok(()));
}

// ---- the orchestrator --------------------------------------------------

#[test]
fn with_nothing_present_every_optional_field_is_none() {
    let result = validate_optional_claims(&verified("{}")).expect("no optional claim present");
    assert_eq!(result.verified_email(), None);
    assert_eq!(result.preferred_username(), None);
    assert_eq!(result.name(), None);
}

#[test]
fn email_plus_verified_true_yields_a_verified_email() {
    let p = r#"{"email":"user@example.com","email_verified":true}"#;
    let result = validate_optional_claims(&verified(p)).expect("well-formed");
    assert_eq!(result.verified_email(), Some("user@example.com"));
}

#[test]
fn email_present_but_verified_false_yields_no_verified_email() {
    let p = r#"{"email":"user@example.com","email_verified":false}"#;
    let result = validate_optional_claims(&verified(p)).expect("well-formed");
    assert_eq!(result.verified_email(), None);
}

#[test]
fn email_present_but_verified_absent_yields_no_verified_email() {
    let p = r#"{"email":"user@example.com"}"#;
    let result = validate_optional_claims(&verified(p)).expect("well-formed");
    assert_eq!(result.verified_email(), None);
}

#[test]
fn a_malformed_amr_fails_the_whole_orchestrator_even_though_amr_is_discarded() {
    let p = r#"{"amr":["pwd","pwd"]}"#;
    // `ValidatedOptionalClaims` has no `Debug` (RFC 096 :707) so `Result<_,
    // _>` as a whole has none either; `matches!` needs none.
    assert!(matches!(
        validate_optional_claims(&verified(p)),
        Err(OptionalClaimsError::AmrDuplicate)
    ));
}
