//! Background hygiene tasks.
//!
//! sui-id is correct without these — the read paths already filter out
//! expired entries. The GC keeps the on-disk representation tidy and
//! prevents unbounded growth of single-use tables (`auth_codes`, `sessions`,
//! `refresh_tokens`).
//!
//! The interval is fixed at 15 minutes to avoid one more configuration knob;
//! the operator can shut down the binary if they need different behaviour.

use crate::AppState;
use std::time::Duration;
use sui_id_store::repos::{auth_codes, refresh_tokens, sessions};

const GC_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        // Initial wait so the first cycle isn't on the same hot path as
        // startup work.
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            run_once(&state);
            tokio::time::sleep(GC_INTERVAL).await;
        }
    });
}

/// Run one GC cycle inline. Public so integration tests can drive it
/// deterministically without waiting on the real interval.
pub fn run_once(state: &AppState) {
    let db = &state.db;
    match auth_codes::purge_expired(db) {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "gc: removed expired auth codes"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: auth_codes purge failed"),
    }
    match sessions::purge_expired(db) {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "gc: removed expired sessions"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: sessions purge failed"),
    }
    match refresh_tokens::purge_expired(db) {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "gc: removed expired refresh tokens"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: refresh_tokens purge failed"),
    }
}
