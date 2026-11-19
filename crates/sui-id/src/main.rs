//! sui-id binary entry point.
//!
//! RFC 148 cycle O2a: this file is a thin shell. The command registry,
//! its dispatch, help, and the generated CLI reference all live in
//! `sui_id::cli` (part of the library target, so `xtask`-free tooling and
//! this binary both reach it) — see `crates/sui-id/src/cli.rs`'s own doc
//! comment for the registry itself.

/// The process entry point. It exists to give a refused database its own
/// exit code (RFC 112 D5): the binary used to return `anyhow::Result`
/// from `main`, so every failure was `1` and printed under `Error:`. The
/// convention is two lines: `0` success; `65` the database is not one
/// this build can use; `1` everything else, printed exactly as before.
#[tokio::main]
async fn main() -> std::process::ExitCode {
    // Must run before any subcommand or code path that could open a TLS
    // connection (LDAPS via ldap3, implicit-TLS SMTP via wasm-smtp-tokio,
    // or reqwest for federation/HIBP) — see the doc comment on this
    // function for why the choice of provider matters, not just that one
    // is installed.
    sui_id::startup::install_rustls_crypto_provider();

    let args: Vec<String> = std::env::args().collect();
    match sui_id::cli::run(args).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => match err.downcast_ref::<sui_id::database::DatabaseRefused>() {
            Some(refused) => {
                eprintln!("{}", refused.line());
                std::process::ExitCode::from(sui_id::database::EXIT_DATABASE_REFUSED)
            }
            None => match err.downcast_ref::<sui_id::database::PreMigrationBackupRefused>() {
                // RFC 142 stage 5 §5: a disk problem, not a schema one — exit
                // 1 like any other failure, but the bare line, not wrapped in
                // "Error: ...", since the check that refused wrote nothing.
                Some(refused) => {
                    eprintln!("{}", refused.line());
                    std::process::ExitCode::FAILURE
                }
                None => {
                    eprintln!("Error: {err:?}");
                    std::process::ExitCode::FAILURE
                }
            },
        },
    }
}
