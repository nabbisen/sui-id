"""RFC 130 D6/D7 negative self-tests for scripts/compute-changed-scope.py.

Run as: python3.14 -m unittest scripts.tests.test_compute_changed_scope

A throwaway git repository, because the script's own correctness depends on
real `git diff`/`git cat-file` behaviour against real commits -- a fixture
that merely asserts what the script would print, without a real repository
underneath it, would not catch a mistake in how it talks to git.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "compute-changed-scope.py"

MANIFEST = """\
version = 1
gate_matrix_version = 1

[lane_profiles]
G01 = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G02 = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G03 = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G04 = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G05 = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G06 = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G07 = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G07b = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G08 = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G09a = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
G09b = { paths = ["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"] }
"""


def git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(root), *args], capture_output=True, text=True, check=True
    )


class ComputeChangedScope(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        git(self.root, "init", "-q", "-b", "main")
        git(self.root, "-c", "user.email=t@example.invalid", "-c", "user.name=t", "config", "commit.gpgsign", "false")
        (self.root / "contracts").mkdir()
        (self.root / "contracts" / "gate-inputs.toml").write_text(MANIFEST, encoding="utf-8")
        (self.root / "README.md").write_text("hello\n", encoding="utf-8")
        git(self.root, "add", "-A")
        git(self.root, "-c", "user.email=t@example.invalid", "-c", "user.name=t", "commit", "-q", "-m", "base")
        self.base = git(self.root, "rev-parse", "HEAD").stdout.strip()

    def write_commit(self, rel: str, content: str) -> str:
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        git(self.root, "add", "-A")
        git(self.root, "-c", "user.email=t@example.invalid", "-c", "user.name=t", "commit", "-q", "-m", rel)
        return git(self.root, "rev-parse", "HEAD").stdout.strip()

    def run_script(self, **kwargs: str) -> subprocess.CompletedProcess:
        args = [
            sys.executable,
            str(SCRIPT),
            "--root",
            str(self.root),
            "--policy",
            "contracts/gate-inputs.toml",
        ]
        for k, v in kwargs.items():
            args += [f"--{k.replace('_', '-')}", v]
        return subprocess.run(args, capture_output=True, text=True)

    # ── the honest cases ──────────────────────────────────────────────────

    def test_a_docs_only_push_is_false(self) -> None:
        after = self.write_commit("docs/guide.md", "hello\n")
        r = self.run_script(event_name="push", before=self.base, after=after)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.strip(), "false")

    def test_a_push_touching_crates_is_true(self) -> None:
        after = self.write_commit("crates/sui-id-core/src/lib.rs", "// hi\n")
        r = self.run_script(event_name="push", before=self.base, after=after)
        self.assertEqual(r.stdout.strip(), "true")

    def test_a_push_touching_cargo_lock_is_true(self) -> None:
        after = self.write_commit("Cargo.lock", "# lock\n")
        r = self.run_script(event_name="push", before=self.base, after=after)
        self.assertEqual(r.stdout.strip(), "true")

    def test_a_push_touching_the_dispatcher_is_true(self) -> None:
        after = self.write_commit("scripts/ci-gate.sh", "#!/bin/bash\n")
        r = self.run_script(event_name="push", before=self.base, after=after)
        self.assertEqual(r.stdout.strip(), "true")

    def test_a_push_touching_the_generated_workflow_is_true(self) -> None:
        after = self.write_commit(".github/workflows/ci.yml", "name: CI\n")
        r = self.run_script(event_name="push", before=self.base, after=after)
        self.assertEqual(r.stdout.strip(), "true")

    def test_a_mixed_push_is_true(self) -> None:
        self.write_commit("docs/a.md", "a\n")
        after = self.write_commit("crates/x/src/lib.rs", "x\n")
        r = self.run_script(event_name="push", before=self.base, after=after)
        self.assertEqual(r.stdout.strip(), "true")

    # ── D7: fail open ─────────────────────────────────────────────────────

    def test_workflow_dispatch_fails_open(self) -> None:
        after = self.write_commit("docs/a.md", "a\n")
        r = self.run_script(event_name="workflow_dispatch", before=self.base, after=after)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.strip(), "true")
        self.assertIn("non-push event", r.stderr)

    def test_pull_request_fails_open(self) -> None:
        after = self.write_commit("docs/a.md", "a\n")
        r = self.run_script(event_name="pull_request", before=self.base, after=after)
        self.assertEqual(r.stdout.strip(), "true")

    def test_empty_before_fails_open(self) -> None:
        after = self.write_commit("docs/a.md", "a\n")
        r = self.run_script(event_name="push", before="", after=after)
        self.assertEqual(r.stdout.strip(), "true")
        self.assertIn("absent or all-zero base ref", r.stderr)

    def test_all_zero_before_fails_open(self) -> None:
        after = self.write_commit("docs/a.md", "a\n")
        r = self.run_script(event_name="push", before="0" * 40, after=after)
        self.assertEqual(r.stdout.strip(), "true")
        self.assertIn("absent or all-zero base ref", r.stderr)

    def test_an_unresolvable_before_fails_open(self) -> None:
        after = self.write_commit("docs/a.md", "a\n")
        r = self.run_script(event_name="push", before="f" * 40, after=after)
        self.assertEqual(r.stdout.strip(), "true")
        self.assertIn("not present in the fetched history", r.stderr)

    def test_a_missing_manifest_fails_open(self) -> None:
        (self.root / "contracts" / "gate-inputs.toml").unlink()
        after = self.write_commit("docs/a.md", "a\n")
        r = self.run_script(event_name="push", before=self.base, after=after)
        self.assertEqual(r.stdout.strip(), "true")
        self.assertIn("could not read the declared Rust scope", r.stderr)

    def test_scoped_lanes_disagreeing_fails_open(self) -> None:
        bad = MANIFEST.replace('G02 = { paths = ["crates/**"', 'G02 = { paths = ["docs/**"')
        (self.root / "contracts" / "gate-inputs.toml").write_text(bad, encoding="utf-8")
        after = self.write_commit("docs/a.md", "a\n")
        r = self.run_script(event_name="push", before=self.base, after=after)
        self.assertEqual(r.stdout.strip(), "true")
        self.assertIn("declare different", r.stderr)


if __name__ == "__main__":
    unittest.main()
