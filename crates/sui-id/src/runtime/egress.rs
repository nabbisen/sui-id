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
//! This module owns the *construction* of the client, including the
//! validating DNS resolver (RFC 134 D2, `crate::resolver::ValidatingResolver`)
//! wired in below. Per-connection header/chain-size bounds (the Tier-3
//! residual RFC 134 D5 records separately) are a later step of the same
//! RFC, dispatched separately.

use std::sync::Arc;
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
    federation_client_builder(Arc::new(crate::resolver::ValidatingResolver), None)
        .build()
        .expect("failed to build federation HTTP client")
}

/// The one list of federation client settings. Production and the test-support
/// constructor below both call it, so they cannot drift apart: the only inputs
/// that differ between them are the two parameters here.
///
/// The `resolver` parameter is the DNS policy (production: the RFC 134 D2
/// validating resolver). `extra_root` adds one trusted certificate on top of
/// the platform roots; production passes `None`, so the platform verifier is
/// selected exactly as before, and verification is never switched off.
fn federation_client_builder(
    resolver: Arc<dyn reqwest::dns::Resolve>,
    extra_root: Option<reqwest::Certificate>,
) -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder()
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
        // RFC 134 D2: the only resolver allowed to answer for federation
        // traffic. It validates every address against the vendored
        // IANA/explicit prefix table and returns exactly one surviving
        // address — the connector dials what it returns, so there is no
        // window for a second, unvalidated lookup to slip in.
        .dns_resolver(resolver);
    match extra_root {
        Some(root) => builder.tls_certs_merge([root]),
        None => builder,
    }
}

/// Test-support twin of [`build_federation_client`]: the same settings, built by
/// the same [`federation_client_builder`], differing only in its two named
/// parameters. Integration tests reach it through the `test-support` feature;
/// a `#[cfg(test)]` item would not be visible to them, since the library is
/// compiled without `cfg(test)` for the integration-test binary. The feature
/// is off by default and is enabled only as a dev-dependency, so the release
/// build contains no seam.
#[cfg(feature = "test-support")]
#[allow(clippy::expect_used)]
pub fn build_federation_client_for_tests(
    resolver: Arc<dyn reqwest::dns::Resolve>,
    extra_root: reqwest::Certificate,
) -> reqwest::Client {
    federation_client_builder(resolver, Some(extra_root))
        .build()
        .expect("failed to build test-support federation HTTP client")
}
