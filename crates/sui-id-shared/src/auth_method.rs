//! Which authentication factors a session was established with.
//!
//! Recorded once at session creation and read back when issuing an
//! ID token, so that the `acr` and `amr` claims (OpenID Connect Core
//! §2; AMR values are RFC 8176) accurately describe how the user
//! proved who they are.
//!
//! The list of methods on a session is the historical record of
//! *that* sign-in. It is not refreshed on subsequent token-endpoint
//! calls — a session that started as password-only does not become
//! MFA later just because the user enrols TOTP afterwards.

use serde::{Deserialize, Serialize};

/// Authentication method that contributed to a session's
/// authentication. The variants map 1:1 to RFC 8176 AMR values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// Username + password. Always present in a sui-id session.
    Pwd,
    /// TOTP authenticator app code.
    Totp,
    /// One of the user's pre-issued recovery codes.
    RecoveryCode,
    /// WebAuthn assertion (passkey / hardware key).
    Webauthn,
}

impl AuthMethod {
    /// The RFC 8176 AMR token for this method. These strings appear
    /// verbatim in the `amr` array of issued ID tokens; relying
    /// parties match on them.
    pub fn as_amr(self) -> &'static str {
        match self {
            // RFC 8176 §2 — values are case-sensitive lowercase.
            Self::Pwd => "pwd",
            // TOTP and recovery codes are both one-time passwords from
            // the perspective of relying parties; RFC 8176 has a
            // single `otp` token for both.
            Self::Totp | Self::RecoveryCode => "otp",
            // RFC 8176 §2 — `hwk` is "proof of possession of a
            // hardware-secured key". This is what a passkey or
            // security key stored in a TPM/Secure Enclave is.
            Self::Webauthn => "hwk",
        }
    }

    /// Whether this method counts as a *second factor* — i.e. it is
    /// neither password nor a knowledge-only credential. Used to
    /// derive the ACR level.
    pub fn is_second_factor(self) -> bool {
        match self {
            Self::Pwd => false,
            Self::Totp | Self::RecoveryCode | Self::Webauthn => true,
        }
    }

    /// Whether this method is *phishing-resistant* in the sense that
    /// the credential cannot be replayed by an attacker who has
    /// captured the user's keystrokes. WebAuthn is; TOTP is not (the
    /// 6-digit code is interceptable inside its 30-second window).
    pub fn is_phishing_resistant(self) -> bool {
        matches!(self, Self::Webauthn)
    }
}

/// OIDC Authentication Context Class Reference value derived from
/// the methods used in a session.
///
/// We follow the ISO/IEC 29115 four-level Level-of-Assurance scheme
/// (also referenced from OpenID Connect Core §2 as the reference
/// example for `acr`), encoded as the bare numeric strings `"1"`,
/// `"2"`, `"3"` that Keycloak and most other off-the-shelf IdPs
/// produce. Numeric strings are the most widely understood form
/// across RP libraries; the longer URI variants used by NIST AAL
/// or eIDAS LoA fit those specific contexts and are needlessly
/// verbose for a general-purpose IdP.
///
/// Mapping:
///
/// - `"1"` — single factor. Password only. Equivalent to ISO 29115
///   LoA 1 / NIST AAL 1.
/// - `"2"` — multi-factor with a software second factor (TOTP,
///   recovery code). Equivalent to ISO 29115 LoA 2.
/// - `"3"` — multi-factor with a phishing-resistant hardware
///   second factor (WebAuthn). Equivalent to ISO 29115 LoA 3.
///
/// LoA 4 (in-person identity proofing) is not something an IdP can
/// assert from authentication alone, so sui-id never produces it.
pub fn acr_from_methods(methods: &[AuthMethod]) -> &'static str {
    if methods.iter().any(|m| m.is_phishing_resistant()) {
        "3"
    } else if methods.iter().any(|m| m.is_second_factor()) {
        "2"
    } else {
        "1"
    }
}

/// Build the `amr` claim array. Includes `"mfa"` (RFC 8176) as the
/// umbrella signal when **two or more distinct factor types** were
/// used — i.e. when the sign-in was genuinely multi-factor. A
/// single-factor sign-in, even one with a hardware key, does not
/// claim `mfa`.
///
/// Deduplicates while preserving order so that `["pwd", "otp", "mfa"]`
/// is the canonical shape.
pub fn amr_from_methods(methods: &[AuthMethod]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(methods.len() + 1);
    for m in methods {
        let v = m.as_amr().to_string();
        if !out.contains(&v) {
            out.push(v);
        }
    }
    // RFC 8176 `mfa`: claim only if two or more *distinct* factor
    // tokens are present. WebAuthn alone, or password alone, does
    // not earn `mfa` even though WebAuthn is phishing-resistant.
    if out.len() >= 2 && !out.contains(&"mfa".to_string()) {
        out.push("mfa".to_string());
    }
    out
}

#[cfg(test)]
mod tests {
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
}
