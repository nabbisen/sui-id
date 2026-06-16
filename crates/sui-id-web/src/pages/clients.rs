//! Page renderers for the "clients" screen domain (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::common::*;
use sui_id_shared::api::ClientSummary;

fn client_row_view(
    t: &'static sui_id_i18n::Strings,
    c: ClientSummary,
    csrf: String,
) -> impl IntoView {
    let is_disabled = c.is_disabled;
    let is_deleted = c.is_deleted;
    let kind = if c.confidential { "confidential" } else { "public" };
    let id_str = c.id.to_string();
    let action_label = if is_disabled { "Enable" } else { "Disable" };
    let action_target = if is_disabled { "false" } else { "true" };
    let disabled_url = format!("/admin/clients/{id_str}/disabled");
    let csrf_disable = csrf.clone();
    let scopes_display = if c.allowed_scopes.trim().is_empty() {
        t.empty_any.to_string()
    } else {
        c.allowed_scopes.clone()
    };
    let logout_count = c.post_logout_redirect_uris.len();
    let logout_display = if logout_count == 0 {
        t.empty_falls_back_redirect_uris.to_string()
    } else {
        format!("{logout_count} URI(s)")
    };

    let status_view = if is_deleted {
        crate::components::status_badge(t, crate::components::StatusKind::Deleted).into_any()
    } else if is_disabled {
        crate::components::status_badge(t, crate::components::StatusKind::Disabled).into_any()
    } else {
        crate::components::status_badge(t, crate::components::StatusKind::Active).into_any()
    };

    let edit_url = format!("/admin/clients/{id_str}/edit");
    let actions = if is_deleted {
        view! { <td><span class="muted">{t.empty_dash}</span></td> }.into_any()
    } else {
        view! {
            <td>
                <div class="row gap-1">
                    <a href=edit_url class="button secondary">"Edit"</a>
                    <form method="post" action=disabled_url class="inline-el">
                        <input type="hidden" name="_csrf" value=csrf_disable />
                        <input type="hidden" name="disabled" value=action_target />
                        <button type="submit" class="secondary">{action_label}</button>
                    </form>
                    <a href=format!("/admin/clients/{}/delete-confirm", id_str.clone()) class="button danger">"Delete"</a>
                </div>
            </td>
        }
        .into_any()
    };

    let id_for_copy = id_str.clone();
    view! {
        <tr>
            <td>{c.name}</td>
            <td>
                <span class="code">{id_str}</span>
                {copy_btn(t, id_for_copy, t.copy_noun_client_id)}
            </td>
            <td>{kind}</td>
            <td><span class="code">{scopes_display}</span></td>
            <td class="muted">{logout_display}</td>
            <td>{status_view}</td>
            {actions}
        </tr>
    }
}


pub fn render_clients(
    clients: Vec<ClientSummary>,
    flash: Option<Flash>,
    new_secret: Option<(String, String)>,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_rows = csrf_token.clone();
        let csrf_for_form = csrf_token.clone();
        let client_count = clients.len();
        let secret_block = new_secret.map(|(cid, sec)| {
            view! {
                <div class="flash warn" role="status">
                    <div class="stack-tight">
                        <strong>{t.clients_secret_once_banner}</strong>
                        <div>"Client ID: "<span class="code">{cid.clone()}</span>{copy_btn(t, cid, t.copy_noun_client_id)}</div>
                        <div>"Client Secret: "<span class="code">{sec.clone()}</span>{copy_btn(t, sec, t.copy_noun_client_secret)}</div>
                    </div>
                </div>
            }
        });
        let rows: Vec<_> = clients
            .into_iter()
            .map(|c| client_row_view(t, c, csrf_for_rows.clone()))
            .collect();
        view! {
            <Shell title=t.clients_title.to_string() show_nav=true current=Some("clients".to_string()) dev_mode=dev_mode lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.clients_title}</h1>
                        <p class="page-header__lede">
                            {t.clients_lede}
                            " "
                            {(t.clients_count_caption)(client_count)}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {secret_block}

                <section>
                    <h2>{t.clients_create_section}</h2>
                    <div class="card">
                        <form method="post" action="/admin/clients" class="stack">
                            <input type="hidden" name="_csrf" value=csrf_for_form />
                            <div class="field">
                                <label for="c-name" class="field__label">{t.clients_label_app_name}</label>
                                <input id="c-name" name="name" type="text" required=true />
                            </div>
                            <div class="field">
                                <label for="c-uris" class="field__label">{t.clients_label_redirect_uris}</label>
                                <textarea id="c-uris" name="redirect_uris" required=true rows="3"></textarea>
                                <span class="field__hint">{t.clients_hint_redirect_uris}</span>
                            </div>
                            <div class="field">
                                <label for="c-scopes" class="field__label">{t.clients_label_allowed_scopes}</label>
                                <input id="c-scopes" name="allowed_scopes" type="text" value="openid profile email" />
                                <span class="field__hint">
                                    {t.clients_hint_scopes_intro}
                                    <code>"openid"</code>{t.clients_hint_scopes_openid_note}
                                    <code>"profile"</code>{t.clients_hint_scopes_profile_note}
                                    <code>"email"</code>{t.clients_hint_scopes_email_note}
                                    <code>"offline_access"</code>{t.clients_hint_scopes_offline_note}
                                    {t.clients_hint_scopes_default}
                                </span>
                            </div>
                            // Single-realm note (RFC 027) — now via clients_single_realm_note key
                            <p class="field__hint mb-0">
                                "ℹ  "
                                {t.clients_single_realm_note}
                            </p>
                            <div class="field">
                                <label for="c-logout" class="field__label">{t.clients_label_post_logout_uris}</label>
                                <textarea id="c-logout" name="post_logout_redirect_uris" rows="2"></textarea>
                                <span class="field__hint">{t.clients_hint_one_per_line}</span>
                            </div>
                            <label class="row gap-2">
                                <input name="confidential" type="checkbox" value="true" checked=true />
                                <span>{t.clients_label_confidential_checkbox}</span>
                            </label>
                            <div>
                                <button type="submit">{t.clients_button_register}</button>
                            </div>
                        </form>
                    </div>
                </section>

                <section>
                    <h2>{t.clients_table_section}</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.clients_table_th_name}</th>
                                    <th>{t.clients_table_th_client_id}</th>
                                    <th>{t.clients_table_th_kind}</th>
                                    <th>{t.clients_table_th_scopes}</th>
                                    <th>{t.clients_table_th_logout}</th>
                                    <th>{t.clients_table_th_status}</th>
                                    <th></th>
                                </tr>
                            </thead>
                            {if rows.is_empty() {
                                view! {
                                    <tbody>{table_empty_row(t.clients_empty, 7)}</tbody>
                                }.into_any()
                            } else {
                                view! { <tbody>{rows}</tbody> }.into_any()
                            }}
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}

// ---------- client edit ----------


pub struct ClientEditData {
    pub id: String,
    pub name: String,
    /// Newline-separated for textarea editing.
    pub redirect_uris: Vec<String>,
    /// Space-separated.
    pub allowed_scopes: String,
    pub post_logout_redirect_uris: Vec<String>,
    pub confidential: bool,
    pub is_disabled: bool,
    /// RFC 038: "none", "first_time", or "always"
    pub consent_policy: String,
    /// RFC 047: populated only when the secret was just rotated. Shown once.
    pub freshly_rotated_secret: Option<String>,
}


pub fn render_client_edit(
    data: ClientEditData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let ClientEditData {
            id,
            name,
            redirect_uris,
            allowed_scopes,
            post_logout_redirect_uris,
            confidential,
            is_disabled,
            consent_policy,
            freshly_rotated_secret,
        } = data;
        let post_url = format!("/admin/clients/{id}/edit");
        let kind = if confidential { "confidential" } else { "public" };
        let redirect_uris_value = redirect_uris.join("\n");
        let post_logout_value = post_logout_redirect_uris.join("\n");

        let status_view = if is_disabled {
            crate::components::status_badge(t, crate::components::StatusKind::Disabled).into_any()
        } else {
            crate::components::status_badge(t, crate::components::StatusKind::Active).into_any()
        };

        view! {
            <Shell title=t.client_edit_title.to_string() show_nav=true current=Some("clients".to_string()) lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.client_edit_title}</h1>
                        <p class="page-header__lede">{name.clone()}</p>
                    </div>
                </header>
                {flash_banner(flash)}
                {freshly_rotated_secret.map(|sec| {
                    let sec2 = sec.clone();
                    view! {
                        <div class="banner banner--warning mb-3" role="alert">
                            <strong>{t.client_edit_new_secret_label}</strong>
                            <span class="code ml-2">{sec}</span>
                            {copy_btn(t, sec2, t.copy_noun_client_secret)}
                        </div>
                    }
                })}

                <div class="card">
                    <h3 class="card__title">{t.client_edit_basic_section}</h3>
                    <div class="stack-tight muted">
                        <div>{t.client_edit_label_client_id}": "<span class="code">{id.clone()}</span>{copy_btn(t, id.clone(), t.copy_noun_client_id)}</div>
                        <div class="row gap-2">
                            <span>{t.client_edit_label_kind}":"</span>
                            <span class="badge badge--accent">{kind}</span>
                            <span>{t.client_edit_label_status}":"</span>
                            {status_view}
                        </div>
                    </div>
                    <p class="muted mt-3">
                        {t.client_edit_basic_note}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_title}</h3>
                    <form method="post" action=post_url class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />
                        <div class="field">
                            <label for="e-name" class="field__label">{t.clients_label_app_name}</label>
                            <input id="e-name" name="name" type="text" required=true value=name />
                        </div>
                        <div class="field">
                            <label for="e-uris" class="field__label">{t.clients_label_redirect_uris}</label>
                            <textarea id="e-uris" name="redirect_uris" required=true rows="3">
                                {redirect_uris_value}
                            </textarea>
                            <span class="field__hint">{t.clients_hint_redirect_uris}</span>
                        </div>
                        <div class="field">
                            <label for="e-scopes" class="field__label">{t.clients_label_allowed_scopes}</label>
                            <input id="e-scopes" name="allowed_scopes" type="text" value=allowed_scopes />
                            <span class="field__hint">
                                {t.clients_hint_scopes_intro}
                                <code>"openid"</code>" · "
                                <code>"profile"</code>" · "
                                <code>"email"</code>" · "
                                <code>"offline_access"</code>"。"
                            </span>
                        </div>
                        <div class="field">
                            <label for="e-logout" class="field__label">{t.clients_label_post_logout_uris}</label>
                            <textarea id="e-logout" name="post_logout_redirect_uris" rows="2">
                                {post_logout_value}
                            </textarea>
                            <span class="field__hint">{t.client_edit_post_logout_hint}</span>
                        </div>
                        <div class="field">
                            <label for="e-consent" class="field__label">{t.consent_policy_label}</label>
                            <select id="e-consent" name="consent_policy">
                                {
                                    let cp = consent_policy.clone();
                                    let cp2 = consent_policy.clone();
                                    let cp3 = consent_policy.clone();
                                    view! {
                                        <>
                                        <option value="none"     selected=move || cp  == "none">
                                            {t.consent_policy_none}
                                        </option>
                                        <option value="first_time" selected=move || cp2 == "first_time">
                                            {t.consent_policy_first_time}
                                        </option>
                                        <option value="always"   selected=move || cp3 == "always">
                                            {t.consent_policy_always}
                                        </option>
                                        </>
                                    }
                                }
                            </select>
                        </div>
                        <div class="row">
                            <button type="submit">{t.button_save}</button>
                            <a href="/admin/clients" class="button secondary">{t.button_cancel}</a>
                        </div>
                    </form>
                </div>

                // RFC-MI-051: danger zone for destructive client operations.
                <section class="danger-zone">
                    <h2 class="danger-zone__title">
                        "⚠ " {t.danger_zone_title}
                    </h2>
                    <div class="form-actions">
                        <a href=format!("/admin/clients/{}/delete-confirm", id) class="button danger">
                            {t.button_delete}
                        </a>
                    </div>
                </section>
            </Shell>
        }
    })
}
