//! Startup sequence: configure logging, resolve the master key, open the
//! database, generate (or print) the setup token, and hand back a ready-to-
//! mount [`AppState`] plus the listen address.

use crate::AppState;
use crate::config::Config;
use crate::keyring::{self, KeyOrigin};
use anyhow::{Context, Result};
use base64ct::{Base64, Encoding};
use getrandom;
use std::sync::Once;
use sui_id_core::time::system_clock;
use sui_id_store::repos::state;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub struct Startup {
    pub state: AppState,
    pub listen_addr: String,
    /// Guard for the non-blocking file-writer background thread.
    /// Must be kept alive for as long as the process runs; dropping it
    /// flushes and joins the writer. Stored here so `main` can hold it.
    pub _log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

static INIT_TRACING: Once = Once::new();

/// How many rows from the tail of the audit log to chain-verify on
/// startup. A few thousand catches anything an attacker is likely
/// to have modified in a recent intrusion; deeper sweeps belong in
/// a scheduled background task or a dedicated CLI subcommand.
const AUDIT_VERIFY_TAIL: i64 = 5_000;

/// Initialise the tracing subscriber from a [`crate::config::LogConfig`].
///
/// # File appender
///
/// When `cfg.file` is `Some(dir)`, a daily-rotated file appender is
/// started. Log lines go to both stderr and the file. The returned
/// `WorkerGuard` must be kept alive for the process lifetime; dropping
/// it flushes and joins the writer thread.
///
/// # Access log
///
/// `cfg.access_log` enables the [`tower_http::trace::TraceLayer`] that
/// the router mounts. This function merely records the setting in a
/// thread-safe cell that `build_router` reads at startup.
/// Initialise the tracing subscriber from a [`crate::config::LogConfig`].
///
/// Returns a `WorkerGuard` when a file appender is configured. The guard
/// must be kept alive for the process lifetime; dropping it flushes and
/// joins the background writer thread.
///
/// When `log.file` is set, log output goes to both stderr and the
/// daily-rotated file. The two sinks are separate non-blocking layers so
/// neither blocks the async runtime.
pub fn init_tracing(
    cfg: &crate::config::LogConfig,
) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let mut guard = None;
    INIT_TRACING.call_once(|| {
        let env_filter = EnvFilter::try_new(&cfg.filter).unwrap_or_else(|_| EnvFilter::new("info"));

        if let Some(log_dir) = &cfg.file {
            // File output: stderr layer + file layer, combined with the
            // registry pattern so both receive every event.
            let file_appender = tracing_appender::rolling::daily(log_dir, "sui-id.log");
            let (nb_file, file_guard) = tracing_appender::non_blocking(file_appender);
            guard = Some(file_guard);

            let stderr_layer = tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_level(true);
            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(nb_file)
                .with_target(true)
                .with_level(true)
                .with_ansi(false); // no ANSI colour codes in files

            tracing_subscriber::registry()
                .with(env_filter)
                .with(stderr_layer)
                .with(file_layer)
                .init();
        } else if cfg.format == "json" {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(env_filter)
                .with_target(true)
                .with_level(true)
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_target(true)
                .with_level(true)
                .init();
        }
    });
    guard
}

/// Install the process-wide rustls crypto provider exactly once.
///
/// `rustls::ClientConfig::builder()` — used by `ldap3`'s `tls-rustls-ring`
/// TLS path and by `wasm-smtp-tokio`'s implicit-TLS SMTP transport — panics
/// if no default provider has been installed and more than one backend is
/// compiled in, which is the case here (`ring` via `ldap3`, `aws_lc_rs` via
/// reqwest). This is RFC 093 G09's "provider panic".
///
/// reqwest does *not* build an isolated provider instance that ignores the
/// global default — it prefers an installed default and only falls back to
/// its own `aws_lc_rs` default when none exists
/// (`reqwest::async_impl::client::default_rustls_crypto_provider`, used via
/// `CryptoProvider::get_default().unwrap_or_else(..)`). So this choice of
/// provider governs reqwest's backend too, not just LDAP/SMTP. We install
/// `aws_lc_rs` specifically to match reqwest's own default and leave
/// outbound HTTPS (federation discovery, JWKS, HIBP) on the same backend it
/// already used, rather than silently migrating it to `ring` as a side
/// effect of fixing the LDAP/SMTP panic.
///
/// Called once, before subcommand dispatch, in `main()` — not from
/// `serve`/`prepare()` alone — because `serve_dev` builds its own
/// `AppState` without going through `prepare()` and still constructs a
/// real SMTP sender that hits the same panic if implicit TLS is
/// configured. Idempotent: a second installation attempt returns `Err`
/// harmlessly, which we ignore.
pub fn install_rustls_crypto_provider() {
    static INIT_CRYPTO_PROVIDER: Once = Once::new();
    INIT_CRYPTO_PROVIDER.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

pub async fn prepare(cfg: Config) -> Result<Startup> {
    let log_guard = init_tracing(&cfg.log);

    // 1. Resolve master key.
    let resolved = keyring::resolve(&cfg.storage.key_file).context("resolving master key")?;
    match resolved.origin {
        KeyOrigin::Env => {
            tracing::info!("master key loaded from SUI_ID_MASTER_KEY environment variable");
        }
        KeyOrigin::KeyFile => {
            tracing::info!(path = %cfg.storage.key_file.display(), "master key loaded from key file");
        }
        KeyOrigin::GeneratedNew => {
            tracing::warn!(
                path = %cfg.storage.key_file.display(),
                "no master key found. A fresh 32-byte key has been generated and written. \
                 Back this file up: losing it makes encrypted columns unreadable."
            );
        }
    }

    // 2. Open the database (runs migrations).
    if let Some(parent) = cfg.storage.db_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }
    let db = crate::database::open(&cfg.storage.db_path, resolved.key)?;

    // Verify the tail of the audit-log hash chain. A mismatch here
    // indicates DB-level tampering since the most recent restart;
    // an attacker who modified an audit row directly via SQL will
    // have left the row's hash mismatching its recomputation.
    //
    // We don't refuse to start on detection — that would let an
    // attacker DoS the IdP by corrupting one row, and the same
    // reasoning applies at least as strongly to a check that could not
    // run at all, whose overwhelmingly common cause is transient
    // (RFC 121 D4) — but we surface the finding loudly so an operator's
    // monitoring catches it. `sui_id_core::audit_chain::check` is the one
    // function every caller of `verify_chain_tail` now goes through
    // (RFC 121): it decides the three outcomes and records a failure to
    // verify itself, so this call site only has to log at startup, once
    // more, that boot completed.
    sui_id_core::audit_chain::check(&db, &system_clock(), AUDIT_VERIFY_TAIL).await;

    // 3. Generate setup token if the system is uninitialized; print it once.
    let setup_token = generate_setup_token();
    let initialized = state::is_initialized(&db).unwrap_or(false);
    if !initialized {
        // v0.48.4: the setup URL now embeds the token as a query parameter
        // so the operator only needs to open the printed URL in a browser
        // — no copy-paste into a text field required.
        //
        // Use eprintln (stderr) per spec: the token never enters the
        // structured tracing pipeline so it does not leak into log
        // aggregators by accident.
        let base = cfg.server.issuer.trim_end_matches('/');
        eprintln!("\n  =====================================================");
        eprintln!("  sui-id has not been initialized yet.");
        eprintln!("  Open the following URL in your browser to begin setup:");
        eprintln!("    {base}/setup?token={setup_token}");
        eprintln!("  (One-time token — restart the process to generate a new one.)");
        eprintln!("  =====================================================\n");
    } else {
        tracing::info!("system already initialized; setup endpoint is closed");
    }

    let listen_addr = cfg.server.listen_addr.clone();
    // Note: a direct `SmtpMailSender` was historically built here for
    // synchronous sends, but the RFC 001 outbox path made it
    // redundant — production routes mail through the
    // `OutboxMailSender` built below; dev mode uses test helpers
    // that inject their own sender.

    // HIBP (Pwned Passwords) client. Constructed unconditionally
    // — the call site short-circuits when `server_settings.hibp_mode
    // = 'off'`, so a deployment that never wants outbound HIBP
    // traffic still pays only the cost of one Arc::new at startup.
    let hibp_client: std::sync::Arc<dyn sui_id_core::hibp::HibpClient> =
        std::sync::Arc::new(sui_id_core::hibp::HttpHibpClient::new());
    // Build hot-path caches (RFC 014). Log and continue on failure —
    // the startup completes with empty caches; the first request will
    // be served from the DB instead, and the next mutation will rebuild.
    let caches = match sui_id_core::cache::Caches::build(&db).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "cache build failed at startup; using empty caches");
            std::sync::Arc::new(sui_id_core::cache::Caches::new())
        }
    };
    // RFC 001: use the persistent outbox sender in production (not dev-mode).
    // Dev mode retains the direct SmtpMailSender (via test_app() helpers).
    let outbox_sender: std::sync::Arc<dyn sui_id_core::mail::MailSender> = std::sync::Arc::new(
        sui_id_core::mail::outbox::OutboxMailSender::new(db.clone(), system_clock()),
    );
    let mut state = AppState::new(db, cfg, setup_token, outbox_sender, hibp_client, caches);
    // RFC 006: initialise the Prometheus metrics registry when enabled.
    if state.config.server.metrics_enabled {
        match sui_id_store::metrics::Metrics::new() {
            Ok(m) => {
                // Install as the global handle so store internals
                // (audit::append, email_outbox::enqueue) can increment
                // counters without any signature change.
                sui_id_store::set_global_metrics(m.clone());
                state.metrics = Some(m);
                tracing::info!("metrics enabled");
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to initialise metrics registry; metrics disabled");
            }
        }
    }
    // RFC 005: build the user-source cascade from config.
    for cfg in &state.config.user_sources {
        match cfg.validate_and_resolve_password() {
            Ok(password) => {
                #[cfg(feature = "ldap")]
                if cfg.kind == "ldap" {
                    let ldap_cfg = sui_id_store::ldap_source::LdapUserSourceConfig {
                        slug: cfg.slug.clone(),
                        url: cfg.url.clone(),
                        bind_dn: cfg.bind_dn.clone(),
                        bind_password: password,
                        user_search_base: cfg.user_search_base.clone(),
                        user_search_filter: cfg.user_search_filter.clone(),
                        stable_id_attribute: cfg.stable_id_attribute.clone(),
                        display_name_attribute: cfg.display_name_attribute.clone(),
                        email_attribute: cfg.email_attribute.clone(),
                        connect_timeout_secs: cfg.connect_timeout_secs,
                        search_timeout_secs: cfg.search_timeout_secs,
                    };
                    state.user_sources.push(std::sync::Arc::new(
                        sui_id_store::ldap_source::LdapUserSource::new(ldap_cfg),
                    ));
                    tracing::info!(slug = %cfg.slug, "LDAP user-source registered");
                }
                #[cfg(not(feature = "ldap"))]
                {
                    tracing::warn!(
                        slug = %cfg.slug,
                        "user_source configured but sui-id was built without the \
                         'ldap' feature — source ignored"
                    );
                    let _ = password;
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "user_source config error; source skipped");
            }
        }
    }
    // RFC 004: sync federation provider config into the DB.
    // Each [[federation_providers]] block is upserted so operators
    // can manage enabled/disabled state via the admin UI.
    for cfg in &state.config.federation_providers {
        use sui_id_shared::ids::FederationProviderId;
        use sui_id_store::models::{FederationProviderRow, ProvisionMode};
        let secret = match cfg.resolve_secret() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, slug = %cfg.slug,
                    "federation_provider secret error; provider skipped");
                continue;
            }
        };
        // Check if already in DB; create if missing.
        match sui_id_store::repos::federation_provider::get_by_slug(&state.db, &cfg.slug).await {
            Ok(existing) => {
                reconcile_federation_provider(&state.db, cfg, &existing, state.clock.now()).await;
            }
            Err(sui_id_store::StoreError::NotFound) => {
                let now = state.clock.now();
                let row = FederationProviderRow {
                    id: FederationProviderId::new(),
                    slug: cfg.slug.clone(),
                    display_name: cfg.display_name.clone(),
                    issuer: cfg.issuer.clone(),
                    client_id: cfg.client_id.clone(),
                    client_secret_enc: None, // encrypted by create()
                    scopes: cfg.scopes.clone(),
                    provision_mode: ProvisionMode::parse(&cfg.provision_mode),
                    enabled: cfg.enabled,
                    allowed_origins: cfg.allowed_origins.join(" "),
                    created_at: now,
                    updated_at: now,
                };
                if let Err(e) = sui_id_store::repos::federation_provider::create(
                    &state.db,
                    &row,
                    secret.as_deref(),
                )
                .await
                {
                    tracing::error!(error = %e, slug = %cfg.slug,
                        "failed to register federation provider");
                } else {
                    tracing::info!(slug = %cfg.slug, "federation provider registered");
                }
            }
            Err(e) => tracing::error!(error = %e, "federation_provider lookup failed"),
        }
    }
    Ok(Startup {
        state,
        listen_addr,
        _log_guard: log_guard,
    })
}

/// RFC 134 step 7 — reconcile one `[[federation_providers]]` block against
/// its already-existing stored row.
///
/// Only `allowed_origins` is ever written here (7a). Before this, the
/// origin set was write-once at creation (`Ok(_) => {}`, this match arm's
/// predecessor): the only way to change it was delete-and-recreate, and
/// `federation_link`'s `ON DELETE CASCADE` means that destroys every
/// federated user's account linkage. An administrator adding a second
/// origin — the Google case RFC 134 D3 itself cites — had no non-destructive
/// option.
///
/// Every other config-managed field that differs from the stored row is
/// only warned about (7b), never written: changing `issuer` or `client_id`
/// on a live provider is a larger, separate decision than changing its
/// origin set, and reconciling it silently here would make that decision
/// for an administrator who may not have intended it. `enabled` is
/// deliberately not compared — the row's own column is meant to be a
/// runtime toggle, independent of this file, so warning every boot about
/// it would be noise even though (see the package this step shipped with)
/// nothing in this codebase actually exposes a way to toggle it today.
/// The client secret is deliberately not compared either — it is
/// encrypted at rest, and comparing it would mean decrypting it on every
/// boot just to produce a log line.
async fn reconcile_federation_provider(
    db: &sui_id_store::Database,
    cfg: &crate::config::FederationProviderConfig,
    existing: &sui_id_store::models::FederationProviderRow,
    now: chrono::DateTime<chrono::Utc>,
) {
    use sui_id_store::models::ProvisionMode;

    let desired_origins = cfg.allowed_origins.join(" ");
    if existing.allowed_origins != desired_origins {
        tracing::info!(
            slug = %cfg.slug,
            old_allowed_origins = %existing.allowed_origins,
            new_allowed_origins = %desired_origins,
            "federation_provider allowed_origins changed in config; updating stored value"
        );
        if let Err(e) = sui_id_store::repos::federation_provider::update_allowed_origins(
            db,
            existing.id,
            &desired_origins,
            now,
        )
        .await
        {
            tracing::error!(error = %e, slug = %cfg.slug,
                "failed to update federation_provider allowed_origins");
        }
    }

    warn_if_federation_field_differs(
        &cfg.slug,
        "display_name",
        &existing.display_name,
        &cfg.display_name,
    );
    warn_if_federation_field_differs(&cfg.slug, "issuer", &existing.issuer, &cfg.issuer);
    warn_if_federation_field_differs(&cfg.slug, "client_id", &existing.client_id, &cfg.client_id);
    warn_if_federation_field_differs(&cfg.slug, "scopes", &existing.scopes, &cfg.scopes);
    let configured_provision_mode = ProvisionMode::parse(&cfg.provision_mode);
    if configured_provision_mode != existing.provision_mode {
        warn_if_federation_field_differs(
            &cfg.slug,
            "provision_mode",
            existing.provision_mode.as_str(),
            &cfg.provision_mode,
        );
    }
}

/// Logs the 7b warning for one differing field, naming the slug, the
/// field, and both values, with one sentence saying the stored value is
/// in force and how to change it. A no-op when the two already agree.
fn warn_if_federation_field_differs(slug: &str, field: &str, stored: &str, configured: &str) {
    if stored != configured {
        tracing::warn!(
            slug = %slug,
            field = %field,
            stored = %stored,
            configured = %configured,
            "federation_provider config field differs from the stored value; \
             the stored value is in force -- there is no admin UI for this field \
             today, so changing it means editing the stored row directly or \
             deleting and recreating the provider (which cascades to its \
             federation links)"
        );
    }
}

fn generate_setup_token() -> String {
    let mut buf = [0u8; 24];
    // A broken OS CSPRNG means we cannot mint a setup token safe to use as
    // a one-time credential; continuing with non-random output would be a
    // real vulnerability, so this must panic rather than degrade.
    #[allow(clippy::expect_used)]
    getrandom::fill(&mut buf).expect("system RNG unavailable");
    let mut out = vec![0u8; 64];
    let n = Base64::encode(&buf, &mut out).map(str::len).unwrap_or(0);
    out.truncate(n);
    String::from_utf8(out).unwrap_or_else(|_| "ERROR_GENERATING_TOKEN".into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "startup/tests.rs"]
mod tests;
