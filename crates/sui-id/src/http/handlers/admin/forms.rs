//! Shared form-data structs for the admin handlers (RFC 066).

use serde::Deserialize;

#[derive(Debug, Deserialize)]

pub struct DisableForm {
    pub disabled: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    /// Confirmation token from the dangerous-action confirm screen
    /// (RFC 030; RFC 060 bug fix). Empty / missing → 400 from
    /// `require_confirmed`.
    #[serde(rename = "_confirmed", default)]
    pub confirmed: String,
    /// Optional reason for disabling the user (RFC 045). Stored in audit note.
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize, Default)]

pub struct CsrfOnlyForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// Body for dangerous-operation POSTs that require both a CSRF token and an
/// explicit `_confirmed=1` field (RFC 030). The confirmation screen supplies
/// this field; direct-POST attacks without it are rejected.
#[derive(Debug, Deserialize, Default)]

pub struct ConfirmedForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    #[serde(rename = "_confirmed", default)]
    pub confirmed: String,
}

/// Body for dangerous-operation POSTs that also carry an operator-supplied
/// reason for the audit log (RFC 060 + RFC 045 pattern). Empty / missing
/// `reason` is OK; the value is stored verbatim (trimmed) in the audit row's
/// `note` column.
#[derive(Debug, Deserialize, Default)]

pub struct ConfirmedReasonForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    #[serde(rename = "_confirmed", default)]
    pub confirmed: String,
    #[serde(default)]
    pub reason: String,
}

impl ConfirmedReasonForm {
    /// Trimmed reason, `None` if empty after trim.
    pub fn reason_opt(&self) -> Option<String> {
        let t = self.reason.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_owned())
        }
    }
}
