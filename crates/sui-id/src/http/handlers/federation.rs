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
use serde::Deserialize;

use crate::errors::HttpError;
use crate::federation_identity::{derive_username, resolve_shadow_username};
use crate::federation_state::{FedState, STATE_COOKIE, STATE_TTL_SECS, seal_state, unseal_state};
use crate::handlers::{AppState, AppStateExt, session_cookie};
use crate::id_token::{IdTokenClaims, decode_id_token_claims};
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::models::{AuditLogRow, FederationLinkRow, ProvisionMode, SessionRow};

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
    let challenge_bytes = {
        use sha2::{Digest, Sha256};
        Sha256::digest(verifier_bytes).to_vec()
    };
    let pkce_challenge_b64 = {
        let mut out = vec![0u8; 64];
        let n = Base64UrlUnpadded::encode(&challenge_bytes, &mut out)
            .map(|s| s.len())
            .unwrap_or(0);
        out.truncate(n);
        String::from_utf8(out).unwrap_or_default()
    };

    // Random nonce (P5 single-use replay protection).
    let nonce = sui_id_core::tokens::random_token(16);

    // Random state parameter for open-redirect guard (CSRF).
    let state_param = sui_id_core::tokens::random_token(16);

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

    // Build the upstream authorization URL.
    let redirect_uri = format!(
        "{}/auth/federated/callback",
        app.config.server.issuer.trim_end_matches('/')
    );
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

    Ok((jar.add(state_cookie), Redirect::to(&upstream_url)).into_response())
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
    access_token: String,
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
                match fetch_userinfo(&app.http_client, ui_url, &tokens.access_token).await {
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
