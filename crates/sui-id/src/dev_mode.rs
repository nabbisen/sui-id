//! Dev-mode startup orchestration.
//!
//! When sui-id is invoked with `--dev`, this module replaces the
//! interactive setup wizard with a one-shot, scripted seeding
//! pass: it constructs an in-memory (or operator-pinned) database,
//! runs `setup::create_initial_admin`, creates a small set of
//! test users, registers a small set of OIDC test clients, and
//! prints a summary describing every credential it just made.
//!
//! The whole point is to let a developer build a relying party
//! (RP) against sui-id without spending five minutes clicking
//! through the setup wizard before each clean run. The module is
//! deliberately *not* available in production: the sui-id binary
//! gates the entire path on the `--dev` flag and prints a stack
//! of warnings to stderr before it starts.
//!
//! ## What dev mode does NOT change
//!
//! - **Cryptography.** Argon2id, XChaCha20-Poly1305, Ed25519 JWT
//!   signing are all on. PKCE S256-only is enforced exactly as
//!   in production.
//! - **OIDC standards.** Discovery, JWKS, authorization code with
//!   PKCE, token endpoint, introspection, revocation, userinfo
//!   all behave identically.
//! - **`unsafe_code = forbid`.** Workspace-level invariant; not
//!   relaxed.
//! - **Migrations.** Same schema, same MAX_SCHEMA_VERSION.
//!
//! Things that ARE relaxed in dev mode (with operator-visible
//! warnings) are listed in `print_dev_warnings`.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use sui_id_core::admin::{create_client, create_user, CreateClientSpec, CreateUserSpec};
use sui_id_core::setup::create_initial_admin;
use sui_id_core::time::SharedClock;
use sui_id_store::crypto::MasterKey;
use sui_id_store::Database;

// ---------- Public seed API ----------

/// In-memory representation of one dev-mode admin/user/client
/// (parsed from a TOML seed file or derived from CLI flags or
/// the hardcoded defaults).
#[derive(Debug, Clone)]
pub struct DevSeed {
    pub admin: DevAdmin,
    pub users: Vec<DevUser>,
    pub clients: Vec<DevClient>,
}

#[derive(Debug, Clone)]
pub struct DevAdmin {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DevUser {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub preferred_lang: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DevClient {
    /// Display label only — the actual `client_id` (a UUID) is
    /// assigned by `create_client` and surfaced in the seed
    /// summary. Dev mode prints the UUID, not this field, as the
    /// value to use as `client_id` in RPs.
    pub name: String,
    pub redirect_uris: Vec<String>,
    /// `None` for confidential clients (sui-id generates a fresh
    /// secret); `Some("")` for public clients (PKCE-only); any
    /// other `Some` for confidential with a caller-chosen secret
    /// (only honoured in dev mode for predictability).
    pub client_secret: Option<String>,
    pub allowed_scopes: String,
    pub post_logout_redirect_uris: Vec<String>,
}

impl Default for DevSeed {
    /// Hardcoded defaults: passwords are intentionally simple but
    /// 12+ characters (sui-id's password policy enforces a minimum
    /// length even in dev mode — we don't relax cryptographic
    /// posture for convenience). The pattern `<name>-<name>-<name>`
    /// is easy to remember and easy to type.
    fn default() -> Self {
        Self {
            admin: DevAdmin {
                username: "admin".into(),
                password: "admin-admin-admin".into(),
                email: Some("admin@example.test".into()),
                display_name: Some("Admin".into()),
            },
            users: vec![
                DevUser {
                    username: "alice".into(),
                    password: "alice-alice-alice".into(),
                    email: Some("alice@example.test".into()),
                    display_name: Some("Alice".into()),
                    preferred_lang: None,
                },
                DevUser {
                    username: "bob".into(),
                    password: "bob-bob-bob-bob".into(),
                    email: Some("bob@example.test".into()),
                    display_name: Some("Bob".into()),
                    preferred_lang: None,
                },
            ],
            clients: vec![DevClient {
                name: "Dev test client".into(),
                redirect_uris: vec![
                    "http://localhost:3000/callback".into(),
                    "http://localhost:5173/callback".into(),
                    "http://localhost:8000/callback".into(),
                ],
                client_secret: Some("test-secret".into()),
                allowed_scopes: "openid profile email".into(),
                post_logout_redirect_uris: vec![],
            }],
        }
    }
}

// ---------- TOML deserialisation ----------
//
// The TOML schema is intentionally close to but not identical to
// the runtime DevSeed struct: TOML conventions favour tables-of-
// arrays (`[[user]]`) and small lower-case field names. We use
// distinct struct names suffixed `Toml` so a future change to the
// runtime shape doesn't accidentally break TOML compatibility.

#[derive(Debug, Deserialize)]
struct DevSeedToml {
    admin: Option<DevAdminToml>,
    #[serde(default)]
    user: Vec<DevUserToml>,
    #[serde(default)]
    client: Vec<DevClientToml>,
}

#[derive(Debug, Deserialize)]
struct DevAdminToml {
    username: String,
    password: String,
    email: Option<String>,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DevUserToml {
    username: String,
    password: String,
    email: Option<String>,
    display_name: Option<String>,
    preferred_lang: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DevClientToml {
    /// Display label. The actual `client_id` (UUID) is assigned
    /// by sui-id and reported back in the seed summary.
    #[serde(default)]
    name: Option<String>,
    redirect_uris: Vec<String>,
    client_secret: Option<String>,
    #[serde(default)]
    allowed_scopes: Option<String>,
    #[serde(default)]
    post_logout_redirect_uris: Vec<String>,
    #[serde(default)]
    public: bool,
}

impl DevSeedToml {
    fn into_seed(self) -> DevSeed {
        let defaults = DevSeed::default();
        let admin = match self.admin {
            Some(a) => DevAdmin {
                username: a.username,
                password: a.password,
                email: a.email,
                display_name: a.display_name,
            },
            None => defaults.admin,
        };
        let users = if self.user.is_empty() {
            defaults.users
        } else {
            self.user
                .into_iter()
                .map(|u| DevUser {
                    username: u.username,
                    password: u.password,
                    email: u.email,
                    display_name: u.display_name,
                    preferred_lang: u.preferred_lang,
                })
                .collect()
        };
        let clients = if self.client.is_empty() {
            defaults.clients
        } else {
            self.client
                .into_iter()
                .enumerate()
                .map(|(i, c)| {
                    let name = c.name.unwrap_or_else(|| format!("Dev client #{}", i + 1));
                    let allowed_scopes =
                        c.allowed_scopes.unwrap_or_else(|| "openid profile email".into());
                    // `public = true` means "PKCE-only public client"
                    // (no secret); any explicit `client_secret` wins.
                    let client_secret = if c.public && c.client_secret.is_none() {
                        Some(String::new())
                    } else {
                        c.client_secret
                    };
                    DevClient {
                        name,
                        redirect_uris: c.redirect_uris,
                        client_secret,
                        allowed_scopes,
                        post_logout_redirect_uris: c.post_logout_redirect_uris,
                    }
                })
                .collect()
        };
        DevSeed {
            admin,
            users,
            clients,
        }
    }
}

/// Read a TOML seed file into a `DevSeed`. Returns the hardcoded
/// default for any field the file omits.
pub fn load_seed_from_toml(path: &Path) -> Result<DevSeed> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading dev-seed file {}", path.display()))?;
    let parsed: DevSeedToml = toml::from_str(&text)
        .with_context(|| format!("parsing dev-seed file {} as TOML", path.display()))?;
    Ok(parsed.into_seed())
}

/// Apply CLI flag overrides on top of an existing `DevSeed`.
/// Each `Some(_)` overwrites; `None` leaves the value alone.
pub struct DevFlagOverrides {
    pub admin_password: Option<String>,
    pub client_secret: Option<String>,
}

impl DevSeed {
    pub fn apply_overrides(&mut self, ovr: DevFlagOverrides) {
        if let Some(pw) = ovr.admin_password {
            self.admin.password = pw;
        }
        if let Some(cs) = ovr.client_secret {
            if let Some(c) = self.clients.first_mut() {
                c.client_secret = Some(cs);
            }
        }
    }
}

// ---------- Runtime: warnings + 0.0.0.0 confirmation ----------

/// Print the dev-mode warning header to stderr. Called once at
/// startup, before the seed is applied.
pub fn print_dev_warnings(bind: &str, seed_source: &str) {
    eprintln!("================================================================");
    eprintln!("  WARNING: sui-id is running in DEV MODE");
    eprintln!("  - Pre-seeded credentials are LISTED IN PLAIN TEXT below.");
    eprintln!("  - This mode MUST NOT be used in production.");
    eprintln!("  - HIBP off, lockout relaxed, cookie_secure off.");
    eprintln!("  - Bound to {bind}.");
    eprintln!("  - Seed source: {seed_source}");
    eprintln!("================================================================");
}

/// Print a per-credential summary after seeding succeeds.
pub fn print_seed_summary(seed: &DevSeed, outcome: &SeedOutcome, listen_addr: &str) {
    eprintln!();
    // RFC 047: tab-separated dev summary for easy terminal triple-click copy.
    eprintln!("==== sui-id dev summary =============================");
    eprintln!("listen\thttp://{listen_addr}");
    eprintln!("admin\t{}:{}", seed.admin.username, seed.admin.password);
    for u in &seed.users {
        eprintln!("user\t{}:{}", u.username, u.password);
    }
    for c in &outcome.clients {
        let secret_part = c.client_secret.as_deref().unwrap_or("(public)");
        let uris = c.redirect_uris.join(",");
        eprintln!("client\t{}\t{}\t{}\t{}",
            c.name, c.client_id, secret_part, uris);
    }
    eprintln!("=====================================================");
    eprintln!("OIDC discovery:\thttp://{listen_addr}/.well-known/openid-configuration");
    eprintln!("Admin console: \thttp://{listen_addr}/admin");
}

/// Prompt for `yes` confirmation when binding to an
/// externally-reachable address. Reads from stdin; returns Err
/// (aborted) if the operator types anything other than `yes`.
///
/// The reason for stricter handling than a plain warning: dev mode
/// already promises ephemeral data and weak defaults. Letting it
/// bind to `0.0.0.0` without an explicit "yes" makes it too easy
/// for someone running `sui-id --dev --dev-bind 0.0.0.0` from a
/// shell history command to expose plaintext credentials to a
/// LAN they didn't expect to be on.
pub fn confirm_external_bind(bind: &str) -> Result<()> {
    use std::io::{BufRead, Write};
    let mut stderr = std::io::stderr();
    eprintln!();
    eprintln!("================================================================");
    eprintln!("  WARNING: --dev-bind {bind} exposes dev-mode sui-id beyond");
    eprintln!("  127.0.0.1. Plaintext seed credentials WILL be reachable from");
    eprintln!("  every host on your network. This is almost never what you");
    eprintln!("  want; reasons it might be (Docker container, LAN demo, ...)");
    eprintln!("  are the operator's responsibility.");
    eprintln!("================================================================");
    write!(stderr, "Type 'yes' to continue: ").ok();
    stderr.flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("reading dev-bind confirmation from stdin")?;
    if line.trim() != "yes" {
        bail!("aborted (no 'yes' confirmation for non-loopback dev-bind)");
    }
    Ok(())
}

// ---------- Database construction ----------

/// Open a fresh in-memory database, or open the given path
/// (creating it if it does not exist). Both paths use a freshly
/// generated `MasterKey` — dev mode does not persist a key file
/// because the data is meant to be ephemeral; a `--dev-db PATH`
/// across restarts would re-seed under a *new* master key,
/// which would not decrypt any prior ciphertext, so the previous
/// file (if any) is wiped on reopen by truncation.
pub fn open_dev_db(path: Option<&Path>) -> Result<Database> {
    let key = MasterKey::generate();
    match path {
        None => Database::open_in_memory(key)
            .context("opening in-memory dev database"),
        Some(p) => {
            // Truncate any previous file so we don't try to decrypt
            // ciphertext under the new master key. Dev-mode writes
            // ephemeral data; if the operator wants persistence
            // across restarts, they should still expect a clean
            // re-seed each time.
            if p.exists() {
                std::fs::remove_file(p)
                    .with_context(|| format!("removing stale {}", p.display()))?;
            }
            Database::open(p, key).context("opening pinned dev database")
        }
    }
}

// ---------- Seeding orchestration ----------

/// Outcome of `apply_seed`. Holds the freshly-created admin's
/// `UserId` (passed as the `actor` for subsequent admin
/// operations), plus the actual `client_id` (UUID) and effective
/// `client_secret` for each created client so the caller can
/// print an accurate seed summary.
pub struct SeedOutcome {
    pub admin_user_id: sui_id_shared::ids::UserId,
    pub clients: Vec<SeededClient>,
}

pub struct SeededClient {
    /// The UUID-shaped `client_id` assigned by `create_client`.
    /// This is what an RP puts in its config as `client_id`.
    pub client_id: sui_id_shared::ids::ClientId,
    /// Display label from the seed input.
    pub name: String,
    /// `Some("...")` for confidential clients (either the
    /// caller-supplied secret or the auto-generated one);
    /// `None` for public clients (PKCE-only, no secret).
    pub client_secret: Option<String>,
    pub redirect_uris: Vec<String>,
    pub allowed_scopes: String,
}

/// Run the full dev-mode seed against `db`, using `setup_token`
/// for the initial-admin creation step.
pub async fn apply_seed(
    db: &Database,
    clock: &SharedClock,
    setup_token: &str,
    seed: &DevSeed,
) -> Result<SeedOutcome> {
    let created = create_initial_admin(
        db,
        clock,
        setup_token,
        setup_token,
        &seed.admin.username,
        &seed.admin.password,
        seed.admin.display_name.as_deref(),
        seed.admin.email.as_deref(),
    ).await
    .context("creating dev-mode admin")?;
    let admin_id = created.user_id;

    for u in &seed.users {
        create_user(
            db,
            clock,
            None,                                    // dev-mode: HIBP off
            sui_id_store::models::HibpMode::Off,
            &sui_id_core::actor::Actor::from_session(admin_id, sui_id_store::models::Role::Admin, sui_id_shared::ids::SessionId::new()).into_admin().unwrap_or_else(|_| unreachable!("dev-mode initial admin must be admin")),
            CreateUserSpec {
                username: &u.username,
                password: &u.password,
                display_name: u.display_name.as_deref(),
                email: u.email.as_deref(),
                is_admin: false,
                // dev_mode always runs at Development level
                min_password_len: sui_id_core::security::SecurityLevel::Development
                    .password_min_len(),
            },
        ).await
        .with_context(|| format!("creating dev-mode user {:?}", u.username))?;
    }

    let mut seeded_clients = Vec::with_capacity(seed.clients.len());
    for c in &seed.clients {
        // `confidential = false` is sui-id's signal for PKCE-only
        // public clients. An explicit empty string for the secret
        // means "public"; any other value means "confidential
        // with caller-chosen secret".
        let confidential = !matches!(c.client_secret.as_deref(), Some(""));
        // dev_mode seeds always rebuild after; pass a throwaway cache since
        // dev_mode runs before the main AppState is wired up.
        let _dev_caches = sui_id_core::cache::Caches::new();
        let created_client = create_client(
            db,
            clock,
            &sui_id_core::actor::Actor::from_session(admin_id, sui_id_store::models::Role::Admin, sui_id_shared::ids::SessionId::new()).into_admin().unwrap_or_else(|_| unreachable!("dev-mode initial admin must be admin")),
            CreateClientSpec {
                name: &c.name,
                redirect_uris: &c.redirect_uris,
                confidential,
                allowed_scopes: &c.allowed_scopes,
                post_logout_redirect_uris: &c.post_logout_redirect_uris,
            },
            &_dev_caches,
        ).await
        .with_context(|| format!("creating dev-mode client {:?}", c.name))?;

        // The runtime API auto-generates a secret for confidential
        // clients. For dev-mode predictability we want the
        // operator-supplied secret in the row; patch it in via
        // a dedicated repo helper. (Public clients receive no
        // secret either way, so the patch is skipped.)
        let effective_secret = if confidential {
            match c.client_secret.as_deref() {
                Some(custom) if !custom.is_empty() => {
                    let hash = sui_id_core::password::hash_password(custom)
                        .context("hashing dev-mode client_secret")?;
                    sui_id_store::repos::clients::set_dev_secret_hash(
                        db,
                        created_client.row.id,
                        Some(&hash),
                    ).await
                    .context("patching dev-mode client_secret_hash")?;
                    Some(custom.to_owned())
                }
                _ => created_client.generated_secret.clone(),
            }
        } else {
            None
        };

        seeded_clients.push(SeededClient {
            client_id: created_client.row.id,
            name: c.name.clone(),
            client_secret: effective_secret,
            redirect_uris: c.redirect_uris.clone(),
            allowed_scopes: c.allowed_scopes.clone(),
        });
    }

    Ok(SeedOutcome {
        admin_user_id: admin_id,
        clients: seeded_clients,
    })
}

// ---------- Utility ----------

/// Resolve `--dev-seed` CLI arg + `--dev-admin-password` etc.
/// into a final `DevSeed`. Order: TOML (if provided) overrides
/// hardcoded defaults; flag overrides apply on top of the
/// resulting seed.
pub fn resolve_seed(
    seed_path: Option<&Path>,
    overrides: DevFlagOverrides,
) -> Result<(DevSeed, String)> {
    let (mut seed, source) = match seed_path {
        Some(p) => (
            load_seed_from_toml(p)?,
            format!("TOML file {}", p.display()),
        ),
        None => (DevSeed::default(), "hardcoded defaults".to_owned()),
    };
    seed.apply_overrides(overrides);
    Ok((seed, source))
}

#[allow(dead_code)]
fn _seed_path_must_be_path(_: PathBuf) {}

#[cfg(test)]
mod tests {
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
        assert_eq!(seed.clients[0].client_secret.as_deref(), Some("test-secret"));
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
}
