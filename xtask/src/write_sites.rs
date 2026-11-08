//! RFC 094 M2b / cycle J stage 10 §1 — the write site, not only the
//! module.
//!
//! `write_authority::check` enforces **where** a raw write handle may
//! exist (the allow-listed file set). This module enforces **who** must
//! be named for holding one: inside an allow-listed file, every function
//! — a free `fn`, an `impl` method, and any closure in its body (a
//! closure is not separately nameable in `module::function` form, so a
//! write inside one counts toward its enclosing named function) — that
//! contains a **write call** must be named, as `module::function`, in
//! some row's `mutation_surface` (`contracts/write-commands.toml`).
//!
//! ## What counts as a write call, and why
//!
//! Measured against every allow-listed file's actual method-call names
//! (cycle J stage 10 dispatch §1, itself drawn from stage 6's own list —
//! statement execution, `execute`/`batch`/transaction calls, writable
//! `PRAGMA`): **`execute`, `execute_batch`, `transaction`,
//! `transaction_with_behavior`, `unchecked_transaction`, `pragma_update`,
//! `pragma_update_and_check`.** `execute_batch` also covers `VACUUM INTO`
//! and `ATTACH`/`DETACH` — those are string content inside the one call
//! this already locates, not a separate call shape, the same reasoning
//! `write-authority.toml`'s own header gives for not enumerating them
//! separately. `pragma_update`/`pragma_update_and_check` are never reads
//! in `rusqlite`'s own API — `pragma_query`/a `query_row` over `PRAGMA …`
//! are the read forms, and are not in this list, along with `query_map`
//! and a `prepare` used only for reads, per the dispatch's explicit
//! exclusion.
//!
//! **Deliberately not independent signals:** `commit`, `rollback`, and
//! the two backend test-instrumentation methods
//! (`fail_next_commit_for_test`, `commit_hook`). A function that commits
//! or rolls back a transaction without itself calling `execute`/
//! `execute_batch` anywhere in the same scope writes nothing on its own —
//! every real write already surfaces through one of the seven names
//! above, measured against this codebase's own call sites rather than
//! assumed.
//!
//! **Why method-name matching is safe here, when `write_authority.rs`'s
//! own header says it is not safe at module scope:** that gate's
//! ambiguity is codebase-wide — `.execute(` could be called on the
//! sanctioned `ReadConn`, which deliberately mirrors `rusqlite`'s
//! read-method names. This check runs only *inside* the allow-listed set,
//! where `ReadConn`'s entire public surface (`read_conn.rs`) is
//! `prepare`/`query_row`/`query_map` — none of the seven names above — so
//! no receiver-type resolution is needed to rule out a false positive
//! from it here.
//!
//! ## Naming convention
//!
//! `module` is the file stem (`users::set_disabled_within_tx`), matching
//! how rows already write it. A row that instead writes
//! `repos::users::set_disabled_within_tx` is also accepted: matching is
//! substring containment of the bare `module::function` form inside the
//! combined `mutation_surface` text of every row, so a longer qualified
//! prefix still matches.
//!
//! ## Exemptions
//!
//! `migrations.rs` and `backup/ops.rs` write by design (the migration
//! runner; the pre-migration and operator-invoked backup snapshot
//! machinery) and are exempted whole-file from this specific check —
//! named in `contracts/write-authority.toml`'s own
//! `write_by_design_exempt` list, with the reason, never inline here.
//! They remain subject to `write_authority::check`'s module-boundary
//! rule; only this function-level rule treats them differently.
//!
//! ## What is out of scope, same as the boundary check
//!
//! `#[cfg(test)]` functions and modules are skipped — the AST is simply
//! not descended into them — matching `write_authority.rs`'s own
//! scope (production code only).

use std::collections::BTreeSet;
use std::path::Path;

use syn::visit::{self, Visit};

/// Measured against this codebase's own allow-listed files (module doc
/// above): every `rusqlite` call that executes a statement, opens a
/// transaction, or sets a `PRAGMA`.
const WRITE_METHODS: &[&str] = &[
    "execute",
    "execute_batch",
    "transaction",
    "transaction_with_behavior",
    "unchecked_transaction",
    "pragma_update",
    "pragma_update_and_check",
];

/// Methods that take SQL text and normally only read, but execute whatever
/// statement they are given: `query_row("UPDATE … RETURNING …")` writes.
/// A call to one of these counts as a write call when its first argument
/// is a string literal whose statement writes ([`literal_sql_writes`]).
/// Measured cycle J stage 10 review: `federation_mfa_pending::
/// increment_failure_within_tx` and `users::increment_mfa_failure_within_tx`
/// both write through `query_row`, invisible to [`WRITE_METHODS`] alone.
/// **Not inspected**: SQL bound to a variable or built at runtime (e.g.
/// `format!`) before reaching one of these. That is a reviewer's to catch;
/// this check is a net against mistakes, not against a determined author.
const SQL_TAKING_METHODS: &[&str] = &[
    "query_row",
    "query_row_and_then",
    "query_one",
    "query_map",
    "query",
    "prepare",
    "prepare_cached",
];

/// Whether `sql` is a statement that writes: it starts with `INSERT`,
/// `UPDATE`, `DELETE` or `REPLACE`, or is a `WITH …` CTE containing one of
/// them as a word.
fn literal_sql_writes(sql: &str) -> bool {
    const WRITERS: &[&str] = &["INSERT", "UPDATE", "DELETE", "REPLACE"];
    let upper = sql.trim_start().to_ascii_uppercase();
    let first = upper
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .find(|w| !w.is_empty())
        .unwrap_or("");
    if WRITERS.contains(&first) {
        return true;
    }
    first == "WITH"
        && upper
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .any(|w| WRITERS.contains(&w))
}

/// Split a `cfg` predicate's top-level, comma-separated items, respecting
/// parentheses.
fn split_top_level(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Whether a `cfg` predicate (whitespace removed) can hold **only** in a
/// test build: `test`, the `test-support` feature (never enabled in a
/// production build), `any(…)` of only such items, or `all(…)` containing
/// one. **`not(test)` is not test-only, it is production-only**, which is
/// what the earlier substring test (`contains("test")`) got backwards: a
/// `#[cfg(not(test))]` writer was skipped exactly in the builds that ship.
fn predicate_is_test_only(p: &str) -> bool {
    if p == "test" || p == "feature=\"test-support\"" {
        return true;
    }
    if let Some(inner) = p.strip_prefix("any(").and_then(|r| r.strip_suffix(')')) {
        let items = split_top_level(inner);
        return !items.is_empty() && items.iter().all(|i| predicate_is_test_only(i));
    }
    if let Some(inner) = p.strip_prefix("all(").and_then(|r| r.strip_suffix(')')) {
        return split_top_level(inner)
            .iter()
            .any(|i| predicate_is_test_only(i));
    }
    false
}

fn has_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("cfg") {
            return false;
        }
        match &a.meta {
            syn::Meta::List(list) => {
                let p: String = list
                    .tokens
                    .to_string()
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                predicate_is_test_only(&p)
            }
            _ => false,
        }
    })
}

/// Whether `qualified` (`module::function`) is named in `haystack` **as a
/// whole identifier path**: not followed by an identifier character, and
/// not preceded by one. A plain substring test let a new `users::set_disabled`
/// pass because `users::set_disabled_within_tx` is named (cycle J stage 10
/// review, probe C).
fn is_named(haystack: &str, qualified: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    haystack.match_indices(qualified).any(|(i, m)| {
        let before = haystack[..i].chars().next_back();
        let after = haystack[i + m.len()..].chars().next();
        !before.is_some_and(ident) && !after.is_some_and(ident)
    })
}

#[derive(Default)]
struct WriteSiteVisitor {
    stack: Vec<String>,
    hits: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for WriteSiteVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if has_cfg_test(&node.attrs) {
            return;
        }
        self.stack.push(node.sig.ident.to_string());
        visit::visit_item_fn(self, node);
        self.stack.pop();
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if has_cfg_test(&node.attrs) {
            return;
        }
        visit::visit_item_mod(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if has_cfg_test(&node.attrs) {
            return;
        }
        self.stack.push(node.sig.ident.to_string());
        visit::visit_impl_item_fn(self, node);
        self.stack.pop();
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        let writes = WRITE_METHODS.contains(&method.as_str())
            || (SQL_TAKING_METHODS.contains(&method.as_str())
                && matches!(
                    node.args.first(),
                    Some(syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }))
                        if literal_sql_writes(&s.value())
                ));
        if writes && let Some(owner) = self.stack.last() {
            self.hits.insert(owner.clone());
        }
        visit::visit_expr_method_call(self, node);
    }
}

/// Every function in `path` (free `fn`s and `impl` methods, including
/// write calls inside a closure in their body) that contains a write
/// call, by its own bare name (not yet qualified with the module).
fn writing_functions(path: &Path) -> anyhow::Result<BTreeSet<String>> {
    let src = std::fs::read_to_string(path)?;
    let file = syn::parse_file(&src).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    let mut visitor = WriteSiteVisitor::default();
    visitor.visit_file(&file);
    Ok(visitor.hits)
}

/// `write-authority.toml`'s `write_by_design_exempt` list: allow-listed
/// files that write by design and are exempt from this specific check.
pub fn load_write_by_design_exempt(
    root: &Path,
    policy_rel: &Path,
) -> anyhow::Result<BTreeSet<String>> {
    let path = root.join(policy_rel);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", policy_rel.display()))?;
    let data: toml::Table = toml::from_str(&text)
        .map_err(|e| anyhow::anyhow!("cannot parse {}: {e}", policy_rel.display()))?;
    let list = match data.get("write_by_design_exempt") {
        Some(v) => v.as_array().ok_or_else(|| {
            anyhow::anyhow!(
                "{}: `write_by_design_exempt` must be an array",
                policy_rel.display()
            )
        })?,
        None => return Ok(BTreeSet::new()),
    };
    list.iter()
        .map(|v| {
            v.as_str().map(str::to_owned).ok_or_else(|| {
                anyhow::anyhow!(
                    "{}: `write_by_design_exempt` must be strings",
                    policy_rel.display()
                )
            })
        })
        .collect()
}

/// Every `[[command]]` row's raw `mutation_surface` text, concatenated —
/// the one place write-site names are recorded, read as text because the
/// inventory writes free prose with backtick-quoted references, not a
/// structured list.
fn mutation_surface_text(root: &Path, commands_rel: &Path) -> anyhow::Result<String> {
    let path = root.join(commands_rel);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", commands_rel.display()))?;
    let data: toml::Table = toml::from_str(&text)
        .map_err(|e| anyhow::anyhow!("cannot parse {}: {e}", commands_rel.display()))?;
    let rows = data
        .get("command")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("{}: no [[command]] rows", commands_rel.display()))?;
    let mut out = String::new();
    for row in rows {
        if let Some(t) = row.as_table()
            && let Some(ms) = t.get("mutation_surface").and_then(|v| v.as_str())
        {
            out.push_str(ms);
            out.push('\n');
        }
    }
    Ok(out)
}

pub struct Outcome {
    pub failures: Vec<String>,
    pub flagged: Vec<String>,
    pub checked_files: usize,
    pub exempt_files: usize,
    pub exempt_functions: usize,
}

/// `cargo xtask audit-structure`'s function-level check (RFC 094 M2b,
/// cycle J stage 10 §1): every write-containing function inside an
/// allow-listed, non-exempt file must be named in some row's
/// `mutation_surface`.
pub fn check(root: &Path, policy_rel: &Path, commands_rel: &Path) -> anyhow::Result<Outcome> {
    let allow_list = crate::write_authority::load_allow_list(root, policy_rel)?;
    let exempt = load_write_by_design_exempt(root, policy_rel)?;
    let haystack = mutation_surface_text(root, commands_rel)?;

    let mut failures = Vec::new();
    let mut flagged = Vec::new();
    let mut checked_files = 0usize;
    let mut exempt_files = 0usize;
    let mut exempt_functions = 0usize;

    for rel_str in &allow_list {
        if exempt.contains(rel_str) {
            exempt_files += 1;
            continue;
        }
        let path = root.join(rel_str);
        if !path.is_file() {
            // write_authority::check already reports a stale allow-list
            // entry; do not double-report it here.
            continue;
        }
        checked_files += 1;
        let module = Path::new(rel_str)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(rel_str);
        for function in writing_functions(&path)? {
            let qualified = format!("{module}::{function}");
            // A function-scoped exemption, `path::function`: exempts that one
            // function and nothing else in the file, so a new writer beside
            // it is still flagged. A whole-file entry exempts everything.
            if exempt.contains(&format!("{rel_str}::{function}")) {
                exempt_functions += 1;
                continue;
            }
            if !is_named(&haystack, &qualified) {
                flagged.push(format!("{rel_str}: `{qualified}`"));
                failures.push(format!(
                    "(write-sites) {rel_str}: `{qualified}` contains a write call and is named \
                     in no row's mutation_surface (contracts/write-commands.toml)"
                ));
            }
        }
    }
    flagged.sort();
    failures.sort();

    Ok(Outcome {
        failures,
        flagged,
        checked_files,
        exempt_files,
        exempt_functions,
    })
}
