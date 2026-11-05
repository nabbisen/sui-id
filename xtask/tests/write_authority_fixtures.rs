//! RFC 094 cycle J stage 6b: coverage for the `syn` write-authority
//! boundary check, built against the design cycle J stage 6's prototype
//! validated (type-path and whole-identifier detection, zero false
//! positives on doc-comment examples or substring matches).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_xtask");

const COMMANDS_RS: &str = r#"static A01_DONE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::Thing,
    name: "thing.done",
    class: AuditClass::Atomic,
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
files = ["crates/store/src/repos/things.rs"]
test_id = "`t`"
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
        f.write("crates/store/src/commands.rs", COMMANDS_RS);
        f.write(
            "contracts/write-commands.toml",
            &format!("# header\n\n{SEALED_A01}"),
        );
        // RFC 094 cycle J stage 7: `audit-structure` now also runs the
        // coverage check, which needs a matrix file to read -- one row,
        // matching this fixture's one real event, so the write-authority
        // cases above stay the only thing under test here.
        f.write(
            "contracts/audit-coverage-matrix.md",
            "# Matrix\n\n| Event name | Class |\n|---|---|\n| `thing.done` | A |\n",
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

    fn set_policy(&self, allowed_files: &[&str]) {
        let list = allowed_files
            .iter()
            .map(|f| format!("\"{f}\""))
            .collect::<Vec<_>>()
            .join(", ");
        self.write(
            "contracts/write-authority.toml",
            &format!("version = 1\nallowed_files = [{list}]\n"),
        );
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
fn a_raw_type_outside_the_allow_list_is_rejected() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/repos/things.rs",
        "pub fn touch(tx: &rusqlite::Transaction<'_>) {}\n",
    );
    f.set_policy(&[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("crates/store/src/repos/things.rs"));
    assert!(stderr(&out).contains("rusqlite::Transaction"));
}

#[test]
fn with_conn_outside_the_allow_list_is_rejected() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/repos/things.rs",
        "pub fn touch(db: &Database) { let _ = db.with_conn(|_c| Ok(())); }\n",
    );
    f.set_policy(&[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("identifier `with_conn`"));
}

#[test]
fn an_allow_listed_file_is_not_scanned_for_violations() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/repos/things.rs",
        "pub fn touch(tx: &rusqlite::Transaction<'_>) {}\n",
    );
    f.set_policy(&["crates/store/src/repos/things.rs"]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn a_doc_comment_example_is_not_a_false_positive() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/repos/things.rs",
        "/// Example: `conn.execute(\"UPDATE things SET x = 1\")`.\npub fn touch() {}\n",
    );
    f.set_policy(&[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn a_substring_match_inside_an_unrelated_identifier_is_not_a_false_positive() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/repos/things.rs",
        "pub fn into_make_service_with_connect_info() {}\n",
    );
    f.set_policy(&[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn a_stale_allow_list_entry_naming_a_missing_file_is_rejected() {
    let f = Fixture::new();
    f.write("crates/store/src/repos/things.rs", "pub fn touch() {}\n");
    f.set_policy(&["crates/store/src/repos/does_not_exist.rs"]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("does_not_exist.rs"));
    assert!(stderr(&out).contains("does not exist"));
}

#[test]
fn the_gap_statement_names_what_is_not_checked() {
    let f = Fixture::new();
    f.write("crates/store/src/repos/things.rs", "pub fn touch() {}\n");
    f.set_policy(&[]);
    let out = f.run_gate();
    let text = stderr(&out);
    assert!(text.contains("receiver-type resolution"));
    assert!(text.contains("alias/generic-indirection tracing"));
}
