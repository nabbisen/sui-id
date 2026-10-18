//! `GET /auth/federated/{slug}/start`  — redirect to upstream IdP
//! `GET /auth/federated/callback`       — exchange code, resolve link
//!
//! RFC 096-B1 stage 5, RFC 096 `:700-701`: `GET /auth/federated/link` and
//! its unsigned pending-link cookie are removed, not reused -- "M4 does
//! not create a link."
//!
//! RFC 004: upstream OIDC relying-party federation.
//!
//! # Security invariants enforced here
//!
//! - **P1**: federation_link key is `(provider_id, upstream_sub)`, never email.
//! - **P2**: email collision with an unlinked local user → denied + audited.
//! - **P3**: provision on first login only when `email_verified = true`.
//! - **P4**: local MFA is always enforced after federated sign-in.
//! - **P5**: state cookie is HMAC'd with the master key, single-use, 10-min TTL.
//! - **P6**: upstream access token is never persisted.
//! - **P7**: username is derived from upstream claims, conflict-resolved.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::errors::HttpError;
use crate::federation_identity::{derive_username, resolve_shadow_username};
use crate::handlers::{AppState, AppStateExt, session_cookie};
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::{FederationProviderId, SessionId, UserId};
use sui_id_store::models::{
    AuditLogRow, FederationLinkRow, FederationProviderRow, Role, SessionRow, UserRow, UserSource,
};

// ── RFC 096-B1 stage 4: 096-A's validators, their first production callers ──

use crate::cache_freshness::{ActivationGeneration, CacheKey, ProviderVersion};
use crate::federation_attempt_terminalize::{
    DeniedDetail, TerminalFailureReason, UpstreamFailureDetail, terminalize_and_audit,
};
use crate::id_token::verify_id_token;
use crate::identity_capability::construct_identity_capability;
use crate::identity_claims::validate_identity_claims;
use crate::identity_resolution::{IdentityResolution, resolve_verified_identity};
use crate::nonce_claim::validate_nonce;
use crate::optional_claims::validate_optional_claims;
use crate::time_claims::validate_iat;
use sui_id_store::StoreError;
use sui_id_store::repos::federation_login_attempt;

// ── Upstream discovery ────────────────────────────────────────────────────────

use crate::discovery::{DiscoveryError, ValidatedDiscovery};

/// Everything that can go wrong fetching and validating discovery. Kept
/// distinct from [`DiscoveryError`] because callers must react
/// differently: a network/parse failure is "the upstream is unreachable",
/// unchanged from before RFC 134 D3; [`FetchDiscoveryError::Invalid`] is
/// "the upstream named an endpoint we must not use" and gets its own
/// handling (RFC 134 D3 §2c) — logged with the provider slug and the
/// offending origin, audited, and surfaced to the browser as a generic
/// `fed_error`, never the offending URL.
enum FetchDiscoveryError {
    Network(String),
    Invalid(DiscoveryError),
}

impl std::fmt::Display for FetchDiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => write!(f, "{e}"),
            Self::Invalid(e) => write!(f, "{e}"),
        }
    }
}

impl FetchDiscoveryError {
    fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid(_))
    }
}

/// Fetch and validate the provider's discovery document. The issuer is
/// checked as canonical `https` *before* any request is made (RFC 134 D3);
/// every endpoint the document names is checked against `allowed_origins`
/// (space-separated, empty meaning "the issuer's origin alone") before
/// this can return `Ok` — see [`ValidatedDiscovery`].
async fn fetch_discovery(
    client: &reqwest::Client,
    issuer: &str,
    allowed_origins: &str,
) -> Result<ValidatedDiscovery, FetchDiscoveryError> {
    let issuer_url =
        crate::discovery::validate_issuer(issuer).map_err(FetchDiscoveryError::Invalid)?;
    let url = format!(
        "{}/.well-known/openid-configuration",
        issuer_url.as_str().trim_end_matches('/')
    );
    // RFC 134 D1: no per-request timeout override. `RequestBuilder::timeout`
    // overrides the client's, which would make the egress client's bound
    // dead code on this path; the client policy is the only source of
    // timeouts for federation traffic.
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| FetchDiscoveryError::Network(format!("discovery fetch failed: {e}")))?;
    // RFC 134 D5 Tier 1: status (200 only -- see response_bounds's own
    // doc comment for why "304 under the cache rules" reduces to that
    // here), media type, and a byte cap honored on bytes actually read --
    // the transport half, unchanged. Supersedes the old bare `is_success()`
    // check (2xx was looser than intended; a discovery document on 201/204
    // was never a real case).
    //
    // RFC 096-B1 stage 12a, §3: the structural/semantic half used to be
    // `read_bounded_json::<RawDiscovery>` (the generic `JWKS_AND_TRANSPORT`
    // member/array/string caps, no depth or duplicate-member check) plus a
    // bare `ValidatedDiscovery::validate`. `discovery::validate_discovery_
    // bytes` is RFC 096 `:531-532`'s own tighter, discovery-specific bounds
    // (depth 16, 32 array members, 2,048 bytes per string,
    // `ResponseCaps::DISCOVERY`) plus the duplicate-key-at-any-depth rule
    // `read_bounded_json` cannot apply once the body is a `Value` (096-A
    // stage 8 built it complete and correct for exactly this call site,
    // forbidden from reaching it at the time). A body that trips one of
    // these is now `FetchDiscoveryError::Invalid` -- "the upstream sent a
    // document we must not trust" -- rather than the generic `Network`
    // variant every malformed body collapsed into before; a genuine
    // transport failure (wrong status, wrong media type, over the byte cap)
    // is unaffected and still `Network`, from `read_bounded_body` itself.
    let bytes = crate::response_bounds::read_bounded_body(resp)
        .await
        .map_err(|e| FetchDiscoveryError::Network(format!("discovery fetch failed: {e}")))?;
    crate::discovery::validate_discovery_bytes(&bytes, issuer, allowed_origins)
        .map_err(FetchDiscoveryError::Invalid)
}

// ── RFC 096-B1 stage 4: shared helpers for the attempt row ──────────────────

/// RFC 096 `:564-566`: the browser-held opaque binding cookie, scoped the
/// same way `STATE_COOKIE` is.
const BROWSER_BINDING_COOKIE: &str = "sui_id_fed_browser_binding";

/// RFC 096-B1 stage 12a: the sealed-state cookie this stage retires is gone
/// (the module that carried it is deleted; the durable attempt row is the
/// sole source of everything it carried), but the name stays as a bare
/// constant so [`terminal`] keeps actively clearing it on every terminal
/// response -- a browser that still holds one from before this stage
/// deployed gets it cleared on its next completed attempt, rather than
/// carrying a cookie no code will ever read again until its own 10-minute
/// `max_age` lapses.
const STATE_COOKIE: &str = "sui_id_fed_state";

/// RFC 096-B1 stage 9, F02: the `federation_mfa_pending` row's own id,
/// so a future consumer (stage 10/11) can find which row belongs to this
/// browser without a session already established. Scoped site-wide
/// (`/`), not to `/auth/federated`: unlike `STATE_COOKIE`/
/// `BROWSER_BINDING_COOKIE`, which only the callback itself ever reads,
/// this cookie's eventual reader is `/admin/login/mfa` (stage 12).
///
/// `pub(crate)` since RFC 096-B1 stage 12b: `/admin/login/mfa`'s own
/// handler (`handlers::admin::auth`) is the reader this doc comment
/// already named, and it lives in a different module.
pub(crate) const FED_MFA_PENDING_COOKIE: &str = "sui_id_fed_mfa_pending";

/// RFC 096-B1 stage 9, F02: the raw CSRF-binding value, CSPRNG-minted the
/// same way `browser_binding`/`nonce`/`state_param` already are (`:564`'s
/// own precedent) — never the browser-binding value itself, a *second*,
/// independent opaque value, so a party holding one binding cannot drive
/// the other. Only `csrf_binding_sha256` (the digest) is stored in the
/// row; this is what the digest is a hash of.
///
/// `pub(crate)`: `/admin/login/mfa`'s POST handler reads this cookie's raw
/// value as `presented_csrf_body_value` only via the hidden form field the
/// GET handler renders it into (RFC 096-B1 stage 12b) -- `repos::
/// federation_mfa_pending`'s own module doc states why the cookie itself,
/// `HttpOnly`, can never be the presented value directly.
pub(crate) const FED_MFA_CSRF_COOKIE: &str = "sui_id_fed_mfa_csrf";

/// RFC 096-B1 stage 9 fix round, R1: the continuation's **own**
/// browser-binding value, CSPRNG-minted the same way as
/// [`FED_MFA_CSRF_COOKIE`]'s and never the attempt's own
/// [`BROWSER_BINDING_COOKIE`] value. It cannot be: `terminal()` (above)
/// expires `BROWSER_BINDING_COOKIE` on every `Ok` path, this one
/// included (RFC 096-B1 stage 6a, `:600-601`'s own "expired on every
/// terminal path"), and it is scoped `Path=/auth/federated`, so
/// `/admin/login/mfa` — this binding's eventual reader — would never be
/// sent it even if it survived. A row whose `browser_binding_sha256`
/// committed to that cookie's value would be a `NOT NULL` column
/// committing to a secret already revoked and out of the reader's path —
/// measured directly (a `Set-Cookie` probe on this exact response caught
/// it, see `federation_fail_closed.rs`'s own test), not assumed safe
/// because the attempt's own binding exists. Minting a second value
/// rather than widening `BROWSER_BINDING_COOKIE` to `Path=/` keeps the
/// OAuth-callback secret's lifetime and scope exactly as narrow as
/// `:600-601` already requires.
pub(crate) const FED_MFA_BROWSER_BINDING_COOKIE: &str = "sui_id_fed_mfa_browser_binding";

/// RFC 096-B1 stage 12b: the three federated-MFA cookies' own builders,
/// shared with `handlers::admin::auth` rather than left as inline
/// `Cookie` literals only this file could reconstruct. `max_age` is a
/// parameter, not `PENDING_LIFETIME_SECS` baked in, because the row's own
/// remaining lifetime -- not a flat constant -- is what the caller must
/// derive it from (`:4` of this stage's own dispatch): a restart reuses
/// an existing row without refreshing its `expires_at`, so a cookie that
/// always claims a fresh 300 seconds can outlive the row it names.
fn fed_mfa_pending_cookie<'a>(value: String, max_age: time::Duration, secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(FED_MFA_PENDING_COOKIE, value);
    c.set_http_only(true);
    c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    c.set_max_age(max_age);
    c.set_path("/");
    if secure {
        c.set_secure(true);
    }
    c
}

fn fed_mfa_csrf_cookie<'a>(value: String, max_age: time::Duration, secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(FED_MFA_CSRF_COOKIE, value);
    c.set_http_only(true);
    c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    c.set_max_age(max_age);
    c.set_path("/");
    if secure {
        c.set_secure(true);
    }
    c
}

fn fed_mfa_browser_binding_cookie<'a>(
    value: String,
    max_age: time::Duration,
    secure: bool,
) -> Cookie<'a> {
    let mut c = Cookie::new(FED_MFA_BROWSER_BINDING_COOKIE, value);
    c.set_http_only(true);
    c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    c.set_max_age(max_age);
    c.set_path("/");
    if secure {
        c.set_secure(true);
    }
    c
}

/// The three federated-MFA cookies' own clearers -- `/admin/login/mfa`'s
/// own handler uses these on `F03Outcome::Promoted` (all four federated
/// cookies, this stage's own dispatch item 3) and on `Invalidated` (the
/// row is terminal; no retry can succeed).
pub(crate) fn clear_fed_mfa_pending_cookie<'a>(secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(FED_MFA_PENDING_COOKIE, "");
    c.set_http_only(true);
    c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    c.set_max_age(time::Duration::seconds(0));
    c.set_path("/");
    if secure {
        c.set_secure(true);
    }
    c
}

pub(crate) fn clear_fed_mfa_csrf_cookie<'a>(secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(FED_MFA_CSRF_COOKIE, "");
    c.set_http_only(true);
    c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    c.set_max_age(time::Duration::seconds(0));
    c.set_path("/");
    if secure {
        c.set_secure(true);
    }
    c
}

pub(crate) fn clear_fed_mfa_browser_binding_cookie<'a>(secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(FED_MFA_BROWSER_BINDING_COOKIE, "");
    c.set_http_only(true);
    c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    c.set_max_age(time::Duration::seconds(0));
    c.set_path("/");
    if secure {
        c.set_secure(true);
    }
    c
}

/// RFC 096-B1 stage 6a, RFC 096 `:600-601`: *"The callback cookie is
/// expired on every terminal path."* -- both of them, `STATE_COOKIE` and
/// [`BROWSER_BINDING_COOKIE`], not only the one the RFC names.
///
/// **What this is, and what it is not.** `federated_callback` and
/// `complete_federated_signin` both return `Result<Terminal, HttpError>`
/// instead of `Result<Response, HttpError>` -- `Terminal` is the only
/// value either function can wrap in `Ok(...)`, and the only way to build
/// one is [`terminal`], which always removes both cookies first. A
/// `return Ok(some_other_response.into_response())` is a type error, not
/// a reviewable omission: the compiler, not a convention, is what a
/// twenty-fourth terminal path would have to get past.
///
/// **What would bypass it.** Building a `Response` *without* going
/// through `terminal()` and smuggling it out some other way `Result<
/// Terminal, HttpError>` still accepts -- there is no such way from
/// inside these two functions, since `Terminal`'s only field is private
/// to this module and its only constructor is `terminal()` itself. The
/// `Err(HttpError)` side is a genuinely different path (axum's generic
/// error rendering, not a federation redirect) and is not claimed to be
/// covered by this -- the dispatch's own count, *"23 `return Ok(...)`
/// terminal paths,"* already scopes to the `Ok` side, and `Err` returns
/// in this file are pre-existing, unrelated to `:600-601`, and left
/// alone.
pub struct Terminal(Response);

impl IntoResponse for Terminal {
    fn into_response(self) -> Response {
        self.0
    }
}

/// The one constructor for [`Terminal`]. `jar` is taken last-known-state
/// (already carrying whatever the caller added -- a session cookie, a
/// pending-MFA cookie, nothing) and both federation cookies are removed
/// from it before the response is built, unconditionally.
///
/// Both removals carry an explicit `Path=/auth/federated`, matching the
/// path both cookies are actually set with (see the two `set_path` calls
/// above). Without it, removal relies on the browser's *default* path --
/// derived from the request path, not from the cookie's own `Path` --
/// which coincides with `/auth/federated` only because every `Terminal`
/// today is built from `/auth/federated/callback`. **The old form was not
/// broken**: when the computed path matches, the browser treats it as the
/// same cookie and the removal works. What the explicit path removes is
/// the coincidence -- a terminal response served from any other depth
/// would silently stop matching (carried from stage 6a's review, C1).
fn terminal(jar: CookieJar, redirect: Redirect) -> Terminal {
    let jar = jar
        .remove(Cookie::build(STATE_COOKIE).path("/auth/federated"))
        .remove(Cookie::build(BROWSER_BINDING_COOKIE).path("/auth/federated"));
    Terminal((jar, redirect).into_response())
}

/// Raw SHA-256 digest, 32 bytes -- what `federation_login_attempt`'s three
/// hash columns store (RFC 096 `:587-588`: "raw fixed-size values", not hex
/// `TEXT`). `Sha256::digest`'s own output is already exactly 32 bytes, so
/// the `try_into` here cannot fail; it exists to produce a plain `[u8; 32]`
/// rather than carry the `GenericArray` type into every caller.
fn sha256_32(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Lowercase hex, matching `sui_id_core::tokens::sha256_hex`'s own output
/// shape exactly -- `validate_nonce` compares digests, not nonces, and
/// `nonce_sha256` is a 32-byte `BLOB`, not hex `TEXT` (RFC 096
/// `:587-588`). Moved here from the now-retired `federation_attempt_claim`
/// module (stage 3), unchanged: RFC 096-B1 stage 6c splits the claim from
/// the nonce comparison (item 1), not this conversion.
fn hex_lower(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(64);
    for b in bytes {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

/// RFC 096-B1 stage 12c: the `Authorization` header value for
/// `client_secret_basic` (RFC 096 `:320`), built by hand rather than via
/// `reqwest::RequestBuilder::basic_auth` -- that method base64-encodes the
/// raw `username:password` bytes, which is plain HTTP Basic auth (RFC
/// 7617), not what RFC 6749 `:2.3.1` asks for. The OAuth rule is stricter:
/// *"the client identifier and client secret... encoded using the
/// `application/x-www-form-urlencoded` algorithm"* **before** the colon
/// join and the base64 step -- the encoding most implementations skip,
/// and the reason a `client_id`/`client_secret` holding a reserved
/// character (`:`, `&`, `%`, a space) is the test that actually catches a
/// wrong implementation: skipping this step still *looks* correct for any
/// value with none of those characters.
fn basic_auth_header(client_id: &str, client_secret: &str) -> String {
    use base64ct::Encoding;
    let encode =
        |s: &str| -> String { url::form_urlencoded::byte_serialize(s.as_bytes()).collect() };
    let credentials = format!("{}:{}", encode(client_id), encode(client_secret));
    format!(
        "Basic {}",
        base64ct::Base64::encode_string(credentials.as_bytes())
    )
}

/// RFC 096 `:580-581`, `:305-306`: the provider's current, durably checked
/// `config_version`/`activation_generation` pair.
///
/// **Placeholder, not a design choice this stage is making.** Those are
/// RFC 096-B2/M2b's own durable monotonic counters on `federation_provider`
/// -- confirmed, not a guess: `contracts/write-commands.toml`'s `C23`
/// ("Replace federation-provider trust policy") is `status =
/// "target-absent"`, `files = []`, "no current function implements
/// provider trust-policy replacement at all. Wholly new, RFC 096-B2 (M2b)."
/// `federation_provider`'s own migrations (`0037`-`0045`) have no version
/// or generation column either -- checked directly, not assumed from the
/// manifest alone.
///
/// Until M2b adds the real columns, every provider's version/generation is
/// fixed at `(0, 0)`. That makes the provider id/version/generation match
/// `federation_login_attempt::claim` now checks (RFC 096-B1 stage 6c, RFC
/// 096 `:597-598`) trivially satisfied in production today — there is
/// only one value in play, so it can never disagree with itself on a
/// genuine request.
///
/// **It is still reachable end-to-end by a tampered row, though — stage
/// 6c changed this, and it is worth being precise about why.** Before
/// stage 6c, the one way to make `provider_config_version`/`provider_
/// activation_generation` disagree with this constant (direct SQL against
/// the row after `/start`) was caught by a *different* mechanism first:
/// those two columns are AAD-bound by stage 2's `insert`/`claim` seal
/// (`:586-587`'s own PKCE binding), and that tamper check ran before the
/// then-separate, post-claim superseded comparison ever got a chance —
/// `fed_error=tamper_detected`, confirmed empirically, never `fed_error=
/// superseded`. Stage 6c moved the match into `claim` itself, ahead of
/// the AAD check (which still runs, after the `UPDATE`, for whatever only
/// it can still catch). The same tampering now reaches the *match* check
/// first and reports `superseded` — `tests/e2e/r096_b1_stage4.rs`'s own
/// `tampering_the_rows_own_provider_generation_is_caught_as_superseded_
/// not_tampering` pins this directly, corrected from what stage 4
/// originally found and recorded there.
///
/// When M2b lands the real columns, this is the one function that
/// changes — most likely into two field reads on `FederationProviderRow`
/// — not every call site that compares against it.
fn placeholder_provider_version_and_generation(_provider: &FederationProviderRow) -> (i64, i64) {
    (0, 0)
}

/// Maps a claim failure to the `fed_error` query value the callback
/// redirects with. `StoreError::Conflict` (not `pending`) really is a
/// replay of an already-claimed attempt. `AttemptProviderMismatch`
/// (RFC 096-B1 stage 6c) keeps the `superseded` value stage 4's own
/// separate post-claim check used to produce -- the external contract is
/// unchanged even though the check moved inside [`federation_login_attempt::claim`]
/// itself. `AttemptBrowserBindingMismatch` (also stage 6c) is new.
fn claim_fed_error(e: &StoreError) -> &'static str {
    match e {
        StoreError::Conflict => "replay",
        StoreError::AttemptExpired => "expired",
        StoreError::ClockRegression => "clock_regression",
        StoreError::Crypto => "tamper_detected",
        StoreError::NotFound => "attempt_not_found",
        StoreError::AttemptProviderMismatch => "superseded",
        StoreError::AttemptBrowserBindingMismatch => "binding_mismatch",
        _ => "attempt_claim_failed",
    }
}

// ── GET /auth/federated/{slug}/start ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct StartQuery {
    #[serde(default)]
    next: String,
}

/// Initiate a federated sign-in: build the upstream authorization URL,
/// stash state + PKCE in a signed cookie, redirect.
pub async fn federated_start(
    state_ext: AppStateExt,
    Path(slug): Path<String>,
    Query(q): Query<StartQuery>,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    // Load the provider.
    let provider = sui_id_store::repos::federation_provider::get_by_slug(&app.db, &slug)
        .await
        .map_err(|_| HttpError::html(CoreError::NotFound))?;

    if !provider.enabled {
        return Err(HttpError::html(CoreError::BadRequest(
            "federation provider is disabled".into(),
        )));
    }

    // Fetch upstream discovery.
    let discovery = match fetch_discovery(
        &app.http_client,
        &provider.issuer,
        &provider.allowed_origins,
    )
    .await
    {
        Ok(d) => d,
        // RFC 134 D3 §2c: a rejected endpoint is a provider configuration
        // failure, not a user error. Reject before any request is made
        // (already true here — `fetch_discovery` never reaches the
        // request that would use the bad endpoint), log with the slug
        // and the offending origin, audit, and redirect — never echo the
        // offending URL to the browser.
        Err(e) if e.is_invalid() => {
            tracing::warn!(slug = %slug, error = %e, "federation discovery endpoint rejected");
            #[allow(clippy::let_underscore_future)]
            let _ = emit_audit_soon(
                app.db.clone(),
                app.clock.now(),
                sui_id_store::repos::federation_provider::AUDIT_SIGNIN_UPSTREAM_FAILURE,
                Some(format!("provider={} error={e}", provider.slug)),
            );
            return Ok(Redirect::to("/admin/login?fed_error=discovery_origin").into_response());
        }
        Err(e) => {
            tracing::warn!(slug = %slug, error = %e, "federation discovery failed");
            return Err(HttpError::html(CoreError::BadRequest(
                "upstream identity provider is unavailable; try again later".into(),
            )));
        }
    };

    // Build PKCE (S256).
    let pkce_verifier = sui_id_core::tokens::random_token(32);
    // base64url-encode the SHA-256 digest bytes (PKCE uses raw bytes, not hex).
    use base64ct::{Base64UrlUnpadded, Encoding};
    let verifier_bytes = pkce_verifier.as_bytes();
    let challenge_bytes = Sha256::digest(verifier_bytes).to_vec();
    let pkce_challenge_b64 = {
        let mut out = vec![0u8; 64];
        let n = Base64UrlUnpadded::encode(&challenge_bytes, &mut out)
            .map(|s| s.len())
            .unwrap_or(0);
        out.truncate(n);
        String::from_utf8(out).unwrap_or_default()
    };

    // Random nonce (P5 single-use replay protection). RFC 096 `:564`:
    // 32 bytes, like the other three independent CSPRNG values below --
    // bumped from the legacy 16 here (the one place this stage touches the
    // old cookie-state code, since it already generates the value the new
    // attempt row's `nonce_sha256` must hash).
    let nonce = sui_id_core::tokens::random_token(32);

    // Random state parameter for open-redirect guard (CSRF). Same RFC 096
    // `:564` bump as `nonce`, same reason.
    let state_param = sui_id_core::tokens::random_token(32);

    // RFC 096 `:564`: the fourth independent CSPRNG value, bound into
    // the sealed verifier's AAD by `federation_login_attempt::insert` via
    // the attempt row it creates below, and (RFC 096 `:564-566`) carried to
    // the browser in its own cookie. **Not yet compared to anything at
    // claim time** -- stage 3's `claim` (already accepted) checks status,
    // clock regression and expiry, and this stage's own new validation
    // block (below) does not add a browser-binding comparison either,
    // since neither dispatch named one. Generated and stored so the value
    // exists for whichever stage does add that check, not a decision this
    // stage is making about whether to enforce it.
    let browser_binding = sui_id_core::tokens::random_token(32);

    // RFC 096-B1 stage 12a, §2: `next` used to be carried in the retired
    // sealed state cookie alongside everything else; the attempt row's own
    // `next_path` column (below) has carried it durably since stage 2, and
    // the callback reads `claimed.next_path`, never a cookie -- so this is
    // the one and only place `next` needs computing now.
    let next_path = if q.next.starts_with('/') {
        Some(q.next)
    } else {
        None
    };

    // RFC 096-B1 stage 4: the durable attempt row the callback claims and
    // validates against -- now the *only* record of this request, since
    // the sealed-state cookie stage 12a retires carried nothing the row
    // did not already carry durably (measured field-by-field in the
    // review package this dispatch was built against).
    let now = app.clock.now();
    let (provider_config_version, provider_activation_generation) =
        placeholder_provider_version_and_generation(&provider);
    let redirect_uri = format!(
        "{}/auth/federated/callback",
        app.config.server.issuer.trim_end_matches('/')
    );
    let attempt = federation_login_attempt::insert(
        &app.db,
        provider.id,
        provider_config_version,
        provider_activation_generation,
        sha256_32(state_param.as_bytes()),
        sha256_32(nonce.as_bytes()),
        sha256_32(browser_binding.as_bytes()),
        pkce_verifier.as_bytes(),
        redirect_uri.clone(),
        next_path,
        now,
    )
    .await
    .map_err(|e| {
        tracing::error!(slug = %slug, error = %e, "federation: attempt row insert failed");
        HttpError::html(CoreError::Internal)
    })?;
    let _ = attempt; // held only for its side effect here; the callback re-finds it by state hash.

    // RFC 096 `:564-566`: the browser receives only the opaque binding, in
    // a Secure, HttpOnly, SameSite=Lax cookie scoped to the provider
    // callback. Its own lifetime is the attempt row's own
    // `ATTEMPT_LIFETIME_SECS` directly (previously a separate constant of
    // the same cookie-state module that happened to equal it) -- one
    // source for the window, not two that could drift apart.
    let browser_binding_cookie = {
        let mut c = Cookie::new(BROWSER_BINDING_COOKIE, browser_binding);
        c.set_http_only(true);
        c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
        c.set_max_age(time::Duration::seconds(
            federation_login_attempt::ATTEMPT_LIFETIME_SECS,
        ));
        c.set_path("/auth/federated");
        if app.config.server.cookie_secure {
            c.set_secure(true);
        }
        c
    };

    // Build the upstream authorization URL. `redirect_uri` is the one
    // computed above, alongside the attempt row -- the same request uses
    // it both places.
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let enc = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();

    let _upstream_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}\
         &code_challenge={}&code_challenge_method=S256",
        discovery.authorization_endpoint(),
        enc(&provider.client_id),
        enc(&redirect_uri),
        enc(&provider.scopes),
        enc(&state_param),
        enc(&nonce),
        enc(&pkce_challenge_b64),
    );
    let upstream_url = _upstream_url;

    Ok((jar.add(browser_binding_cookie), Redirect::to(&upstream_url)).into_response())
}

// ── GET /auth/federated/callback ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    error: Option<String>,
    state: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    // RFC 096 `:593-595`, P6: the upstream access token is never persisted.
    // Stage 12a strengthens this from "captured, zeroized on drop, never
    // read" to "never captured at all" -- `access_token`'s one reader was
    // the legacy userinfo-fallback branch this stage removes (dead in
    // practice since the mandatory-id_token refusal above it, and now
    // unreachable in full), and `serde`'s default struct deserialization
    // never retains a JSON member it has no field for, the same reasoning
    // a `refresh_token` field (which this struct also omits) already
    // relied on. A field that is never named here is dropped at parse
    // time, not read into memory as a named value this file would then
    // have to remember never to use.
    id_token: Option<String>,
}

/// Handle the upstream callback: exchange code, validate ID token, resolve link.
pub async fn federated_callback(
    state_ext: AppStateExt,
    Query(q): Query<CallbackQuery>,
    jar: CookieJar,
) -> Result<Terminal, HttpError> {
    let State(app) = state_ext;

    // Reject upstream errors. RFC 096 `:714-715` (review R2's full-path
    // grep): the `error` query parameter is attacker-controlled upstream
    // text (RFC 096's own threat model treats the upstream as potentially
    // hostile or compromised) -- it is never logged, not even truncated or
    // allowlisted, which is also why it is dropped here before the attempt
    // row is even looked up and no provider identity is available to log
    // instead.
    if q.error.is_some() {
        tracing::warn!("federation callback: upstream returned an error response");
        // RFC 096 `:603-604`: error responses consume the matching attempt
        // without token exchange. The attempt is findable from the `state`
        // the error response itself carries -- not the sealed cookie, which
        // this path never reads -- hashed and looked up the same way a
        // genuine callback finds its own attempt. Best-effort and silent
        // either way: whether `state` is absent, matches no row, or finds
        // one, the response is identical, so this can never become an
        // oracle for which state digests exist (an unauthenticated caller
        // controls this entire request).
        if let Some(state_param) = q.state.as_deref() {
            let state_hash = sha256_32(state_param.as_bytes());
            if let Ok(attempt) =
                federation_login_attempt::find_by_state_sha256(&app.db, state_hash).await
            {
                terminalize_and_audit(
                    &app.db,
                    attempt.id,
                    TerminalFailureReason::UpstreamFailure(
                        UpstreamFailureDetail::UpstreamErrorResponse,
                    ),
                    app.clock.now(),
                    attempt.provider_id,
                    None,
                )
                .await;
            }
        }
        return Ok(terminal(
            jar,
            Redirect::to("/admin/login?fed_error=upstream"),
        ));
    }

    let code = q
        .code
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("missing code in callback".into())))?;

    // ── RFC 096-B1 stage 12a, §2: the state match, and finding the attempt ──
    //
    // The retired sealed state cookie used to carry its own
    // copy of the `state` value we sent, `ct_eq`-compared against the
    // query's returned value, *then* hashed to find the attempt row by
    // `state_sha256`. Both steps compared against the same durable
    // `state_sha256` column in the end -- hashing the query's own returned
    // `state` and looking the row up by that hash directly collapses them
    // into one: a wrong, missing, or replayed `state` finds no row, which
    // is the same refusal `ct_eq` gave before, now pinned by
    // `tests/e2e/r096_b1_stage12a.rs`'s own
    // `wrong_state_finds_no_attempt_row` rather than only where it used to
    // live. RFC 096 `:714-715`: nothing about which digests do or do not
    // exist is observable from the response either way (module doc, above).
    let returned_state = q.state.as_deref().unwrap_or("");
    let state_hash = sha256_32(returned_state.as_bytes());
    let pre_claim_attempt =
        match federation_login_attempt::find_by_state_sha256(&app.db, state_hash).await {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(error = %e, "federation: state mismatch — possible CSRF");
                return Ok(terminal(
                    jar,
                    Redirect::to("/admin/login?fed_error=state_mismatch"),
                ));
            }
        };

    // Load the provider by the attempt row's own internal id -- not by a
    // slug a cookie would have had to carry (RFC 096 `:715`'s own
    // preference for internal ids, applied here as well as in every audit
    // call site).
    let provider =
        sui_id_store::repos::federation_provider::get(&app.db, pre_claim_attempt.provider_id)
            .await
            .map_err(|_| HttpError::html(CoreError::NotFound))?;

    if !provider.enabled {
        return Ok(terminal(
            jar,
            Redirect::to("/admin/login?fed_error=disabled"),
        ));
    }

    // ── RFC 096-B1 stage 6c, RFC 096 `:597-599`: the claim, moved here ───────
    //
    // *"One immediate database transaction matches state digest, provider
    // ID/version/generation, pending state, unexpired time, and constant-
    // time browser binding; it changes pending to exchanging before token
    // exchange."* Everything below this point runs with the attempt
    // `exchanging`, and every failure from here on terminalizes it via F05
    // (`terminalize_and_audit`) rather than leaving it to expire.
    //
    // Provider id/version/generation and the browser binding are the two
    // elements `:597-598` added to the match set this stage; pending state,
    // unexpired time and clock-regression were already checked by `claim`
    // (stage 3) and are unchanged here, only moved earlier.

    // RFC 096 `:564-566`: the opaque browser-binding cookie, read here
    // rather than at claim time in the old code (there was no comparison
    // at all before this stage). A missing cookie is represented as a
    // fixed sentinel rather than skipping the comparison -- `:598`
    // requires it to refuse the same way a wrong one does, not take a
    // different path through this function that an absent cookie could
    // otherwise be used to distinguish.
    let presented_browser_binding = jar
        .get(BROWSER_BINDING_COOKIE)
        .map(|c| sha256_32(c.value().as_bytes()))
        .unwrap_or([0u8; 32]);

    let (current_version, current_generation) =
        placeholder_provider_version_and_generation(&provider);
    let claim_time = app.clock.now();
    let claimed = match federation_login_attempt::claim(
        &app.db,
        pre_claim_attempt.id,
        claim_time,
        provider.id,
        current_version,
        current_generation,
        presented_browser_binding,
    )
    .await
    {
        Ok(row) => row,
        Err(e) => {
            // Refused before any mutation: the row is exactly as it was
            // (still `pending`, or already terminal) -- RFC 096 `:601-605`'s
            // own "or leaves it safely reconcilable as failed" clause is
            // what covers this case, via the 24-hour purge
            // (`federation_login_attempt::purge_expired`), not an explicit
            // F05 call here. There is nothing yet to terminalize: F05
            // guards `pending|exchanging -> failed`, and a claim this
            // request lost (`Conflict`) may already be owned by a
            // concurrent winner this request must not disturb.
            tracing::warn!(slug = %provider.slug, error = ?e, "federation: attempt claim failed");
            return Ok(terminal(
                jar,
                Redirect::to(&format!("/admin/login?fed_error={}", claim_fed_error(&e))),
            ));
        }
    };

    // RFC 096-B1 stage 12a, §2: the PKCE verifier, read back from the
    // attempt row rather than the retired sealed state cookie. `claim`'s
    // own tamper check just opened this exact sealed
    // blob with this exact AAD and discarded the plaintext (it had no
    // caller for it yet); this is that caller, opening the same ciphertext
    // a second time with the same inputs rather than growing `claim`'s own
    // signature and its tests for a second return value. Guaranteed to
    // succeed whenever `claim` just did: the AAD is built from four columns
    // `claim` already re-read unchanged (`row.id`/`provider_id`/`provider_
    // config_version`/`provider_activation_generation`, carried into
    // `claimed` via its own `..row`), and the sealed bytes and the master
    // key are the same two values either open call sees.
    let pkce_verifier_aad = federation_login_attempt::build_aad(
        claimed.id,
        claimed.provider_id,
        claimed.provider_config_version,
        claimed.provider_activation_generation,
    );
    let pkce_verifier = match sui_id_store::crypto::open(
        app.db.key(),
        &claimed.pkce_verifier_sealed,
        &pkce_verifier_aad,
    )
    .and_then(|plain| String::from_utf8(plain).map_err(|_| StoreError::Crypto))
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(slug = %provider.slug, error = %e, "federation: pkce verifier decrypt failed");
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::Denied(DeniedDetail::PkceVerifierDecryptFailed),
                app.clock.now(),
                provider.id,
                None,
            )
            .await;
            return Err(HttpError::html(CoreError::from(e)));
        }
    };

    // Fetch upstream discovery for token_endpoint.
    let discovery = match fetch_discovery(
        &app.http_client,
        &provider.issuer,
        &provider.allowed_origins,
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            let log_msg = if e.is_invalid() {
                "federation token exchange: discovery endpoint rejected"
            } else {
                "federation token exchange: discovery failed"
            };
            tracing::warn!(slug = %provider.slug, error = %e, "{log_msg}");
            // RFC 096-B1 stage 6c, F05: a post-claim transport failure --
            // the attempt is now `exchanging` (claimed above), so this is
            // F05's own job, through `terminalize_and_audit` rather than
            // the old fire-and-forget `emit_audit_soon`/`AUDIT_SIGNIN_
            // UPSTREAM_FAILURE` best-effort append. (`federated_start`'s
            // own discovery failure keeps that old path: no attempt row
            // exists yet at that point, so there is nothing for F05 to
            // terminalize.)
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::UpstreamFailure(if e.is_invalid() {
                    UpstreamFailureDetail::DiscoveryRejected
                } else {
                    UpstreamFailureDetail::DiscoveryFailed
                }),
                app.clock.now(),
                provider.id,
                None,
            )
            .await;
            // RFC 134 D3 §2c: a rejected endpoint redirects with the same
            // fed_error shape every other callback failure uses, not the
            // generic bad-request page the pre-existing network/parse
            // failure path used (unchanged below for that case, since it
            // predates this control and isn't what's dispatched here).
            if e.is_invalid() {
                return Ok(terminal(
                    jar,
                    Redirect::to("/admin/login?fed_error=discovery_origin"),
                ));
            }
            return Err(HttpError::html(CoreError::BadRequest(
                "upstream IdP unavailable".into(),
            )));
        }
    };

    // Decrypt client secret (P6 — used for token exchange only, not stored).
    let client_secret = match sui_id_store::repos::federation_provider::decrypt_secret(
        app.db.key(),
        &provider,
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(slug = %provider.slug, error = %e, "federation: client secret decrypt failed");
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::Denied(DeniedDetail::ClientSecretDecryptFailed),
                app.clock.now(),
                provider.id,
                None,
            )
            .await;
            return Err(HttpError::html(CoreError::from(e)));
        }
    };

    // RFC 096-B1 stage 12c: the configured-method cross-check RFC 096
    // `:570` asks for (*"discovery's `token_endpoint_auth_methods_supported`
    // must contain the configured method"*). `:320`'s own profile table
    // allows exactly `client_secret_basic` or `none` -- `client_secret_post`
    // is explicitly excluded. Stage 12a built this check, watched it refuse
    // every provider this handler could reach, and found why: the handler
    // sent a confidential client's secret in the form body
    // (`client_secret_post`), the one method `:320` excludes. Fixed here --
    // see `basic_auth_header` below -- so the configured method is now
    // always one `:320` allows.
    let configured_auth_method = if client_secret.is_some() {
        "client_secret_basic"
    } else {
        "none"
    };
    if !discovery.supports_token_endpoint_auth_method(configured_auth_method) {
        tracing::warn!(
            slug = %provider.slug,
            method = configured_auth_method,
            "federation: token endpoint does not advertise the configured auth method"
        );
        terminalize_and_audit(
            &app.db,
            claimed.id,
            TerminalFailureReason::Denied(DeniedDetail::TokenEndpointAuthMethodUnsupported),
            app.clock.now(),
            provider.id,
            None,
        )
        .await;
        return Ok(terminal(
            jar,
            Redirect::to("/admin/login?fed_error=token_endpoint_auth_method"),
        ));
    }

    // Exchange the code for tokens at the upstream token_endpoint.
    let redirect_uri = format!(
        "{}/auth/federated/callback",
        app.config.server.issuer.trim_end_matches('/')
    );
    let form_params = vec![
        ("grant_type", "authorization_code".to_owned()),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", provider.client_id.clone()),
        ("code_verifier", pkce_verifier),
    ];

    // RFC 096-B1 stage 12c: `client_secret_basic` for a confidential
    // client, nothing for a public one (`:320`) -- the secret moves from
    // the form body to the `Authorization` header, never both.
    let mut request_builder = app
        .http_client
        .post(discovery.token_endpoint())
        .form(&form_params);
    if let Some(ref secret) = client_secret {
        request_builder = request_builder.header(
            reqwest::header::AUTHORIZATION,
            basic_auth_header(&provider.client_id, secret),
        );
    }

    let token_resp = request_builder
        .send()
        .await
        .and_then(|r| r.error_for_status());

    let token_resp = match token_resp {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, slug = %provider.slug, "token exchange failed");
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::UpstreamFailure(UpstreamFailureDetail::TokenExchangeFailed),
                app.clock.now(),
                provider.id,
                None,
            )
            .await;
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=token_exchange"),
            ));
        }
    };

    // RFC 134 D5 Tier 1: the same bounded pipeline as discovery.
    let tokens: TokenResponse = match crate::response_bounds::read_bounded_json(token_resp).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "federation token parse failed");
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::UpstreamFailure(UpstreamFailureDetail::TokenParseFailed),
                app.clock.now(),
                provider.id,
                None,
            )
            .await;
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=token_parse"),
            ));
        }
    };

    // ── RFC 096-B1 stage 4/12a: the attempt claim and 096-A's validators ────
    //
    // Everything below fails the request closed on any error -- a forged
    // signature, wrong issuer, stale `iat`, replayed or mismatched nonce,
    // or a superseded attempt is refused here, by the validators 096-A
    // built and stage 12a made the sole, authoritative identity decision
    // for this handler (the trust-on-TLS decode they shadowed, and the
    // identity-mapping code built on top of it, are gone).
    //
    // RFC 096's own requirement (`:593-594`): the ID token is mandatory,
    // checked immediately below -- the userinfo-endpoint fallback this
    // handler used before stage 12a, for a response with no `id_token` at
    // all, had been unreachable since this refusal was added and is
    // removed along with everything else this stage retires.
    let id_token_jwt = match tokens.id_token.as_deref() {
        Some(jwt) => jwt,
        None => {
            tracing::warn!(slug = %provider.slug, "federation: id_token absent (mandatory, RFC 096 :593-594)");
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::Denied(DeniedDetail::MissingIdToken),
                app.clock.now(),
                provider.id,
                None,
            )
            .await;
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=missing_id_token"),
            ));
        }
    };

    // The provider's startup-configured trust: `id_token_algs` is never
    // learned from discovery, only narrowed by it (`id_token_algs_
    // intersection`'s own doc comment) -- looked up by slug since this is
    // `[[federation_providers]]` config, not a `federation_provider` DB
    // column.
    let Some(provider_config) = app
        .config
        .federation_providers
        .iter()
        .find(|p| p.slug == provider.slug)
    else {
        // Measured, not assumed: a provider enabled in the database with
        // no matching `[[federation_providers]]` block is a startup
        // configuration drift this handler cannot resolve on its own.
        tracing::error!(slug = %provider.slug, "federation: enabled provider has no matching config block");
        terminalize_and_audit(
            &app.db,
            claimed.id,
            TerminalFailureReason::Denied(DeniedDetail::ConfigDrift),
            app.clock.now(),
            provider.id,
            None,
        )
        .await;
        return Ok(terminal(
            jar,
            Redirect::to("/admin/login?fed_error=config_drift"),
        ));
    };
    let allowed_algs = discovery.id_token_algs_intersection(&provider_config.id_token_algs);

    let verified_claims = match verify_id_token(
        &app.http_client,
        id_token_jwt,
        &allowed_algs,
        discovery.jwks_uri(),
    )
    .await
    {
        Ok(claims) => claims,
        Err(e) => {
            tracing::warn!(slug = %provider.slug, error = ?e, "federation: id_token signature/structure verification failed");
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::Denied(DeniedDetail::SignatureInvalid),
                app.clock.now(),
                provider.id,
                None,
            )
            .await;
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=signature_invalid"),
            ));
        }
    };

    // One clock sample for everything below: `iat`'s lower/upper bound and
    // the capability's `validated_at` -- not `app.clock.now()` called
    // several separate times. `claim_time` (above, at the claim itself) is
    // deliberately a separate, earlier sample: RFC 096-B1 stage 6c moved
    // the claim before token exchange, so it is no longer the same instant
    // as the checks below it.
    let validation_now = app.clock.now();

    // RFC 096-B1 stage 6c, item 1: the nonce comparison, split from the
    // claim (RFC 096 `:65-68` -- 096-A's validation rule, applied to
    // `claimed`'s own `nonce_sha256` now that the ID token exists to
    // compare against it). Unlike the claim itself, a nonce mismatch does
    // not roll back `claimed`'s `pending -> exchanging` transition --
    // matching stage 3's own original decision, carried forward rather
    // than revisited by this stage -- so this is F05's job, not a second
    // path back to `pending`.
    let expected_nonce_hex = hex_lower(&claimed.nonce_sha256);
    if let Err(e) = validate_nonce(&verified_claims, &expected_nonce_hex) {
        tracing::warn!(slug = %provider.slug, error = ?e, "federation: nonce mismatch");
        terminalize_and_audit(
            &app.db,
            claimed.id,
            TerminalFailureReason::Denied(DeniedDetail::NonceMismatch),
            validation_now,
            provider.id,
            None,
        )
        .await;
        return Ok(terminal(
            jar,
            Redirect::to("/admin/login?fed_error=nonce_mismatch"),
        ));
    }

    let identity = match validate_identity_claims(
        &verified_claims,
        &provider.issuer,
        &provider.client_id,
    ) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(slug = %provider.slug, error = ?e, "federation: identity claims rejected");
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::Denied(DeniedDetail::IdentityClaimsInvalid),
                validation_now,
                provider.id,
                None,
            )
            .await;
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=identity_claims_invalid"),
            ));
        }
    };

    // RFC 096 `:676`: `iat`'s lower bound is the attempt's own `created_at`
    // -- the attempt-binding defence against a token minted long before
    // this attempt existed.
    if let Err(e) = validate_iat(&verified_claims, claimed.created_at, validation_now) {
        tracing::warn!(slug = %provider.slug, error = ?e, "federation: iat rejected");
        terminalize_and_audit(
            &app.db,
            claimed.id,
            TerminalFailureReason::Denied(DeniedDetail::TimeClaimsInvalid),
            validation_now,
            provider.id,
            None,
        )
        .await;
        return Ok(terminal(
            jar,
            Redirect::to("/admin/login?fed_error=time_claims_invalid"),
        ));
    }

    let optional = match validate_optional_claims(&verified_claims) {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(slug = %provider.slug, error = ?e, "federation: optional claims rejected");
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::Denied(DeniedDetail::OptionalClaimsInvalid),
                validation_now,
                provider.id,
                None,
            )
            .await;
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=optional_claims_invalid"),
            ));
        }
    };

    let capability = construct_identity_capability(
        &identity,
        &optional,
        CacheKey::new(
            provider.id,
            ProviderVersion(claimed.provider_config_version as u64),
            ActivationGeneration(claimed.provider_activation_generation as u64),
        ),
        validation_now,
    );

    // RFC 096-B1 stage 5: resolve the capability to one of the five named
    // states (RFC 096 `:693-711`) -- a decision, not a mutation. "What it
    // does with the decision is: nothing yet -- hold it, and let stage 6
    // consume it." Computed on every real request, so it is exercised
    // rather than dead code; each state is unit-tested on its own
    // (`identity_resolution::tests`). What a unit suite cannot prove is
    // what a caller observes on the live path -- that authority still
    // belongs to the legacy code below, unchanged, until stage 6 makes
    // this resolution the one that acts; the `link_only`-is-generic
    // property specifically is proven against that live path by this
    // file's own e2e tests, not by the unit suite.
    // RFC 096-B1 stage 12a, §1: `resolution` now acts -- the five states
    // RFC 096 `:693-711` names, each mapped onto exactly the outcome the
    // legacy block (removed this stage) produced for it, confirmed arm by
    // arm against that block before it was deleted:
    //
    // | `IdentityResolution`     | legacy outcome it replaces              |
    // |--------------------------|------------------------------------------|
    // | `ExistingLink { user_id }` | `link_opt: Some(existing_link)` branch |
    // | `LinkRequired`           | the `ProvisionMode::LinkOnly` arm         |
    // | `ProvisionEligible`      | falls through to `provision_federated_user` |
    // | `DeniedCollision`        | the `find_by_email_normalized` P2 check   |
    // | `DeniedUnverified`       | `email.is_none() || !email_verified`      |
    let resolution = match resolve_verified_identity(
        &app.db,
        &capability,
        provider.provision_mode.clone(),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::Denied(DeniedDetail::IdentityResolutionDbError),
                validation_now,
                provider.id,
                None,
            )
            .await;
            return Err(HttpError::html(CoreError::from(e)));
        }
    };

    // P6: the upstream access token was used for discovery/token-exchange
    // bookkeeping above and is dropped with `tokens` at the end of this
    // scope — never persisted.

    let user_id: UserId = match resolution {
        // ── Known user: update last_seen and proceed to MFA gate ─────────────
        IdentityResolution::ExistingLink { user_id } => {
            // `linked_at` is read by `upsert`'s own `INSERT`, but this row
            // is only ever reached via `ExistingLink`, which
            // `resolve_verified_identity` only returns when a link for
            // `(provider.id, capability.sub())` already exists -- so the
            // `(user_id, provider_id)` conflict this call's own `INSERT`
            // target fires every time, and `federation_link::upsert`'s own
            // `ON CONFLICT` clause never writes `linked_at` at all (only
            // `upstream_email`/`last_seen_at` are in its `SET`). The value
            // passed here is therefore never actually stored; `validation_now`
            // is used rather than re-reading the row a second time for a
            // column this call can never persist.
            if let Err(e) = sui_id_store::repos::federation_link::upsert(
                &app.db,
                FederationLinkRow {
                    user_id,
                    provider_id: provider.id,
                    upstream_sub: capability.sub().to_owned(),
                    upstream_email: capability.verified_email().map(str::to_owned),
                    linked_at: validation_now,
                    last_seen_at: validation_now,
                },
            )
            .await
            {
                terminalize_and_audit(
                    &app.db,
                    claimed.id,
                    TerminalFailureReason::Denied(DeniedDetail::LinkUpsertDbError),
                    validation_now,
                    provider.id,
                    Some(user_id),
                )
                .await;
                return Err(HttpError::html(CoreError::from(e)));
            }
            user_id
        }

        // ── No local account is linked; `link_only` denies generically ──────
        IdentityResolution::LinkRequired => {
            // RFC 096-B1 stage 5, RFC 096 `:699-701`: one generic "local
            // account link required" result for every unlinked upstream
            // identity under `link_only` -- generic is load-bearing, since
            // `resolve_verified_identity` itself never reads local state on
            // this path (its own doc comment), so there is nothing here
            // that could narrow it either.
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::LinkRequired,
                validation_now,
                provider.id,
                None,
            )
            .await;
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=link_required"),
            ));
        }

        // ── Verified email collides with a different, unlinked local user ──
        IdentityResolution::DeniedCollision => {
            // RFC 096 `:714-715` (review R2): no raw email in the log field
            // or the audit note -- internal `provider_id` and a bounded
            // reason token only.
            tracing::warn!(
                provider_id = %provider.id,
                "federation: email collision — potential takeover attempt blocked (P2)"
            );
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::TakeoverBlocked,
                validation_now,
                provider.id,
                None,
            )
            .await;
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=email_collision"),
            ));
        }

        // ── No verified email; `provision_on_first_login` cannot proceed ───
        IdentityResolution::DeniedUnverified => {
            tracing::info!(slug = %provider.slug, "provision held: absent or unverified email");
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::Denied(DeniedDetail::UnverifiedEmail),
                validation_now,
                provider.id,
                None,
            )
            .await;
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=unverified_email"),
            ));
        }

        // ── No local account is linked; eligible for first-login provisioning ──
        IdentityResolution::ProvisionEligible => {
            // P7: derive username from the validated capability, never
            // trust upstream directly.
            let proposed = derive_username(&capability);
            let username = resolve_shadow_username(&app.db, &proposed).await;

            // RFC 096-B1 stage 7, F04: attempt completion, passwordless
            // non-admin user, link, `[Fed]` session/cap, and the
            // `auth.federation.provisioned` commit, all in one Class-A
            // transaction (RFC 096 `:706-711`). The id is generated here,
            // before the transaction, and used identically on the user,
            // link and session rows -- the three rows the transaction
            // creates must already agree on who they are about.
            let uid = UserId::new();
            let user = UserRow {
                id: uid,
                username,
                display_name: capability.name().map(str::to_owned),
                is_admin: false,
                role: Role::User,
                is_disabled: false,
                is_deleted: false,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: validation_now,
                updated_at: validation_now,
                failed_login_count: 0,
                locked_until: None,
                email: capability.verified_email().map(str::to_owned),
                preferred_lang: None,
                email_normalized: None,
                email_verified_at: None,
                last_login_at: None,
                // `source` stays the plain default, not `UserSource::Ldap`
                // the legacy writer used -- unchanged from before this
                // stage (review package §2 of the stage this code
                // originated in); `sui_id_core::authn::step_up::first_
                // factor_proof`, the one place a federated user's `source`
                // could matter, already checks `federation_link` existence
                // first.
                source: UserSource::Local,
                external_stable_id: None,
            };
            let link = FederationLinkRow {
                user_id: uid,
                provider_id: provider.id,
                upstream_sub: capability.sub().to_owned(),
                upstream_email: capability.verified_email().map(str::to_owned),
                linked_at: validation_now,
                last_seen_at: validation_now,
            };
            let session_row = SessionRow {
                id: SessionId::new(),
                user_id: uid,
                expires_at: validation_now + chrono::Duration::hours(24),
                created_at: validation_now,
                revoked_at: None,
                auth_methods: vec![sui_id_shared::AuthMethod::Fed],
                last_step_up_at: None,
                last_used_at: None,
            };
            let session_id = session_row.id;
            return match sui_id_store::commands::provision_federated_user(
                &app.db,
                claimed.id,
                provider.id,
                user,
                link,
                session_row,
            )
            .await
            {
                Ok(_audited) => {
                    if let Some(m) = app.metric() {
                        m.signin(sui_id_store::metrics::signin_result::SUCCESS);
                    }
                    let cookie =
                        session_cookie(session_id.to_string(), app.config.server.cookie_secure);
                    let jar = jar.add(cookie);
                    Ok(terminal(jar, Redirect::to("/admin")))
                }
                Err(e) => {
                    tracing::warn!(
                        provider_id = %provider.id,
                        error = %e,
                        "federation: first-login provisioning failed"
                    );
                    terminalize_and_audit(
                        &app.db,
                        claimed.id,
                        TerminalFailureReason::Denied(DeniedDetail::ProvisionTransactionFailed),
                        validation_now,
                        provider.id,
                        None,
                    )
                    .await;
                    Ok(terminal(
                        jar,
                        Redirect::to("/admin/login?fed_error=attempt_claim_failed"),
                    ))
                }
            };
        }
    };

    // ── P4: enforce local MFA ─────────────────────────────────────────────────
    complete_federated_signin(
        app,
        jar,
        user_id,
        &claimed,
        provider.id,
        &provider.slug,
        validation_now,
    )
    .await
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Complete a federated sign-in: enforce local MFA then issue a session.
///
/// `attempt_id`/`provider_id` (RFC 096-B1 stage 6c) are for F05 and this
/// function's own failure paths only: a successful sign-in or a
/// pending-MFA continuation does *not* terminalize the attempt here --
/// that is F01's (RFC 096-B1 stage 6d, inside `commands::
/// sign_in_federated`'s own transaction) or F02's (stage 9, inside
/// `commands::issue_federated_mfa_pending`'s) own guarded
/// `exchanging -> completed` transition, each atomic with the write it
/// gates rather than a step this function takes on their behalf. Only
/// this function's own *failure* paths call `terminalize_and_audit`.
#[allow(clippy::too_many_arguments)]
async fn complete_federated_signin(
    app: AppState,
    jar: CookieJar,
    user_id: UserId,
    claimed: &sui_id_store::models::FederationLoginAttemptRow,
    provider_id: FederationProviderId,
    provider_slug: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Terminal, HttpError> {
    let attempt_id = claimed.id;
    // Refuse an inactive user before any pending-MFA or session row. The
    // user is re-read here: the link may belong to an account disabled or
    // deleted since it was created.
    let signin_failed = |jar: CookieJar| {
        Ok(terminal(
            jar,
            Redirect::to("/admin/login?fed_error=signin_failed"),
        ))
    };
    match sui_id_store::repos::users::get(&app.db, user_id).await {
        Ok(user) if !user.is_disabled && !user.is_deleted => {}
        Ok(_) => {
            tracing::warn!(
                provider = %provider_slug,
                user_id = %user_id,
                "federation: sign-in refused for a disabled or deleted user"
            );
            terminalize_and_audit(
                &app.db,
                attempt_id,
                TerminalFailureReason::Denied(DeniedDetail::AccountDisabledOrDeleted),
                now,
                provider_id,
                Some(user_id),
            )
            .await;
            return signin_failed(jar);
        }
        Err(e) => {
            tracing::error!(
                provider = %provider_slug,
                user_id = %user_id,
                error = %e,
                "federation: user read failed; sign-in refused"
            );
            terminalize_and_audit(
                &app.db,
                attempt_id,
                TerminalFailureReason::Denied(DeniedDetail::UserReadDbError),
                now,
                provider_id,
                Some(user_id),
            )
            .await;
            return signin_failed(jar);
        }
    }

    // P4: check local MFA. The decision must come from a successful read:
    // treating a failed read as "no MFA" would skip the second factor.
    let mfa_enabled = match sui_id_core::mfa::is_mfa_enabled(&app.db, user_id).await {
        Ok(enabled) => enabled,
        Err(e) => {
            tracing::error!(
                provider = %provider_slug,
                user_id = %user_id,
                error = %e,
                "federation: MFA state read failed; sign-in refused"
            );
            terminalize_and_audit(
                &app.db,
                attempt_id,
                TerminalFailureReason::Denied(DeniedDetail::MfaStateReadDbError),
                now,
                provider_id,
                Some(user_id),
            )
            .await;
            return signin_failed(jar);
        }
    };

    if mfa_enabled {
        // RFC 096-B1 stage 9, F02: complete the attempt and record the
        // durable, federation-shaped continuation (migration 0047) in one
        // transaction -- fixing the status-quo bug this branch had before
        // F02 existed: the attempt was left `exchanging` forever (collected
        // only by the 24-hour purge), because nothing on this branch ever
        // completed it.
        //
        // The generic `login_pending_mfa` row immediately below is
        // UNCHANGED and is still what the user actually completes MFA
        // through -- nothing consumes `federation_mfa_pending` yet
        // (`FederatedMfaVerifier` is stage 10/11; stage 12 switches
        // `/admin/login/mfa`'s federated branch over to it). The two rows
        // exist side by side, deliberately, until then: ripping out the
        // generic row now would break federated sign-in for every user
        // with local MFA enabled, for no consumer yet able to replace it.
        let csrf_raw = sui_id_core::tokens::random_token(32);
        // RFC 096-B1 stage 9 fix round, R1: a browser-binding value of
        // this continuation's own, not the attempt's -- see
        // `FED_MFA_BROWSER_BINDING_COOKIE`'s own doc for why reusing
        // `claimed.browser_binding_sha256` is unsound (its cookie is
        // revoked by `terminal()` on this very response, and scoped away
        // from the reader regardless).
        let fed_browser_binding_raw = sui_id_core::tokens::random_token(32);
        let (fed_pending_id, fed_pending_expires_at) =
            match sui_id_store::commands::issue_federated_mfa_pending(
                &app.db,
                attempt_id,
                provider_id,
                claimed.provider_config_version,
                claimed.provider_activation_generation,
                user_id,
                claimed.next_path.clone(),
                sha256_32(fed_browser_binding_raw.as_bytes()),
                sha256_32(csrf_raw.as_bytes()),
                now,
            )
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!(
                        provider = %provider_slug,
                        user_id = %user_id,
                        error = %e,
                        "federation: F02 (attempt completion + MFA continuation) failed"
                    );
                    terminalize_and_audit(
                        &app.db,
                        attempt_id,
                        TerminalFailureReason::Denied(DeniedDetail::MfaPendingTransactionFailed),
                        now,
                        provider_id,
                        Some(user_id),
                    )
                    .await;
                    return signin_failed(jar);
                }
            };

        let pending = match sui_id_core::mfa::issue_pending_mfa(&app.db, &app.clock, user_id).await
        {
            Ok(p) => p,
            Err(e) => {
                // F02 already completed the attempt above; this is only
                // the generic row the *current* MFA-prompt page still
                // reads. Nothing to terminalize a second time -- the
                // attempt is already `completed`, not `exchanging`, so
                // `terminalize_and_audit`'s own `UPDATE ... WHERE status
                // IN ('pending', 'exchanging')` would no-op anyway.
                tracing::error!(
                    provider = %provider_slug,
                    user_id = %user_id,
                    error = %e,
                    "federation: generic pending-MFA issue failed"
                );
                return Err(HttpError::html(e));
            }
        };
        let cookie = crate::handlers::pending_mfa_cookie(
            pending.id.to_string(),
            app.config.server.cookie_secure,
        );
        // RFC 096-B1 stage 12b, §4: the row's own remaining lifetime, not
        // a flat `PENDING_LIFETIME_SECS` -- `fed_pending_expires_at` is
        // the *existing* row's own value on a restart (returned by F02
        // above unchanged, never refreshed), so a cookie that always
        // claimed a fresh 300 seconds could outlive the row it names.
        // Clamped at zero: a row already expired (reachable only for the
        // `Exhausted` branch, which still returns its own stale
        // `expires_at`) gets a cookie that expires immediately rather
        // than a negative `Duration`, which `time` itself would refuse.
        let fed_cookie_max_age =
            time::Duration::seconds((fed_pending_expires_at - now).num_seconds().max(0));
        let fed_pending_cookie = fed_mfa_pending_cookie(
            fed_pending_id.to_string(),
            fed_cookie_max_age,
            app.config.server.cookie_secure,
        );
        let fed_csrf_cookie = fed_mfa_csrf_cookie(
            csrf_raw,
            fed_cookie_max_age,
            app.config.server.cookie_secure,
        );
        let fed_browser_binding_cookie = fed_mfa_browser_binding_cookie(
            fed_browser_binding_raw,
            fed_cookie_max_age,
            app.config.server.cookie_secure,
        );
        let jar = jar
            .add(cookie)
            .add(fed_pending_cookie)
            .add(fed_csrf_cookie)
            .add(fed_browser_binding_cookie);
        let attributes = sui_id_store::registry::AuditAttributes::builder()
            .attribute("provider_id", provider_id.to_string())
            .build();
        let _ = sui_id_store::repos::audit::emit_must_attempt(
            &app.db,
            &sui_id_store::repos::federation_mfa_pending::AUDIT_MFA_REQUIRED_DESCRIPTOR,
            now,
            Some(user_id),
            Some(sui_id_store::registry::AuditTarget(user_id.to_string())),
            "ok",
            attributes,
        )
        .await;
        return Ok(terminal(jar, Redirect::to("/admin/login/mfa")));
    }

    // No MFA — create session directly.
    let session_row = SessionRow {
        id: SessionId::new(),
        user_id,
        expires_at: now + chrono::Duration::hours(24),
        created_at: now,
        revoked_at: None,
        auth_methods: vec![sui_id_shared::AuthMethod::Fed],
        last_step_up_at: None,
        last_used_at: None,
    };
    // RFC 102 L04 (RFC 094 `:548`, amended 2026-10-15 -- F01 folded in):
    // the attempt's own `exchanging -> completed` transition, the
    // enabled-provider/active-link/user authority recheck, `last_login_at`,
    // the session, eviction, and `auth.federation.signin.success` all
    // commit together, atomically. A failure is never counted (A9) and
    // gets the uniform redirect; the cause is logged.
    if let Err(e) = sui_id_store::commands::sign_in_federated(
        &app.db,
        attempt_id,
        provider_id,
        provider_slug.to_owned(),
        session_row.clone(),
    )
    .await
    {
        if matches!(e, sui_id_store::StoreError::NotFound) {
            tracing::warn!(
                provider = %provider_slug,
                user_id = %user_id,
                "federation: user no longer active at commit; sign-in refused"
            );
        } else {
            tracing::error!(
                provider = %provider_slug,
                user_id = %user_id,
                error = %e,
                detail = ?e,
                "federation: sign-in transaction failed; sign-in refused"
            );
        }
        terminalize_and_audit(
            &app.db,
            attempt_id,
            TerminalFailureReason::Denied(DeniedDetail::SigninCommitFailed),
            now,
            provider_id,
            Some(user_id),
        )
        .await;
        return signin_failed(jar);
    }

    // Metrics: record as a successful federated sign-in.
    if let Some(m) = app.metric() {
        m.signin(sui_id_store::metrics::signin_result::SUCCESS);
    }

    let cookie = session_cookie(session_row.id.to_string(), app.config.server.cookie_secure);
    let jar = jar.add(cookie);
    Ok(terminal(jar, Redirect::to("/admin")))
}

// ── RFC 096-B1 stage 6e: the `fed_error` surfacing map ───────────────────────
//
// `:714-716`'s rule applies to rendered output at least as strongly as to
// audit: no raw email, `sub`, ID token or upstream error text reaches the
// login page, and no distinction this file's own handlers do not already
// make in the open is reintroduced here either. `fed_error` is caller-
// supplied as far as the login page is concerned -- anyone can visit
// `/admin/login?fed_error=anything` -- so this map, not the raw value, is
// what reaches the template, and an unrecognised value is never
// distinguished from a known one.

/// Every distinct `fed_error` value this module's handlers can produce, as
/// measured directly against this file (not assumed): 26 values, 19 of
/// them collapsing into [`Self::Retry`] below. `claim_fed_error`'s own six
/// named `StoreError` arms plus its wildcard account for seven of the
/// nineteen; the rest are this file's static `Redirect::to(...)` literals.
///
/// RFC 096-B1 stage 12a recounted this from the 28/21 the comment
/// previously stated: `userinfo`, `no_id_token`, and `no_sub` are gone with
/// the legacy identity-mapping block that produced them. Stage 12a also
/// wrote `token_endpoint_auth_method`'s check and backed it out -- fixing
/// *why* it refused everything (`:320`'s profile, not a test-fixture gap)
/// is stage 12c's own job, which is what makes `token_endpoint_auth_method`
/// a producible value for the first time. `attempt_not_found`, `nonce_
/// mismatch`, `state_mismatch`, and `token_parse` all still occur, each
/// from exactly one producer since stage 12a's own removals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FedErrorBucket {
    /// Every transport, token, signature, claims, or validation failure
    /// during the federation handshake, plus an unrecognised value:
    /// `upstream`, `state_mismatch`, `attempt_not_found`, `replay`,
    /// `expired`, `clock_regression`, `tamper_detected`, `superseded`,
    /// `binding_mismatch`, `attempt_claim_failed`, `token_exchange`,
    /// `token_parse`, `missing_id_token`, `signature_invalid`,
    /// `nonce_mismatch`, `identity_claims_invalid`, `time_claims_invalid`,
    /// `optional_claims_invalid`, `token_endpoint_auth_method`, and
    /// anything not in the other three buckets. None of these distinguish
    /// one local account from another -- each is about this one request's
    /// own protocol state, not about whether some other upstream identity
    /// would have succeeded -- so lumping them together costs nothing and
    /// keeps the message set small.
    Retry,
    /// A provider- or account-level issue only an administrator can act
    /// on: `disabled` (provider disabled), `config_drift` (startup config
    /// missing its DB-enabled provider), `discovery_origin` (the
    /// provider's own discovery document names a disallowed origin), and
    /// `signin_failed` (RFC 102 L04's own uniform post-identification
    /// failure -- disabled/deleted user, a DB error, or a commit
    /// failure, already indistinguishable from each other upstream of
    /// this map).
    ContactAdmin,
    /// RFC 096 `:699-701`/`:707-708`'s own pairing, carried into the
    /// message: `email_collision` (P2, a local account exists and was not
    /// linked) and `link_required` (`link_only`, no local account was
    /// even consulted) must render identically, or the message itself
    /// becomes the oracle stage 5's generic *decision* was built to
    /// close. This is the one bucket whose membership is the security-
    /// relevant fact about this map, not an editorial grouping.
    LinkRequired,
    /// `unverified_email` (P3) alone: the one value whose remedy the end
    /// user can act on without an administrator, by verifying their
    /// address with the upstream identity provider rather than with
    /// sui-id.
    UnverifiedEmail,
}

impl FedErrorBucket {
    fn classify(fed_error: &str) -> Self {
        match fed_error {
            "email_collision" | "link_required" => Self::LinkRequired,
            "unverified_email" => Self::UnverifiedEmail,
            "disabled" | "config_drift" | "discovery_origin" | "signin_failed" => {
                Self::ContactAdmin
            }
            _ => Self::Retry,
        }
    }
}

/// The one bounded map `/admin/login`'s own `GET` handler calls: a
/// `fed_error` value in, one of exactly four localized [`Flash`](sui_id_web::Flash)es
/// out. The raw value itself never reaches the returned `Flash` -- only a
/// `&'static str` selected by [`FedErrorBucket::classify`], so there is
/// nothing here for a caller-supplied value to be reflected into.
pub fn fed_error_message(fed_error: &str, t: &'static sui_id_i18n::Strings) -> sui_id_web::Flash {
    let text = match FedErrorBucket::classify(fed_error) {
        FedErrorBucket::Retry => t.login_fed_retry,
        FedErrorBucket::ContactAdmin => t.login_fed_contact_admin,
        FedErrorBucket::LinkRequired => t.login_fed_link_required,
        FedErrorBucket::UnverifiedEmail => t.login_fed_unverified_email,
    };
    sui_id_web::Flash {
        kind: sui_id_web::FlashKind::Error,
        text: text.to_owned(),
    }
}

/// Fire-and-forget audit append (for paths where we can't await).
fn emit_audit_soon(
    db: sui_id_store::Database,
    at: chrono::DateTime<chrono::Utc>,
    action: &'static str,
    note: Option<String>,
) -> tokio::task::JoinHandle<()> {
    let row = AuditLogRow {
        at,
        actor: None,
        action: action.into(),
        target: None,
        result: "fail".into(),
        note,
    };
    tokio::spawn(async move {
        let _ = sui_id_store::repos::audit::append(&db, &row).await;
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "federation/tests.rs"]
mod tests;
