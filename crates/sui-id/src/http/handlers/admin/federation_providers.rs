//! Admin handlers for federation providers (RFC 096-B2 S5).
//!
//! The precedent copied here is `signing_keys.rs`: a list page
//! (`CurrentAdminOrAuditor`, auditor read-only), a step-up-gated confirm
//! page per dangerous action, and a POST that re-renders the same
//! confirm page inline with a translated refusal on failure (the
//! `auth.rs`/`users.rs` "re-render with a `Flash`" shape), rather than
//! the generic `HttpError::html` error page.
//!
//! `check_and_enable` and `preflight_provider` (`crate::federation_
//! preflight`) and `disable_federation_provider`
//! (`sui_id_store::commands`) are S4's own entry points — this module
//! is their one HTTP-facing caller (besides `check_and_enable`'s own
//! scope-test consumer).

use super::forms::ConfirmedForm;
use super::with_csrf_cookie;
use crate::discovery::DiscoveryError;
use crate::errors::HttpError;
use crate::federation_preflight::{self, CheckAndEnableError, PreflightError, TransportCause};
use crate::federation_provider_admin::{self, DeleteProviderError};
use crate::handlers::{AppStateExt, CurrentAdmin, CurrentAdminOrAuditor};
use crate::jwks::JwksError;
use crate::response_bounds::BoundsError;
use axum::Form;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use std::str::FromStr;
use sui_id_core::errors::CoreError;
use sui_id_shared::api::FederationProviderSummary;
use sui_id_shared::ids::FederationProviderId;
use sui_id_store::commands::DisableFederationProviderError;
use sui_id_store::models::ValidationStatus;
use sui_id_store::repos::{federation_link, federation_provider};
use sui_id_web::{
    Flash, FlashKind,
    pages::{
        ConfirmDeleteFederationProviderData, ConfirmDisableFederationProviderData,
        ConfirmEnableFederationProviderData,
    },
    render_confirm_delete_federation_provider, render_confirm_disable_federation_provider,
    render_confirm_enable_federation_provider, render_federation_providers,
};

fn parse_provider_id(id: &str) -> Result<FederationProviderId, HttpError> {
    FederationProviderId::from_str(id).map_err(|_| {
        HttpError::html(CoreError::BadRequest(
            "invalid federation provider id".into(),
        ))
    })
}

/// `0.93.0` O6 §3: an administrator's page may show a field name (a
/// fixed, closed set sui-id itself names), or a host/origin sui-id has
/// already parsed and validated — bounded to 255 bytes, with the rest
/// elided, so nothing attacker-sized ever reaches the page even once
/// "safe" content is allowed through. Never applied to a raw
/// upstream-supplied string (a URL's own text, a JSON member name, a
/// `kid`) — those are not shown at all, named or not.
fn bounded(s: &str) -> String {
    const LIMIT: usize = 255;
    if s.len() <= LIMIT {
        return s.to_owned();
    }
    let mut end = LIMIT;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// `0.93.0` O6 §3: [`DiscoveryError`]'s own closed set, each reaching a
/// sentence an administrator can act on. Never the raw `value` a
/// `NotAbsolute`/`NotHttps` carries (upstream-supplied, unbounded,
/// possibly hostile) — only the field name, which sui-id itself chose.
/// `InvalidIssuer`'s string is not shown either, even though it is
/// ordinarily the administrator's own configured issuer: nothing here
/// assumes that always holds.
fn discovery_cause_text(e: &DiscoveryError, t: &'static sui_id_i18n::Strings) -> String {
    match e {
        DiscoveryError::InvalidIssuer(_) => t.fed_cause_invalid_issuer.to_owned(),
        DiscoveryError::NotAbsolute { field, .. } => (t.fed_cause_not_absolute)(field),
        DiscoveryError::NotHttps { field, .. } => (t.fed_cause_not_https)(field),
        DiscoveryError::HasUserinfo { field } => (t.fed_cause_has_userinfo)(field),
        DiscoveryError::HasQuery { field } => (t.fed_cause_has_query)(field),
        DiscoveryError::HasFragment { field } => (t.fed_cause_has_fragment)(field),
        DiscoveryError::Noncanonical { field } => (t.fed_cause_noncanonical)(field),
        DiscoveryError::OriginNotAllowed { field, origin } => {
            (t.fed_cause_origin_not_allowed)(field, &bounded(origin))
        }
        DiscoveryError::IssuerMismatch => t.fed_cause_issuer_mismatch.to_owned(),
        DiscoveryError::ResponseTypesMissingCode => {
            t.fed_cause_response_types_missing_code.to_owned()
        }
        DiscoveryError::GrantTypesMissingAuthorizationCode => {
            t.fed_cause_grant_types_missing_auth_code.to_owned()
        }
        DiscoveryError::CodeChallengeMethodsMissingS256 => {
            t.fed_cause_code_challenge_missing_s256.to_owned()
        }
        DiscoveryError::AuthorizationResponseIssNotTrue => {
            t.fed_cause_auth_response_iss_not_true.to_owned()
        }
        DiscoveryError::SubjectTypesMissingPublic => {
            t.fed_cause_subject_types_missing_public.to_owned()
        }
        DiscoveryError::Bounds(b) => bounds_cause_text(b, t),
        DiscoveryError::TooDeep { limit } => (t.fed_cause_too_deep)(*limit),
        DiscoveryError::DuplicateMember(_) => t.fed_cause_duplicate_member.to_owned(),
        DiscoveryError::NotJson => t.fed_cause_not_json.to_owned(),
    }
}

/// Same rule, for [`JwksError`]. Never the raw member name a
/// `DuplicateMember` carries, nor the `kid` a `DuplicateKid` carries.
fn jwks_cause_text(e: &JwksError, t: &'static sui_id_i18n::Strings) -> String {
    match e {
        JwksError::Bounds(b) => bounds_cause_text(b, t),
        JwksError::NotJson => t.fed_cause_not_json.to_owned(),
        JwksError::NotObject => t.fed_cause_jwks_not_object.to_owned(),
        JwksError::NoKeys => t.fed_cause_jwks_no_keys.to_owned(),
        JwksError::TooDeep { limit } => (t.fed_cause_too_deep)(*limit),
        JwksError::DuplicateMember(_) => t.fed_cause_duplicate_member.to_owned(),
        JwksError::TooManyKeys { limit } => (t.fed_cause_jwks_too_many_keys)(*limit),
        JwksError::KeyNotObject { .. } => t.fed_cause_jwks_key_not_object.to_owned(),
        JwksError::DuplicateKid(_) => t.fed_cause_jwks_duplicate_kid.to_owned(),
    }
}

/// Same rule, for [`BoundsError`], shared by discovery and JWKS: the
/// status as a number, and limits; never the upstream's content-type
/// value or its JSON parser's text. A transport failure while reading is
/// classified like any other.
fn bounds_cause_text(b: &BoundsError, t: &'static sui_id_i18n::Strings) -> String {
    match b {
        BoundsError::WrongStatus(s) => (t.fed_cause_bounds_status)(s.as_u16()),
        BoundsError::MissingContentType | BoundsError::WrongContentType(_) => {
            t.fed_cause_bounds_content_type.to_owned()
        }
        BoundsError::TooLarge { limit } => (t.fed_cause_bounds_too_large)(*limit),
        BoundsError::TooManyMembers { limit }
        | BoundsError::ArrayTooLong { limit }
        | BoundsError::StringTooLong { limit } => (t.fed_cause_bounds_structure)(*limit),
        BoundsError::Json(_) => t.fed_cause_not_json.to_owned(),
        BoundsError::Transport(e) => {
            transport_cause_text(&federation_preflight::classify_transport(e), t)
        }
    }
}

/// Same rule, for [`TransportCause`]. `host` is sui-id's own resolution
/// target (the configured endpoint's hostname), not upstream content;
/// `reason` (`DnsDenied`) is the denied range's *class* -- never an
/// address.
fn transport_cause_text(c: &TransportCause, t: &'static sui_id_i18n::Strings) -> String {
    match c {
        TransportCause::NoCaCertificates => t.fed_cause_no_ca_certificates.to_owned(),
        TransportCause::DnsAnswerRejected { host, count } => {
            (t.fed_cause_dns_over_cap)(&bounded(host), *count, crate::resolver::MAX_ANSWERS)
        }
        TransportCause::DnsDenied { host, reason } => {
            (t.fed_cause_dns_denied)(&bounded(host), reason)
        }
        TransportCause::Tls => t.fed_cause_tls.to_owned(),
        TransportCause::Timeout => t.fed_cause_timeout.to_owned(),
        TransportCause::Unreachable => t.fed_cause_unreachable.to_owned(),
    }
}

/// RFC 096-B2 S4 §D, translated: one i18n key (or, for the typed causes
/// `0.93.0` O6 adds, one rendering function) per [`CheckAndEnableError`]
/// variant, an exhaustive `match` so a new variant fails to compile
/// until it is translated (dispatch §B).
fn check_and_enable_error_text(e: CheckAndEnableError, t: &'static sui_id_i18n::Strings) -> String {
    match e {
        CheckAndEnableError::NotFound => t.fed_err_not_found.to_owned(),
        CheckAndEnableError::Storage => t.fed_err_storage.to_owned(),
        CheckAndEnableError::NotDisabled => t.fed_err_not_disabled.to_owned(),
        // Same operator-facing meaning as `NotDisabled` (`Display`'s own
        // comment, `federation_preflight.rs:427-429`): a race landed on
        // it at the very end instead of at the start.
        CheckAndEnableError::AlreadyEnabled => t.fed_err_not_disabled.to_owned(),
        CheckAndEnableError::NotInConfiguration => t.fed_err_not_in_configuration.to_owned(),
        CheckAndEnableError::PolicyNotRecorded => t.fed_err_policy_not_recorded.to_owned(),
        CheckAndEnableError::Transport(c) => transport_cause_text(&c, t),
        CheckAndEnableError::Discovery(e) => discovery_cause_text(&e, t),
        CheckAndEnableError::NoJwksUri => t.fed_cause_no_jwks_uri.to_owned(),
        CheckAndEnableError::Jwks(e) => jwks_cause_text(&e, t),
        CheckAndEnableError::NoUsableKey => t.fed_cause_no_usable_key.to_owned(),
        CheckAndEnableError::AuthMethodUnsupported => t.fed_err_auth_method_unsupported.to_owned(),
        CheckAndEnableError::NoCommonAlgorithm => t.fed_err_no_common_algorithm.to_owned(),
        CheckAndEnableError::ChangedDuringPreflight => t.fed_err_changed_during_check.to_owned(),
        CheckAndEnableError::ChangedSinceCheck => t.fed_err_changed_since_check.to_owned(),
        CheckAndEnableError::CheckTooOld => t.fed_err_check_too_old.to_owned(),
        CheckAndEnableError::StepUpRequired => t.fed_err_step_up_lapsed.to_owned(),
    }
}

/// Same rule as [`check_and_enable_error_text`], for "Check only"
/// (`preflight_provider` alone, no enable, so [`CheckAndEnableError`]'s
/// three enable-only variants cannot occur here).
fn preflight_error_text(e: PreflightError, t: &'static sui_id_i18n::Strings) -> String {
    match e {
        PreflightError::NotFound => t.fed_err_not_found.to_owned(),
        PreflightError::Storage => t.fed_err_storage.to_owned(),
        PreflightError::NotDisabled => t.fed_err_not_disabled.to_owned(),
        PreflightError::NotInConfiguration => t.fed_err_not_in_configuration.to_owned(),
        PreflightError::PolicyNotRecorded => t.fed_err_policy_not_recorded.to_owned(),
        PreflightError::Transport(c) => transport_cause_text(&c, t),
        PreflightError::Discovery(e) => discovery_cause_text(&e, t),
        PreflightError::NoJwksUri => t.fed_cause_no_jwks_uri.to_owned(),
        PreflightError::Jwks(e) => jwks_cause_text(&e, t),
        PreflightError::NoUsableKey => t.fed_cause_no_usable_key.to_owned(),
        PreflightError::AuthMethodUnsupported => t.fed_err_auth_method_unsupported.to_owned(),
        PreflightError::NoCommonAlgorithm => t.fed_err_no_common_algorithm.to_owned(),
        PreflightError::ChangedDuringPreflight => t.fed_err_changed_during_check.to_owned(),
    }
}

/// Same rule, for [`DisableFederationProviderError`] (three variants;
/// its `NotFound`/`StepUpRequired`/`Storage` share the same operator
/// meaning as the identically-named `CheckAndEnableError` variants, so
/// they share the same keys).
fn disable_error_text(
    e: DisableFederationProviderError,
    t: &'static sui_id_i18n::Strings,
) -> &'static str {
    match e {
        DisableFederationProviderError::NotFound => t.fed_err_not_found,
        DisableFederationProviderError::StepUpRequired => t.fed_err_step_up_lapsed,
        DisableFederationProviderError::Storage => t.fed_err_storage,
    }
}

/// RFC 096-B2 S6 §C: one i18n key per [`DeleteProviderError`] variant.
fn delete_provider_error_text(
    e: DeleteProviderError,
    t: &'static sui_id_i18n::Strings,
) -> &'static str {
    match e {
        DeleteProviderError::NotFound => t.fed_err_not_found,
        DeleteProviderError::StillEnabled => t.fed_err_delete_still_enabled,
        DeleteProviderError::StillConfigured => t.fed_err_still_configured,
        DeleteProviderError::StepUpRequired => t.fed_err_step_up_lapsed,
        DeleteProviderError::Storage => t.fed_err_storage,
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct FederationProvidersListQuery {
    #[serde(default)]
    flash: String,
}

/// The list page's own success flash, threaded across the POST's
/// redirect as a plain query value (RFC 089 carries no secret or
/// trust-field content in it — just which of three fixed outcomes just
/// happened).
fn list_flash(q: &FederationProvidersListQuery, t: &'static sui_id_i18n::Strings) -> Option<Flash> {
    let text = match q.flash.as_str() {
        "enabled" => t.federation_providers_enabled_flash,
        "disabled" => t.federation_providers_disabled_flash,
        "already_disabled" => t.federation_providers_already_disabled_flash,
        "deleted" => t.federation_providers_deleted_flash,
        _ => return None,
    };
    Some(Flash {
        kind: FlashKind::Info,
        text: text.to_owned(),
    })
}

async fn provider_summary(
    app: &crate::state::AppState,
    row: sui_id_store::models::FederationProviderRow,
) -> FederationProviderSummary {
    let has_linked_users = federation_link::any_link_exists_for_provider(&app.db, row.id)
        .await
        .unwrap_or(false);
    let is_configured = app
        .config
        .federation_providers
        .iter()
        .any(|c| c.slug == row.slug);
    FederationProviderSummary {
        id: row.id,
        slug: row.slug,
        display_name: row.display_name,
        issuer: row.issuer,
        enabled: row.enabled,
        requires_review: row.validation_status == ValidationStatus::RequiresReview,
        last_checked_at: row.evidence_observed_at,
        has_linked_users,
        is_configured,
    }
}

pub async fn federation_providers_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, role, _, username): CurrentAdminOrAuditor,
    Query(q): Query<FederationProvidersListQuery>,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let rows = federation_provider::list(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::Store(e)))?;
    let mut summaries = Vec::with_capacity(rows.len());
    for row in rows {
        summaries.push(provider_summary(&app, row).await);
    }
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let flash = list_flash(&q, lang.strings());
    let resp = Html(render_federation_providers(
        role.is_admin(),
        summaries,
        flash,
        token.clone(),
        app.is_dev_mode,
        lang,
        username,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn federation_providers_enable_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(HttpError::html_403_auditor());
    }
    let State(app) = state_ext;
    // Parsed before anything uses it, so `return_to` and the page carry the
    // canonical id, never the raw path segment (S5 review).
    let provider_id = parse_provider_id(&id)?;
    let id = provider_id.to_string();
    let return_to = format!("/admin/federation-providers/{id}/enable-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let row = federation_provider::get(&app.db, provider_id)
        .await
        .map_err(|e| HttpError::html(CoreError::Store(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let data = ConfirmEnableFederationProviderData {
        provider_id: id,
        display_name: row.display_name,
        csrf_token: token.clone(),
        flash: None,
        username,
    };
    let resp = Html(render_confirm_enable_federation_provider(
        data,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn federation_providers_enable_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, ref admin_actor, username): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    // Parsed before anything uses it, so `return_to` and the page carry the
    // canonical id, never the raw path segment (S5 review).
    let provider_id = parse_provider_id(&id)?;
    let id = provider_id.to_string();
    let return_to = format!("/admin/federation-providers/{id}/enable-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    match federation_preflight::check_and_enable(&app, admin_actor, provider_id).await {
        Ok(()) => Ok(Redirect::to("/admin/federation-providers?flash=enabled").into_response()),
        Err(CheckAndEnableError::StepUpRequired) => {
            Ok(crate::handlers::step_up_redirect(&return_to))
        }
        Err(e) => {
            let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
            render_enable_confirm_with_error(&app, &jar, id, provider_id, e, lang, username).await
        }
    }
}

pub async fn federation_providers_check_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, _, username): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<super::forms::CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let provider_id = parse_provider_id(&id)?;
    let id = provider_id.to_string();
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let row = federation_provider::get(&app.db, provider_id)
        .await
        .map_err(|e| HttpError::html(CoreError::Store(e)))?;
    let flash = match federation_preflight::preflight_provider(&app, provider_id).await {
        Ok(_capability) => Some(Flash {
            kind: FlashKind::Info,
            text: lang
                .strings()
                .federation_providers_check_passed_flash
                .into(),
        }),
        Err(e) => Some(Flash {
            kind: FlashKind::Error,
            text: preflight_error_text(e, lang.strings()),
        }),
    };
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmEnableFederationProviderData {
        provider_id: id,
        display_name: row.display_name,
        csrf_token: token.clone(),
        flash,
        username,
    };
    let resp = Html(render_confirm_enable_federation_provider(
        data,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

async fn render_enable_confirm_with_error(
    app: &crate::state::AppState,
    jar: &CookieJar,
    id: String,
    provider_id: FederationProviderId,
    e: CheckAndEnableError,
    lang: sui_id_i18n::Locale,
    username: String,
) -> Result<Response, HttpError> {
    let row = federation_provider::get(&app.db, provider_id)
        .await
        .map_err(|e| HttpError::html(CoreError::Store(e)))?;
    let token = crate::csrf::ensure_token(jar);
    let data = ConfirmEnableFederationProviderData {
        provider_id: id,
        display_name: row.display_name,
        csrf_token: token.clone(),
        flash: Some(Flash {
            kind: FlashKind::Error,
            text: check_and_enable_error_text(e, lang.strings()),
        }),
        username,
    };
    let resp = Html(render_confirm_enable_federation_provider(
        data,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, app, &token))
}

pub async fn federation_providers_disable_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(HttpError::html_403_auditor());
    }
    let State(app) = state_ext;
    // Parsed before anything uses it, so `return_to` and the page carry the
    // canonical id, never the raw path segment (S5 review).
    let provider_id = parse_provider_id(&id)?;
    let id = provider_id.to_string();
    let return_to = format!("/admin/federation-providers/{id}/disable-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let row = federation_provider::get(&app.db, provider_id)
        .await
        .map_err(|e| HttpError::html(CoreError::Store(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let data = ConfirmDisableFederationProviderData {
        provider_id: id,
        display_name: row.display_name,
        csrf_token: token.clone(),
        flash: None,
        username,
    };
    let resp = Html(render_confirm_disable_federation_provider(
        data,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn federation_providers_disable_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, ref admin_actor, username): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    // Parsed before anything uses it, so `return_to` and the page carry the
    // canonical id, never the raw path segment (S5 review).
    let provider_id = parse_provider_id(&id)?;
    let id = provider_id.to_string();
    let return_to = format!("/admin/federation-providers/{id}/disable-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let now = app.clock.now();
    match sui_id_store::commands::disable_federation_provider(
        &app.db,
        admin_actor.user_id(),
        admin_actor.session_id(),
        provider_id,
        now,
    )
    .await
    {
        Ok(Some(_)) => {
            Ok(Redirect::to("/admin/federation-providers?flash=disabled").into_response())
        }
        // RFC 096 `:449-450`: already disabled commits nothing and is
        // not an error -- a neutral flash on the list, not a refusal
        // on the confirm page.
        Ok(None) => {
            Ok(Redirect::to("/admin/federation-providers?flash=already_disabled").into_response())
        }
        Err(DisableFederationProviderError::StepUpRequired) => {
            Ok(crate::handlers::step_up_redirect(&return_to))
        }
        Err(e) => {
            let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
            let row = federation_provider::get(&app.db, provider_id)
                .await
                .map_err(|e| HttpError::html(CoreError::Store(e)))?;
            let token = crate::csrf::ensure_token(&jar);
            let data = ConfirmDisableFederationProviderData {
                provider_id: id,
                display_name: row.display_name,
                csrf_token: token.clone(),
                flash: Some(Flash {
                    kind: FlashKind::Error,
                    text: disable_error_text(e, lang.strings()).into(),
                }),
                username,
            };
            let resp = Html(render_confirm_disable_federation_provider(
                data,
                app.is_dev_mode,
                lang,
            ))
            .into_response();
            Ok(with_csrf_cookie(resp, &app, &token))
        }
    }
}

// ── C18 — delete ──────────────────────────────────────────────────────────

pub async fn federation_providers_delete_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(HttpError::html_403_auditor());
    }
    let State(app) = state_ext;
    // Parsed before anything uses it, so `return_to` and the page carry
    // the canonical id, never the raw path segment (S5 review).
    let provider_id = parse_provider_id(&id)?;
    let id = provider_id.to_string();
    let return_to = format!("/admin/federation-providers/{id}/delete-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let row = federation_provider::get(&app.db, provider_id)
        .await
        .map_err(|e| HttpError::html(CoreError::Store(e)))?;
    let linked_users_count = federation_link::count_for_provider(&app.db, provider_id)
        .await
        .map_err(|e| HttpError::html(CoreError::Store(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let data = ConfirmDeleteFederationProviderData {
        provider_id: id,
        display_name: row.display_name,
        linked_users_count: linked_users_count.max(0) as usize,
        csrf_token: token.clone(),
        flash: None,
        username,
    };
    let resp = Html(render_confirm_delete_federation_provider(
        data,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn federation_providers_delete_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, ref admin_actor, username): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    let provider_id = parse_provider_id(&id)?;
    let id = provider_id.to_string();
    let return_to = format!("/admin/federation-providers/{id}/delete-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    match federation_provider_admin::delete_provider(&app, admin_actor, provider_id).await {
        Ok(()) => Ok(Redirect::to("/admin/federation-providers?flash=deleted").into_response()),
        Err(DeleteProviderError::StepUpRequired) => {
            Ok(crate::handlers::step_up_redirect(&return_to))
        }
        Err(e) => {
            let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
            let row = federation_provider::get(&app.db, provider_id)
                .await
                .map_err(|e| HttpError::html(CoreError::Store(e)))?;
            let linked_users_count = federation_link::count_for_provider(&app.db, provider_id)
                .await
                .map_err(|e| HttpError::html(CoreError::Store(e)))?;
            let token = crate::csrf::ensure_token(&jar);
            let data = ConfirmDeleteFederationProviderData {
                provider_id: id,
                display_name: row.display_name,
                linked_users_count: linked_users_count.max(0) as usize,
                csrf_token: token.clone(),
                flash: Some(Flash {
                    kind: FlashKind::Error,
                    text: delete_provider_error_text(e, lang.strings()).into(),
                }),
                username,
            };
            let resp = Html(render_confirm_delete_federation_provider(
                data,
                app.is_dev_mode,
                lang,
            ))
            .into_response();
            Ok(with_csrf_cookie(resp, &app, &token))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "federation_providers/tests.rs"]
mod tests;
