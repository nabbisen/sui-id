//! RFC 094 cycle J stage 6b: ported from the deleted
//! `scripts/tests/test_write_commands.py` (RFC 116 stage 1's negative
//! self-tests for `scripts/check-write-commands.py`, retired in the same
//! change this test exists to justify). Same fixtures, same mutations,
//! same black-box-subprocess convention -- run against the built
//! `xtask` binary instead of the Python script, so stage 6b's review
//! requirement ("per check, ported behaviour equal or stronger, show
//! one case per check where it fails") is evidence, not an assertion.
//! Every test here passed against the Python script before this port
//! and passes against this binary now; none were loosened to pass.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_xtask");

const COMMANDS_RS: &str = r#"static A01_DONE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::Thing,
    name: "thing.done",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    attributes: &[AttributeSpec { name: "note", description: "", required: false }],
};

static A01_FAILED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::Thing,
    name: "thing.failed",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// A01 -- a doc comment that mentions `command Z99 = "Z99"` and
    /// `declare_write_command! { }` must not count.
    command A01 = "A01" {
        system_principal: forbidden;
        enum A01Event {
            Done { id: String } => &A01_DONE,
            Failed => &A01_FAILED,
        }
    }
}
"#;

const SECOND_RS: &str = r#"static B02_DONE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::Other,
    name: "other.done",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    command B02 = "B02" {
        system_principal: forbidden;
        enum B02Event {
            Done => &B02_DONE,
        }
    }
}
"#;

const HEADER: &str = "# The inventory. Two states: sealed, planned.\n# There is no count in this header on purpose.\n\n";

const SEALED_A01: &str = r#"[[command]]
id = "A01"
class = "A"
status = "implemented"
sealed = true
owner = "Do the thing"
rationale = "r"
mutation_surface = "`m`"
files = ["crates/store/src/repos/things.rs"]
test_id = "none -- fixture, not meant to resolve"
event = ["thing.done", "thing.failed"]
descriptor = ["A01_DONE", "A01_FAILED"]
"#;

const PLANNED_P01: &str = r#"[[command]]
id = "P01"
class = "A"
status = "target-absent"
sealed = false
owner = "Later"
rationale = "r"
mutation_surface = "`m`"
files = []
test_id = "none -- fixture, not meant to resolve"
"#;

const SEALED_BY_A01_P01: &str = r#"[[command]]
id = "P01"
class = "A"
status = "implemented"
sealed = false
sealed_by = "A01"
owner = "Covered by A01's transaction"
rationale = "r"
mutation_surface = "`m`"
files = ["crates/store/src/repos/things.rs"]
test_id = "none -- fixture, not meant to resolve"
"#;

const SEALED_B02: &str = r#"[[command]]
id = "B02"
class = "A"
status = "implemented"
sealed = true
owner = "Another"
rationale = "r"
mutation_surface = "`m`"
files = ["crates/store/src/repos/things.rs"]
test_id = "none -- fixture, not meant to resolve"
event = ["other.done"]
descriptor = ["B02_DONE"]
"#;

fn rows(blocks: &[&str]) -> String {
    format!("{HEADER}{}", blocks.join("\n"))
}

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let f = Self {
            dir: tempfile::tempdir().expect("tempdir"),
        };
        f.write("crates/store/src/commands.rs", COMMANDS_RS);
        f.write("crates/store/src/repos/things.rs", "// a repo\n");
        f.write(
            "contracts/write-commands.toml",
            &rows(&[SEALED_A01, PLANNED_P01]),
        );
        // RFC 094 cycle J stage 7: `audit-structure` now also runs the
        // coverage check. One row, matching the baseline's one real
        // event (A01's `thing.done`) -- the red-path tests below add a
        // second command without a matching row on purpose; coverage
        // flags that too, which `assert_red`'s needle check tolerates
        // (it does not require the listed failures to be the only ones).
        f.write(
            "contracts/audit-coverage-matrix.md",
            "# Matrix\n\n| Event name | Actor | Class |\n|---|---|---|\n\
             | `thing.done` | admin user id | A |\n| `thing.failed` | admin user id | A |\n",
        );
        // Empty, valid write-authority policy: the synthetic fixture
        // tree touches no rusqlite construct at all, so this is never
        // the thing under test here -- `audit-structure` just needs a
        // policy file to exist to run at all.
        f.write(
            "contracts/write-authority.toml",
            "version = 1\nallowed_files = []\n",
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

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn inventory(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("contracts/write-commands.toml"))
            .expect("read inventory")
    }

    fn run_gate(&self, extra: &[&str]) -> Output {
        let mut cmd = Command::new(BIN);
        cmd.arg("audit-structure")
            .arg("--root")
            .arg(self.root())
            .arg("--policy")
            .arg("contracts/write-authority.toml")
            .arg("--commands")
            .arg("contracts/write-commands.toml")
            .arg("--matrix")
            .arg("contracts/audit-coverage-matrix.md");
        for e in extra {
            cmd.arg(e);
        }
        cmd.output().expect("run xtask")
    }

    fn assert_green(&self) {
        let out = self.run_gate(&[]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    }

    fn assert_red(&self, needles: &[&str]) {
        let out = self.run_gate(&[]);
        assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
        let text = stderr(&out);
        for needle in needles {
            assert!(text.contains(needle), "{needle:?} not in: {text}");
        }
    }
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn combined(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), stderr(o))
}

// ── the baseline, and what it must not trip over ───────────────────────

#[test]
fn baseline_is_green_and_counts_are_derived() {
    let f = Fixture::new();
    let out = f.run_gate(&[]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("2 rows: 1 sealed, 1 planned"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_planned_row_needs_no_command() {
    Fixture::new().assert_green();
}

#[test]
fn a_doc_comment_or_test_file_or_macro_definition_does_not_declare() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/tests.rs",
        "crate::declare_write_command! {\n command Q01 = \"Q01\" {\n system_principal: forbidden; enum E { X => &S } } }\n",
    );
    f.write(
        "crates/store/src/macros.rs",
        "// declare_write_command! { command Q02 = \"Q02\" { } }\nmacro_rules! declare_write_command { () => {}; }\n",
    );
    f.assert_green();
}

// ── the three mutations the handoff names ──────────────────────────────

#[test]
fn a_command_added_without_a_row_is_caught() {
    let f = Fixture::new();
    f.write("crates/store/src/more.rs", SECOND_RS);
    f.assert_red(&["(A) command B02", "has no row in the inventory"]);
}

#[test]
fn a_row_naming_a_command_that_does_not_exist_is_caught() {
    let f = Fixture::new();
    f.write(
        "contracts/write-commands.toml",
        &rows(&[SEALED_A01, PLANNED_P01, SEALED_B02]),
    );
    f.assert_red(&["(B) row B02", "no command B02 is declared"]);
}

#[test]
fn a_wrong_files_path_is_caught() {
    let f = Fixture::new();
    let text = f.inventory().replace("repos/things.rs", "repos/nothing.rs");
    f.write("contracts/write-commands.toml", &text);
    f.assert_red(&["(D) row A01", "repos/nothing.rs", "does not exist"]);
}

// ── the rest of the contract ────────────────────────────────────────────

#[test]
fn a_planned_row_whose_command_has_been_sealed_is_caught() {
    let f = Fixture::new();
    f.write(
        "contracts/write-commands.toml",
        &rows(&[SEALED_A01, &PLANNED_P01.replace("\"P01\"", "\"B02\"")]),
    );
    f.write("crates/store/src/more.rs", SECOND_RS);
    f.assert_red(&["(A) command B02", "`sealed = false`"]);
}

#[test]
fn a_wrong_event_list_is_caught() {
    let f = Fixture::new();
    let text = f.inventory().replace(
        r#"event = ["thing.done", "thing.failed"]"#,
        r#"event = ["thing.done"]"#,
    );
    f.write("contracts/write-commands.toml", &text);
    f.assert_red(&["(C) row A01", "`event`"]);
}

#[test]
fn a_wrong_descriptor_list_is_caught() {
    let f = Fixture::new();
    let text = f.inventory().replace("\"A01_FAILED\"]", "\"A01_OTHER\"]");
    f.write("contracts/write-commands.toml", &text);
    f.assert_red(&["(C) row A01", "`descriptor`"]);
}

#[test]
fn a_sealed_row_without_its_derived_fields_is_caught() {
    let f = Fixture::new();
    let inventory = f.inventory();
    let lines: Vec<&str> = inventory
        .lines()
        .filter(|l| !l.starts_with("event =") && !l.starts_with("descriptor ="))
        .collect();
    f.write("contracts/write-commands.toml", &(lines.join("\n") + "\n"));
    f.assert_red(&[
        "(C) row A01",
        "`event` is missing",
        "`descriptor` is missing",
    ]);
}

#[test]
fn a_planned_row_carrying_a_derived_field_is_caught() {
    let f = Fixture::new();
    let text = f
        .inventory()
        .replace("files = []\n", "files = []\nevent = [\"x.y\"]\n");
    f.write("contracts/write-commands.toml", &text);
    f.assert_red(&["(C) row P01", "carries no `event`"]);
}

#[test]
fn target_absent_may_not_name_a_file_and_others_must() {
    let f = Fixture::new();
    let text = f.inventory().replace(
        "files = []",
        r#"files = ["crates/store/src/repos/things.rs"]"#,
    );
    f.write("contracts/write-commands.toml", &text);
    f.assert_red(&["(D) row P01", "`target-absent` names no file"]);

    let text2 = f.inventory().replace(
        r#"files = ["crates/store/src/repos/things.rs"]"#,
        "files = []",
    );
    f.write("contracts/write-commands.toml", &text2);
    f.assert_red(&["(D) row A01", "names no file"]);
}

#[test]
fn a_command_whose_id_is_not_of_the_inventory_shape_is_caught() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/more.rs",
        &SECOND_RS.replace("\"B02\"", "\"NOT-AN-ID\""),
    );
    f.assert_red(&["more.rs", "not of the inventory's shape"]);
}

#[test]
fn the_registry_proof_only_command_is_tolerated_there_and_only_there() {
    let f = Fixture::new();
    let proof = SECOND_RS.replace("\"B02\"", "\"PROOFONLY-NOT-AN-INVENTORY-ROW\"");
    f.write("crates/sui-id-store/src/registry.rs", &proof);
    f.assert_green();
}

#[test]
fn a_command_declared_twice_is_caught() {
    let f = Fixture::new();
    f.write("crates/store/src/more.rs", COMMANDS_RS);
    f.assert_red(&["command A01 is declared twice"]);
}

#[test]
fn a_malformed_row_is_caught() {
    let f = Fixture::new();
    let text = f.inventory().replace(
        "class = \"A\"\nstatus = \"implemented\"",
        "class = \"Z\"\nstatus = \"done\"",
    );
    f.write("contracts/write-commands.toml", &text);
    f.assert_red(&["status \"done\"", "class \"Z\""]);

    let text2 = f.inventory().replace("sealed = true\n", "");
    f.write("contracts/write-commands.toml", &text2);
    f.assert_red(&["missing `sealed`"]);

    f.write(
        "contracts/write-commands.toml",
        &rows(&[SEALED_A01, SEALED_A01, PLANNED_P01]),
    );
    f.assert_red(&["id appears twice"]);
}

#[test]
fn a_sealed_row_must_be_implemented_and_class_a() {
    let f = Fixture::new();
    let text = f.inventory().replacen(
        "class = \"A\"\nstatus = \"implemented\"",
        "class = \"P\"\nstatus = \"target-legacy\"",
        1,
    );
    f.write("contracts/write-commands.toml", &text);
    f.assert_red(&["must be `implemented`", "Class A by construction"]);
}

// ── (G) sealed_by ────────────────────────────────────────────────────────

#[test]
fn a_row_sealed_by_a_sealed_command_is_accepted() {
    let f = Fixture::new();
    f.write(
        "contracts/write-commands.toml",
        &rows(&[SEALED_A01, SEALED_BY_A01_P01]),
    );
    f.assert_green();
}

#[test]
fn sealed_true_and_sealed_by_together_is_caught() {
    let f = Fixture::new();
    let text = SEALED_BY_A01_P01.replace("sealed = false", "sealed = true");
    f.write("contracts/write-commands.toml", &rows(&[SEALED_A01, &text]));
    f.assert_red(&["(G) row P01", "`sealed = true` and `sealed_by` together"]);
}

#[test]
fn sealed_by_naming_an_undeclared_row_is_caught() {
    let f = Fixture::new();
    let text = SEALED_BY_A01_P01.replace("sealed_by = \"A01\"", "sealed_by = \"Z99\"");
    f.write("contracts/write-commands.toml", &rows(&[SEALED_A01, &text]));
    f.assert_red(&["(G) row P01", "\"Z99\"", "not a declared row"]);
}

#[test]
fn sealed_by_naming_an_unsealed_row_is_caught() {
    let f = Fixture::new();
    let text = SEALED_BY_A01_P01.replace("sealed_by = \"A01\"", "sealed_by = \"P02\"");
    let p02 = PLANNED_P01.replace("\"P01\"", "\"P02\"");
    f.write(
        "contracts/write-commands.toml",
        &rows(&[SEALED_A01, &text, &p02]),
    );
    f.assert_red(&["(G) row P01", "\"P02\"", "not `sealed = true`"]);
}

#[test]
fn fix_leaves_sealed_by_untouched() {
    let f = Fixture::new();
    f.write(
        "contracts/write-commands.toml",
        &rows(&[SEALED_A01, SEALED_BY_A01_P01]),
    );
    let out = f.run_gate(&["--fix"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(f.inventory().contains("sealed_by = \"A01\""));
}

#[test]
fn an_unknown_key_is_caught() {
    let f = Fixture::new();
    let text = f.inventory().replace(
        "owner = \"Do the thing\"",
        "owner = \"Do the thing\"\nnotes = \"x\"",
    );
    f.write("contracts/write-commands.toml", &text);
    f.assert_red(&["unknown key `notes`"]);
}

#[test]
fn a_hand_written_count_in_the_header_is_caught() {
    let f = Fixture::new();
    f.write(
        "contracts/write-commands.toml",
        &format!("# implemented -- code exists (82)\n\n{}", f.inventory()),
    );
    f.assert_red(&["(F)", "hand-written count"]);

    let f2 = Fixture::new();
    f2.write(
        "contracts/write-commands.toml",
        &format!("# 99 entries in all.\n\n{}", f2.inventory()),
    );
    f2.assert_red(&["(F)", "hand-written count"]);
}

#[test]
fn no_command_at_all_fails_closed() {
    let f = Fixture::new();
    f.write("crates/store/src/commands.rs", "// nothing\n");
    let out = f.run_gate(&[]);
    assert_eq!(out.status.code(), Some(2), "{}", combined(&out));
    assert!(stderr(&out).contains("fails closed"));
}

#[test]
fn an_unreadable_inventory_is_exit_two() {
    let f = Fixture::new();
    f.write("contracts/write-commands.toml", "this is = not [valid");
    let out = f.run_gate(&[]);
    assert_eq!(out.status.code(), Some(2), "{}", combined(&out));
}

// ── --fix ─────────────────────────────────────────────────────────────

#[test]
fn fix_rewrites_derived_fields_and_nothing_else() {
    let f = Fixture::new();
    let stale = f
        .inventory()
        .replace(
            r#"event = ["thing.done", "thing.failed"]"#,
            r#"event = ["wrong"]"#,
        )
        .replace(
            r#"descriptor = ["A01_DONE", "A01_FAILED"]"#,
            "descriptor = []",
        )
        .replace("files = []\n", "files = []\nevent = [\"x.y\"]\n");
    f.write("contracts/write-commands.toml", &stale);
    f.assert_red(&["(C)"]);

    let out = f.run_gate(&["--fix"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let expected = rows(&[SEALED_A01, PLANNED_P01])
        .trim_end_matches('\n')
        .to_owned()
        + "\n";
    assert_eq!(f.inventory(), expected);
    f.assert_green();
}

#[test]
fn fix_does_not_hide_a_missing_row() {
    let f = Fixture::new();
    f.write("crates/store/src/more.rs", SECOND_RS);
    let out = f.run_gate(&["--fix"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert!(stderr(&out).contains("(A) command B02"));
}

#[test]
fn fix_does_not_make_a_dead_path_pass() {
    let f = Fixture::new();
    f.write(
        "contracts/write-commands.toml",
        &f.inventory().replace("repos/things.rs", "repos/nothing.rs"),
    );
    let out = f.run_gate(&["--fix"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert!(stderr(&out).contains("(D) row A01"));
}
