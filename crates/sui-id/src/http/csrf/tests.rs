use super::*;
use axum::http::HeaderValue;

fn jar_with_cookie(value: &str) -> CookieJar {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        HeaderValue::from_str(&format!("{CSRF_COOKIE}={value}")).expect("static"),
    );
    CookieJar::from_headers(&headers)
}

#[test]
fn new_token_has_expected_format() {
    let t = new_token();
    // Base64URL no-pad of 32 bytes is 43 chars.
    assert_eq!(t.len(), 43);
    assert!(
        t.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    );
}

#[test]
fn ensure_token_reuses_existing_value() {
    let jar = jar_with_cookie("abc-existing");
    assert_eq!(ensure_token(&jar), "abc-existing");
}

#[test]
fn ensure_token_mints_new_value_when_cookie_missing() {
    let jar = CookieJar::from_headers(&HeaderMap::new());
    let t = ensure_token(&jar);
    assert_eq!(t.len(), 43);
}

#[test]
fn check_token_accepts_matching_pair() {
    let jar = jar_with_cookie("the-secret");
    assert_eq!(
        check_token(&jar, Some("the-secret")).as_deref(),
        Some("the-secret")
    );
}

#[test]
fn check_token_rejects_mismatch() {
    let jar = jar_with_cookie("the-secret");
    assert!(check_token(&jar, Some("not-the-secret")).is_none());
}

#[test]
fn check_token_rejects_missing_cookie() {
    let jar = CookieJar::from_headers(&HeaderMap::new());
    assert!(check_token(&jar, Some("anything")).is_none());
}

#[test]
fn check_token_rejects_missing_form_field() {
    let jar = jar_with_cookie("the-secret");
    assert!(check_token(&jar, None).is_none());
}

#[test]
fn check_token_rejects_empty_strings() {
    let jar = jar_with_cookie("");
    assert!(check_token(&jar, Some("")).is_none());
}
