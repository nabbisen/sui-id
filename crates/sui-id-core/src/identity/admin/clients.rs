//! Client admin operations (RFC 075, v0.62.0).
use crate::actor::{AdminActor, ReadOnlyAdminActor};
use crate::cache::Caches;
use crate::errors::{CoreError, CoreResult};
use crate::password::hash_password;
use crate::time::SharedClock;
use crate::tokens;
use sui_id_shared::ids::ClientId;
use sui_id_store::Database;
use sui_id_store::models::ClientRow;
use sui_id_store::repos::clients;
pub struct CreatedClient {
    pub row: ClientRow,
    pub generated_secret: Option<String>,
}

/// Input to `create_client`. Supplied as a single struct so the call site
/// reads as a labelled record rather than a long positional list.
pub struct CreateClientSpec<'a> {
    pub name: &'a str,
    pub redirect_uris: &'a [String],
    pub confidential: bool,
    /// Space-separated list of allowed scopes. Empty → permit any
    /// scope, unchanged. A *non-empty* value gets `openid` added if
    /// missing (RFC 149 Q4 §1) — OpenID Connect requires it whenever
    /// the caller names scopes at all. The admin form (this stage's
    /// own scope) never submits an empty string here.
    pub allowed_scopes: &'a str,
    /// RP-initiated logout return URIs. Empty → fall back to
    /// `redirect_uris` at logout time (with a deprecation warning logged).
    pub post_logout_redirect_uris: &'a [String],
    /// RFC 147 §5: the create form now offers the same field the edit
    /// form already does, with the same default — callers that still
    /// want today's exact behaviour pass
    /// `sui_id_store::models::ConsentPolicy::default()` explicitly,
    /// same as this function used to hardcode.
    pub consent_policy: sui_id_store::models::ConsentPolicy,
    /// RFC 149 Q4 §3: the create form now offers the same three
    /// fields editing already did. Already validated by the caller
    /// (`validate_application_uri`, the same check edit's own form
    /// uses) — `None` means unset, not a caller shortcut.
    pub homepage_uri: Option<String>,
    pub privacy_policy_uri: Option<String>,
    pub tos_uri: Option<String>,
}

pub async fn create_client(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    spec: CreateClientSpec<'_>,
    caches: &Caches,
) -> CoreResult<CreatedClient> {
    if spec.name.trim().is_empty() {
        return Err(CoreError::BadRequest(
            "client name must not be empty".into(),
        ));
    }
    if spec.redirect_uris.is_empty() {
        return Err(CoreError::BadRequest(
            "at least one redirect_uri must be provided".into(),
        ));
    }
    for uri in spec.redirect_uris {
        validate_redirect_uri(uri)?;
    }
    for uri in spec.post_logout_redirect_uris {
        validate_redirect_uri(uri)?;
    }
    // RFC 6749 §3.3 restricts scope tokens to a printable subset.
    for tok in spec.allowed_scopes.split_whitespace() {
        if !tok
            .chars()
            .all(|c| c == '!' || ('#'..='[').contains(&c) || (']'..='~').contains(&c))
        {
            return Err(CoreError::BadRequest(format!(
                "invalid character in scope token {tok:?}"
            )));
        }
    }
    // RFC 149 Q4 §1: OpenID Connect requires `openid`; it is included
    // whatever was submitted, never decided by the caller.
    let allowed_scopes = ensure_openid_scope(spec.allowed_scopes);

    let secret_plain = if spec.confidential {
        Some(tokens::random_token(32))
    } else {
        None
    };
    let secret_hash = match secret_plain.as_deref() {
        Some(s) => Some(hash_password(s).await?),
        None => None,
    };

    let now = clock.now();
    let row = ClientRow {
        id: ClientId::new(),
        name: spec.name.to_owned(),
        confidential: spec.confidential,
        secret_hash,
        redirect_uris: spec.redirect_uris.to_vec(),
        allowed_scopes,
        post_logout_redirect_uris: spec.post_logout_redirect_uris.to_vec(),
        is_disabled: false,
        is_deleted: false,
        // RFC 147 §5: the caller's own choice now, offered by the create
        // form with the same default the RFC 136 D1 reasoning above
        // always assumed (`ConsentPolicy::default()` -> `None`, no
        // consent screen) -- an administrator may still override it at
        // creation time, the same as editing already allowed.
        consent_policy: spec.consent_policy.clone(),
        registered_via: sui_id_store::models::RegistrationSource::Admin,
        logo_uri: None,
        // RFC 149 Q4 §3: the create form now offers these same three
        // fields editing already did, already validated by the caller.
        homepage_uri: spec.homepage_uri.clone(),
        privacy_policy_uri: spec.privacy_policy_uri.clone(),
        tos_uri: spec.tos_uri.clone(),
        // RFC 095 M3 stage 4a: an administrator creating a client was not
        // asked for any of these either -- `None` here is "unrecorded",
        // the same as it is for a client that predates the column, not a
        // default standing in for a real choice.
        token_endpoint_auth_method: None,
        grant_types: None,
        response_types: None,
        created_at: now,
        updated_at: now,
    };
    sui_id_store::commands::create_client(db, actor.user_id(), row.clone())
        .await
        .map_err(CoreError::from)?;
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after create_client");
    }
    Ok(CreatedClient {
        row,
        generated_secret: secret_plain,
    })
}

/// Update the client-update form's three coordinated fields: allowed
/// scopes, post-logout redirect URIs, and consent policy. RFC 094 M2b
/// wave 4 stage 2 (C03, C04, C09): one sealed command, one transaction,
/// one audit record for the whole form — see `sui_id_store::commands::
/// update_client_form`'s own doc comment for why these three collapse
/// into a single sealed command. Validation (scope token charset,
/// redirect URI shape) happens here, before the sealed command runs,
/// the same boundary every other admin use-case function in this file
/// keeps.
pub async fn update_client_form(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: ClientId,
    allowed_scopes: &str,
    post_logout_redirect_uris: &[String],
    consent_policy: sui_id_store::models::ConsentPolicy,
) -> CoreResult<()> {
    for tok in allowed_scopes.split_whitespace() {
        if !tok
            .chars()
            .all(|c| c == '!' || ('#'..='[').contains(&c) || (']'..='~').contains(&c))
        {
            return Err(CoreError::BadRequest(format!(
                "invalid character in scope token {tok:?}"
            )));
        }
    }
    for uri in post_logout_redirect_uris {
        validate_redirect_uri(uri)?;
    }
    // RFC 149 Q4 §1: same as `create_client` — `openid` is included
    // whatever was submitted.
    let allowed_scopes = ensure_openid_scope(allowed_scopes);
    sui_id_store::commands::update_client_form(
        db,
        actor.user_id(),
        target,
        allowed_scopes,
        post_logout_redirect_uris.to_vec(),
        consent_policy,
        clock.now(),
    )
    .await
    .map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    Ok(())
}

/// Update the basic client metadata: human-readable name and the
/// authorization redirect URIs. The id, type (confidential vs public),
/// and `secret_hash` are immutable.
pub async fn update_client_basic(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: ClientId,
    name: &str,
    redirect_uris: &[String],
    caches: &Caches,
) -> CoreResult<()> {
    if name.trim().is_empty() {
        return Err(CoreError::BadRequest(
            "client name must not be empty".into(),
        ));
    }
    if redirect_uris.is_empty() {
        return Err(CoreError::BadRequest(
            "at least one redirect_uri must be provided".into(),
        ));
    }
    for uri in redirect_uris {
        validate_redirect_uri(uri)?;
    }
    sui_id_store::commands::update_client_basic(
        db,
        actor.user_id(),
        target,
        name.trim().to_owned(),
        redirect_uris.to_vec(),
        clock.now(),
    )
    .await
    .map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after update_client");
    }
    Ok(())
}

/// Convenience: fetch a single client (admin-gated).
pub async fn get_client(
    db: &Database,
    actor: &ReadOnlyAdminActor,
    target: ClientId,
) -> CoreResult<ClientRow> {
    let _ = actor;
    clients::get(db, target).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })
}

pub async fn list_clients(db: &Database, actor: &ReadOnlyAdminActor) -> CoreResult<Vec<ClientRow>> {
    let _ = actor;
    Ok(clients::list(db).await?)
}

pub async fn set_client_disabled(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: ClientId,
    disabled: bool,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<()> {
    let now = clock.now();
    if disabled {
        sui_id_store::commands::disable_client(db, actor.user_id(), target, reason, now)
            .await
            .map_err(|e| match e {
                sui_id_store::StoreError::NotFound => CoreError::NotFound,
                other => CoreError::from(other),
            })?;
    } else {
        sui_id_store::commands::enable_client(db, actor.user_id(), target, now)
            .await
            .map_err(|e| match e {
                sui_id_store::StoreError::NotFound => CoreError::NotFound,
                other => CoreError::from(other),
            })?;
    }
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after set_client_disabled");
    }
    Ok(())
}

pub async fn delete_client(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: ClientId,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<()> {
    sui_id_store::commands::delete_client(db, actor.user_id(), target, reason, clock.now())
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => CoreError::NotFound,
            other => CoreError::from(other),
        })?;
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after delete_client");
    }
    Ok(())
}

// ---------- redirect URIs ----------

/// Why a redirect URI is refused — structured, so a caller that shows
/// one error per URI (RFC 149 Q4 §2) can build its own localized
/// sentence instead of reading [`CoreError::BadRequest`]'s English
/// text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectUriError {
    NotAUrl,
    SchemeNotAllowed,
    HasFragment,
}

/// The structured check [`validate_redirect_uri`] wraps. Public so a
/// caller that validates many URIs at once (the app form) can collect
/// one [`RedirectUriError`] per URI, by index, rather than stopping at
/// the first.
pub fn check_redirect_uri(uri: &str) -> Result<(), RedirectUriError> {
    let parsed = url::Url::parse(uri).map_err(|_| RedirectUriError::NotAUrl)?;
    let scheme = parsed.scheme();
    let host = parsed.host_str().unwrap_or("");
    // Permit https everywhere; permit http only on loopback addresses for
    // local development.
    let ok = match scheme {
        "https" => true,
        "http" => matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1"),
        _ => false,
    };
    if !ok {
        return Err(RedirectUriError::SchemeNotAllowed);
    }
    if parsed.fragment().is_some() {
        return Err(RedirectUriError::HasFragment);
    }
    Ok(())
}

/// Validate a redirect URI. Public so other crates (e.g. dynamic
/// registration handler) can call it without duplicating the logic.
pub fn validate_redirect_uri(uri: &str) -> CoreResult<()> {
    check_redirect_uri(uri).map_err(|e| {
        CoreError::BadRequest(match e {
            RedirectUriError::NotAUrl => format!("redirect_uri is not a valid URL: {uri}"),
            RedirectUriError::SchemeNotAllowed => {
                format!("redirect_uri must use https (http permitted only on loopback): {uri}")
            }
            RedirectUriError::HasFragment => "redirect_uri must not contain a fragment".into(),
        })
    })
}

/// [`check_redirect_uri`], over every URI, never short-circuiting —
/// RFC 149 Q4 §2: "each URI is checked on its own... all the URIs'
/// errors are shown at once". `Ok(())` at index `i` means `uris[i]`
/// is valid.
pub fn check_redirect_uris(uris: &[String]) -> Vec<Result<(), RedirectUriError>> {
    uris.iter().map(|u| check_redirect_uri(u)).collect()
}

// ---------- scopes ----------

/// `openid` is included in a *non-empty* `raw` whatever else it
/// already contains — RFC 149 Q4 §1: OpenID Connect requires it, and
/// this is never decided by the caller. A no-op if already present
/// (whitespace-split, exact token match, so `"topenid"` is not
/// mistaken for it). A genuinely empty `raw` is left empty: that is
/// this function's caller's own "permit any scope" legacy meaning
/// (`CreateClientSpec::allowed_scopes`'s own doc comment), which
/// predates this stage and is not this stage's to end — the admin
/// form (this stage's own scope) never sends an empty string in the
/// first place, since it always lists `openid` itself before calling
/// either `create_client` or `update_client_form`.
pub fn ensure_openid_scope(raw: &str) -> String {
    if raw.trim().is_empty() || raw.split_whitespace().any(|tok| tok == "openid") {
        raw.to_owned()
    } else {
        format!("openid {raw}")
    }
}

/// Why an *additional scope* token is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdditionalScopeError {
    /// Fails RFC 6749 §3.3's token syntax.
    InvalidSyntax(String),
    /// Exactly a standard scope — it has its own checkbox.
    IsStandardScope(&'static str),
    /// A case difference or a one-edit typo of a standard scope.
    NearMiss {
        typed: String,
        standard: &'static str,
    },
}

/// Validate the *additional scopes* field: each whitespace-separated
/// token must pass RFC 6749 §3.3's syntax, then must not be a
/// standard scope or a near miss of one (RFC 149 Q4 §1). Stops at the
/// first refusal — unlike [`check_redirect_uris`], this field is one
/// box, not one row per token, so there is nowhere to show more than
/// one error beside. A duplicate token is kept once, in the order
/// first seen.
pub fn validate_additional_scopes(raw: &str) -> Result<Vec<String>, AdditionalScopeError> {
    let mut out: Vec<String> = Vec::new();
    for tok in raw.split_whitespace() {
        if !tok
            .chars()
            .all(|c| c == '!' || ('#'..='[').contains(&c) || (']'..='~').contains(&c))
        {
            return Err(AdditionalScopeError::InvalidSyntax(tok.to_owned()));
        }
        match scope_check::classify_against_standard_scopes(tok) {
            scope_check::StandardScopeMatch::ExactStandardScope(s) => {
                return Err(AdditionalScopeError::IsStandardScope(s));
            }
            scope_check::StandardScopeMatch::NearMiss(standard) => {
                return Err(AdditionalScopeError::NearMiss {
                    typed: tok.to_owned(),
                    standard,
                });
            }
            scope_check::StandardScopeMatch::None => {}
        }
        if !out.iter().any(|existing| existing == tok) {
            out.push(tok.to_owned());
        }
    }
    Ok(out)
}

#[path = "clients/scope_check.rs"]
pub mod scope_check;

#[cfg(test)]
#[path = "clients/tests.rs"]
mod tests;

/// Rotate the client secret for a confidential client.
///
/// Returns the new plaintext secret (shown to the operator once).
/// The plaintext is never stored — only the Argon2id hash is persisted.
/// Returns `Err(CoreError::BadRequest)` if the client is a public client.
pub async fn rotate_client_secret(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    client_id: ClientId,
    reason: Option<String>,
) -> CoreResult<String> {
    let client = clients::get(db, client_id).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if !client.confidential {
        return Err(CoreError::BadRequest(
            "cannot rotate secret for a public (PKCE-only) client".into(),
        ));
    }
    let new_secret = tokens::random_token(32);
    let new_hash = crate::password::hash_password(&new_secret).await?;
    sui_id_store::commands::rotate_client_secret(
        db,
        actor.user_id(),
        client_id,
        new_hash,
        reason,
        clock.now(),
    )
    .await
    .map_err(CoreError::from)?;
    Ok(new_secret)
}

/// Patch a dev-mode-seeded client's secret hash to an operator-supplied,
/// predictable value (RFC 094 M2b wave 4 stage 3, C08). Not an
/// administrator action in the ordinary sense — the only caller is
/// dev-mode seeding — but it runs under the same synthetic admin actor
/// `dev_mode.rs` already constructs for [`create_client`], so this stays
/// a thin, generic wrapper rather than dev-mode-specific logic living in
/// this crate.
pub async fn set_dev_client_secret(
    db: &Database,
    actor: &AdminActor,
    client_id: ClientId,
    new_hash: &str,
) -> CoreResult<()> {
    sui_id_store::commands::set_dev_client_secret(
        db,
        actor.user_id(),
        client_id,
        new_hash.to_owned(),
    )
    .await
    .map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    Ok(())
}

/// Update a client's administrator-settable application-identity links
/// (RFC 140 stage 1: homepage, privacy policy, terms of service). C10.
///
/// `logo_uri` is not a parameter here — stage 2, RFC 140 §5 still
/// undecided — and the sealed command preserves the client's existing
/// stored `logo_uri` unchanged; this function cannot blank it because it
/// never receives it.
///
/// URI validation (HTTPS only) is the HTTP layer's job
/// (`validate_application_uri`, in the binary crate — `sui-id-core`
/// cannot depend on it), before this runs. This function trusts its
/// input, the same validation boundary `update_client_form`'s own doc
/// comment describes. `sui_id_store::repos::clients::update_app_identity_
/// within_tx` still re-checks with the store's own, laxer rule
/// (`is_valid_app_uri`) as a backstop.
pub async fn update_client_app_identity(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: ClientId,
    homepage_uri: Option<String>,
    privacy_policy_uri: Option<String>,
    tos_uri: Option<String>,
) -> CoreResult<()> {
    sui_id_store::commands::update_client_app_identity(
        db,
        actor.user_id(),
        target,
        homepage_uri,
        privacy_policy_uri,
        tos_uri,
        clock.now(),
    )
    .await
    .map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    Ok(())
}
