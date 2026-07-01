//! Backup-and-restore round-trip preserving users and clients.
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use super::common::*;

fn utf8_encode(s: &str) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

#[tokio::test]
async fn backup_then_restore_preserves_users_and_clients() {
    use sui_id::backup;
    use sui_id::config::{LogConfig, ServerConfig, StorageConfig, TokensConfig};
    use sui_id_store::Database;
    use sui_id_store::crypto::MasterKey;

    // Step 1: build a real on-disk database with users + a client.
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("source.sqlite");
    let key_path = tmp.path().join("source.key");

    let key = MasterKey::generate();
    let key_b64 = key.to_base64();
    std::fs::write(&key_path, &key_b64).expect("write key");

    let key2 = MasterKey::from_base64(&key_b64).expect("decode key");
    let db = Database::open(&db_path, key2).expect("open db");
    let cfg_src = sui_id::config::Config {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".into(),
            issuer: "https://idp.test".into(),
            cookie_secure: false,
            trusted_proxies: Vec::new(),
        },
        storage: StorageConfig {
            db_path: db_path.clone(),
            key_file: key_path.clone(),
        },
        tokens: TokensConfig::default(),
        log: LogConfig {
            format: "fmt".into(),
            filter: "off".into(),
            access_log: false,
            file: None,
        },
        security: sui_id::config::SecurityConfig::default(),
    };
    let mailer: std::sync::Arc<dyn sui_id_core::mail::MailSender> =
        std::sync::Arc::new(sui_id_core::mail::InMemoryMailSender::new());
    let hibp_client: std::sync::Arc<dyn sui_id_core::hibp::HibpClient> =
        std::sync::Arc::new(sui_id_core::hibp::test_support::InMemoryHibpClient::new());
    let caches = std::sync::Arc::new(sui_id_core::cache::Caches::new());
    let state = sui_id::AppState::new(
        db,
        cfg_src.clone(),
        SETUP_TOKEN.into(),
        mailer,
        hibp_client,
        caches,
    );
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

    // Step 2: take a backup.
    let archive = tmp.path().join("backup.tar");
    backup::run_backup(&cfg_src, &archive, &backup::BackupOptions::default()).expect("backup");
    assert!(archive.exists());

    // Step 3: restore into a fresh location and re-open.
    let cfg_dst = sui_id::config::Config {
        storage: StorageConfig {
            db_path: tmp.path().join("restored.sqlite"),
            key_file: tmp.path().join("restored.key"),
        },
        ..cfg_src.clone()
    };
    backup::run_restore(
        &cfg_dst,
        &archive,
        &backup::RestoreOptions {
            force: false,
            passphrase: None,
        },
    )
    .expect("restore");

    // Step 4: open the restored DB with the restored key and verify the
    // user and client are still there.
    let restored_key_b64 = std::fs::read_to_string(&cfg_dst.storage.key_file).expect("read key");
    let restored_key = MasterKey::from_base64(restored_key_b64.trim()).expect("decode");
    let db2 = Database::open(&cfg_dst.storage.db_path, restored_key).expect("open restored");
    let users = sui_id_store::repos::users::list(&db2)
        .await
        .expect("list users");
    assert_eq!(
        users.len(),
        1,
        "the admin user should survive the round trip"
    );
    let clients = sui_id_store::repos::clients::list(&db2)
        .await
        .expect("list clients");
    assert_eq!(clients.len(), 1, "the client should survive the round trip");
    assert_eq!(clients[0].id.to_string(), client_id);
}
