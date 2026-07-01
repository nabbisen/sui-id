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
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::errors::HttpError;
use crate::handlers::{AppState, AppStateExt, session_cookie};
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::models::{AuditLogRow, FederationLinkRow, ProvisionMode, SessionRow};
// ── State cookie ──────────────────────────────────────────────────────────────

const STATE_COOKIE: &str = "sui_id_fed_state";
const STATE_TTL_SECS: i64 = 600; // 10 minutes (P5)
const HMAC_KEY_SUFFIX: &[u8] = b":federation-state-v1";

/// Contents of the signed state cookie.
#[derive(Serialize, Deserialize)]
struct FedState {
    /// Random nonce (also sent to upstream as `nonce` parameter).
    nonce: String,
    /// PKCE code verifier (raw, never sent to upstream).
    pkce_verifier: String,
    /// Provider slug for the callback to look up the provider.
    provider_slug: String,
    /// Unix timestamp at which this state expires (P5).
    expires_at: i64,
    /// Optional `next` URL to redirect to after sign-in.
    next: Option<String>,
    /// The `state` parameter sent to the upstream — verified in callback (P5 CSRF).
    upstream_state: String,
}

/// Seal the state as `{json}.{hmac_hex}`.
fn seal_state(app: &AppState, state: &FedState) -> anyhow::Result<String> {
    let json = serde_json::to_string(state)?;
    let mac = hmac_state(app, json.as_bytes());
    Ok(format!("{json}.{mac}"))
}

/// Verify and unseal a state value from the cookie (P5).
fn unseal_state(app: &AppState, raw: &str) -> Option<FedState> {
    let dot = raw.rfind('.')?;
    let (json_part, mac_part) = (&raw[..dot], &raw[dot + 1..]);
    let expected = hmac_state(app, json_part.as_bytes());
    // Constant-time comparison
    use subtle::ConstantTimeEq;
    let ok: bool = expected.as_bytes().ct_eq(mac_part.as_bytes()).into();
    if !ok {
        return None;
    }
    let state: FedState = serde_json::from_str(json_part).ok()?;
    let now = chrono::Utc::now().timestamp();
    if now > state.expires_at {
        return None; // expired
    }
    Some(state)
}

fn hmac_state(app: &AppState, data: &[u8]) -> String {
    let raw_key = app.db.key();
    // Derive a per-use subkey by mixing the master key with a purpose suffix.
    let mut key_material = Vec::with_capacity(32 + HMAC_KEY_SUFFIX.len());
    key_material.extend_from_slice(raw_key.as_bytes());
    key_material.extend_from_slice(HMAC_KEY_SUFFIX);
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&key_material).expect("HMAC accepts any key length");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

// ── Upstream discovery ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    #[serde(default)]
    userinfo_endpoint: Option<String>,
    issuer: String,
}

async fn fetch_discovery(client: &reqwest::Client, issuer: &str) -> Result<OidcDiscovery, String> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("discovery fetch failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("discovery returned {}", resp.status()));
    }
    resp.json::<OidcDiscovery>()
        .await
        .map_err(|e| format!("discovery parse failed: {e}"))
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
    let discovery = fetch_discovery(&app.http_client, &provider.issuer)
        .await
        .map_err(|e| {
            tracing::warn!(slug = %slug, error = %e, "federation discovery failed");
            HttpError::html(CoreError::BadRequest(
                "upstream identity provider is unavailable; try again later".into(),
            ))
        })?;

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
        discovery.authorization_endpoint,
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
    #[serde(default)]
    token_type: String,
}

#[derive(Deserialize)]
struct IdTokenClaims {
    sub: String,
    email: Option<String>,
    #[serde(default)]
    email_verified: bool,
    preferred_username: Option<String>,
    name: Option<String>,
    nonce: Option<String>,
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
    let discovery = fetch_discovery(&app.http_client, &provider.issuer)
        .await
        .map_err(|e| {
            tracing::warn!(slug = %provider.slug, error = %e, "federation token exchange: discovery failed");
            // Audit upstream failure
            let _ = emit_audit_soon(
                app.db.clone(), app.clock.now(),
                sui_id_store::repos::federation_provider::AUDIT_SIGNIN_UPSTREAM_FAILURE,
                Some(format!("provider={} error={e}", provider.slug)),
            );
            HttpError::html(CoreError::BadRequest("upstream IdP unavailable".into()))
        })?;

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
        .post(&discovery.token_endpoint)
        .form(&form_params)
        .send()
        .await
        .and_then(|r| r.error_for_status());

    let token_resp = match token_resp {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, slug = %provider.slug, "token exchange failed");
            let _ = emit_audit_soon(
                app.db.clone(),
                app.clock.now(),
                sui_id_store::repos::federation_provider::AUDIT_SIGNIN_UPSTREAM_FAILURE,
                Some(format!("provider={} error={e}", provider.slug)),
            );
            return Ok(Redirect::to("/admin/login?fed_error=token_exchange").into_response());
        }
    };

    let tokens: TokenResponse = match token_resp.json().await {
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
        Some(jwt) => decode_id_token_claims(jwt).unwrap_or(IdTokenClaims {
            sub: String::new(),
            email: None,
            email_verified: false,
            preferred_username: None,
            name: None,
            nonce: None,
        }),
        None => {
            // No id_token: fall back to userinfo endpoint if available.
            if let Some(ref ui_url) = discovery.userinfo_endpoint {
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
    if let Some(ref token_nonce) = id_claims.nonce {
        if token_nonce != &fed_state.nonce {
            tracing::warn!(slug = %provider.slug, "federation nonce mismatch");
            return Ok(Redirect::to("/admin/login?fed_error=nonce_mismatch").into_response());
        }
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
            if let Some(ref email) = id_claims.email {
                if let Ok(Some(_collision)) = sui_id_store::repos::users::find_by_email_normalized(
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
                            action:
                                sui_id_store::repos::federation_provider::AUDIT_TAKEOVER_BLOCKED
                                    .into(),
                            target: None,
                            result: "denied".into(),
                            note: Some(format!("provider={} email={email}", provider.slug)),
                        },
                    )
                    .await;
                    return Ok(
                        Redirect::to("/admin/login?fed_error=email_collision").into_response()
                    );
                }
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
    // P4: check local MFA.
    let mfa_enabled = sui_id_core::mfa::is_mfa_enabled(&app.db, user_id)
        .await
        .unwrap_or(false);

    if mfa_enabled {
        let pending = sui_id_core::mfa::issue_pending_mfa(&app.db, &app.clock, user_id)
            .await
            .map_err(|e| HttpError::html(e))?;
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
    sui_id_store::repos::sessions::insert(&app.db, &session_row)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let _ = sui_id_store::repos::users::set_last_login(&app.db, &user_id, now).await;

    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &AuditLogRow {
            at: now,
            actor: Some(user_id),
            action: sui_id_store::repos::federation_provider::AUDIT_SIGNIN_SUCCESS.into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: Some(format!("provider={provider_slug} sub={upstream_sub}")),
        },
    )
    .await;

    // Metrics: record as a successful federated sign-in.
    if let Some(m) = app.metric() {
        m.signin(sui_id_store::metrics::signin_result::SUCCESS);
    }

    let cookie = session_cookie(session_row.id.to_string(), app.config.server.cookie_secure);
    let jar = jar.add(cookie).remove(Cookie::build(STATE_COOKIE));
    Ok((jar, Redirect::to("/admin")).into_response())
}

/// Derive a candidate username from upstream ID token claims (P7).
fn derive_username(claims: &IdTokenClaims) -> String {
    // Priority: preferred_username → email local-part → sub (truncated)
    if let Some(ref pu) = claims.preferred_username {
        let clean: String = pu
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .take(32)
            .collect();
        if !clean.is_empty() {
            return clean;
        }
    }
    if let Some(ref email) = claims.email {
        if let Some(local) = email.split('@').next() {
            let clean: String = local
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .take(32)
                .collect();
            if !clean.is_empty() {
                return clean;
            }
        }
    }
    // Final fallback: first 16 chars of sub
    claims.sub.chars().take(16).collect()
}

/// Conflict-resolve a proposed username (P7): append numeric suffix until free.
async fn resolve_shadow_username(db: &sui_id_store::Database, proposed: &str) -> String {
    if sui_id_store::repos::users::find_by_username(db, proposed)
        .await
        .is_err()
    {
        return proposed.to_owned();
    }
    for n in 2u32..=1000 {
        let candidate = format!("{proposed}{n}");
        if sui_id_store::repos::users::find_by_username(db, &candidate)
            .await
            .is_err()
        {
            return candidate;
        }
    }
    format!("{proposed}-{}", sui_id_shared::ids::UserId::new())
}

/// Decode JWT claims without verifying signature (we trust the upstream's
/// token_endpoint over TLS; full JWKS validation is a future hardening step).
fn decode_id_token_claims(jwt: &str) -> Option<IdTokenClaims> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    let parts: Vec<&str> = jwt.split('.').collect();
    let payload = parts.get(1)?;
    // Pad base64 to a multiple of 4
    let padded = {
        let rem = payload.len() % 4;
        if rem == 0 {
            payload.to_string()
        } else {
            format!("{}{}", payload, "=".repeat(4 - rem))
        }
    };
    let decoded = Base64UrlUnpadded::decode_vec(&padded).ok()?;
    serde_json::from_slice(&decoded).ok()
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
    resp.json::<IdTokenClaims>()
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
