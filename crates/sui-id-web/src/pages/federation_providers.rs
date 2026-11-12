//! Page renderers for the "federation_providers" screen domain
//! (RFC 096-B2 S5).
//!
//! RFC 096 `:380`: "admin UI may enable or disable a provider but may
//! not mutate trust fields." No form on any page here carries an
//! `<input>`/`<select>` for a trust field — [`sui_id_shared::api::
//! FederationProviderSummary`] itself does not even carry one.

use super::common::*;
use crate::components::empty_state;
use leptos::prelude::*;
use sui_id_shared::api::FederationProviderSummary;

use crate::layout::Shell;

fn provider_row_view(
    can_write: bool,
    p: FederationProviderSummary,
    t: &'static sui_id_i18n::Strings,
) -> impl IntoView {
    let state_text = if p.enabled {
        t.federation_providers_state_enabled
    } else {
        t.federation_providers_state_disabled
    };
    let review_text = if p.requires_review {
        t.federation_providers_review_requires_review
    } else {
        t.federation_providers_review_validated
    };
    let last_checked = p
        .last_checked_at
        .map(fmt_time)
        .unwrap_or_else(|| t.federation_providers_never_checked.to_string());
    let linked = if p.has_linked_users {
        t.federation_providers_linked_yes
    } else {
        t.federation_providers_linked_no
    };
    let id_str = p.id.to_string();
    // RFC 096 `:380`: every action is admin-only; an auditor's row ends
    // after `linked`, with no action cell at all.
    let actions = if !can_write {
        view! { <td></td> }.into_any()
    } else if p.enabled {
        view! {
            <td>
                <a href=format!("/admin/federation-providers/{id_str}/disable-confirm")
                   class="button danger">
                    {t.federation_providers_disable_button}
                </a>
            </td>
        }
        .into_any()
    } else if p.is_configured {
        view! {
            <td>
                <a href=format!("/admin/federation-providers/{id_str}/enable-confirm")
                   class="button">
                    {t.federation_providers_check_and_enable_button}
                </a>
            </td>
        }
        .into_any()
    } else {
        view! {
            <td><span class="muted">{t.federation_providers_not_configured_note}</span></td>
        }
        .into_any()
    };
    view! {
        <tr>
            <td>{p.display_name}</td>
            <td class="muted">{p.issuer}</td>
            <td>{state_text}</td>
            <td>{review_text}</td>
            <td class="muted">{last_checked}</td>
            <td>{linked}</td>
            {actions}
        </tr>
    }
}

pub fn render_federation_providers(
    can_write: bool,
    providers: Vec<FederationProviderSummary>,
    flash: Option<Flash>,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let rows: Vec<_> = providers
            .into_iter()
            .map(|p| provider_row_view(can_write, p, t))
            .collect();
        let has_rows = !rows.is_empty();
        view! {
            <Shell
                title=t.federation_providers_title.to_string()
                show_nav=true
                current=Some("federation-providers".to_string())
                dev_mode=dev_mode
                lang=lang
                csrf_token=csrf_token.clone()
            >
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.federation_providers_title}</h1>
                        <p class="page-header__lede">{t.federation_providers_lede}</p>
                    </div>
                </header>
                {flash_banner(flash)}
                <section>
                    <div class="table-wrap">
                        {if has_rows {
                            view! {
                                <table>
                                    <thead>
                                        <tr>
                                            <th>{t.federation_providers_th_name}</th>
                                            <th>{t.federation_providers_th_issuer}</th>
                                            <th>{t.federation_providers_th_state}</th>
                                            <th>{t.federation_providers_th_review}</th>
                                            <th>{t.federation_providers_th_last_checked}</th>
                                            <th>{t.federation_providers_th_linked_users}</th>
                                            <th></th>
                                        </tr>
                                    </thead>
                                    <tbody>{rows}</tbody>
                                </table>
                            }.into_any()
                        } else {
                            empty_state(t.empty_federation_providers, None).into_any()
                        }}
                    </div>
                </section>
            </Shell>
        }
    })
}

// ── Enable confirm (check + enable) ─────────────────────────────────────

pub struct ConfirmEnableFederationProviderData {
    pub provider_id: String,
    pub display_name: String,
    pub csrf_token: String,
    /// Already built by the caller (the handler, which alone knows how
    /// to map `CheckAndEnableError`/`PreflightError` to a translated
    /// message, or the "Check only" success text) — `None` on the
    /// first, unrefused visit to this page.
    pub flash: Option<Flash>,
}

pub fn render_confirm_enable_federation_provider(
    data: ConfirmEnableFederationProviderData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_enable = data.csrf_token.clone();
        let csrf_for_check = data.csrf_token.clone();
        let enable_url = format!("/admin/federation-providers/{}/enable", data.provider_id);
        let check_url = format!("/admin/federation-providers/{}/check", data.provider_id);
        let cancel_url = "/admin/federation-providers".to_string();
        let flash = data.flash;
        view! {
            <Shell
                title=t.federation_providers_enable_confirm_title.to_string()
                show_nav=true
                current=Some("federation-providers".to_string())
                dev_mode=dev_mode
                lang=lang
                csrf_token=data.csrf_token.clone()
            >
                <div class="auth-card max-w-card">
                    <h1>{t.federation_providers_enable_confirm_title}</h1>
                    <p><strong>{data.display_name}</strong></p>
                    <p class="muted">{t.federation_providers_enable_confirm_body}</p>
                    {flash_banner(flash)}
                    <div class="row gap-2 mt-4">
                        <form method="post" action=enable_url>
                            <input type="hidden" name="_csrf" value=csrf_for_enable />
                            <input type="hidden" name="_confirmed" value="1" />
                            <button type="submit">
                                {t.federation_providers_check_and_enable_button}
                            </button>
                        </form>
                        <form method="post" action=check_url>
                            <input type="hidden" name="_csrf" value=csrf_for_check />
                            <button type="submit" class="button secondary">
                                {t.federation_providers_check_only_button}
                            </button>
                        </form>
                        <a href=cancel_url class="button secondary">{t.confirm_cancel}</a>
                    </div>
                </div>
            </Shell>
        }
    })
}

// ── Disable confirm ──────────────────────────────────────────────────────

pub struct ConfirmDisableFederationProviderData {
    pub provider_id: String,
    pub display_name: String,
    pub csrf_token: String,
    pub flash: Option<Flash>,
}

pub fn render_confirm_disable_federation_provider(
    data: ConfirmDisableFederationProviderData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let disable_url = format!("/admin/federation-providers/{}/disable", data.provider_id);
        let cancel_url = "/admin/federation-providers".to_string();
        let flash = data.flash;
        view! {
            <Shell
                title=t.federation_providers_disable_confirm_title.to_string()
                show_nav=true
                current=Some("federation-providers".to_string())
                dev_mode=dev_mode
                lang=lang
                csrf_token=data.csrf_token.clone()
            >
                <div class="auth-card max-w-card">
                    <h1>{t.federation_providers_disable_confirm_title}</h1>
                    <p><strong>{data.display_name}</strong></p>
                    <p class="muted">{t.federation_providers_disable_confirm_body}</p>
                    {flash_banner(flash)}
                    <form method="post" action=disable_url class="stack mt-4">
                        <input type="hidden" name="_csrf" value=data.csrf_token.clone() />
                        <input type="hidden" name="_confirmed" value="1" />
                        <div class="row gap-2">
                            <button type="submit" class="danger">
                                {t.federation_providers_disable_button}
                            </button>
                            <a href=cancel_url class="button secondary">{t.confirm_cancel}</a>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}
