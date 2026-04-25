//! Root path handler. Decides where to send a fresh visitor.

use crate::errors::HttpError;
use crate::handlers::AppStateExt;
use axum::response::{IntoResponse, Redirect};
use axum::Json;
use serde::Serialize;
use sui_id_core::errors::CoreError;
use sui_id_store::repos::state;

pub async fn root(state_ext: AppStateExt) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized = state::is_initialized(&app.db)
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let target = if !initialized {
        "/setup"
    } else {
        "/admin"
    };
    Ok(Redirect::to(target).into_response())
}

/// Health-check endpoint. Returns a small JSON document and 200 OK as long
/// as the database is reachable. Suitable for liveness/readiness probes.
///
/// Crucially, this endpoint does *not* leak whether the server is in the
/// uninitialized state, who is logged in, or how many users exist — only
/// that the process is alive and the database accepts a query.
pub async fn healthz(state_ext: AppStateExt) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    // Touch the database with a trivial query so we surface storage problems.
    app.db
        .with_conn(|conn| {
            let _: i64 = conn.query_row("SELECT 1", [], |r| r.get(0))?;
            Ok(())
        })
        .map_err(|e| HttpError::api(CoreError::from(e)))?;
    Ok(Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
    .into_response())
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}
