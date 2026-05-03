//! Startup sequence: configure logging, resolve the master key, open the
//! database, generate (or print) the setup token, and hand back a ready-to-
//! mount [`AppState`] plus the listen address.

use crate::config::Config;
use crate::keyring::{self, KeyOrigin};
use crate::AppState;
use anyhow::{Context, Result};
use base64ct::{Base64, Encoding};
use rand::rngs::OsRng;
use rand::RngCore;
use std::sync::Once;
use sui_id_store::repos::state;
use sui_id_store::Database;
use tracing_subscriber::EnvFilter;

pub struct Startup {
    pub state: AppState,
    pub listen_addr: String,
}

static INIT_TRACING: Once = Once::new();

/// How many rows from the tail of the audit log to chain-verify on
/// startup. A few thousand catches anything an attacker is likely
/// to have modified in a recent intrusion; deeper sweeps belong in
/// a scheduled background task or a dedicated CLI subcommand.
const AUDIT_VERIFY_TAIL: i64 = 5_000;

pub fn init_tracing(filter: &str, json: bool) {
    INIT_TRACING.call_once(|| {
        let env_filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"));
        let builder = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_level(true);
        if json {
            builder.json().init();
        } else {
            builder.init();
        }
    });
}

pub fn prepare(cfg: Config) -> Result<Startup> {
    init_tracing(&cfg.log.filter, cfg.log.format == "json");

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
    match sui_id_store::repos::audit::verify_chain_tail(&db, AUDIT_VERIFY_TAIL) {
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
    // Build the production mail sender. It captures references to
    // the database and the master key (cloned, both Arc-shaped
    // internally) so it can read the live `smtp_config` row on
    // every send.
    let mailer: std::sync::Arc<dyn sui_id_core::mail::MailSender> = std::sync::Arc::new(
        sui_id_core::mail::SmtpMailSender::new(
            db.clone(),
            ehlo_hostname_from_issuer(&cfg.server.issuer),
        ),
    );
    // HIBP (Pwned Passwords) client. Constructed unconditionally
    // — the call site short-circuits when `server_settings.hibp_mode
    // = 'off'`, so a deployment that never wants outbound HIBP
    // traffic still pays only the cost of one Arc::new at startup.
    let hibp_client: std::sync::Arc<dyn sui_id_core::hibp::HibpClient> =
        std::sync::Arc::new(sui_id_core::hibp::HttpHibpClient::new());
    let state = AppState::new(db, cfg, setup_token, mailer, hibp_client);
    Ok(Startup { state, listen_addr })
}

fn ehlo_hostname_from_issuer(issuer: &str) -> String {
    // Best-effort: parse the issuer URL and pick its host. If it
    // doesn't parse, fall back to the issuer string itself —
    // wrong-looking but harmless (the SMTP server logs it; no
    // delivery decision rests on it for plain SMTP relays).
    url::Url::parse(issuer)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| "sui-id.local".to_owned())
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
