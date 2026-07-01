use super::*;

#[test]
fn random_token_has_expected_length_and_alphabet() {
    let t = random_token(32);
    // base64url-no-pad of 32 bytes is ceil(32 * 4 / 3) = 43 chars.
    assert_eq!(t.len(), 43);
    assert!(
        t.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    );
}

#[test]
fn random_tokens_are_distinct() {
    let a = random_token(32);
    let b = random_token(32);
    assert_ne!(a, b);
}

#[test]
fn sha256_hex_matches_known_vector() {
    // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    assert_eq!(
        sha256_hex("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn pkce_s256_accepts_valid_verifier() {
    // verifier "test_verifier_123456789012345678901234567890123" → S256 challenge.
    let verifier = "test_verifier_123456789012345678901234567890123";
    let digest = sha2::Sha256::digest(verifier.as_bytes());
    let mut out = vec![0u8; 64];
    let n = Base64UrlUnpadded::encode(&digest, &mut out)
        .map(str::len)
        .unwrap_or(0);
    out.truncate(n);
    let challenge = String::from_utf8(out).expect("ascii");
    verify_pkce("S256", verifier, &challenge).expect("S256 ok");
}

#[test]
fn pkce_s256_rejects_wrong_verifier() {
    let r = verify_pkce("S256", "wrong", "AAAAAA");
    assert!(matches!(r, Err(CoreError::Protocol { .. })));
}

#[test]
fn pkce_unsupported_method_is_rejected_as_invalid_grant() {
    // v0.17.0: previously this returned `BadRequest`. Now that
    // `verify_pkce` is also part of the defense-in-depth chain
    // against `code_challenge_method=plain` slipping through, all
    // unknown / disallowed methods return the OAuth-spec
    // `invalid_grant` error code uniformly.
    let r = verify_pkce("md5", "x", "y");
    assert!(matches!(
        r,
        Err(CoreError::Protocol {
            code: crate::errors::ProtocolError::InvalidGrant,
            ..
        })
    ));
}

// ---------- property-based tests (v0.13.0) ----------

use base64ct::{Base64UrlUnpadded, Encoding};
use proptest::prelude::*;
use sha2::{Digest, Sha256};

/// Reference implementation: derive an S256 PKCE challenge from a
/// verifier the way RFC 7636 §4.2 describes, with no shared code
/// path with the production verify_pkce. The property below cross-
/// checks production against this.
fn s256_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    let mut out = vec![0u8; 64];
    let n = Base64UrlUnpadded::encode(&digest, &mut out)
        .map(str::len)
        .unwrap_or(0);
    out.truncate(n);
    String::from_utf8(out).unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    })]

    #[test]
    fn s256_verifies_iff_challenge_was_derived_from_same_verifier(
        // RFC 7636 §4.1: verifier is 43..128 chars from
        // [A-Z][a-z][0-9]-._~. Our verify_pkce doesn't enforce that
        // alphabet at this layer (the higher level does), but we
        // stick to it here so the challenges round-trip cleanly.
        verifier in "[A-Za-z0-9._~-]{43,128}",
    ) {
        let challenge = s256_challenge(&verifier);
        prop_assert!(verify_pkce("S256", &verifier, &challenge).is_ok());
    }

    #[test]
    fn s256_rejects_any_distinct_verifier(
        verifier1 in "[A-Za-z0-9._~-]{43,128}",
        verifier2 in "[A-Za-z0-9._~-]{43,128}",
    ) {
        prop_assume!(verifier1 != verifier2);
        let challenge1 = s256_challenge(&verifier1);
        prop_assert!(verify_pkce("S256", &verifier2, &challenge1).is_err());
    }

    #[test]
    fn s256_challenge_size_is_43_chars(
        verifier in "[A-Za-z0-9._~-]{43,128}",
    ) {
        // SHA-256 output is 32 bytes → base64url-no-pad is exactly
        // 43 characters. Any drift here would be a framing bug.
        let challenge = s256_challenge(&verifier);
        prop_assert_eq!(challenge.len(), 43);
    }
}

#[test]
fn verify_pkce_rejects_plain_method() {
    // Defense-in-depth: even if `/oauth2/authorize` ever stops
    // refusing `code_challenge_method=plain`, `verify_pkce` still
    // says no. This test pins the layer behaviour.
    let r = super::verify_pkce("plain", "verifier", "verifier");
    assert!(
        matches!(
            r,
            Err(crate::errors::CoreError::Protocol {
                code: crate::errors::ProtocolError::InvalidGrant,
                ..
            })
        ),
        "expected InvalidGrant for method=plain, got: {r:?}"
    );
}

#[test]
fn verify_pkce_rejects_unknown_method() {
    let r = super::verify_pkce("S512", "v", "c");
    assert!(matches!(
        r,
        Err(crate::errors::CoreError::Protocol {
            code: crate::errors::ProtocolError::InvalidGrant,
            ..
        })
    ));
    let r = super::verify_pkce("", "v", "c");
    assert!(r.is_err());
}
