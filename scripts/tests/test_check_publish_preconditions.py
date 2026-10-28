"""RFC 141 §3 negative self-tests for scripts/check-publish-preconditions.py.

Run as: python3.14 -m unittest scripts.tests.test_check_publish_preconditions

Four required fixtures, one per RFC 141 §3 bullet -- tag identity, the six
Cargo.toml version fields, the CI run, the Security audit run -- each
proving the refusal its own bullet exists for. A fifth group of extra
fixtures pins the CI job-counting rigor specifically (RFC 141 dispatch §2:
"a job still running has conclusion null, which is not skipped"), since
that is the one carried mistake this round exists to not repeat.

Tag identity and the Cargo.toml read are tested against a real throwaway
git repository (this project's own convention for anything whose
correctness depends on real git behaviour -- see
test_compute_changed_scope.py's own reasoning). The CI and Security audit
checks instead `unittest.mock.patch.object` the four network-facing
functions (`ci_run_for_sha`, `jobs_for_run`, `audit_run_for_sha`), the same
shape `test_check_published_versions.py` already uses for
`registry_max_version` -- the one other live-network call among this
project's own checkers.
"""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT_PATH = REPO_ROOT / "scripts" / "check-publish-preconditions.py"

spec = importlib.util.spec_from_file_location("check_publish_preconditions", SCRIPT_PATH)
cpp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cpp)  # type: ignore[union-attr]


def git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(root), *args], capture_output=True, text=True
    )


def git_ok(root: Path, *args: str) -> str:
    result = git(root, *args)
    assert result.returncode == 0, result.stderr
    return result.stdout


def init_repo(root: Path) -> None:
    git_ok(root, "init", "-q", "-b", "main")
    # Same rationale as test_check_published_versions.py's own init_repo:
    # async gc/maintenance writes race TemporaryDirectory's teardown.
    git_ok(root, "config", "gc.auto", "0")
    git_ok(root, "config", "maintenance.auto", "false")
    git_ok(root, "config", "commit.gpgsign", "false")
    git_ok(root, "config", "user.email", "t@example.invalid")
    git_ok(root, "config", "user.name", "t")


CARGO_TOML_GOOD = """\
[workspace]
members = []

[workspace.package]
version = "1.2.3"

[workspace.dependencies]
sui-id-core = { version = "1.2.3", path = "crates/sui-id-core" }
sui-id-i18n = { version = "1.2.3", path = "crates/sui-id-i18n" }
sui-id-shared = { version = "1.2.3", path = "crates/sui-id-shared" }
sui-id-store = { version = "1.2.3", path = "crates/sui-id-store" }
sui-id-web = { version = "1.2.3", path = "crates/sui-id-web" }
"""


def commit_cargo_toml(root: Path, content: str = CARGO_TOML_GOOD) -> str:
    (root / "Cargo.toml").write_text(content, encoding="utf-8")
    git_ok(root, "add", "Cargo.toml")
    git_ok(root, "commit", "-q", "-m", "release commit")
    return git_ok(root, "rev-parse", "HEAD").strip()


class TagIdentity(unittest.TestCase):
    """RFC 141 §3 bullet 3: the tag is annotated and points at a commit on
    main. Required fixture 1 of 4."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        init_repo(self.root)
        self.sha = commit_cargo_toml(self.root)

    def test_a_lightweight_tag_is_refused(self) -> None:
        # `git tag <name>` (no -a/-m) is the ordinary way to make a
        # lightweight tag, but this environment's own config makes a
        # bare `git tag` demand a message regardless -- `update-ref`
        # writes the same ref directly (a tag pointing straight at the
        # commit object, which is what "lightweight" means) without
        # going through that config path.
        git_ok(self.root, "update-ref", "refs/tags/1.2.3", self.sha)
        errors, sha = cpp.check_tag_identity(self.root, "1.2.3", "main")
        self.assertIsNone(sha, "a non-annotated tag must not resolve to a commit for the later checks")
        self.assertTrue(
            any("not annotated" in e for e in errors),
            f"expected an 'is not annotated' refusal, got: {errors}",
        )

    def test_an_annotated_tag_not_on_main_is_refused(self) -> None:
        git_ok(self.root, "checkout", "-q", "-b", "side")
        (self.root / "extra.txt").write_text("x", encoding="utf-8")
        git_ok(self.root, "add", "extra.txt")
        git_ok(self.root, "commit", "-q", "-m", "a commit main does not have")
        git_ok(self.root, "tag", "-a", "1.2.3", "-m", "t")
        errors, sha = cpp.check_tag_identity(self.root, "1.2.3", "main")
        self.assertIsNotNone(sha, "an ancestry failure still returns the resolved sha")
        self.assertTrue(
            any("is not an ancestor" in e for e in errors),
            f"expected an 'is not an ancestor' refusal, got: {errors}",
        )

    def test_an_annotated_tag_on_main_is_accepted(self) -> None:
        git_ok(self.root, "tag", "-a", "1.2.3", "-m", "t")
        errors, sha = cpp.check_tag_identity(self.root, "1.2.3", "main")
        self.assertEqual(errors, [])
        self.assertEqual(sha, self.sha)


class CargoVersions(unittest.TestCase):
    """RFC 141 §3 bullet 4: the six Cargo.toml version fields equal the
    tag. Required fixture 2 of 4."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        init_repo(self.root)

    def test_a_mismatched_dependency_version_is_refused(self) -> None:
        bad = CARGO_TOML_GOOD.replace(
            'sui-id-store = { version = "1.2.3"',
            'sui-id-store = { version = "1.2.2"',
        )
        sha = commit_cargo_toml(self.root, bad)
        errors = cpp.check_cargo_versions(self.root, sha, "1.2.3")
        self.assertTrue(
            any("sui-id-store" in e and "1.2.2" in e for e in errors),
            f"expected a sui-id-store version mismatch, got: {errors}",
        )
        # The other four dependency entries and [workspace.package] were
        # correct and must not be reported -- this is the one mismatch,
        # not every entry name appearing by coincidence.
        self.assertEqual(
            len(errors), 1, f"exactly one field should mismatch, got: {errors}"
        )

    def test_a_mismatched_workspace_package_version_is_refused(self) -> None:
        bad = CARGO_TOML_GOOD.replace('version = "1.2.3"', 'version = "1.2.4"', 1)
        sha = commit_cargo_toml(self.root, bad)
        errors = cpp.check_cargo_versions(self.root, sha, "1.2.3")
        self.assertTrue(
            any("[workspace.package]" in e for e in errors),
            f"expected a [workspace.package] version mismatch, got: {errors}",
        )

    def test_the_version_is_read_from_the_tagged_commit_not_the_working_tree(self) -> None:
        # The working tree can differ from the commit the sha names (a
        # later, uncommitted edit, or a checkout of something else
        # entirely) -- the check must read via `git show <sha>:Cargo.toml`,
        # never the filesystem, exactly the staleness class of bug this
        # round's own §1 fixed for `origins_before`.
        sha = commit_cargo_toml(self.root)
        (self.root / "Cargo.toml").write_text(
            CARGO_TOML_GOOD.replace('version = "1.2.3"', 'version = "9.9.9"'),
            encoding="utf-8",
        )
        errors = cpp.check_cargo_versions(self.root, sha, "1.2.3")
        self.assertEqual(
            errors, [], f"must read the committed content at {sha}, not the dirty working tree: {errors}"
        )

    def test_all_six_fields_matching_is_accepted(self) -> None:
        sha = commit_cargo_toml(self.root)
        errors = cpp.check_cargo_versions(self.root, sha, "1.2.3")
        self.assertEqual(errors, [])


class CiCheck(unittest.TestCase):
    """RFC 141 §3 bullet 1: a CI workflow_dispatch run on the exact commit
    concluded success, with 0 skipped jobs. Required fixture 3 of 4, plus
    extra fixtures pinning the job-counting rigor the dispatch's §2 names
    directly (a still-running job's conclusion is null, which is neither
    success, failure, nor skipped)."""

    def test_no_matching_run_is_refused(self) -> None:
        with mock.patch.object(cpp, "ci_run_for_sha", return_value=None):
            errors = cpp.check_ci("o/r", "deadbeef", "tok", 1.0)
        self.assertTrue(any("no CI workflow_dispatch run found" in e for e in errors))

    def test_a_failed_run_is_refused(self) -> None:
        with mock.patch.object(
            cpp, "ci_run_for_sha", return_value={"id": 1, "conclusion": "failure"}
        ):
            errors = cpp.check_ci("o/r", "deadbeef", "tok", 1.0)
        self.assertTrue(any("concluded 'failure'" in e for e in errors))

    def test_a_still_running_job_is_refused_as_not_finished_not_skipped(self) -> None:
        # The dispatch's own carried mistake: `conclusion: null` must not
        # be folded into "skipped" (nor into "success" by omission).
        run = {"id": 1, "conclusion": "success"}
        jobs = [{"conclusion": "success"}, {"conclusion": None}]
        with mock.patch.object(cpp, "ci_run_for_sha", return_value=run), mock.patch.object(
            cpp, "jobs_for_run", return_value=jobs
        ):
            errors = cpp.check_ci("o/r", "deadbeef", "tok", 1.0)
        # Exactly one error: the single null-conclusion job is "not
        # finished", and nothing else is wrong (0 skipped, 0 failed) --
        # so no skipped-count or failure-count message is generated at
        # all, which is the real way to prove a still-running job was
        # not folded into "skipped" (its own message would say
        # "0 job(s) ... skipped" only if the count leaked, and it would
        # not be generated here if the counting is right).
        self.assertEqual(
            len(errors), 1, f"expected exactly one 'not finished' refusal, got: {errors}"
        )
        self.assertIn("not finished", errors[0])

    def test_a_skipped_job_is_refused_distinctly_from_not_finished(self) -> None:
        run = {"id": 1, "conclusion": "success"}
        jobs = [{"conclusion": "success"}, {"conclusion": "skipped"}]
        with mock.patch.object(cpp, "ci_run_for_sha", return_value=run), mock.patch.object(
            cpp, "jobs_for_run", return_value=jobs
        ):
            errors = cpp.check_ci("o/r", "deadbeef", "tok", 1.0)
        self.assertTrue(any("1 job(s)" in e and "skipped" in e for e in errors))
        self.assertFalse(any("not finished" in e for e in errors))

    def test_a_failed_job_is_counted_and_refused(self) -> None:
        run = {"id": 1, "conclusion": "success"}
        jobs = [{"conclusion": "success"}, {"conclusion": "failure"}]
        with mock.patch.object(cpp, "ci_run_for_sha", return_value=run), mock.patch.object(
            cpp, "jobs_for_run", return_value=jobs
        ):
            errors = cpp.check_ci("o/r", "deadbeef", "tok", 1.0)
        self.assertTrue(any("1 job(s)" in e and "failed" in e for e in errors))

    def test_all_jobs_success_and_none_skipped_is_accepted(self) -> None:
        run = {"id": 1, "conclusion": "success"}
        jobs = [{"conclusion": "success"} for _ in range(27)]
        with mock.patch.object(cpp, "ci_run_for_sha", return_value=run), mock.patch.object(
            cpp, "jobs_for_run", return_value=jobs
        ):
            errors = cpp.check_ci("o/r", "deadbeef", "tok", 1.0)
        self.assertEqual(errors, [])


class AuditCheck(unittest.TestCase):
    """RFC 141 §3 bullet 2: a Security audit run on the same commit
    concluded success. Required fixture 4 of 4."""

    def test_no_matching_run_is_refused(self) -> None:
        with mock.patch.object(cpp, "audit_run_for_sha", return_value=None):
            errors = cpp.check_audit("o/r", "deadbeef", "tok", 1.0)
        self.assertTrue(any("no Security audit run found" in e for e in errors))

    def test_a_failed_run_is_refused(self) -> None:
        with mock.patch.object(
            cpp, "audit_run_for_sha", return_value={"id": 2, "conclusion": "failure"}
        ):
            errors = cpp.check_audit("o/r", "deadbeef", "tok", 1.0)
        self.assertTrue(any("concluded 'failure'" in e for e in errors))

    def test_a_successful_run_is_accepted(self) -> None:
        with mock.patch.object(
            cpp, "audit_run_for_sha", return_value={"id": 2, "conclusion": "success"}
        ):
            errors = cpp.check_audit("o/r", "deadbeef", "tok", 1.0)
        self.assertEqual(errors, [])


class MainIntegration(unittest.TestCase):
    """`main`'s own exit-code and exit-1-vs-exit-2 distinction, over a
    real repo, with the two network checks mocked."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        init_repo(self.root)
        self.sha = commit_cargo_toml(self.root)
        git_ok(self.root, "tag", "-a", "1.2.3", "-m", "t")

    def test_all_four_satisfied_exits_zero(self) -> None:
        with mock.patch.object(
            cpp, "ci_run_for_sha", return_value={"id": 1, "conclusion": "success"}
        ), mock.patch.object(cpp, "jobs_for_run", return_value=[{"conclusion": "success"}]), mock.patch.object(
            cpp, "audit_run_for_sha", return_value={"id": 2, "conclusion": "success"}
        ):
            code = cpp.main(
                [
                    "--root", str(self.root),
                    "--repo", "o/r",
                    "--tag", "1.2.3",
                    "--token", "tok",
                    "--main-ref", "main",
                ]
            )
        self.assertEqual(code, 0)

    def test_a_refusal_exits_one_not_two(self) -> None:
        with mock.patch.object(
            cpp, "ci_run_for_sha", return_value={"id": 1, "conclusion": "failure"}
        ), mock.patch.object(
            cpp, "audit_run_for_sha", return_value={"id": 2, "conclusion": "success"}
        ):
            code = cpp.main(
                [
                    "--root", str(self.root),
                    "--repo", "o/r",
                    "--tag", "1.2.3",
                    "--token", "tok",
                    "--main-ref", "main",
                ]
            )
        self.assertEqual(code, 1, "a refusal (checked, and it says no) must be exit 1, not exit 2")

    def test_an_api_error_exits_two_not_one(self) -> None:
        with mock.patch.object(
            cpp, "ci_run_for_sha", side_effect=cpp.ApiError("boom")
        ), mock.patch.object(
            cpp, "audit_run_for_sha", return_value={"id": 2, "conclusion": "success"}
        ):
            code = cpp.main(
                [
                    "--root", str(self.root),
                    "--repo", "o/r",
                    "--tag", "1.2.3",
                    "--token", "tok",
                    "--main-ref", "main",
                ]
            )
        self.assertEqual(code, 2, "could not verify must be exit 2, distinct from a refusal")


if __name__ == "__main__":
    unittest.main()
