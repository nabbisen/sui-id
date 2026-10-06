use super::*;
use crate::config::FederationProviderConfig;
use chrono::Utc;
use std::io::Write;
use std::sync::{Arc, Mutex};
use sui_id_shared::ids::{FederationProviderId, UserId};
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{
    FederationLinkRow, FederationProviderRow, ProvisionMode, Role, UserRow, UserSource,
};
use sui_id_store::{Database, repos::federation_link, repos::federation_provider};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

fn seeded_row(slug: &str) -> FederationProviderRow {
    let now = Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Google".into(),
        issuer: "https://accounts.google.com".into(),
        client_id: "client-abc".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: true,
        allowed_origins: "https://accounts.google.com".into(),
        created_at: now,
        updated_at: now,
    }
}

fn matching_cfg(row: &FederationProviderRow) -> FederationProviderConfig {
    FederationProviderConfig {
        slug: row.slug.clone(),
        display_name: row.display_name.clone(),
        issuer: row.issuer.clone(),
        client_id: row.client_id.clone(),
        client_secret_env: String::new(),
        scopes: row.scopes.clone(),
        provision_mode: row.provision_mode.as_str().into(),
        enabled: row.enabled,
        allowed_origins: row
            .allowed_origins
            .split_whitespace()
            .map(String::from)
            .collect(),
    }
}

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn capture() -> (Captured, tracing::subscriber::DefaultGuard) {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    (captured, guard)
}

fn logged(captured: &Captured) -> String {
    String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned()
}

/// 7a: a stored origin set differing from config is updated on boot.
#[tokio::test]
async fn differing_allowed_origins_are_updated() {
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.allowed_origins = vec![
        "https://accounts.google.com".into(),
        "https://oauth2.googleapis.com".into(),
    ];
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(
        fetched.allowed_origins,
        "https://accounts.google.com https://oauth2.googleapis.com"
    );
}

/// 7a's other half: a stored origin set that already matches config is
/// **not written** -- asserted on the actual write, not just the end
/// state, since an unconditional update would pass a same-value check
/// alone.
#[tokio::test]
async fn matching_allowed_origins_produce_no_write() {
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let cfg = matching_cfg(&row);
    let later = row.updated_at + chrono::Duration::hours(1);
    reconcile_federation_provider(&db, &cfg, &row, later).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(
        fetched.updated_at, row.updated_at,
        "updated_at must not move if nothing was written"
    );
}

/// The update touches only `allowed_origins`: every other column is
/// unchanged across the reconciliation.
#[tokio::test]
async fn update_touches_only_allowed_origins() {
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.allowed_origins = vec!["https://oauth2.googleapis.com".into()];
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(fetched.allowed_origins, "https://oauth2.googleapis.com");
    assert_eq!(fetched.display_name, row.display_name);
    assert_eq!(fetched.issuer, row.issuer);
    assert_eq!(fetched.client_id, row.client_id);
    assert_eq!(fetched.scopes, row.scopes);
    assert_eq!(fetched.provision_mode, row.provision_mode);
    assert_eq!(fetched.enabled, row.enabled);
    assert_eq!(fetched.created_at, row.created_at);
}

/// RFC 134's whole point: changing the origin set must not destroy
/// federation links to the provider (the old fix -- delete and
/// recreate -- cascades and does exactly that).
#[tokio::test]
async fn federation_links_survive_an_origin_set_change() {
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let now = Utc::now();
    let user_id = UserId::new();
    sui_id_store::repos::users::create(
        &db,
        &UserRow {
            id: user_id,
            username: "fed-user".into(),
            display_name: None,
            is_admin: false,
            role: Role::User,
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: UserSource::Local,
            external_stable_id: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .unwrap();
    federation_link::upsert(
        &db,
        FederationLinkRow {
            user_id,
            provider_id: row.id,
            upstream_sub: "sub-1".into(),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.allowed_origins = vec!["https://oauth2.googleapis.com".into()];
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let still_there = federation_link::list_for_user(&db, user_id).await.unwrap();
    assert_eq!(still_there.len(), 1, "the federation link must survive");
}

/// 7b: a differing `scopes` produces a warning and does not change the
/// stored value.
#[tokio::test]
async fn differing_scopes_produces_a_warning_and_no_write() {
    let (captured, _guard) = capture();
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.scopes = "openid email profile".into();
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(
        fetched.scopes, row.scopes,
        "scopes must not be silently reconciled"
    );
    let log = logged(&captured);
    assert!(log.contains("field=scopes") || log.contains("field=\"scopes\""));
    assert!(log.contains("openid email profile"));
}

/// `enabled` is never compared -- it is meant to be a runtime toggle,
/// independent of this file, so a differing `enabled` must produce
/// neither a write nor a warning.
#[tokio::test]
async fn differing_enabled_produces_neither_write_nor_warning() {
    let (captured, _guard) = capture();
    let db = fresh_db();
    let row = seeded_row("google");
    federation_provider::create(&db, &row, None).await.unwrap();
    let mut cfg = matching_cfg(&row);
    cfg.enabled = !row.enabled;
    reconcile_federation_provider(&db, &cfg, &row, Utc::now()).await;
    let fetched = federation_provider::get(&db, row.id).await.unwrap();
    assert_eq!(fetched.enabled, row.enabled);
    let log = logged(&captured);
    assert!(!log.contains("field=enabled") && !log.contains("field=\"enabled\""));
}
