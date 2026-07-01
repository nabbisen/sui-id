//! Dev mode `--dev` flag-based startup with seeded data (v0.28.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

// ---------- v0.28.0: Dev mode ----------

/// Default-seed dev mode: open an in-memory DB, run apply_seed
/// with hardcoded defaults, assert that the admin and the two
/// test users land in the DB, and the OIDC test client is
/// usable.
#[tokio::test]
async fn dev_mode_default_seed_creates_admin_users_and_client() {
    use sui_id::dev_mode::{DevSeed, apply_seed, open_dev_db};

    let db = open_dev_db(None).expect("open in-memory dev db");
    let setup_token = "test-dev-setup-token";
    let seed = DevSeed::default();
    let clock = sui_id_core::time::system_clock();
    let outcome = apply_seed(&db, &clock, setup_token, &seed)
        .await
        .expect("apply_seed");

    // Admin lands.
    let admin = sui_id_store::repos::users::find_by_username(&db, "admin")
        .await
        .expect("admin");
    assert!(admin.is_admin);
    assert_eq!(admin.id, outcome.admin_user_id);

    // Two default users land.
    let alice = sui_id_store::repos::users::find_by_username(&db, "alice")
        .await
        .expect("alice");
    assert!(!alice.is_admin);
    let bob = sui_id_store::repos::users::find_by_username(&db, "bob")
        .await
        .expect("bob");
    assert!(!bob.is_admin);

    // One client landed.
    assert_eq!(outcome.clients.len(), 1);
    let client = &outcome.clients[0];
    assert_eq!(client.name, "Dev test client");
    assert_eq!(client.client_secret.as_deref(), Some("test-secret"));
    assert!(client.redirect_uris.iter().any(|u| u.contains(":3000")));
}

/// Flag overrides: `--dev-admin-password` and
/// `--dev-client-secret` reach apply_seed.
#[tokio::test]
async fn dev_mode_flag_overrides_apply_to_seed() {
    use sui_id::dev_mode::{DevFlagOverrides, DevSeed, apply_seed, open_dev_db};

    let db = open_dev_db(None).expect("open");
    let setup_token = "test-dev-setup-token";
    let mut seed = DevSeed::default();
    seed.apply_overrides(DevFlagOverrides {
        admin_password: Some("hunter2-and-then-some".into()),
        client_secret: Some("custom-cs-value-xyz".into()),
    });
    let clock = sui_id_core::time::system_clock();
    let outcome = apply_seed(&db, &clock, setup_token, &seed)
        .await
        .expect("apply");

    // Login as admin with the overridden password should succeed.
    let result = sui_id_core::session::login(&db, &clock, "admin", "hunter2-and-then-some", 0)
        .await
        .expect("admin login");
    let _ = result;

    // The first client's effective secret is the override.
    assert_eq!(
        outcome.clients[0].client_secret.as_deref(),
        Some("custom-cs-value-xyz")
    );
}

/// TOML seed: a custom user list and client list replace the
/// defaults, and `public = true` produces a PKCE-only client.
#[tokio::test]
async fn dev_mode_toml_seed_replaces_defaults() {
    use sui_id::dev_mode::{apply_seed, load_seed_from_toml, open_dev_db};

    let toml = r#"
[admin]
username = "ops"
password = "ops-pw-strong-enough"

[[user]]
username = "u1"
password = "u1-pw-strong-enough"

[[client]]
name = "spa"
redirect_uris = ["http://localhost:5173/cb"]
public = true

[[client]]
name = "api"
redirect_uris = ["http://localhost:8000/cb"]
client_secret = "api-secret-strong"
"#;

    let dir = tempfile::tempdir().expect("tmpdir");
    let path = dir.path().join("dev-seed.toml");
    std::fs::write(&path, toml).expect("write toml");

    let seed = load_seed_from_toml(&path).expect("load seed");
    assert_eq!(seed.admin.username, "ops");
    assert_eq!(seed.users.len(), 1);
    assert_eq!(seed.clients.len(), 2);

    let db = open_dev_db(None).expect("open");
    let clock = sui_id_core::time::system_clock();
    let outcome = apply_seed(&db, &clock, "test-dev-setup-token", &seed)
        .await
        .expect("apply");

    // Admin login works.
    let _ = sui_id_core::session::login(&db, &clock, "ops", "ops-pw-strong-enough", 0)
        .await
        .expect("admin login");

    // u1 exists, alice and bob do NOT.
    let _u1 = sui_id_store::repos::users::find_by_username(&db, "u1")
        .await
        .expect("u1 exists");
    let alice = sui_id_store::repos::users::find_by_username(&db, "alice").await;
    assert!(
        alice.is_err(),
        "alice should not exist when TOML supplies users"
    );

    // First client is public (PKCE-only): no secret.
    assert_eq!(outcome.clients.len(), 2);
    assert!(outcome.clients[0].client_secret.is_none());
    // Second client has the supplied secret.
    assert_eq!(
        outcome.clients[1].client_secret.as_deref(),
        Some("api-secret-strong")
    );
}

/// Pinning the dev DB to a path: the file is created, and a
/// pre-existing file is truncated each restart.
#[tokio::test]
async fn dev_mode_pinned_db_truncates_existing_file() {
    use sui_id::dev_mode::open_dev_db;

    let dir = tempfile::tempdir().expect("tmpdir");
    let path = dir.path().join("dev.sqlite");
    // Pre-create the file with junk content.
    std::fs::write(&path, b"junk").expect("pre-create");
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 4);

    // open_dev_db should remove and re-create.
    let db = open_dev_db(Some(&path)).expect("open with path");
    drop(db);
    // The file now exists and is a real SQLite DB (size > 4 bytes
    // due to the migrations applied at open time).
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size > 4, "expected SQLite file, size = {size}");
}

/// resolve_seed: TOML overrides defaults, flag overrides apply
/// on top.
#[tokio::test]
async fn dev_mode_resolve_seed_applies_priority() {
    use sui_id::dev_mode::{DevFlagOverrides, resolve_seed};

    let toml = r#"
[admin]
username = "admin-from-toml"
password = "toml-pw-strong"
"#;
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = dir.path().join("dev-seed.toml");
    std::fs::write(&path, toml).expect("write");

    let (seed, source) = resolve_seed(
        Some(&path),
        DevFlagOverrides {
            admin_password: Some("flag-overrides-toml".into()),
            client_secret: None,
        },
    )
    .expect("resolve");
    // TOML supplied the username; flag overrode the password.
    assert_eq!(seed.admin.username, "admin-from-toml");
    assert_eq!(seed.admin.password, "flag-overrides-toml");
    assert!(source.contains("TOML"));
}
