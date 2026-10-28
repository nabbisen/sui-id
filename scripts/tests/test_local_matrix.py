"""RFC 093 cycle H item 4 self-tests for scripts/local-matrix.sh.

Run as: python3.14 -m unittest scripts.tests.test_local_matrix

scripts/local-matrix.sh derives its full set of checks from
`.github/workflows/ci.yml` rather than a hand-written list, because a
hand-written list is exactly what went wrong twice in four consecutive
`0.85.0` rounds: a dispatch's own count came back thirteen of the fourteen
`python3.14 -m unittest scripts.tests.test_*` steps, missing
`test_markdown_links`, and before that none of the four rounds ran those
fourteen steps at all. This module's own derivation (`ci_yml_derived_set`)
is written independently of the shell script's own awk/grep -- in Python,
against the same real `.github/workflows/ci.yml` -- rather than by
importing or re-using the script's logic, so a bug shared between the two
would not go undetected here.

Fixtures are synthetic `.github/workflows/ci.yml` files in a tempdir,
following this project's existing convention (test_generate_ci_workflow.py)
of mutating real project content rather than freestanding look-alikes:
the "missing lane" fixture is built from the real file's own unittest
lines, minus one, rather than a hand-typed count retyped from scratch.
"""

import re
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "local-matrix.sh"
REAL_CI_YML = REPO_ROOT / ".github" / "workflows" / "ci.yml"
REAL_GATE_INPUTS = REPO_ROOT / "contracts" / "gate-inputs.toml"

GATE_RE = re.compile(r"bash scripts/ci-gate\.sh ([A-Za-z0-9]+)")
CMD_RE = re.compile(r'echo "command=(.*)"')
UNITTEST_CMD_RE = re.compile(r"python3\.14 -m unittest scripts\.tests\.\w+")


def ci_yml_derived_set(path: Path) -> set[str]:
    """Independent re-derivation of what `ci.yml` runs: every
    `bash scripts/ci-gate.sh <ID>` gate id (prefixed `GATE:`) and every
    `echo "command=<CMD>"` literal command (prefixed `CMD:`) -- the two
    markers the dispatch's own measurement (its §2) found cover everything
    ci.yml runs that `contracts/gate-inputs.toml`'s `[gates]` table alone
    does not."""
    text = path.read_text(encoding="utf-8")
    out = {f"GATE:{m}" for m in GATE_RE.findall(text)}
    out |= {f"CMD:{m}" for m in CMD_RE.findall(text)}
    return out


def run_list(root: Path) -> set[str]:
    result = subprocess.run(
        ["bash", str(SCRIPT), "--root", str(root), "--list"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    out = set()
    for line in result.stdout.splitlines():
        kind, _, value = line.partition("\t")
        out.add(f"{kind}:{value}")
    return out


def _write_gate_inputs(root: Path, extra_gates: str = "") -> None:
    """A minimal, valid [gates]/[lane_profiles] table -- real gate-inputs.toml
    content is not needed for a ci.yml-only fixture; only a parseable file."""
    (root / "contracts").mkdir(parents=True, exist_ok=True)
    (root / "contracts" / "gate-inputs.toml").write_text(
        "[gates]\n" + extra_gates + "\n[lane_profiles]\n",
        encoding="utf-8",
    )


def _write_ci_yml(root: Path, body: str) -> None:
    dest = root / ".github" / "workflows" / "ci.yml"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(body, encoding="utf-8")


class LocalMatrixDerivation(unittest.TestCase):
    def test_the_scripts_derived_set_matches_the_real_ci_yml(self):
        self.assertEqual(run_list(REPO_ROOT), ci_yml_derived_set(REAL_CI_YML))

    def test_a_lane_missing_from_ci_yml_is_absent_from_the_derived_set(self):
        # Modelled on the historical defect itself (dispatch §1): a synthetic
        # ci.yml carrying thirteen of the real fourteen unittest lines,
        # missing exactly the one an actual dispatch's hand count missed.
        # Built from the real file's own lines, not retyped, so this fixture
        # cannot itself drift from what ci.yml actually contains.
        real_text = REAL_CI_YML.read_text(encoding="utf-8")
        unittest_lines = sorted(set(UNITTEST_CMD_RE.findall(real_text)))
        self.assertIn(
            "python3.14 -m unittest scripts.tests.test_markdown_links",
            unittest_lines,
            "the real ci.yml no longer names test_markdown_links -- update this fixture",
        )
        omitted = "python3.14 -m unittest scripts.tests.test_markdown_links"
        kept = [line for line in unittest_lines if line != omitted]

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            body = "".join(f'      - name: dummy\n        run: echo "command={cmd}"\n' for cmd in kept)
            _write_ci_yml(root, body)
            derived = run_list(root)

        self.assertNotIn(f"CMD:{omitted}", derived, "a lane absent from ci.yml must not appear in the derived set")
        for cmd in kept:
            self.assertIn(f"CMD:{cmd}", derived)

    def test_a_newly_added_lane_is_picked_up_without_editing_the_script(self):
        # The flip side, and the property this whole round exists for: a
        # lane genuinely present in ci.yml must appear in the derived set,
        # proving the script re-derives from the file rather than from a
        # cached or hand-written list.
        new_lane = "python3.14 -m unittest scripts.tests.test_a_brand_new_lane_not_known_today"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            _write_ci_yml(root, f'      - name: dummy\n        run: echo "command={new_lane}"\n')
            derived = run_list(root)
        self.assertIn(f"CMD:{new_lane}", derived)

    def test_a_gate_id_not_yet_declared_anywhere_is_still_derived_by_name(self):
        # Gate-id derivation reads only ci.yml's own `bash scripts/ci-gate.sh
        # <ID>` lines, never a fixed G01..G21 range, so a 25th gate (G99, not
        # a real id) is still picked up by name.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            _write_ci_yml(root, "      - name: dummy\n        run: bash scripts/ci-gate.sh G99\n")
            derived = run_list(root)
        self.assertIn("GATE:G99", derived)


class LocalMatrixExecution(unittest.TestCase):
    """End-to-end: the script actually runs what it derives, applies the
    [gates] command and [lane_profiles] tz for a gate id, and respects the
    inverted-polarity idiom (`-eq 0` present in a command's own step)."""

    def _run(self, root: Path) -> subprocess.CompletedProcess:
        return subprocess.run(
            ["bash", str(SCRIPT), "--root", str(root)],
            capture_output=True,
            text=True,
        )

    def test_gate_command_and_tz_are_read_from_gate_inputs_and_applied(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "contracts").mkdir()
            (root / "contracts" / "gate-inputs.toml").write_text(
                '[gates]\nZZZ = "echo \\"TZ=$TZ\\""\n\n'
                '[lane_profiles]\nZZZ = { tz = "Pacific/Kiritimati" }\n',
                encoding="utf-8",
            )
            _write_ci_yml(root, "      - name: dummy\n        run: bash scripts/ci-gate.sh ZZZ\n")
            result = self._run(root)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("TZ=Pacific/Kiritimati", result.stdout)

    def test_pass_fail_counting_and_polarity_inversion(self):
        # Four checks: a normal pass (`true`), a normal fail (`false`), an
        # inverted pass (a command that fails, under a step whose body
        # contains the `-eq 0` idiom), and an inverted fail (a command that
        # succeeds under that same idiom) -- one of each of the four
        # (polarity x outcome) combinations the script must tell apart.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            body = (
                '      - name: normal pass\n        run: echo "command=true"\n'
                '      - name: normal fail\n        run: echo "command=false"\n'
                '      - name: inverted pass\n'
                '        run: |\n'
                '          echo "command=false"\n'
                '          if [[ "$status" -eq 0 ]]; then exit 1; fi\n'
                '      - name: inverted fail\n'
                '        run: |\n'
                '          echo "command=true"\n'
                '          if [[ "$status" -eq 0 ]]; then exit 1; fi\n'
            )
            _write_ci_yml(root, body)
            result = self._run(root)

        self.assertEqual(result.returncode, 1, "one normal and one inverted check must fail overall")
        self.assertIn("local-matrix: 4 check(s) run, 2 passed, 2 failed.", result.stdout)
        self.assertIn("  - false", result.stdout.splitlines(), "the normal `false` check must be listed as failed")
        self.assertIn("  - true", result.stdout.splitlines(), "the inverted `true` check must be listed as failed")

    def test_mutation_an_unconditional_pass_rule_would_hide_the_normal_failure(self):
        # Mutation evidence for the counting logic itself: if `run_one`
        # treated every exit status as a pass (the bug a matrix whose value
        # is completeness must not have), this fixture's "normal fail" check
        # would be missing from the failure list. Asserted here on the
        # actual, unmutated script, which must show it.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            _write_ci_yml(root, '      - name: normal fail\n        run: echo "command=false"\n')
            result = self._run(root)
        self.assertEqual(result.returncode, 1)
        self.assertIn("result=FAIL", result.stdout)
        self.assertIn("local-matrix: 1 check(s) run, 0 passed, 1 failed.", result.stdout)

    def test_the_not_level_b_banner_is_in_the_output_not_only_the_docs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _write_gate_inputs(root)
            _write_ci_yml(root, '      - name: x\n        run: echo "command=true"\n')
            result = self._run(root)
        self.assertIn("NOT Level B", result.stdout)
        self.assertIn("+stable", result.stdout)


if __name__ == "__main__":
    unittest.main()
