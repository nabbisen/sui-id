//! Shared form / query structs for /me/security/* handlers (RFC 068).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CsrfOnlyForm {
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

#[derive(Debug, Deserialize)]
pub struct RevokeAllOthersForm {
    #[serde(rename = "_csrf")]
    pub csrf: String,
    /// The session id of the request itself, posted from a hidden
    /// field. We don't trust it on its own — we cross-check against
    /// the cookie — but having it in the form means the keep-set is
    /// explicit and auditable.
    pub current_session: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordChangeForm {
    #[serde(rename = "_csrf")]
    pub csrf: String,
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
    /// Checkbox value. Browsers send the field only when checked,
    /// so the option is presence-detected. Any non-empty string
    /// means "yes, sweep my other sessions and refresh tokens".
    #[serde(default)]
    pub revoke_others: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct PasskeyRenameForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub nickname: String,
}

/// POST /me/security/passkeys/{id}/rename

#[derive(serde::Deserialize)]
pub struct LanguageGetQuery {
    pub saved: Option<u8>,
}

/// GET /me/security/language

#[derive(serde::Deserialize)]
pub struct LanguageForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    /// "ja" / "en" / "zh" / "" (= clear preference)
    pub locale: String,
}

/// POST /me/security/language

#[derive(Debug, Deserialize)]
pub struct MfaConfirmForm {
    pub code: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// POST /me/security/mfa/enroll/start — begin TOTP enrollment

#[derive(Debug, Deserialize)]
pub struct PasskeyRegisterStartForm {
    pub nickname: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// POST /me/security/passkeys/register/start

#[derive(Debug, Deserialize)]
pub struct PasskeyRegisterCompleteForm {
    pub credential: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// POST /me/security/passkeys/register/complete

#[derive(Debug, Deserialize)]
pub struct PasskeyDeleteForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}
