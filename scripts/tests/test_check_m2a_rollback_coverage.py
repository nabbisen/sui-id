"""RFC 094 M2a (G20) self-tests for scripts/check-m2a-rollback-coverage.sh.

Run as: python3.14 -m unittest scripts.tests.test_check_m2a_rollback_coverage

The checker is bash, not Python (its own job needs the Rust toolchain to
run the test suite first, and generate-ci-workflow.py provisions exactly
one of {rust, python} per lane -- see the checker's own header). Each
fixture is a throwaway synthetic tree mirroring the real crate's layout,
mutated one way per test, invoked as a subprocess matching this project's
convention elsewhere. The exemption-list cases use the checker's
`M2A_EXEMPT_IDS_OVERRIDE` test-only environment hook.
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


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def make_baseline(root: Path) -> None:
    write(root / "crates/sui-id-store/src/commands.rs", GOOD_COMMANDS_RS)
    write(root / "crates/sui-id-store/src/commands/mutations.rs", GOOD_SEAM_SITE)


def run_checker(
    root: Path, coverage_ids: list[str] | None = None, exempt_override: str | None = None
) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    if exempt_override is not None:
        env["M2A_EXEMPT_IDS_OVERRIDE"] = exempt_override
    else:
        env.pop("M2A_EXEMPT_IDS_OVERRIDE", None)
    if coverage_ids is not None:
        coverage_file = root / "target" / "rfc094-rollback-coverage.txt"
        coverage_file.parent.mkdir(parents=True, exist_ok=True)
        coverage_file.write_text("\n".join(coverage_ids) + ("\n" if coverage_ids else ""))
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
            result = run_checker(root, ["A01", "A02"])
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_an_uncovered_id_with_no_exemption_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root, ["A01"])
            self.assertEqual(result.returncode, 1)
            self.assertIn("A02", result.stderr)
            self.assertIn("neither a registered rollback test nor", result.stderr)

    def test_a_declared_id_with_no_class_a_seam_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root / "crates/sui-id-store/src/commands.rs", GOOD_COMMANDS_RS)
            # No seam file at all: neither A01 nor A02 reaches class_a.
            result = run_checker(root, ["A01", "A02"])
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
            result = run_checker(root, ["A01"])
            self.assertEqual(result.returncode, 2)
            self.assertIn("command declared as A01 but string literal is", result.stderr)

    def test_missing_coverage_file_is_exit_2_not_a_silent_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            # Deliberately do not write a coverage file.
            result = run_checker(root)
            self.assertEqual(result.returncode, 2)
            self.assertIn("does not exist", result.stderr)

    def test_the_repository_as_it_stands_passes(self):
        # The real tree, with the real coverage file this crate's test
        # suite just wrote. Skipped if that file is absent -- this test
        # does not run the test suite itself (the gate command does).
        coverage_file = REPO_ROOT / "target" / "rfc094-rollback-coverage.txt"
        if not coverage_file.is_file():
            self.skipTest(
                "target/rfc094-rollback-coverage.txt absent -- run "
                "`cargo test -p sui-id-store --lib` first"
            )
        result = subprocess.run(
            ["bash", str(CHECKER), "--root", str(REPO_ROOT)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    # ── the exemption list, via the test-only environment override ──

    def test_an_exempted_id_with_no_coverage_is_allowed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root, ["A01"], exempt_override="A02")
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_an_exempted_id_that_already_has_coverage_fails(self):
        # The shrink-only rule: an exemption must be deleted the moment
        # its test lands, not left stale.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root, ["A01", "A02"], exempt_override="A02")
            self.assertEqual(result.returncode, 1)
            self.assertIn("A02", result.stderr)
            self.assertIn("must be removed from the exemption list", result.stderr)

    def test_an_exempted_id_not_in_the_registry_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root, ["A01", "A02"], exempt_override="Z99")
            self.assertEqual(result.returncode, 1)
            self.assertIn("not declared", result.stderr)


if __name__ == "__main__":
    unittest.main()
