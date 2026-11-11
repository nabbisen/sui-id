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

/// RFC 143 stage 2 part A: one operator-facing warning, logged once, if
/// this host has no CA certificates to verify outbound TLS with.
///
/// Probes with the same `reqwest::ClientBuilder` construction every
/// startup-time TLS-capable client (HIBP, federation) attempts — the
/// one that fails, cleanly, with no certificates on disk. LDAPS and
/// SMTP build their own TLS roots lazily, per connection, not at
/// startup, so this probe is the only place that would otherwise see
/// no signal at all that anything is wrong until the first federated
/// sign-in or outbound mail attempt fails.
///
/// Deliberately **not** called from the probe's own result: a true
/// positive here is read once, logged once, and forgotten — each
/// feature's own call site degrades and fails on its own, every time,
/// not just the first.
pub fn warn_if_no_tls_roots() {
    if reqwest::Client::builder().build().is_err() {
        tracing::warn!(
            "no CA certificates were found on this host: outbound TLS cannot verify \
             servers, so HIBP breach checks, SMTP, LDAPS and federation sign-in will \
             all fail until the system's CA bundle is installed (e.g. the \
             `ca-certificates` package on Debian/Ubuntu). HIBP fails open: with no \
             CA certificates, no password will be breach-checked."
        );
    }
}

pub async fn prepare(cfg: Config, backup: crate::database::PreMigrationBackup) -> Result<Startup> {
    let log_guard = init_tracing(&cfg.log);
    warn_if_no_tls_roots();

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

    // 2. Open the database (runs migrations; RFC 142 stage 5: takes a
    // pre-migration backup first, unless `backup` declines it).
    if let Some(parent) = cfg.storage.db_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }
    let db = crate::database::open(&cfg.storage.db_path, resolved.key, backup)?;

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
        // RFC 096-B2 S2a Part E: `enabled` no longer enables anything --
        // enabling is C17's job, after a preflight (S4) -- but the key
        // stays accepted for now, so a block that still sets it true
        // gets one warning per boot rather than a silent no-op.
        warn_if_enabled_key_is_set(cfg);
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
                reconcile_federation_provider(&state.db, cfg, &existing, secret, state.clock.now())
                    .await;
            }
            Err(sui_id_store::StoreError::NotFound) => {
                register_new_federation_provider(&state.db, cfg, secret, state.clock.now()).await;
            }
            Err(e) => tracing::error!(error = %e, "federation_provider lookup failed"),
        }
    }
    // RFC 096-B2 S2b §E (amendment A2): every stored provider whose slug
    // no configured block names any more. Runs after the loop above, so
    // a slug present in both a config block and the database is never
    // considered "removed" by this pass. Nothing between here and the
    // `Ok(Startup {...})` return below serves a request or routes a
    // sign-in -- `prepare`'s only caller (`main.rs`) starts listening
    // after `prepare` returns, so this and the review-warning pass below
    // both run before the affected providers can be routed or
    // preflighted (RFC 094 `:547-575` clause 11).
    remove_providers_no_longer_configured(
        &state.db,
        &state.config.federation_providers,
        state.clock.now(),
    )
    .await;
    // RFC 096-B2 S1 §D: one warning per provider still awaiting review --
    // every provider migration 0050 disabled on upgrade, and any later
    // provider that has never been through a successful C17 enable (S4).
    // Every DB row, not only this boot's configured blocks: a provider
    // removed from config (A2) or never reconciled above still needs its
    // operator told.
    warn_providers_requiring_review(&state.db).await;
    Ok(Startup {
        state,
        listen_addr,
        _log_guard: log_guard,
    })
}

/// RFC 096-B2 S1 §D. Logged once per `requires_review` provider, every
/// boot -- not only the first one after migration 0050 runs -- since an
/// administrator who has not yet reviewed a provider should keep hearing
/// about it. A provider already `validated` (a successful C17 enable,
/// S4) produces no warning.
///
/// TODO(096-B2 S5): "see the operator guide's ..." below names the
/// section S7's operator guide will add; there is no admin UI yet for an
/// administrator to act on this warning through. Update the section name
/// in the message below (and this doc comment) once S5/S7 land, so it
/// names what actually exists instead of what is still planned.
async fn warn_providers_requiring_review(db: &sui_id_store::Database) {
    let providers = match sui_id_store::repos::federation_provider::list(db).await {
        Ok(providers) => providers,
        Err(e) => {
            tracing::error!(
                error = %e,
                "could not read federation providers to check which still require review"
            );
            return;
        }
    };
    for provider in providers {
        if provider.validation_status != sui_id_store::models::ValidationStatus::RequiresReview {
            continue;
        }
        // RFC 096-B2 S2a Part F: `architecture.md:306-307`'s own upgrade
        // report asks for "internal ID, slug, whether a link exists, and
        // missing policy fields but no secret, issuer query, email, or
        // subject" -- S1's warning named only the slug. The only policy
        // field that can be missing after this stage is `id_token_algs`
        // (every other trust column is `NOT NULL`).
        let has_link =
            sui_id_store::repos::federation_link::any_link_exists_for_provider(db, provider.id)
                .await
                .unwrap_or_else(|e| {
                    tracing::error!(error = %e, slug = %provider.slug,
                "could not check for existing links; reporting none, not blocking the warning");
                    false
                });
        let missing = if provider.id_token_algs.is_none() {
            "id_token_algs"
        } else {
            "none"
        };
        tracing::warn!(
            slug = %provider.slug,
            provider_id = %provider.id,
            has_link,
            missing_policy_fields = missing,
            "federation provider {:?} (id {}) requires review before federated sign-in \
             through it is enabled again -- {}, missing policy fields: {} -- \
             see the operator guide's \"Reviewing and re-enabling a federation provider\" \
             section",
            provider.slug,
            provider.id,
            if has_link {
                "has a linked user"
            } else {
                "has no linked user"
            },
            missing,
        );
    }
}

/// RFC 004 / RFC 094 M2b wave 3, C16 — create the row for a
/// `[[federation_providers]]` config block absent from the database.
///
/// Extracted into its own function, the same shape
/// [`reconcile_federation_provider`] already has, so the create path is
/// directly testable at the layer that actually runs it rather than only
/// through the whole of [`prepare`] — the precedent is
/// `dev_mode_seed_client_secret_patch_is_audited`
/// (`crates/sui-id/tests/e2e/dev_mode.rs`), which drives `apply_seed`
/// directly for the same reason.
///
/// A provider that fails to register must not abort startup — preserved
/// from the pre-Class-A call this replaces: still logged at `error`, not
/// propagated with `?`.
async fn register_new_federation_provider(
    db: &sui_id_store::Database,
    cfg: &crate::config::FederationProviderConfig,
    secret: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
) {
    use sui_id_shared::ids::FederationProviderId;
    use sui_id_store::federation_policy::FederationTrustPolicy;
    use sui_id_store::models::ProvisionMode;
    use sui_id_store::repos::federation_provider::NewFederationProvider;

    // RFC 096-B2 S2a Part B/C: C16 stores the validated policy, not raw
    // config strings. `Config::validate` already ran this exact
    // constructor at startup and refused to start on failure (`one
    // validator, not two`), so re-running it here cannot fail -- it is
    // re-run rather than threaded through `AppState` because it is pure
    // and cheap, and caching its result would be a second place this
    // provider's policy could silently drift from what config load time
    // already proved.
    let Ok(provision_mode) = ProvisionMode::try_parse(&cfg.provision_mode) else {
        // Never reached in practice: `Config::validate` already refused
        // startup for this. Logged and skipped, not panicked -- the
        // same "never abort startup for this" rule the caller's own
        // doc comment states, applied to a defensive check instead.
        tracing::error!(slug = %cfg.slug,
            "unreachable: provision_mode failed to parse after Config::validate; skipping");
        return;
    };
    let algs: Vec<&str> = cfg.id_token_algs.iter().map(String::as_str).collect();
    let origins: Vec<&str> = cfg.allowed_origins.iter().map(String::as_str).collect();
    let Ok(policy) = FederationTrustPolicy::new(
        &cfg.issuer,
        &cfg.client_id,
        !cfg.client_secret_env.is_empty(),
        &cfg.scopes,
        &algs,
        &origins,
        provision_mode,
    ) else {
        tracing::error!(slug = %cfg.slug,
            "unreachable: trust policy failed to validate after Config::validate; skipping");
        return;
    };

    let row = NewFederationProvider {
        id: FederationProviderId::new(),
        slug: cfg.slug.clone(),
        display_name: cfg.display_name.clone(),
        issuer: policy.issuer().to_string(),
        client_id: policy.client_id().to_string(),
        client_secret_enc: None, // sealed by create_federation_provider()
        scopes: policy.scopes_canonical(),
        provision_mode: policy.provision_mode(),
        // The column's own encoding (migration 0045): the operator's list,
        // or "" for "the issuer's origin alone". Each entry was already
        // proven canonical by the policy above. Storing the policy's
        // effective list instead would make `reconcile_federation_provider`
        // see "" != the stored list on the next boot, and C24 would rewrite
        // the column and audit a change nobody made.
        allowed_origins: cfg.allowed_origins.join(" "),
        id_token_algs: policy.id_token_algs_canonical(),
        created_at: now,
        updated_at: now,
    };
    // RFC 094 M2b wave 3, C16: the insert and its
    // `federation.provider.created` audit append now commit in one
    // transaction. RFC 096-B2 S2a Part C: the row this creates is always
    // disabled -- `NewFederationProvider` has no `enabled` field for
    // `cfg.enabled` to be copied into even if this function wanted to.
    if let Err(e) = sui_id_store::commands::create_federation_provider(db, row, secret).await {
        tracing::error!(error = %e, slug = %cfg.slug,
            "failed to register federation provider");
    } else {
        tracing::info!(slug = %cfg.slug, "federation provider registered");
    }
}

/// RFC 096-B2 S2a Part E: `enabled` no longer enables a provider --
/// enabling is C17's job (S4), after a preflight. The key stays accepted
/// in the config file for now (refusing it outright is a later
/// decision), so this is the one place that notices a block still
/// setting it and says so, once per boot per provider.
///
/// TODO(096-B2 S5): "how a provider is enabled" below names no mechanism
/// yet, because the admin UI that will be it does not exist. Update the
/// message once S5 lands.
fn warn_if_enabled_key_is_set(cfg: &crate::config::FederationProviderConfig) {
    if cfg.enabled {
        tracing::warn!(
            slug = %cfg.slug,
            "federation provider {:?}'s config sets enabled = true, but this key no longer \
             enables it -- enabling a federation provider now requires an administrator's \
             review, through a mechanism this release does not yet provide",
            cfg.slug,
        );
    }
}

/// RFC 096-B2 S2b — reconcile one `[[federation_providers]]` block
/// against its already-existing stored row.
///
/// **`display_name` (C26) applies without disabling** — it is not trust
/// policy (RFC 096 `:434`). **Every other field (C23) applies too, now**
/// — unlike the S2a-era 7b warning this replaces, a trust-field edit is
/// no longer silently inert: it is applied on this boot, through a
/// guarded full-policy replacement that forces the provider disabled
/// and `requires_review` until an administrator re-checks and
/// re-enables it (RFC 096-B2 S2b §0/§A). `enabled` is still never
/// compared here — Part E's own warning covers it, independently, in
/// the main loop.
async fn reconcile_federation_provider(
    db: &sui_id_store::Database,
    cfg: &crate::config::FederationProviderConfig,
    existing: &sui_id_store::models::FederationProviderRow,
    secret: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
) {
    use sui_id_store::federation_policy::FederationTrustPolicy;
    use sui_id_store::models::ProvisionMode;

    // Never reached in practice: `Config::validate` already refused
    // startup for an unparseable value. A no-op skip, not a panic, is
    // this function's own existing rule for every other defensive case.
    let Ok(provision_mode) = ProvisionMode::try_parse(&cfg.provision_mode) else {
        tracing::error!(slug = %cfg.slug,
            "unreachable: provision_mode failed to parse after Config::validate; skipping policy reconcile");
        return;
    };
    let algs: Vec<&str> = cfg.id_token_algs.iter().map(String::as_str).collect();
    let origins: Vec<&str> = cfg.allowed_origins.iter().map(String::as_str).collect();
    let Ok(proposed) = FederationTrustPolicy::new(
        &cfg.issuer,
        &cfg.client_id,
        !cfg.client_secret_env.is_empty(),
        &cfg.scopes,
        &algs,
        &origins,
        provision_mode,
    ) else {
        tracing::error!(slug = %cfg.slug,
            "unreachable: trust policy failed to validate after Config::validate; skipping policy reconcile");
        return;
    };

    // RFC 096-B2 S2b §A, C23: one guarded, atomic comparison-and-replace
    // against the stored, *decoded* value (never column strings -- the
    // rule the S2a review's own three fold-ins exist to enforce) --
    // `Ok(None)` means nothing differed, no transaction committed.
    match sui_id_store::commands::replace_federation_provider_policy(
        db,
        existing.id,
        proposed,
        // The column's own 0045 encoding, the same choice C16 makes for
        // the same reason (S2a fold-in 1): storing the policy's
        // *effective*, expanded origin list here instead would make the
        // very next boot's own decode-and-compare see a difference that
        // is not one.
        cfg.allowed_origins.join(" "),
        secret,
        now,
    )
    .await
    {
        Ok(Some(audited)) => {
            let changed = audited.into_inner();
            // TODO(096-B2 S5): "an administrator reviews" below names no
            // mechanism yet -- there is no admin UI for this today.
            // Update this message once S5 lands.
            tracing::warn!(
                slug = %cfg.slug,
                changed_fields = %changed.join(","),
                "federation provider {:?}'s trust policy changed in config ({}); the stored \
                 policy has been replaced and it is now disabled until an administrator \
                 reviews and re-enables it -- see the operator guide's \"Reviewing and \
                 re-enabling a federation provider\" section",
                cfg.slug,
                changed.join(","),
            );
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!(error = %e, slug = %cfg.slug,
                "failed to replace federation_provider trust policy");
        }
    }
    // RFC 094 `:555`: cache eviction only after commit. Nothing to evict:
    // `http/federation_cache.rs` exists, but nothing on the request path
    // calls it yet (the handler fetches discovery on each sign-in), and
    // startup runs before the router exists. Its entries bind (provider,
    // version, activation generation), so when it is wired in, an entry
    // from before C23 is not served anyway.

    // C26 after C23 (S2b §D): a boot that changes both trust policy and
    // label records the policy replacement first. Each is its own
    // transaction; a label is not trust policy and never disables.
    if existing.display_name != cfg.display_name
        && let Err(e) = sui_id_store::commands::update_federation_provider_display_name(
            db,
            existing.id,
            cfg.display_name.clone(),
            now,
        )
        .await
    {
        tracing::error!(error = %e, slug = %cfg.slug,
            "failed to update federation_provider display_name");
    }
}

/// RFC 096-B2 S2b §E (amendment A2): a stored provider whose slug no
/// configured `[[federation_providers]]` block names any more. If it is
/// enabled, C25 disables it (keeping its links); if it is already
/// disabled, C25 commits nothing. Either way, one warning per provider,
/// every boot — not only the first boot after the block disappears, the
/// same "keep saying it until it's addressed" choice
/// [`warn_providers_requiring_review`] already makes.
///
/// TODO(096-B2 S6): "restore the block, or delete the provider" below
/// names deletion as a path with no admin-UI mechanism yet either —
/// update this message once one exists.
async fn remove_providers_no_longer_configured(
    db: &sui_id_store::Database,
    configured: &[crate::config::FederationProviderConfig],
    now: chrono::DateTime<chrono::Utc>,
) {
    let providers = match sui_id_store::repos::federation_provider::list(db).await {
        Ok(providers) => providers,
        Err(e) => {
            tracing::error!(
                error = %e,
                "could not read federation providers to check for ones removed from configuration"
            );
            return;
        }
    };
    for provider in providers {
        if configured.iter().any(|c| c.slug == provider.slug) {
            continue;
        }
        if let Err(e) = sui_id_store::commands::remove_federation_provider_from_configuration(
            db,
            provider.id,
            now,
        )
        .await
        {
            // Not followed by the warning below: it says the provider is
            // disabled, which a failed C25 has not made true.
            tracing::error!(error = %e, slug = %provider.slug,
                "failed to disable federation_provider removed from configuration");
            continue;
        }
        tracing::warn!(
            slug = %provider.slug,
            provider_id = %provider.id,
            "federation provider {:?} (id {}) has no matching [[federation_providers]] \
             block in this configuration; it is disabled and keeps its existing federation \
             links -- restore the block, or delete the provider",
            provider.slug,
            provider.id,
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
