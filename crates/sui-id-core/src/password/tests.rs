use super::*;

#[test]
fn hash_then_verify_roundtrips() {
    let pw = "correct horse battery staple";
    let phc = hash_password(pw).expect("hash");
    verify_password(pw, &phc).expect("verify");
}

#[test]
fn wrong_password_is_rejected() {
    let phc = hash_password("a-very-strong-password").expect("hash");
    let r = verify_password("not the right password", &phc);
    assert!(matches!(r, Err(CoreError::InvalidCredentials)));
}

#[test]
fn malformed_stored_hash_returns_password_error() {
    let r = verify_password("anything", "this is not phc");
    assert!(matches!(r, Err(CoreError::Password)));
}

#[test]
fn policy_rejects_short_passwords() {
    let r = check_password_policy("short");
    assert!(matches!(r, Err(CoreError::BadRequest(_))));
}

#[test]
fn policy_accepts_reasonable_length_password() {
    check_password_policy("a-perfectly-reasonable-pass").expect("policy");
}
