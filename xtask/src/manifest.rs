//! RFC 094 §F3, settled (`m2b-criterion-2-and-f3-verified-2026-11-08.md`
//! §2): **production reach** — `rusqlite` appears in the
//! `[dependencies]` table (including any `[target.*.dependencies]`) of no
//! crate other than `sui-id-store`.
//!
//! **`[dev-dependencies]` is allowed.** Cargo never links a
//! dev-dependency into a shipped artifact, so it protects no production
//! path; forbidding it would cost real black-box test coverage
//! (`tests/e2e/*.rs` files that inspect or seed the database directly,
//! the stronger test, not the weaker one) for no corresponding safety.
//! M2a already closed on this reading (`tests/e2e/r094_m2a_db_access_scope.rs`).
//!
//! **`[build-dependencies]` is treated the same as `[dependencies]`: forbidden
//! outside `sui-id-store`.** A build script is not shipped code, but it
//! does run — with full filesystem access, on every contributor's and
//! CI's machine, at build time. "Not shipped" is not "harmless." Measured:
//! no crate in this workspace has a `build.rs` today, so the stricter
//! reading costs nothing now and only ever costs something if a future
//! build script would have reached for `rusqlite` directly, which the
//! same invariant 6 this gate protects says it should not.
//!
//! **`[workspace.dependencies]` is a declaration, not a dependency of any
//! particular crate — allowed unconditionally**, and only ever appears in
//! the root manifest, which this check does not otherwise inspect (the
//! root is a virtual manifest with no `[package]` of its own, so it has
//! no `[dependencies]` to check).

use std::path::Path;

/// Tables, read literally by name (not resolved against `cfg(...)`,
/// matching `toml::Table`'s own flat key access): any `[dependencies]` or
/// `[build-dependencies]` table, and any `[target.<spec>.dependencies]`
/// or `[target.<spec>.build-dependencies]` table. `[dev-dependencies]`
/// and `[target.<spec>.dev-dependencies]` are deliberately absent from
/// this list.
fn forbidden_dependency_tables(doc: &toml::Table) -> Vec<(String, &toml::Table)> {
    let mut out = Vec::new();
    for key in ["dependencies", "build-dependencies"] {
        if let Some(t) = doc.get(key).and_then(|v| v.as_table()) {
            out.push((format!("[{key}]"), t));
        }
    }
    if let Some(targets) = doc.get("target").and_then(|v| v.as_table()) {
        for (spec, spec_table) in targets {
            let Some(spec_table) = spec_table.as_table() else {
                continue;
            };
            for key in ["dependencies", "build-dependencies"] {
                if let Some(t) = spec_table.get(key).and_then(|v| v.as_table()) {
                    out.push((format!("[target.{spec}.{key}]"), t));
                }
            }
        }
    }
    out
}

pub struct Outcome {
    pub failures: Vec<String>,
    pub manifests_checked: usize,
}

/// `cargo xtask audit-structure`'s manifest check (RFC 094 §F3, cycle J
/// stage 10 §5): walk every crate's `Cargo.toml` under `--root` and
/// reject `rusqlite` in a forbidden table of any crate other than
/// `sui-id-store`.
pub fn check(root: &Path) -> anyhow::Result<Outcome> {
    let mut failures = Vec::new();
    let mut manifests_checked = 0usize;

    let crates_dir = root.join("crates");
    let mut manifests: Vec<std::path::PathBuf> = Vec::new();
    if crates_dir.is_dir() {
        for entry in std::fs::read_dir(&crates_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let manifest = entry.path().join("Cargo.toml");
                if manifest.is_file() {
                    manifests.push(manifest);
                }
            }
        }
    }
    let xtask_manifest = root.join("xtask/Cargo.toml");
    if xtask_manifest.is_file() {
        manifests.push(xtask_manifest);
    }
    manifests.sort();

    for manifest in &manifests {
        let rel = manifest.strip_prefix(root).unwrap_or(manifest);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let is_store = rel_str == "crates/sui-id-store/Cargo.toml";
        manifests_checked += 1;

        let text = std::fs::read_to_string(manifest)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", rel_str))?;
        let doc: toml::Table =
            toml::from_str(&text).map_err(|e| anyhow::anyhow!("cannot parse {}: {e}", rel_str))?;

        // No production table may switch on a `test-support` feature (cycle J
        // stage 10 review). Test seams belong under `[dev-dependencies]`, and
        // this applies to every crate, `sui-id-store` included. Found when
        // `crates/sui-id/Cargo.toml` had enabled `sui-id-core/test-support`
        // under `[dependencies]` since 2026-07-02, compiling an in-memory HIBP
        // mock into every release build while the dev-dependency meant to
        // carry the feature carried none.
        for (table_name, table) in forbidden_dependency_tables(&doc) {
            for (dep, spec) in table {
                let enables_test_support = spec
                    .as_table()
                    .and_then(|t| t.get("features"))
                    .and_then(|f| f.as_array())
                    .is_some_and(|fs| fs.iter().any(|f| f.as_str() == Some("test-support")));
                if enables_test_support {
                    failures.push(format!(
                        "(manifest) {rel_str}: `{dep}` in {table_name} enables `test-support`, \
                         which would compile test seams into a production build; move the \
                         feature to the [dev-dependencies] entry"
                    ));
                }
            }
        }

        if is_store {
            continue;
        }
        for (table_name, table) in forbidden_dependency_tables(&doc) {
            if table.contains_key("rusqlite") {
                failures.push(format!(
                    "(manifest) {rel_str}: `rusqlite` appears in {table_name}, but RFC 094 \
                     permits it only in sui-id-store (production reach: [dev-dependencies] \
                     is allowed, and is probably what was meant)"
                ));
            }
        }
    }

    Ok(Outcome {
        failures,
        manifests_checked,
    })
}
