//! RFC 094 M2a: "raw database access is confined to `sui-id-store` by the
//! dependency graph" is gate-asserted here, not left to be remembered. The
//! clause is about production reach, not every Cargo edge: a crate may use
//! `rusqlite` under `[dev-dependencies]` for its own tests (as both
//! `sui-id-store` and `sui-id` do) without compromising it; only a
//! `[dependencies]` entry outside `sui-id-store` does.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const RESTRICTED_CRATE: &str = "rusqlite";
const HOME_CRATE: &str = "sui-id-store";

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("crates/ directory")
}

fn production_dependency_names(manifest: &str) -> Vec<String> {
    let table: toml::Table = manifest.parse().expect("valid Cargo.toml");
    let Some(deps) = table.get("dependencies").and_then(|v| v.as_table()) else {
        return Vec::new();
    };
    deps.keys().cloned().collect()
}

#[test]
fn raw_database_access_is_confined_to_sui_id_store() {
    let root = crates_dir();
    let mut offenders: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in std::fs::read_dir(&root).expect("read crates/ directory") {
        let entry = entry.expect("dir entry");
        let crate_dir = entry.path();
        let manifest_path = crate_dir.join("Cargo.toml");
        if !manifest_path.is_file() {
            continue;
        }
        let crate_name = crate_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if crate_name == HOME_CRATE {
            continue;
        }
        let manifest = std::fs::read_to_string(&manifest_path).expect("read Cargo.toml");
        let deps = production_dependency_names(&manifest);
        if deps.iter().any(|d| d == RESTRICTED_CRATE) {
            offenders.insert(crate_name, deps);
        }
    }
    assert!(
        offenders.is_empty(),
        "these crates declare `{RESTRICTED_CRATE}` under [dependencies], not \
         [dev-dependencies] -- raw database access must be confined to \
         `{HOME_CRATE}` (RFC 094 M2a's closure prerequisite): {offenders:#?}"
    );
}
