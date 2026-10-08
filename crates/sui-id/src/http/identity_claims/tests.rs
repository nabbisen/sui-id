use super::*;
use crate::id_token::verify_id_token_against_jwks;
use crate::jwks::Jwks;
use base64ct::{Base64UrlUnpadded, Encoding};

/// PKCS#1 DER (`jsonwebtoken`'s `aws-lc-rs` RSA backend needs PKCS#1, not
/// PKCS#8 -- the same finding stage 4a's own key material comment records).
/// Generated once for this module's fixtures; signs nothing outside this file.
const RSA_DER: &str = "MIIEogIBAAKCAQEArLOyRn5+sfSg9jNccpJTXiSiLtW/IJPvHZq81XO8v3U+gOPYVKDSNXy26GLAaZgNdB7KOIQsNnmw/OYtxlkBAV/vS0zRwLI5qwNIfnVp0SQMPkJgXSWbAkL/hVnmb6mgobevlke/2lBGEyO7RAnFfEpzATb666Avbf+BMK0+uL96OhgYKY5nqa6U3rohiMr+Fx8uOBdk2GbFn6IWXhsmK9+QVCY3YT40OP/WhjDtTSO04Cf66p+nib4mBqdOQvSokGhbYMnCEvEUkXWu4maOW0dB03CaNDmV86kovn024CSjnERgMHOsRdMtuFm0yQRpVlGRkFYX8OY5A/8gHmoIgwIDAQABAoIBAASKxkbX47TO7NRrwh+4FHuyaQVsyFGQ/jLK050uNqOD51D2Svjhwg0IirIIRx+OxKMDoLFQKWXrpeMZ71iY82TrGF8XhQdESLa5igOXjuDvSaxYUKFdMGX5/w0lBx1RtBKnbbnSeCEZED9HfDLuZe0G801NpMZjnOVUFN6l98zmYxpPVgZAG1BqzXCRquGOu/PszDZ07aDCC6zRZxCqmi+mYsAL0N4K2J8yy/HgV1kQ8lzilDKL0EPMNtyNzf+QBV7q6kl9Ob4uoHLHYwX1F9D4XoVhaXv/Uu0fTdG/qCiZC7woCmqjEf/AJiuJaaBEi2w1PychP9EKZxHtNQ/5qwUCgYEA25g/vCzxkQrJ6pMlHTwIQmGHMVRLM60MmJ+jdOUSc9gHIslz8T1gLc9RwwhPQyCYEiBIZ1qrUoFWQMfyixQk5WZ+ydN0icAoF0s0g0qKQNQDnG2RMD0R620KBzLEDzBKbmWB8U+Y49JKzGoV4Cx+kRrCOZR/ZVfzN3qOAmyT06UCgYEAyVVJEtnS5rZNkZwj4QgdpdpOX7+10l5trpE+d/mYhNeFRkhEUe8ibiyxiQb9lB9DZ4sXgMfAyHGc8bh+02gPPwlVV2jRqb6h7FO69tN+yVaKcY5NA72ERuJvL7oXQ2xIj5XMX5QrQKxJS055uGjj+a1iCjmschrozQUESh7iEwcCgYAnqmKo3P1tk6NRae7kTvm28+L1uCI1XWbPEtb1wIMKxdTUJct5ofqDi9VbA1894t9VNtudP7V+m7o2zWc0VBkuDsuMLVP5peoX+w+rP4WlnCZi1S/KpN1dxz5uem8Lx09Kja9hJV2amVvFfMwiyCa8kzbOK9KvPanDNbH9Ihu5uQKBgA4gE65k5e0V4T9UCxhgr2PReyowkxsdUOisfAuC0XaQgGM78r8k3e+I5zPL78KSpvH+yjlYymfFwNMctJk0dc1gZEJrsjoMi+O+xCFJGV4a2j+5UiHvC/bFMDPTBIrQcA7S3bHe/WHeNI46BUQw572+smAxR64BwU+RCIoCvK3FAoGAMaVCgqKomZMB1hasz+vflPG+W0FukaQ0f1pjAYSRDlahRdlYNVrKpB+dto+BXLgpRHNn0YoJFU7ZtMPl1XtmYawF4WPu5Alp7UT7OwXIuk2CZpbtL28pxuA6zWkKKrvBpqv8KEOSO8ah2pVfybCilFtw8O9DICaiGrj/G0UcF4E=";
const RSA_N: &str = "rLOyRn5-sfSg9jNccpJTXiSiLtW_IJPvHZq81XO8v3U-gOPYVKDSNXy26GLAaZgNdB7KOIQsNnmw_OYtxlkBAV_vS0zRwLI5qwNIfnVp0SQMPkJgXSWbAkL_hVnmb6mgobevlke_2lBGEyO7RAnFfEpzATb666Avbf-BMK0-uL96OhgYKY5nqa6U3rohiMr-Fx8uOBdk2GbFn6IWXhsmK9-QVCY3YT40OP_WhjDtTSO04Cf66p-nib4mBqdOQvSokGhbYMnCEvEUkXWu4maOW0dB03CaNDmV86kovn024CSjnERgMHOsRdMtuFm0yQRpVlGRkFYX8OY5A_8gHmoIgw";
const RSA_E: &str = "AQAB";
const KID: &str = "test-key";
const ISSUER: &str = "https://idp.example.com";
const CLIENT_ID: &str = "this-rp-client-id";

fn der(b64_std: &str) -> Vec<u8> {
    base64ct::Base64::decode_vec(b64_std).expect("valid standard base64 DER")
}

fn jwks() -> Jwks {
    Jwks {
        keys: vec![serde_json::json!({"kty": "RSA", "kid": KID, "n": RSA_N, "e": RSA_E})],
    }
}

/// Signs a token with a header naming `kid`/`alg` and a **raw, caller-chosen
/// payload string** -- not a `Serialize` struct -- so a test can put a
/// duplicate key, a wrong type, or any other malformed shape directly into
/// the payload bytes. `jsonwebtoken::crypto::sign` signs the exact message
/// bytes given to it; nothing about this path goes through `encode()`,
/// which could only ever serialize a well-formed Rust value.
fn sign_raw_payload(payload_json: &str) -> String {
    let header_json = format!(r#"{{"alg":"RS256","kid":"{KID}"}}"#);
    // `validation_for`'s library default still requires `exp` (stage 4a
    // left it that way deliberately; 6b owns it). Every test here is about
    // the four stage-6a claims, none of which is `exp`, so it is injected
    // uniformly here rather than in each payload literal -- inserted right
    // after the opening brace, which cannot collide with any of this
    // stage's own duplicate-member tests (none of them duplicate `exp`).
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

/// Signs and runs the raw payload through the real `verify_id_token_against_jwks`
/// pipeline (stage 4a), the only way to obtain a `VerifiedIdTokenClaims` --
/// its fields are private, so there is no shortcut around real signature
/// verification even for a claims-only test.
fn verify_raw(
    payload_json: &str,
) -> Result<crate::id_token::VerifiedIdTokenClaims, crate::id_token::VerificationError> {
    let token = sign_raw_payload(payload_json);
    verify_id_token_against_jwks(&token, &["RS256".to_string()], &jwks())
}

fn verified(payload_json: &str) -> crate::id_token::VerifiedIdTokenClaims {
    verify_raw(payload_json).expect("a genuinely signed, structurally valid token must verify")
}

fn payload(extra: &str) -> String {
    format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}"{extra}}}"#)
}

/// Asserts `verify_raw` refuses `payload_json` with
/// `VerificationError::DuplicatePayloadMember(name)` carrying exactly
/// `expected_name` -- not `is_err()`, and not a wildcard `matches!` that
/// would pass for any name.
fn assert_duplicate_payload_member(payload_json: &str, expected_name: &str) {
    match verify_raw(payload_json) {
        Err(crate::id_token::VerificationError::DuplicatePayloadMember(name)) => {
            assert_eq!(name, expected_name);
        }
        other => panic!("expected DuplicatePayloadMember({expected_name:?}), got {other:?}"),
    }
}

// ---- the happy path ---------------------------------------------------------

#[test]
fn a_well_formed_token_satisfies_every_required_claim() {
    let claims = verified(&payload(""));
    let result = validate_identity_claims(&claims, ISSUER, CLIENT_ID);
    assert_eq!(result.as_ref().map(|c| c.sub()), Ok("user-1"));
}

#[test]
fn a_single_audience_with_a_matching_azp_is_accepted() {
    let claims = verified(&payload(r#","azp":"this-rp-client-id""#));
    assert!(validate_identity_claims(&claims, ISSUER, CLIENT_ID).is_ok());
}

#[test]
fn multiple_audiences_with_the_exact_azp_are_accepted() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","aud":["{CLIENT_ID}","other-client"],"azp":"{CLIENT_ID}"}}"#
    );
    assert!(validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID).is_ok());
}

// ---- iss --------------------------------------------------------------------

#[test]
fn a_missing_issuer_is_refused() {
    let p = format!(r#"{{"sub":"user-1","aud":"{CLIENT_ID}"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::IssuerMissing)
    );
}

#[test]
fn a_non_string_issuer_is_refused() {
    let p = format!(r#"{{"iss":123,"sub":"user-1","aud":"{CLIENT_ID}"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::IssuerNotString)
    );
}

#[test]
fn an_issuer_differing_by_one_byte_is_refused_with_no_normalisation() {
    // Trailing slash: the kind of difference URL normalisation would erase.
    let p = format!(r#"{{"iss":"{ISSUER}/","sub":"user-1","aud":"{CLIENT_ID}"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::IssuerMismatch)
    );
}

/// A repeated `iss` does **not** reach `validate_identity_claims` at all: it
/// is refused one layer earlier, inside `verify_id_token_against_jwks`
/// itself, by `first_duplicate_member`'s scan over `compact.payload` --
/// before `jsonwebtoken::decode` ever runs -- naming the real fault rather
/// than the `SignatureInvalid` a valid-signature token would otherwise be
/// mischarged with (`jsonwebtoken`'s own internal `ClaimsForValidation`
/// struct also rejects a duplicate `iss`, but only after decode has already
/// run, and under the wrong name).
#[test]
fn a_repeated_iss_member_is_refused_by_its_own_name() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","iss":"https://evil.example.com","sub":"user-1","aud":"{CLIENT_ID}"}}"#
    );
    assert_duplicate_payload_member(&p, "iss");
}

// ---- sub ----------------------------------------------------------------

#[test]
fn a_missing_subject_is_refused() {
    let p = format!(r#"{{"iss":"{ISSUER}","aud":"{CLIENT_ID}"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::SubjectMissing)
    );
}

#[test]
fn a_non_string_subject_is_refused() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":42,"aud":"{CLIENT_ID}"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::SubjectNotString)
    );
}

#[test]
fn an_empty_subject_is_refused() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":"","aud":"{CLIENT_ID}"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::SubjectLength)
    );
}

#[test]
fn a_subject_of_256_bytes_is_refused_255_is_accepted() {
    let at_limit = "s".repeat(255);
    let p255 = format!(r#"{{"iss":"{ISSUER}","sub":"{at_limit}","aud":"{CLIENT_ID}"}}"#);
    assert!(validate_identity_claims(&verified(&p255), ISSUER, CLIENT_ID).is_ok());

    let over_limit = "s".repeat(256);
    let p256 = format!(r#"{{"iss":"{ISSUER}","sub":"{over_limit}","aud":"{CLIENT_ID}"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p256), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::SubjectLength)
    );
}

#[test]
fn a_subject_with_a_control_character_is_refused() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":"user\u0007","aud":"{CLIENT_ID}"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::SubjectControlCharacter)
    );
}

/// Same mechanism as the repeated-`iss` case above.
#[test]
fn a_repeated_sub_member_is_refused_by_its_own_name() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":"user-1","sub":"user-2","aud":"{CLIENT_ID}"}}"#);
    assert_duplicate_payload_member(&p, "sub");
}

// ---- aud ----------------------------------------------------------------

#[test]
fn a_missing_audience_is_refused() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":"user-1"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AudienceMissing)
    );
}

#[test]
fn an_audience_given_as_a_number_is_refused() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":42}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AudienceInvalidShape)
    );
}

#[test]
fn an_empty_audience_array_is_refused() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":[]}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AudienceEmpty)
    );
}

#[test]
fn nine_audiences_are_refused_eight_are_accepted() {
    let eight: Vec<String> = (0..8)
        .map(|i| {
            if i == 0 {
                CLIENT_ID.to_string()
            } else {
                format!("other-{i}")
            }
        })
        .collect();
    let eight_json = serde_json::to_string(&eight).unwrap();
    let p8 =
        format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":{eight_json},"azp":"{CLIENT_ID}"}}"#);
    assert!(validate_identity_claims(&verified(&p8), ISSUER, CLIENT_ID).is_ok());

    let nine: Vec<String> = (0..9)
        .map(|i| {
            if i == 0 {
                CLIENT_ID.to_string()
            } else {
                format!("other-{i}")
            }
        })
        .collect();
    let nine_json = serde_json::to_string(&nine).unwrap();
    let p9 =
        format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":{nine_json},"azp":"{CLIENT_ID}"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p9), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AudienceTooMany)
    );
}

#[test]
fn a_duplicate_value_within_the_audience_array_is_refused() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","aud":["{CLIENT_ID}","{CLIENT_ID}"],"azp":"{CLIENT_ID}"}}"#
    );
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AudienceDuplicateValue)
    );
}

#[test]
fn an_audience_not_containing_the_client_id_is_refused() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"someone-else"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AudienceDoesNotContainClientId)
    );
}

/// Same mechanism again.
#[test]
fn a_repeated_aud_member_is_refused_by_its_own_name() {
    let p =
        format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}","aud":"someone-else"}}"#);
    assert_duplicate_payload_member(&p, "aud");
}

// ---- azp ------------------------------------------------------------------

#[test]
fn azp_is_required_when_audience_has_more_than_one_value() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":["{CLIENT_ID}","other-client"]}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AzpMissing)
    );
}

#[test]
fn azp_is_optional_when_audience_has_exactly_one_value() {
    assert!(validate_identity_claims(&verified(&payload("")), ISSUER, CLIENT_ID).is_ok());
}

#[test]
fn a_non_string_azp_is_refused() {
    let p = format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}","azp":7}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AzpNotString)
    );
}

#[test]
fn an_azp_present_for_a_single_audience_but_not_matching_is_refused() {
    let p =
        format!(r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}","azp":"someone-else"}}"#);
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AzpMismatch)
    );
}

#[test]
fn an_azp_present_for_multiple_audiences_but_not_matching_is_refused() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","aud":["{CLIENT_ID}","other-client"],"azp":"someone-else"}}"#
    );
    assert_eq!(
        validate_identity_claims(&verified(&p), ISSUER, CLIENT_ID),
        Err(IdentityClaimsError::AzpMismatch)
    );
}

#[test]
fn a_repeated_azp_member_is_refused_by_its_own_name() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","aud":"{CLIENT_ID}","azp":"{CLIENT_ID}","azp":"other"}}"#
    );
    assert_duplicate_payload_member(&p, "azp");
}

/// The scan is positional, not "compare neighbours": a repeat separated by
/// another member in between must still be caught. Matches what stages 2
/// and 3a already prove for the header and the JWKS document
/// (`a_member_repeated_non_adjacently_is_refused`).
#[test]
fn a_non_adjacent_repeated_member_is_refused_by_its_own_name() {
    let p = format!(
        r#"{{"iss":"{ISSUER}","sub":"user-1","iss":"https://evil.example.com","aud":"{CLIENT_ID}"}}"#
    );
    assert_duplicate_payload_member(&p, "iss");
}
