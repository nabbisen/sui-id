//! `cargo xtask audit-structure` — RFC 094's structural gate (cycle J
//! stage 6b). Replaces G17 (`scripts/check-write-commands.py`, retired in
//! this change) and adds the `syn` write-authority boundary condition 7
//! names.

mod write_authority;
mod write_commands;

use std::path::PathBuf;

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("audit-structure") => run_audit_structure(&args[1..]),
        Some(other) => {
            eprintln!("xtask: unknown subcommand {other:?}");
            std::process::ExitCode::FAILURE
        }
        None => {
            eprintln!(
                "xtask: usage: cargo xtask audit-structure --policy PATH --commands PATH [--fix] [--negative-fixtures]"
            );
            std::process::ExitCode::FAILURE
        }
    }
}

fn run_audit_structure(args: &[String]) -> std::process::ExitCode {
    let args: Vec<String> = args.to_vec();
    // `--locked`: accepted and currently a no-op. This tool makes no
    // network or registry call and resolves nothing from crates.io -- it
    // only reads files already on disk -- so there is nothing for it to
    // lock against. Accepted for consistency with the RFC's own quoted
    // invocation and so a caller passing it does not get an "unknown
    // flag" error.
    let _locked = has_flag(&args, "--locked");
    let fix = has_flag(&args, "--fix");
    let negative_fixtures = has_flag(&args, "--negative-fixtures");

    let root = parse_flag_value(&args, "--root")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let policy = parse_flag_value(&args, "--policy")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("contracts/write-authority.toml"));
    let commands = parse_flag_value(&args, "--commands")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("contracts/write-commands.toml"));

    if negative_fixtures {
        return write_authority::run_negative_fixtures(&root);
    }

    let mut failures: Vec<String> = Vec::new();

    match write_commands::run(&root, &commands, fix) {
        Ok(outcome) => {
            failures.extend(outcome.failures);
            if let Some(summary) = &outcome.summary {
                eprintln!("audit-structure: write-commands: {summary}");
            }
        }
        Err(e) => {
            eprintln!("audit-structure: {e}");
            return std::process::ExitCode::from(2);
        }
    }

    match write_authority::check(&root, &policy) {
        Ok(outcome) => {
            failures.extend(outcome.failures);
            eprintln!(
                "audit-structure: write-authority: {} file(s) scanned, {} allow-listed. \
                 Deliberately not checked (named, not silent): raw method-call-name \
                 matching without receiver-type resolution, and alias/generic-indirection \
                 tracing beyond the allow-listed executor cluster. See \
                 contracts/write-authority.toml's header.",
                outcome.scanned, outcome.allow_listed
            );
        }
        Err(e) => {
            eprintln!("audit-structure: {e}");
            return std::process::ExitCode::from(2);
        }
    }

    if !failures.is_empty() {
        for line in &failures {
            eprintln!("audit-structure: {line}");
        }
        eprintln!(
            "audit-structure: failed with {} violation(s)",
            failures.len()
        );
        return std::process::ExitCode::FAILURE;
    }

    eprintln!("audit-structure: all conditions satisfied");
    std::process::ExitCode::SUCCESS
}
