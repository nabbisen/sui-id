use super::*;

#[test]
fn raw_refresh_token_debug_does_not_contain_value() {
    let t = RawRefreshToken::from_untrusted("super-secret-12345".to_owned());
    let dbg = format!("{t:?}");
    assert!(
        !dbg.contains("super-secret-12345"),
        "debug must not expose value"
    );
    assert!(dbg.contains("REDACTED"));
}

#[test]
fn raw_refresh_token_display_does_not_contain_value() {
    let t = RawRefreshToken::from_untrusted("super-secret-12345".to_owned());
    let display = format!("{t}");
    assert!(!display.contains("super-secret-12345"));
}

#[test]
fn raw_refresh_token_expose_returns_value() {
    let t = RawRefreshToken::from_untrusted("my-token".to_owned());
    assert_eq!(t.expose(), "my-token");
}

#[test]
fn refresh_token_hash_of_is_deterministic() {
    let t = RawRefreshToken::from_untrusted("hello".to_owned());
    let h1 = RefreshTokenHash::of(&t);
    let h2 = RefreshTokenHash::of(&t);
    assert_eq!(h1.as_bytes(), h2.as_bytes());
}

#[test]
fn code_hash_of_is_hex_string() {
    let h = CodeHash::of("testcode");
    // SHA-256 produces 32 bytes → 64 hex chars
    assert_eq!(h.as_str().len(), 64);
    assert!(h.as_str().chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn family_id_root_of_equals_token_id_string() {
    let id = RefreshTokenId::generate();
    let fam = FamilyId::root_of(&id);
    assert_eq!(id.as_str(), fam.as_str());
}

#[test]
fn refresh_token_id_generate_looks_reasonable() {
    let id = RefreshTokenId::generate();
    // 16 bytes base64url → ~22 chars (no padding)
    assert!(id.as_str().len() >= 20);
    // Only base64url chars
    assert!(
        id.as_str()
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    );
}
