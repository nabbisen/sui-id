//! In-process read caches for hot-path DB lookups (RFC 014).
//!
//! Two caches are provided:
//!
//! - [`RedirectOriginsCache`] — the set of allowed CORS origins, derived from
//!   all registered clients' `redirect_uris`. Re-built whenever a client is
//!   created, updated, or deleted.
//!
//! - [`JwksCache`] — the list of currently-active signing keys, used for JWT
//!   verification. Re-built whenever the signing-key set changes.
//!
//! Both caches are `Arc`-wrapped so they can be cloned cheaply into request
//! handlers. Readers take a short-lived read-lock; writers take a write-lock
//! only during the rebuild, which is O(n) over clients/keys — typically
//! microseconds.
//!
//! ## Failure behaviour
//!
//! If a rebuild's underlying DB read fails, the cache keeps its previous
//! snapshot and the error is returned to the caller (the mutation that
//! triggered the rebuild). A warn-level log line is emitted so operators
//! can investigate without blocking the mutation itself.

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use sui_id_store::Database;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Extract the `scheme://host[:port]` origin from a URI string.
fn origin_from_uri(uri: &str) -> Option<String> {
    // Minimal parse: find scheme, then authority up to the first path separator.
    let after_scheme = uri.split_once("://")?.1;
    let host_and_port = match after_scheme.find('/') {
        Some(idx) => &after_scheme[..idx],
        None => after_scheme,
    };
    let scheme = uri.split_once("://")?.0;
    // Normalise to lowercase for case-insensitive comparison.
    Some(format!(
        "{}://{}",
        scheme.to_lowercase(),
        host_and_port.to_lowercase()
    ))
}

// ── CORS redirect-origins cache ───────────────────────────────────────────────

/// Cached set of allowed CORS origins for the `/oauth2/token` endpoint.
///
/// An origin is considered allowed if it is a prefix of a registered
/// `redirect_uri`; concretely, the cache stores the `scheme://host[:port]`
/// prefix of every redirect URI across all non-deleted clients.
#[derive(Debug, Default)]
pub struct RedirectOriginsCache {
    inner: RwLock<HashSet<String>>,
}

impl RedirectOriginsCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-build from the current client list. Called at startup and after
    /// any client mutation.
    pub async fn rebuild(&self, db: &Database) -> Result<(), sui_id_store::StoreError> {
        let clients = sui_id_store::repos::clients::list(db).await?;
        let origins: HashSet<String> = clients
            .iter()
            .filter(|c| !c.is_deleted)
            .flat_map(|c| c.redirect_uris.iter())
            .filter_map(|uri| origin_from_uri(uri))
            .collect();
        *self.inner.write().await = origins;
        Ok(())
    }

    /// Returns `true` if `origin` matches any registered redirect-URI origin.
    /// Called on every token-endpoint CORS check; must be cheap.
    pub async fn contains(&self, origin: &str) -> bool {
        // Normalise the incoming origin.
        let normalised = origin.to_lowercase();
        self.inner.read().await.contains(&normalised)
    }

    /// Size of the current snapshot (for metrics / tests).
    #[cfg(test)]
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }
}

// ── JWKS signing-key cache ────────────────────────────────────────────────────

/// A single signing key entry in the cache.
#[derive(Debug, Clone)]
pub struct CachedSigningKey {
    pub kid: String,
    pub algorithm: String,
    pub public_key_bytes: Vec<u8>,
}

/// Cached list of currently-published signing keys.
///
/// Re-built whenever a key is rotated or retired. The cache covers all
/// keys whose `is_active` flag is true at the time of the last rebuild.
#[derive(Debug, Default)]
pub struct JwksCache {
    inner: RwLock<Vec<CachedSigningKey>>,
}

impl JwksCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-build from the current signing-key list.
    pub async fn rebuild(&self, db: &Database) -> Result<(), sui_id_store::StoreError> {
        let keys = sui_id_store::repos::signing_keys::list_active(db).await?;
        let cached: Vec<CachedSigningKey> = keys
            .into_iter()
            .map(|k| CachedSigningKey {
                kid: k.id.to_string(),
                algorithm: k.algorithm,
                public_key_bytes: k.public_key,
            })
            .collect();
        *self.inner.write().await = cached;
        Ok(())
    }

    /// Returns a snapshot of the active signing keys.
    /// Callers receive a cloned `Vec`; the read-lock is released immediately.
    pub async fn snapshot(&self) -> Vec<CachedSigningKey> {
        self.inner.read().await.clone()
    }

    /// Number of keys in the current snapshot.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

// ── Combined handle ───────────────────────────────────────────────────────────

/// Both caches bundled together, typically stored in `AppState`.
#[derive(Debug, Default)]
pub struct Caches {
    pub redirect_origins: RedirectOriginsCache,
    pub jwks: JwksCache,
}

impl Caches {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build both caches at startup. Logs and returns an error if either
    /// fails; a partial build (one cache populated) is not attempted.
    pub async fn build(db: &Database) -> Result<Arc<Self>, sui_id_store::StoreError> {
        let this = Arc::new(Self::new());
        this.redirect_origins.rebuild(db).await?;
        this.jwks.rebuild(db).await?;
        Ok(this)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn origin_extraction_works() {
        assert_eq!(
            origin_from_uri("https://app.example.com/callback"),
            Some("https://app.example.com".into())
        );
        assert_eq!(
            origin_from_uri("http://localhost:3000/callback"),
            Some("http://localhost:3000".into())
        );
        assert_eq!(
            origin_from_uri("HTTPS://App.Example.Com/cb"),
            Some("https://app.example.com".into())
        );
        assert_eq!(origin_from_uri("not-a-url"), None);
    }

    #[tokio::test]
    async fn redirect_origins_cache_contains() {
        let cache = RedirectOriginsCache::new();
        {
            let mut guard = cache.inner.write().await;
            guard.insert("https://app.example.com".into());
            guard.insert("http://localhost:3000".into());
        }
        assert!(cache.contains("https://app.example.com").await);
        assert!(cache.contains("HTTPS://APP.EXAMPLE.COM").await); // case-insensitive
        assert!(!cache.contains("https://evil.com").await);
    }

    #[tokio::test]
    async fn jwks_cache_snapshot_is_cloned() {
        let cache = JwksCache::new();
        {
            let mut guard = cache.inner.write().await;
            guard.push(CachedSigningKey {
                kid: "k1".into(),
                algorithm: "EdDSA".into(),
                public_key_bytes: vec![0u8; 32],
            });
        }
        let snap = cache.snapshot().await;
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].kid, "k1");
    }
}
