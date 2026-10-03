"""RFC 094 M2a (G20) self-tests for scripts/check-m2a-rollback-coverage.sh.

Run as: python3.14 -m unittest scripts.tests.test_check_m2a_rollback_coverage

The checker is bash, not Python (its own job needs the Rust toolchain to
run the test suite first, and generate-ci-workflow.py provisions exactly
one of {rust, python} per lane -- see the checker's own header). Each
fixture is a throwaway synthetic tree mirroring the real crate's layout,
mutated one way per test, invoked as a subprocess matching this project's
convention elsewhere. The checker now asserts two independent registries
(rollback, exactly-once); most tests exercise one at a time, holding the
other trivially satisfied via its own exemption override so it cannot
contaminate the result under test.
"""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-m2a-rollback-coverage.sh"

GOOD_COMMANDS_RS = """\
crate::declare_write_command! {
    command A01 = "A01" {
        system_principal: forbidden;
    }
}

crate::declare_write_command! {
    command A02 = "A02" {
        system_principal: forbidden;
    }
}
"""

GOOD_SEAM_SITE = """\
pub async fn do_a01(db: &crate::Database) {
    db.class_a(context, move |tx: &mut ClassATx<'_, A01>| Ok(((), Event)))
}

pub async fn do_a02(db: &crate::Database) {
    db.class_a(context, move |tx: &mut ClassATx<'_, A02>| Ok(((), Event)))
}
"""

# The default exemption for the dimension not under test, so a test
# focused on one registry is not also tripped up by the other one's
# "neither covered nor exempt" check.
ALL_EXEMPT = "A01 A02"


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def make_baseline(root: Path) -> None:
    write(root / "crates/sui-id-store/src/commands.rs", GOOD_COMMANDS_RS)
    write(root / "crates/sui-id-store/src/commands/mutations.rs", GOOD_SEAM_SITE)


_UNSET = object()  # distinguishes "not passed" from "explicitly None"


def run_checker(
    root: Path,
    coverage_ids=_UNSET,
    exempt_override=_UNSET,
    exactly_once_coverage_ids=_UNSET,
    exactly_once_exempt_override=_UNSET,
    write_missing_files: bool = True,
) -> subprocess.CompletedProcess:
    """Each dimension not explicitly exercised defaults to "trivially
    satisfied" (an empty coverage file, every id exempted) so a test
    focused on one registry is not tripped up by the other one's
    "neither covered nor exempt" check. Pass the coverage list
    explicitly (even `[]`) to exercise a dimension for real, in which
    case its default exemption is empty, not `ALL_EXEMPT`."""
    if coverage_ids is _UNSET:
        coverage_ids = [] if write_missing_files else None
        if exempt_override is _UNSET:
            exempt_override = ALL_EXEMPT
    elif exempt_override is _UNSET:
        exempt_override = ""

    if exactly_once_coverage_ids is _UNSET:
        exactly_once_coverage_ids = [] if write_missing_files else None
        if exactly_once_exempt_override is _UNSET:
            exactly_once_exempt_override = ALL_EXEMPT
    elif exactly_once_exempt_override is _UNSET:
        exactly_once_exempt_override = ""

    env = dict(os.environ)

    def set_env(key: str, value: str | None) -> None:
        # Always *set* (never unset): the checker distinguishes an unset
        # override (its own hardcoded default applies) from one
        # explicitly set to empty (no exemptions at all) -- tests must
        # always land in the latter case, never fall through to the
        # real script's real exemption list.
        env[key] = "" if value is None else value

    set_env("M2A_EXEMPT_IDS_OVERRIDE", exempt_override)
    set_env("M2A_EXACTLY_ONCE_EXEMPT_IDS_OVERRIDE", exactly_once_exempt_override)

    def write_coverage(rel: str, ids: list[str] | None) -> None:
        if ids is None:
            return
        f = root / rel
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text("\n".join(ids) + ("\n" if ids else ""))

    write_coverage("target/rfc094-rollback-coverage.txt", coverage_ids)
    write_coverage("target/rfc094-exactly-once-coverage.txt", exactly_once_coverage_ids)

    return subprocess.run(
        ["bash", str(CHECKER), "--root", str(root)],
        capture_output=True,
        text=True,
        env=env,
    )


class M2aRollbackCoverageTest(unittest.TestCase):
    def test_fully_covered_baseline_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root, coverage_ids=["A01", "A02"])
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_an_uncovered_id_with_no_exemption_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root, coverage_ids=["A01"], exempt_override=None)
            self.assertEqual(result.returncode, 1)
            self.assertIn("A02", result.stderr)
            self.assertIn("neither registered coverage nor", result.stderr)
            self.assertIn("(rollback)", result.stderr)

    def test_a_declared_id_with_no_class_a_seam_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root / "crates/sui-id-store/src/commands.rs", GOOD_COMMANDS_RS)
            # No seam file at all: neither A01 nor A02 reaches class_a.
            result = run_checker(root, coverage_ids=["A01", "A02"])
            self.assertEqual(result.returncode, 1)
            self.assertIn("no `ClassATx<'_, ID>` site", result.stderr)
            self.assertIn("A01", result.stderr)
            self.assertIn("A02", result.stderr)

    def test_a_mismatched_command_declaration_is_a_hard_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(
                root / "crates/sui-id-store/src/commands.rs",
                'command A01 = "A99" {\n}\n',
            )
            result = run_checker(root, coverage_ids=["A01"])
            self.assertEqual(result.returncode, 2)
            self.assertIn("command declared as A01 but string literal is", result.stderr)

    def test_missing_coverage_file_is_exit_2_not_a_silent_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            # Deliberately write neither coverage file.
            result = run_checker(root, write_missing_files=False)
            self.assertEqual(result.returncode, 2)
            self.assertIn("does not exist", result.stderr)

    def test_the_repository_as_it_stands_passes(self):
        # The real tree, with the real coverage files this crate's test
        # suite just wrote. Skipped if they're absent -- this test does
        # not run the test suite itself (the gate command does).
        rollback_file = REPO_ROOT / "target" / "rfc094-rollback-coverage.txt"
        exactly_once_file = REPO_ROOT / "target" / "rfc094-exactly-once-coverage.txt"
        if not rollback_file.is_file() or not exactly_once_file.is_file():
            self.skipTest(
                "target/rfc094-*-coverage.txt absent -- run "
                "`cargo test -p sui-id-store --lib` first"
            )
        result = subprocess.run(
            ["bash", str(CHECKER), "--root", str(REPO_ROOT)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    # ── the rollback exemption list, via the test-only environment override ──

    def test_an_exempted_id_with_no_coverage_is_allowed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root, coverage_ids=["A01"], exempt_override="A02")
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_an_exempted_id_that_already_has_coverage_fails(self):
        # The shrink-only rule: an exemption must be deleted the moment
        # its test lands, not left stale.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root, coverage_ids=["A01", "A02"], exempt_override="A02")
            self.assertEqual(result.returncode, 1)
            self.assertIn("A02", result.stderr)
            self.assertIn("must be removed from the exemption list", result.stderr)

    def test_an_exempted_id_not_in_the_registry_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root, coverage_ids=["A01", "A02"], exempt_override="Z99")
            self.assertEqual(result.returncode, 1)
            self.assertIn("not declared", result.stderr)

    # ── the exactly-once registry, independently of the rollback one ──

    def test_exactly_once_fully_covered_baseline_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(
                root,
                exactly_once_coverage_ids=["A01", "A02"],
                exactly_once_exempt_override=None,
            )
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_exactly_once_uncovered_id_with_no_exemption_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(
                root,
                exactly_once_coverage_ids=["A01"],
                exactly_once_exempt_override=None,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("A02", result.stderr)
            self.assertIn("(exactly-once)", result.stderr)

    def test_exactly_once_exempted_id_that_already_has_coverage_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(
                root,
                exactly_once_coverage_ids=["A01", "A02"],
                exactly_once_exempt_override="A02",
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("(exactly-once)", result.stderr)
            self.assertIn("must be removed from the exemption list", result.stderr)

    def test_the_two_registries_are_independent(self):
        # A02 covered on rollback but not exactly-once, and the reverse
        # for A01 -- both dimensions must be satisfied for both ids.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(
                root,
                coverage_ids=["A02"],
                exempt_override="A01",
                exactly_once_coverage_ids=["A01"],
                exactly_once_exempt_override="A02",
            )
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
