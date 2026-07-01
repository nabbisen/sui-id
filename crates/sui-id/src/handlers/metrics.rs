//! `GET /metrics` — Prometheus metrics endpoint (RFC 006).
//!
//! # Auth
//!
//! The endpoint is authenticated via either:
//!
//! - An active admin session cookie (same as the admin panel), or
//! - `Authorization: Bearer <token>` where the token matches
//!   `server_settings.metrics_token_hash` (checked in constant time,
//!   P2).  This is the path Prometheus scrape configs use.
//!
//! No credential → 401.  The route is **not registered** when
//! `metrics_enabled = false` (returns 404, P5).
//!
//! # Security properties
//!
//! All properties from RFC 006 §"Security properties / invariants" apply.
//! In particular: no PII in labels, no per-user/per-client cardinality,
//! constant-time bearer-token comparison, disabled-by-default.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use prometheus::Encoder;
use prometheus::TextEncoder;

use crate::handlers::AppStateExt;

/// Serve the Prometheus text-format metrics page.
///
/// Authentication: admin session cookie OR `Authorization: Bearer <token>`.
/// Returns 401 if neither is present or valid.
/// Returns 503 if the metrics registry is not initialised (should not happen
/// when the route is registered, but handled defensively).
pub async fn metrics_get(
    state_ext: AppStateExt,
    headers: HeaderMap,
    jar: axum_extra::extract::cookie::CookieJar,
) -> Result<Response, StatusCode> {
    let State(app) = state_ext;

    // Retrieve the metrics registry (registered only when metrics_enabled).
    let metrics = app.metrics.as_deref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // ── P1: authentication ────────────────────────────────────────────────────

    let authed = is_session_authed(&app, &jar).await
        || is_bearer_authed(&app, &headers).await;

    if !authed {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // ── Gather and encode ─────────────────────────────────────────────────────

    let encoder = TextEncoder::new();
    let families = metrics.registry.gather();
    let mut buf = Vec::with_capacity(4096);
    encoder
        .encode(&families, &mut buf)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::OK,
        [("Content-Type", encoder.format_type())],
        buf,
    )
        .into_response())
}

// ── Auth helpers ──────────────────────────────────────────────────────────────

/// Check whether the request carries a valid admin (or auditor) session cookie.
/// Auditors are allowed to read metrics (read-only access).
async fn is_session_authed(
    app: &crate::handlers::AppState,
    jar: &axum_extra::extract::cookie::CookieJar,
) -> bool {
    use crate::handlers::SESSION_COOKIE;
    use sui_id_shared::ids::SessionId;
    use std::str::FromStr;

    let Some(cookie) = jar.get(SESSION_COOKIE) else {
        return false;
    };
    let Ok(sid) = SessionId::from_str(cookie.value()) else {
        return false;
    };
    // Verify the session exists and resolves to an admin or auditor account.
    match sui_id_store::repos::sessions::get(&app.db, sid).await {
        Ok(session) => {
            // Session must not be expired; also check the user has admin/auditor role.
            match sui_id_store::repos::users::get(&app.db, session.user_id).await {
                Ok(user) => user.role.can_read_admin(),
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

/// Check `Authorization: Bearer <token>` against the stored hash.
/// Comparison is done in constant time (P2).
async fn is_bearer_authed(
    app: &crate::handlers::AppState,
    headers: &HeaderMap,
) -> bool {
    use subtle::ConstantTimeEq;

    let Some(auth_value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(auth_str) = auth_value.to_str() else {
        return false;
    };
    let Some(token) = auth_str.strip_prefix("Bearer ") else {
        return false;
    };
    let token = token.trim();
    if token.is_empty() {
        return false;
    }

    // Load the stored hash from server_settings.
    let Ok(settings) = sui_id_store::repos::server_settings::get(&app.db).await else {
        return false;
    };
    let Some(stored_hash) = settings.metrics_token_hash else {
        // No token configured — bearer auth is unavailable.
        return false;
    };

    // Constant-time byte comparison (P2).
    // Both sides must be the same length for the comparison to be timing-safe.
    // We hash the supplied token with SHA-256 before comparing so that a
    // timing side channel on string length is eliminated (stored_hash is also
    // SHA-256 of the raw token, stored as hex).
    let supplied_hash = sha2_hex(token);
    let stored_bytes = stored_hash.as_bytes();
    let supplied_bytes = supplied_hash.as_bytes();

    if stored_bytes.len() != supplied_bytes.len() {
        return false;
    }
    stored_bytes.ct_eq(supplied_bytes).into()
}

/// Compute the hex-encoded SHA-256 of `input`.
fn sha2_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(input.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}
