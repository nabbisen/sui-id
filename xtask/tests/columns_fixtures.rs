//! RFC 094 cycle J stage 7c: coverage for the AST-based column check
//! (`class`, `actor`, `step_up`, `schema`), ported from the deleted
//! `scripts/tests/test_audit_matrix_columns.py` (RFC 116 stage 2's
//! negative self-tests for `scripts/check-audit-matrix-columns.py`,
//! retired in the same change this file exists to justify). Same
//! fixtures, same mutations, same assertions -- run against the built
//! `xtask` binary's full `audit-structure` pipeline instead of the
//! standalone Python script.
//!
//! **Not a byte-for-byte subprocess port.** The Python script ran alone,
//! with no write-commands/write-authority/coverage checks around it. Here
//! those run first, in the same binary, so the fixture's `commands.rs`
//! also has to satisfy the write-commands inventory check and the
//! coverage check -- `client.create` and `auth.login.failure` are matrix
//! rows with no sealed descriptor at all, by the Python fixture's own
//! design (exercised by the class check below), so a bare `AuditLogRow`
//! mention of each is added purely to satisfy *coverage's* forward
//! direction without giving either a descriptor, which would defeat the
//! point of the test that uses them.
//!
//! Two of the 19 Python cases do not port as literal assertions, and are
//! replaced by the equivalent fact about the new tool rather than forced
//! to fail for a reason that no longer exists -- see
//! `a_tree_with_no_commands_rs_fails_closed_the_same_way_every_other_lane_does`
//! and the two `a_drifted_*` tests below for why.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_xtask");

const COMMANDS_RS: &str = r#"const STEP_UP_ATTRIBUTE: AttributeSpec = AttributeSpec {
    name: "step_up",
    description: "what authorized the action",
    required: true,
};

static U01_CREATE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserCreate,
    name: "user.create",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[],
};

static U02_DISABLE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::UserDisable,
    name: "user.disable",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec {
            name: "reason",
            description: "why, with a } brace in it",
            required: false,
        },
        STEP_UP_ATTRIBUTE,
    ],
};

static U07_RESET: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::MfaAdminReset,
    name: "mfa.admin_reset",
    class: AuditClass::Atomic,
    actor: ActorRequirement::Optional,
    target: TargetRequirement::Required,
    attributes: &[
        AttributeSpec { name: "totp", description: "", required: false },
        STEP_UP_ATTRIBUTE,
    ],
};

static U08_UNLOCK: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::AdminUserUnlock,
    name: "admin.user.unlock",
    class: AuditClass::Atomic,
    actor: ActorRequirement::None,
    target: TargetRequirement::Required,
    attributes: &[],
};

crate::declare_write_command! {
    /// A doc comment quoting `declare_write_command! { command Z99 = "Z99" }`.
    command U01 = "U01" {
        system_principal: forbidden;
        enum U01Event { Created => &U01_CREATE, }
    }
}

crate::declare_write_command! {
    command U02 = "U02" {
        system_principal: forbidden;
        enum U02Event { Disabled => &U02_DISABLE, }
    }
}

crate::declare_write_command! {
    command U07 = "U07" {
        system_principal: permitted;
        enum U07Event { Reset => &U07_RESET, }
    }
}

crate::declare_write_command! {
    command U08 = "U08" {
        system_principal: permitted;
        enum U08Event { Unlocked => &U08_UNLOCK, }
    }
}

// RFC 096-B1 stage 6b: a `MustAttemptDescriptor`. Deliberately not named by
// any `declare_write_command!` event above -- that absence is what makes it
// Class B rather than Class A, and is the shape this reader must include
// unconditionally rather than through command-reachability.
static F99_TAKEOVER: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationTakeoverBlocked,
    name: "auth.federation.takeover_blocked",
    actor: ActorRequirement::None,
    target: TargetRequirement::None,
    attributes: &[],
};
"#;

/// `client.create` and `auth.login.failure` carry no descriptor at all in
/// the Python fixture, on purpose -- the class check's "no sealed
/// descriptor" path needs a row backed by *nothing*. A bare `AuditLogRow`
/// literal mention satisfies the coverage check's forward direction (this
/// binary runs that check too, unlike the standalone Python script)
/// without adding a descriptor for either name.
const LEGACY_MENTIONS_RS: &str = r#"struct AuditLogRow { action: &'static str }
pub fn mentions() -> [AuditLogRow; 2] {
    [
        AuditLogRow { action: "client.create".into() },
        AuditLogRow { action: "auth.login.failure".into() },
    ]
}
"#;

const WRITE_COMMANDS_TOML: &str = r#"# header

[[command]]
id = "U01"
class = "A"
status = "implemented"
sealed = true
owner = "o"
rationale = "r"
mutation_surface = "`m`"
files = ["crates/sui-id-store/src/repos/things.rs"]
test_id = "`t`"
event = ["user.create"]
descriptor = ["U01_CREATE"]

[[command]]
id = "U02"
class = "A"
status = "implemented"
sealed = true
owner = "o"
rationale = "r"
mutation_surface = "`m`"
files = ["crates/sui-id-store/src/repos/things.rs"]
test_id = "`t`"
event = ["user.disable"]
descriptor = ["U02_DISABLE"]

[[command]]
id = "U07"
class = "A"
status = "implemented"
sealed = true
owner = "o"
rationale = "r"
mutation_surface = "`m`"
files = ["crates/sui-id-store/src/repos/things.rs"]
test_id = "`t`"
event = ["mfa.admin_reset"]
descriptor = ["U07_RESET"]

[[command]]
id = "U08"
class = "A"
status = "implemented"
sealed = true
owner = "o"
rationale = "r"
mutation_surface = "`m`"
files = ["crates/sui-id-store/src/repos/things.rs"]
test_id = "`t`"
event = ["admin.user.unlock"]
descriptor = ["U08_UNLOCK"]
"#;

const MATRIX: &str = "\
# Audit coverage matrix

Prose that mentions `user.create`, `user.disable`, `mfa.admin_reset` and
`admin.user.unlock` by name, which must not count as a row.

## Atomicity classes

| Class | Guarantee |
|---|---|
| **A** | atomic |

### Users

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `user.create` | Create user | admin user id | new user id | — | A |
| `user.disable` | Disable | admin user id | target | reason (optional), `step_up` (required) | A |
| `mfa.admin_reset` | Reset \\| by pipe | admin user id; none for the CLI | target | `totp=…`; `step_up` (required) | A |
| `admin.user.unlock` | Unlock | — *(see note)* | target | — | **A** |
| `client.create` | Create client | admin user id | client id | — | B *(A required)* |

### Flow

| Event name | Trigger | Actor | Class |
|---|---|---|---|
| `auth.login.failure` | A refused attempt | — | B |
| `auth.federation.takeover_blocked` | Collision rejected | — | B |
";

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
            "crates/sui-id-store/src/legacy_mentions.rs",
            LEGACY_MENTIONS_RS,
        );
        f.write("crates/sui-id-store/src/repos/things.rs", "// a repo\n");
        f.write("contracts/write-commands.toml", WRITE_COMMANDS_TOML);
        f.write(
            "contracts/write-authority.toml",
            "version = 1\nallowed_files = []\n",
        );
        f.write("contracts/audit-coverage-matrix.md", MATRIX);
        f
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.dir.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, text).expect("write");
    }

    /// Replace `old` with `new` in the baseline matrix text and write the
    /// result, asserting the replacement actually matched something.
    fn matrix(&self, old: &str, new: &str) {
        let text = MATRIX.replace(old, new);
        assert_ne!(text, MATRIX, "mutation did not apply: {old:?}");
        self.write("contracts/audit-coverage-matrix.md", &text);
    }

    fn source(&self, text: &str) {
        self.write("crates/sui-id-store/src/commands.rs", text);
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

    fn assert_green(&self) {
        let out = self.run_gate();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    }

    fn assert_red(&self, needles: &[&str]) {
        let out = self.run_gate();
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        let text = stderr(&out);
        for needle in needles {
            assert!(text.contains(needle), "missing {needle:?} in: {text}");
        }
    }
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

// ── the baseline, and what it must not trip over ─────────────────────────

#[test]
fn baseline_is_green_and_says_what_it_read() {
    let f = Fixture::new();
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stderr(&out);
    assert!(text.contains("7 table rows, 4 sealed Class-A events; 5 rows checked"));
    assert!(text.contains("EventDescriptor 4"));
    assert!(text.contains("MustAttemptDescriptor 1"));
}

#[test]
fn an_escaped_pipe_does_not_shift_the_columns() {
    // The mfa.admin_reset row carries `\|` in its Operation cell; if the
    // split honoured it wrongly, its Actor cell would read as another
    // column and the actor check would fail.
    Fixture::new().assert_green();
}

#[test]
fn a_fenced_block_and_a_non_table_line_are_not_rows() {
    let f = Fixture::new();
    f.matrix(
        "### Flow",
        "```\n| Event name | Operation | Actor | Target | Note fields | Class |\n\
         |---|---|---|---|---|---|\n| `user.enable` | x | y | z | w | A |\n```\n\n### Flow",
    );
    f.assert_green();
}

#[test]
fn a_tree_with_no_commands_rs_fails_closed_the_same_way_every_other_lane_does() {
    // The Python script, run standalone, skipped and exited 0 on a tree
    // with no `commands.rs`. That shape does not port: this check runs
    // inside `cargo xtask audit-structure`, and the write-commands check
    // ahead of it already fails closed on exactly this tree (stage 6b,
    // unrelated to this stage) -- there is no longer a point at which the
    // column check could run standalone and skip. Asserting the real,
    // pre-existing failure instead of a skip that can no longer happen.
    let f = Fixture::new();
    std::fs::remove_file(f.dir.path().join("crates/sui-id-store/src/commands.rs"))
        .expect("remove commands.rs");
    let out = f.run_gate();
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("no sealed command found"));
}

#[test]
fn a_descriptor_no_command_maps_to_is_not_on_the_seam() {
    // registry.rs's proof-only descriptor: not an event, needs no row.
    // Deliberately re-declares U02/U07/U08's statics under the same
    // names `commands.rs` uses, reachable only there -- the exact shape
    // that would leak reachability across files if it were tracked by
    // bare static name instead of (file, static name).
    let f = Fixture::new();
    let before_commands = COMMANDS_RS
        .split("crate::declare_write_command!")
        .next()
        .unwrap();
    let proof_block = before_commands.replace("user.create", "proof.only");
    f.write(
        "crates/sui-id-store/src/registry.rs",
        &format!(
            "{proof_block}crate::declare_write_command! {{\n    command PROOFONLY = \"PROOFONLY-NOT-AN-INVENTORY-ROW\" {{\n        system_principal: forbidden;\n        enum E {{ X => &U01_CREATE, }}\n    }}\n}}\n"
        ),
    );
    f.assert_green();
}

// ── the mutations the handoff names ───────────────────────────────────────

#[test]
fn a_flipped_class_is_caught_both_ways() {
    let f = Fixture::new();
    f.matrix("| **A** |\n| `client.create`", "| B |\n| `client.create`");
    f.assert_red(&[
        "(class)",
        "admin.user.unlock claims Class B but a sealed Class-A descriptor",
    ]);

    let f = Fixture::new();
    f.matrix(
        "| admin user id | client id | — | B *(A required)* |",
        "| admin user id | client id | — | A |",
    );
    f.assert_red(&["client.create claims Class A but no sealed"]);

    // RFC 096-B1 stage 6b fix round (R1): a `MustAttemptDescriptor` is
    // never in `sealed_atomic` (it is never command-reachable, which is
    // what makes it Class B), so a row claiming A on its strength alone
    // must fail exactly like client.create's case above.
    let f = Fixture::new();
    f.matrix(
        "| `auth.federation.takeover_blocked` | Collision rejected | — | B |",
        "| `auth.federation.takeover_blocked` | Collision rejected | — | A |",
    );
    f.assert_red(&[
        "(class)",
        "auth.federation.takeover_blocked claims Class A but no sealed",
    ]);
}

#[test]
fn a_changed_actor_is_caught_for_each_requirement() {
    // Required, but the cell says none.
    let f = Fixture::new();
    f.matrix(
        "| `user.create` | Create user | admin user id |",
        "| `user.create` | Create user | — |",
    );
    f.assert_red(&[
        "(actor)",
        "user.create",
        "ActorRequirement::Required",
        "reads as None",
    ]);

    // None, but the cell names an actor.
    let f = Fixture::new();
    f.matrix("| — *(see note)* |", "| admin user id |");
    f.assert_red(&[
        "admin.user.unlock",
        "ActorRequirement::None",
        "reads as Required",
    ]);

    // Optional, but the cell names one actor only.
    let f = Fixture::new();
    f.matrix("admin user id; none for the CLI", "admin user id");
    f.assert_red(&[
        "mfa.admin_reset",
        "ActorRequirement::Optional",
        "reads as Required",
    ]);

    // RFC 096-B1 stage 6b fix round (R1): the Actor check applies to any
    // descriptor the matrix names, `MustAttemptDescriptor` included --
    // None, but the cell names an actor.
    let f = Fixture::new();
    f.matrix(
        "| `auth.federation.takeover_blocked` | Collision rejected | — | B |",
        "| `auth.federation.takeover_blocked` | Collision rejected | admin user id | B |",
    );
    f.assert_red(&[
        "auth.federation.takeover_blocked",
        "ActorRequirement::None",
        "reads as Required",
    ]);
}

#[test]
fn a_dropped_step_up_marker_is_caught() {
    let f = Fixture::new();
    f.matrix(
        "reason (optional), `step_up` (required)",
        "reason (optional)",
    );
    f.assert_red(&["(step_up)", "user.disable", "the note does not say so"]);
}

#[test]
fn a_step_up_marker_on_a_command_without_the_attribute_is_caught() {
    let f = Fixture::new();
    f.matrix(
        "| new user id | — | A |",
        "| new user id | `step_up` (required) | A |",
    );
    f.assert_red(&[
        "(step_up)",
        "user.create",
        "no required `step_up` attribute",
    ]);
}

// ── the row-keyed parse (D3b) ──────────────────────────────────────────────

#[test]
fn a_row_deleted_while_its_name_survives_in_prose_is_caught() {
    // G13's name extraction would still see `user.disable` in the prose.
    let f = Fixture::new();
    let lines: Vec<&str> = MATRIX
        .lines()
        .filter(|l| !l.starts_with("| `user.disable`"))
        .collect();
    let text = lines.join("\n") + "\n";
    assert!(text.contains("`user.disable`"));
    f.write("contracts/audit-coverage-matrix.md", &text);
    f.assert_red(&["user.disable is a sealed Class-A event", "no table row"]);
}

#[test]
fn a_table_of_an_unknown_shape_may_not_hold_event_rows() {
    let f = Fixture::new();
    f.matrix(
        "### Flow",
        "### Extra\n\n| Event | Emitted by |\n|---|---|\n| `user.delete` | somewhere |\n\n### Flow",
    );
    f.assert_red(&[
        "(schema)",
        "user.delete",
        "does not start with `Event name`",
    ]);
}

#[test]
fn a_malformed_row_is_caught() {
    let f = Fixture::new();
    f.matrix("| `admin.user.unlock` |", "| admin.user.unlock |");
    f.assert_red(&["(schema)", "not a single backticked event name"]);

    let f = Fixture::new();
    f.matrix("| — | B |\n", "| B |\n");
    f.assert_red(&["(schema)", "cells under a"]);
}

#[test]
fn an_unrecognised_class_cell_is_caught() {
    let f = Fixture::new();
    f.matrix("| — | B |\n", "| — | C |\n");
    f.assert_red(&["(class)", "does not start with A or B"]);
}

#[test]
fn a_second_row_for_one_event_is_caught() {
    let f = Fixture::new();
    let text = format!("{MATRIX}| `auth.login.failure` | again | — | B |\n");
    f.write("contracts/audit-coverage-matrix.md", &text);
    f.assert_red(&["auth.login.failure has a second row"]);
}

// ── the source side ────────────────────────────────────────────────────────

#[test]
fn a_must_attempt_descriptor_is_not_class_a() {
    // `AuditClass::MustAttempt` does not exist in the real enum; `syn`
    // parses it as written (it is not a type checker), so the class
    // check reads the variant name verbatim and still catches the claim.
    let src = COMMANDS_RS.replace(
        "name: \"user.create\",\n    class: AuditClass::Atomic",
        "name: \"user.create\",\n    class: AuditClass::MustAttempt",
    );
    assert_ne!(src, COMMANDS_RS);
    let f = Fixture::new();
    f.source(&src);
    f.assert_red(&["user.create claims Class A but no sealed"]);
}

#[test]
fn a_descriptor_this_reader_cannot_parse_fails_loudly() {
    let src = COMMANDS_RS.replace("kind: AuditEventKind::UserCreate,\n    ", "");
    assert_ne!(src, COMMANDS_RS);
    let f = Fixture::new();
    f.source(&src);
    f.assert_red(&["(source)", "not in the shape"]);
}

#[test]
fn a_new_sealed_command_without_a_row_is_caught() {
    let src = format!(
        "{COMMANDS_RS}\nstatic U05_ROLE: EventDescriptor = EventDescriptor {{\n    kind: AuditEventKind::UserRoleChange,\n    name: \"user.role_change\",\n    class: AuditClass::Atomic,\n    actor: ActorRequirement::Required,\n    target: TargetRequirement::Required,\n    attributes: &[],\n}};\ncrate::declare_write_command! {{\n    command U05 = \"U05\" {{\n        system_principal: forbidden;\n        enum U05Event {{ Changed => &U05_ROLE, }}\n    }}\n}}\n"
    );
    let f = Fixture::new();
    f.source(&src);
    f.write(
        "contracts/write-commands.toml",
        &format!(
            "{WRITE_COMMANDS_TOML}\n[[command]]\nid = \"U05\"\nclass = \"A\"\nstatus = \"implemented\"\nsealed = true\nowner = \"o\"\nrationale = \"r\"\nmutation_surface = \"`m`\"\nfiles = [\"crates/sui-id-store/src/repos/things.rs\"]\ntest_id = \"`t`\"\nevent = [\"user.role_change\"]\ndescriptor = [\"U05_ROLE\"]\n"
        ),
    );
    f.assert_red(&["user.role_change is a sealed Class-A event"]);
}

// ── the strict-vs-loose declaration check does not port ────────────────────
//
// `rust_registry.py` carries a second, looser regex purely to detect its
// own strict regex's blind spot: a path-qualified or multi-line descriptor
// declaration that the strict pattern cannot see. `syn` has no such blind
// spot -- `is_type_named` matches a type's last path segment regardless of
// qualification or line breaks -- so there is no mismatch for these two
// cases to produce. Both are rewritten to assert the thing that is now
// true: the previously-invisible shape is found and used, not missed.

#[test]
fn a_path_qualified_multi_line_must_attempt_descriptor_is_found_not_missed() {
    // The exact shape that made `AUDIT_TAKEOVER_BLOCKED_DESCRIPTOR`
    // invisible to the strict Python parser even after its own fix
    // landed: a path-qualified type, split across two lines.
    let src = format!(
        "{COMMANDS_RS}\nstatic F98_DRIFTED: crate::registry::MustAttemptDescriptor =\n    crate::registry::MustAttemptDescriptor {{\n        kind: AuditEventKind::AuthFederationLinkRequired,\n        name: \"auth.federation.drifted_example\",\n        actor: ActorRequirement::None,\n        target: TargetRequirement::None,\n        attributes: &[],\n    }};\n"
    );
    let f = Fixture::new();
    f.source(&src);
    // A row-less MustAttemptDescriptor is a coverage *backward* obligation
    // (it is a real source name now, included unconditionally like
    // F99_TAKEOVER), not a (declarations) mismatch -- proving it was
    // actually parsed, not silently dropped the way the strict regex
    // would have dropped it.
    f.assert_red(&["(coverage backward) auth.federation.drifted_example"]);
}

#[test]
fn a_path_qualified_event_descriptor_is_found_not_missed() {
    // The same drift, on the `EventDescriptor` side: a path-qualified type
    // on its own line is enough, even without the Must-Attempt case's
    // split across lines.
    let src = format!(
        "{COMMANDS_RS}\nstatic U06_DRIFTED: crate::registry::EventDescriptor = crate::registry::EventDescriptor {{\n    kind: AuditEventKind::UserRoleChange,\n    name: \"user.drifted_example\",\n    class: AuditClass::Atomic,\n    actor: ActorRequirement::Required,\n    target: TargetRequirement::Required,\n    attributes: &[],\n}};\n"
    );
    let f = Fixture::new();
    f.source(&src);
    // Found, but unreachable (no command maps to it) -- excluded from the
    // *columns* registry the same way `registry.rs`'s proof-only
    // descriptor is, so no class-check row is demanded for it. What
    // proves it was actually parsed by `syn` (not silently dropped the
    // way the strict Python regex would drop it) is that `coverage.rs`'s
    // own source-reading -- which sees every `EventDescriptor` static
    // regardless of command-reachability -- reports it as a real,
    // uncovered backward obligation rather than missing it entirely.
    f.assert_red(&["(coverage backward) user.drifted_example"]);
}
