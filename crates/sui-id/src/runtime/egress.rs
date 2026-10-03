//! RFC 134 D1/D4 — the one federation egress client, and the one place its
//! policy is set.
//!
//! Federation talks to an administrator-configured upstream provider, which
//! RFC 096's threat model already treats as potentially hostile or
//! compromised (the user-facing federation routes take no caller-supplied
//! URL — see `http/router.rs`'s `federated_start`/`federated_callback`/
//! `federated_link_get`). This client is the one boundary that traffic
//! crosses, so its policy is set here, once, rather than left to whatever
//! `reqwest`'s defaults happen to be on a given release.
//!
//! This module owns the *construction* of the client. It does not touch DNS
//! resolution (step 3) or per-connection header/chain-size bounds (steps 2
//! and the Tier-3 residual RFC 134 D5 records separately) — those are later
//! steps of the same RFC, dispatched separately.

use std::time::Duration;

/// Build the federation-only outbound HTTP client. The only caller is
/// [`crate::state::AppState::new`]; nothing else may construct a `reqwest`
/// client for federation traffic — G19 (RFC 134 step 1c) asserts that
/// `reqwest::Client::builder()` appears nowhere else under the federation
/// path.
// A startup-time constructor with no `Result` return; a TLS-backend init
// failure here means the process cannot serve federation at all, so
// failing fast is the intended behavior, not a routine failure mode
// (moved from `AppState::new`, which carried the same allow for the same
// reason).
#[allow(clippy::expect_used)]
pub fn build_federation_client() -> reqwest::Client {
    reqwest::Client::builder()
        // D1's three bounds. `timeout` is the total deadline; a per-request
        // override on top of it would make this dead code on that request
        // (see `http/handlers/federation.rs`'s fetch functions, which no
        // longer set one of their own).
        .connect_timeout(Duration::from_secs(3))
        .read_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        // A hostile or compromised upstream redirecting us is exactly the
        // threat RFC 096 names. `reqwest` follows up to 10 redirects by
        // default; refuse all of them, same origin or not.
        .redirect(reqwest::redirect::Policy::none())
        // No ambient proxy, no ambient Referer header on outbound requests.
        .no_proxy()
        .referer(false)
        // HTTP/1.1 ALPN only, per RFC 134's validation matrix.
        .http1_only()
        .tls_version_min(reqwest::tls::Version::TLS_1_2)
        .tls_version_max(reqwest::tls::Version::TLS_1_3)
        // D5 Tier 2. All three are already `reqwest`'s defaults today —
        // that is exactly why they are set explicitly here rather than
        // left unstated: a future `reqwest` release that relaxes one of
        // these defaults changes this client's behaviour the moment it is
        // upgraded to, with nothing in this file to notice. Stating the
        // requirement in the source means a relaxed default cannot reach
        // federation silently, and nobody auditing this client later has
        // to go verify what today's default actually was. Do not delete
        // these calls because they look like they assert nothing; that is
        // the point of writing them.
        .http1_allow_obsolete_multiline_headers_in_responses(false)
        .http1_ignore_invalid_headers_in_responses(false)
        .http1_allow_spaces_after_header_name_in_responses(false)
        .build()
        .expect("failed to build federation HTTP client")
}
