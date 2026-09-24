"""RFC 116 stage 1 negative self-tests for scripts/check-write-commands.py.

Run as: python3.14 -m unittest scripts.tests.test_write_commands

Each fixture is a throwaway synthetic tree carrying a minimal valid inventory
and a minimal `commands.rs`, mutated one way per test. The checker is invoked
as a subprocess, matching this project's convention of testing checkers as
black boxes. The mutations are the ones RFC 116's handoff names -- a command
added without a row, a row naming a command that does not exist, a `files`
path that is wrong -- plus one per remaining condition.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-write-commands.py"

COMMANDS_RS = """\
static A01_DONE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::Thing,
    name: "thing.done",
    class: AuditClass::Atomic,
    attributes: &[AttributeSpec { name: "note", description: "", required: false }],
};

static A01_FAILED: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::Thing,
    name: "thing.failed",
    class: AuditClass::Atomic,
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
"""

SECOND_RS = """\
static B02_DONE: EventDescriptor = EventDescriptor {
    kind: AuditEventKind::Other,
    name: "other.done",
    class: AuditClass::Atomic,
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
"""

HEADER = """\
# The inventory. Two states: sealed, planned.
# There is no count in this header on purpose.

"""

SEALED_A01 = """\
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
event = ["thing.done", "thing.failed"]
descriptor = ["A01_DONE", "A01_FAILED"]
"""

PLANNED_P01 = """\
[[command]]
id = "P01"
class = "A"
status = "target-absent"
sealed = false
owner = "Later"
rationale = "r"
mutation_surface = "`m`"
files = []
test_id = "`t`"
"""

SEALED_B02 = """\
[[command]]
id = "B02"
class = "A"
status = "implemented"
sealed = true
owner = "Another"
rationale = "r"
mutation_surface = "`m`"
files = ["crates/store/src/repos/things.rs"]
test_id = "`t`"
event = ["other.done"]
descriptor = ["B02_DONE"]
"""


def rows(*blocks: str) -> str:
    return HEADER + "\n".join(blocks)


class WriteCommandsGate(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.write("crates/store/src/commands.rs", COMMANDS_RS)
        self.write("crates/store/src/repos/things.rs", "// a repo\n")
        self.write("ci/write-commands.toml", rows(SEALED_A01, PLANNED_P01))

    def write(self, rel: str, text: str) -> None:
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def inventory(self) -> str:
        return (self.root / "ci/write-commands.toml").read_text(encoding="utf-8")

    def run_gate(self, *extra: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(self.root),
             "--inventory", "ci/write-commands.toml", *extra],
            capture_output=True,
            text=True,
        )

    def assert_green(self) -> None:
        r = self.run_gate()
        self.assertEqual(r.returncode, 0, r.stderr)

    def assert_red(self, *needles: str) -> None:
        r = self.run_gate()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        for needle in needles:
            self.assertIn(needle, r.stderr)

    # ── the baseline, and what it must not trip over ─────────────────────

    def test_baseline_is_green_and_counts_are_derived(self) -> None:
        r = self.run_gate()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("2 rows: 1 sealed, 1 planned", r.stdout)

    def test_a_planned_row_needs_no_command(self) -> None:
        # P01 names no command in the code and the gate is green: this is D2a.
        self.assert_green()

    def test_a_doc_comment_or_test_file_or_macro_definition_does_not_declare(self) -> None:
        self.write(
            "crates/store/src/tests.rs",
            'crate::declare_write_command! {\n command Q01 = "Q01" {\n'
            " system_principal: forbidden; enum E { X => &S } } }\n",
        )
        self.write(
            "crates/store/src/macros.rs",
            "// declare_write_command! { command Q02 = \"Q02\" { } }\n"
            "macro_rules! declare_write_command { () => {}; }\n",
        )
        self.assert_green()

    # ── the three mutations the handoff names ────────────────────────────

    def test_a_command_added_without_a_row_is_caught(self) -> None:
        self.write("crates/store/src/more.rs", SECOND_RS)
        self.assert_red("(A) command B02", "has no row in the inventory")

    def test_a_row_naming_a_command_that_does_not_exist_is_caught(self) -> None:
        self.write("ci/write-commands.toml", rows(SEALED_A01, PLANNED_P01, SEALED_B02))
        self.assert_red("(B) row B02", "no command B02 is declared")

    def test_a_wrong_files_path_is_caught(self) -> None:
        text = self.inventory().replace("repos/things.rs", "repos/nothing.rs")
        self.write("ci/write-commands.toml", text)
        self.assert_red("(D) row A01", "repos/nothing.rs", "does not exist")

    # ── the rest of the contract ─────────────────────────────────────────

    def test_a_planned_row_whose_command_has_been_sealed_is_caught(self) -> None:
        self.write("ci/write-commands.toml", rows(SEALED_A01, PLANNED_P01.replace('"P01"', '"B02"')))
        self.write("crates/store/src/more.rs", SECOND_RS)
        self.assert_red("(A) command B02", "`sealed = false`")

    def test_a_wrong_event_list_is_caught(self) -> None:
        text = self.inventory().replace('event = ["thing.done", "thing.failed"]', 'event = ["thing.done"]')
        self.write("ci/write-commands.toml", text)
        self.assert_red("(C) row A01", "`event`")

    def test_a_wrong_descriptor_list_is_caught(self) -> None:
        text = self.inventory().replace('"A01_FAILED"]', '"A01_OTHER"]')
        self.write("ci/write-commands.toml", text)
        self.assert_red("(C) row A01", "`descriptor`")

    def test_a_sealed_row_without_its_derived_fields_is_caught(self) -> None:
        lines = [l for l in self.inventory().splitlines() if not l.startswith(("event =", "descriptor ="))]
        self.write("ci/write-commands.toml", "\n".join(lines) + "\n")
        self.assert_red("(C) row A01", "`event` is missing", "`descriptor` is missing")

    def test_a_planned_row_carrying_a_derived_field_is_caught(self) -> None:
        text = self.inventory().replace('files = []\n', 'files = []\nevent = ["x.y"]\n')
        self.write("ci/write-commands.toml", text)
        self.assert_red("(C) row P01", "carries no `event`")

    def test_target_absent_may_not_name_a_file_and_others_must(self) -> None:
        text = self.inventory().replace('files = []', 'files = ["crates/store/src/repos/things.rs"]')
        self.write("ci/write-commands.toml", text)
        self.assert_red("(D) row P01", "`target-absent` names no file")
        text = self.inventory().replace('files = ["crates/store/src/repos/things.rs"]', "files = []")
        self.write("ci/write-commands.toml", text)
        self.assert_red("(D) row A01", "names no file")

    def test_a_command_whose_id_is_not_of_the_inventory_shape_is_caught(self) -> None:
        self.write("crates/store/src/more.rs", SECOND_RS.replace('"B02"', '"NOT-AN-ID"'))
        self.assert_red("more.rs", "not of the inventory's shape")

    def test_the_registry_proof_only_command_is_tolerated_there_and_only_there(self) -> None:
        proof = SECOND_RS.replace('"B02"', '"PROOFONLY-NOT-AN-INVENTORY-ROW"')
        self.write("crates/sui-id-store/src/registry.rs", proof)
        self.assert_green()

    def test_a_command_declared_twice_is_caught(self) -> None:
        self.write("crates/store/src/more.rs", COMMANDS_RS)
        self.assert_red("command A01 is declared twice")

    def test_a_malformed_row_is_caught(self) -> None:
        text = self.inventory().replace('class = "A"\nstatus = "implemented"', 'class = "Z"\nstatus = "done"')
        self.write("ci/write-commands.toml", text)
        self.assert_red("status 'done'", "class 'Z'")
        self.write("ci/write-commands.toml", self.inventory().replace('sealed = true\n', ''))
        self.assert_red("missing `sealed`")
        self.write("ci/write-commands.toml", rows(SEALED_A01, SEALED_A01, PLANNED_P01))
        self.assert_red("id appears twice")

    def test_a_sealed_row_must_be_implemented_and_class_a(self) -> None:
        text = self.inventory().replace('class = "A"\nstatus = "implemented"', 'class = "P"\nstatus = "target-legacy"', 1)
        self.write("ci/write-commands.toml", text)
        self.assert_red("must be `implemented`", "Class A by construction")

    def test_an_unknown_key_is_caught(self) -> None:
        text = self.inventory().replace('owner = "Do the thing"', 'owner = "Do the thing"\nnotes = "x"')
        self.write("ci/write-commands.toml", text)
        self.assert_red("unknown key `notes`")

    def test_a_hand_written_count_in_the_header_is_caught(self) -> None:
        self.write("ci/write-commands.toml", "# implemented -- code exists (82)\n\n" + self.inventory())
        self.assert_red("(F)", "hand-written count")
        self.write("ci/write-commands.toml", "# 99 entries in all.\n\n" + self.inventory())
        self.assert_red("(F)", "hand-written count")

    def test_no_command_at_all_fails_closed(self) -> None:
        self.write("crates/store/src/commands.rs", "// nothing\n")
        r = self.run_gate()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
        self.assertIn("fails closed", r.stderr)

    def test_an_unreadable_inventory_is_exit_two(self) -> None:
        self.write("ci/write-commands.toml", "this is = not [valid")
        r = self.run_gate()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)

    # ── --fix ────────────────────────────────────────────────────────────

    def test_fix_rewrites_derived_fields_and_nothing_else(self) -> None:
        stale = (
            self.inventory()
            .replace('event = ["thing.done", "thing.failed"]', 'event = ["wrong"]')
            .replace('descriptor = ["A01_DONE", "A01_FAILED"]', 'descriptor = []')
        )
        stale = stale.replace('files = []\n', 'files = []\nevent = ["x.y"]\n')
        self.write("ci/write-commands.toml", stale)
        self.assert_red("(C)")
        r = self.run_gate("--fix")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.inventory(), rows(SEALED_A01, PLANNED_P01).rstrip("\n") + "\n")
        self.assert_green()

    def test_fix_does_not_hide_a_missing_row(self) -> None:
        self.write("crates/store/src/more.rs", SECOND_RS)
        r = self.run_gate("--fix")
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("(A) command B02", r.stderr)

    def test_fix_does_not_make_a_dead_path_pass(self) -> None:
        self.write("ci/write-commands.toml", self.inventory().replace("repos/things.rs", "repos/nothing.rs"))
        r = self.run_gate("--fix")
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("(D) row A01", r.stderr)


if __name__ == "__main__":
    unittest.main()
