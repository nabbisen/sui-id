//! `cargo xtask audit-structure` — RFC 094's structural gate. Stage 6b
//! replaced G17 (`scripts/check-write-commands.py`) and added the `syn`
//! write-authority boundary condition 7 names. Stage 7 added the audit-
//! coverage check (RFC 094 `:1105-1107` item 6's third clause), built on
//! the same typed AST paths rather than G13's string extraction. Stage 7c
//! moves G13's second step (RFC 116 stage 2's `class`/`actor`/`step_up`
//! column check, `scripts/check-audit-matrix-columns.py`, deleted by this
//! change) onto the same typed statics, for the same reason: a text read
//! cannot see a type. `scripts/check-audit-matrix.sh` (G13) remains
//! registered for vocabulary-drift diagnosis, per RFC 094's own two
//! endings for it — retiring it is an amendment, not this stage's to make.

mod columns;
mod coverage;
mod manifest;
mod matrix_table;
mod write_authority;
mod write_commands;
mod write_sites;

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
                "xtask: usage: cargo xtask audit-structure --policy PATH --commands PATH --matrix PATH [--fix] [--negative-fixtures]"
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
    let matrix = parse_flag_value(&args, "--matrix")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("contracts/audit-coverage-matrix.md"));

    if negative_fixtures {
        let write_authority_result = write_authority::run_negative_fixtures(&root);
        if write_authority_result != std::process::ExitCode::SUCCESS {
            return write_authority_result;
        }
        return write_commands::run_test_id_negative_fixture(&root);
    }

    let mut failures: Vec<String> = Vec::new();

    let code = match write_commands::run(&root, &commands, fix) {
        Ok(outcome) => {
            failures.extend(outcome.failures);
            if let Some(summary) = &outcome.summary {
                eprintln!("audit-structure: write-commands: {summary}");
            }
            outcome.code
        }
        Err(e) => {
            eprintln!("audit-structure: {e}");
            return std::process::ExitCode::from(2);
        }
    };

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

    match manifest::check(&root) {
        Ok(outcome) => {
            failures.extend(outcome.failures);
            eprintln!(
                "audit-structure: manifest: {} Cargo.toml(s) checked. `rusqlite` permitted \
                 only in sui-id-store's [dependencies]/[build-dependencies]; \
                 [dev-dependencies] and [workspace.dependencies] are allowed everywhere \
                 (RFC 094 §F3, production reach).",
                outcome.manifests_checked
            );
        }
        Err(e) => {
            eprintln!("audit-structure: {e}");
            return std::process::ExitCode::from(2);
        }
    }

    match write_sites::check(&root, &policy, &commands) {
        Ok(outcome) => {
            failures.extend(outcome.failures);
            eprintln!(
                "audit-structure: write-sites: {} file(s) checked, {} exempt (write by \
                 design), {} function(s) exempt by name, {} function(s) flagged.",
                outcome.checked_files,
                outcome.exempt_files,
                outcome.exempt_functions,
                outcome.flagged.len()
            );
            for f in &outcome.flagged {
                eprintln!("audit-structure: write-sites: flagged {f}");
            }
        }
        Err(e) => {
            eprintln!("audit-structure: {e}");
            return std::process::ExitCode::from(2);
        }
    }

    match coverage::check(&root, &matrix) {
        Ok(outcome) => {
            failures.extend(outcome.failures);
            eprintln!(
                "audit-structure: coverage: {} matrix entries, {} source names \
                 (EventDescriptor/MustAttemptDescriptor statics, events.rs-shaped \
                 name() match arms, AuditLogRow{{action: \"literal\"}} sites). \
                 Not resolved: a name reached only through a named const/static \
                 referenced at the call site with no typed descriptor alongside \
                 it. G13 (scripts/check-audit-matrix.sh) remains registered for \
                 vocabulary-drift diagnosis; this is the coverage authority.",
                outcome.matrix_count, outcome.source_count
            );
        }
        Err(e) => {
            eprintln!("audit-structure: {e}");
            return std::process::ExitCode::from(2);
        }
    }

    match columns::check(&root, &matrix, &code) {
        Ok(outcome) => {
            failures.extend(outcome.failures);
            eprintln!(
                "audit-structure: columns: {} (class/actor/step_up read from \
                 EventDescriptor/MustAttemptDescriptor statics, not from text; \
                 reachability from write-commands' own command map). G13 \
                 (scripts/check-audit-matrix.sh) no longer runs this check.",
                outcome.summary
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
