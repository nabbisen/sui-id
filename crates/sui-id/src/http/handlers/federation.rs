//! `GET /auth/federated/{slug}/start`  — redirect to upstream IdP
//! `GET /auth/federated/callback`       — exchange code, resolve link
//! `GET|POST /auth/federated/link`      — link-only approval flow
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
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::models::{
    AuditLogRow, FederationLinkRow, FederationProviderRow, ProvisionMode, SessionRow,
};

// ── RFC 096-B1 stage 4: 096-A's validators, their first production callers ──

use crate::cache_freshness::{ActivationGeneration, CacheKey, ProviderVersion};
use crate::federation_attempt_claim::{ClaimAndNonceError, claim_and_consume_nonce};
use crate::id_token::verify_id_token;
use crate::identity_capability::construct_identity_capability;
use crate::identity_claims::validate_identity_claims;
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

/// RFC 096 `:584-585`: the browser-held opaque binding cookie, scoped the
/// same way `STATE_COOKIE` is.
const BROWSER_BINDING_COOKIE: &str = "sui_id_fed_browser_binding";

/// Raw SHA-256 digest, 32 bytes -- what `federation_login_attempt`'s three
/// hash columns store (RFC 096 `:587-588`: "raw fixed-size values", not hex
/// `TEXT`). `Sha256::digest`'s own output is already exactly 32 bytes, so
/// the `try_into` here cannot fail; it exists to produce a plain `[u8; 32]`
/// rather than carry the `GenericArray` type into every caller.
fn sha256_32(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
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
/// fixed at `(0, 0)`. That makes the superseded-attempt comparison this
/// stage wires (`federated_callback`, via [`attempt_is_superseded`])
/// trivially satisfied in production today — there is only one value in
/// play, so it can never disagree with itself — and, more than that,
/// *unreachable end-to-end by any test either*: the one column that could
/// be made to disagree with this constant is the row's own
/// `provider_config_version`/`provider_activation_generation`, and those
/// are AAD-bound by stage 2's `insert`/`claim` seal (`:583`'s own PKCE
/// binding) — tampering them after insert is caught by that *earlier*
/// tamper check (`StoreError::Crypto`), not by this comparison. Confirmed
/// empirically, not assumed: an end-to-end attempt to do exactly that
/// surfaced `fed_error=tamper_detected`, never `fed_error=superseded`.
///
/// That is why [`attempt_is_superseded`] is tested at the comparison level
/// (`tests::` below) rather than end-to-end — it is genuinely wired into
/// `federated_callback`, but what makes it *reachable* with real data is
/// M2b's own work, not this stage's. When M2b lands the real columns,
/// this is the one function that changes — most likely into two field
/// reads on `FederationProviderRow` — not every call site that compares
/// against it.
fn current_provider_version_and_generation(_provider: &FederationProviderRow) -> (i64, i64) {
    (0, 0)
}

/// RFC 096 `:580-581`: an attempt whose recorded provider trust generation
/// no longer matches "current" is superseded and must fail, not silently
/// validate against whatever the live config says now. Extracted from
/// `federated_callback` so the comparison itself has a test independent of
/// whether production data can reach a mismatch today (see
/// [`current_provider_version_and_generation`]'s doc comment for why it
/// cannot, yet).
fn attempt_is_superseded(
    claimed_version: i64,
    claimed_generation: i64,
    current_version: i64,
    current_generation: i64,
) -> bool {
    claimed_version != current_version || claimed_generation != current_generation
}

/// Maps a claim/nonce failure to the `fed_error` query value the callback
/// redirects with. Grouped by user-facing outcome, not by Rust variant:
/// every [`NonceError`] variant means "this token's nonce does not match
/// this attempt's," so all four collapse to the one value the *old*
/// nonce check already uses (`nonce_mismatch`) -- the new path enforces
/// the same user-facing condition by the real mechanism, not a different
/// one. `StoreError::Conflict` (not `pending`) really is a replay of an
/// already-claimed attempt, so it gets its own, more specific value
/// rather than sharing one of the others.
fn claim_and_nonce_fed_error(e: &ClaimAndNonceError) -> &'static str {
    match e {
        ClaimAndNonceError::Nonce(_) => "nonce_mismatch",
        ClaimAndNonceError::Claim(store_err) => match store_err {
            StoreError::Conflict => "replay",
            StoreError::AttemptExpired => "expired",
            StoreError::ClockRegression => "clock_regression",
            StoreError::Crypto => "tamper_detected",
            StoreError::NotFound => "attempt_not_found",
            _ => "attempt_claim_failed",
        },
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

    // Random nonce (P5 single-use replay protection). RFC 096 `:583-584`:
    // 32 bytes, like the other three independent CSPRNG values below --
    // bumped from the legacy 16 here (the one place this stage touches the
    // old cookie-state code, since it already generates the value the new
    // attempt row's `nonce_sha256` must hash).
    let nonce = sui_id_core::tokens::random_token(32);

    // Random state parameter for open-redirect guard (CSRF). Same RFC 096
    // `:583-584` bump as `nonce`, same reason.
    let state_param = sui_id_core::tokens::random_token(32);

    // RFC 096 `:583-584`: the fourth independent CSPRNG value, bound into
    // the sealed verifier's AAD by `federation_login_attempt::insert` via
    // the attempt row it creates below, and (RFC 096 `:584-585`) carried to
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
        current_provider_version_and_generation(&provider);
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

    // RFC 096 `:584-585`: the browser receives only the opaque binding, in
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
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    // Reject upstream errors.
    if let Some(err) = q.error {
        tracing::warn!(upstream_error = %err, "federation callback: upstream returned error");
        let jar = jar.remove(Cookie::build(STATE_COOKIE));
        return Ok((jar, Redirect::to("/admin/login?fed_error=upstream")).into_response());
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
        let jar = jar.remove(Cookie::build(STATE_COOKIE));
        return Ok((jar, Redirect::to("/admin/login?fed_error=state_mismatch")).into_response());
    }

    // Load the provider.
    let provider =
        sui_id_store::repos::federation_provider::get_by_slug(&app.db, &fed_state.provider_slug)
            .await
            .map_err(|_| HttpError::html(CoreError::NotFound))?;

    if !provider.enabled {
        return Ok(Redirect::to("/admin/login?fed_error=disabled").into_response());
    }

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
            // Audit upstream failure. emit_audit_soon already tokio::spawn()s
            // before returning; the JoinHandle is fire-and-forget by design.
            #[allow(clippy::let_underscore_future)]
            let _ = emit_audit_soon(
                app.db.clone(),
                app.clock.now(),
                sui_id_store::repos::federation_provider::AUDIT_SIGNIN_UPSTREAM_FAILURE,
                Some(format!("provider={} error={e}", provider.slug)),
            );
            // RFC 134 D3 §2c: a rejected endpoint redirects with the same
            // fed_error shape every other callback failure uses, not the
            // generic bad-request page the pre-existing network/parse
            // failure path used (unchanged below for that case, since it
            // predates this control and isn't what's dispatched here).
            if e.is_invalid() {
                return Ok(Redirect::to("/admin/login?fed_error=discovery_origin").into_response());
            }
            return Err(HttpError::html(CoreError::BadRequest(
                "upstream IdP unavailable".into(),
            )));
        }
    };

    // Decrypt client secret (P6 — used for token exchange only, not stored).
    let client_secret =
        sui_id_store::repos::federation_provider::decrypt_secret(app.db.key(), &provider)
            .map_err(|e| HttpError::html(CoreError::from(e)))?;

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
            // emit_audit_soon already tokio::spawn()s before returning; the
            // JoinHandle is fire-and-forget by design.
            #[allow(clippy::let_underscore_future)]
            let _ = emit_audit_soon(
                app.db.clone(),
                app.clock.now(),
                sui_id_store::repos::federation_provider::AUDIT_SIGNIN_UPSTREAM_FAILURE,
                Some(format!("provider={} error={e}", provider.slug)),
            );
            return Ok(Redirect::to("/admin/login?fed_error=token_exchange").into_response());
        }
    };

    // RFC 134 D5 Tier 1: the same bounded pipeline as discovery.
    let tokens: TokenResponse = match crate::response_bounds::read_bounded_json(token_resp).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "federation token parse failed");
            return Ok(Redirect::to("/admin/login?fed_error=token_parse").into_response());
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
            return Ok(Redirect::to("/admin/login?fed_error=missing_id_token").into_response());
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
        return Ok(Redirect::to("/admin/login?fed_error=config_drift").into_response());
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
            return Ok(Redirect::to("/admin/login?fed_error=signature_invalid").into_response());
        }
    };

    // The attempt row, found by the hash of the `state` value this exact
    // request already proved (above, by `ct_eq`) matches what `federated_
    // start` sent -- the same value `federated_start` hashed into
    // `state_sha256` when it created this row.
    let state_hash = sha256_32(fed_state.upstream_state.as_bytes());
    let attempt = match federation_login_attempt::find_by_state_sha256(&app.db, state_hash).await {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(slug = %provider.slug, error = %e, "federation: no attempt row for this state");
            return Ok(Redirect::to("/admin/login?fed_error=attempt_not_found").into_response());
        }
    };

    // One clock sample for everything below: the claim, `iat`'s lower/upper
    // bound, and the capability's `validated_at` -- not `app.clock.now()`
    // called three separate times.
    let validation_now = app.clock.now();

    let claimed = match claim_and_consume_nonce(
        &app.db,
        attempt.id,
        &verified_claims,
        validation_now,
    )
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!(slug = %provider.slug, error = ?e, "federation: attempt claim or nonce consumption failed");
            return Ok(Redirect::to(&format!(
                "/admin/login?fed_error={}",
                claim_and_nonce_fed_error(&e)
            ))
            .into_response());
        }
    };

    // RFC 096 `:580-581`: if the provider's trust config has moved on
    // since this attempt started, it is superseded and must fail -- not
    // silently validate against whatever the config says now. Re-reading
    // `provider.issuer`/`provider.client_id` below (live config) is only
    // safe because this check has just proven nothing has moved.
    let (current_version, current_generation) = current_provider_version_and_generation(&provider);
    if attempt_is_superseded(
        claimed.provider_config_version,
        claimed.provider_activation_generation,
        current_version,
        current_generation,
    ) {
        tracing::warn!(slug = %provider.slug, "federation: attempt superseded (provider config/activation moved on)");
        return Ok(Redirect::to("/admin/login?fed_error=superseded").into_response());
    }

    let identity = match validate_identity_claims(
        &verified_claims,
        &provider.issuer,
        &provider.client_id,
    ) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(slug = %provider.slug, error = ?e, "federation: identity claims rejected");
            return Ok(
                Redirect::to("/admin/login?fed_error=identity_claims_invalid").into_response(),
            );
        }
    };

    // RFC 096 `:676`: `iat`'s lower bound is the attempt's own `created_at`
    // -- the attempt-binding defence against a token minted long before
    // this attempt existed.
    if let Err(e) = validate_iat(&verified_claims, claimed.created_at, validation_now) {
        tracing::warn!(slug = %provider.slug, error = ?e, "federation: iat rejected");
        return Ok(Redirect::to("/admin/login?fed_error=time_claims_invalid").into_response());
    }

    let optional = match validate_optional_claims(&verified_claims) {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(slug = %provider.slug, error = ?e, "federation: optional claims rejected");
            return Ok(
                Redirect::to("/admin/login?fed_error=optional_claims_invalid").into_response(),
            );
        }
    };

    let _capability = construct_identity_capability(
        &identity,
        &optional,
        CacheKey::new(
            provider.id,
            ProviderVersion(claimed.provider_config_version as u64),
            ActivationGeneration(claimed.provider_activation_generation as u64),
        ),
        validation_now,
    );
    // "What it does with the capability is: nothing yet -- hold it, and
    // let stage 5 consume it." Constructed, proven well-formed by every
    // check above, and deliberately unused past this point.

    // ── End of stage 4's new validation. The legacy path below is
    // unchanged and still authoritative for identity mapping and session
    // establishment (stages 5/6) -- it re-decodes the same token itself,
    // without signature verification, exactly as it always has. ──────────

    // Decode the ID token claims (light validation — nonce check + sub extraction).
    // We trust the token_endpoint over TLS; full signature verification would
    // require fetching the upstream JWKS — out of scope for Step 1.
    let id_claims: IdTokenClaims = match tokens.id_token.as_deref() {
        Some(jwt) => match decode_id_token_claims(jwt) {
            Some(claims) => claims,
            None => {
                tracing::warn!("federation ID token claims parse failed");
                return Ok(Redirect::to("/admin/login?fed_error=token_parse").into_response());
            }
        },
        None => {
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
                        return Ok(Redirect::to("/admin/login?fed_error=userinfo").into_response());
                    }
                }
            } else {
                return Ok(Redirect::to("/admin/login?fed_error=no_id_token").into_response());
            }
        }
    };

    // Nonce check (P5 replay protection).
    if let Some(ref token_nonce) = id_claims.nonce
        && token_nonce != &fed_state.nonce
    {
        tracing::warn!(slug = %provider.slug, "federation nonce mismatch");
        return Ok(Redirect::to("/admin/login?fed_error=nonce_mismatch").into_response());
    }

    if id_claims.sub.is_empty() {
        return Ok(Redirect::to("/admin/login?fed_error=no_sub").into_response());
    }

    // P6: upstream access token is used above and then dropped — never persisted.

    // Resolve the federation link by (provider_id, upstream_sub) — P1.
    let now = app.clock.now();
    let link_opt =
        sui_id_store::repos::federation_link::find_by_sub(&app.db, provider.id, &id_claims.sub)
            .await
            .map_err(|e| HttpError::html(CoreError::from(e)))?;

    let user_id: UserId = match link_opt {
        // ── Known user: update last_seen and proceed to MFA gate ─────────────
        Some(existing_link) => {
            sui_id_store::repos::federation_link::upsert(
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
            .map_err(|e| HttpError::html(CoreError::from(e)))?;
            existing_link.user_id
        }

        // ── Unknown: provision or link-only ─────────────────────────────────
        None => {
            // P2: check for email collision with an existing user that has a
            // different provider link (attempted account takeover).
            if let Some(ref email) = id_claims.email
                && let Ok(Some(_collision)) = sui_id_store::repos::users::find_by_email_normalized(
                    &app.db,
                    &sui_id_shared::normalize_email(email),
                )
                .await
            {
                // An existing local user has this email but is NOT linked
                // to this provider. Treat as attempted takeover.
                tracing::warn!(
                    provider = %provider.slug,
                    email = %email,
                    "federation: email collision — potential takeover attempt blocked (P2)"
                );
                let _ = sui_id_store::repos::audit::append(
                    &app.db,
                    &AuditLogRow {
                        at: now,
                        actor: None,
                        action: sui_id_store::repos::federation_provider::AUDIT_TAKEOVER_BLOCKED
                            .into(),
                        target: None,
                        result: "denied".into(),
                        note: Some(format!("provider={} email={email}", provider.slug)),
                    },
                )
                .await;
                return Ok(Redirect::to("/admin/login?fed_error=email_collision").into_response());
            }

            match provider.provision_mode {
                ProvisionMode::ProvisionOnFirstLogin => {
                    // P3: provision on first login requires either:
                    //   - email present AND email_verified = true, OR
                    //   - email entirely absent (no email claim → no email
                    //     verification requirement, no takeover risk via
                    //     email, provisioning is permitted).
                    // Block only when email is present but unverified.
                    if id_claims.email.is_some() && !id_claims.email_verified {
                        tracing::info!(slug = %provider.slug, "provision held: unverified email");
                        return Ok(
                            Redirect::to("/admin/login?fed_error=unverified_email").into_response()
                        );
                    }
                    // P7: derive username, never trust upstream directly.
                    let proposed = derive_username(&id_claims);
                    let username = resolve_shadow_username(&app.db, &proposed).await;

                    let shadow = sui_id_store::repos::users::LdapShadowData {
                        username,
                        display_name: id_claims.name.clone(),
                        email: id_claims.email.clone(),
                        external_stable_id: format!("{}:{}", provider.id, id_claims.sub),
                    };
                    let uid = sui_id_store::repos::users::upsert_ldap_shadow(&app.db, shadow, now)
                        .await
                        .map_err(|e| HttpError::html(CoreError::from(e)))?;

                    // Insert federation link.
                    sui_id_store::repos::federation_link::upsert(
                        &app.db,
                        FederationLinkRow {
                            user_id: uid,
                            provider_id: provider.id,
                            upstream_sub: id_claims.sub.clone(),
                            upstream_email: id_claims.email.clone(),
                            linked_at: now,
                            last_seen_at: now,
                        },
                    )
                    .await
                    .map_err(|e| HttpError::html(CoreError::from(e)))?;

                    let _ = sui_id_store::repos::audit::append(
                        &app.db,
                        &AuditLogRow {
                            at: now,
                            actor: Some(uid),
                            action: sui_id_store::repos::federation_provider::AUDIT_LINK_CREATED
                                .into(),
                            target: Some(uid.to_string()),
                            result: "ok".into(),
                            note: Some(format!("provider={} sub={}", provider.slug, id_claims.sub)),
                        },
                    )
                    .await;

                    uid
                }

                ProvisionMode::LinkOnly => {
                    // Store the upstream claims in a short-lived cookie so the
                    // link confirmation page can complete the link.
                    let pending = serde_json::json!({
                        "provider_id": provider.id.to_string(),
                        "provider_slug": provider.slug,
                        "upstream_sub": id_claims.sub,
                        "upstream_email": id_claims.email,
                        "upstream_name": id_claims.name,
                    });
                    let pending_str = pending.to_string();
                    let mut c = Cookie::new("sui_id_fed_pending", pending_str);
                    c.set_http_only(true);
                    c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
                    c.set_max_age(time::Duration::seconds(600));
                    c.set_path("/auth/federated");
                    if app.config.server.cookie_secure {
                        c.set_secure(true);
                    }
                    let jar = jar.add(c);
                    return Ok((jar, Redirect::to("/auth/federated/link")).into_response());
                }
            }
        }
    };

    // ── P4: enforce local MFA ─────────────────────────────────────────────────
    complete_federated_signin(app, jar, user_id, &provider.slug, &id_claims.sub, now).await
}

// ── GET /auth/federated/link — link-only approval ────────────────────────────

pub async fn federated_link_get(jar: CookieJar) -> Result<Response, HttpError> {
    // If no pending cookie, redirect to login.
    if jar.get("sui_id_fed_pending").is_none() {
        return Ok(Redirect::to("/admin/login").into_response());
    }
    // Render the "confirm link" page — for now a simple redirect with a query
    // param to the regular login page which will POST back to /auth/federated/link.
    // Full UI page is a future iteration; this wires the flow skeleton.
    Ok(Redirect::to("/admin/login?fed_link=pending").into_response())
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Complete a federated sign-in: enforce local MFA then issue a session.
async fn complete_federated_signin(
    app: AppState,
    jar: CookieJar,
    user_id: UserId,
    provider_slug: &str,
    upstream_sub: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Response, HttpError> {
    // Refuse an inactive user before any pending-MFA or session row. The
    // user is re-read here: the link may belong to an account disabled or
    // deleted since it was created.
    let signin_failed = |jar: CookieJar| {
        let jar = jar.remove(Cookie::build(STATE_COOKIE));
        Ok((jar, Redirect::to("/admin/login?fed_error=signin_failed")).into_response())
    };
    match sui_id_store::repos::users::get(&app.db, user_id).await {
        Ok(user) if !user.is_disabled && !user.is_deleted => {}
        Ok(_) => {
            tracing::warn!(
                provider = %provider_slug,
                user_id = %user_id,
                "federation: sign-in refused for a disabled or deleted user"
            );
            return signin_failed(jar);
        }
        Err(e) => {
            tracing::error!(
                provider = %provider_slug,
                user_id = %user_id,
                error = %e,
                "federation: user read failed; sign-in refused"
            );
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
            return signin_failed(jar);
        }
    };

    if mfa_enabled {
        let pending = sui_id_core::mfa::issue_pending_mfa(&app.db, &app.clock, user_id)
            .await
            .map_err(HttpError::html)?;
        let cookie = crate::handlers::pending_mfa_cookie(
            pending.id.to_string(),
            app.config.server.cookie_secure,
        );
        let jar = jar.add(cookie);
        let _ = sui_id_store::repos::audit::append(
            &app.db,
            &AuditLogRow {
                at: now,
                actor: Some(user_id),
                action: "auth.login.password_ok_mfa_required".into(),
                target: Some(user_id.to_string()),
                result: "ok".into(),
                note: Some(format!("via federation provider={provider_slug}")),
            },
        )
        .await;
        return Ok((jar, Redirect::to("/admin/login/mfa")).into_response());
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
        upstream_sub.to_owned(),
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
        return signin_failed(jar);
    }

    // Metrics: record as a successful federated sign-in.
    if let Some(m) = app.metric() {
        m.signin(sui_id_store::metrics::signin_result::SUCCESS);
    }

    let cookie = session_cookie(session_row.id.to_string(), app.config.server.cookie_secure);
    let jar = jar.add(cookie).remove(Cookie::build(STATE_COOKIE));
    Ok((jar, Redirect::to("/admin")).into_response())
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
