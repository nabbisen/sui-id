"""RFC 131 D4 (widened) negative self-tests for scripts/check-verification-commands.py.

Run as: python3.14 -m unittest scripts.tests.test_verification_commands
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "check-verification-commands.py"

POLICY = """\
[gates]
G07 = "cargo +stable clippy --workspace --all-targets --all-features --locked -- -D warnings"
G08 = "cargo +stable fmt --all -- --check"
G02 = "cargo +1.95 test --workspace --locked"
"""


class CheckVerificationCommands(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        (self.root / "contracts").mkdir()
        (self.root / "contracts" / "gate-inputs.toml").write_text(POLICY, encoding="utf-8")

    def write(self, rel: str, text: str) -> None:
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def run_checker(self) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root), "--policy", "contracts/gate-inputs.toml"],
            capture_output=True,
            text=True,
        )

    # ── the real, historical violations this exists to catch ──────────────

    def test_contributings_three_restated_commands_are_all_caught(self) -> None:
        # The exact pre-fix text, reproduced: this is the evidence requirement
        # ("check it would have failed on the pre-change document").
        self.write(
            ".github/CONTRIBUTING.md",
            "## Code style\n\n"
            "- `cargo fmt` before pushing.\n"
            "- `cargo clippy --workspace --all-targets` should be clean.\n"
            "- `cargo test --workspace` should pass.\n",
        )
        r = self.run_checker()
        self.assertEqual(r.returncode, 1)
        self.assertIn(".github/CONTRIBUTING.md:3:", r.stderr)
        self.assertIn(".github/CONTRIBUTING.md:4:", r.stderr)
        self.assertIn(".github/CONTRIBUTING.md:5:", r.stderr)

    def test_release_process_checklist_is_caught(self) -> None:
        self.write(
            "docs/src/contributing/release-process.md",
            "1. `cargo fmt --all -- --check` is clean.\n"
            "2. `cargo clippy --workspace --all-targets -- -D warnings` is clean.\n"
            "3. `cargo test --workspace` is green.\n",
        )
        r = self.run_checker()
        self.assertEqual(r.returncode, 1)
        self.assertEqual(r.stderr.count("release-process.md:"), 3)

    def test_local_dev_clippy_and_fmt_are_caught(self) -> None:
        self.write(
            "docs/src/contributing/local-dev.md",
            "```bash\ncargo clippy --workspace -- -D warnings\ncargo fmt --check\n```\n",
        )
        r = self.run_checker()
        self.assertEqual(r.returncode, 1)
        self.assertEqual(r.stderr.count("local-dev.md:"), 2)

    # ── what must stay exempt ───────────────────────────────────────────

    def test_a_package_scoped_test_command_is_exempt(self) -> None:
        self.write(
            "docs/src/contributing/local-dev.md",
            "```bash\ncargo test -p sui-id-core --lib password\n```\n",
        )
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_a_lib_scoped_workspace_test_command_is_exempt(self) -> None:
        self.write(
            "docs/src/contributing/local-dev.md",
            "```bash\ncargo test --workspace --lib\n```\n",
        )
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_a_specific_test_binary_is_exempt(self) -> None:
        self.write(
            "docs/src/contributing/local-dev.md",
            "```bash\nCARGO_BUILD_JOBS=1 cargo test --test e2e\n```\n",
        )
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_a_bare_cargo_build_is_exempt(self) -> None:
        # No --workspace claim, and build is not special-cased like fmt.
        self.write("docs/src/contributing/local-dev.md", "```bash\ncargo build\n```\n")
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_an_env_prefixed_workspace_test_command_is_exempt(self) -> None:
        # A parameterised example for a specific purpose (RFC 132 D6's wide
        # proptest run), not a claim about the verification bar.
        self.write(
            ".github/CONTRIBUTING.md",
            "```bash\nPROPTEST_CASES=4096 cargo test --workspace\n```\n",
        )
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_a_historical_audit_record_outside_docs_src_is_not_scanned(self) -> None:
        # docs/ top-level (not docs/src/) is RFC 098's dated-record category
        # (D2), not live contributor documentation (D1).
        self.write(
            "docs/security-assurance-audit-v0.99.0.md",
            "Not fixed this release: `cargo fmt --check` reports mechanical diffs.\n",
        )
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_a_run_dev_command_is_not_a_gate_subcommand(self) -> None:
        self.write("docs/src/contributing/local-dev.md", "```bash\ncargo run -- --dev\n```\n")
        r = self.run_checker()
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_the_real_repository_passes(self) -> None:
        r = subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(REPO_ROOT), "--policy", "contracts/gate-inputs.toml"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    def test_an_unreadable_policy_is_exit_two(self) -> None:
        (self.root / "contracts" / "gate-inputs.toml").unlink()
        r = self.run_checker()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)


if __name__ == "__main__":
    unittest.main()
