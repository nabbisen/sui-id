//! /me/security language tab handlers (RFC 068).

use crate::{csrf, errors::HttpError};
use axum::extract::{Form, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use sui_id_core::errors::CoreError;


use super::forms::*;
use crate::handlers::{AppStateExt, CurrentUser, enforce_csrf};
use crate::handlers::admin::with_csrf_cookie;
use sui_id_web::pages::{MeShellData, MeTab, MeLanguageData};

pub async fn language_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
    axum::extract::Query(q): axum::extract::Query<LanguageGetQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let lang = req_locale;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let shell = MeShellData {
        username: user.username.clone(),
        is_admin: user.is_admin,
        active_tab: MeTab::Language,
    };
    let just_saved = q.saved == Some(1);
    let flash: Option<sui_id_web::Flash> = None;
    let csrf_tok = csrf::ensure_token(&jar);
    let resp = axum::response::Html(sui_id_web::render_me_language(
        MeLanguageData {
            shell,
            current_preferred_lang: user.preferred_lang.clone(),
            csrf_token: csrf_tok.clone(),
            just_saved,
        },
        flash, app.is_dev_mode, lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}

pub async fn language_post(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
    Form(form): Form<LanguageForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let lang = req_locale;
    let new_lang = if form.locale.trim().is_empty() {
        None
    } else {
        match sui_id_i18n::Locale::parse(form.locale.trim()) {
            Some(loc) => Some(loc.tag().to_string()),
            None => return Err(HttpError::html(CoreError::BadRequest(
                "unsupported locale".into()
            )).with_lang(lang)),
        }
    };
    sui_id_store::repos::users::set_preferred_lang(
        &app.db,
        user_id,
        new_lang.as_deref(),
        app.clock.now(),
    ).await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    Ok(Redirect::to("/me/security/language?saved=1").into_response())
}
