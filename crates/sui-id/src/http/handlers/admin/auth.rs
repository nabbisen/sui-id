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
    if let Some(cookie) = jar.get(SESSION_COOKIE)
        && let Ok(sid) = SessionId::from_str(cookie.value())
        && session::resolve(&app.db, &app.clock, sid).await.is_ok()
    {
        let dest = if q.next.starts_with('/') {
            q.next.clone()
        } else {
            "/admin".into()
        };
        return Ok(Redirect::to(&dest).into_response());
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
/// The username is looked up first. Only `NotFound` means "unknown
/// locally"; any other lookup error is returned (the uniform 401, logged
/// by `login_post`), and no directory is asked (roadmap
/// `ldap-returning-signin`, L4).
///
/// - A local row whose `source` is `ldap` is a returning directory user:
///   it authenticates against the user sources by its stable id, never
///   against a local credential ([`directory_sign_in`]).
/// - Any other known user takes the local path (`session::login_with_mfa`),
///   which covers lockout, disabled users and MFA.
/// - An unknown user takes the local path (for its timing-equivalent
///   refusal) and then the cascade; a cascade hit upserts a password-less
///   shadow row.
///
/// This preserves the **local-first** invariant (P4): a local user's
/// decision is final and never falls through to a directory.
pub async fn try_login_with_cascade(
    app: &crate::handlers::AppState,
    username: &str,
    password: &str,
    audience: sui_id_core::session::SessionAudience,
) -> sui_id_core::errors::CoreResult<sui_id_core::session::LoginOutcome> {
    use sui_id_core::errors::CoreError;
    use sui_id_store::user_source::{CascadeOutcome, cascade_sources};

    let max_lockout = app.config.security.max_lockout.as_secs();

    let local_user = match sui_id_store::repos::users::find_by_username(&app.db, username).await {
        Ok(user) => Some(user),
        Err(sui_id_store::StoreError::NotFound) => None,
        Err(e) => return Err(e.into()),
    };
    if let Some(user) = local_user
        .as_ref()
        .filter(|u| u.source == sui_id_store::models::UserSource::Ldap)
    {
        return directory_sign_in(app, user, password, audience).await;
    }

    // 1. Local path (always first — P4).
    let local_result = sui_id_core::session::login_with_mfa(
        &app.db,
        &app.clock,
        username,
        password,
        max_lockout,
        audience,
    )
    .await;

    match local_result {
        // Local success or MFA-required: return directly.
        Ok(outcome) => Ok(outcome),
        Err(CoreError::InvalidCredentials) => {
            if local_user.is_some() || app.user_sources.is_empty() {
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
                    directory_session(app, user_id, &record, audience).await
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

/// A returning directory user: a local row with `source = ldap`. Refused
/// without asking any directory when disabled, deleted, locked or linked to
/// a federation provider. Otherwise each user source authenticates the
/// row's stable id; a wrong password is counted on the shadow row (U22),
/// exactly as a local wrong password is.
async fn directory_sign_in(
    app: &crate::handlers::AppState,
    user: &sui_id_store::models::UserRow,
    password: &str,
    audience: sui_id_core::session::SessionAudience,
) -> sui_id_core::errors::CoreResult<sui_id_core::session::LoginOutcome> {
    use sui_id_core::errors::CoreError;

    let refused = |reason: &'static str| async move {
        sui_id_core::session::record_refused_login(&app.db, &app.clock, &user.username, reason)
            .await;
        Err(CoreError::InvalidCredentials)
    };
    if user.is_disabled || user.is_deleted {
        return refused("user disabled or deleted").await;
    }
    if user
        .locked_until
        .is_some_and(|until| until > app.clock.now())
    {
        return refused("account locked").await;
    }
    // Federation provisioning also writes `source = ldap` shadow rows; a
    // federated user has no directory password to check.
    if !sui_id_store::repos::federation_link::list_for_user(&app.db, user.id)
        .await?
        .is_empty()
    {
        return refused("federated user has no directory password").await;
    }
    let Some(stable_id) = user.external_stable_id.as_deref() else {
        return refused("directory user without a stable id").await;
    };

    let mut answered = false;
    for source in &app.user_sources {
        match source.authenticate_stable_id(stable_id, password).await {
            Ok(Some(record)) if record.stable_id == stable_id => {
                // Refresh the display fields through today's upsert; the
                // existing row keeps its username.
                sui_id_store::repos::users::upsert_ldap_shadow(
                    &app.db,
                    sui_id_store::repos::users::LdapShadowData {
                        username: user.username.clone(),
                        display_name: record.display_name.clone(),
                        email: record.email.clone(),
                        external_stable_id: record.stable_id.clone(),
                    },
                    app.clock.now(),
                )
                .await?;
                return directory_session(app, user.id, &record, audience).await;
            }
            Ok(Some(_)) => {
                // The source authenticated a different identity for this
                // id. Refuse uniformly, and do not count it: the password
                // was not wrong for the person who typed it.
                tracing::warn!(
                    source = source.slug(),
                    user_id = %user.id,
                    "user source returned a different stable id for a returning directory user; \
                     sign-in refused"
                );
                return Err(CoreError::InvalidCredentials);
            }
            Ok(None) => answered = true,
            Err(e) => tracing::warn!(
                source = source.slug(),
                error = %e,
                "user source unavailable for a returning directory user"
            ),
        }
    }
    if !answered {
        // No source could check the password: not a wrong password, so not
        // counted. `login_post` logs this error and returns the uniform 401.
        return Err(CoreError::BadRequest(
            "no user source could authenticate a returning directory user".into(),
        ));
    }
    let max_lockout = app.config.security.max_lockout.as_secs();
    sui_id_store::commands::record_login_failure(&app.db, user.id, move |count| {
        sui_id_core::session::lockout_backoff(count, max_lockout)
    })
    .await?;
    Err(CoreError::InvalidCredentials)
}

/// The end of a directory sign-in (first or returning): the audit row, then
/// the MFA branch exactly as `login_with_mfa` takes it, or today's session
/// creation. RFC 102 L03 converts this later.
async fn directory_session(
    app: &crate::handlers::AppState,
    user_id: sui_id_shared::ids::UserId,
    record: &sui_id_store::user_source::ExternalUserRecord,
    audience: sui_id_core::session::SessionAudience,
) -> sui_id_core::errors::CoreResult<sui_id_core::session::LoginOutcome> {
    use sui_id_core::errors::CoreError;

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

    // A factor enrolled through RFC 102 B7's re-bind must be asked for here;
    // skipping it would be an MFA bypass. A7 first: a destination the user
    // cannot read gets no pending row.
    if sui_id_core::mfa::is_mfa_enabled(&app.db, user_id).await? {
        let user = sui_id_store::repos::users::get(&app.db, user_id).await?;
        if audience == sui_id_core::session::SessionAudience::AdminReaders
            && !user.role.can_read_admin()
        {
            return Ok(sui_id_core::session::LoginOutcome::AudienceRefused);
        }
        let pending = sui_id_core::mfa::issue_pending_mfa(&app.db, &app.clock, user_id).await?;
        let _ = sui_id_store::repos::audit::append(
            &app.db,
            &sui_id_store::models::AuditLogRow {
                at: app.clock.now(),
                actor: Some(user_id),
                action: "auth.login.password_ok_mfa_required".into(),
                target: Some(user_id.to_string()),
                result: "ok".into(),
                note: None,
            },
        )
        .await;
        return Ok(sui_id_core::session::LoginOutcome::MfaRequired { pending });
    }

    // Create a session for the shadow user.
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
    let _ = sui_id_store::repos::users::set_last_login(&app.db, &user_id, now).await;
    // A correct directory password ends a run of counted wrong ones, as a
    // local sign-in does.
    let _ = sui_id_store::repos::users::clear_lockout(&app.db, user_id).await;
    Ok(sui_id_core::session::LoginOutcome::SessionEstablished(
        session_row,
    ))
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
    request_id: Option<axum::Extension<crate::request_id::RequestId>>,
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
    let target = if form.next.starts_with('/') {
        form.next.clone()
    } else {
        "/admin".into()
    };
    // RFC 102 A7: an admin-panel destination is decided before the sign-in
    // writes anything, so a refused sign-in commits no session and no
    // success event.
    let audience = if target.starts_with("/oauth2/") || target.starts_with("/me/") {
        session::SessionAudience::Any
    } else {
        session::SessionAudience::AdminReaders
    };
    let no_admin_access = |next: String| {
        let t = lang.strings();
        let flash = Flash {
            kind: FlashKind::Error,
            text: t.login_no_admin_access.into(),
        };
        let next = if next.is_empty() { None } else { Some(next) };
        Html(render_login(Some(flash), next, lang, false, None)).into_response()
    };
    // RFC 005: local-first cascade; falls back to external user-sources
    // when the username is not found locally.
    match try_login_with_cascade(&app, form.username.trim(), &form.password, audience).await {
        Ok(session::LoginOutcome::AudienceRefused) => Ok(no_admin_access(form.next)),
        Ok(session::LoginOutcome::SessionEstablished(row)) => {
            // RFC 006: record successful sign-in.
            if let Some(m) = app.metric() {
                m.signin(sui_id_store::metrics::signin_result::SUCCESS);
            }

            // The admin login page also serves as the authentication gate
            // for the OIDC authorize flow (next = "/oauth2/authorize?...").
            // Any authenticated user — admin or not — may complete that
            // flow. But the admin panel itself requires admin or auditor
            // role; check here so a non-privileged user gets a clear
            // message rather than a 403 page after being redirected.
            //
            // A local password sign-in already refused this before L01
            // (`AudienceRefused` above). The directory cascade has not been
            // converted yet (RFC 102 L03): its session row is already
            // written here, so if the role check fails we simply don't hand
            // out the cookie and the row expires unused.
            if audience == session::SessionAudience::AdminReaders {
                let user = users::get(&app.db, row.user_id)
                    .await
                    .map_err(|e| HttpError::html(CoreError::from(e)))?;
                if !user.role.can_read_admin() {
                    return Ok(no_admin_access(form.next));
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
        Err(err) => {
            // R11 1b: every failure gets the same response below, so the
            // cause of a non-credential failure (a store error from U22,
            // for instance) would otherwise be lost. Log it; never the
            // submitted password. The request id is recorded explicitly:
            // the request span is not reliably entered once the handler
            // has been resumed after an await.
            if !matches!(err, CoreError::InvalidCredentials) {
                let request_id = request_id.as_ref().map(|e| e.0.0.as_str()).unwrap_or("-");
                tracing::error!(
                    request_id,
                    error = %err,
                    detail = ?err,
                    "sign-in failed for a reason other than invalid credentials; \
                     returning the uniform failure response"
                );
            }
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
    request_id: Option<axum::Extension<crate::request_id::RequestId>>,
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
    let max_lockout = app.config.security.max_lockout.as_secs();
    match sui_id_core::mfa::verify_pending(&app.db, &app.clock, pending_id, &form.code, max_lockout)
        .await
    {
        Ok(session) => {
            let cookie = session_cookie(session.id.to_string(), app.config.server.cookie_secure);
            // Compose the redirect target from the optional next cookie.
            let next_target = jar
                .get(PENDING_MFA_NEXT_COOKIE)
                .map(|c| c.value().to_owned())
                .filter(|s| s.starts_with('/'))
                .unwrap_or_else(|| "/admin".into());
            // `auth.mfa.success` was committed by L02 with the session.
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
        Err(err) => {
            // A wrong code was counted by L07 inside `verify_pending`; a
            // lost guard is `Unauthenticated`. Anything else is a storage
            // or internal failure: the same response, with the cause
            // logged (RFC 102 C1, R11 1b).
            if !matches!(
                err,
                CoreError::InvalidCredentials | CoreError::Unauthenticated
            ) {
                let request_id = request_id.as_ref().map(|e| e.0.0.as_str()).unwrap_or("-");
                tracing::error!(
                    request_id,
                    error = %err,
                    detail = ?err,
                    "second-factor sign-in failed for a reason other than a wrong code; \
                     returning the uniform failure response"
                );
            }
            let t = lang.strings();
            let flash = Flash {
                kind: FlashKind::Error,
                text: t.mfa_challenge_failed_flash.into(),
            };
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
    if let Some(c) = jar.get(SESSION_COOKIE)
        && let Ok(sid) = SessionId::from_str(c.value())
    {
        let _ = session::logout(&app.db, sid).await;
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
