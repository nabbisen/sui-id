//! Page renderers for the "confirm" screen domain (RFC 065).

use super::common::*;
use crate::layout::{Chrome, Shell};
use leptos::prelude::*;

fn reversibility_badge(recoverable: bool, t: &'static sui_id_i18n::Strings) -> impl IntoView {
    if recoverable {
        view! {
            <span class="reversibility-badge reversibility-badge--recoverable">
                "✓ " {t.badge_recoverable}
            </span>
        }
        .into_any()
    } else {
        view! {
            <span class="reversibility-badge reversibility-badge--permanent">
                "✗ " {t.badge_not_recoverable}
            </span>
        }
        .into_any()
    }
}

/// Reversibility classification for the confirm-screen badge (RFC 059).
/// `Recoverable` shows the green check; `Irreversible` shows the
/// permanent-action badge. Colour is never the only signal.
pub enum ReversibilityKind {
    Recoverable,
    Irreversible,
}

/// `0.93.0` O6 §4 item 2: "a structured count... a number and an i18n
/// noun, with plural forms from the locale and never built in Rust."
/// `suffix` is everything the sentence says *after* the number
/// (already pluralized for `count` by the locale that produced it) --
/// the number itself is rendered separately, as its own prominent
/// element, never interpolated into a template string here.
pub struct CountedImpact {
    pub count: usize,
    pub suffix: String,
}

/// `0.93.0` O6 §4 item 4: an impact sentence that ends with a link
/// (the unlink page's own sessions note, with its link to disable the
/// account) -- kept apart from `impact_count` since nothing else needs
/// this shape today.
pub struct ImpactLink {
    pub text: String,
    pub href: String,
    pub link_text: String,
}

/// Data driving the shared `confirm_screen` component (RFC 059).
///
/// Every dangerous-action confirm page builds one of these and
/// delegates the body rendering to `confirm_screen`. The caller still
/// owns the Shell wrap (so it can set the right `current=` nav
/// highlight); this struct owns the page body.
pub struct ConfirmScreenData {
    /// Page heading (also used as Shell title at the call site).
    pub title: String,
    /// Visible identity of the action target — username, client name,
    /// or `kid (RS256)` for signing keys. Rendered inside
    /// `<p><strong>...</strong></p>`.
    pub identity: String,
    /// Optional impact description — the page's lead consequence,
    /// rendered at full contrast (`<p class="confirm-impact">`, RFC 144
    /// tokens; `0.93.0` O6 §4: no longer `.muted`). `None` skips the
    /// line entirely — used by the user re-enable case where there is
    /// no destructive impact to warn about.
    pub impact: Option<String>,
    /// A counted consequence, its own prominent figure inside the
    /// impact area (`0.93.0` O6 §4 item 2) — rendered as its own
    /// paragraph, after `impact` if both are present. `None` for a
    /// confirm page whose impact carries no count.
    pub impact_count: Option<CountedImpact>,
    /// An impact consequence that ends in a link (`0.93.0` O6 §4 item
    /// 4: the unlink page's sessions note, with its link to disable the
    /// account) — part of the impact area, not a `notice` footnote.
    /// `None` for every other confirm page.
    pub impact_link: Option<ImpactLink>,
    /// Reversibility badge kind. `None` skips the badge.
    pub badge: Option<ReversibilityKind>,
    /// Optional small-print reversibility note.
    pub reversibility_text: Option<String>,
    /// Form action URL (POST target).
    pub action_url: String,
    /// Pre-resolved CSRF token.
    pub csrf_token: String,
    /// Additional hidden inputs the action needs. For the disable
    /// form, this carries `("disabled", "true"|"false")`. Empty for
    /// delete and reset forms.
    pub extra_hidden: Vec<(String, String)>,
    /// If true, render the disable-reason textarea (RFC 045).
    pub include_reason_field: bool,
    /// The reason is mandatory (RFC 103 D6): the field is marked `required`
    /// and labelled as such. The server enforces it either way.
    pub reason_required: bool,
    /// An extra paragraph under the impact text, for guidance the person
    /// must read before confirming (for example the recovery-link handover).
    pub notice: Option<String>,
    /// Submit button label.
    pub button_label: String,
    /// True → `class="danger"`, false → `class="btn"`. The re-enable
    /// case is the only `btn` path.
    pub button_danger: bool,
    /// Cancel link URL.
    pub cancel_url: String,
}

/// Shared confirm-screen body (RFC 059).
///
/// Five `render_confirm_*` functions previously duplicated this
/// structure. They now all delegate to `confirm_screen`, which
/// guarantees:
///
/// - Identity is always shown in `<p><strong>`.
/// - Reversibility badge is colour + symbol (✓ / ✗), never colour alone.
/// - The `_confirmed=1` hidden input is unconditionally present —
///   callers cannot accidentally forget it.
/// - The cancel button is always present, always `secondary` style.
///
/// The Shell wrap stays at the caller: `current=` must reflect the
/// route's nav highlight, which differs per call site.
pub fn confirm_screen(data: ConfirmScreenData, lang: sui_id_i18n::Locale) -> impl IntoView {
    let t = lang.strings();
    let ConfirmScreenData {
        title,
        identity,
        impact,
        impact_count,
        impact_link,
        badge,
        reversibility_text,
        action_url,
        csrf_token,
        extra_hidden,
        include_reason_field,
        reason_required,
        notice,
        button_label,
        button_danger,
        cancel_url,
    } = data;
    let title_owned = title.clone();
    let badge_view = badge.map(|k| match k {
        ReversibilityKind::Recoverable => reversibility_badge(true, t).into_any(),
        ReversibilityKind::Irreversible => reversibility_badge(false, t).into_any(),
    });
    let extra_inputs: Vec<_> = extra_hidden
        .into_iter()
        .map(|(n, v)| {
            view! { <input type="hidden" name=n value=v /> }
        })
        .collect();
    let reason_block = include_reason_field.then(|| {
        // RFC 144 (Refresh, §3/§5): the field's own label and hint carry
        // the guidance now — no placeholder — and the label says what
        // the reason is for, not whether it is required. N2a review F5:
        // the optional label used to say "for disabling" on all four
        // confirmations that don't disable anything.
        let (label, hint) = if reason_required {
            (t.recovery_reason_label, t.recovery_reason_hint)
        } else {
            (t.optional_reason_label, t.optional_reason_hint)
        };
        view! {
            <div class="field">
                <label for="disable-reason" class="field__label">
                    {label}
                </label>
                <textarea id="disable-reason" name="reason" rows="2" maxlength="200"
                          required=reason_required
                          aria-describedby="disable-reason-hint"></textarea>
                <span id="disable-reason-hint" class="field__hint">{hint}</span>
            </div>
        }
    });
    let button_class = if button_danger { "danger" } else { "btn" };
    view! {
        <div class="auth-card max-w-card">
            <h1>{title_owned}</h1>
            <p><strong>{identity}</strong></p>
            {impact.map(|s| view! { <p class="confirm-impact">{s}</p> })}
            {impact_count.map(|c| view! {
                <p class="confirm-impact">
                    <span class="confirm-impact__count">{c.count.to_string()}</span>
                    " " {c.suffix}
                </p>
            })}
            {impact_link.map(|l| view! {
                <p class="confirm-impact">
                    {l.text} " " <a href=l.href>{l.link_text}</a>
                </p>
            })}
            {notice.map(|s| view! { <p class="muted">{s}</p> })}
            {badge_view.map(|b| view! { <p>{b}</p> })}
            {reversibility_text.map(|s| view! {
                <p class="muted text-caption">{s}</p>
            })}
            <form method="post" action=action_url class="stack mt-4">
                <input type="hidden" name="_csrf" value=csrf_token />
                <input type="hidden" name="_confirmed" value="1" />
                {extra_inputs}
                {reason_block}
                <div class="row gap-2">
                    <button type="submit" class=button_class>{button_label}</button>
                    <a href=cancel_url class="button secondary">{t.confirm_cancel}</a>
                </div>
            </form>
        </div>
    }
}

pub struct ConfirmDisableData {
    pub user_id: String,
    pub username: String,
    pub is_disabled: bool,
    pub csrf_token: String,
    /// RFC 144 (Refresh, §4): the signed-in administrator's own name,
    /// for the account menu — distinct from `username` above, which
    /// names the user being acted on.
    pub admin_username: String,
}

pub fn render_confirm_disable_user(
    data: ConfirmDisableData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let new_state = if data.is_disabled { "false" } else { "true" };
        let (title, impact_opt, rev_opt, btn, btn_danger) = if data.is_disabled {
            (
                t.confirm_enable_title,
                None::<&'static str>,
                None::<&'static str>,
                t.confirm_enable_button,
                false,
            )
        } else {
            (
                t.confirm_disable_title,
                Some(t.confirm_disable_impact),
                Some(t.confirm_disable_reversibility),
                t.confirm_disable_button,
                true,
            )
        };
        let admin_username = data.admin_username.clone();
        let body = confirm_screen(
            ConfirmScreenData {
                title: title.into(),
                identity: data.username.clone(),
                impact: impact_opt.map(|s| s.into()),
                impact_count: None,
                impact_link: None,
                badge: (!data.is_disabled).then_some(ReversibilityKind::Recoverable),
                reversibility_text: rev_opt.map(|s| s.into()),
                action_url: format!("/admin/users/{}/disabled", data.user_id),
                csrf_token: data.csrf_token.clone(),
                extra_hidden: vec![("disabled".into(), new_state.into())],
                include_reason_field: !data.is_disabled,
                reason_required: false,
                notice: None,
                button_label: btn.into(),
                button_danger: btn_danger,
                cancel_url: "/admin/users".into(),
            },
            lang,
        );
        view! {
            <Shell title=title.to_string() nav=Chrome::Console{username: admin_username}
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang csrf_token=data.csrf_token.clone()>
                {body}
            </Shell>
        }
    })
}

pub struct ConfirmDeleteUserData {
    pub user_id: String,
    pub username: String,
    pub csrf_token: String,
    /// RFC 144 (Refresh, §4): the signed-in administrator's own name,
    /// for the account menu — distinct from `username` above.
    pub admin_username: String,
}

pub fn render_confirm_delete_user(
    data: ConfirmDeleteUserData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let admin_username = data.admin_username.clone();
        let body = confirm_screen(
            ConfirmScreenData {
                title: t.confirm_delete_user_title.into(),
                identity: data.username.clone(),
                impact: Some(t.confirm_delete_user_impact.into()),
                impact_count: None,
                impact_link: None,
                badge: Some(ReversibilityKind::Irreversible),
                reversibility_text: Some(t.confirm_delete_user_reversibility.into()),
                action_url: format!("/admin/users/{}/delete", data.user_id),
                csrf_token: data.csrf_token.clone(),
                extra_hidden: vec![],
                include_reason_field: true,
                reason_required: false,
                notice: None,
                button_label: t.confirm_delete_user_button.into(),
                button_danger: true,
                cancel_url: "/admin/users".into(),
            },
            lang,
        );
        view! {
            <Shell title=t.confirm_delete_user_title.to_string() nav=Chrome::Console{username: admin_username}
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang csrf_token=data.csrf_token.clone()>
                {body}
            </Shell>
        }
    })
}

pub struct ConfirmResetMfaData {
    pub user_id: String,
    pub username: String,
    pub csrf_token: String,
    /// RFC 144 (Refresh, §4): the signed-in administrator's own name,
    /// for the account menu — distinct from `username` above.
    pub admin_username: String,
}

pub fn render_confirm_reset_mfa(
    data: ConfirmResetMfaData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let admin_username = data.admin_username.clone();
        let body = confirm_screen(
            ConfirmScreenData {
                title: t.confirm_reset_mfa_title.into(),
                identity: data.username.clone(),
                impact: Some(t.confirm_reset_mfa_impact.into()),
                impact_count: None,
                impact_link: None,
                badge: Some(ReversibilityKind::Recoverable),
                reversibility_text: Some(t.confirm_reset_mfa_reversibility.into()),
                action_url: format!("/admin/users/{}/mfa-reset", data.user_id),
                csrf_token: data.csrf_token.clone(),
                extra_hidden: vec![],
                include_reason_field: true,
                reason_required: false,
                notice: None,
                button_label: t.confirm_reset_mfa_button.into(),
                button_danger: true,
                cancel_url: "/admin/users".into(),
            },
            lang,
        );
        view! {
            <Shell title=t.confirm_reset_mfa_title.to_string() nav=Chrome::Console{username: admin_username}
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang csrf_token=data.csrf_token.clone()>
                {body}
            </Shell>
        }
    })
}

/// The confirm screen for issuing an account-recovery link (RFC 103 D6): the
/// same template as the other dangerous operations, with the reason required
/// and the handover guidance (D11) shown before the button.
pub struct ConfirmRecoveryLinkData {
    pub user_id: String,
    pub username: String,
    pub csrf_token: String,
    /// RFC 144 (Refresh, §4): the signed-in administrator's own name,
    /// for the account menu — distinct from `username` above.
    pub admin_username: String,
}

pub fn render_confirm_recovery_link(
    data: ConfirmRecoveryLinkData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let admin_username = data.admin_username.clone();
        let body = confirm_screen(
            ConfirmScreenData {
                title: t.confirm_recovery_link_title.into(),
                identity: data.username.clone(),
                impact: Some(t.confirm_recovery_link_impact.into()),
                impact_count: None,
                impact_link: None,
                badge: Some(ReversibilityKind::Irreversible),
                reversibility_text: Some(t.confirm_recovery_link_reversibility.into()),
                action_url: format!("/admin/users/{}/recovery-link", data.user_id),
                csrf_token: data.csrf_token.clone(),
                extra_hidden: vec![],
                include_reason_field: true,
                reason_required: true,
                notice: Some(t.recovery_link_handover.into()),
                button_label: t.confirm_recovery_link_button.into(),
                button_danger: true,
                cancel_url: format!("/admin/users/{}", data.user_id),
            },
            lang,
        );
        view! {
            <Shell title=t.confirm_recovery_link_title.to_string() nav=Chrome::Console{username: admin_username}
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang csrf_token=data.csrf_token.clone()>
                {body}
            </Shell>
        }
    })
}

pub struct ConfirmDeleteClientData {
    pub client_id: String,
    pub client_name: String,
    pub csrf_token: String,
    /// RFC 144 (Refresh, §4): the signed-in administrator's name, for
    /// the account menu.
    pub admin_username: String,
}

pub fn render_confirm_delete_client(
    data: ConfirmDeleteClientData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let admin_username = data.admin_username.clone();
        let body = confirm_screen(
            ConfirmScreenData {
                title: t.confirm_delete_client_title.into(),
                identity: data.client_name.clone(),
                impact: Some(t.confirm_delete_client_impact.into()),
                impact_count: None,
                impact_link: None,
                badge: Some(ReversibilityKind::Irreversible),
                reversibility_text: Some(t.confirm_delete_client_reversibility.into()),
                action_url: format!("/admin/clients/{}/delete", data.client_id),
                csrf_token: data.csrf_token.clone(),
                extra_hidden: vec![],
                include_reason_field: true,
                reason_required: false,
                notice: None,
                button_label: t.confirm_delete_client_button.into(),
                button_danger: true,
                cancel_url: "/admin/clients".into(),
            },
            lang,
        );
        view! {
            <Shell title=t.confirm_delete_client_title.to_string() nav=Chrome::Console{username: admin_username}
                   current=Some("clients".to_string()) dev_mode=dev_mode lang=lang csrf_token=data.csrf_token.clone()>
                {body}
            </Shell>
        }
    })
}

pub struct ConfirmDeleteSigningKeyData {
    pub key_id: String,
    pub algorithm: String,
    pub csrf_token: String,
    /// RFC 144 (Refresh, §4): the signed-in administrator's name, for
    /// the account menu.
    pub admin_username: String,
}

pub fn render_confirm_delete_signing_key(
    data: ConfirmDeleteSigningKeyData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let identity = format!("{} ({})", data.key_id, data.algorithm);
        let admin_username = data.admin_username.clone();
        let body = confirm_screen(
            ConfirmScreenData {
                title: t.confirm_delete_signing_key_title.into(),
                identity,
                impact: Some(t.confirm_delete_signing_key_impact.into()),
                impact_count: None,
                impact_link: None,
                badge: Some(ReversibilityKind::Irreversible),
                reversibility_text: Some(t.confirm_delete_signing_key_reversibility.into()),
                action_url: format!("/admin/signing-keys/{}/delete", data.key_id),
                csrf_token: data.csrf_token.clone(),
                extra_hidden: vec![],
                include_reason_field: true,
                reason_required: false,
                notice: None,
                button_label: t.confirm_delete_signing_key_button.into(),
                button_danger: true,
                cancel_url: "/admin/signing-keys".into(),
            },
            lang,
        );
        view! {
            <Shell title=t.confirm_delete_signing_key_title.to_string() nav=Chrome::Console{username: admin_username}
                   current=Some("signing_keys".to_string()) dev_mode=dev_mode lang=lang csrf_token=data.csrf_token.clone()>
                {body}
            </Shell>
        }
    })
}

// ── RFC 090: Signing-key rotation confirm ────────────────────────────────────

pub struct ConfirmRotateSigningKeyData {
    pub csrf_token: String,
    pub active_key_count: usize,
    /// RFC 144 (Refresh, §4): the signed-in administrator's name, for
    /// the account menu.
    pub admin_username: String,
}

pub fn render_confirm_rotate_signing_key(
    data: ConfirmRotateSigningKeyData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        // `0.93.0` O6 §4 item 3: the count's own sentence no longer
        // builds English text in Rust (`"({} active key{} will become
        // retired)"`) -- the locale's own pluralized suffix does, and
        // the number is rendered as its own prominent element, not
        // parenthesized into the lead sentence.
        let impact = if data.active_key_count == 0 {
            format!(
                "{} {}",
                t.confirm_rotate_signing_key_impact, t.confirm_rotate_signing_key_count_zero
            )
        } else {
            t.confirm_rotate_signing_key_impact.to_owned()
        };
        let impact_count = (data.active_key_count > 0).then(|| CountedImpact {
            count: data.active_key_count,
            suffix: (t.confirm_rotate_signing_key_count_suffix)(data.active_key_count),
        });
        let admin_username = data.admin_username.clone();
        let body = confirm_screen(
            ConfirmScreenData {
                title: t.confirm_rotate_signing_key_title.into(),
                identity: String::new(),
                impact: Some(impact),
                impact_count,
                impact_link: None,
                badge: Some(ReversibilityKind::Irreversible),
                reversibility_text: Some(t.confirm_rotate_signing_key_reversibility.into()),
                action_url: "/admin/signing-keys/rotate".into(),
                csrf_token: data.csrf_token.clone(),
                extra_hidden: vec![],
                include_reason_field: false,
                reason_required: false,
                notice: None,
                button_label: t.confirm_rotate_signing_key_button.into(),
                button_danger: true,
                cancel_url: "/admin/signing-keys".into(),
            },
            lang,
        );
        view! {
            <Shell title=t.confirm_rotate_signing_key_title.to_string() nav=Chrome::Console{username: admin_username}
                   current=Some("signing_keys".to_string()) dev_mode=dev_mode lang=lang csrf_token=data.csrf_token.clone()>
                {body}
            </Shell>
        }
    })
}

// ── RFC 090: SMTP settings pending-change confirm ─────────────────────────────

pub struct SettingsEmailConfirmData {
    /// The opaque pending-change id (safe to include in the form).
    pub pending_change_id: String,
    /// Non-secret human-readable description of the pending change.
    pub summary: String,
    /// CSRF token for the confirm POST.
    pub csrf_token: String,
    /// RFC 144 (Refresh, §4): the signed-in administrator's name, for
    /// the account menu.
    pub admin_username: String,
}

pub fn render_settings_email_confirm(
    data: SettingsEmailConfirmData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let admin_username = data.admin_username.clone();
        let body = confirm_screen(
            ConfirmScreenData {
                title: t.confirm_email_settings_title.into(),
                identity: data.summary.clone(),
                impact: Some(t.confirm_email_settings_impact.into()),
                impact_count: None,
                impact_link: None,
                badge: None,
                reversibility_text: None,
                action_url: "/admin/settings/email/confirm".into(),
                csrf_token: data.csrf_token.clone(),
                extra_hidden: vec![("pending_change_id".into(), data.pending_change_id.clone())],
                include_reason_field: false,
                reason_required: false,
                notice: None,
                button_label: t.confirm_email_settings_button.into(),
                button_danger: false,
                cancel_url: "/admin/settings/email".into(),
            },
            lang,
        );
        view! {
            <Shell title=t.confirm_email_settings_title.to_string() nav=Chrome::Console{username: admin_username}
                   current=Some("settings".to_string()) dev_mode=dev_mode lang=lang csrf_token=data.csrf_token.clone()>
                {body}
            </Shell>
        }
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests;
