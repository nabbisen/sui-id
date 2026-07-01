//! Client admin operations (RFC 075, v0.62.0).
use crate::errors::{CoreError, CoreResult};
use crate::password::hash_password;
use crate::time::SharedClock;
use crate::tokens;
use crate::cache::Caches;
use sui_id_shared::ids::ClientId;
use sui_id_store::models::ClientRow;
use sui_id_store::repos::{
    clients, refresh_tokens,
};
use sui_id_store::Database;
// Shared audit helpers from parent module.
use crate::actor::{AdminActor, ReadOnlyAdminActor};
use super::{audit_ok, audit_with_note};
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
    /// Space-separated list of allowed scopes. Empty → permit any scope.
    pub allowed_scopes: &'a str,
    /// RP-initiated logout return URIs. Empty → fall back to
    /// `redirect_uris` at logout time (with a deprecation warning logged).
    pub post_logout_redirect_uris: &'a [String],
}

pub async fn create_client(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    spec: CreateClientSpec<'_>,
    _caches: &Caches,
) -> CoreResult<CreatedClient> {
    let actor_id = actor.user_id();
    if spec.name.trim().is_empty() {
        return Err(CoreError::BadRequest("client name must not be empty".into()));
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
    // Empty scope policy is allowed (means "permit any") — but if a list
    // is given, sanity-check that scope tokens look reasonable. RFC 6749
    // §3.3 restricts scope tokens to a printable subset.
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

    let secret_plain = if spec.confidential {
        Some(tokens::random_token(32))
    } else {
        None
    };
    let secret_hash = match secret_plain.as_deref() {
        Some(s) => Some(hash_password(s)?),
        None => None,
    };

    let now = clock.now();
    let row = ClientRow {
        id: ClientId::new(),
        name: spec.name.to_owned(),
        confidential: spec.confidential,
        secret_hash,
        redirect_uris: spec.redirect_uris.to_vec(),
        allowed_scopes: spec.allowed_scopes.to_owned(),
        post_logout_redirect_uris: spec.post_logout_redirect_uris.to_vec(),
        is_disabled: false,
        is_deleted: false,
        consent_policy: sui_id_store::models::ConsentPolicy::default(),
        created_at: now,
        updated_at: now,
    };
    clients::create(db, &row).await?;
    audit_ok(db, actor_id, "client.create", Some(row.id.to_string())).await;
    Ok(CreatedClient {
        row,
        generated_secret: secret_plain,
    })
}

/// Update the per-client scope policy. Empty string means "permit any".
pub async fn set_client_allowed_scopes(
    db: &Database,
    actor: &AdminActor,
    target: ClientId,
    scopes: &str,
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    for tok in scopes.split_whitespace() {
        if !tok
            .chars()
            .all(|c| c == '!' || ('#'..='[').contains(&c) || (']'..='~').contains(&c))
        {
            return Err(CoreError::BadRequest(format!(
                "invalid character in scope token {tok:?}"
            )));
        }
    }
    clients::set_allowed_scopes(db, target, scopes).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    audit_ok(db, actor_id, "client.set_allowed_scopes", Some(target.to_string())).await;
    Ok(())
}

/// Replace the `post_logout_redirect_uris` for a client.
pub async fn set_client_post_logout_redirect_uris(
    db: &Database,
    actor: &AdminActor,
    target: ClientId,
    uris: &[String],
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    for uri in uris {
        validate_redirect_uri(uri)?;
    }
    clients::set_post_logout_redirect_uris(db, target, uris).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    audit_ok(
        db,
        actor_id,
        "client.set_post_logout_redirect_uris",
        Some(target.to_string()),
    ).await;
    Ok(())
}

/// Update the basic client metadata: human-readable name and the
/// authorization redirect URIs. The id, type (confidential vs public),
/// and `secret_hash` are immutable.
pub async fn update_client_basic(
    db: &Database,
    actor: &AdminActor,
    target: ClientId,
    name: &str,
    redirect_uris: &[String],
    caches: &Caches,
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    if name.trim().is_empty() {
        return Err(CoreError::BadRequest("client name must not be empty".into()));
    }
    if redirect_uris.is_empty() {
        return Err(CoreError::BadRequest(
            "at least one redirect_uri must be provided".into(),
        ));
    }
    for uri in redirect_uris {
        validate_redirect_uri(uri)?;
    }
    clients::update_basic(db, target, Some(name.trim()), Some(redirect_uris)).await.map_err(|e| {
        match e {
            sui_id_store::StoreError::NotFound => CoreError::NotFound,
            other => CoreError::from(other),
        }
    })?;
    audit_ok(db, actor_id, "client.update", Some(target.to_string())).await;
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after update_client");
    }
    Ok(())
}

/// Convenience: fetch a single client (admin-gated).
pub async fn get_client(db: &Database, actor: &ReadOnlyAdminActor, target: ClientId) -> CoreResult<ClientRow> {
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

pub async fn update_client(
    db: &Database,
    actor: &AdminActor,
    target: ClientId,
    name: Option<&str>,
    redirect_uris: Option<&[String]>,
    _caches: &Caches,
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    if let Some(uris) = redirect_uris {
        if uris.is_empty() {
            return Err(CoreError::BadRequest(
                "at least one redirect_uri must remain".into(),
            ));
        }
        for u in uris {
            validate_redirect_uri(u)?;
        }
    }
    clients::update_basic(db, target, name, redirect_uris).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    audit_ok(db, actor_id, "client.update", Some(target.to_string())).await;
    Ok(())
}

pub async fn set_client_disabled(
    db: &Database,
    _clock: &SharedClock,
    actor: &AdminActor,
    target: ClientId,
    disabled: bool,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    clients::set_disabled(db, target, disabled).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if disabled {
        refresh_tokens::revoke_all_for_client(db, target).await?;
    }
    audit_with_note(
        db,
        actor_id,
        if disabled { "client.disable" } else { "client.enable" },
        Some(target.to_string()),
        if disabled { reason } else { None },
    ).await;
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after set_client_disabled");
    }
    Ok(())
}

pub async fn delete_client(
    db: &Database,
    actor: &AdminActor,
    target: ClientId,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    clients::soft_delete(db, target).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after delete_client");
    }
    refresh_tokens::revoke_all_for_client(db, target).await?;
    audit_with_note(db, actor_id, "client.delete", Some(target.to_string()), reason).await;
    Ok(())
}

// ---------- signing keys ----------

fn validate_redirect_uri(uri: &str) -> CoreResult<()> {
    let parsed = url::Url::parse(uri).map_err(|_| {
        CoreError::BadRequest(format!("redirect_uri is not a valid URL: {uri}"))
    })?;
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
        return Err(CoreError::BadRequest(format!(
            "redirect_uri must use https (http permitted only on loopback): {uri}"
        )));
    }
    if parsed.fragment().is_some() {
        return Err(CoreError::BadRequest(
            "redirect_uri must not contain a fragment".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::errors::CoreError;
// Shared audit helpers from parent module.
use super::validate_redirect_uri;

    #[test]
    fn https_redirect_is_accepted() {
        validate_redirect_uri("https://app.example.com/callback").expect("ok");
    }

    #[test]
    fn http_loopback_is_accepted() {
        validate_redirect_uri("http://localhost:8080/cb").expect("ok");
        validate_redirect_uri("http://127.0.0.1/cb").expect("ok");
    }

    #[test]
    fn http_non_loopback_is_rejected() {
        let r = validate_redirect_uri("http://example.com/cb");
        assert!(matches!(r, Err(CoreError::BadRequest(_))));
    }

    #[test]
    fn fragment_is_rejected() {
        let r = validate_redirect_uri("https://x/cb#frag");
        assert!(matches!(r, Err(CoreError::BadRequest(_))));
    }

    #[test]
    fn non_http_scheme_is_rejected() {
        let r = validate_redirect_uri("javascript:alert(1)");
        assert!(matches!(r, Err(CoreError::BadRequest(_))));
    }
}

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
    let actor_id = actor.user_id();
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
    let new_hash = crate::password::hash_password(&new_secret)?;
    clients::set_secret_hash(db, client_id, Some(&new_hash), clock.now()).await
        .map_err(CoreError::from)?;
    audit_with_note(
        db, actor_id, "client.rotate_secret", Some(client_id.to_string()), reason
    ).await;
    Ok(new_secret)
}
