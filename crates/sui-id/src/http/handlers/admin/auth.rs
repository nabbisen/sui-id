//! Admin handlers for auth (RFC 066).

use super::forms::CsrfOnlyForm;
use super::with_csrf_cookie;
use crate::errors::HttpError;
use crate::handlers::federation::{
    FED_MFA_BROWSER_BINDING_COOKIE, FED_MFA_CSRF_COOKIE, FED_MFA_PENDING_COOKIE,
    clear_fed_mfa_browser_binding_cookie, clear_fed_mfa_csrf_cookie, clear_fed_mfa_pending_cookie,
};
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
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::str::FromStr;
use sui_id_core::errors::CoreError;
use sui_id_core::session;
use sui_id_shared::ids::SessionId;
use sui_id_web::{Flash, FlashKind, LoginContext, render_login};

#[derive(Debug, Deserialize)]

pub struct LoginForm {
    pub username: String,
    pub password: SecretString,
    #[serde(default)]
    pub next: String,
}

/// Query parameters accepted by `GET /admin/login`.
/// The `next` value is URL-encoded by the `/oauth2/authorize` endpoint
/// so the login form can redirect back after a successful sign-in.
///
/// `fed_error` (RFC 096-B1 stage 6e) is caller-supplied, exactly like
/// `next` already is -- anyone can visit `/admin/login?fed_error=anything`.
/// It is never rendered itself; `login_get` passes it through
/// `crate::handlers::federation::fed_error_message`'s bounded map, which
/// is the only thing that reaches the page.
#[derive(Debug, Deserialize, Default)]
pub struct LoginGetQuery {
    #[serde(default)]
    pub next: String,
    #[serde(default)]
    pub fed_error: Option<String>,
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
    // RFC 096-B1 stage 6e: `fed_error`, mapped to one of four bounded
    // messages -- never the raw value itself (see `LoginGetQuery`'s own
    // doc comment and `fed_error_message`'s).
    let flash = q
        .fed_error
        .as_deref()
        .map(|v| crate::handlers::federation::fed_error_message(v, lang.strings()));
    Ok(Html(render_login(flash, next, lang, false, Some(login_ctx))).into_response())
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
                    directory_session(app, None, shadow_data, &record, audience).await
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
                // The upsert refreshes the display fields; the existing row
                // keeps its username.
                let shadow = sui_id_store::repos::users::LdapShadowData {
                    username: user.username.clone(),
                    display_name: record.display_name.clone(),
                    email: record.email.clone(),
                    external_stable_id: record.stable_id.clone(),
                };
                return directory_session(app, Some(user.id), shadow, &record, audience).await;
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

/// The end of a directory sign-in (first or returning), after a user source
/// verified the password. `known_user` is the shadow row a returning user
/// was found by; for a first sign-in the row is looked up by stable id.
///
/// - **MFA branch**, exactly as `login_with_mfa` takes it: A7, then the
///   shadow upsert commits, then the pending-MFA row and the best-effort
///   `auth.login.password_ok_mfa_required` row. Those three are what is
///   written before the second factor; L02 commits the rest.
/// - **No MFA:** A7 from the role (a new shadow user is always `user`),
///   before any write; then **L03** commits the upsert, the bookkeeping,
///   the session, eviction and `auth.login.success` together (RFC 102).
async fn directory_session(
    app: &crate::handlers::AppState,
    known_user: Option<sui_id_shared::ids::UserId>,
    shadow: sui_id_store::repos::users::LdapShadowData,
    record: &sui_id_store::user_source::ExternalUserRecord,
    audience: sui_id_core::session::SessionAudience,
) -> sui_id_core::errors::CoreResult<sui_id_core::session::LoginOutcome> {
    use sui_id_core::errors::CoreError;
    use sui_id_core::session::{LoginOutcome, SessionAudience};

    let existing = match known_user {
        Some(id) => Some(sui_id_store::repos::users::get(&app.db, id).await?),
        None => match sui_id_store::repos::users::find_by_external_stable_id(
            &app.db,
            &sui_id_store::models::UserSource::Ldap,
            &record.stable_id,
        )
        .await
        {
            Ok(user) => Some(user),
            Err(sui_id_store::StoreError::NotFound) => None,
            Err(e) => return Err(e.into()),
        },
    };
    // RFC 102 A7, before any write.
    let can_read_admin = existing
        .as_ref()
        .is_some_and(|user| user.role.can_read_admin());
    if audience == SessionAudience::AdminReaders && !can_read_admin {
        return Ok(LoginOutcome::AudienceRefused);
    }

    // A factor enrolled through RFC 102 B7's re-bind must be asked for here;
    // skipping it would be an MFA bypass.
    if let Some(user) = &existing
        && sui_id_core::mfa::is_mfa_enabled(&app.db, user.id).await?
    {
        sui_id_store::repos::users::upsert_ldap_shadow(&app.db, shadow, app.clock.now()).await?;
        let pending = sui_id_core::mfa::issue_pending_mfa(&app.db, &app.clock, user.id).await?;
        let _ = sui_id_store::repos::audit::append(
            &app.db,
            &sui_id_store::models::AuditLogRow {
                at: app.clock.now(),
                actor: Some(user.id),
                action: "auth.login.password_ok_mfa_required".into(),
                target: Some(user.id.to_string()),
                result: "ok".into(),
                note: None,
            },
        )
        .await;
        return Ok(LoginOutcome::MfaRequired { pending });
    }

    let now = app.clock.now();
    let session_row = sui_id_store::models::SessionRow {
        id: sui_id_shared::ids::SessionId::new(),
        user_id: existing.as_ref().map(|u| u.id).unwrap_or_default(),
        expires_at: now + chrono::Duration::hours(24),
        created_at: now,
        revoked_at: None,
        auth_methods: vec![sui_id_shared::AuthMethod::Fed],
        last_step_up_at: None,
        last_used_at: None,
    };
    // RFC 102 L03. A failure is never counted (A9); `login_post` logs it and
    // returns the uniform 401. A lost re-read (the user was disabled,
    // deleted or locked meanwhile, or another sign-in created the row
    // first) is an ordinary refusal.
    sui_id_store::commands::sign_in_from_directory(
        &app.db,
        shadow,
        record.source_slug.clone(),
        session_row.clone(),
    )
    .await
    .map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::InvalidCredentials,
        other => other.into(),
    })?;
    Ok(LoginOutcome::SessionEstablished(session_row))
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
    match try_login_with_cascade(
        &app,
        form.username.trim(),
        form.password.expose_secret(),
        audience,
    )
    .await
    {
        Ok(session::LoginOutcome::AudienceRefused) => Ok(no_admin_access(form.next)),
        Ok(session::LoginOutcome::SessionEstablished(row)) => {
            // RFC 006: record successful sign-in.
            if let Some(m) = app.metric() {
                m.signin(sui_id_store::metrics::signin_result::SUCCESS);
            }

            // The admin panel needs a role that can read it. Every sign-in
            // path (L01, L02's password step, L03) refuses a non-reader
            // bound for it before writing anything (`AudienceRefused`
            // above), so no session reaches here that nobody will hold.

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
    // RFC 096-B1 stage 12b: a federated continuation's own CSRF-binding
    // value, read back here so the POST form can carry it in a hidden
    // field -- `None` for the local path, unchanged. `has_passkey`, above,
    // needs no federated branch of its own: the generic `login_pending_mfa`
    // row is still issued alongside the federated one (this stage's own
    // "leave the dual-row emission in place"), so it already answers
    // correctly either way.
    let fed_csrf = jar.get(FED_MFA_CSRF_COOKIE).map(|c| c.value().to_owned());
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(sui_id_web::render_mfa_challenge(
        None,
        token.clone(),
        has_passkey,
        lang,
        fed_csrf,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]

pub struct MfaChallengeForm {
    pub code: SecretString,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    /// RFC 096-B1 stage 12b: a federated continuation's own CSRF-binding
    /// value, echoed back from the hidden field `render_mfa_challenge`
    /// renders it into. Empty, and never read, on the local path.
    #[serde(default)]
    pub fed_csrf: String,
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

    // RFC 096-B1 stage 12b: branch on `FED_MFA_PENDING_COOKIE` being
    // present. Present -> the federated path, switched this stage onto
    // `FederatedMfaVerifier`. Absent -> the existing local path below,
    // unchanged. Not also "and parsing" as a precondition of the branch
    // itself: `federated_mfa::promote` parses `presented_pending_id`
    // internally and folds an unparseable value into the same
    // `BindingFailure` every other binding refusal already is, so
    // re-validating the shape here first would just be a second,
    // redundant gate in front of a check that already exists and already
    // produces the uniform response this handler needs either way.
    if let Some(fed_pending_raw) = jar
        .get(FED_MFA_PENDING_COOKIE)
        .map(|c| c.value().to_owned())
    {
        return mfa_challenge_post_federated(app, request_id, lang, jar, form, fed_pending_raw)
            .await;
    }

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
    match sui_id_core::mfa::verify_pending(
        &app.db,
        &app.clock,
        pending_id,
        form.code.expose_secret(),
        max_lockout,
    )
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
                    None,
                )),
            )
                .into_response();
            Ok(with_csrf_cookie(resp, &app, &token))
        }
    }
}

/// RFC 096-B1 stage 12b: the federated half of [`mfa_challenge_post`],
/// switched onto `sui_id_core::federated_mfa::promote` rather than the
/// generic `login_pending_mfa` row `FederatedMfaVerifier` (stages 10a,
/// 11) was built to replace. The generic row is still issued alongside
/// the federated one (`complete_federated_signin`'s own "leave the
/// dual-row emission in place") and is what the webauthn-passkey path
/// (`/admin/login/webauthn/*`, untouched this stage) still completes
/// through; this function only switches the *code* submission.
///
/// `presented_pending_id` goes to `promote` as the raw cookie value,
/// unparsed by this function first (see the caller's own comment on
/// why). A separate, best-effort row read happens only in the two arms
/// that actually need a field no `F03Outcome` variant carries:
/// `Promoted`'s own `next_path`, and the rejection arms' own `user_id`
/// for the account-lock hook (§3).
async fn mfa_challenge_post_federated(
    app: crate::handlers::AppState,
    request_id: Option<axum::Extension<crate::request_id::RequestId>>,
    lang: sui_id_i18n::Locale,
    jar: CookieJar,
    form: MfaChallengeForm,
    fed_pending_raw: String,
) -> Result<Response, HttpError> {
    let browser_binding_raw = jar
        .get(FED_MFA_BROWSER_BINDING_COOKIE)
        .map(|c| c.value().to_owned())
        .unwrap_or_default();

    let outcome = sui_id_core::federated_mfa::promote(
        &app.db,
        &app.clock,
        &fed_pending_raw,
        &browser_binding_raw,
        &form.fed_csrf,
        form.code.expose_secret(),
    )
    .await;

    match outcome {
        Ok(sui_id_store::commands::F03Outcome::Promoted { session, .. }) => {
            let next_target = find_fed_pending_row(&app, &fed_pending_raw)
                .await
                .and_then(|row| row.next_path)
                .filter(|s| s.starts_with('/'))
                .unwrap_or_else(|| "/admin".into());
            // RFC 096-B1 stage 12b fix round, item 1: the generic
            // `login_pending_mfa` row dual-issued alongside the federated
            // one (stage 9) is never reached by L02 on this path --
            // `verify_pending`'s own `consume_within_tx` only runs from
            // the local branch. Best-effort, same shape as the §3 lockout
            // hook below: a different crate, a different lifetime, no
            // business inside F03's own guarded transaction.
            if let Some(pending_id) = jar
                .get(PENDING_MFA_COOKIE)
                .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok())
                && let Err(e) =
                    sui_id_store::repos::login_pending_mfa::delete(&app.db, pending_id).await
            {
                let rid = request_id.as_ref().map(|e| e.0.0.as_str()).unwrap_or("-");
                tracing::error!(
                    request_id = rid,
                    error = %e,
                    "federated promotion: consuming the generic pending-MFA row failed"
                );
            }
            // `auth.mfa.success` was committed by F03 with the session
            // (RFC 096 `:812-818`). RFC 006: MFA verified -> full
            // sign-in success, the same metric the local path emits.
            if let Some(m) = app.metric() {
                m.signin(sui_id_store::metrics::signin_result::SUCCESS);
            }
            let cookie = session_cookie(session.id.to_string(), app.config.server.cookie_secure);
            // Dispatch item 3: clear all four federated cookies *and*
            // the generic pending-MFA pair -- the generic row's own job
            // ends here too, even though this path never consumed it.
            let jar = jar
                .add(cookie)
                .add(clear_fed_mfa_pending_cookie(
                    app.config.server.cookie_secure,
                ))
                .add(clear_fed_mfa_csrf_cookie(app.config.server.cookie_secure))
                .add(clear_fed_mfa_browser_binding_cookie(
                    app.config.server.cookie_secure,
                ))
                .add(clear_pending_mfa_cookie(app.config.server.cookie_secure))
                .add(clear_pending_mfa_next_cookie(
                    app.config.server.cookie_secure,
                ));
            Ok((jar, Redirect::to(&next_target)).into_response())
        }
        Ok(
            sui_id_store::commands::F03Outcome::RejectedStillPending { reason, .. }
            | sui_id_store::commands::F03Outcome::AttemptsExhausted { reason },
        ) => {
            // RFC 096-B1 stage 12b §3: the account lock. F03 counts the
            // *row's* own `failure_count` (the per-continuation cap of
            // five, unchanged); this feeds the *user's* own
            // `mfa_failure_count` (L07), the separate, escalating bound
            // the local path already enforces -- so both doors to an
            // account share one lock, which is the whole reason this
            // call exists rather than a federated-only backoff.
            match find_fed_pending_row(&app, &fed_pending_raw).await {
                Some(row) => {
                    let max_lockout = app.config.security.max_lockout.as_secs();
                    if let Err(e) = sui_id_core::mfa::record_second_factor_failure(
                        &app.db,
                        row.user_id,
                        max_lockout,
                    )
                    .await
                    {
                        let rid = request_id.as_ref().map(|e| e.0.0.as_str()).unwrap_or("-");
                        tracing::error!(
                            request_id = rid,
                            error = %e,
                            "federated second-factor lockout bookkeeping failed"
                        );
                    }
                }
                // `promote` only reaches this arm after its own
                // `bind_pending` found a real row, so this is not
                // expected -- logged as the anomaly it would be, not
                // silently skipped, rather than failing the response.
                None => {
                    tracing::error!(
                        "federated second-factor rejected but its pending row could not be \
                         re-read for the account-lock hook"
                    );
                }
            }
            mfa_challenge_federated_failure(&app, jar, lang, Some(reason)).await
        }
        Ok(sui_id_store::commands::F03Outcome::Invalidated { .. }) => {
            // Terminal; no retry against this row can succeed. Clear
            // only the federated cookies (dispatch item 3) -- the
            // generic pending-MFA pair is this stage's own declared
            // scope boundary ("leave the dual-row emission in place"),
            // so its own lifecycle is untouched here.
            let jar = jar
                .add(clear_fed_mfa_pending_cookie(
                    app.config.server.cookie_secure,
                ))
                .add(clear_fed_mfa_csrf_cookie(app.config.server.cookie_secure))
                .add(clear_fed_mfa_browser_binding_cookie(
                    app.config.server.cookie_secure,
                ));
            mfa_challenge_federated_failure(&app, jar, lang, None).await
        }
        Err(sui_id_core::federated_mfa::VerifyError::Binding(_)) => {
            mfa_challenge_federated_failure(&app, jar, lang, None).await
        }
        Err(sui_id_core::federated_mfa::VerifyError::Internal(e)) => {
            // Not a verdict on the presented proof (RFC 102 C1, R11 1b) --
            // the same shape the local path's own non-credential-error
            // branch already logs with.
            let rid = request_id.as_ref().map(|e| e.0.0.as_str()).unwrap_or("-");
            tracing::error!(
                request_id = rid,
                error = %e,
                detail = ?e,
                "federated second-factor sign-in failed for a reason other than a wrong proof; \
                 returning the uniform failure response"
            );
            mfa_challenge_federated_failure(&app, jar, lang, None).await
        }
    }
}

async fn find_fed_pending_row(
    app: &crate::handlers::AppState,
    fed_pending_raw: &str,
) -> Option<sui_id_store::models::FederationMfaPendingRow> {
    let id = fed_pending_raw
        .parse::<sui_id_shared::ids::FederationMfaPendingId>()
        .ok()?;
    sui_id_store::repos::federation_mfa_pending::find_by_id(&app.db, id)
        .await
        .ok()
        .flatten()
}

/// The federated path's own uniform failure response -- the local path's
/// (above) stays untouched rather than sharing this, since nothing about
/// it needed to change. `reason` selects the one copy RFC 096-B1 stage
/// 12b §5 allows to be specific (`MethodMismatch`, direction 2 only --
/// the only direction `promote`/`verify` can ever produce, since this
/// handler never calls `verify_webauthn`); every other reason, and every
/// `None` (a binding failure or a terminal row), gets the same generic
/// flash the local path's own wrong-code case does.
async fn mfa_challenge_federated_failure(
    app: &crate::handlers::AppState,
    jar: CookieJar,
    lang: sui_id_i18n::Locale,
    reason: Option<sui_id_store::repos::federation_mfa_pending::MfaRejectionReason>,
) -> Result<Response, HttpError> {
    if let Some(m) = app.metric() {
        m.signin(sui_id_store::metrics::signin_result::MFA_FAILED);
    }
    let t = lang.strings();
    let text = if reason
        == Some(sui_id_store::repos::federation_mfa_pending::MfaRejectionReason::MethodMismatch)
    {
        t.mfa_challenge_method_mismatch_flash
    } else {
        t.mfa_challenge_failed_flash
    };
    let flash = Flash {
        kind: FlashKind::Error,
        text: text.into(),
    };
    // has_passkey: the same `PENDING_MFA_COOKIE`-keyed check the local
    // path's own failure branch uses -- the generic row still exists
    // (dispatch's own "leave the dual-row emission in place"), so it
    // already answers correctly for a federated continuation too.
    let has_passkey = {
        let pid_opt = jar
            .get(PENDING_MFA_COOKIE)
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
    let fed_csrf = jar.get(FED_MFA_CSRF_COOKIE).map(|c| c.value().to_owned());
    let token = crate::csrf::ensure_token(&jar);
    let inner = (
        StatusCode::UNAUTHORIZED,
        Html(sui_id_web::render_mfa_challenge(
            Some(flash),
            token.clone(),
            has_passkey,
            lang,
            fed_csrf,
        )),
    )
        .into_response();
    let resp = (jar, inner).into_response();
    Ok(with_csrf_cookie(resp, app, &token))
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
