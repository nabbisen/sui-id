//! Admin handlers for auth (RFC 066).

use super::forms::CsrfOnlyForm;
use super::with_csrf_cookie;
use crate::errors::HttpError;
use crate::handlers::{
    AppStateExt, PENDING_MFA_COOKIE, PENDING_MFA_NEXT_COOKIE, SESSION_COOKIE,
    clear_pending_mfa_cookie, clear_pending_mfa_next_cookie, clear_session_cookie,
    pending_mfa_cookie, pending_mfa_next_cookie, session_cookie,
};
use axum::Form;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use std::str::FromStr;
use sui_id_core::errors::CoreError;
use sui_id_core::session;
use sui_id_shared::ids::SessionId;
use sui_id_store::repos::users;
use sui_id_web::{Flash, FlashKind, LoginContext, render_login};

#[derive(Debug, Deserialize)]

pub struct LoginForm {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub next: String,
}

/// Query parameters accepted by `GET /admin/login`.
/// The `next` value is URL-encoded by the `/oauth2/authorize` endpoint
/// so the login form can redirect back after a successful sign-in.
#[derive(Debug, Deserialize, Default)]
pub struct LoginGetQuery {
    #[serde(default)]
    pub next: String,
}

/// Derive the `LoginContext` from the `?next=` parameter (RFC 091).
///
/// Trusted-name invariant: `OidcAuthorize` is only produced after a
/// successful synchronous lookup of the client record by UUID.
/// A malformed or unknown client_id falls back to `AdminPanel`.
async fn derive_login_context(db: &sui_id_store::Database, next: &str) -> LoginContext {
    if next.starts_with("/oauth2/") {
        // Extract client_id from the authorize URL and look up the
        // registered client name.  Any parse or DB failure → AdminPanel.
        let client_name = url::Url::parse(&format!("https://localhost{next}"))
            .ok()
            .and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == "client_id")
                    .map(|(_, v)| v.into_owned())
            })
            .and_then(|cid| cid.parse::<sui_id_shared::ids::ClientId>().ok())
            .and_then(|cid| {
                // Block-in-place is acceptable here because login_get is
                // already on an async Tokio thread and this is a single
                // short SQLite lookup.
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(sui_id_store::repos::clients::get(db, cid))
                        .ok()
                })
            })
            .map(|r| r.name);
        if let Some(name) = client_name {
            return LoginContext::OidcAuthorize { client_name: name };
        }
    } else if next.starts_with("/me/") {
        return LoginContext::SelfService;
    }
    LoginContext::AdminPanel
}

pub async fn login_get(
    jar: CookieJar,
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    Query(q): Query<LoginGetQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Already logged in? Forward to `next` if present, otherwise /admin.
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        if let Ok(sid) = SessionId::from_str(cookie.value()) {
            if session::resolve(&app.db, &app.clock, sid).await.is_ok() {
                let dest = if q.next.starts_with('/') {
                    q.next.clone()
                } else {
                    "/admin".into()
                };
                return Ok(Redirect::to(&dest).into_response());
            }
        }
    }
    // Thread `next` into the form so login_post can redirect to it.
    let next = if q.next.is_empty() {
        None
    } else {
        Some(q.next.clone())
    };
    // RFC 091: derive LoginContext from `next` for context-aware copy.
    let login_ctx = derive_login_context(&app.db, &q.next).await;
    Ok(Html(render_login(None, next, lang, false, Some(login_ctx))).into_response())
}

/// Attempt to sign in, trying the local credential store first, then any
/// configured external user-sources (RFC 005 cascade).
///
/// The local path (`session::login_with_mfa`) is tried unconditionally and
/// covers all local-only invariants (lockout, disabled, MFA).
///
/// If the local path returns `Err(CoreError::InvalidCredentials)` AND the
/// username is not found in the local `users` table (meaning it is genuinely
/// unknown locally, not just a wrong password), the cascade is attempted.
/// On a cascade hit, a password-less shadow row is upserted and a session is
/// created via `session::create_session`.
///
/// This function preserves the **local-first** invariant (P4): even if the
/// cascade would succeed, a local user's locked account still blocks
/// sign-in through the cascade — the local check runs first and its
/// lockout decision is final.
pub async fn try_login_with_cascade(
    app: &crate::handlers::AppState,
    username: &str,
    password: &str,
) -> sui_id_core::errors::CoreResult<sui_id_core::session::LoginOutcome> {
    use sui_id_core::errors::CoreError;
    use sui_id_store::user_source::{CascadeOutcome, cascade_sources};

    let max_lockout = app.config.security.max_lockout.as_secs();

    // 1. Try local path (always first — P4).
    let local_result =
        sui_id_core::session::login_with_mfa(&app.db, &app.clock, username, password, max_lockout)
            .await;

    match local_result {
        // Local success or MFA-required: return directly.
        Ok(outcome) => return Ok(outcome),
        Err(CoreError::InvalidCredentials) => {
            // Could be wrong password OR unknown user.  Check whether the
            // username exists locally to decide whether to try the cascade.
            let is_unknown_locally =
                sui_id_store::repos::users::find_by_username(&app.db, username)
                    .await
                    .is_err(); // NotFound or any DB error → treat as unknown

            if !is_unknown_locally || app.user_sources.is_empty() {
                // Known locally but wrong password, OR no external sources
                // configured — return the original error.
                return Err(CoreError::InvalidCredentials);
            }

            // Unknown locally: try the external cascade.
            match cascade_sources(&app.user_sources, username, password).await {
                CascadeOutcome::Matched(record) => {
                    // Resolve (or create) the local shadow row.
                    let shadow_data = sui_id_store::repos::users::LdapShadowData {
                        username: resolve_shadow_username(&app.db, &record.display_username).await,
                        display_name: record.display_name.clone(),
                        email: record.email.clone(),
                        external_stable_id: record.stable_id.clone(),
                    };
                    let user_id = sui_id_store::repos::users::upsert_ldap_shadow(
                        &app.db,
                        shadow_data,
                        app.clock.now(),
                    )
                    .await
                    .map_err(CoreError::from)?;

                    // Emit audit event.
                    let _ = sui_id_store::repos::audit::append(
                        &app.db,
                        &sui_id_store::models::AuditLogRow {
                            at: app.clock.now(),
                            actor: Some(user_id),
                            action: "auth.user_source.matched".into(),
                            target: Some(user_id.to_string()),
                            result: "ok".into(),
                            note: Some(format!(
                                "source={} stable_id={}",
                                record.source_slug, record.stable_id
                            )),
                        },
                    )
                    .await;

                    // Create a session for the shadow user (no MFA on first sign-in).
                    let now = app.clock.now();
                    let session_row = sui_id_store::models::SessionRow {
                        id: sui_id_shared::ids::SessionId::new(),
                        user_id,
                        expires_at: now + chrono::Duration::hours(24),
                        created_at: now,
                        revoked_at: None,
                        auth_methods: vec![sui_id_shared::AuthMethod::Fed],
                        last_step_up_at: None,
                        last_used_at: None,
                    };
                    sui_id_store::repos::sessions::insert(&app.db, &session_row)
                        .await
                        .map_err(CoreError::from)?;
                    // Session cap enforcement happens on the next local login;
                    // omitted here (the cap function is internal to sui-id-core).
                    let _ =
                        sui_id_store::repos::users::set_last_login(&app.db, &user_id, now).await;
                    Ok(sui_id_core::session::LoginOutcome::SessionEstablished(
                        session_row,
                    ))
                }
                CascadeOutcome::NotFound => {
                    // All external sources returned None or errored.
                    // Emit a transport-failure audit event if at least one
                    // source errored (the cascade already logged the individual
                    // errors at WARN level).
                    Err(CoreError::InvalidCredentials)
                }
            }
        }
        Err(other) => Err(other),
    }
}

/// Resolve a display_username for a new shadow row.
///
/// If `proposed` is already taken (another local user), appends numeric
/// suffixes until a free name is found.  This is best-effort: in the
/// extremely unlikely case of 1000 collisions we fall back to a UUID-derived
/// name.
async fn resolve_shadow_username(db: &sui_id_store::Database, proposed: &str) -> String {
    // Check the proposed name first (fast path).
    if sui_id_store::repos::users::find_by_username(db, proposed)
        .await
        .is_err()
    {
        return proposed.to_owned();
    }
    // Conflict: try "alice2", "alice3", …
    for n in 2u32..=1000 {
        let candidate = format!("{proposed}{n}");
        if sui_id_store::repos::users::find_by_username(db, &candidate)
            .await
            .is_err()
        {
            return candidate;
        }
    }
    // Extreme fallback: use a UUID suffix.
    format!("{proposed}-{}", uuid::Uuid::new_v4().simple())
}

pub async fn login_post(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Login,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;
    // RFC 005: local-first cascade; falls back to external user-sources
    // when the username is not found locally.
    match try_login_with_cascade(&app, form.username.trim(), &form.password).await {
        Ok(session::LoginOutcome::SessionEstablished(row)) => {
            // RFC 006: record successful sign-in.
            if let Some(m) = app.metric() {
                m.signin(sui_id_store::metrics::signin_result::SUCCESS);
            }
            let target = if form.next.starts_with('/') {
                form.next.clone()
            } else {
                "/admin".into()
            };

            // The admin login page also serves as the authentication gate
            // for the OIDC authorize flow (next = "/oauth2/authorize?...").
            // Any authenticated user — admin or not — may complete that
            // flow. But the admin panel itself requires admin or auditor
            // role; check here so a non-privileged user gets a clear
            // message rather than a 403 page after being redirected.
            //
            // The session row is already written at this point; if the
            // role check fails we simply don't hand out the cookie and the
            // row expires unused after its normal 24-hour lifetime.
            let is_oidc_or_me = target.starts_with("/oauth2/") || target.starts_with("/me/");
            if !is_oidc_or_me {
                let user = users::get(&app.db, row.user_id)
                    .await
                    .map_err(|e| HttpError::html(CoreError::from(e)))?;
                if !user.role.can_read_admin() {
                    let t = lang.strings();
                    let flash = Flash {
                        kind: FlashKind::Error,
                        text: t.login_no_admin_access.into(),
                    };
                    let next = if form.next.is_empty() {
                        None
                    } else {
                        Some(form.next)
                    };
                    return Ok(
                        Html(render_login(Some(flash), next, lang, false, None)).into_response()
                    );
                }
            }

            let cookie = session_cookie(row.id.to_string(), app.config.server.cookie_secure);
            let jar = jar.add(cookie);
            Ok((jar, Redirect::to(&target)).into_response())
        }
        Ok(session::LoginOutcome::MfaRequired { pending }) => {
            // RFC 006: password correct but MFA still required.
            if let Some(m) = app.metric() {
                m.signin(sui_id_store::metrics::signin_result::MFA_FAILED);
            }
            // Drop the user a short-lived cookie pointing at the
            // pending row, and bounce them into the MFA challenge page.
            let cookie =
                pending_mfa_cookie(pending.id.to_string(), app.config.server.cookie_secure);
            let next_cookie = if !form.next.is_empty() {
                Some(pending_mfa_next_cookie(
                    form.next.clone(),
                    app.config.server.cookie_secure,
                ))
            } else {
                None
            };
            let jar = jar.add(cookie);
            let jar = match next_cookie {
                Some(c) => jar.add(c),
                None => jar,
            };
            Ok((jar, Redirect::to("/admin/login/mfa")).into_response())
        }
        Err(_) => {
            // RFC 006: failed sign-in (wrong password, locked, or disabled).
            if let Some(m) = app.metric() {
                m.signin(sui_id_store::metrics::signin_result::WRONG_PASSWORD);
            }
            let flash = Flash {
                kind: FlashKind::Error,
                text: "Sign-in failed. Check your username and password.".into(),
            };
            let next = if form.next.is_empty() {
                None
            } else {
                Some(form.next)
            };
            Ok((
                StatusCode::UNAUTHORIZED,
                Html(render_login(Some(flash), next, lang, false, None)),
            )
                .into_response())
        }
    }
}

pub async fn mfa_challenge_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Best-effort: if we can resolve the pending-MFA row, look up
    // whether the user has any passkeys so the page can offer that
    // path. If we can't (cookie missing or row gone), default to
    // hiding the passkey button — the user can still type a TOTP code.
    let has_passkey = {
        let pid_opt = jar
            .get(crate::handlers::PENDING_MFA_COOKIE)
            .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok());
        if let Some(pid) = pid_opt {
            let row_opt = sui_id_store::repos::login_pending_mfa::get(&app.db, pid)
                .await
                .ok()
                .flatten();
            if let Some(row) = row_opt {
                sui_id_core::webauthn::has_credentials(&app.db, row.user_id)
                    .await
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        }
    };
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(sui_id_web::render_mfa_challenge(
        None,
        token.clone(),
        has_passkey,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]

pub struct MfaChallengeForm {
    pub code: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn mfa_challenge_post(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<MfaChallengeForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Same rate-limit bucket as password attempts: a user who is past
    // the password step still uses a single login budget.
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Login,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let pending_value = match jar.get(PENDING_MFA_COOKIE) {
        Some(c) => c.value().to_owned(),
        None => {
            return Ok(Redirect::to("/admin/login").into_response());
        }
    };
    let pending_id = match pending_value.parse::<sui_id_shared::ids::PendingMfaId>() {
        Ok(id) => id,
        Err(_) => return Ok(Redirect::to("/admin/login").into_response()),
    };
    match sui_id_core::mfa::verify_pending(&app.db, &app.clock, pending_id, &form.code).await {
        Ok(session) => {
            let cookie = session_cookie(session.id.to_string(), app.config.server.cookie_secure);
            // Compose the redirect target from the optional next cookie.
            let next_target = jar
                .get(PENDING_MFA_NEXT_COOKIE)
                .map(|c| c.value().to_owned())
                .filter(|s| s.starts_with('/'))
                .unwrap_or_else(|| "/admin".into());
            // Audit the MFA success.
            let _ = sui_id_store::repos::audit::append(
                &app.db,
                &sui_id_store::models::AuditLogRow {
                    at: app.clock.now(),
                    actor: Some(session.user_id),
                    action: "auth.mfa.success".into(),
                    target: Some(session.user_id.to_string()),
                    result: "ok".into(),
                    note: None,
                },
            )
            .await;
            // RFC 006: MFA verified → full sign-in success.
            if let Some(m) = app.metric() {
                m.signin(sui_id_store::metrics::signin_result::SUCCESS);
            }
            let jar = jar
                .add(cookie)
                .add(clear_pending_mfa_cookie(app.config.server.cookie_secure))
                .add(clear_pending_mfa_next_cookie(
                    app.config.server.cookie_secure,
                ));
            Ok((jar, Redirect::to(&next_target)).into_response())
        }
        Err(_) => {
            let t = lang.strings();
            let flash = Flash {
                kind: FlashKind::Error,
                text: t.mfa_challenge_failed_flash.into(),
            };
            let _ = sui_id_store::repos::audit::append(
                &app.db,
                &sui_id_store::models::AuditLogRow {
                    at: app.clock.now(),
                    actor: None,
                    action: "auth.mfa.failure".into(),
                    target: None,
                    result: "denied".into(),
                    note: None,
                },
            )
            .await;
            // RFC 006: MFA code rejected.
            if let Some(m) = app.metric() {
                m.signin(sui_id_store::metrics::signin_result::MFA_FAILED);
            }
            let has_passkey = {
                let pid_opt2 = jar
                    .get(crate::handlers::PENDING_MFA_COOKIE)
                    .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok());
                if let Some(pid) = pid_opt2 {
                    let row_opt2 = sui_id_store::repos::login_pending_mfa::get(&app.db, pid)
                        .await
                        .ok()
                        .flatten();
                    if let Some(row) = row_opt2 {
                        sui_id_core::webauthn::has_credentials(&app.db, row.user_id)
                            .await
                            .unwrap_or(false)
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            let token = crate::csrf::ensure_token(&jar);
            let resp = (
                StatusCode::UNAUTHORIZED,
                Html(sui_id_web::render_mfa_challenge(
                    Some(flash),
                    token.clone(),
                    has_passkey,
                    lang,
                )),
            )
                .into_response();
            Ok(with_csrf_cookie(resp, &app, &token))
        }
    }
}

pub async fn logout(
    jar: CookieJar,
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    axum::Form(form): axum::Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Validate CSRF to prevent logout-CSRF attacks.
    // On failure, redirect to login rather than returning 403 —
    // a stale CSRF token (e.g. after a previous logout) should not
    // leave the operator staring at an error page.
    if crate::handlers::enforce_csrf(&jar, Some(&form.csrf)).is_err() {
        return Ok(Redirect::to("/admin/login").into_response());
    }
    if let Some(c) = jar.get(SESSION_COOKIE) {
        if let Ok(sid) = SessionId::from_str(c.value()) {
            let _ = session::logout(&app.db, sid).await;
        }
    }
    let jar = jar.add(clear_session_cookie(app.config.server.cookie_secure));
    // Render the login page with a "Signed out" confirmation.
    let flash = Flash {
        kind: FlashKind::Info,
        text: lang.strings().signed_out_flash.into(),
    };
    Ok((
        jar,
        Html(render_login(Some(flash), None, lang, false, None)),
    )
        .into_response())
}
