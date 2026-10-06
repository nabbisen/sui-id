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
