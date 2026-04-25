//! Axum router construction.

use crate::handlers::{admin, index, oidc, setup};
use crate::AppState;
use axum::routing::{get, post};
use axum::Router;

pub fn build_router(app: AppState) -> Router {
    Router::new()
        .route("/", get(index::root))
        .route("/healthz", get(index::healthz))
        .route("/setup", get(setup::get).post(setup::post))
        .route("/.well-known/openid-configuration", get(oidc::discovery))
        .route("/.well-known/jwks.json", get(oidc::jwks))
        .route("/oauth2/authorize", get(oidc::authorize))
        .route("/oauth2/token", post(oidc::token))
        .route("/oauth2/userinfo", get(oidc::userinfo).post(oidc::userinfo))
        .route("/oauth2/logout", get(oidc::logout))
        .route("/admin/login", get(admin::login_get).post(admin::login_post))
        .route("/admin/logout", post(admin::logout).get(admin::logout))
        .route("/admin", get(admin::dashboard))
        .route("/admin/users", get(admin::users_get).post(admin::users_create))
        .route("/admin/users/{id}/disabled", post(admin::users_set_disabled))
        .route("/admin/users/{id}/delete", post(admin::users_delete))
        .route(
            "/admin/clients",
            get(admin::clients_get).post(admin::clients_create),
        )
        .route(
            "/admin/clients/{id}/disabled",
            post(admin::clients_set_disabled),
        )
        .route("/admin/clients/{id}/delete", post(admin::clients_delete))
        .route("/admin/signing-keys", get(admin::signing_keys_get))
        .route("/admin/signing-keys/rotate", post(admin::signing_keys_rotate))
        .route(
            "/admin/signing-keys/{id}/delete",
            post(admin::signing_keys_delete),
        )
        .route("/admin/audit", get(admin::audit_get))
        .route("/static/{*path}", get(crate::assets::serve))
        .with_state(app)
}
