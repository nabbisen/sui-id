use super::*;

#[test]
fn random_token_has_expected_length_and_alphabet() {
    let t = random_token(32);
    // base64url-no-pad of 32 bytes is ceil(32 * 4 / 3) = 43 chars.
    assert_eq!(t.len(), 43);
    assert!(t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
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
fn pkce_unsupported_method_is_rejected_as_bad_request() {
    let r = verify_pkce("md5", "x", "y");
    assert!(matches!(r, Err(CoreError::BadRequest(_))));
}
