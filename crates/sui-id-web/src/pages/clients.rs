//! Page renderers for the "clients" screen domain (RFC 065, RFC 149 Q4:
//! the app form).

use super::common::*;
use crate::components::empty_state;
use crate::components::forms::{FieldError, error_summary};
use crate::layout::{Chrome, NavItem, Shell};
use leptos::prelude::*;
use sui_id_shared::api::ClientSummary;

fn client_row_view(
    can_write: bool,
    t: &'static sui_id_i18n::Strings,
    c: ClientSummary,
    csrf: String,
) -> impl IntoView {
    let is_disabled = c.is_disabled;
    let is_deleted = c.is_deleted;
    let kind = if c.confidential {
        "confidential"
    } else {
        "public"
    };
    let id_str = c.id.to_string();
    let action_label = if is_disabled { "Enable" } else { "Disable" };
    let action_target = if is_disabled { "false" } else { "true" };
    let disabled_url = format!("/admin/clients/{id_str}/disabled");

    let status_view = if is_deleted {
        crate::components::status_badge(t, crate::components::StatusKind::Deleted).into_any()
    } else if is_disabled {
        crate::components::status_badge(t, crate::components::StatusKind::Disabled).into_any()
    } else {
        crate::components::status_badge(t, crate::components::StatusKind::Active).into_any()
    };
    // RFC 136 D2: a marker on `registered_via = 'dynamic'` only. An
    // administrator-created client gets nothing here -- the administrator
    // created it, so it is not news.
    let self_registered_badge = (c.registered_via == "dynamic")
        .then(|| crate::components::status_badge(t, crate::components::StatusKind::SelfRegistered));
    // RFC 095 M3 stage 4a §5: same shape, on the opposite condition -- a
    // client whose auth method was never recorded, same as the detail page.
    let auth_method_unrecorded_badge = c.token_endpoint_auth_method.is_none().then(|| {
        crate::components::status_badge(t, crate::components::StatusKind::AuthMethodUnrecorded)
    });
    // RFC 095 M3 stage 4b §0.2: distinct from "unrecorded" -- a value
    // present but outside the closed set, which only a database changed
    // from outside can produce. Mutually exclusive with the marker above:
    // `token_endpoint_auth_method` is `Some(_)` whenever this is true.
    let auth_method_unrecognized_badge = c.auth_method_unrecognized.then(|| {
        crate::components::status_badge(t, crate::components::StatusKind::AuthMethodUnrecognized)
    });

    let edit_url = format!("/admin/clients/{id_str}/edit");
    let actions = if is_deleted {
        view! { <td><span class="muted">{t.empty_dash}</span></td> }.into_any()
    } else if !can_write {
        // RFC 071: auditors see no mutation controls.
        view! {
            <td><a href=edit_url class="button secondary">{t.button_view_detail}</a></td>
        }
        .into_any()
    } else {
        view! {
            <td>
                <div class="row gap-1">
                    <a href=edit_url class="button secondary">{t.button_edit}</a>
                    <form method="post" action=disabled_url class="inline-el">
                        <input type="hidden" name="_csrf" value=csrf />
                        <input type="hidden" name="disabled" value=action_target />
                        <button type="submit" class="secondary">{action_label}</button>
                    </form>
                </div>
            </td>
        }
        .into_any()
    };

    let id_for_copy = id_str.clone();
    view! {
        <tr>
            <td>
                <div class="row gap-1">
                    <span>{c.name}</span>
                    {self_registered_badge}
                    {auth_method_unrecorded_badge}
                    {auth_method_unrecognized_badge}
                </div>
            </td>
            <td>
                <span class="code">{id_str}</span>
                {copy_btn(t, id_for_copy, t.copy_noun_client_id)}
            </td>
            <td>{kind}</td>
            <td>{status_view}</td>
            {actions}
        </tr>
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_clients(
    can_write: bool,
    clients: Vec<ClientSummary>,
    page_info: crate::components::pager::OffsetPageInfo,
    flash: Option<Flash>,
    new_secret: Option<(String, String)>,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
    username: String,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_rows = csrf_token.clone();
        let (from, to) = page_info.range();
        let total = page_info.total;
        let pager = crate::components::pager::offset_pager_view(t, "/admin/clients", &page_info);
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
            .map(|c| client_row_view(can_write, t, c, csrf_for_rows.clone()))
            .collect();
        view! {
            <Shell title=t.clients_title.to_string() nav=Chrome::Console{username, item: NavItem::Apps} dev_mode=dev_mode lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.clients_title}</h1>
                        <p class="page-header__lede">
                            {t.clients_lede}
                            " "
                            {(t.clients_count_caption)(from, to, total)}
                        </p>
                    </div>
                    {can_write.then(|| view! {
                        <div class="page-header__actions">
                            <a href="/admin/clients/new" class="button">
                                <span class="button__icon" aria-hidden="true">"+"</span>{t.clients_create_section}
                            </a>
                        </div>
                    })}
                </header>
                {flash_banner(flash)}
                {secret_block}

                <section>
                    <h2>{t.clients_table_section}</h2>
                    <div class="table-wrap">
                        {if rows.is_empty() {
                            empty_state(t.empty_clients,
                                if can_write { Some(("/admin/clients/new", t.empty_clients_cta)) } else { None })
                            .into_any()
                        } else {
                            view! {
                                <table>
                                    <thead>
                                        <tr>
                                            <th>{t.clients_table_th_name}</th>
                                            <th>{t.clients_table_th_client_id}</th>
                                            <th>{t.clients_table_th_kind}</th>
                                            <th>{t.clients_table_th_status}</th>
                                            <th></th>
                                        </tr>
                                    </thead>
                                    <tbody>{rows}</tbody>
                                </table>
                            }.into_any()
                        }}
                    </div>
                    {pager}
                </section>

            </Shell>
        }
    })
}

// ---------- the app form (RFC 149 Q4): shared by create and edit ----------

/// Every field the create and edit forms both offer. RFC 149 Q4 §3: "one
/// component renders both" — this struct, and [`render_app_form`], are
/// that component. The caller (`clients_new_get`/`clients_edit_get`/the
/// two POST handlers' re-renders) resolves every value and every error
/// first; this function only lays them out.
pub struct AppFormData {
    pub action_url: String,
    pub name: String,
    pub name_error: Option<String>,
    /// Always at least one row — the handler never lets this reach zero
    /// (RFC 149 Q4 §2: removing the last row is a no-op, same as the
    /// command layer's own "at least one redirect_uri" rule).
    pub redirect_uris: Vec<String>,
    /// Same length as `redirect_uris`; `Some(_)` at index `i` is that
    /// row's own error.
    pub redirect_uri_errors: Vec<Option<String>>,
    pub post_logout_uris: Vec<String>,
    pub post_logout_uri_errors: Vec<Option<String>>,
    /// Every scope `openid` doesn't already cover, in the exact order
    /// and membership `sui_id_core::discovery::STANDARD_SCOPES` lists
    /// them (RFC 149 Q4 §1: discovery and this form share that one
    /// constant, so they cannot drift) -- the caller builds this list
    /// from it; this struct only renders whatever it is given.
    pub standard_scopes: Vec<(&'static str, bool)>,
    /// A warning above the scopes, when what is stored cannot be shown
    /// as checkboxes (an empty list, which permits any scope).
    pub scopes_notice: Option<String>,
    pub additional_scopes: String,
    pub additional_scopes_error: Option<String>,
    /// "none", "first_time", or "always".
    pub consent_policy: String,
    pub homepage_uri: String,
    pub homepage_uri_error: Option<String>,
    pub privacy_policy_uri: String,
    pub privacy_policy_uri_error: Option<String>,
    pub tos_uri: String,
    pub tos_uri_error: Option<String>,
    /// `Some(_)` on create (still choosable); `None` on edit — kind is
    /// immutable after creation (RFC 149 Q4 §3's own "basic info" card
    /// shows it read-only instead).
    pub confidential: Option<bool>,
    pub csrf_token: String,
    pub submit_label: &'static str,
    pub cancel_url: String,
}

fn uri_row_view(
    t: &'static sui_id_i18n::Strings,
    field_prefix: &'static str,
    remove_button_name: &'static str,
    index: usize,
    value: &str,
    error: &Option<String>,
    label: String,
) -> impl IntoView {
    let input_id = format!("{field_prefix}-{index}");
    let input_name = format!("{field_prefix}_{index}");
    let error_id = format!("{input_id}-error");
    let has_error = error.is_some();
    let row_class = if has_error {
        "uri-row field field--invalid"
    } else {
        "uri-row field"
    };
    let described_by = has_error.then(|| error_id.clone());
    let value = value.to_owned();
    let error_text = error.clone();
    let index_str = index.to_string();
    view! {
        <div class=row_class data-uri-row="">
            <div class="row gap-1">
                <input type="url" id=input_id.clone() name=input_name aria-label=label value=value
                       aria-invalid=has_error
                       aria-describedby=described_by.clone() />
                <button type="submit" name=remove_button_name value=index_str formnovalidate=true class="button secondary" data-remove-uri-row="">
                    {t.clients_button_remove}
                </button>
            </div>
            {error_text.map(|e| view! { <p class="field__error" id=error_id>{e}</p> })}
        </div>
    }
}

fn uri_rows_view(
    t: &'static sui_id_i18n::Strings,
    field_prefix: &'static str,
    remove_button_name: &'static str,
    uris: &[String],
    errors: &[Option<String>],
    label_fn: fn(usize) -> String,
) -> Vec<leptos::prelude::AnyView> {
    uris.iter()
        .enumerate()
        .map(|(i, uri)| {
            let ordinal = i + 1;
            let label = if uri.is_empty() {
                format!("{}{}", label_fn(ordinal), t.clients_uri_empty_suffix)
            } else {
                label_fn(ordinal)
            };
            let error = errors.get(i).cloned().unwrap_or(None);
            uri_row_view(t, field_prefix, remove_button_name, i, uri, &error, label).into_any()
        })
        .collect()
}

fn scope_check_view(
    id: String,
    name: Option<String>,
    checked: bool,
    disabled: bool,
    label: &'static str,
    desc: &'static str,
) -> impl IntoView {
    let for_id = id.clone();
    view! {
        <div class="scope-check">
            <input type="checkbox" id=id name=name value="true" checked=checked disabled=disabled />
            <label for=for_id>
                <strong>{label}</strong>
                <span class="scope-check__desc">{desc}</span>
            </label>
        </div>
    }
}

/// The one place a standard scope's name maps to its own description
/// -- the *set* and *order* of scopes come from the discovery list
/// the caller already generated `standard_scopes` from; this is only
/// the leaf text lookup for a name that list is known to contain.
fn scope_description(name: &str, t: &'static sui_id_i18n::Strings) -> &'static str {
    match name {
        "profile" => t.clients_scope_profile_desc,
        "email" => t.clients_scope_email_desc,
        "offline_access" => t.clients_scope_offline_access_desc,
        _ => "",
    }
}

fn render_app_form(data: AppFormData, t: &'static sui_id_i18n::Strings) -> impl IntoView {
    let AppFormData {
        action_url,
        name,
        name_error,
        redirect_uris,
        redirect_uri_errors,
        post_logout_uris,
        post_logout_uri_errors,
        standard_scopes,
        scopes_notice,
        additional_scopes,
        additional_scopes_error,
        consent_policy,
        homepage_uri,
        homepage_uri_error,
        privacy_policy_uri,
        privacy_policy_uri_error,
        tos_uri,
        tos_uri_error,
        confidential,
        csrf_token,
        submit_label,
        cancel_url,
    } = data;

    // RFC 092: a top summary when more than one field is wrong, beside
    // each field's own inline message either way.
    let mut errors: Vec<FieldError> = Vec::new();
    if let Some(e) = &name_error {
        errors.push(FieldError {
            field: "name",
            message: e.clone(),
        });
    }
    for e in redirect_uri_errors.iter().flatten() {
        errors.push(FieldError {
            field: "redirect_uri",
            message: e.clone(),
        });
    }
    for e in post_logout_uri_errors.iter().flatten() {
        errors.push(FieldError {
            field: "post_logout_uri",
            message: e.clone(),
        });
    }
    if let Some(e) = &additional_scopes_error {
        errors.push(FieldError {
            field: "additional_scopes",
            message: e.clone(),
        });
    }
    for (field, e) in [
        ("homepage_uri", &homepage_uri_error),
        ("privacy_policy_uri", &privacy_policy_uri_error),
        ("tos_uri", &tos_uri_error),
    ] {
        if let Some(e) = e {
            errors.push(FieldError {
                field,
                message: e.clone(),
            });
        }
    }
    let summary = error_summary(t, &errors);

    let name_invalid = name_error.is_some();
    let name_field_class = if name_invalid {
        "field field--invalid"
    } else {
        "field"
    };

    let redirect_next_index = redirect_uris.len().to_string();
    let post_logout_next_index = post_logout_uris.len().to_string();
    let redirect_rows = uri_rows_view(
        t,
        "redirect_uri",
        "remove_redirect_uri",
        &redirect_uris,
        &redirect_uri_errors,
        t.clients_uri_redirect_label,
    );
    let post_logout_rows = uri_rows_view(
        t,
        "post_logout_uri",
        "remove_post_logout_uri",
        &post_logout_uris,
        &post_logout_uri_errors,
        t.clients_uri_post_logout_label,
    );

    let scope_rows: Vec<leptos::prelude::AnyView> = standard_scopes
        .iter()
        .map(|&(name, checked)| {
            scope_check_view(
                format!("scope-{}", name.replace('_', "-")),
                Some(format!("scope_{name}")),
                checked,
                false,
                name,
                scope_description(name, t),
            )
            .into_any()
        })
        .collect();

    let additional_scopes_class = if additional_scopes_error.is_some() {
        "field field--invalid"
    } else {
        "field"
    };

    fn app_uri_field(
        id: &'static str,
        name: &'static str,
        label: &'static str,
        value: String,
        error: Option<String>,
    ) -> impl IntoView {
        let field_class = if error.is_some() {
            "field field--invalid"
        } else {
            "field"
        };
        let error_id = format!("{id}-error");
        let described_by = error.as_ref().map(|_| error_id.clone());
        view! {
            <div class=field_class>
                <label for=id class="field__label">{label}</label>
                <input id=id name=name type="url" value=value
                       aria-invalid=error.is_some()
                       aria-describedby=described_by.clone() />
                {error.map(|e| view! { <p class="field__error" id=error_id>{e}</p> })}
            </div>
        }
    }

    view! {
        <>
        <form method="post" action=action_url class="stack">
            <input type="hidden" name="_csrf" value=csrf_token />
            {summary}

            <div class="card">
                <h3 class="card__title">{t.clients_section_about}</h3>
                <div class=name_field_class>
                    <label for="app-name" class="field__label">{t.clients_label_app_name}</label>
                    <input id="app-name" name="name" type="text" required=true value=name
                           aria-invalid=name_invalid
                           aria-describedby=name_invalid.then_some("app-name-error") />
                    {name_error.map(|e| view! { <p class="field__error" id="app-name-error">{e}</p> })}
                </div>
                {app_uri_field("app-homepage", "homepage_uri", t.clients_label_homepage_uri, homepage_uri, homepage_uri_error)}
                {app_uri_field("app-privacy", "privacy_policy_uri", t.clients_label_privacy_policy_uri, privacy_policy_uri, privacy_policy_uri_error)}
                {app_uri_field("app-tos", "tos_uri", t.clients_label_tos_uri, tos_uri, tos_uri_error)}
                <div class="field">
                    <label for="app-consent" class="field__label">{t.consent_policy_label}</label>
                    <select id="app-consent" name="consent_policy">
                        {
                            let cp1 = consent_policy.clone();
                            let cp2 = consent_policy.clone();
                            let cp3 = consent_policy.clone();
                            view! {
                                <>
                                <option value="none" selected=move || cp1 == "none">
                                    {t.consent_policy_none}
                                </option>
                                <option value="first_time" selected=move || cp2 == "first_time">
                                    {t.consent_policy_first_time}
                                </option>
                                <option value="always" selected=move || cp3 == "always">
                                    {t.consent_policy_always}
                                </option>
                                </>
                            }
                        }
                    </select>
                </div>
                {confidential.map(|checked| view! {
                    <label class="row gap-2">
                        <input name="confidential" type="checkbox" value="true" checked=checked />
                        <span>{t.clients_label_confidential_checkbox}</span>
                    </label>
                })}
                <p class="field__hint mb-0">
                    "\u{2139}  "
                    {t.clients_single_realm_note}
                </p>
            </div>

            <div class="card">
                <h3 class="card__title">
                    {t.clients_section_redirect_uris}
                    {crate::components::help_details(
                        t,
                        t.help_about_redirect_uri.to_string(),
                        t.help_redirect_uri_body,
                        concat!(
                            crate::github_doc_url!("docs/src/guides/operators.md"),
                            "#how-a-redirect-uri-is-matched",
                        ),
                    )}
                </h3>
                <div class="stack" id="redirect-uri-rows" data-min-rows="1" data-next-index=redirect_next_index>{redirect_rows}</div>
                <button type="submit" name="add_redirect_uri" value="1" formnovalidate=true class="button secondary mt-2"
                        data-add-uri-row="" data-rows-for="redirect-uri-rows"
                        data-input-name-prefix="redirect_uri"
                        data-label-template=t.clients_uri_redirect_label_template
                        data-remove-label=t.clients_button_remove>
                    {t.clients_button_add_another}
                </button>

                <h4 class="mt-4">{t.clients_label_post_logout_uris}</h4>
                <div class="stack" id="post-logout-uri-rows" data-min-rows="0" data-next-index=post_logout_next_index>{post_logout_rows}</div>
                <button type="submit" name="add_post_logout_uri" value="1" formnovalidate=true class="button secondary mt-2"
                        data-add-uri-row="" data-rows-for="post-logout-uri-rows"
                        data-input-name-prefix="post_logout_uri"
                        data-label-template=t.clients_uri_post_logout_label_template
                        data-remove-label=t.clients_button_remove>
                    {t.clients_button_add_another}
                </button>
            </div>

            <div class="card">
                <h3 class="card__title">{t.clients_section_scopes}</h3>
                {scopes_notice.map(|n| view! {
                    <p class="callout callout--warning" role="note">{n}</p>
                })}
                <div class="stack-tight">
                    {scope_check_view("scope-openid".to_owned(), None, true, true, "openid", t.clients_scope_openid_desc)}
                    {scope_rows}
                </div>
                <div class=additional_scopes_class>
                    <label for="app-additional-scopes" class="field__label">
                        {t.clients_label_additional_scopes}
                        {crate::components::help_details(
                            t,
                            t.help_about_additional_scopes.to_string(),
                            t.help_additional_scopes_body,
                            concat!(
                                crate::github_doc_url!("docs/src/guides/operators.md"),
                                "#what-allowed_scopes-controls",
                            ),
                        )}
                    </label>
                    <input id="app-additional-scopes" name="additional_scopes" type="text" value=additional_scopes
                           aria-invalid=additional_scopes_error.is_some()
                           aria-describedby=additional_scopes_error.as_ref().map(|_| "app-additional-scopes-error") />
                    {additional_scopes_error.map(|e| view! { <p class="field__error" id="app-additional-scopes-error">{e}</p> })}
                    <span class="field__hint">{t.clients_hint_additional_scopes}</span>
                </div>
            </div>

            <div class="form-actions">
                <button type="submit" name="save" value="1">{submit_label}</button>
                <a href=cancel_url class="button secondary">{t.button_cancel}</a>
            </div>
        </form>
        <script src="/static/uri-rows.js"></script>
        </>
    }
}

// ---------- clients/new ----------

pub fn render_clients_new(
    form: AppFormData,
    flash: Option<Flash>,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
    username: String,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_shell = form.csrf_token.clone();
        view! {
            <Shell title=t.clients_create_section.to_string() nav=Chrome::Console{username, item: NavItem::Apps} dev_mode=dev_mode lang=lang csrf_token=csrf_for_shell>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.clients_create_section}</h1>
                    </div>
                    <div class="page-header__actions">
                        <a href="/admin/clients" class="button secondary">{t.button_cancel}</a>
                    </div>
                </header>
                {flash_banner(flash)}
                {render_app_form(form, t)}
            </Shell>
        }
    })
}

// ---------- client edit ----------

pub struct ClientEditData {
    pub id: String,
    pub confidential: bool,
    pub is_disabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// RFC 065: "admin" or "dynamic". RFC 136 D2: drives the
    /// self-registered marker; "admin" renders no marker at all.
    pub registered_via: String,
    /// RFC 095 M3 stage 4a §5: `None` drives the "unrecorded" marker;
    /// `Some(_)` renders no marker at all. The wire value itself is
    /// carried but not yet displayed when present — see `ClientSummary`'s
    /// own field for why it is kept a real value rather than a `bool`.
    pub token_endpoint_auth_method: Option<String>,
    /// RFC 095 M3 stage 4b §0.2: `true` renders its own, more alarming
    /// marker than "unrecorded" — see `ClientSummary`'s own field.
    pub auth_method_unrecognized: bool,
    /// RFC 047, RFC 122 D2: populated only by the handler that just rotated
    /// the secret, from the value that operation produced — never from a
    /// caller-supplied value. Shown once.
    pub freshly_rotated_secret: Option<String>,
    /// RFC 144 (Refresh, §4): the signed-in administrator's name, for
    /// the account menu.
    pub username: String,
    /// RFC 088: false for auditors → render static rows instead of form.
    pub can_write: bool,
    /// RFC 149 Q4 §3: the fields create and edit share.
    pub form: AppFormData,
}

pub fn render_client_edit(
    data: ClientEditData,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let ClientEditData {
            id,
            confidential,
            is_disabled,
            created_at,
            updated_at,
            registered_via,
            token_endpoint_auth_method,
            auth_method_unrecognized,
            freshly_rotated_secret,
            username,
            can_write,
            form,
        } = data;
        let self_registered_badge = (registered_via == "dynamic").then(|| {
            crate::components::status_badge(t, crate::components::StatusKind::SelfRegistered)
        });
        let auth_method_unrecorded_badge = token_endpoint_auth_method.is_none().then(|| {
            crate::components::status_badge(t, crate::components::StatusKind::AuthMethodUnrecorded)
        });
        let auth_method_unrecognized_badge = auth_method_unrecognized.then(|| {
            crate::components::status_badge(
                t,
                crate::components::StatusKind::AuthMethodUnrecognized,
            )
        });
        let kind = if confidential {
            "confidential"
        } else {
            "public"
        };

        let status_view = if is_disabled {
            crate::components::status_badge(t, crate::components::StatusKind::Disabled).into_any()
        } else {
            crate::components::status_badge(t, crate::components::StatusKind::Active).into_any()
        };

        let name_for_lede = form.name.clone();
        let csrf_for_shell = form.csrf_token.clone();

        let form_or_table = if can_write {
            render_app_form(form, t).into_any()
        } else {
            // RFC 071: an auditor sees the values, never the form.
            view! {
                <div class="card">
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_text(t.clients_label_app_name, form.name.clone())}
                                {kv_text(t.clients_section_redirect_uris, form.redirect_uris.join(", "))}
                                {kv_text(t.clients_label_post_logout_uris, form.post_logout_uris.join(", "))}
                                {kv_text(t.consent_policy_label, form.consent_policy.clone())}
                            </tbody>
                        </table>
                    </div>
                </div>
            }
            .into_any()
        };

        view! {
            <Shell title=t.client_edit_title.to_string() nav=Chrome::Console{username, item: NavItem::Apps} lang=lang csrf_token=csrf_for_shell>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.client_edit_title}</h1>
                        <p class="page-header__lede">{name_for_lede}</p>
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
                            {self_registered_badge}
                            {auth_method_unrecorded_badge}
                            {auth_method_unrecognized_badge}
                        </div>
                        <div>{t.client_edit_label_created_at}": "{fmt_time(created_at)}</div>
                        <div>{t.client_edit_label_updated_at}": "{fmt_time(updated_at)}</div>
                    </div>
                    <p class="muted mt-3">
                        {t.client_edit_basic_note}
                    </p>
                </div>

                {form_or_table}

                // RFC-MI-051 danger zone; RFC 071: hidden for auditors.
                {can_write.then(|| view! {
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
                })}
            </Shell>
        }
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests;
