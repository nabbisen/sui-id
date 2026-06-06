//! Startup sequence: configure logging, resolve the master key, open the
//! database, generate (or print) the setup token, and hand back a ready-to-
//! mount [`AppState`] plus the listen address.

use crate::config::Config;
use crate::keyring::{self, KeyOrigin};
use crate::AppState;
use sui_id_core::time::system_clock;
use anyhow::{Context, Result};
use base64ct::{Base64, Encoding};
use rand::rngs::OsRng;
use rand::RngCore;
use std::sync::Once;
use sui_id_store::repos::state;
use sui_id_store::Database;
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
pub fn init_tracing(cfg: &crate::config::LogConfig) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let mut guard = None;
    INIT_TRACING.call_once(|| {
        let env_filter = EnvFilter::try_new(&cfg.filter)
            .unwrap_or_else(|_| EnvFilter::new("info"));

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
    if let Some(parent) = cfg.storage.db_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    let db = Database::open(&cfg.storage.db_path, resolved.key)
        .context("opening database")?;

    // Verify the tail of the audit-log hash chain. A mismatch here
    // indicates DB-level tampering since the most recent restart;
    // an attacker who modified an audit row directly via SQL will
    // have left the row's hash mismatching its recomputation.
    //
    // We don't refuse to start on detection — that would let an
    // attacker DoS the IdP by corrupting one row — but we surface
    // the finding loudly so an operator's monitoring catches it.
    match sui_id_store::repos::audit::verify_chain_tail(&db, AUDIT_VERIFY_TAIL).await {
        Ok(report) => {
            if let Some(seq) = report.broken_at_seq {
                tracing::error!(
                    broken_at_seq = seq,
                    checked = report.checked,
                    legacy_unhashed = report.legacy_unhashed,
                    "audit-log hash-chain verification FAILED — tampering or DB corruption suspected"
                );
            } else {
                tracing::info!(
                    checked = report.checked,
                    legacy_unhashed = report.legacy_unhashed,
                    "audit-log hash chain verified"
                );
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "audit-log chain verification could not run");
        }
    }

    // 3. Generate setup token if the system is uninitialized; print it once.
    let setup_token = generate_setup_token();
    let initialized = state::is_initialized(&db).unwrap_or(false);
    if !initialized {
        // Use eprintln (stderr) per spec: "管理者操作の無監査化を防ぐ" + "失敗時に内部情報を返しすぎない"
        // The token never enters the structured tracing pipeline so it does not
        // leak into log aggregators by accident.
        eprintln!("\n  =====================================================");
        eprintln!("  sui-id has not been initialized yet.");
        eprintln!("  Open  {}/setup", cfg.server.issuer.trim_end_matches('/'));
        eprintln!("  Setup token (one-time, stays only in this process):");
        eprintln!("    {setup_token}");
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
    let outbox_sender: std::sync::Arc<dyn sui_id_core::mail::MailSender> =
        std::sync::Arc::new(sui_id_core::mail::outbox::OutboxMailSender::new(
            db.clone(),
            system_clock(),
        ));
    let state = AppState::new(db, cfg, setup_token, outbox_sender, hibp_client, caches);
    Ok(Startup { state, listen_addr, _log_guard: log_guard })
}

fn generate_setup_token() -> String {
    let mut buf = [0u8; 24];
    OsRng.fill_bytes(&mut buf);
    let mut out = vec![0u8; 64];
    let n = Base64::encode(&buf, &mut out)
        .map(str::len)
        .unwrap_or(0);
    out.truncate(n);
    String::from_utf8(out).unwrap_or_else(|_| "ERROR_GENERATING_TOKEN".into())
}
