//! Pwned Passwords (HIBP) breach check.
//!
//! Optional, opt-in pre-acceptance check that asks the public
//! Pwned Passwords API whether a candidate password has shown up
//! in known data breaches. Used at password-set time (currently
//! only at setup-wizard admin-creation; see ROADMAP for the
//! scope-expansion entry that adds the other entry points).
//!
//! ## Why k-anonymity matters
//!
//! The naive "send the password and ask" approach is obviously
//! wrong. Even sending the SHA-1 hash is dangerous — the hash
//! space is small enough that HIBP could (and would) be a
//! plaintext oracle for popular passwords.
//!
//! The Pwned Passwords API is built around the
//! [k-anonymity model][1]:
//!
//! 1. Client SHA-1s the candidate password.
//! 2. Client sends only the first 5 hex characters of the hash
//!    (the *prefix*) over HTTPS to
//!    `https://api.pwnedpasswords.com/range/{prefix}`.
//! 3. Server returns every hash in the database matching that
//!    prefix, with each hash's count appended:
//!    `<35-char-suffix>:<count>` per line, hundreds of rows per
//!    prefix.
//! 4. Client searches the response for its own suffix. If
//!    present, the count is the number of times the password
//!    has appeared across all known breaches; if absent, the
//!    password is not (yet) known to be breached.
//!
//! sui-id never sends the password, never sends the full hash,
//! and never logs the SHA-1 hash anywhere. The only thing on the
//! wire is the 5-character prefix, which (at SHA-1 width) is
//! shared by ~16,000-32,000 distinct passwords on average — far
//! too many for the prefix alone to identify the candidate.
//!
//! [1]: https://haveibeenpwned.com/API/v3#PwnedPasswords
//!
//! ## Why fail-open
//!
//! When this module's HTTP request fails (timeout, DNS, TLS,
//! 5xx response), the policy is to *let the password through*
//! regardless of mode (`warn` or `block`). The audit log
//! records the failure so an operator can investigate, but a
//! flaky external service must not be allowed to lock an admin
//! out of password operations. This explicit trade-off is
//! restated in migration 0017's comment.

use crate::errors::{CoreError, CoreResult};
use sha1::{Digest, Sha1};
use std::time::Duration;
use sui_id_store::models::HibpMode;
use zeroize::Zeroize;

/// Outcome of a single HIBP check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HibpCheckOutcome {
    /// HIBP responded and the password is not in any known breach.
    NotBreached,
    /// HIBP responded and the password has been seen in known
    /// breaches `count` times.
    Breached { count: u64 },
    /// The check could not be performed — network or service
    /// failure. Callers treat this as "no information available"
    /// and apply the fail-open policy.
    Unavailable,
}

/// Object-safe async trait so the production reqwest-backed
/// implementation can be swapped for a deterministic in-memory
/// mock in tests (RFC 070, v0.57.1).
#[async_trait::async_trait]
pub trait HibpClient: Send + Sync {
    /// Look up `password` in the Pwned Passwords range API.
    /// The implementation must handle its own timeouts and
    /// convert I/O / parse failures into `Unavailable` — the
    /// fail-open policy lives in the implementation, not the caller.
    async fn check(&self, password: &str) -> HibpCheckOutcome;
}

// ---------- Production: HttpHibpClient (reqwest) ----------

/// Async HTTP-backed `HibpClient` talking to the public Pwned
/// Passwords API via `reqwest`. The `reqwest::Client` is shared
/// and cloned from the one stored in `AppState`.
///
/// RFC 070 (v0.57.1): replaced ureq 2 (synchronous,
/// `spawn_blocking`-wrapped) with reqwest 0.12 (async, no wrapper
/// needed). The `HibpClient` trait is now `async fn check`.
pub struct HttpHibpClient {
    client: reqwest::Client,
    endpoint: String,
    user_agent: String,
    timeout: Duration,
}

impl HttpHibpClient {
    pub fn new() -> Self {
        let timeout = Duration::from_secs(5);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("failed to build reqwest client for HIBP");
        Self {
            client,
            endpoint: "https://api.pwnedpasswords.com/range".to_owned(),
            user_agent: format!("sui-id/{}", env!("CARGO_PKG_VERSION")),
            timeout,
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        // Rebuild client with new timeout
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("failed to build reqwest client for HIBP");
        self
    }
}

impl Default for HttpHibpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl HibpClient for HttpHibpClient {
    async fn check(&self, password: &str) -> HibpCheckOutcome {
        // 1. SHA-1 the candidate.
        let mut hasher = Sha1::new();
        hasher.update(password.as_bytes());
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(40);
        for b in digest.iter() {
            use std::fmt::Write;
            let _ = write!(&mut hex, "{:02X}", b);
        }
        // 2. Split into 5-char prefix + 35-char suffix.
        let (prefix, suffix) = hex.split_at(5);
        let url = format!("{}/{}", self.endpoint, prefix);

        let result = self
            .client
            .get(&url)
            .header("User-Agent", &self.user_agent)
            // Add-Padding defends against traffic-analysis attacks that
            // infer the queried prefix from response length.
            .header("Add-Padding", "true")
            .send()
            .await;

        let body = match result {
            Ok(resp) => match resp.text().await {
                Ok(s) => s,
                Err(_) => {
                    hex.zeroize();
                    return HibpCheckOutcome::Unavailable;
                }
            },
            Err(_) => {
                hex.zeroize();
                return HibpCheckOutcome::Unavailable;
            }
        };

        // 3. Parse `<suffix>:<count>` lines.
        let outcome = parse_response(&body, suffix);

        // 4. Zero the hash before return.
        hex.zeroize();
        outcome
    }
}

/// Pure parser, exposed for unit tests. Returns
/// `Breached { count }` for the first line whose suffix matches
/// (case-insensitively); `NotBreached` if no line matches.
pub fn parse_response(body: &str, suffix: &str) -> HibpCheckOutcome {
    let target = suffix.to_ascii_uppercase();
    for line in body.lines() {
        // Padding lines are documented as "lines that look like
        // hash:0". We skip count == 0 entries; a real breach hit
        // always has count >= 1.
        let (s, c) = match line.split_once(':') {
            Some(pair) => pair,
            None => continue,
        };
        if !s.eq_ignore_ascii_case(&target) {
            continue;
        }
        let count: u64 = c.trim().parse().unwrap_or(0);
        if count == 0 {
            // Conservative: a 0 count means "padding" and the
            // password is treated as not breached.
            return HibpCheckOutcome::NotBreached;
        }
        return HibpCheckOutcome::Breached { count };
    }
    HibpCheckOutcome::NotBreached
}

// ---------- Policy: enforce_hibp ----------

/// Outcome of [`enforce_hibp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HibpEnforcement {
    /// Password may proceed (modes Off, or Warn / Block when the
    /// password is not breached, or any mode when HIBP is
    /// unavailable — fail-open).
    Allowed,
    /// Password is breached and the configured mode warns but
    /// does not block. Caller should record an audit event;
    /// password is accepted.
    AllowedWithWarning { count: u64 },
    /// Password is breached and the configured mode blocks. The
    /// caller MUST refuse the password.
    Blocked { count: u64 },
}

/// Apply the configured HIBP policy to a candidate password.
///
/// `client` is `None` when the mode is `Off` (no client is
/// constructed in that case), or `Some(client)` for Warn/Block.
/// The synchronous client call should be wrapped in
/// `tokio::task::spawn_blocking` at the HTTP-handler call site
/// so the axum runtime is not blocked.
pub async fn enforce_hibp(
    mode: HibpMode,
    client: Option<&dyn HibpClient>,
    password: &str,
) -> HibpEnforcement {
    if matches!(mode, HibpMode::Off) {
        return HibpEnforcement::Allowed;
    }
    let Some(client) = client else {
        return HibpEnforcement::Allowed;
    };
    match client.check(password).await {
        HibpCheckOutcome::NotBreached | HibpCheckOutcome::Unavailable => HibpEnforcement::Allowed,
        HibpCheckOutcome::Breached { count } => match mode {
            HibpMode::Off => HibpEnforcement::Allowed, // unreachable
            HibpMode::Warn => HibpEnforcement::AllowedWithWarning { count },
            HibpMode::Block => HibpEnforcement::Blocked { count },
        },
    }
}

/// Convenience: convert [`HibpEnforcement::Blocked`] into a
/// `CoreError::BadRequest` with a friendly message; pass-through
/// otherwise. Caller picks the audit-event handling for `Allowed`
/// / `AllowedWithWarning`.
pub async fn enforce_hibp_or_reject(
    mode: HibpMode,
    client: Option<&dyn HibpClient>,
    password: &str,
) -> CoreResult<HibpEnforcement> {
    let result = enforce_hibp(mode, client, password).await;
    match result {
        HibpEnforcement::Blocked { .. } => Err(CoreError::BadRequest(
            "このパスワードは過去のデータ漏洩で確認されています。別のものを選んでください。"
                .to_owned(),
        )),
        other => Ok(other),
    }
}

// ---------- In-memory mock for tests ----------

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    //! In-memory `HibpClient` whose responses are pre-programmed.
    //! Test code constructs one of these, registers a few
    //! "breached" passwords, and injects it into the AppState in
    //! place of the production HTTP client.
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    pub struct InMemoryHibpClient {
        // Map password -> count (0 means "respond Unavailable",
        // not in map means "NotBreached").await.
        plan: Mutex<HashMap<String, BreachCount>>,
    }

    enum BreachCount {
        Count(u64),
        Unavailable,
    }

    impl Default for InMemoryHibpClient {
        fn default() -> Self {
            Self {
                plan: Mutex::new(HashMap::new()),
            }
        }
    }

    impl InMemoryHibpClient {
        pub fn new() -> Self {
            Self::default()
        }
        /// Register `password` as breached with the given count.
        pub fn set_breached(&self, password: impl Into<String>, count: u64) {
            self.plan
                .lock()
                .expect("hibp plan mutex")
                .insert(password.into(), BreachCount::Count(count));
        }
        /// Register `password` as triggering an Unavailable
        /// response. Useful for testing the fail-open path.
        pub fn set_unavailable(&self, password: impl Into<String>) {
            self.plan
                .lock()
                .expect("hibp plan mutex")
                .insert(password.into(), BreachCount::Unavailable);
        }
    }

    #[async_trait::async_trait]
    impl HibpClient for InMemoryHibpClient {
        async fn check(&self, password: &str) -> HibpCheckOutcome {
            match self.plan.lock().expect("hibp plan mutex").get(password) {
                Some(BreachCount::Count(c)) => HibpCheckOutcome::Breached { count: *c },
                Some(BreachCount::Unavailable) => HibpCheckOutcome::Unavailable,
                None => HibpCheckOutcome::NotBreached,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubBreached(u64);
    #[async_trait::async_trait]
    impl HibpClient for StubBreached {
        async fn check(&self, _password: &str) -> HibpCheckOutcome {
            HibpCheckOutcome::Breached { count: self.0 }
        }
    }
    struct StubClean;
    #[async_trait::async_trait]
    impl HibpClient for StubClean {
        async fn check(&self, _password: &str) -> HibpCheckOutcome {
            HibpCheckOutcome::NotBreached
        }
    }
    struct StubUnavailable;
    #[async_trait::async_trait]
    impl HibpClient for StubUnavailable {
        async fn check(&self, _password: &str) -> HibpCheckOutcome {
            HibpCheckOutcome::Unavailable
        }
    }

    #[tokio::test]
    async fn parse_response_finds_match() {
        // Real-shape line for "P@ssw0rd" — SHA-1 of "P@ssw0rd"
        // is "21BD12DC183F740EE76F27B78EB39C8AD972A757", so the
        // first 5 chars are "21BD1" and the suffix is the rest.
        let body = "0001E1559DBC1641BCFD3A30E18AAB52CDA:1\r\n\
                    2DC183F740EE76F27B78EB39C8AD972A757:42\r\n\
                    FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF:99\r\n";
        let result = parse_response(body, "2DC183F740EE76F27B78EB39C8AD972A757");
        assert_eq!(result, HibpCheckOutcome::Breached { count: 42 });
    }

    #[tokio::test]
    async fn parse_response_returns_not_breached_on_no_match() {
        let body = "0001E1559DBC1641BCFD3A30E18AAB52CDA:1\r\n\
                    FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF:99\r\n";
        let result = parse_response(body, "2DC183F740EE76F27B78EB39C8AD972A757");
        assert_eq!(result, HibpCheckOutcome::NotBreached);
    }

    #[tokio::test]
    async fn parse_response_treats_zero_count_as_padding() {
        // Add-Padding lines have count 0; we treat a match with
        // count 0 as not-breached out of caution.
        let body = "2DC183F740EE76F27B78EB39C8AD972A757:0\r\n";
        let result = parse_response(body, "2DC183F740EE76F27B78EB39C8AD972A757");
        assert_eq!(result, HibpCheckOutcome::NotBreached);
    }

    #[tokio::test]
    async fn parse_response_is_case_insensitive_on_suffix() {
        let body = "2dc183f740ee76f27b78eb39c8ad972a757:5\r\n";
        let result = parse_response(body, "2DC183F740EE76F27B78EB39C8AD972A757");
        assert_eq!(result, HibpCheckOutcome::Breached { count: 5 });
    }

    #[tokio::test]
    async fn enforce_off_skips_check_entirely() {
        // Passing a "would-be-breached" stub but mode=Off must
        // not even consult the client.
        let stub = StubBreached(99);
        let result = enforce_hibp(HibpMode::Off, Some(&stub), "anything").await;
        assert_eq!(result, HibpEnforcement::Allowed);
    }

    #[tokio::test]
    async fn enforce_warn_lets_breached_through_with_count() {
        let stub = StubBreached(42);
        let result = enforce_hibp(HibpMode::Warn, Some(&stub), "p").await;
        assert_eq!(result, HibpEnforcement::AllowedWithWarning { count: 42 });
    }

    #[tokio::test]
    async fn enforce_block_refuses_breached() {
        let stub = StubBreached(42);
        let result = enforce_hibp(HibpMode::Block, Some(&stub), "p").await;
        assert_eq!(result, HibpEnforcement::Blocked { count: 42 });
    }

    #[tokio::test]
    async fn enforce_warn_lets_clean_through() {
        let stub = StubClean;
        let result = enforce_hibp(HibpMode::Warn, Some(&stub), "p").await;
        assert_eq!(result, HibpEnforcement::Allowed);
    }

    #[tokio::test]
    async fn enforce_block_lets_clean_through() {
        let stub = StubClean;
        let result = enforce_hibp(HibpMode::Block, Some(&stub), "p").await;
        assert_eq!(result, HibpEnforcement::Allowed);
    }

    #[tokio::test]
    async fn enforce_warn_fail_open_when_unavailable() {
        let stub = StubUnavailable;
        let result = enforce_hibp(HibpMode::Warn, Some(&stub), "p").await;
        assert_eq!(result, HibpEnforcement::Allowed);
    }

    #[tokio::test]
    async fn enforce_block_fail_open_when_unavailable() {
        // Crucial: block mode must NOT block when the API is
        // unreachable. This is the documented fail-open policy.
        let stub = StubUnavailable;
        let result = enforce_hibp(HibpMode::Block, Some(&stub), "p").await;
        assert_eq!(result, HibpEnforcement::Allowed);
    }

    #[tokio::test]
    async fn enforce_hibp_or_reject_returns_bad_request_on_block() {
        let stub = StubBreached(7);
        let err = enforce_hibp_or_reject(HibpMode::Block, Some(&stub), "p")
            .await
            .expect_err("should reject");
        match err {
            CoreError::BadRequest(_) => {}
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }
}
