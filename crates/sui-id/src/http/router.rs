//! Axum router construction.

use crate::AppState;
use crate::handlers::{admin, index, oidc, setup};
use crate::security_headers::SecurityHeaderConfig;
use axum::Router;
use axum::routing::{get, post};
use tower_http::trace::TraceLayer;

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
        // RFC 122 D3: the returned `client_secret` is shown once; this
        // response is unstorable, the same pattern `/reset-password` uses.
        .route(
            "/oauth2/register",
            post(crate::handlers::dynamic_register::dynamic_register).layer(
                tower_http::set_header::SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                ),
            ),
        )
        .layer(token_cors);

    // RFC 120 D5: the setup steps that follow the creation of the first
    // administrator change server-wide settings, so they are one group whose
    // requirement is stated once: an authenticated administrator, checked by
    // the same extractor every admin route uses. A step added here inherits it.
    // (CSRF, the rate limit and the audit row are each handler's own.)
    let setup_steps = Router::new()
        .route("/setup/lang", get(setup::lang_get).post(setup::lang_post))
        .route("/setup/hibp", get(setup::hibp_get).post(setup::hibp_post))
        .layer(axum::middleware::from_extractor_with_state::<
            crate::handlers::CurrentAdmin,
            _,
        >(app.clone()));

    let router = Router::new()
        .route("/", get(index::root))
        .route("/healthz", get(index::healthz))
        .route("/setup", get(setup::welcome_get))
        .route(
            "/setup/admin",
            get(setup::admin_get).post(setup::admin_post),
        )
        .route("/setup/done", get(setup::done_get))
        .merge(setup_steps)
        .merge(public_routes)
        .merge(token_routes)
        .route("/oauth2/authorize", get(oidc::authorize))
        .route("/oauth2/consent", post(oidc::consent_post)) // RFC 038: consent gate
        .route("/oauth2/logout", get(oidc::logout))
        .route(
            "/oauth2/introspect",
            post(crate::handlers::oauth_token::introspect),
        )
        .route("/oauth2/revoke", post(crate::handlers::oauth_token::revoke))
        .route(
            "/admin/login",
            get(admin::login_get).post(admin::login_post),
        )
        .route(
            "/admin/login/mfa",
            get(admin::mfa_challenge_get).post(admin::mfa_challenge_post),
        )
        // RFC 004: federated sign-in routes
        .route(
            "/auth/federated/{slug}/start",
            get(crate::handlers::federation::federated_start),
        )
        .route(
            "/auth/federated/callback",
            get(crate::handlers::federation::federated_callback),
        )
        // RFC 096-B1 stage 5, RFC 096 `:700-701`: `/auth/federated/link`
        // and its unsigned pending-link cookie are removed, not reused.
        // ---------- forgot-password (since v0.22.0) ----------
        // Forgot-password lives under the public path, not /admin/*,
        // because the user is by definition not signed in. Both the
        // form and the post are publicly reachable; rate-limiting
        // and constant-time response handle abuse.
        .route(
            "/forgot-password",
            get(crate::handlers::forgot_password::forgot_password_get)
                .post(crate::handlers::forgot_password::forgot_password_post),
        )
        // RFC 103 stage 2: every response on this route is `no-store`. The
        // form re-shown after a password-policy refusal holds the reset
        // token in a field value, and no page here should be kept by a
        // browser or proxy cache.
        .route(
            "/reset-password",
            get(crate::handlers::forgot_password::reset_password_get)
                .post(crate::handlers::forgot_password::reset_password_post)
                .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                )),
        )
        .route("/admin/logout", post(admin::logout))
        // RFC 055 (v0.44.0): /admin/profile consolidated onto /me/security/*.
        // GET stays as a 301 redirect to honour bookmarks; the POST
        // routes are removed entirely since their only callers were
        // the legacy `render_profile` forms.
        .route(
            "/admin/profile",
            get(crate::handlers::me_security::admin_profile_redirect),
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
        .route(
            "/admin/users",
            get(admin::users_get).post(admin::users_create),
        )
        .route("/admin/users/new", get(admin::users_new_get))
        .route("/admin/users/{id}", get(admin::users_detail_get))
        .route(
            "/admin/users/{id}/disabled",
            post(admin::users_set_disabled),
        )
        .route(
            "/admin/users/{id}/disable-confirm",
            get(admin::users_disable_confirm_get),
        )
        .route("/admin/users/{id}/delete", post(admin::users_delete))
        .route(
            "/admin/users/{id}/delete-confirm",
            get(admin::users_delete_confirm_get),
        )
        .route("/admin/users/{id}/mfa-reset", post(admin::users_mfa_reset))
        .route(
            "/admin/users/{id}/mfa-reset-confirm",
            get(admin::users_mfa_reset_confirm_get),
        )
        .route(
            "/admin/users/{id}/recovery-link",
            post(admin::users_recovery_link),
        )
        .route(
            "/admin/users/{id}/recovery-link-confirm",
            get(admin::users_recovery_link_confirm_get),
        )
        .route("/admin/users/{id}/role", post(admin::users_set_role))
        // RFC 122 D3: the POST here shows a freshly generated client secret
        // once; this route's whole response is unstorable, the same pattern
        // `/reset-password` uses.
        .route(
            "/admin/clients",
            get(admin::clients_get).post(admin::clients_create).layer(
                tower_http::set_header::SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                ),
            ),
        )
        .route("/admin/clients/new", get(admin::clients_new_get))
        .route(
            "/admin/clients/{id}/disabled",
            post(admin::clients_set_disabled),
        )
        .route("/admin/clients/{id}/delete", post(admin::clients_delete))
        .route(
            "/admin/clients/{id}/delete-confirm",
            get(admin::clients_delete_confirm_get),
        )
        .route(
            "/admin/clients/{id}/edit",
            get(admin::clients_edit_get).post(admin::clients_edit_post),
        )
        .route("/admin/signing-keys", get(admin::signing_keys_get))
        .route(
            "/admin/signing-keys/rotate-confirm",
            get(admin::signing_keys_rotate_confirm_get),
        )
        .route(
            "/admin/signing-keys/rotate",
            post(admin::signing_keys_rotate),
        )
        // RFC 122 D1/D3: the rotated secret is rendered directly from this
        // response (never a URL), which is unstorable at the router layer.
        .route(
            "/admin/clients/{id}/rotate-secret",
            post(admin::clients_rotate_secret_post).layer(
                tower_http::set_header::SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                ),
            ),
        )
        .route(
            "/admin/signing-keys/{id}/delete",
            post(admin::signing_keys_delete),
        )
        .route(
            "/admin/signing-keys/{id}/delete-confirm",
            get(admin::signing_keys_delete_confirm_get),
        )
        .route("/admin/audit", get(admin::audit_get))
        .route("/admin/audit.csv", get(admin::audit_csv_get))
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
            "/admin/settings/basic/lang",
            post(crate::handlers::settings::basic_lang_post),
        )
        .route(
            "/admin/settings/security",
            get(crate::handlers::settings::security_get),
        )
        .route(
            "/admin/settings/security/idle-timeout",
            post(crate::handlers::settings::idle_timeout_post),
        )
        .route(
            "/admin/settings/security/max-sessions",
            post(crate::handlers::settings::max_sessions_post),
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
        .route(
            "/admin/settings/email",
            get(crate::handlers::settings::email_get).post(crate::handlers::settings::email_post),
        )
        .route(
            "/admin/settings/email/confirm",
            get(crate::handlers::settings::email_confirm_get)
                .post(crate::handlers::settings::email_confirm_post),
        )
        .route(
            "/admin/settings/email/test",
            post(crate::handlers::settings::email_test),
        )
        // ---------- self-service security (since v0.18.0) ----------
        // These routes require an authenticated session but do *not*
        // require admin privilege; they're for any signed-in user.
        // The handler enforces ownership: a user can only see and
        // revoke their own sessions.
        .route(
            "/me/security",
            get(crate::handlers::me_security::security_redirect),
        )
        .route(
            "/me/security/overview",
            get(crate::handlers::me_security::overview_get),
        )
        .route(
            "/me/security/mfa",
            get(crate::handlers::me_security::mfa_get),
        )
        .route(
            "/me/security/sessions",
            get(crate::handlers::me_security::sessions_tab_get),
        )
        .route(
            "/me/security/passkeys",
            get(crate::handlers::me_security::passkeys_get),
        )
        .route(
            "/me/security/passkeys/{id}/rename",
            post(crate::handlers::me_security::passkey_rename_post),
        )
        // MFA mutative routes (RFC 055, v0.44.0)
        // RFC 122 D3: each of the next three shows a TOTP secret/QR or
        // recovery codes once; unstorable at the router layer.
        .route(
            "/me/security/mfa/enroll/start",
            post(crate::handlers::me_security::mfa_enroll_start).layer(
                tower_http::set_header::SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                ),
            ),
        )
        .route(
            "/me/security/mfa/enroll/confirm",
            post(crate::handlers::me_security::mfa_enroll_confirm).layer(
                tower_http::set_header::SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                ),
            ),
        )
        .route(
            "/me/security/mfa/disable",
            post(crate::handlers::me_security::mfa_disable),
        )
        .route(
            "/me/security/mfa/recovery-codes/regenerate",
            post(crate::handlers::me_security::mfa_regenerate_recovery).layer(
                tower_http::set_header::SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                ),
            ),
        )
        // Passkey mutative routes (RFC 055, v0.44.0)
        .route(
            "/me/security/passkeys/register/start",
            post(crate::handlers::me_security::passkey_register_start),
        )
        .route(
            "/me/security/passkeys/register/complete",
            post(crate::handlers::me_security::passkey_register_complete),
        )
        .route(
            "/me/security/passkeys/{id}/delete",
            post(crate::handlers::me_security::passkey_delete),
        )
        .route(
            "/me/security/language",
            get(crate::handlers::me_security::language_get)
                .post(crate::handlers::me_security::language_post),
        )
        .route("/me/apps", get(crate::handlers::me_security::me_apps_get))
        .route(
            "/me/apps/{client_id}/revoke",
            post(crate::handlers::me_security::me_apps_revoke),
        )
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
        // ---------- step-up auth (since v0.21.0) ----------
        .route(
            "/me/security/step-up",
            get(crate::handlers::step_up::get).post(crate::handlers::step_up::post),
        )
        .route(
            "/me/security/step-up/webauthn/start",
            post(crate::handlers::step_up::webauthn_start),
        )
        .route(
            "/me/security/step-up/webauthn/finish",
            post(crate::handlers::step_up::webauthn_finish),
        )
        .route("/static/{*path}", get(crate::assets::serve));

    // RFC 006: mount /metrics only when metrics_enabled = true (P5).
    // When disabled, the route is not registered — returns 404, not an
    // empty endpoint, so the endpoint's existence is not leaked.
    let router = if app.config.server.metrics_enabled {
        router.route("/metrics", get(crate::handlers::metrics::metrics_get))
    } else {
        router
    };

    let router = router
        .with_state(app.clone())
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
        .layer(axum::middleware::from_fn(crate::request_id::middleware));

    // RFC 016: mount TraceLayer only when access_log is enabled.
    // In dev mode the binary sets access_log = true so every request
    // is visible in the terminal. In production it is off by default
    // and opt-in via `log.access_log = true` in the config file.
    //
    // Security invariant: TraceLayer operates at the span level; it
    // does not log request/response bodies, cookies, or Authorization
    // headers. No secret value reaches any log sink through this layer.
    if app.config.log.access_log {
        router.layer(TraceLayer::new_for_http())
    } else {
        router
    }
}
