use crate::errors::CoreError;
// Shared audit helpers from parent module.
use super::validate_redirect_uri;

#[test]
fn https_redirect_is_accepted() {
    validate_redirect_uri("https://app.example.com/callback").expect("ok");
}

#[test]
fn http_loopback_is_accepted() {
    validate_redirect_uri("http://localhost:8080/cb").expect("ok");
    validate_redirect_uri("http://127.0.0.1/cb").expect("ok");
}

#[test]
fn http_non_loopback_is_rejected() {
    let r = validate_redirect_uri("http://example.com/cb");
    assert!(matches!(r, Err(CoreError::BadRequest(_))));
}

#[test]
fn fragment_is_rejected() {
    let r = validate_redirect_uri("https://x/cb#frag");
    assert!(matches!(r, Err(CoreError::BadRequest(_))));
}

#[test]
fn non_http_scheme_is_rejected() {
    let r = validate_redirect_uri("javascript:alert(1)");
    assert!(matches!(r, Err(CoreError::BadRequest(_))));
}
