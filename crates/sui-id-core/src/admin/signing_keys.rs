//! Signing-key admin operations (RFC 075, v0.62.0).
use crate::cache::Caches;
use crate::errors::{CoreError, CoreResult};
use crate::time::SharedClock;
use sui_id_store::Database;
use zeroize::Zeroizing;
use crate::actor::{AdminActor, ReadOnlyAdminActor};
use super::audit_with_note;
pub async fn list_signing_keys(
    db: &Database,
    actor: &ReadOnlyAdminActor,
) -> CoreResult<Vec<sui_id_store::models::SigningKeyRow>> {
    let actor_id = actor.user_id();
    Ok(sui_id_store::repos::signing_keys::list_published(db).await?)
}

/// Generate a fresh Ed25519 signing key, persist it as the new active key,
/// and retire the previous one. The previous key's row stays in the table
/// (and therefore in JWKS) so that tokens already issued under it can still
/// be verified during their lifetime — a "grace window" of one access-token
/// lifetime is sufficient. The retired key can be deleted afterwards by an
/// administrator.
///
/// Returns the new key id.
pub async fn rotate_signing_key(
    db: &Database,
    clock: &SharedClock,
    keyring_path: &str,
    actor: &AdminActor,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<sui_id_shared::ids::SigningKeyId> {
    use ed25519_dalek::SigningKey;
    use sui_id_shared::ids::SigningKeyId;
    use sui_id_store::repos::signing_keys;

    let actor_id = actor.user_id();

    // Generate the new key material first (outside the DB lock).
    // RFC 069: getrandom + from_bytes replaces SigningKey::generate(&mut OsRng).
        // Semantically equivalent: secret key material from OS RNG; memory
        // zeroized on drop via Zeroizing<>.
    let mut secret = Zeroizing::new([0u8; 32]);
    getrandom::fill(secret.as_mut()).expect("system RNG unavailable");
    let sk = SigningKey::from_bytes(&secret);
    let pk = sk.verifying_key();
    let new_id = SigningKeyId::new();

    // Delegate the retire-then-insert to the store layer. Migration 0021
    // adds a partial unique index (at most one is_active=1 row), so the
    // old insert-then-retire order would violate the constraint. The new
    // order retires first and inserts second inside one transaction.
    signing_keys::rotate_atomic(
        db,
        new_id,
        "EdDSA",
        sk.to_bytes().as_ref(),
        pk.to_bytes().as_ref(),
    ).await?;
    if let Err(e) = caches.jwks.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after rotate_signing_key");
    }
    let _ = clock;
    let _ = keyring_path;
    audit_with_note(db, actor_id, "signing_key.rotate", Some(new_id.to_string()), reason).await;
    Ok(new_id)
}

/// Permanently delete a retired signing key. Refuses to delete the
/// currently active key.
pub async fn delete_signing_key(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: sui_id_shared::ids::SigningKeyId,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    sui_id_store::repos::signing_keys::delete(db, target).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        sui_id_store::StoreError::Conflict => CoreError::Conflict(
            "cannot delete the active signing key; rotate first".into(),
        ),
        other => CoreError::from(other),
    })?;
    let _ = clock;
    audit_with_note(db, actor_id, "signing_key.delete", Some(target.to_string()), reason).await;
    if let Err(e) = caches.jwks.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after delete_signing_key");
    }
    Ok(())
}

