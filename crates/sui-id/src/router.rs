//! Axum router construction.

use crate::handlers::{admin, index, oidc, setup};
use crate::security_headers::SecurityHeaderConfig;
use crate::AppState;
use axum::routing::{get, post};
use axum::Router;

pub fn build_router(app: AppState) -> Router {
    let hsts_enabled = app.config.server.cookie_secure;

    // Routes that emit fully-public bodies and want browsers from any
    // origin to be able to fetch them. We attach `cors::public_read`
    // here so the OPTIONS preflight is answered with `*`.
    let public_cors = axum::middleware::from_fn(crate::cors::public_read);

    // Token endpoint: per-origin allowlist driven by registered
    // redirect_uris. State-aware because it has to consult the
    // database on each request.
    let token_cors = axum::middleware::from_fn_with_state(app.clone(), crate::cors::token_endpoint);

    let public_routes = Router::new()
        .route("/.well-known/openid-configuration", get(oidc::discovery))
        .route("/.well-known/jwks.json", get(oidc::jwks))
        .route("/oauth2/userinfo", get(oidc::userinfo).post(oidc::userinfo))
        .layer(public_cors);

    let token_routes = Router::new()
        .route("/oauth2/token", post(oidc::token))
        .layer(token_cors);

    Router::new()
        .route("/", get(index::root))
        .route("/healthz", get(index::healthz))
        .route("/setup", get(setup::welcome_get))
        .route(
            "/setup/admin",
            get(setup::admin_get).post(setup::admin_post),
        )
        .route("/setup/done", get(setup::done_get))
        .merge(public_routes)
        .merge(token_routes)
        .route("/oauth2/authorize", get(oidc::authorize))
        .route("/oauth2/logout", get(oidc::logout))
        .route(
            "/oauth2/introspect",
            post(crate::handlers::oauth_token::introspect),
        )
        .route(
            "/oauth2/revoke",
            post(crate::handlers::oauth_token::revoke),
        )
        .route("/admin/login", get(admin::login_get).post(admin::login_post))
        .route(
            "/admin/login/mfa",
            get(admin::mfa_challenge_get).post(admin::mfa_challenge_post),
        )
        .route("/admin/logout", post(admin::logout).get(admin::logout))
        .route("/admin/profile", get(admin::profile_get))
        .route(
            "/admin/profile/mfa/enroll/start",
            post(admin::profile_mfa_enroll_start),
        )
        .route(
            "/admin/profile/mfa/enroll/confirm",
            post(admin::profile_mfa_enroll_confirm),
        )
        .route("/admin/profile/mfa/disable", post(admin::profile_mfa_disable))
        .route(
            "/admin/profile/mfa/recovery-codes/regenerate",
            post(admin::profile_mfa_regenerate_recovery),
        )
        .route(
            "/admin/profile/webauthn/register/start",
            post(admin::webauthn_register_start),
        )
        .route(
            "/admin/profile/webauthn/register/complete",
            post(admin::webauthn_register_complete),
        )
        .route(
            "/admin/profile/webauthn/{id}/delete",
            post(admin::webauthn_delete),
        )
        .route(
            "/admin/login/webauthn/start",
            post(admin::webauthn_auth_start),
        )
        .route(
            "/admin/login/webauthn/complete",
            post(admin::webauthn_auth_complete),
        )
        .route("/admin", get(admin::dashboard))
        .route("/admin/users", get(admin::users_get).post(admin::users_create))
        .route("/admin/users/{id}/disabled", post(admin::users_set_disabled))
        .route("/admin/users/{id}/delete", post(admin::users_delete))
        .route("/admin/users/{id}/mfa-reset", post(admin::users_mfa_reset))
        .route(
            "/admin/clients",
            get(admin::clients_get).post(admin::clients_create),
        )
        .route(
            "/admin/clients/{id}/disabled",
            post(admin::clients_set_disabled),
        )
        .route("/admin/clients/{id}/delete", post(admin::clients_delete))
        .route(
            "/admin/clients/{id}/edit",
            get(admin::clients_edit_get).post(admin::clients_edit_post),
        )
        .route("/admin/signing-keys", get(admin::signing_keys_get))
        .route("/admin/signing-keys/rotate", post(admin::signing_keys_rotate))
        .route(
            "/admin/signing-keys/{id}/delete",
            post(admin::signing_keys_delete),
        )
        .route("/admin/audit", get(admin::audit_get))
        // ---------- settings (since v0.20.3) ----------
        .route(
            "/admin/settings",
            get(crate::handlers::settings::index_redirect),
        )
        .route(
            "/admin/settings/basic",
            get(crate::handlers::settings::basic_get),
        )
        .route(
            "/admin/settings/security",
            get(crate::handlers::settings::security_get),
        )
        .route(
            "/admin/settings/authentication",
            get(crate::handlers::settings::authentication_get),
        )
        .route(
            "/admin/settings/logs",
            get(crate::handlers::settings::logs_get),
        )
        .route(
            "/admin/settings/other",
            get(crate::handlers::settings::other_get),
        )
        // ---------- self-service security (since v0.18.0) ----------
        // These routes require an authenticated session but do *not*
        // require admin privilege; they're for any signed-in user.
        // The handler enforces ownership: a user can only see and
        // revoke their own sessions.
        .route("/me/security", get(crate::handlers::me_security::page_get))
        .route(
            "/me/security/sessions/{id}/revoke",
            post(crate::handlers::me_security::revoke_one),
        )
        .route(
            "/me/security/sessions/revoke-all-others",
            post(crate::handlers::me_security::revoke_all_others),
        )
        .route(
            "/me/security/password",
            get(crate::handlers::me_security::password_change_get)
                .post(crate::handlers::me_security::password_change_post),
        )
        .route("/static/{*path}", get(crate::assets::serve))
        .with_state(app)
        // Security-headers middleware applies to *every* response,
        // including the OIDC public ones merged above. State-aware so
        // it can decide whether to emit HSTS based on cookie_secure.
        .layer(axum::middleware::from_fn_with_state(
            SecurityHeaderConfig {
                enable_hsts: hsts_enabled,
            },
            crate::security_headers::middleware,
        ))
        // request-id middleware runs first (outermost) so the id is
        // attached before TraceLayer's span is opened.
        .layer(axum::middleware::from_fn(crate::request_id::middleware))
}
