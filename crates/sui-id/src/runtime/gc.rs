//! Background hygiene tasks.
//!
//! sui-id is correct without these — the read paths already filter out
//! expired entries. The GC keeps the on-disk representation tidy and
//! prevents unbounded growth of single-use tables (`auth_codes`, `sessions`,
//! `refresh_tokens`, `federation_login_attempt`, `federation_mfa_pending`).
//!
//! The interval is fixed at 15 minutes to avoid one more configuration knob;
//! the operator can shut down the binary if they need different behaviour.

use crate::AppState;
use std::time::Duration;
use sui_id_store::repos::{
    auth_codes, federation_login_attempt, federation_mfa_pending, login_pending_mfa,
    refresh_tokens, revoked_access_tokens, sessions, webauthn_pending,
};

const GC_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        // Initial wait so the first cycle isn't on the same hot path as
        // startup work.
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            run_once(&state).await;
            tokio::time::sleep(GC_INTERVAL).await;
        }
    });
}

/// Run one GC cycle inline. Public so integration tests can drive it
/// deterministically without waiting on the real interval.
pub async fn run_once(state: &AppState) {
    let db = &state.db;
    match auth_codes::purge_expired(db).await {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "gc: removed expired auth codes"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: auth_codes purge failed"),
    }
    match sessions::purge_expired(db).await {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "gc: removed expired sessions"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: sessions purge failed"),
    }
    match refresh_tokens::purge_expired(db).await {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "gc: removed expired refresh tokens"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: refresh_tokens purge failed"),
    }
    match login_pending_mfa::purge_expired(db).await {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "gc: removed expired pending-MFA rows"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: login_pending_mfa purge failed"),
    }
    match webauthn_pending::purge_expired(db).await {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "gc: removed expired WebAuthn ceremonies"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: webauthn_pending purge failed"),
    }
    match revoked_access_tokens::purge_expired(db).await {
        Ok(n) if n > 0 => {
            tracing::info!(deleted = n, "gc: removed expired access-token deny entries")
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: revoked_access_tokens purge failed"),
    }
    // RFC 096-B1 stage 6a, RFC 096 `:604-605`. `purge_expired` takes `now`
    // as a parameter (its own doc comment explains why, against a seventh
    // precedent the other six don't follow) -- this is the one call site
    // that samples the clock, same discipline as `insert`/`claim`'s own
    // callers.
    match federation_login_attempt::purge_expired(db, state.clock.now()).await {
        Ok(n) if n > 0 => {
            tracing::info!(deleted = n, "gc: removed expired federation login attempts")
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: federation_login_attempt purge failed"),
    }
    // RFC 096-B1 stage 9, RFC 096 `:734`: the row this stage's F02 first
    // writes. Not filtered on `status` (`federation_mfa_pending::
    // purge_expired`'s own doc explains why) and needs no separate
    // ceremony-table purge: `federation_webauthn_ceremony`'s composite
    // `FOREIGN KEY ... ON DELETE CASCADE` (migration 0047's fix round)
    // collects any ceremony this `DELETE` removes the parent of.
    match federation_mfa_pending::purge_expired(db, state.clock.now()).await {
        Ok(n) if n > 0 => {
            tracing::info!(
                deleted = n,
                "gc: removed expired federated MFA continuations"
            )
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "gc: federation_mfa_pending purge failed"),
    }
}
