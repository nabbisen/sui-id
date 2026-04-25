//! sui-id binary entry point.
//!
//! Usage:
//!     sui-id [--config PATH]
//!     sui-id --print-sample-config
//!
//! With no `--config`, the program looks for `./sui-id.toml`.

use anyhow::{Context, Result};
use std::path::PathBuf;
use sui_id::{build_router, config::Config, startup};

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

    let config_path = parse_config_path(&args).unwrap_or_else(|| PathBuf::from("./sui-id.toml"));
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

fn parse_config_path(args: &[String]) -> Option<PathBuf> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == "--config" {
            return iter.next().map(PathBuf::from);
        }
        if let Some(rest) = a.strip_prefix("--config=") {
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
    sui-id --print-sample-config
    sui-id --version
    sui-id --help

OPTIONS:
    --config PATH            Path to the TOML configuration file
                             (default: ./sui-id.toml)
    --print-sample-config    Print a sample configuration and exit
    --version, -V            Print version information and exit
    --help, -h               Print this help and exit

ENVIRONMENT:
    SUI_ID_MASTER_KEY        Base64-encoded 32-byte master key.
                             Overrides the key file if set.

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
