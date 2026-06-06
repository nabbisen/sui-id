//! Page renderers for the "confirm" screen domain (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::common::*;

fn reversibility_badge(recoverable: bool, t: &'static sui_id_i18n::Strings) -> impl IntoView {
    if recoverable {
        view! {
            <span class="reversibility-badge reversibility-badge--recoverable">
                "✓ " {t.badge_recoverable}
            </span>
        }.into_any()
    } else {
        view! {
            <span class="reversibility-badge reversibility-badge--permanent">
                "✗ " {t.badge_not_recoverable}
            </span>
        }.into_any()
    }
}

/// Reversibility classification for the confirm-screen badge (RFC 059).
/// `Recoverable` shows the green check; `Irreversible` shows the
/// permanent-action badge. Colour is never the only signal.

pub enum ReversibilityKind { Recoverable, Irreversible }

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
    /// Optional impact description (`<p class="muted">`).
    /// `None` skips the line entirely — used by the user re-enable
    /// case where there is no destructive impact to warn about.
    pub impact: Option<String>,
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

pub fn confirm_screen(
    data: ConfirmScreenData,
    lang: sui_id_i18n::Locale,
) -> impl IntoView {
    let t = lang.strings();
    let ConfirmScreenData {
        title,
        identity,
        impact,
        badge,
        reversibility_text,
        action_url,
        csrf_token,
        extra_hidden,
        include_reason_field,
        button_label,
        button_danger,
        cancel_url,
    } = data;
    let title_owned = title.clone();
    let badge_view = badge.map(|k| match k {
        ReversibilityKind::Recoverable => reversibility_badge(true, t).into_any(),
        ReversibilityKind::Irreversible => reversibility_badge(false, t).into_any(),
    });
    let extra_inputs: Vec<_> = extra_hidden.into_iter().map(|(n, v)| {
        view! { <input type="hidden" name=n value=v /> }
    }).collect();
    let reason_block = include_reason_field.then(|| view! {
        <div class="field">
            <label for="disable-reason" class="field__label">
                {t.disable_reason_label}
            </label>
            <textarea id="disable-reason" name="reason" rows="2" maxlength="200"
                      placeholder=t.disable_reason_placeholder></textarea>
            <span class="field__hint">{t.disable_reason_hint}</span>
        </div>
    });
    let button_class = if button_danger { "danger" } else { "btn" };
    view! {
        <div class="auth-card max-w-card">
            <h1>{title_owned}</h1>
            <p><strong>{identity}</strong></p>
            {impact.map(|s| view! { <p class="muted">{s}</p> })}
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
            (t.confirm_enable_title, None::<&'static str>, None::<&'static str>,
             t.confirm_enable_button, false)
        } else {
            (t.confirm_disable_title,
             Some(t.confirm_disable_impact),
             Some(t.confirm_disable_reversibility),
             t.confirm_disable_button,
             true)
        };
        let body = confirm_screen(ConfirmScreenData {
            title: title.into(),
            identity: data.username.clone(),
            impact: impact_opt.map(|s| s.into()),
            badge: (!data.is_disabled).then_some(ReversibilityKind::Recoverable),
            reversibility_text: rev_opt.map(|s| s.into()),
            action_url: format!("/admin/users/{}/disabled", data.user_id),
            csrf_token: data.csrf_token.clone(),
            extra_hidden: vec![("disabled".into(), new_state.into())],
            include_reason_field: !data.is_disabled,
            button_label: btn.into(),
            button_danger: btn_danger,
            cancel_url: "/admin/users".into(),
        }, lang);
        view! {
            <Shell title=title.to_string() show_nav=true
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang>
                {body}
            </Shell>
        }
    })
}


pub struct ConfirmDeleteUserData {
    pub user_id: String,
    pub username: String,
    pub csrf_token: String,
}


pub fn render_confirm_delete_user(
    data: ConfirmDeleteUserData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let body = confirm_screen(ConfirmScreenData {
            title: t.confirm_delete_user_title.into(),
            identity: data.username.clone(),
            impact: Some(t.confirm_delete_user_impact.into()),
            badge: Some(ReversibilityKind::Irreversible),
            reversibility_text: Some(t.confirm_delete_user_reversibility.into()),
            action_url: format!("/admin/users/{}/delete", data.user_id),
            csrf_token: data.csrf_token.clone(),
            extra_hidden: vec![],
            include_reason_field: true,
            button_label: t.confirm_delete_user_button.into(),
            button_danger: true,
            cancel_url: "/admin/users".into(),
        }, lang);
        view! {
            <Shell title=t.confirm_delete_user_title.to_string() show_nav=true
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang>
                {body}
            </Shell>
        }
    })
}


pub struct ConfirmResetMfaData {
    pub user_id: String,
    pub username: String,
    pub csrf_token: String,
}


pub fn render_confirm_reset_mfa(
    data: ConfirmResetMfaData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let body = confirm_screen(ConfirmScreenData {
            title: t.confirm_reset_mfa_title.into(),
            identity: data.username.clone(),
            impact: Some(t.confirm_reset_mfa_impact.into()),
            badge: Some(ReversibilityKind::Recoverable),
            reversibility_text: Some(t.confirm_reset_mfa_reversibility.into()),
            action_url: format!("/admin/users/{}/mfa-reset", data.user_id),
            csrf_token: data.csrf_token.clone(),
            extra_hidden: vec![],
            include_reason_field: true,
            button_label: t.confirm_reset_mfa_button.into(),
            button_danger: true,
            cancel_url: "/admin/users".into(),
        }, lang);
        view! {
            <Shell title=t.confirm_reset_mfa_title.to_string() show_nav=true
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang>
                {body}
            </Shell>
        }
    })
}


pub struct ConfirmDeleteClientData {
    pub client_id: String,
    pub client_name: String,
    pub csrf_token: String,
}


pub fn render_confirm_delete_client(
    data: ConfirmDeleteClientData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let body = confirm_screen(ConfirmScreenData {
            title: t.confirm_delete_client_title.into(),
            identity: data.client_name.clone(),
            impact: Some(t.confirm_delete_client_impact.into()),
            badge: Some(ReversibilityKind::Irreversible),
            reversibility_text: Some(t.confirm_delete_client_reversibility.into()),
            action_url: format!("/admin/clients/{}/delete", data.client_id),
            csrf_token: data.csrf_token.clone(),
            extra_hidden: vec![],
            include_reason_field: true,
            button_label: t.confirm_delete_client_button.into(),
            button_danger: true,
            cancel_url: "/admin/clients".into(),
        }, lang);
        view! {
            <Shell title=t.confirm_delete_client_title.to_string() show_nav=true
                   current=Some("clients".to_string()) dev_mode=dev_mode lang=lang>
                {body}
            </Shell>
        }
    })
}


pub struct ConfirmDeleteSigningKeyData {
    pub key_id: String,
    pub algorithm: String,
    pub csrf_token: String,
}


pub fn render_confirm_delete_signing_key(
    data: ConfirmDeleteSigningKeyData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let identity = format!("{} ({})", data.key_id, data.algorithm);
        let body = confirm_screen(ConfirmScreenData {
            title: t.confirm_delete_signing_key_title.into(),
            identity,
            impact: Some(t.confirm_delete_signing_key_impact.into()),
            badge: Some(ReversibilityKind::Irreversible),
            reversibility_text: Some(t.confirm_delete_signing_key_reversibility.into()),
            action_url: format!("/admin/signing-keys/{}/delete", data.key_id),
            csrf_token: data.csrf_token.clone(),
            extra_hidden: vec![],
            include_reason_field: true,
            button_label: t.confirm_delete_signing_key_button.into(),
            button_danger: true,
            cancel_url: "/admin/signing-keys".into(),
        }, lang);
        view! {
            <Shell title=t.confirm_delete_signing_key_title.to_string() show_nav=true
                   current=Some("signing_keys".to_string()) dev_mode=dev_mode lang=lang>
                {body}
            </Shell>
        }
    })
}
