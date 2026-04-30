//! sui-id binary entry point.
//!
//! Usage:
//!     sui-id [--config PATH]
//!     sui-id backup --to PATH [--config PATH]
//!     sui-id restore --from PATH [--config PATH] [--force]
//!     sui-id --print-sample-config
//!
//! With no `--config`, the program looks for `./sui-id.toml`.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use sui_id::{backup, build_router, config::Config, startup};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("sui-id {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
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
        Some("backup") => return run_backup_subcommand(&args),
        Some("restore") => return run_restore_subcommand(&args),
        Some("verify-backup") => return run_verify_backup_subcommand(&args),
        Some("admin") => return run_admin_subcommand(&args),
        Some(other) => bail!(
            "unknown subcommand {other:?}. Run `sui-id --help` for usage."
        ),
        None => {} // fall through to `serve`.
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
    let config_path = parse_config_path(args).unwrap_or_else(|| PathBuf::from("./sui-id.toml"));
    let cfg = Config::load(&config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;

    let startup = startup::prepare(cfg)?;
    let router = build_router(startup.state.clone());

    sui_id::gc::spawn(startup.state.clone());

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

fn run_backup_subcommand(args: &[String]) -> Result<()> {
    let dest = parse_named_path(args, "--to")
        .context("backup requires --to PATH")?;
    let config_path = parse_config_path(args).unwrap_or_else(|| PathBuf::from("./sui-id.toml"));
    let cfg = Config::load(&config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let opts = if args.iter().any(|a| a == "--encrypt") {
        let pass = read_passphrase("Encryption passphrase", true)?;
        backup::BackupOptions {
            passphrase: Some(pass),
        }
    } else {
        backup::BackupOptions::default()
    };
    backup::run_backup(&cfg, &dest, &opts)?;
    if opts.passphrase.is_some() {
        eprintln!(
            "encrypted backup written to {} (mode 0600). Store the passphrase \
             separately from the file — losing it makes the backup unrecoverable.",
            dest.display()
        );
    } else {
        eprintln!(
            "backup written to {} (mode 0600). The archive contains the master key; \
             treat it as a secret. For transport over an untrusted channel, use --encrypt.",
            dest.display()
        );
    }
    Ok(())
}

fn run_restore_subcommand(args: &[String]) -> Result<()> {
    let src = parse_named_path(args, "--from")
        .context("restore requires --from PATH")?;
    let config_path = parse_config_path(args).unwrap_or_else(|| PathBuf::from("./sui-id.toml"));
    let cfg = Config::load(&config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let force = args.iter().any(|a| a == "--force");
    let passphrase = if args.iter().any(|a| a == "--decrypt") {
        Some(read_passphrase("Decryption passphrase", false)?)
    } else {
        None
    };
    backup::run_restore(
        &cfg,
        &src,
        &backup::RestoreOptions { force, passphrase },
    )?;
    eprintln!(
        "restored from {} into {} and {}",
        src.display(),
        cfg.storage.db_path.display(),
        cfg.storage.key_file.display()
    );
    Ok(())
}

fn run_verify_backup_subcommand(args: &[String]) -> Result<()> {
    let src = parse_named_path(args, "--from")
        .context("verify-backup requires --from PATH")?;
    let passphrase = if args.iter().any(|a| a == "--decrypt") {
        Some(read_passphrase("Decryption passphrase", false)?)
    } else {
        None
    };
    let report = backup::run_verify(&src, passphrase.as_deref())?;
    println!("Format version: {}", report.manifest.format_version);
    println!("sui-id version: {}", report.manifest.sui_id_version);
    println!("Schema version: {}", report.manifest.schema_version);
    println!("Created at:     {}", report.manifest.created_at);
    println!("Hostname:       {}", report.manifest.hostname);
    println!("Issuer:         {}", report.manifest.issuer);
    println!("Encrypted:      {}", report.encrypted);
    println!("Tar size:       {} bytes", report.tar_bytes);
    println!("Database size:  {} bytes", report.db_bytes);
    println!(
        "Master key:     {}",
        if report.key_present { "present" } else { "MISSING" }
    );
    println!();
    println!("✓ SQLite integrity check passed");
    if report.encrypted {
        println!("✓ Decrypted with provided passphrase");
    }
    Ok(())
}

/// Dispatcher for `sui-id admin <subaction> ...`. Currently the only
/// admin subaction is `unlock-user`, but the surface is shaped to grow.
fn run_admin_subcommand(args: &[String]) -> Result<()> {
    let action = args.get(2).map(String::as_str);
    match action {
        Some("unlock-user") => run_admin_unlock_user(args),
        Some(other) => bail!(
            "unknown admin subaction `{other}`. Known subactions: unlock-user"
        ),
        None => bail!(
            "admin requires a subaction. Try: sui-id admin unlock-user --username NAME"
        ),
    }
}

/// `sui-id admin unlock-user --username NAME [--config PATH]`
///
/// Clears `failed_login_count` and `locked_until` on a user without
/// requiring a password verification — the operator's own
/// authority, witnessed by their access to the host's master key
/// material, is what authorises this. Used to recover a real user
/// who's been locked out by a typo storm or by a brute-force
/// attempt that exceeded the auto-unlock window.
fn run_admin_unlock_user(args: &[String]) -> Result<()> {
    let username = args
        .iter()
        .position(|a| a == "--username")
        .and_then(|i| args.get(i + 1))
        .context("admin unlock-user requires --username NAME")?;
    let config_path = parse_config_path(args).unwrap_or_else(|| PathBuf::from("./sui-id.toml"));
    let cfg = Config::load(&config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;

    // Open the database using the same key-resolution logic the
    // server uses (env var > file). No need to spin up the HTTP
    // layer or the clock; we read one row, write one row, exit.
    let resolved = sui_id::keyring::resolve(&cfg.storage.key_file)
        .context("loading master key")?;
    let db = sui_id_store::Database::open(&cfg.storage.db_path, resolved.key)
        .context("opening database")?;

    let user = sui_id_store::repos::users::find_by_username(&db, username)
        .with_context(|| format!("looking up user {username:?}"))?;
    sui_id_store::repos::users::admin_unlock(&db, user.id)
        .context("clearing lockout")?;
    // Mirror the operator-facing audit-log entry the live admin UI
    // would write for this action.
    let _ = sui_id_store::repos::audit::append(
        &db,
        &sui_id_store::models::AuditLogRow {
            at: chrono::Utc::now(),
            actor: None, // command-line operator; not a sui-id user
            action: "admin.user.unlock".into(),
            target: Some(user.id.to_string()),
            result: "ok".into(),
            note: Some(format!("unlocked via command line for username={username}")),
        },
    );
    eprintln!("unlocked {username} (id={})", user.id);
    Ok(())
}

/// Read a passphrase from stdin (interactive TTY) or from the
/// `SUI_ID_BACKUP_PASSPHRASE` environment variable (for cron and
/// scripted use). When `confirm` is true and we're on a TTY, the
/// passphrase is asked twice and rejected on mismatch.
fn read_passphrase(prompt: &str, confirm: bool) -> Result<String> {
    if let Ok(env) = std::env::var("SUI_ID_BACKUP_PASSPHRASE") {
        if !env.is_empty() {
            return Ok(env);
        }
    }
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut stderr = std::io::stderr();
    write!(stderr, "{prompt}: ").ok();
    stderr.flush().ok();
    let mut first = String::new();
    stdin.lock().read_line(&mut first).context("reading passphrase")?;
    let first = first.trim_end_matches(['\r', '\n']).to_string();
    if first.is_empty() {
        bail!("passphrase must not be empty");
    }
    if confirm {
        write!(stderr, "{prompt} (again): ").ok();
        stderr.flush().ok();
        let mut second = String::new();
        stdin.lock().read_line(&mut second).context("reading passphrase confirmation")?;
        let second = second.trim_end_matches(['\r', '\n']).to_string();
        if first != second {
            bail!("passphrases did not match");
        }
    }
    Ok(first)
}

fn parse_config_path(args: &[String]) -> Option<PathBuf> {
    parse_named_path(args, "--config")
}

fn parse_named_path(args: &[String], flag: &str) -> Option<PathBuf> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == flag {
            return iter.next().map(PathBuf::from);
        }
        let prefix = format!("{flag}=");
        if let Some(rest) = a.strip_prefix(&prefix) {
            return Some(PathBuf::from(rest));
        }
    }
    None
}

fn print_help() {
    println!(
        "sui-id {ver}

Self-hosted OpenID Connect provider.

USAGE:
    sui-id [--config PATH]
    sui-id backup --to PATH [--config PATH] [--encrypt]
    sui-id restore --from PATH [--config PATH] [--force] [--decrypt]
    sui-id verify-backup --from PATH [--decrypt]
    sui-id admin unlock-user --username NAME [--config PATH]
    sui-id --print-sample-config
    sui-id --version
    sui-id --help

SUBCOMMANDS:
    (none)                   Run the HTTP server.
    backup                   Write a tarball containing a SQLite-consistent
                             snapshot of the database, a copy of the master
                             key file, and a manifest describing the
                             provenance. The output file is mode 0600.
                             With --encrypt, the tarball is wrapped in an
                             XChaCha20-Poly1305 envelope keyed by an
                             Argon2id derivation of a passphrase you supply.
                             Use --encrypt for backups that will leave the
                             trust boundary of the host.
    restore                  Restore a backup tarball into the configured
                             storage paths. Refuses to overwrite existing
                             files unless --force is supplied. Use --decrypt
                             when restoring an encrypted backup.
    verify-backup            Read a backup file and report what it contains
                             without writing anything. Runs a SQLite
                             integrity check on the inner snapshot. Use
                             before a real restore to catch a corrupted or
                             mismatched backup.
    admin unlock-user        Clear an account-lockout state for the given
                             user. Resets the failed-login counter and
                             removes any active lock. Use to recover a real
                             user who's been locked out by a typo storm or
                             whose lockout window hasn't expired yet.

OPTIONS:
    --config PATH            Path to the TOML configuration file
                             (default: ./sui-id.toml)
    --to PATH                Output path for `backup`.
    --from PATH              Input path for `restore` / `verify-backup`.
    --username NAME          Target username for `admin unlock-user`.
    --force                  Allow `restore` to overwrite existing files.
    --encrypt                Encrypt the backup with a passphrase.
    --decrypt                Treat the input as an encrypted backup.
    --print-sample-config    Print a sample configuration and exit.
    --version, -V            Print version information and exit.
    --help, -h               Print this help and exit.

ENVIRONMENT:
    SUI_ID_MASTER_KEY        Base64-encoded 32-byte master key.
                             Overrides the key file if set.
    SUI_ID_BACKUP_PASSPHRASE Passphrase for `--encrypt` / `--decrypt`,
                             when running non-interactively (cron, scripts).

DOCUMENTATION:
    See README.md and docs/operators.md for the operator's guide.
",
        ver = env!("CARGO_PKG_VERSION")
    );
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
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
