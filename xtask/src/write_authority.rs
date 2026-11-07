//! RFC 094 condition 7: the `syn`-based write-authority boundary check.
//!
//! Cycle J stage 6 (measurement) found the dispatch's own three-module
//! allow-list estimate wrong and prototyped the detection design this
//! module builds: **type-path detection** (`rusqlite::Connection`/
//! `Transaction`/`Statement`, named qualified or import-resolved within
//! the same file) and **whole-identifier detection** (`with_conn`/
//! `with_tx`), both measured at zero false positives against this
//! codebase. Stage 6b's review direction: drop raw method-call-name
//! matching (`.execute(`, `.prepare(`, etc.) as a primary signal — it
//! cannot distinguish a call on `&ReadConn` (sanctioned, mirrors the
//! same method names) from one on a raw `&rusqlite::Connection`/
//! `Transaction` without resolving the receiver's type, which `syn`
//! alone cannot do. That gap, and exhaustive alias/generic-indirection
//! tracing beyond the one instance found (`backend.rs`'s `ConnFn`/
//! `TxFn`), are named here and in the policy file's own header — a
//! structural gate that checks less than its name implies is this RFC's
//! own named worst case.

use std::collections::BTreeSet;
use std::path::Path;

use syn::visit::{self, Visit};

const FORBIDDEN_TYPES: &[&str] = &["Connection", "Transaction", "Statement"];
const FORBIDDEN_IDENTS: &[&str] = &["with_conn", "with_tx"];

#[derive(Default)]
struct Finder {
    hits: Vec<(String, usize)>, // (description, line)
    imported: Vec<String>,
}

fn collect_use_tree(prefix: &str, tree: &syn::UseTree, out: &mut Vec<String>) {
    match tree {
        syn::UseTree::Path(p) => {
            let next = if prefix.is_empty() {
                p.ident.to_string()
            } else {
                format!("{prefix}::{}", p.ident)
            };
            collect_use_tree(&next, &p.tree, out);
        }
        syn::UseTree::Name(n) => {
            if prefix == "rusqlite" && FORBIDDEN_TYPES.contains(&n.ident.to_string().as_str()) {
                out.push(n.ident.to_string());
            }
        }
        syn::UseTree::Group(g) => {
            for t in &g.items {
                collect_use_tree(prefix, t, out);
            }
        }
        syn::UseTree::Rename(r) => {
            if prefix == "rusqlite" {
                out.push(r.rename.to_string());
            }
        }
        syn::UseTree::Glob(_) => {
            if prefix == "rusqlite" {
                out.push("*".to_owned());
            }
        }
    }
}

pub(crate) fn line_of(span: proc_macro2::Span) -> usize {
    span.start().line
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        collect_use_tree("", &node.tree, &mut self.imported);
        visit::visit_item_use(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        let segs: Vec<String> = node.segments.iter().map(|s| s.ident.to_string()).collect();
        if segs.len() >= 2 && segs[0] == "rusqlite" && FORBIDDEN_TYPES.contains(&segs[1].as_str()) {
            self.hits.push((
                format!("raw type `rusqlite::{}`", segs[1]),
                line_of(node.segments[0].ident.span()),
            ));
        } else if segs.len() == 1
            && (self.imported.contains(&segs[0]) || self.imported.contains(&"*".to_owned()))
            && FORBIDDEN_TYPES.contains(&segs[0].as_str())
        {
            self.hits.push((
                format!("raw type `{}` (imported from rusqlite)", segs[0]),
                line_of(node.segments[0].ident.span()),
            ));
        }
        visit::visit_path(self, node);
    }

    fn visit_ident(&mut self, node: &'ast proc_macro2::Ident) {
        let s = node.to_string();
        if FORBIDDEN_IDENTS.contains(&s.as_str()) {
            self.hits
                .push((format!("identifier `{s}`"), line_of(node.span())));
        }
        visit::visit_ident(self, node);
    }
}

pub struct Outcome {
    pub failures: Vec<String>,
    pub scanned: usize,
    pub allow_listed: usize,
}

/// `contracts/write-authority.toml`'s `allowed_files` list, exact (per
/// RFC 094: "changing it requires independent review") — no globs, no
/// directory prefixes, one entry per file.
pub fn load_allow_list(root: &Path, policy_rel: &Path) -> anyhow::Result<BTreeSet<String>> {
    let path = root.join(policy_rel);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", policy_rel.display()))?;
    let data: toml::Table = toml::from_str(&text)
        .map_err(|e| anyhow::anyhow!("cannot parse {}: {e}", policy_rel.display()))?;
    let list = data
        .get("allowed_files")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("{}: no `allowed_files` array", policy_rel.display()))?;
    list.iter()
        .map(|v| {
            v.as_str().map(str::to_owned).ok_or_else(|| {
                anyhow::anyhow!("{}: `allowed_files` must be strings", policy_rel.display())
            })
        })
        .collect()
}

fn scan_file(path: &Path) -> anyhow::Result<Vec<(String, usize)>> {
    let src = std::fs::read_to_string(path)?;
    let file = syn::parse_file(&src).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    let mut finder = Finder::default();
    finder.visit_file(&file);
    Ok(finder.hits)
}

pub fn check(root: &Path, policy_rel: &Path) -> anyhow::Result<Outcome> {
    let allow_list = load_allow_list(root, policy_rel)?;
    let mut failures = Vec::new();
    let mut scanned = 0usize;
    let mut allow_listed = 0usize;

    for path in crate::write_commands::source_files(root) {
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        scanned += 1;
        let is_allowed = allow_list.contains(&rel_str);
        if is_allowed {
            allow_listed += 1;
            continue;
        }
        let hits = scan_file(&path)?;
        for (desc, line) in hits {
            failures.push(format!(
                "(write-authority) {rel_str}:{line}: {desc} outside the allow-listed \
                 module set (contracts/write-authority.toml)"
            ));
        }
    }

    // Every allow-listed path must actually exist and be a production
    // file, so the list cannot accumulate dead entries unnoticed -- the
    // same "stale exemption is a violation" principle G18 already uses.
    for entry in &allow_list {
        if !root.join(entry).is_file() {
            failures.push(format!(
                "(write-authority) allowed_files names {entry:?}, which does not exist"
            ));
        }
    }

    Ok(Outcome {
        failures,
        scanned,
        allow_listed,
    })
}

/// `--negative-fixtures`: RFC 094's mandatory negative fixture — a new
/// repository-shaped function calling `Connection::execute("UPDATE …")`
/// with no declaration, nowhere near the allow-list. Two controls, per
/// stage 6b's settled reading of the RFC's own (self-inconsistent)
/// sentence: the fixture package must `cargo check` cleanly (it is
/// valid Rust; the compiler does not, and cannot, reject an unregistered
/// write — that is why this gate exists at all), and this scan must
/// reject it, by name, as an unregistered write.
pub fn run_negative_fixtures(root: &Path) -> std::process::ExitCode {
    let fixture_dir = root.join("scripts/tests/fixtures/write-authority/unregistered-write");
    let src = fixture_dir.join("src/lib.rs");

    eprintln!(
        "audit-structure --negative-fixtures: cargo check {}",
        fixture_dir.display()
    );
    let check = std::process::Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(fixture_dir.join("Cargo.toml"))
        .output();
    match check {
        Ok(out) if out.status.success() => {
            eprintln!("  cargo check: ok (the fixture is valid Rust, as it must be)");
        }
        Ok(out) => {
            eprintln!(
                "audit-structure --negative-fixtures: FAIL: the fixture must compile \
                 cleanly -- a fixture that fails to build proves nothing. stderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            );
            return std::process::ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("audit-structure --negative-fixtures: FAIL: could not run cargo check: {e}");
            return std::process::ExitCode::FAILURE;
        }
    }

    let hits = match scan_file(&src) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("audit-structure --negative-fixtures: FAIL: could not scan fixture: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if hits.is_empty() {
        eprintln!(
            "audit-structure --negative-fixtures: FAIL: the fixture's unregistered write \
             was not rejected -- the gate passed on a codebase it should have flagged"
        );
        return std::process::ExitCode::FAILURE;
    }
    eprintln!(
        "  audit-structure rejected it, by name, as an unregistered write: {:?}",
        hits
    );
    eprintln!("audit-structure --negative-fixtures: PASS (both controls exercised)");
    std::process::ExitCode::SUCCESS
}
