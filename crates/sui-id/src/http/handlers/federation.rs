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
use chrono::Duration;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::errors::HttpError;
use crate::federation_identity::{derive_username, resolve_shadow_username};
use crate::federation_state::{FedState, STATE_COOKIE, STATE_TTL_SECS, seal_state, unseal_state};
use crate::handlers::{AppState, AppStateExt, session_cookie};
use crate::id_token::{IdTokenClaims, decode_id_token_claims};
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::{FederationProviderId, SessionId, UserId};
use sui_id_store::models::{
    AuditLogRow, FederationLinkRow, FederationProviderRow, ProvisionMode, Role, SessionRow,
    UserRow, UserSource,
};

// ── RFC 096-B1 stage 4: 096-A's validators, their first production callers ──

use crate::cache_freshness::{ActivationGeneration, CacheKey, ProviderVersion};
use crate::federation_attempt_terminalize::{
    DeniedDetail, TerminalFailureReason, UpstreamFailureDetail, terminalize_and_audit,
};
use crate::id_token::verify_id_token;
use crate::identity_capability::construct_identity_capability;
use crate::identity_claims::validate_identity_claims;
use crate::identity_resolution::resolve_verified_identity;
use crate::nonce_claim::validate_nonce;
use crate::optional_claims::validate_optional_claims;
use crate::time_claims::validate_iat;
use sui_id_store::StoreError;
use sui_id_store::repos::federation_login_attempt;

// ── Upstream discovery ────────────────────────────────────────────────────────

use crate::discovery::{DiscoveryError, RawDiscovery, ValidatedDiscovery};

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
    // here), media type, a byte cap honored on bytes actually read, and
    // member/array/string caps, all before this ever reaches `RawDiscovery`.
    // Supersedes the old bare `is_success()` check (2xx was looser than
    // intended; a discovery document on 201/204 was never a real case).
    let raw = crate::response_bounds::read_bounded_json::<RawDiscovery>(resp)
        .await
        .map_err(|e| FetchDiscoveryError::Network(format!("discovery parse failed: {e}")))?;
    ValidatedDiscovery::validate(raw, issuer, allowed_origins).map_err(FetchDiscoveryError::Invalid)
}

// ── RFC 096-B1 stage 4: shared helpers for the attempt row ──────────────────

/// RFC 096 `:564-566`: the browser-held opaque binding cookie, scoped the
/// same way `STATE_COOKIE` is.
const BROWSER_BINDING_COOKIE: &str = "sui_id_fed_browser_binding";

/// RFC 096-B1 stage 9, F02: the `federation_mfa_pending` row's own id,
/// so a future consumer (stage 10/11) can find which row belongs to this
/// browser without a session already established. Scoped site-wide
/// (`/`), not to `/auth/federated`: unlike `STATE_COOKIE`/
/// `BROWSER_BINDING_COOKIE`, which only the callback itself ever reads,
/// this cookie's eventual reader is `/admin/login/mfa` (stage 12).
const FED_MFA_PENDING_COOKIE: &str = "sui_id_fed_mfa_pending";

/// RFC 096-B1 stage 9, F02: the raw CSRF-binding value, CSPRNG-minted the
/// same way `browser_binding`/`nonce`/`state_param` already are (`:564`'s
/// own precedent) — never the browser-binding value itself, a *second*,
/// independent opaque value, so a party holding one binding cannot drive
/// the other. Only `csrf_binding_sha256` (the digest) is stored in the
/// row; this is what the digest is a hash of.
const FED_MFA_CSRF_COOKIE: &str = "sui_id_fed_mfa_csrf";

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
const FED_MFA_BROWSER_BINDING_COOKIE: &str = "sui_id_fed_mfa_browser_binding";

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

    let fed_state = FedState {
        nonce: nonce.clone(),
        pkce_verifier,
        provider_slug: slug.clone(),
        expires_at: (chrono::Utc::now() + Duration::seconds(STATE_TTL_SECS)).timestamp(),
        next: if q.next.starts_with('/') {
            Some(q.next)
        } else {
            None
        },
        upstream_state: state_param.clone(),
    };
    let sealed = seal_state(&app, &fed_state).map_err(|_| HttpError::html(CoreError::Internal))?;

    // RFC 096-B1 stage 4: the durable attempt row the callback claims and
    // validates against. Created here, alongside the legacy sealed-state
    // cookie above, not in place of it -- "do not remove the old paths"
    // (this stage's own scope boundary; stage 7 removes it). Both carry
    // the *same* `nonce`/`state_param`/`pkce_verifier` values generated
    // above: one real OAuth request goes out below, so both records of it
    // must agree on what was actually sent.
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
        fed_state.pkce_verifier.as_bytes(),
        redirect_uri.clone(),
        fed_state.next.clone(),
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
    // callback -- the same scoping as the state cookie below.
    let browser_binding_cookie = {
        let mut c = Cookie::new(BROWSER_BINDING_COOKIE, browser_binding);
        c.set_http_only(true);
        c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
        c.set_max_age(time::Duration::seconds(STATE_TTL_SECS));
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

    // Store sealed state in a short-lived HttpOnly cookie (P5).
    let state_cookie = {
        let mut c = Cookie::new(STATE_COOKIE, sealed);
        c.set_http_only(true);
        c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
        c.set_max_age(time::Duration::seconds(STATE_TTL_SECS));
        c.set_path("/auth/federated");
        if app.config.server.cookie_secure {
            c.set_secure(true);
        }
        c
    };

    Ok((
        jar.add(state_cookie).add(browser_binding_cookie),
        Redirect::to(&upstream_url),
    )
        .into_response())
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
    // RFC 096 `:593-595`: held only in a zeroize-on-drop value until the
    // response is discarded; never persisted (P6). A `refresh_token`, if
    // the upstream returns one despite the request not asking for it, is
    // not a field of this struct at all -- `serde`'s default struct
    // deserialization never retains a JSON member it has no field for, so
    // it is dropped at parse time, not read into memory as a named value
    // to then have to remember to ignore.
    access_token: SecretString,
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
    // allowlisted, which is also why it is dropped here before `fed_state`
    // is even unsealed and no provider identity is available to log
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

    // Validate and consume the state cookie (P5).
    let sealed = jar
        .get(STATE_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("missing state cookie".into())))?;

    let fed_state = unseal_state(&app, &sealed)
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("invalid or expired state".into())))?;

    // P5 CSRF: verify the upstream echoes back the same state we sent.
    let returned_state = q.state.as_deref().unwrap_or("");
    use subtle::ConstantTimeEq;
    let state_ok: bool = fed_state
        .upstream_state
        .as_bytes()
        .ct_eq(returned_state.as_bytes())
        .into();
    if !state_ok {
        tracing::warn!(slug = %fed_state.provider_slug, "federation: state mismatch — possible CSRF");
        return Ok(terminal(
            jar,
            Redirect::to("/admin/login?fed_error=state_mismatch"),
        ));
    }

    // Load the provider.
    let provider =
        sui_id_store::repos::federation_provider::get_by_slug(&app.db, &fed_state.provider_slug)
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
    // State digest: by `find_by_state_sha256` below, then claiming by id --
    // kept in this two-step shape (stage 4's own), not folded into one
    // statement, since a wrong state can never find a row to begin with.
    // Provider id/version/generation and the browser binding are the two
    // elements `:597-598` added to the match set this stage; pending state,
    // unexpired time and clock-regression were already checked by `claim`
    // (stage 3) and are unchanged here, only moved earlier.
    let state_hash = sha256_32(fed_state.upstream_state.as_bytes());
    let pre_claim_attempt = match federation_login_attempt::find_by_state_sha256(
        &app.db, state_hash,
    )
    .await
    {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(slug = %provider.slug, error = %e, "federation: no attempt row for this state");
            return Ok(terminal(
                jar,
                Redirect::to("/admin/login?fed_error=attempt_not_found"),
            ));
        }
    };

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

    // Exchange the code for tokens at the upstream token_endpoint.
    let redirect_uri = format!(
        "{}/auth/federated/callback",
        app.config.server.issuer.trim_end_matches('/')
    );
    let mut form_params = vec![
        ("grant_type", "authorization_code".to_owned()),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", provider.client_id.clone()),
        ("code_verifier", fed_state.pkce_verifier.clone()),
    ];
    if let Some(ref secret) = client_secret {
        form_params.push(("client_secret", secret.clone()));
    }

    let token_resp = app
        .http_client
        .post(discovery.token_endpoint())
        .form(&form_params)
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

    // ── RFC 096-B1 stage 4: the attempt claim and 096-A's validators ─────────
    //
    // Everything below runs *before* the legacy trust-on-TLS decode a few
    // lines down, and fails the request closed on any error -- this is
    // "096-A stops being dormant" (the dispatch's own title for this
    // stage): a forged signature, wrong issuer, stale `iat`, replayed or
    // mismatched nonce, or a superseded attempt is refused *here*, by the
    // validators that have had zero production callers until now, not by
    // the old code a few lines below (which still runs afterward, on the
    // *same* token, redundantly and unchanged -- "do not remove the old
    // paths," stage 7's job). A success here changes nothing about what
    // the old code does with its own, separately-decoded claims: "no
    // identity mapping and no session... hold [the capability], and let
    // stage 5 consume it."
    //
    // RFC 096's own requirement (`:593-594`): the ID token is mandatory.
    // The pre-existing userinfo-fallback branch a few lines down, for a
    // response with no `id_token` at all, is unreachable once this refuses
    // first -- not deleted (same "do not remove" reasoning), but dead in
    // practice from this stage on.
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
    let _resolution = match resolve_verified_identity(
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

    // ── End of stage 4/5's new validation and resolution. The legacy path
    // below is unchanged (except RFC 096 `:700-701`'s removal of the
    // pending-link cookie and skeleton, below) and still authoritative for
    // identity mapping and session establishment (stage 6) -- it re-
    // decodes the same token itself, without signature verification,
    // exactly as it always has. ───────────────────────────────────────────

    // Decode the ID token claims (light validation — nonce check + sub extraction).
    // We trust the token_endpoint over TLS; full signature verification would
    // require fetching the upstream JWKS — out of scope for Step 1.
    let id_claims: IdTokenClaims = match tokens.id_token.as_deref() {
        Some(jwt) => match decode_id_token_claims(jwt) {
            Some(claims) => claims,
            None => {
                // Dead in practice (module doc, above): the mandatory
                // id_token check earlier already refused a response this
                // legacy decode could fail to parse. Still terminalized,
                // for defense in depth rather than left exchanging.
                tracing::warn!("federation ID token claims parse failed");
                terminalize_and_audit(
                    &app.db,
                    claimed.id,
                    TerminalFailureReason::Denied(DeniedDetail::LegacyTokenParseFailed),
                    validation_now,
                    provider.id,
                    None,
                )
                .await;
                return Ok(terminal(
                    jar,
                    Redirect::to("/admin/login?fed_error=token_parse"),
                ));
            }
        },
        None => {
            // Dead in practice, same reason: no_id_token's own branch
            // above already refuses a response with no id_token.
            // No id_token: fall back to userinfo endpoint if available.
            if let Some(ui_url) = discovery.userinfo_endpoint() {
                match fetch_userinfo(
                    &app.http_client,
                    ui_url,
                    tokens.access_token.expose_secret(),
                )
                .await
                {
                    Ok(claims) => claims,
                    Err(e) => {
                        tracing::warn!(error = %e, "userinfo fetch failed");
                        terminalize_and_audit(
                            &app.db,
                            claimed.id,
                            TerminalFailureReason::UpstreamFailure(
                                UpstreamFailureDetail::UserinfoFetchFailed,
                            ),
                            validation_now,
                            provider.id,
                            None,
                        )
                        .await;
                        return Ok(terminal(
                            jar,
                            Redirect::to("/admin/login?fed_error=userinfo"),
                        ));
                    }
                }
            } else {
                terminalize_and_audit(
                    &app.db,
                    claimed.id,
                    TerminalFailureReason::Denied(DeniedDetail::LegacyNoIdToken),
                    validation_now,
                    provider.id,
                    None,
                )
                .await;
                return Ok(terminal(
                    jar,
                    Redirect::to("/admin/login?fed_error=no_id_token"),
                ));
            }
        }
    };

    // Nonce check (P5 replay protection). Dead in practice, same reason:
    // the new, split nonce comparison (item 1, above) already refuses a
    // mismatch before this point is ever reached.
    if let Some(ref token_nonce) = id_claims.nonce
        && token_nonce != &fed_state.nonce
    {
        tracing::warn!(slug = %provider.slug, "federation nonce mismatch");
        terminalize_and_audit(
            &app.db,
            claimed.id,
            TerminalFailureReason::Denied(DeniedDetail::LegacyNonceMismatch),
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

    // Still reachable: nothing above validates that `sub` is non-empty.
    if id_claims.sub.is_empty() {
        terminalize_and_audit(
            &app.db,
            claimed.id,
            TerminalFailureReason::Denied(DeniedDetail::EmptySub),
            validation_now,
            provider.id,
            None,
        )
        .await;
        return Ok(terminal(jar, Redirect::to("/admin/login?fed_error=no_sub")));
    }

    // P6: upstream access token is used above and then dropped — never persisted.

    // Resolve the federation link by (provider_id, upstream_sub) — P1.
    let now = app.clock.now();
    let link_opt = match sui_id_store::repos::federation_link::find_by_sub(
        &app.db,
        provider.id,
        &id_claims.sub,
    )
    .await
    {
        Ok(l) => l,
        Err(e) => {
            terminalize_and_audit(
                &app.db,
                claimed.id,
                TerminalFailureReason::Denied(DeniedDetail::LinkLookupDbError),
                now,
                provider.id,
                None,
            )
            .await;
            return Err(HttpError::html(CoreError::from(e)));
        }
    };

    let user_id: UserId = match link_opt {
        // ── Known user: update last_seen and proceed to MFA gate ─────────────
        Some(existing_link) => {
            if let Err(e) = sui_id_store::repos::federation_link::upsert(
                &app.db,
                FederationLinkRow {
                    user_id: existing_link.user_id,
                    provider_id: provider.id,
                    upstream_sub: id_claims.sub.clone(),
                    upstream_email: id_claims.email.clone(),
                    linked_at: existing_link.linked_at,
                    last_seen_at: now,
                },
            )
            .await
            {
                terminalize_and_audit(
                    &app.db,
                    claimed.id,
                    TerminalFailureReason::Denied(DeniedDetail::LinkUpsertDbError),
                    now,
                    provider.id,
                    Some(existing_link.user_id),
                )
                .await;
                return Err(HttpError::html(CoreError::from(e)));
            }
            existing_link.user_id
        }

        // ── Unknown: provision or link-only ─────────────────────────────────
        None => {
            match provider.provision_mode {
                ProvisionMode::ProvisionOnFirstLogin => {
                    // P3: RFC 096 `:727-728`, "`provision_on_first_login`
                    // requires a present email and `email_verified=true`"
                    // -- both conditions, not only the unverified
                    // one. Before RFC 096-B1 stage 7 this checked
                    // `email.is_some() && !email_verified`, which let an
                    // absent email fall through to provisioning: that was
                    // legacy behaviour stage 6c's own fix round deliberately
                    // left alone ("that resolution is not yet authoritative
                    // ... a check-order fix is not where to change it"),
                    // and stage 7 -- F04's own stage -- is where it changes.
                    // `docs/src/guides/operators.md`'s "requires
                    // `email_verified = true` ... otherwise the sign-in is
                    // rejected" becomes true for the first time with this
                    // change; confirmed, not merely asserted, in this
                    // stage's own review package.
                    //
                    // RFC 096-B1 stage 6e fix round (R1): this check still
                    // runs BEFORE P2's collision check below, not after.
                    // RFC 096 `:727-728` requires `email_verified = true`
                    // before an email is even eligible for the collision
                    // comparison ("if any local user already has *that
                    // normalized email*" -- the verified one): the
                    // verification requirement precedes the collision rule
                    // in the RFC's own sequence, and the code must too.
                    // Checking collision before verification let an
                    // attacker who can only present an address, never
                    // verify one they don't own, learn whether an
                    // *unverified* copy of a victim's email collides with
                    // a local account -- `email_collision` vs
                    // `unverified_email` differ by exactly that, which is
                    // the same local-account-enumeration oracle stage 5
                    // closed, reached by a different route. Running this
                    // check first means no unverified or absent email ever
                    // reaches the collision comparison: every one of them
                    // gets `unverified_email`, full stop, carrying no
                    // information about whether any local account matches
                    // it.
                    if id_claims.email.is_none() || !id_claims.email_verified {
                        tracing::info!(slug = %provider.slug, "provision held: absent or unverified email");
                        terminalize_and_audit(
                            &app.db,
                            claimed.id,
                            TerminalFailureReason::Denied(DeniedDetail::UnverifiedEmail),
                            now,
                            provider.id,
                            None,
                        )
                        .await;
                        return Ok(terminal(
                            jar,
                            Redirect::to("/admin/login?fed_error=unverified_email"),
                        ));
                    }

                    // P2. RFC 096 `:707-708`: collision denial is specific
                    // to `provision_on_first_login` -- under `link_only`
                    // nothing is created and nothing is linked, so there is
                    // no takeover to block. It lives *inside* this arm, and
                    // must stay inside it: `:699` requires `link_only` to
                    // return one generic result for every unlinked upstream
                    // identity, and any read of local state on that path
                    // is a way for the result to stop being generic. Inside
                    // the arm rather than guarded in place, so a reader of
                    // the `LinkOnly` arm can see directly that nothing
                    // about local accounts is consulted there at all.
                    //
                    // Reached only with a present, verified email: P3 above
                    // now denies both the absent and the unverified case
                    // (RFC 096-B1 stage 7), so `id_claims.email` is always
                    // `Some` here and `find_by_email_normalized` is never
                    // called on an email the upstream has not vouched for.
                    // By the time this runs, the only email this comparison can
                    // see is one the upstream has already vouched for.
                    if let Some(ref email) = id_claims.email
                        && let Ok(Some(_collision)) =
                            sui_id_store::repos::users::find_by_email_normalized(
                                &app.db,
                                &sui_id_shared::normalize_email(email),
                            )
                            .await
                    {
                        // An existing local user has this email but is NOT
                        // linked to this provider. Treat as attempted
                        // takeover. RFC 096 `:714-715` (review R2): no raw
                        // email in the log field or the audit note --
                        // internal `provider_id` and a bounded reason token
                        // only.
                        //
                        // RFC 096-B1 stage 6b built this as the one
                        // converted Class-B site, calling `emit_must_
                        // attempt` directly; stage 6c's F05 subsumes that
                        // emission rather than sitting beside it -- the
                        // event (`takeover_blocked`) and its one-line
                        // reason token are unchanged, but the call now also
                        // terminalizes the attempt, which the 6b version
                        // never did. `terminalize_and_audit`'s own module
                        // doc restates why this `let _`-equivalent discard
                        // (its return is `()`, nothing to discard) is still
                        // correct here: A6 (RFC 094 `:120`) requires the
                        // denial below to happen regardless of whether
                        // either internal step succeeded.
                        tracing::warn!(
                            provider_id = %provider.id,
                            "federation: email collision — potential takeover attempt blocked (P2)"
                        );
                        terminalize_and_audit(
                            &app.db,
                            claimed.id,
                            TerminalFailureReason::TakeoverBlocked,
                            now,
                            provider.id,
                            None,
                        )
                        .await;
                        return Ok(terminal(
                            jar,
                            Redirect::to("/admin/login?fed_error=email_collision"),
                        ));
                    }
                    // P7: derive username, never trust upstream directly.
                    let proposed = derive_username(&id_claims);
                    let username = resolve_shadow_username(&app.db, &proposed).await;

                    // RFC 096-B1 stage 7, F04: attempt completion,
                    // passwordless non-admin user, link, `[Fed]` session/
                    // cap, and the `auth.federation.provisioned` commit,
                    // all in one Class-A transaction (RFC 096 `:706-711`).
                    // The id is generated here, before the transaction,
                    // and used identically on the user, link and session
                    // rows -- the three rows the transaction creates must
                    // already agree on who they are about.
                    let uid = UserId::new();
                    let user = UserRow {
                        id: uid,
                        username,
                        display_name: id_claims.name.clone(),
                        is_admin: false,
                        role: Role::User,
                        is_disabled: false,
                        is_deleted: false,
                        user_uuid: uuid::Uuid::new_v4(),
                        created_at: now,
                        updated_at: now,
                        failed_login_count: 0,
                        locked_until: None,
                        email: id_claims.email.clone(),
                        preferred_lang: None,
                        email_normalized: None,
                        email_verified_at: None,
                        last_login_at: None,
                        // This stage's own decision (review package §2):
                        // `source` stays the plain default, not
                        // `UserSource::Ldap` the legacy writer used.
                        // `sui_id_core::authn::step_up::first_factor_
                        // proof` -- the one place a federated user's
                        // `source` could matter -- already checks
                        // `federation_link` existence first (this row's
                        // own link, created below, in the same
                        // transaction), so nothing reads `source` to
                        // decide federation status. `forgot_password`'s
                        // `source == Local` filter and
                        // `try_login_with_cascade`'s `source == Ldap`
                        // directory-bind routing both stay correct for a
                        // `Local`-tagged federated user too: it has no
                        // `credentials` row, so RFC 115 D8's existing
                        // passwordless-local-account refusal already
                        // handles it safely either way.
                        source: UserSource::Local,
                        external_stable_id: None,
                    };
                    let link = FederationLinkRow {
                        user_id: uid,
                        provider_id: provider.id,
                        upstream_sub: id_claims.sub.clone(),
                        upstream_email: id_claims.email.clone(),
                        linked_at: now,
                        last_seen_at: now,
                    };
                    let session_row = SessionRow {
                        id: SessionId::new(),
                        user_id: uid,
                        expires_at: now + chrono::Duration::hours(24),
                        created_at: now,
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
                            let cookie = session_cookie(
                                session_id.to_string(),
                                app.config.server.cookie_secure,
                            );
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
                                TerminalFailureReason::Denied(
                                    DeniedDetail::ProvisionTransactionFailed,
                                ),
                                now,
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

                ProvisionMode::LinkOnly => {
                    // RFC 096-B1 stage 5, RFC 096 `:699-701`: one generic
                    // "local account link required" result for every
                    // unlinked upstream identity under `link_only` --
                    // generic is load-bearing, since it must not reveal
                    // whether the upstream identity is known locally
                    // (`identity_resolution::IdentityResolution::
                    // LinkRequired` is unit-tested to be the same state
                    // whether or not the email collides, and
                    // `tests/e2e/r096_b1_stage5.rs` pins the same property
                    // on this live path, which is where it counts). The
                    // unsigned
                    // `sui_id_fed_pending` cookie and the incomplete
                    // `/auth/federated/link` confirmation skeleton this
                    // branch used to redirect to are removed, not reused
                    // (`:700-701`) -- "M4 does not create a link." A later
                    // design may restore self-service linking with fresh
                    // local authentication, local MFA, CSRF, a single-use
                    // durable intent, explicit approval, and C19 atomicity.
                    //
                    // RFC 096-B1 stage 6c, F05: `link_required`'s own
                    // registry-declared descriptor. No email/collision
                    // detail in the `reason` token either, matching the
                    // generic-result requirement above: this call site is
                    // one of the things that must not distinguish a
                    // collision from any other unlinked identity.
                    terminalize_and_audit(
                        &app.db,
                        claimed.id,
                        TerminalFailureReason::LinkRequired,
                        now,
                        provider.id,
                        None,
                    )
                    .await;
                    return Ok(terminal(
                        jar,
                        Redirect::to("/admin/login?fed_error=link_required"),
                    ));
                }
            }
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
        now,
    )
    .await
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Complete a federated sign-in: enforce local MFA then issue a session.
///
/// `attempt_id`/`provider_id` (RFC 096-B1 stage 6c) are for F05 alone: a
/// successful sign-in or a pending-MFA continuation does *not* terminalize
/// the attempt here -- that is F01's/F02's own `exchanging -> completed`
/// transition (stage 6d, not yet implemented), and this function is not
/// where that boundary is decided. Only this function's own *failure*
/// paths call `terminalize_and_audit`.
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
        let fed_pending_id = match sui_id_store::commands::issue_federated_mfa_pending(
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
            Ok(id) => id,
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
        let fed_pending_cookie = {
            let mut c = Cookie::new(FED_MFA_PENDING_COOKIE, fed_pending_id.to_string());
            c.set_http_only(true);
            c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
            c.set_max_age(time::Duration::seconds(
                sui_id_store::repos::federation_mfa_pending::PENDING_LIFETIME_SECS,
            ));
            c.set_path("/");
            if app.config.server.cookie_secure {
                c.set_secure(true);
            }
            c
        };
        let fed_csrf_cookie = {
            let mut c = Cookie::new(FED_MFA_CSRF_COOKIE, csrf_raw);
            c.set_http_only(true);
            c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
            c.set_max_age(time::Duration::seconds(
                sui_id_store::repos::federation_mfa_pending::PENDING_LIFETIME_SECS,
            ));
            c.set_path("/");
            if app.config.server.cookie_secure {
                c.set_secure(true);
            }
            c
        };
        let fed_browser_binding_cookie = {
            let mut c = Cookie::new(FED_MFA_BROWSER_BINDING_COOKIE, fed_browser_binding_raw);
            c.set_http_only(true);
            c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
            c.set_max_age(time::Duration::seconds(
                sui_id_store::repos::federation_mfa_pending::PENDING_LIFETIME_SECS,
            ));
            c.set_path("/");
            if app.config.server.cookie_secure {
                c.set_secure(true);
            }
            c
        };
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
    // RFC 102 L04: `last_login_at`, the session, eviction and
    // `auth.federation.signin.success` commit together, after an
    // in-transaction re-read of the user. A failure is never counted (A9)
    // and gets the uniform redirect; the cause is logged.
    if let Err(e) = sui_id_store::commands::sign_in_federated(
        &app.db,
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

async fn fetch_userinfo(
    client: &reqwest::Client,
    url: &str,
    access_token: &str,
) -> Result<IdTokenClaims, String> {
    let resp = client
        .get(url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    // RFC 134 D5 Tier 1: the same bounded pipeline as discovery and the
    // token response. This site previously had no status check at all
    // before deserializing -- `read_bounded_json`'s 200-only requirement
    // closes that, not only adds the size/shape caps.
    crate::response_bounds::read_bounded_json(resp)
        .await
        .map_err(|e| e.to_string())
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
/// measured directly against this file (not assumed): 28 values, 21 of
/// them collapsing into [`Self::Retry`] below. `claim_fed_error`'s own six
/// named `StoreError` arms plus its wildcard account for seven of the
/// twenty-one; the rest are this file's static `Redirect::to(...)`
/// literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FedErrorBucket {
    /// Every transport, token, signature, claims, or validation failure
    /// during the federation handshake, plus an unrecognised value:
    /// `upstream`, `state_mismatch`, `attempt_not_found`, `replay`,
    /// `expired`, `clock_regression`, `tamper_detected`, `superseded`,
    /// `binding_mismatch`, `attempt_claim_failed`, `token_exchange`,
    /// `token_parse`, `missing_id_token`, `signature_invalid`,
    /// `nonce_mismatch`, `identity_claims_invalid`, `time_claims_invalid`,
    /// `optional_claims_invalid`, `userinfo`, `no_id_token`, `no_sub`, and
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
#[path = "federation/tests.rs"]
mod tests;
