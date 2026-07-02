//! Per-IP rate limiting.
//!
//! Implemented as a fixed-window counter map keyed on `(route_key, client_ip)`.
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
use std::net::IpAddr;
use std::sync::Mutex;

/// One named limiter, e.g. "login" or "token". Each takes a separate
/// per-IP counter map.
pub struct Limiter {
    per_window: i64,
    window: Duration,
    state: Mutex<HashMap<(String, IpAddr), Window>>,
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

impl Limiter {
    pub fn new(per_window: i64, window_secs: i64) -> Self {
        Self {
            per_window,
            window: Duration::seconds(window_secs),
            state: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, key: &str, ip: IpAddr, now: DateTime<Utc>) -> Decision {
        let mut guard = match self.state.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        // Periodically prune entries whose window has long since closed.
        if guard.len() > 1024 {
            let cutoff = now - self.window * 4;
            guard.retain(|_, w| w.started_at >= cutoff);
        }
        let entry = guard.entry((key.to_owned(), ip)).or_insert(Window {
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
    pub login: Limiter,
    pub token: Limiter,
    pub setup: Limiter,
    /// Per-IP throttle on `POST /forgot-password`. The flow is
    /// safe-by-design (constant-time response, audit log records
    /// real outcome, single-use 30-minute tokens, outstanding-token
    /// ceiling per user) but a per-IP limiter still blunts a
    /// would-be enumeration scanner before it generates audit-log
    /// noise.
    pub forgot_password: Limiter,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).expect("valid epoch")
    }

    #[test]
    fn first_request_is_allowed() {
        let l = Limiter::new(3, 60);
        let d = l.check("k", "127.0.0.1".parse().unwrap(), t(0));
        assert!(d.allowed);
        assert_eq!(d.remaining, 2);
    }

    #[test]
    fn limit_blocks_within_window() {
        let l = Limiter::new(2, 60);
        let ip = "127.0.0.1".parse().unwrap();
        assert!(l.check("k", ip, t(0)).allowed);
        assert!(l.check("k", ip, t(1)).allowed);
        let d = l.check("k", ip, t(2));
        assert!(!d.allowed);
        assert!(d.retry_after_secs > 0);
    }

    #[test]
    fn limit_resets_after_window() {
        let l = Limiter::new(1, 60);
        let ip = "127.0.0.1".parse().unwrap();
        assert!(l.check("k", ip, t(0)).allowed);
        assert!(!l.check("k", ip, t(30)).allowed);
        // After the window, fresh count.
        assert!(l.check("k", ip, t(61)).allowed);
    }

    #[test]
    fn different_ips_are_independent() {
        let l = Limiter::new(1, 60);
        let a = "10.0.0.1".parse().unwrap();
        let b = "10.0.0.2".parse().unwrap();
        assert!(l.check("k", a, t(0)).allowed);
        assert!(l.check("k", b, t(0)).allowed);
        assert!(!l.check("k", a, t(1)).allowed);
    }

    #[test]
    fn different_keys_are_independent() {
        let l = Limiter::new(1, 60);
        let ip = "10.0.0.1".parse().unwrap();
        assert!(l.check("login", ip, t(0)).allowed);
        assert!(l.check("token", ip, t(0)).allowed);
        assert!(!l.check("login", ip, t(1)).allowed);
    }
}
