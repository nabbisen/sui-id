//! Admin handlers for clients (RFC 066, RFC 149 Q4: the app form).

#[path = "clients/app_form.rs"]
mod app_form;

use super::forms::{ConfirmedReasonForm, DisableForm};
use super::with_csrf_cookie;
use crate::errors::HttpError;
use crate::handlers::{AppStateExt, CurrentAdmin, CurrentAdminOrAuditor};
use axum::Form;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use std::str::FromStr;
use sui_id_core::admin::{self as admin_uc};
use sui_id_core::errors::CoreError;
use sui_id_shared::api::ClientSummary;
use sui_id_shared::ids::ClientId;
use sui_id_store::models::TokenEndpointAuthMethod;
use sui_id_store::repos::clients;
use sui_id_web::{
    pages::{AppFormData, ClientEditData, ConfirmDeleteClientData},
    render_client_edit, render_clients, render_clients_new, render_confirm_delete_client,
};

pub async fn clients_delete_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref actor, admin_username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(crate::errors::HttpError::html_403_auditor());
    }
    let State(app) = state_ext;
    let return_to = format!("/admin/clients/{id}/delete-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let cid = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let client = clients::get(&app.db, cid)
        .await
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmDeleteClientData {
        client_id: id,
        client_name: client.name,
        csrf_token: token.clone(),
        admin_username,
    };
    let resp = Html(render_confirm_delete_client(data, app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn clients_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, role, ref read_actor, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let rows = admin_uc::list_clients(&app.db, read_actor)
        .await
        .map_err(HttpError::html)?;
    let summaries: Vec<ClientSummary> = rows
        .into_iter()
        .map(|r| {
            // RFC 095 M3 stage 4b §0.2: computed from a reference before
            // `token_endpoint_auth_method` is moved into the field below --
            // worth more alarm than "unrecorded" in the admin UI, not less.
            let auth_method_unrecognized = matches!(
                r.token_endpoint_auth_method.as_ref(),
                Some(TokenEndpointAuthMethod::Unknown(_))
            );
            ClientSummary {
                id: r.id,
                name: r.name,
                redirect_uris: r.redirect_uris,
                allowed_scopes: r.allowed_scopes,
                post_logout_redirect_uris: r.post_logout_redirect_uris,
                confidential: r.confidential,
                is_disabled: r.is_disabled,
                is_deleted: r.is_deleted,
                consent_policy: r.consent_policy.as_str().to_string(),
                registered_via: r.registered_via.as_str().to_string(),
                token_endpoint_auth_method: r.token_endpoint_auth_method.map(|m| m.as_str()),
                auth_method_unrecognized,
                created_at: r.created_at,
            }
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_clients(
        role.is_admin(),
        summaries,
        None,
        None,
        token.clone(),
        app.is_dev_mode,
        lang,
        username,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// Build the shared `AppFormData` from a parsed/typed-in-progress
/// form plus whatever errors a save attempt found (`None` for a
/// fresh GET or an add/remove round trip, which never validates —
/// RFC 149 Q4 §5 test 1).
#[allow(clippy::too_many_arguments)]
fn form_data_from_parsed(
    parsed: &app_form::ParsedAppForm,
    errors: Option<&app_form::AppFormErrors>,
    action_url: String,
    confidential: Option<bool>,
    csrf: String,
    submit_label: &'static str,
    cancel_url: String,
) -> AppFormData {
    let empty = app_form::AppFormErrors::default();
    let e = errors.unwrap_or(&empty);
    let redirect_uri_errors = if e.redirect_uris.len() == parsed.redirect_uris.len() {
        e.redirect_uris.clone()
    } else {
        vec![None; parsed.redirect_uris.len()]
    };
    let post_logout_uri_errors = if e.post_logout_uris.len() == parsed.post_logout_uris.len() {
        e.post_logout_uris.clone()
    } else {
        vec![None; parsed.post_logout_uris.len()]
    };
    AppFormData {
        action_url,
        name: parsed.name.clone(),
        name_error: e.name.clone(),
        redirect_uris: parsed.redirect_uris.clone(),
        redirect_uri_errors,
        post_logout_uris: parsed.post_logout_uris.clone(),
        post_logout_uri_errors,
        standard_scopes: parsed.standard_scopes_checked.clone(),
        scopes_notice: None,
        additional_scopes: parsed.additional_scopes.clone(),
        additional_scopes_error: e.additional_scopes.clone(),
        consent_policy: if parsed.consent_policy.is_empty() {
            "none".to_owned()
        } else {
            parsed.consent_policy.clone()
        },
        homepage_uri: parsed.homepage_uri.clone(),
        homepage_uri_error: e.homepage_uri.clone(),
        privacy_policy_uri: parsed.privacy_policy_uri.clone(),
        privacy_policy_uri_error: e.privacy_policy_uri.clone(),
        tos_uri: parsed.tos_uri.clone(),
        tos_uri_error: e.tos_uri.clone(),
        confidential,
        csrf_token: csrf,
        submit_label,
        cancel_url,
    }
}

/// `GET /admin/clients/new` — isolated register-client form.
pub async fn clients_new_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(crate::errors::HttpError::html_403_auditor());
    }
    let State(app) = state_ext;
    let token = crate::csrf::ensure_token(&jar);
    let t = lang.strings();
    let form = AppFormData {
        action_url: "/admin/clients".into(),
        name: String::new(),
        name_error: None,
        redirect_uris: vec![String::new()],
        redirect_uri_errors: vec![None],
        post_logout_uris: vec![],
        post_logout_uri_errors: vec![],
        standard_scopes: new_app_default_scopes(),
        scopes_notice: None,
        additional_scopes: String::new(),
        additional_scopes_error: None,
        consent_policy: "none".into(),
        homepage_uri: String::new(),
        homepage_uri_error: None,
        privacy_policy_uri: String::new(),
        privacy_policy_uri_error: None,
        tos_uri: String::new(),
        tos_uri_error: None,
        confidential: Some(true),
        csrf_token: token.clone(),
        submit_label: t.clients_button_register,
        cancel_url: "/admin/clients".into(),
    };
    let resp = Html(render_clients_new(
        form,
        None,
        app.is_dev_mode,
        lang,
        username,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn clients_create(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id, ref admin_actor, username): CurrentAdmin,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    body: Bytes,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let mut parsed = app_form::parse(&body);
    crate::handlers::enforce_csrf(&jar, Some(&parsed.csrf))?;
    let t = lang.strings();

    if parsed.action != app_form::FormAction::Save {
        // RFC 149 Q4 §2/§5 test 1: "Add another"/"Remove" save
        // nothing and record nothing — just a different row count,
        // with everything else the person typed kept.
        app_form::apply_uri_action(
            parsed.action,
            &mut parsed.redirect_uris,
            &mut parsed.post_logout_uris,
        );
        let token = crate::csrf::ensure_token(&jar);
        let form = form_data_from_parsed(
            &parsed,
            None,
            "/admin/clients".into(),
            Some(matches!(
                parsed.confidential.as_deref(),
                Some("true" | "on" | "1")
            )),
            token.clone(),
            t.clients_button_register,
            "/admin/clients".into(),
        );
        let resp = Html(render_clients_new(
            form,
            None,
            app.is_dev_mode,
            lang,
            username,
        ))
        .into_response();
        return Ok(with_csrf_cookie(resp, &app, &token));
    }

    let confidential = matches!(parsed.confidential.as_deref(), Some("true" | "on" | "1"));
    let validated = match app_form::validate(&parsed, t) {
        Ok(v) => v,
        Err(errors) => {
            let token = crate::csrf::ensure_token(&jar);
            let form = form_data_from_parsed(
                &parsed,
                Some(&errors),
                "/admin/clients".into(),
                Some(confidential),
                token.clone(),
                t.clients_button_register,
                "/admin/clients".into(),
            );
            let html = render_clients_new(form, None, app.is_dev_mode, lang, username);
            return Ok(with_csrf_cookie(
                (axum::http::StatusCode::BAD_REQUEST, Html(html)).into_response(),
                &app,
                &token,
            ));
        }
    };

    let created = admin_uc::create_client(
        &app.db,
        &app.clock,
        admin_actor,
        sui_id_core::admin::CreateClientSpec {
            name: &validated.name,
            redirect_uris: &validated.redirect_uris,
            confidential,
            allowed_scopes: &validated.allowed_scopes,
            post_logout_redirect_uris: &validated.post_logout_uris,
            consent_policy: validated.consent_policy,
            homepage_uri: validated.homepage_uri,
            privacy_policy_uri: validated.privacy_policy_uri,
            tos_uri: validated.tos_uri,
        },
        &app.caches,
    )
    .await
    .map_err(HttpError::html)?;

    // Re-list and pass the secret through to the page so it is shown once.
    let rows = admin_uc::list_clients(&app.db, &admin_actor.as_read_only())
        .await
        .map_err(HttpError::html)?;
    let summaries: Vec<ClientSummary> = rows
        .into_iter()
        .map(|r| {
            let auth_method_unrecognized = matches!(
                r.token_endpoint_auth_method.as_ref(),
                Some(TokenEndpointAuthMethod::Unknown(_))
            );
            ClientSummary {
                id: r.id,
                name: r.name,
                redirect_uris: r.redirect_uris,
                allowed_scopes: r.allowed_scopes,
                post_logout_redirect_uris: r.post_logout_redirect_uris,
                confidential: r.confidential,
                is_disabled: r.is_disabled,
                is_deleted: r.is_deleted,
                consent_policy: r.consent_policy.as_str().to_string(),
                registered_via: r.registered_via.as_str().to_string(),
                token_endpoint_auth_method: r.token_endpoint_auth_method.map(|m| m.as_str()),
                auth_method_unrecognized,
                created_at: r.created_at,
            }
        })
        .collect();

    let secret_payload = created
        .generated_secret
        .map(|s| (created.row.id.to_string(), s));
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_clients(
        true,
        summaries,
        None,
        secret_payload,
        token.clone(),
        app.is_dev_mode,
        lang,
        username,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn clients_set_disabled(
    state_ext: AppStateExt,
    CurrentAdmin(_, ref admin_actor, _): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<DisableForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    // RFC 060 bug fix: also gate on `_confirmed=1`.
    crate::handlers::require_confirmed(&form.confirmed)?;
    // RFC 058: step-up immediately after CSRF + confirm gate.
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/clients").await
    {
        return Ok(redirect);
    }
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let value = matches!(form.disabled.as_str(), "true" | "on" | "1");
    let reason_opt = if form.reason.trim().is_empty() {
        None
    } else {
        Some(form.reason.trim().to_string())
    };
    admin_uc::set_client_disabled(
        &app.db,
        &app.clock,
        admin_actor,
        target,
        value,
        reason_opt,
        &app.caches,
    )
    .await
    .map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/clients").into_response())
}

pub async fn clients_delete(
    state_ext: AppStateExt,
    CurrentAdmin(_, ref admin_actor, _): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedReasonForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    // RFC 060: enforce the confirm screen's `_confirmed=1` token.
    crate::handlers::require_confirmed(&form.confirmed)?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/clients").await
    {
        return Ok(redirect);
    }
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    admin_uc::delete_client(
        &app.db,
        &app.clock,
        admin_actor,
        target,
        form.reason_opt(),
        &app.caches,
    )
    .await
    .map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/clients").into_response())
}

/// The standard scopes a new app starts with (RFC 027, RFC 149 Q4):
/// `profile` and `email` checked, covering what most basic OIDC
/// integrations need beyond the always-included `openid`. One place,
/// used by the create form and by an app whose stored list is empty.
fn new_app_default_scopes() -> Vec<(&'static str, bool)> {
    sui_id_core::discovery::STANDARD_SCOPES
        .iter()
        .skip(1)
        .map(|&s| (s, s == "profile" || s == "email"))
        .collect()
}

/// Split a stored `allowed_scopes` string into the three standard
/// checkboxes and whatever remains for *Additional scopes*. `openid`
/// is dropped — it is never shown as an editable checkbox. RFC 149
/// Q4 §1's own closing bullet: an app saved before this release with
/// a near-miss additional scope is re-checked here, against today's
/// rule, so the form shows the stored value under *Additional
/// scopes* with the same message it would get on a fresh save — not
/// silently dropped, and not silently accepted either.
///
/// **An empty stored list is not "no scopes"**: `enforce_scope_policy`
/// lets a request name any scope at all (`oidc/authorize.rs`). The
/// checkboxes cannot show that, and a save always writes a list, so
/// an unrelated edit would silently restrict the app. So the form
/// says so in a notice, and starts from the new-app default, labelled
/// as such (Q4's review).
fn split_stored_scopes(stored: &str, t: &'static sui_id_i18n::Strings) -> SplitScopes {
    if stored.trim().is_empty() {
        return SplitScopes {
            standard: new_app_default_scopes(),
            additional_scopes: String::new(),
            additional_scopes_error: None,
            scopes_notice: Some(t.clients_scopes_empty_notice.to_owned()),
        };
    }
    let mut standard: Vec<(&'static str, bool)> = sui_id_core::discovery::STANDARD_SCOPES
        .iter()
        .skip(1)
        .map(|&s| (s, false))
        .collect();
    let mut additional: Vec<&str> = Vec::new();
    for tok in stored.split_whitespace() {
        if tok == "openid" {
            continue;
        }
        match standard.iter_mut().find(|(n, _)| *n == tok) {
            Some(entry) => entry.1 = true,
            None => additional.push(tok),
        }
    }
    let additional_scopes = additional.join(" ");
    let additional_scopes_error =
        match sui_id_core::admin::clients::validate_additional_scopes(&additional_scopes) {
            Ok(_) => None,
            Err(e) => Some(app_form::additional_scope_error_text(e, t)),
        };
    SplitScopes {
        standard,
        additional_scopes,
        additional_scopes_error,
        scopes_notice: None,
    }
}

/// What [`split_stored_scopes`] returns: the form's scope fields for one
/// stored list, and a notice when the list cannot be shown as checkboxes.
struct SplitScopes {
    standard: Vec<(&'static str, bool)>,
    additional_scopes: String,
    additional_scopes_error: Option<String>,
    scopes_notice: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn row_to_edit_data(
    row: sui_id_store::models::ClientRow,
    can_write: bool,
    username: String,
    t: &'static sui_id_i18n::Strings,
    freshly_rotated_secret: Option<String>,
) -> ClientEditData {
    let auth_method_unrecognized = matches!(
        row.token_endpoint_auth_method.as_ref(),
        Some(TokenEndpointAuthMethod::Unknown(_))
    );
    let SplitScopes {
        standard: standard_scopes,
        additional_scopes,
        additional_scopes_error,
        scopes_notice,
    } = split_stored_scopes(&row.allowed_scopes, t);
    let id = row.id.to_string();
    let redirect_uris = if row.redirect_uris.is_empty() {
        vec![String::new()]
    } else {
        row.redirect_uris
    };
    let form = AppFormData {
        action_url: format!("/admin/clients/{id}/edit"),
        name: row.name.clone(),
        name_error: None,
        redirect_uri_errors: vec![None; redirect_uris.len()],
        redirect_uris,
        post_logout_uri_errors: vec![None; row.post_logout_redirect_uris.len()],
        post_logout_uris: row.post_logout_redirect_uris,
        standard_scopes,
        scopes_notice,
        additional_scopes,
        additional_scopes_error,
        consent_policy: row.consent_policy.as_str().to_string(),
        homepage_uri: row.homepage_uri.unwrap_or_default(),
        homepage_uri_error: None,
        privacy_policy_uri: row.privacy_policy_uri.unwrap_or_default(),
        privacy_policy_uri_error: None,
        tos_uri: row.tos_uri.unwrap_or_default(),
        tos_uri_error: None,
        confidential: None,
        csrf_token: String::new(), // set by the caller, below
        submit_label: t.button_save,
        cancel_url: "/admin/clients".into(),
    };
    ClientEditData {
        id,
        confidential: row.confidential,
        is_disabled: row.is_disabled,
        created_at: row.created_at,
        updated_at: row.updated_at,
        registered_via: row.registered_via.as_str().to_string(),
        token_endpoint_auth_method: row.token_endpoint_auth_method.map(|m| m.as_str()),
        auth_method_unrecognized,
        freshly_rotated_secret,
        username,
        can_write,
        form,
    }
}

pub async fn clients_edit_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref read_actor, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let row = admin_uc::get_client(&app.db, read_actor, target)
        .await
        .map_err(HttpError::html)?;
    let token = crate::csrf::ensure_token(&jar);
    let mut data = row_to_edit_data(row, read_actor.can_write(), username, lang.strings(), None);
    data.form.csrf_token = token.clone();
    let resp = Html(render_client_edit(data, None, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn clients_edit_post(
    state_ext: AppStateExt,
    CurrentAdmin(_, ref admin_actor, username): CurrentAdmin,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let mut parsed = app_form::parse(&body);
    crate::handlers::enforce_csrf(&jar, Some(&parsed.csrf))?;
    let t = lang.strings();
    let action_url = format!("/admin/clients/{id}/edit");

    if parsed.action != app_form::FormAction::Save {
        app_form::apply_uri_action(
            parsed.action,
            &mut parsed.redirect_uris,
            &mut parsed.post_logout_uris,
        );
        let token = crate::csrf::ensure_token(&jar);
        let row = admin_uc::get_client(&app.db, &admin_actor.as_read_only(), target)
            .await
            .map_err(HttpError::html)?;
        let mut data = row_to_edit_data(row, true, username, t, None);
        data.form = form_data_from_parsed(
            &parsed,
            None,
            action_url,
            None,
            token.clone(),
            t.button_save,
            "/admin/clients".into(),
        );
        let resp = Html(render_client_edit(data, None, lang)).into_response();
        return Ok(with_csrf_cookie(resp, &app, &token));
    }

    let validated = match app_form::validate(&parsed, t) {
        Ok(v) => v,
        Err(errors) => {
            let token = crate::csrf::ensure_token(&jar);
            let row = admin_uc::get_client(&app.db, &admin_actor.as_read_only(), target)
                .await
                .map_err(HttpError::html)?;
            let mut data = row_to_edit_data(row, true, username, t, None);
            data.form = form_data_from_parsed(
                &parsed,
                Some(&errors),
                action_url,
                None,
                token.clone(),
                t.button_save,
                "/admin/clients".into(),
            );
            let html = render_client_edit(data, None, lang);
            return Ok(with_csrf_cookie(
                (axum::http::StatusCode::BAD_REQUEST, Html(html)).into_response(),
                &app,
                &token,
            ));
        }
    };

    // Basic metadata (name, redirect_uris, C02) stays its own update,
    // separate from the three-field form (C03/C04/C09, stage 2).
    admin_uc::update_client_basic(
        &app.db,
        &app.clock,
        admin_actor,
        target,
        &validated.name,
        &validated.redirect_uris,
        &app.caches,
    )
    .await
    .map_err(HttpError::html)?;
    // Allowed scopes, post-logout redirect URIs and consent policy
    // (RFC 038) commit together, in one transaction, as one audit
    // record (RFC 094 M2b wave 4 stage 2, C03/C04/C09) — a failure
    // partway through no longer leaves one field changed and another
    // not.
    admin_uc::update_client_form(
        &app.db,
        &app.clock,
        admin_actor,
        target,
        &validated.allowed_scopes,
        &validated.post_logout_uris,
        validated.consent_policy,
    )
    .await
    .map_err(HttpError::html)?;
    // RFC 140 stage 1 (C10): homepage, privacy policy and terms links,
    // already validated above.
    admin_uc::update_client_app_identity(
        &app.db,
        &app.clock,
        admin_actor,
        target,
        validated.homepage_uri,
        validated.privacy_policy_uri,
        validated.tos_uri,
    )
    .await
    .map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/clients").into_response())
}

// ---------- audit ----------

pub async fn clients_rotate_secret_post(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id, ref admin_actor, username): CurrentAdmin,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedReasonForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/clients").await
    {
        return Ok(redirect);
    }
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let new_secret =
        admin_uc::rotate_client_secret(&app.db, &app.clock, admin_actor, target, form.reason_opt())
            .await
            .map_err(HttpError::html)?;
    // RFC 122 D1: the new secret never becomes a URL component. Render the
    // edit page directly from this response — the same pattern
    // `clients_create` already uses for the creation secret — instead of
    // redirecting with the secret in the query string. The route carries
    // `Cache-Control: no-store` at the router layer (router.rs).
    let read_actor = admin_actor.as_read_only();
    let row = admin_uc::get_client(&app.db, &read_actor, target)
        .await
        .map_err(HttpError::html)?;
    let token = crate::csrf::ensure_token(&jar);
    let mut data = row_to_edit_data(
        row,
        read_actor.can_write(),
        username,
        lang.strings(),
        Some(new_secret),
    );
    data.form.csrf_token = token.clone();
    let resp = Html(render_client_edit(data, None, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}
