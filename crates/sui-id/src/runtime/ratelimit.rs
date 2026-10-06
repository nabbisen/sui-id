//! Per-caller rate limiting.
//!
//! Implemented as a fixed-window counter map keyed on `(route_key, id)`,
//! where `id` is whatever the caller claims to be — an IP address for every
//! limiter this module originally had, and (RFC 123 D1) a claimed
//! `client_id` string for the two new ones `/oauth2/introspect` and
//! `/oauth2/revoke` add. Neither identity is authenticated before the check
//! runs: it is used as a rate-limiting key, not an identity assertion,
//! exactly as an IP address always has been here.
//!
//! A fixed window is slightly less accurate than a sliding window or token
//! bucket, but it is simple, allocation-light, and the failure mode (some
//! callers get a slightly more or slightly less generous quota near a
//! window boundary) is benign for the endpoints we apply it to.
//!
//! State lives in a `Mutex<HashMap<...>>`. For a single-process IDaaS this
//! is fine — the hot endpoints (`/admin/login`, `/oauth2/token`, `/setup`)
//! are not high-throughput.

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::hash::Hash;
use std::net::IpAddr;
use std::sync::Mutex;

/// One named limiter, e.g. "login" or "token". Each takes a separate
/// per-`Id` counter map. `Id` is `IpAddr` for every limiter that existed
/// before RFC 123, and `String` (a claimed `client_id`) for the two it adds.
pub struct Limiter<Id: Eq + Hash + Clone> {
    per_window: i64,
    window: Duration,
    state: Mutex<HashMap<(String, Id), Window>>,
}

#[derive(Debug, Clone, Copy)]
struct Window {
    started_at: DateTime<Utc>,
    count: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct Decision {
    pub allowed: bool,
    pub remaining: i64,
    pub retry_after_secs: i64,
}

impl<Id: Eq + Hash + Clone> Limiter<Id> {
    pub fn new(per_window: i64, window_secs: i64) -> Self {
        Self {
            per_window,
            window: Duration::seconds(window_secs),
            state: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, key: &str, id: Id, now: DateTime<Utc>) -> Decision {
        let mut guard = match self.state.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        // Periodically prune entries whose window has long since closed.
        if guard.len() > 1024 {
            let cutoff = now - self.window * 4;
            guard.retain(|_, w| w.started_at >= cutoff);
        }
        let entry = guard.entry((key.to_owned(), id)).or_insert(Window {
            started_at: now,
            count: 0,
        });
        if now - entry.started_at >= self.window {
            entry.started_at = now;
            entry.count = 0;
        }
        if entry.count >= self.per_window {
            let retry_after = (entry.started_at + self.window - now).num_seconds().max(1);
            return Decision {
                allowed: false,
                remaining: 0,
                retry_after_secs: retry_after,
            };
        }
        entry.count += 1;
        Decision {
            allowed: true,
            remaining: (self.per_window - entry.count).max(0),
            retry_after_secs: 0,
        }
    }
}

/// Bundle of named limiters used by the HTTP layer.
pub struct Limiters {
    pub login: Limiter<IpAddr>,
    pub token: Limiter<IpAddr>,
    pub setup: Limiter<IpAddr>,
    /// Per-IP throttle on `POST /forgot-password`. The flow is
    /// safe-by-design (constant-time response, audit log records
    /// real outcome, single-use 30-minute tokens, outstanding-token
    /// ceiling per user) but a per-IP limiter still blunts a
    /// would-be enumeration scanner before it generates audit-log
    /// noise.
    pub forgot_password: Limiter<IpAddr>,
    /// Per-IP throttle on step-up re-authentication (RFC 102 B2): the
    /// step-up TOTP form, both WebAuthn step-up endpoints, and every
    /// password re-entry for adding a first second factor (B7). Separate
    /// from `login` so a signed-in session's guesses do not spend, or
    /// borrow from, the sign-in budget.
    pub step_up: Limiter<IpAddr>,
    /// RFC 123 D1: `/oauth2/introspect` and `/oauth2/revoke`, per source IP.
    /// Sized more generously than `token` — a resource server calling
    /// introspection on every request it serves, possibly from behind one
    /// shared egress address, is a heavier traffic shape than a browser's
    /// token exchanges. The exact number is an operational starting point,
    /// not a measured ceiling; `@nabbisen` and the implementer can retune it
    /// once real integrations exist.
    pub introspect_revoke_ip: Limiter<IpAddr>,
    /// RFC 123 D1: `/oauth2/introspect` and `/oauth2/revoke`, per **claimed**
    /// `client_id` (not yet authenticated when this bucket is checked — see
    /// the module doc). Sized higher than the per-IP bucket above: one
    /// legitimate integration can be served by several resource-server
    /// instances behind different addresses, all authenticating as the same
    /// client. Defends the shape `introspect_revoke_ip` alone would miss —
    /// one attacker rotating source addresses against a single known id.
    pub introspect_revoke_client: Limiter<String>,
}

impl Default for Limiters {
    fn default() -> Self {
        Self {
            // Conservative defaults intended to discourage online password
            // guessing without breaking legitimate retry patterns.
            login: Limiter::new(10, 60),
            token: Limiter::new(60, 60),
            setup: Limiter::new(20, 60),
            // Forgot-password is a heavier operation (it sends an
            // email per request when matched). Half the login
            // budget is plenty for a real user mistyping their
            // address a few times.
            forgot_password: Limiter::new(5, 60),
            // The same budget as sign-in. L06 revokes the session after
            // five consecutive failures anyway; this bounds a thief who
            // holds several sessions.
            step_up: Limiter::new(10, 60),
            introspect_revoke_ip: Limiter::new(300, 60),
            introspect_revoke_client: Limiter::new(600, 60),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "ratelimit/tests.rs"]
mod tests;
