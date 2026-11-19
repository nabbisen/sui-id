//! RFC 148 §4 (cycle O2b §2/§3): the first run is a conversation, but
//! only in a terminal. Scoped to bare `sui-id` and `sui-id serve` only —
//! every other command that needs a configuration keeps printing
//! [`crate::config::missing_config_guidance`] through the ordinary,
//! `anyhow`-wrapped `Config::load` path, unchanged in shape.

use std::io::IsTerminal as _;
use std::path::Path;

use crate::database::PreMigrationBackup;

/// Pure: every input the first-run choice depends on, named — so each of
/// the eight combinations below is one test case, not an integration
/// scenario that happens to exercise one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decision {
    /// A configuration exists at the resolved path — serve it normally.
    Proceed,
    /// No configuration, in a terminal, with no explicit `--config` —
    /// ask.
    Prompt,
    /// No configuration, and either not a terminal or `--config` was
    /// explicit (RFC 148 §6 answer 3: an explicit path is a deliberate
    /// choice, so the answer is an error that names it, never a prompt).
    /// Print the guidance and exit 1, never waiting.
    GuidanceAndExit,
}

pub(crate) fn decide(explicit_config: bool, config_exists: bool, is_terminal: bool) -> Decision {
    if config_exists {
        return Decision::Proceed;
    }
    if explicit_config || !is_terminal {
        return Decision::GuidanceAndExit;
    }
    Decision::Prompt
}

/// Both must be a terminal: stdin, to read an answer from, and stderr,
/// where the question and every other diagnostic this binary prints go.
pub(crate) fn stdin_and_stderr_are_terminals() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Bare `sui-id [--config PATH]` or `sui-id serve [--config PATH]`.
/// Decides before `Config::load` ever runs, so the no-terminal and
/// explicit-path-missing cases never wait on anything (RFC 148 §4).
pub(crate) async fn serve_or_first_run(
    cfg_path: &Path,
    explicit_config: bool,
    policy: PreMigrationBackup,
) -> anyhow::Result<()> {
    match decide(
        explicit_config,
        cfg_path.exists(),
        stdin_and_stderr_are_terminals(),
    ) {
        Decision::Proceed => super::handlers::serve(cfg_path, policy).await,
        Decision::Prompt => run_prompt(cfg_path),
        Decision::GuidanceAndExit => exit_with_guidance(cfg_path),
    }
}

fn exit_with_guidance(cfg_path: &Path) -> anyhow::Result<()> {
    eprintln!(
        "error: {}",
        crate::config::missing_config_guidance(cfg_path)
    );
    std::process::exit(1);
}

/// RFC 148 §4's own example conversation. `l`/`p` write through
/// [`super::handlers::run_init`] (the same path `sui-id init` itself
/// uses — one function, so this and `init` can never disagree) and
/// print its own "wrote ... / Next: ..." success message; the first-run
/// flow adds nothing after that, and does **not** go on to serve (RFC
/// 148 §4: "so the operator can read and edit the file first"). `n`, an
/// empty answer, or end of input is "not now".
fn run_prompt(cfg_path: &Path) -> anyhow::Result<()> {
    use std::io::Write as _;

    eprintln!(
        "sui-id {} — no configuration found at {}.",
        env!("CARGO_PKG_VERSION"),
        cfg_path.display()
    );
    eprintln!();
    eprintln!("  sui-id needs a configuration file to start. Create one now?");
    eprintln!("    [l] a local trial  — http://127.0.0.1:8801, data in this directory");
    eprintln!("    [p] a production template — HTTPS issuer, /var/lib/sui-id; edit before use");
    eprintln!("    [n] not now");
    eprintln!();
    let mut stderr = std::io::stderr();
    write!(stderr, "  choice [l/p/n]: ").ok();
    stderr.flush().ok();

    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok();
    match line.trim().to_ascii_lowercase().as_str() {
        "l" => super::handlers::run_init(cfg_path, false),
        "p" => super::handlers::run_init(cfg_path, true),
        _ => exit_with_guidance(cfg_path),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "first_run/tests.rs"]
mod tests;
