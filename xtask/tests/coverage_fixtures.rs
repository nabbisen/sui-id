//! RFC 094 cycle J stage 7: coverage for the AST-based audit-coverage
//! check, built to the stage 6b equivalence bar restated for this stage --
//! one case per direction where the new implementation fails, and a name
//! G13 catches today is still caught by the typed-path extraction, not
//! only by a string match.

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

const WRITE_COMMANDS_TOML: &str = r#"# header

[[command]]
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

const MATRIX_MD: &str = "# Matrix\n\n| Event name | Class |\n|---|---|\n| `thing.done` | A |\n";

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
        f.write("contracts/write-commands.toml", WRITE_COMMANDS_TOML);
        f.write(
            "contracts/write-authority.toml",
            "version = 1\nallowed_files = []\n",
        );
        f.write("contracts/audit-coverage-matrix.md", MATRIX_MD);
        f
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.dir.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, text).expect("write");
    }

    fn append(&self, rel: &str, text: &str) {
        let path = self.dir.path().join(rel);
        let mut existing = std::fs::read_to_string(&path).unwrap_or_default();
        existing.push_str(text);
        self.write(rel, &existing);
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
fn baseline_is_green() {
    let f = Fixture::new();
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("1 matrix entries, 1 source names"),
        "{}",
        stderr(&out)
    );
}

// ── one case per direction, per the stage 7 equivalence bar ──────────────

#[test]
fn forward_a_matrix_name_absent_from_source_is_caught() {
    let f = Fixture::new();
    f.write(
        "contracts/audit-coverage-matrix.md",
        "# Matrix\n\n| Event name | Class |\n|---|---|\n| `thing.done` | A |\n| `thing.ghost` | A |\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("(coverage forward) thing.ghost"));
    assert!(stderr(&out).contains("in matrix but not found"));
}

#[test]
fn backward_a_source_name_absent_from_the_matrix_is_caught() {
    let f = Fixture::new();
    // A second sealed command, same shape as A01 -- a real typed source,
    // with no corresponding matrix row.
    f.append(
        "crates/store/src/commands.rs",
        "\nstatic B02_DONE: EventDescriptor = EventDescriptor {\n    kind: AuditEventKind::Other,\n    name: \"other.done\",\n    class: AuditClass::Atomic,\n    attributes: &[],\n};\n\ncrate::declare_write_command! {\n    command B02 = \"B02\" {\n        system_principal: forbidden;\n        enum B02Event {\n            Done => &B02_DONE,\n        }\n    }\n}\n",
    );
    f.write(
        "contracts/write-commands.toml",
        &format!(
            "{WRITE_COMMANDS_TOML}\n[[command]]\nid = \"B02\"\nclass = \"A\"\nstatus = \"implemented\"\nsealed = true\nowner = \"o\"\nrationale = \"r\"\nmutation_surface = \"`m`\"\nfiles = [\"crates/store/src/repos/things.rs\"]\ntest_id = \"`t`\"\nevent = [\"other.done\"]\ndescriptor = [\"B02_DONE\"]\n"
        ),
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("(coverage backward) other.done"));
    assert!(stderr(&out).contains("not in contracts/audit-coverage-matrix.md"));
}

// ── each typed source, individually ───────────────────────────────────────

#[test]
fn a_must_attempt_descriptor_static_is_a_source() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/pending.rs",
        "pub static X: MustAttemptDescriptor = MustAttemptDescriptor {\n    kind: AuditEventKind::Other,\n    name: \"mfa.required\",\n    actor: ActorRequirement::None,\n    target: TargetRequirement::None,\n    attributes: &[],\n};\n",
    );
    let out = f.run_gate();
    // Backward: mfa.required is a source name with no matrix row.
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("(coverage backward) mfa.required"));
}

#[test]
fn an_events_rs_shaped_name_method_is_a_source() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/events.rs",
        "pub enum SecurityEvent {\n    Thing,\n    Other,\n}\nimpl SecurityEvent {\n    pub fn name(&self) -> &'static str {\n        match self {\n            Self::Thing => \"legacy.thing_happened\",\n            Self::Other => \"legacy.other_happened\",\n        }\n    }\n}\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("(coverage backward) legacy.other_happened"));
    assert!(stderr(&out).contains("(coverage backward) legacy.thing_happened"));
}

#[test]
fn an_audit_log_row_literal_construction_site_is_a_source() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/cli_like.rs",
        "pub fn run() {\n    let _ = AuditLogRow {\n        at: now,\n        actor: None,\n        action: \"legacy.direct_append\".into(),\n        target: None,\n        result: \"ok\".into(),\n        note: None,\n    };\n}\n",
    );
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("(coverage backward) legacy.direct_append"));
}

// ── plumbing that must NOT be mistaken for a second literal source ────────

#[test]
fn a_dynamic_action_field_is_not_a_false_positive() {
    let f = Fixture::new();
    f.write(
        "crates/store/src/plumbing.rs",
        "pub fn build(descriptor: &EventDescriptor) -> AuditLogRow {\n    AuditLogRow {\n        at: now,\n        actor: None,\n        action: descriptor.name.into(),\n        target: None,\n        result: \"ok\".into(),\n        note: None,\n    }\n}\n",
    );
    let out = f.run_gate();
    // Still green: descriptor.name.into() is not a literal, so it adds no
    // spurious source name -- the baseline's one entry is unaffected.
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn the_registry_proof_only_descriptor_is_excluded() {
    let f = Fixture::new();
    f.write(
        "crates/sui-id-store/src/registry.rs",
        "static PROOF_ONLY_FORBIDDEN_DESCRIPTOR: EventDescriptor = EventDescriptor {\n    kind: AuditEventKind::Other,\n    name: \"proof_only.system_principal_forbidden\",\n    class: AuditClass::Atomic,\n    attributes: &[],\n};\n",
    );
    let out = f.run_gate();
    // Still green: registry.rs is skipped entirely, so this never becomes
    // a (spurious) backward-direction coverage obligation.
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}
