//! sui-id binary entry point.
//!
//! Usage:
//!     sui-id [--config PATH]
//!     sui-id backup --to PATH [--config PATH]
//!     sui-id restore --from PATH [--config PATH] [--force]
//!     sui-id --print-sample-config
//!
//! With no `--config`, the program looks for `./sui-id.toml`.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use sui_id::{build_router, config::Config, startup};

mod cli;

#[tokio::main]

async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("sui-id {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        cli::print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--print-sample-config") {
        let cfg = Config::sample();
        let s = toml::to_string_pretty(&cfg).context("serializing sample config")?;
        println!("{s}");
        return Ok(());
    }

    // Subcommands. Walk the argv carefully: skip past flags that take a
    // value so we don't treat the value (e.g. the path after `--config`)
    // as the subcommand.
    let subcommand = find_subcommand(&args);

    match subcommand.as_deref() {
        Some("backup") => return cli::run_backup_subcommand(&args),
        Some("restore") => return cli::run_restore_subcommand(&args),
        Some("verify-backup") => return cli::run_verify_backup_subcommand(&args),
        Some("admin") => return cli::run_admin_subcommand(&args).await,
        Some("setup") => return cli::run_setup_subcommand(&args).await,
        Some(other) => bail!("unknown subcommand {other:?}. Run `sui-id --help` for usage."),
        None => {} // fall through to `serve`.
    }

    if args.iter().any(|a| a == "--dev") {
        return serve_dev(&args).await;
    }

    serve(&args).await
}

/// First positional argument that is a real subcommand name, not a flag and
/// not the value of a flag that takes one.
fn find_subcommand(args: &[String]) -> Option<String> {
    const FLAGS_WITH_VALUE: &[&str] = &["--config", "--to", "--from"];
    let mut i = 1; // skip program name
    while i < args.len() {
        let a = &args[i];
        if a.starts_with('-') {
            // `--flag=value` is one token; otherwise it consumes the next arg
            // when it's a value-taking flag.
            if FLAGS_WITH_VALUE.contains(&a.as_str()) {
                i += 2;
            } else {
                i += 1;
            }
        } else {
            return Some(a.clone());
        }
    }
    None
}

async fn serve(args: &[String]) -> Result<()> {
    let config_path =
        cli::parse_config_path(args).unwrap_or_else(|| PathBuf::from("./sui-id.toml"));
    let cfg = Config::load(&config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;

    let startup = startup::prepare(cfg).await?;
    let router = build_router(startup.state.clone());

    sui_id::gc::spawn(startup.state.clone());
    // RFC 001: spawn the persistent email outbox worker.
    {
        let smtp = std::sync::Arc::new(sui_id_core::mail::SmtpMailSender::new(
            startup.state.db.clone(),
            String::from("sui-id"),
        ));
        let worker = sui_id_core::mail::outbox::OutboxWorker::new(
            startup.state.db.clone(),
            smtp,
            startup.state.clock.clone(),
            5, // idle_tick_secs
            5, // max_attempts
        );
        worker.spawn();
    }
    // One-shot backfill: populate token_hash for any refresh_token rows
    // that predate migration 0019. Runs in the background; the system is
    // correct before it completes.
    {
        let db = startup.state.db.clone();
        tokio::spawn(async move {
            match sui_id_store::repos::refresh_tokens::backfill_token_hashes(&db).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(rows = n, "backfill: populated refresh_token.token_hash"),
                Err(e) => tracing::warn!(error = %e, "backfill: refresh_token.token_hash failed"),
            }
        });
    }

    let addr: std::net::SocketAddr = startup
        .listen_addr
        .parse()
        .with_context(|| format!("invalid listen_addr {}", startup.listen_addr))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, "sui-id listening");

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("running server")?;
    Ok(())
}

/// `--dev` startup path.
///
/// Diverges from `serve` in three ways:
///
///   1. **Database.** Opens an in-memory SQLite DB (or a
///      caller-pinned path which is truncated each restart) under
///      a freshly generated master key. No `sui-id.toml`, no key
///      file.
///   2. **Config.** Synthesises a Config from `Config::sample()`,
///      adjusts `listen_addr` from `--dev-bind` (default
///      `127.0.0.1:8801`), keeps `cookie_secure = false`, and
///      passes it through unchanged otherwise. Production-relevant
///      knobs that we do not relax in dev — PKCE, AAD binding,
///      Argon2id parameters, etc — are decided in core, not here.
///   3. **Seed.** Runs `dev_mode::resolve_seed` over CLI flags
///      and an optional TOML file, then `apply_seed` against the
///      freshly opened DB. Prints both warning header and seed
///      summary to stderr around the call.
///
/// 0.0.0.0 binding requires explicit `yes` confirmation
/// from stdin; this is in `dev_mode::confirm_external_bind`.
async fn serve_dev(args: &[String]) -> Result<()> {
    use sui_id::dev_mode;

    // Flag parsing.
    let dev_db: Option<PathBuf> = args
        .iter()
        .position(|a| a == "--dev-db")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let dev_seed_path: Option<PathBuf> = args
        .iter()
        .position(|a| a == "--dev-seed")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let dev_bind: String = args
        .iter()
        .position(|a| a == "--dev-bind")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "127.0.0.1:8801".into());
    let dev_admin_password: Option<String> = args
        .iter()
        .position(|a| a == "--dev-admin-password")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let dev_client_secret: Option<String> = args
        .iter()
        .position(|a| a == "--dev-client-secret")
        .and_then(|i| args.get(i + 1))
        .cloned();

    // 0.0.0.0 (or any non-loopback) binding requires explicit
    // `yes` confirmation. We treat anything that is not `127.*`
    // or `[::1]` or `localhost` as "external" for this check.
    let host_part = dev_bind
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(&dev_bind);
    let is_loopback = host_part.starts_with("127.")
        || host_part == "::1"
        || host_part == "[::1]"
        || host_part == "localhost";
    if !is_loopback {
        dev_mode::confirm_external_bind(&dev_bind)?;
    }

    // Resolve seed.
    let (seed, seed_source) = dev_mode::resolve_seed(
        dev_seed_path.as_deref(),
        dev_mode::DevFlagOverrides {
            admin_password: dev_admin_password,
            client_secret: dev_client_secret,
        },
    )?;

    // Build a Config from sample(), patch the bind, keep
    // cookie_secure = false, set issuer to match the bind so
    // OIDC discovery returns a working URL.
    let mut cfg = Config::sample();
    cfg.server.listen_addr = dev_bind.clone();
    // The issuer needs an http(s):// prefix per validation; reuse
    // the bind for the host (this is a dev-mode-only scheme).
    cfg.server.issuer = format!("http://{dev_bind}");
    cfg.server.cookie_secure = false;
    // RFC 016: dev mode enables access logging by default so operators
    // can immediately see requests arriving in the terminal.
    cfg.log.access_log = true;
    // No persisted key file: the DB lives under an ephemeral
    // master key that this process generates. Keep storage paths
    // unset by pointing them at /dev/null-style placeholders that
    // the dev-mode flow does not read from.
    cfg.storage.db_path = std::path::PathBuf::from(
        dev_db
            .as_deref()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from(":memory:")),
    );

    // Open DB and seed.
    let db = dev_mode::open_dev_db(dev_db.as_deref())?;
    let setup_token = {
        use base64ct::Encoding;
        let mut buf = [0u8; 24];
        getrandom::fill(&mut buf).expect("system RNG unavailable");
        base64ct::Base64::encode_string(&buf)
    };
    dev_mode::print_dev_warnings(&dev_bind, &seed_source);
    let outcome = {
        let clock = sui_id_core::time::system_clock();
        dev_mode::apply_seed(&db, &clock, &setup_token, &seed).await?
    };
    dev_mode::print_seed_summary(&seed, &outcome, &dev_bind);
    let _ = outcome.admin_user_id; // captured for symmetry; not needed below.

    // Initialise tracing with dev defaults (access_log = true set above).
    // In normal mode startup::prepare does this; dev mode builds AppState
    // directly so we must call it explicitly here.
    let _log_guard = sui_id::startup::init_tracing(&cfg.log);

    // Build the AppState directly (we can't use startup::prepare
    // because it opens its own DB). Mirror its mailer + HIBP
    // construction; the in-memory mailer keeps the dev path
    // self-contained and offline.
    let mailer: std::sync::Arc<dyn sui_id_core::mail::MailSender> = std::sync::Arc::new(
        sui_id_core::mail::SmtpMailSender::new(db.clone(), String::from("sui-id-dev.local")),
    );
    let hibp_client: std::sync::Arc<dyn sui_id_core::hibp::HibpClient> =
        std::sync::Arc::new(sui_id_core::hibp::HttpHibpClient::new());
    let caches = std::sync::Arc::new(sui_id_core::cache::Caches::new());
    let mut state = sui_id::AppState::new(db, cfg, setup_token, mailer, hibp_client, caches);
    state.is_dev_mode = true; // RFC 032: enable browser-side dev banner

    let router = build_router(state.clone());
    sui_id::gc::spawn(state.clone());
    {
        let db = state.db.clone();
        tokio::spawn(async move {
            match sui_id_store::repos::refresh_tokens::backfill_token_hashes(&db).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(rows = n, "backfill: populated refresh_token.token_hash"),
                Err(e) => tracing::warn!(error = %e, "backfill: refresh_token.token_hash failed"),
            }
        });
    }

    let addr: std::net::SocketAddr = dev_bind
        .parse()
        .with_context(|| format!("invalid --dev-bind {dev_bind}"))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, "sui-id (dev mode) listening");

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("running dev server")?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("graceful shutdown initiated");
}
