//! Page renderers for administrator-issued account recovery (RFC 103).
//!
//! The issuance page shows the link and the token **once**, in the response
//! to the POST that created them: the handler renders it directly and never
//! redirects with either in the URL (D10). Every string comes from
//! `sui-id-i18n`.

use super::common::*;
use crate::layout::Shell;
use leptos::prelude::*;

/// What an administrator has just issued. `url` and `token` are the only
/// place the token exists outside the database's hash.
pub struct RecoveryLinkIssuedData {
    pub user_id: String,
    pub username: String,
    /// `<issuer>/reset-password#t=<token>`.
    pub url: String,
    /// The token on its own, for a browser without JavaScript or a mail
    /// program that drops the fragment (D10).
    pub token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub invalidated: usize,
    pub csrf_token: String,
}

pub fn render_recovery_link_issued(
    data: RecoveryLinkIssuedData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let back = format!("/admin/users/{}", data.user_id);
        view! {
            <Shell title=t.recovery_link_issued_title.to_string() show_nav=true
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang
                   csrf_token=data.csrf_token.clone()>
                <div class="auth-card max-w-card">
                    <h1>{t.recovery_link_issued_title}</h1>
                    <p><strong>{data.username.clone()}</strong></p>
                    <p class="muted">{t.recovery_link_issued_once}</p>
                    <div class="field">
                        <span class="field__label">{t.recovery_link_url_label}</span>
                        {crate::components::copy_field(
                            t,
                            data.url.clone(),
                            t.copy_noun_recovery_link,
                            t.recovery_link_url_aria,
                        )}
                    </div>
                    <div class="field">
                        <span class="field__label">{t.recovery_link_token_label}</span>
                        {crate::components::copy_field(
                            t,
                            data.token.clone(),
                            t.copy_noun_recovery_token,
                            t.recovery_link_token_aria,
                        )}
                        <span class="field__hint">{t.recovery_link_token_hint}</span>
                    </div>
                    <dl class="kv-list">
                        <div class="kv-list__row">
                            <dt>{t.recovery_link_expires_label}</dt>
                            <dd>{fmt_time(data.expires_at)}</dd>
                        </div>
                        <div class="kv-list__row">
                            <dt>{t.recovery_link_invalidated_label}</dt>
                            <dd>{data.invalidated.to_string()}</dd>
                        </div>
                    </dl>
                    <p class="muted">{t.recovery_link_handover}</p>
                    <div class="row gap-2 mt-4">
                        <a href=back class="button secondary">{t.recovery_link_back}</a>
                    </div>
                </div>
            </Shell>
        }
    })
}

/// Why no link was issued. The handler maps `RecoveryRefusal` and the
/// no-second-factor case onto this; each has its own message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryRefusedKind {
    ReasonRequired,
    ReasonTooLong,
    ReasonHasControlCharacters,
    TargetUnknown,
    TargetIsSelf,
    TargetIsAdmin,
    TargetNonLocal,
    TargetDisabled,
    TargetDeleted,
    Throttled,
    /// D6: the issuing administrator holds no second factor.
    NeedsSecondFactor,
}

pub struct RecoveryLinkRefusedData {
    pub user_id: String,
    pub kind: RecoveryRefusedKind,
    pub csrf_token: String,
}

pub fn render_recovery_link_refused(
    data: RecoveryLinkRefusedData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let message = match data.kind {
            RecoveryRefusedKind::ReasonRequired => t.recovery_refused_reason_required,
            RecoveryRefusedKind::ReasonTooLong => t.recovery_refused_reason_too_long,
            RecoveryRefusedKind::ReasonHasControlCharacters => t.recovery_refused_reason_control,
            RecoveryRefusedKind::TargetUnknown => t.recovery_refused_target_unknown,
            RecoveryRefusedKind::TargetIsSelf => t.recovery_refused_target_self,
            RecoveryRefusedKind::TargetIsAdmin => t.recovery_refused_target_admin,
            RecoveryRefusedKind::TargetNonLocal => t.recovery_refused_target_non_local,
            RecoveryRefusedKind::TargetDisabled => t.recovery_refused_target_disabled,
            RecoveryRefusedKind::TargetDeleted => t.recovery_refused_target_deleted,
            RecoveryRefusedKind::Throttled => t.recovery_refused_throttled,
            RecoveryRefusedKind::NeedsSecondFactor => t.recovery_refused_needs_second_factor,
        };
        let back = format!("/admin/users/{}", data.user_id);
        view! {
            <Shell title=t.recovery_refused_title.to_string() show_nav=true
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang
                   csrf_token=data.csrf_token.clone()>
                <div class="auth-card max-w-card">
                    <h1>{t.recovery_refused_title}</h1>
                    <p>{message}</p>
                    <p class="muted">{t.recovery_refused_unchanged}</p>
                    <div class="row gap-2 mt-4">
                        <a href=back class="button secondary">{t.recovery_link_back}</a>
                    </div>
                </div>
            </Shell>
        }
    })
}
