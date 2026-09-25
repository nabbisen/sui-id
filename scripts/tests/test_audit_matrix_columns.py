"""RFC 116 stage 2 negative self-tests for scripts/check-audit-matrix-columns.py.

Run as: python3.14 -m unittest scripts.tests.test_audit_matrix_columns

Each fixture is a throwaway synthetic tree with a minimal `commands.rs` and a
minimal audit matrix, mutated one way per test. The checker is invoked as a
subprocess. The mutations are the ones the handoff names for stage 2 -- flip a
row's class, change an actor, drop a required attribute -- plus the row-keyed
parse RFC 116 D3b exists for: a row deleted while its name survives in prose.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-audit-matrix-columns.py"

COMMANDS_RS = """\
const STEP_UP_ATTRIBUTE: AttributeSpec = AttributeSpec {
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
"""

MATRIX = """\
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
"""


class MatrixColumns(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.write("crates/sui-id-store/src/commands.rs", COMMANDS_RS)
        self.write("contracts/audit-coverage-matrix.md", MATRIX)

    def write(self, rel: str, text: str) -> None:
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def matrix(self, old: str | None = None, new: str = "") -> None:
        text = MATRIX if old is None else MATRIX.replace(old, new)
        if old is not None:
            self.assertNotEqual(text, MATRIX, f"mutation did not apply: {old!r}")
        self.write("contracts/audit-coverage-matrix.md", text)

    def run_checker(self) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(self.root),
             "--matrix", "contracts/audit-coverage-matrix.md"],
            capture_output=True,
            text=True,
        )

    def assert_green(self) -> None:
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)

    def assert_red(self, *needles: str) -> None:
        r = self.run_checker()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        for needle in needles:
            self.assertIn(needle, r.stderr)

    # ── the baseline, and what it must not trip over ─────────────────────

    def test_baseline_is_green_and_says_what_it_read(self) -> None:
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("6 table rows, 4 sealed Class-A events; 4 rows checked", r.stdout)

    def test_an_escaped_pipe_does_not_shift_the_columns(self) -> None:
        # The mfa.admin_reset row carries `\\|` in its Operation cell; if the
        # split honoured it wrongly, its Actor cell would read as another
        # column and the actor check would fail.
        self.assert_green()

    def test_a_fenced_block_and_a_non_table_line_are_not_rows(self) -> None:
        self.matrix(
            "### Flow",
            "```\n| Event name | Operation | Actor | Target | Note fields | Class |\n"
            "|---|---|---|---|---|---|\n| `user.enable` | x | y | z | w | A |\n```\n\n### Flow",
        )
        self.assert_green()

    def test_a_tree_with_no_commands_rs_is_skipped_and_says_so(self) -> None:
        (self.root / "crates/sui-id-store/src/commands.rs").unlink()
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("skipped", r.stdout)

    def test_a_descriptor_no_command_maps_to_is_not_on_the_seam(self) -> None:
        # registry.rs's proof-only descriptor: not an event, needs no row.
        self.write(
            "crates/sui-id-store/src/registry.rs",
            COMMANDS_RS.split("crate::declare_write_command!")[0].replace("user.create", "proof.only")
            + 'crate::declare_write_command! {\n command PROOFONLY = "PROOFONLY-NOT-AN-INVENTORY-ROW" {\n'
            " system_principal: forbidden;\n enum E { X => &U01_CREATE, } } }\n",
        )
        self.assert_green()

    # ── the mutations the handoff names ──────────────────────────────────

    def test_a_flipped_class_is_caught_both_ways(self) -> None:
        self.matrix("| **A** |\n| `client.create`", "| B |\n| `client.create`")
        self.assert_red("(class)", "admin.user.unlock claims Class B but a sealed Class-A descriptor")
        self.matrix("| admin user id | client id | — | B *(A required)* |", "| admin user id | client id | — | A |")
        self.assert_red("client.create claims Class A but no sealed")

    def test_a_changed_actor_is_caught_for_each_requirement(self) -> None:
        # Required, but the cell says none.
        self.matrix("| `user.create` | Create user | admin user id |", "| `user.create` | Create user | — |")
        self.assert_red("(actor)", "user.create", "ActorRequirement::Required", "reads as None")
        # None, but the cell names an actor.
        self.matrix("| — *(see note)* |", "| admin user id |")
        self.assert_red("admin.user.unlock", "ActorRequirement::None", "reads as Required")
        # Optional, but the cell names one actor only.
        self.matrix("admin user id; none for the CLI", "admin user id")
        self.assert_red("mfa.admin_reset", "ActorRequirement::Optional", "reads as Required")

    def test_a_dropped_step_up_marker_is_caught(self) -> None:
        self.matrix("reason (optional), `step_up` (required)", "reason (optional)")
        self.assert_red("(step_up)", "user.disable", "the note does not say so")

    def test_a_step_up_marker_on_a_command_without_the_attribute_is_caught(self) -> None:
        self.matrix("| new user id | — | A |", "| new user id | `step_up` (required) | A |")
        self.assert_red("(step_up)", "user.create", "no required `step_up` attribute")

    # ── the row-keyed parse (D3b) ────────────────────────────────────────

    def test_a_row_deleted_while_its_name_survives_in_prose_is_caught(self) -> None:
        # G13's name extraction would still see `user.disable` in the prose.
        lines = [l for l in MATRIX.splitlines() if not l.startswith("| `user.disable`")]
        self.write("contracts/audit-coverage-matrix.md", "\n".join(lines) + "\n")
        self.assertIn("`user.disable`", "\n".join(lines))
        self.assert_red("user.disable is a sealed Class-A event", "no table row")

    def test_a_table_of_an_unknown_shape_may_not_hold_event_rows(self) -> None:
        self.matrix(
            "### Flow",
            "### Extra\n\n| Event | Emitted by |\n|---|---|\n| `user.delete` | somewhere |\n\n### Flow",
        )
        self.assert_red("(schema)", "user.delete", "does not start with `Event name`")

    def test_a_malformed_row_is_caught(self) -> None:
        self.matrix("| `admin.user.unlock` |", "| admin.user.unlock |")
        self.assert_red("(schema)", "not a single backticked event name")
        self.matrix("| — | B |\n", "| B |\n")
        self.assert_red("(schema)", "cells under a")

    def test_an_unrecognised_class_cell_is_caught(self) -> None:
        self.matrix("| — | B |\n", "| — | C |\n")
        self.assert_red("(class)", "does not start with A or B")

    def test_a_second_row_for_one_event_is_caught(self) -> None:
        self.write("contracts/audit-coverage-matrix.md", MATRIX + "| `auth.login.failure` | again | — | B |\n")
        self.assert_red("auth.login.failure has a second row")

    # ── the source side ──────────────────────────────────────────────────

    def test_a_must_attempt_descriptor_is_not_class_a(self) -> None:
        src = COMMANDS_RS.replace(
            'name: "user.create",\n    class: AuditClass::Atomic',
            'name: "user.create",\n    class: AuditClass::MustAttempt',
        )
        self.assertNotEqual(src, COMMANDS_RS)
        self.write("crates/sui-id-store/src/commands.rs", src)
        self.assert_red("user.create claims Class A but no sealed")

    def test_a_descriptor_this_reader_cannot_parse_fails_loudly(self) -> None:
        src = COMMANDS_RS.replace("kind: AuditEventKind::UserCreate,\n    ", "")
        self.write("crates/sui-id-store/src/commands.rs", src)
        self.assert_red("(source)", "not in the shape")

    def test_a_new_sealed_command_without_a_row_is_caught(self) -> None:
        src = COMMANDS_RS + (
            'static U05_ROLE: EventDescriptor = EventDescriptor {\n'
            '    kind: AuditEventKind::UserRoleChange,\n    name: "user.role_change",\n'
            "    class: AuditClass::Atomic,\n    actor: ActorRequirement::Required,\n"
            "    target: TargetRequirement::Required,\n    attributes: &[],\n};\n"
            'crate::declare_write_command! {\n    command U05 = "U05" {\n'
            "        system_principal: forbidden;\n        enum U05Event { Changed => &U05_ROLE, }\n    }\n}\n"
        )
        self.write("crates/sui-id-store/src/commands.rs", src)
        self.assert_red("user.role_change is a sealed Class-A event")


if __name__ == "__main__":
    unittest.main()
