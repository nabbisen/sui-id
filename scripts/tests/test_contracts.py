"""RFC 116 stage 6 (G18) negative self-tests for scripts/check-contracts.py.

Run as: python3.14 -m unittest scripts.tests.test_contracts

Each fixture is a throwaway git repository (the gate enumerates files with
`git ls-files`, so an ignored file must not be able to satisfy or fail it) with a
minimal contracts directory, a manifest, one RFC lane table and a policy, mutated
one way per test. The checker is invoked as a subprocess. The mutations are the
ones the handoff names: a path that does not exist, a file with no README row, a
row with no file, a gate that is not in the Gate Matrix, and an allow-list entry
with no reason.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-contracts.py"

POLICY = """\
version = 1
directory = "ci"
former_directories = []
manifest = "ci/gate-inputs.toml"
readme_file = "ci/README.md"
extensions = ["toml", "md", "txt", "py"]

[readme]
kinds = ["policy", "registry", "generator input", "baseline"]
must_name = ["scripts/ci-gate.sh"]

[[record]]
path = "rfcs/handoffs"
reason = "dated companions"

[[allow]]
path = "ci/planned.toml"
reason = "planned by an RFC, not yet built"
"""

MANIFEST = """\
version = 1

[gate_lane_sources]
"116" = "Gate Matrix lanes owned by RFC 116"

[gate_owners]
G17 = "116"
G18 = "116"

[gates]
G17 = "python3.14 scripts/check-a.py --policy ci/a.toml"
G18 = "python3.14 scripts/check-contracts.py --root . --policy ci/contract-paths.toml"
"""

RFC = """\
# RFC 116 - fixture

## Gate Matrix lanes owned by RFC 116

| ID | Toolchain | Features | Blocking command / assertion |
|---|---|---|---|
| G17 | Python 3.14 | n/a | `python3.14 scripts/check-a.py --policy ci/a.toml` |
| G18 | Python 3.14 | n/a | `python3.14 scripts/check-contracts.py --root . --policy ci/contract-paths.toml` |
"""

README = """\
# ci

The contracts a gate compares against. CI is one caller and `scripts/ci-gate.sh`
is the other.

| File | Kind | Read by | Owning RFC |
|---|---|---|---|
| `a.toml` | policy | G17 | RFC 116 |
| `gate-inputs.toml` | registry, generator input | `scripts/check-a.py` | RFC 116 |
| `contract-paths.toml` | policy | G18 | RFC 116 |
"""


class Contracts(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self._git("init", "-q", "-b", "main")
        self.write("ci/contract-paths.toml", POLICY)
        self.write("ci/gate-inputs.toml", MANIFEST)
        self.write("ci/a.toml", "x = 1\n")
        self.write("ci/README.md", README)
        self.write("rfcs/accepted/116-fixture.md", RFC)
        self.write("rfcs/handoffs/116-x/README.md", "A dated companion.\n")
        # Names the allowed, not-yet-built file, so the allowance is in use.
        self.write("rfcs/accepted/094-x.md", "It will be `ci/planned.toml`.\n")
        # Named by a machine-read file, so the (read) proxy is satisfied.
        self.write("scripts/check-a.py", "# reads ci/a.toml, ci/gate-inputs.toml and ci/contract-paths.toml\n")

    def _git(self, *args: str) -> None:
        subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)

    def write(self, rel: str, text: str) -> None:
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def read(self, rel: str) -> str:
        return (self.root / rel).read_text(encoding="utf-8")

    def mutate(self, rel: str, old: str, new: str) -> None:
        text = self.read(rel)
        self.assertIn(old, text, f"{rel}: {old!r}")
        self.write(rel, text.replace(old, new))

    def run_gate(self) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(self.root), "--policy", "ci/contract-paths.toml"],
            capture_output=True,
            text=True,
        )

    def assert_green(self) -> None:
        r = self.run_gate()
        self.assertEqual(r.returncode, 0, r.stderr)

    def assert_red(self, *needles: str) -> subprocess.CompletedProcess:
        r = self.run_gate()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        for needle in needles:
            self.assertIn(needle, r.stderr)
        return r

    def assert_policy_error(self, *needles: str) -> None:
        r = self.run_gate()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
        for needle in needles:
            self.assertIn(needle, r.stderr)

    # ── the baseline, and what it must not trip over ─────────────────────

    def test_baseline_is_green(self) -> None:
        r = self.run_gate()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("3 contract files", r.stdout)

    def test_the_real_repository_is_green(self) -> None:
        r = subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(REPO_ROOT), "--policy", "ci/contract-paths.toml"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_a_path_that_is_not_this_directory_is_not_a_reference(self) -> None:
        self.write(
            "docs/x.md",
            "See scripts/ci/nothing.toml, oci/nothing.toml, https://example.com/ci/nothing.toml "
            "and ci/ on its own, and ci/no-extension.\n",
        )
        self.assert_green()

    # ── (paths) ──────────────────────────────────────────────────────────

    def test_a_reference_to_a_file_that_does_not_exist_is_caught(self) -> None:
        self.write("docs/guide.md", "Edit `ci/nothing.toml` first.\n")
        self.assert_red("(paths) docs/guide.md:1", "ci/nothing.toml", "does not exist")

    def test_a_record_keeps_its_path_and_a_live_file_does_not(self) -> None:
        self.write("rfcs/handoffs/116-x/notes.md", "The manifest was `ci/gone.toml` then.\n")
        self.assert_green()
        self.write("rfcs/accepted/117-live.md", "The manifest is `ci/gone.toml`.\n")
        self.assert_red("rfcs/accepted/117-live.md:1")

    def test_a_file_is_in_scope_unless_a_record_exempts_it(self) -> None:
        # A new top-level directory is scanned by default, not missed by default.
        self.write("brand-new-dir/x.md", "Read `ci/nothing.toml`.\n")
        self.assert_red("brand-new-dir/x.md:1")

    def test_an_allowed_name_passes_and_a_stale_allowance_fails(self) -> None:
        self.assert_green()
        (self.root / "rfcs/accepted/094-x.md").unlink()
        self.assert_red("[[allow]] ci/planned.toml is stale", "planned by an RFC")

    def test_an_allowance_without_a_reason_is_a_policy_error(self) -> None:
        self.mutate("ci/contract-paths.toml", 'reason = "planned by an RFC, not yet built"', 'reason = "  "')
        self.assert_policy_error("[[allow]] ci/planned.toml has no reason", "hiding place")

    def test_a_record_without_a_reason_is_a_policy_error(self) -> None:
        self.mutate("ci/contract-paths.toml", 'reason = "dated companions"', 'reason = ""')
        self.assert_policy_error("[[record]] rfcs/handoffs has no reason")

    def test_a_record_that_matches_nothing_is_stale(self) -> None:
        self.mutate("ci/contract-paths.toml", 'path = "rfcs/handoffs"', 'path = "rfcs/nowhere"')
        self.assert_red("[[record]] rfcs/nowhere matches no file")

    def test_a_former_directory_names_where_the_file_moved(self) -> None:
        self.mutate("ci/contract-paths.toml", "former_directories = []", 'former_directories = ["old"]')
        self.write("docs/guide.md", "Edit `old/a.toml`, and `old/vanished.toml`.\n")
        r = self.assert_red("names old/a.toml, a former directory; it moved to ci/a.toml")
        self.assertIn("old/vanished.toml, a former directory; and it is not in the directory either", r.stderr)
        # ...but not in a record, which keeps the path it had.
        (self.root / "docs/guide.md").unlink()
        self.write("rfcs/handoffs/x.md", "Edit `old/a.toml`.\n")
        self.assert_green()

    # ── (readme) ─────────────────────────────────────────────────────────

    def test_a_file_with_no_readme_row_is_caught(self) -> None:
        self.write("ci/extra.toml", "y = 1\n")
        self.write("scripts/check-a.py", self.read("scripts/check-a.py") + "# and ci/extra.toml\n")
        self.assert_red("ci/extra.toml has no row in ci/README.md")

    def test_a_row_with_no_file_is_caught(self) -> None:
        self.mutate("ci/README.md", "| `a.toml` | policy | G17 | RFC 116 |", "| `a.toml` | policy | G17 | RFC 116 |\n| `ghost.toml` | policy | G17 | RFC 116 |")
        self.assert_red("names ghost.toml, which is not in ci/")

    def test_a_second_row_for_one_file_is_caught(self) -> None:
        self.mutate("ci/README.md", "| `a.toml` | policy | G17 | RFC 116 |", "| `a.toml` | policy | G17 | RFC 116 |\n| `a.toml` | policy | G17 | RFC 116 |")
        self.assert_red("a.toml has a second row")

    def test_a_kind_outside_the_vocabulary_is_caught(self) -> None:
        self.mutate("ci/README.md", "| `a.toml` | policy |", "| `a.toml` | secret sauce |")
        self.assert_red("Kind 'secret sauce' is not one of")

    def test_a_gate_that_is_not_in_gates_is_caught(self) -> None:
        self.mutate("ci/README.md", "| `a.toml` | policy | G17 |", "| `a.toml` | policy | G99 |")
        self.assert_red("G99 is not in [gates]")

    def test_a_gate_in_gates_but_not_in_the_gate_matrix_is_caught(self) -> None:
        self.mutate("rfcs/accepted/116-fixture.md", "| G17 | Python 3.14 | n/a | `python3.14 scripts/check-a.py --policy ci/a.toml` |\n", "")
        self.assert_red("G17 is in [gates] but has no row in the Gate Matrix")

    def test_a_gate_whose_owner_rfc_does_not_exist_is_caught(self) -> None:
        (self.root / "rfcs/accepted/116-fixture.md").unlink()
        self.assert_red("owner RFC 116 resolves to 0 files")

    def test_a_script_that_does_not_exist_is_caught(self) -> None:
        self.mutate("ci/README.md", "`scripts/check-a.py`", "`scripts/nope.py`")
        self.assert_red("Read by names scripts/nope.py, which does not exist")

    def test_an_rfc_that_does_not_exist_is_caught(self) -> None:
        self.mutate("ci/README.md", "| `a.toml` | policy | G17 | RFC 116 |", "| `a.toml` | policy | G17 | RFC 777 |")
        self.assert_red("RFC 777 resolves to 0 files")
        self.mutate("ci/README.md", "RFC 777", "the architect")
        self.assert_red("names no RFC")

    def test_an_empty_read_by_cell_is_caught(self) -> None:
        self.mutate("ci/README.md", "| `a.toml` | policy | G17 |", "| `a.toml` | policy |  |")
        self.assert_red("empty Read by cell")

    def test_a_readme_that_does_not_name_the_dispatcher_is_caught(self) -> None:
        self.mutate("ci/README.md", "`scripts/ci-gate.sh`", "the dispatcher")
        self.assert_red("does not name scripts/ci-gate.sh")

    def test_a_malformed_row_is_caught(self) -> None:
        self.mutate("ci/README.md", "| `a.toml` | policy | G17 | RFC 116 |", "| `a.toml` | policy | G17 |")
        self.assert_red("3 cells; a row is File | Kind | Read by | Owning RFC")
        self.mutate("ci/README.md", "| `a.toml` | policy | G17 |", "| a.toml | policy | G17 | RFC 116 |")
        self.assert_red("not one backticked file name")

    def test_a_missing_readme_is_caught(self) -> None:
        (self.root / "ci/README.md").unlink()
        self.assert_red("ci/README.md does not exist")

    # ── (read) ───────────────────────────────────────────────────────────

    def test_a_file_named_by_nothing_machine_read_is_caught(self) -> None:
        # A file with a README row and no machine-read reference: nothing can fail
        # when it stops being true.
        self.write("ci/orphan.toml", "z = 1\n")
        self.mutate("ci/README.md", "| `a.toml` | policy | G17 | RFC 116 |", "| `a.toml` | policy | G17 | RFC 116 |\n| `orphan.toml` | policy | G17 | RFC 116 |")
        self.assert_red("(read) ci/orphan.toml is named by no file under scripts/")
        self.write("scripts/check-a.py", self.read("scripts/check-a.py") + "# and ci/orphan.toml\n")
        self.assert_green()

    # ── the unreadable ───────────────────────────────────────────────────

    def test_a_missing_policy_is_exit_two(self) -> None:
        (self.root / "ci/contract-paths.toml").unlink()
        self.assert_policy_error("cannot read policy")


if __name__ == "__main__":
    unittest.main()
