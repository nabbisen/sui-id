//! RFC 094 §F3, settled (production reach): `cargo xtask audit-structure`'s
//! manifest check. `rusqlite` is permitted in `sui-id-store`'s own
//! `[dependencies]`; everywhere else it is a violation in `[dependencies]`
//! or `[build-dependencies]` (including `[target.*.…]` forms), and
//! deliberately not a violation in `[dev-dependencies]`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_xtask");

const COMMANDS_RS: &str = r#"static A01_DONE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::Thing,
    name: "thing.done",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    command A01 = "A01" {
        system_principal: forbidden;
        enum A01Event {
            Done => &A01_DONE,
        }
    }
}
"#;

const SEALED_A01: &str = r#"[[command]]
id = "A01"
class = "A"
status = "implemented"
sealed = true
owner = "Do the thing"
rationale = "r"
mutation_surface = "`m`"
files = ["crates/sui-id-store/src/commands.rs"]
test_id = "none -- fixture, not meant to resolve"
event = ["thing.done"]
descriptor = ["A01_DONE"]
"#;

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let f = Self {
            dir: tempfile::tempdir().expect("tempdir"),
        };
        f.write("crates/sui-id-store/src/commands.rs", COMMANDS_RS);
        f.write(
            "contracts/write-commands.toml",
            &format!("# header\n\n{SEALED_A01}"),
        );
        f.write(
            "contracts/audit-coverage-matrix.md",
            "# Matrix\n\n| Event name | Actor | Class |\n|---|---|---|\n\
             | `thing.done` | admin user id | A |\n",
        );
        f.write(
            "contracts/write-authority.toml",
            "version = 1\nallowed_files = [\"crates/sui-id-store/src/commands.rs\"]\n",
        );
        // The crate this check treats as the one permitted home for
        // `rusqlite` — present, but with no `rusqlite` of its own, so a
        // pass/fail only ever turns on the *other* crate's manifest.
        f.write(
            "crates/sui-id-store/Cargo.toml",
            "[package]\nname = \"sui-id-store\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        );
        f
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.dir.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, text).expect("write");
    }

    fn run_gate(&self) -> Output {
        Command::new(BIN)
            .arg("audit-structure")
            .arg("--root")
            .arg(self.dir.path())
            .arg("--policy")
            .arg("contracts/write-authority.toml")
            .arg("--commands")
            .arg("contracts/write-commands.toml")
            .arg("--matrix")
            .arg("contracts/audit-coverage-matrix.md")
            .output()
            .expect("run xtask")
    }
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn rusqlite_in_dependencies_of_another_crate_is_rejected() {
    let f = Fixture::new();
    f.write(
        "crates/other/Cargo.toml",
        "[package]\nname = \"other\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         [dependencies]\nrusqlite = \"0.40\"\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("crates/other/Cargo.toml"));
    assert!(stderr(&out).contains("[dependencies]"));
}

#[test]
fn rusqlite_in_dev_dependencies_of_another_crate_passes() {
    let f = Fixture::new();
    f.write(
        "crates/other/Cargo.toml",
        "[package]\nname = \"other\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         [dev-dependencies]\nrusqlite = \"0.40\"\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn rusqlite_in_build_dependencies_of_another_crate_is_rejected() {
    let f = Fixture::new();
    f.write(
        "crates/other/Cargo.toml",
        "[package]\nname = \"other\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         [build-dependencies]\nrusqlite = \"0.40\"\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("[build-dependencies]"));
}

#[test]
fn rusqlite_in_a_target_specific_dependencies_table_is_rejected() {
    let f = Fixture::new();
    f.write(
        "crates/other/Cargo.toml",
        "[package]\nname = \"other\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         [target.'cfg(unix)'.dependencies]\nrusqlite = \"0.40\"\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("crates/other/Cargo.toml"));
}

#[test]
fn rusqlite_in_sui_id_stores_own_dependencies_passes() {
    let f = Fixture::new();
    f.write(
        "crates/sui-id-store/Cargo.toml",
        "[package]\nname = \"sui-id-store\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         [dependencies]\nrusqlite = \"0.40\"\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

// ── cycle J stage 10 review: test seams stay out of production builds ─────

/// `crates/sui-id/Cargo.toml` enabled `sui-id-core/test-support` under
/// `[dependencies]` from 2026-07-02 until this review, compiling a test mock
/// into every release build.
#[test]
fn test_support_on_a_normal_dependency_is_rejected() {
    let f = Fixture::new();
    f.write(
        "crates/other/Cargo.toml",
        "[package]\nname = \"other\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         [dependencies]\nsui-id-core = { path = \"../core\", features = [\"test-support\"] }\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("enables `test-support`"),
        "{}",
        stderr(&out)
    );
}

/// The same feature on a dev-dependency is how tests are meant to get it.
#[test]
fn test_support_on_a_dev_dependency_passes() {
    let f = Fixture::new();
    f.write(
        "crates/other/Cargo.toml",
        "[package]\nname = \"other\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         [dev-dependencies]\nsui-id-core = { path = \"../core\", features = [\"test-support\"] }\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}
