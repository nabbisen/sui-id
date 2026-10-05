use super::*;

#[test]
fn password_only_is_loa_1() {
    assert_eq!(acr_from_methods(&[AuthMethod::Pwd]), "1");
    assert_eq!(amr_from_methods(&[AuthMethod::Pwd]), vec!["pwd"]);
}

#[test]
fn password_plus_totp_is_loa_2_with_mfa() {
    let m = [AuthMethod::Pwd, AuthMethod::Totp];
    assert_eq!(acr_from_methods(&m), "2");
    assert_eq!(amr_from_methods(&m), vec!["pwd", "otp", "mfa"]);
}

#[test]
fn password_plus_recovery_is_loa_2_with_otp_amr() {
    // Recovery codes share the `otp` AMR with TOTP — both are
    // one-time codes from the RP's perspective.
    let m = [AuthMethod::Pwd, AuthMethod::RecoveryCode];
    assert_eq!(acr_from_methods(&m), "2");
    assert_eq!(amr_from_methods(&m), vec!["pwd", "otp", "mfa"]);
}

#[test]
fn password_plus_webauthn_is_loa_3() {
    let m = [AuthMethod::Pwd, AuthMethod::Webauthn];
    assert_eq!(acr_from_methods(&m), "3");
    assert_eq!(amr_from_methods(&m), vec!["pwd", "hwk", "mfa"]);
}

#[test]
fn duplicates_are_deduped_and_mfa_isnt_added_twice() {
    let m = [
        AuthMethod::Pwd,
        AuthMethod::Pwd,
        AuthMethod::Totp,
        AuthMethod::Totp,
    ];
    assert_eq!(amr_from_methods(&m), vec!["pwd", "otp", "mfa"]);
}

#[test]
fn empty_methods_falls_back_to_loa_1() {
    // An empty slice is a corruption case in practice — any
    // sui-id session has at least Pwd. Be conservative: the
    // lowest LoA, no `mfa`, no second factor.
    assert_eq!(acr_from_methods(&[]), "1");
    assert!(amr_from_methods(&[]).is_empty());
}

#[test]
fn webauthn_alone_is_phishing_resistant_so_loa_3_but_not_mfa() {
    // Hypothetical future: passwordless WebAuthn-only sign-in.
    // The hardware-bound key assertion clears LoA 3 by itself,
    // but it is *not* multi-factor — just one phishing-
    // resistant factor.
    let m = [AuthMethod::Webauthn];
    assert_eq!(acr_from_methods(&m), "3");
    assert_eq!(amr_from_methods(&m), vec!["hwk"]);
}
