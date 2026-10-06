use super::*;

#[test]
fn defaults_are_sensible() {
    let seed = DevSeed::default();
    assert_eq!(seed.admin.username, "admin");
    assert!(
        seed.admin.password.len() >= 12,
        "default admin password must satisfy production policy (12+ chars)"
    );
    assert!(seed.users.iter().any(|u| u.username == "alice"));
    assert!(seed.users.iter().any(|u| u.username == "bob"));
    assert_eq!(seed.clients.len(), 1);
    assert_eq!(seed.clients[0].name, "Dev test client");
    assert_eq!(
        seed.clients[0].client_secret.as_deref(),
        Some("test-secret")
    );
}

#[test]
fn flag_overrides_apply_in_order() {
    let mut seed = DevSeed::default();
    seed.apply_overrides(DevFlagOverrides {
        admin_password: Some("hunter2".into()),
        client_secret: Some("zzz".into()),
    });
    assert_eq!(seed.admin.password, "hunter2");
    assert_eq!(seed.clients[0].client_secret.as_deref(), Some("zzz"));
}

#[test]
fn toml_partial_falls_back_to_defaults() {
    let toml = r#"
[admin]
username = "boss"
password = "secret"
"#;
    let parsed: DevSeedToml = toml::from_str(toml).expect("parse");
    let seed = parsed.into_seed();
    assert_eq!(seed.admin.username, "boss");
    assert_eq!(seed.admin.password, "secret");
    assert_eq!(seed.users.len(), 2);
    assert_eq!(seed.clients.len(), 1);
}

#[test]
fn toml_empty_users_uses_defaults() {
    let toml = r#"
[admin]
username = "admin"
password = "admin"
"#;
    let parsed: DevSeedToml = toml::from_str(toml).expect("parse");
    let seed = parsed.into_seed();
    assert_eq!(seed.users.len(), 2);
}

#[test]
fn toml_full_replacement() {
    let toml = r#"
[admin]
username = "ops"
password = "ops-pw"
email = "ops@example.test"

[[user]]
username = "u1"
password = "u1-pw"

[[user]]
username = "u2"
password = "u2-pw"
preferred_lang = "ja"

[[client]]
name = "spa"
redirect_uris = ["http://localhost:5173/cb"]
public = true

[[client]]
name = "api"
redirect_uris = ["http://localhost:8000/cb"]
client_secret = "api-secret"
allowed_scopes = "openid"
"#;
    let parsed: DevSeedToml = toml::from_str(toml).expect("parse");
    let seed = parsed.into_seed();
    assert_eq!(seed.admin.username, "ops");
    assert_eq!(seed.users.len(), 2);
    assert_eq!(seed.users[1].preferred_lang.as_deref(), Some("ja"));
    assert_eq!(seed.clients.len(), 2);
    assert_eq!(seed.clients[0].client_secret.as_deref(), Some(""));
    assert_eq!(seed.clients[1].client_secret.as_deref(), Some("api-secret"));
}

#[test]
fn toml_invalid_returns_error() {
    let bad = "this is not toml [[[";
    let result: Result<DevSeedToml, _> = toml::from_str(bad);
    assert!(result.is_err());
}
