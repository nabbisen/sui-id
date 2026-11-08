//! RFC 094 M2b / cycle J stage 10 §1 — the second negative fixture, inside
//! an allow-listed path (the adversarial assessment's probe A, kept): a
//! write-containing function with no row naming it must be rejected even
//! though the file itself is allow-listed and the module boundary alone
//! would pass it; a named one, in the same file, must not be.

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

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new(sealed_row_mutation_surface: &str) -> Self {
        let f = Self {
            dir: tempfile::tempdir().expect("tempdir"),
        };
        f.write("crates/store/src/commands.rs", COMMANDS_RS);
        f.write(
            "contracts/write-commands.toml",
            &format!(
                "# header\n\n[[command]]\n\
                 id = \"A01\"\n\
                 class = \"A\"\n\
                 status = \"implemented\"\n\
                 sealed = true\n\
                 owner = \"Do the thing\"\n\
                 rationale = \"r\"\n\
                 mutation_surface = \"{sealed_row_mutation_surface}\"\n\
                 files = [\"crates/store/src/commands.rs\"]\n\
                 test_id = \"`t`\"\n\
                 event = [\"thing.done\"]\n\
                 descriptor = [\"A01_DONE\"]\n"
            ),
        );
        f.write(
            "contracts/audit-coverage-matrix.md",
            "# Matrix\n\n| Event name | Actor | Class |\n|---|---|---|\n\
             | `thing.done` | admin user id | A |\n",
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

    fn set_policy(&self, allowed_files: &[&str], write_by_design_exempt: &[&str]) {
        let list = |items: &[&str]| {
            items
                .iter()
                .map(|f| format!("\"{f}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        self.write(
            "contracts/write-authority.toml",
            &format!(
                "version = 1\nallowed_files = [{}]\nwrite_by_design_exempt = [{}]\n",
                list(allowed_files),
                list(write_by_design_exempt)
            ),
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

/// Probe A, kept: an allow-listed file, a writing function named in no
/// row, no descriptor. The module boundary alone passes this (the file is
/// allow-listed); this check must still reject it, by name.
#[test]
fn an_unnamed_writing_function_inside_an_allow_listed_file_is_rejected() {
    let f = Fixture::new("`users::named_write`");
    f.write(
        "crates/store/src/repos/users.rs",
        "pub fn named_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n\
         pub fn unregistered_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n",
    );
    f.set_policy(&["crates/store/src/repos/users.rs"], &[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("users::unregistered_write"),
        "{}",
        stderr(&out)
    );
    assert!(
        !stderr(&out).contains("users::named_write"),
        "the named function must not also be flagged: {}",
        stderr(&out)
    );
}

/// The same file, both functions named: passes.
#[test]
fn every_writing_function_named_passes() {
    let f = Fixture::new("`users::named_write`, `users::also_named`");
    f.write(
        "crates/store/src/repos/users.rs",
        "pub fn named_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n\
         pub fn also_named(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 0\", []).unwrap(); \
         }\n",
    );
    f.set_policy(&["crates/store/src/repos/users.rs"], &[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

/// A write inside a closure is attributed to its enclosing named function,
/// not left unnameable.
#[test]
fn a_write_inside_a_closure_is_attributed_to_the_enclosing_function() {
    let f = Fixture::new("`users::named_write`");
    f.write(
        "crates/store/src/repos/users.rs",
        "pub fn named_write(conn: &rusqlite::Connection) { \
             let go = |c: &rusqlite::Connection| { c.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); }; \
             go(conn); \
         }\n",
    );
    f.set_policy(&["crates/store/src/repos/users.rs"], &[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

/// A `#[cfg(test)]` function is out of scope, same as the boundary check.
#[test]
fn a_cfg_test_function_is_not_flagged() {
    let f = Fixture::new("`users::named_write`");
    f.write(
        "crates/store/src/repos/users.rs",
        "pub fn named_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n\
         #[cfg(test)]\n\
         fn test_helper(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n",
    );
    f.set_policy(&["crates/store/src/repos/users.rs"], &[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

/// A file named in `write_by_design_exempt` gets no function-level check
/// at all, even with an unnamed write inside it.
#[test]
fn a_write_by_design_exempt_file_is_not_checked() {
    let f = Fixture::new("`users::named_write`");
    f.write(
        "crates/store/src/migrations.rs",
        "pub fn named_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n\
         pub fn unregistered_migration_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n",
    );
    f.set_policy(
        &["crates/store/src/migrations.rs"],
        &["crates/store/src/migrations.rs"],
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

/// A read-only function (`query_row` only) in an allow-listed file is not
/// flagged -- the point of excluding read methods from the write-call
/// definition.
#[test]
fn a_read_only_function_is_not_flagged() {
    let f = Fixture::new("`users::named_write`");
    f.write(
        "crates/store/src/repos/users.rs",
        "pub fn named_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n\
         pub fn find_by_id(conn: &rusqlite::Connection) { \
             let _: i64 = conn.query_row(\"SELECT 1\", [], |r| r.get(0)).unwrap(); \
         }\n",
    );
    f.set_policy(&["crates/store/src/repos/users.rs"], &[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

// ── cycle J stage 10 review: four ways the check could be passed ──────────

/// Probe C, kept: a new writer whose name is a *prefix* of a named one.
/// `users::set_disabled_within_tx` is named; `users::set_disabled` is not,
/// and a substring match used to pass it.
#[test]
fn a_prefix_of_a_named_function_is_not_named() {
    let f = Fixture::new("`users::set_disabled_within_tx`");
    f.write(
        "crates/store/src/repos/users.rs",
        "pub fn set_disabled_within_tx(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_disabled = 1\", []).unwrap(); \
         }\n\
         pub fn set_disabled(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_disabled = 1\", []).unwrap(); \
         }\n",
    );
    f.set_policy(&["crates/store/src/repos/users.rs"], &[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("`users::set_disabled` contains a write call"),
        "{}",
        stderr(&out)
    );
}

/// Probe D, kept: a write through `query_row` (`UPDATE … RETURNING`), the
/// shape two real functions in this tree use.
#[test]
fn a_write_through_query_row_returning_is_a_write() {
    let f = Fixture::new("`users::named_write`");
    f.write(
        "crates/store/src/repos/users.rs",
        "pub fn named_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n\
         pub fn bump(conn: &rusqlite::Connection) -> i64 { \
             conn.query_row(\"UPDATE users SET n = n + 1 RETURNING n\", [], |r| r.get(0)).unwrap() \
         }\n",
    );
    f.set_policy(&["crates/store/src/repos/users.rs"], &[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("users::bump"), "{}", stderr(&out));
}

/// Probe E, kept: `#[cfg(not(test))]` is production-only, not test-only,
/// and must be checked. A substring test for "test" used to skip it.
#[test]
fn a_cfg_not_test_writer_is_checked() {
    let f = Fixture::new("`users::named_write`");
    f.write(
        "crates/store/src/repos/users.rs",
        "pub fn named_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n\
         #[cfg(not(test))]\n\
         pub fn production_only(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n",
    );
    f.set_policy(&["crates/store/src/repos/users.rs"], &[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("users::production_only"),
        "{}",
        stderr(&out)
    );
}

/// The test-only forms still skip: `any(test, feature = "test-support")`
/// is how this tree gates cross-crate test helpers.
#[test]
fn a_test_or_test_support_writer_is_not_flagged() {
    let f = Fixture::new("`users::named_write`");
    f.write(
        "crates/store/src/repos/users.rs",
        "pub fn named_write(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n\
         #[cfg(any(test, feature = \"test-support\"))]\n\
         pub fn seed(conn: &rusqlite::Connection) { \
             conn.execute(\"UPDATE users SET is_admin = 1\", []).unwrap(); \
         }\n",
    );
    f.set_policy(&["crates/store/src/repos/users.rs"], &[]);
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

/// A function-scoped exemption exempts that function only: a new writer
/// beside it, in the same file, is still flagged.
#[test]
fn a_function_scoped_exemption_does_not_cover_a_new_writer_beside_it() {
    let f = Fixture::new("`users::named_write`");
    f.write(
        "crates/store/src/repos/audit.rs",
        "pub fn append(conn: &rusqlite::Connection) { \
             conn.execute(\"INSERT INTO audit_log VALUES (1)\", []).unwrap(); \
         }\n\
         pub fn wipe(conn: &rusqlite::Connection) { \
             conn.execute(\"DELETE FROM audit_log\", []).unwrap(); \
         }\n",
    );
    f.set_policy(
        &["crates/store/src/repos/audit.rs"],
        &["crates/store/src/repos/audit.rs::append"],
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("audit::wipe"), "{}", stderr(&out));
    assert!(
        !stderr(&out).contains("`audit::append`"),
        "{}",
        stderr(&out)
    );
}
