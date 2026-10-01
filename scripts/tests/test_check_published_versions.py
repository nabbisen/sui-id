"""RFC 131 D7 negative self-tests for scripts/check-published-versions.py.

Run as: python3.14 -m unittest scripts.tests.test_check_published_versions
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
SCRIPT_PATH = REPO_ROOT / "scripts" / "check-published-versions.py"

spec = importlib.util.spec_from_file_location("check_published_versions", SCRIPT_PATH)
cpv = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cpv)  # type: ignore[union-attr]


def git(root: Path, *args: str) -> None:
    subprocess.run(
        ["git", "-C", str(root), *args], check=True, capture_output=True, text=True
    )


def init_repo(root: Path) -> None:
    git(root, "init", "-q", "-b", "main")
    git(root, "config", "commit.gpgsign", "false")
    git(root, "config", "user.email", "t@example.invalid")
    git(root, "config", "user.name", "t")


class SemverAndAbandoned(unittest.TestCase):
    def test_semver_key_orders_numerically_not_lexicographically(self) -> None:
        versions = ["0.76.9", "0.76.10", "0.76.2", "0.9.0"]
        self.assertEqual(
            sorted(versions, key=cpv.semver_key), ["0.9.0", "0.76.2", "0.76.9", "0.76.10"]
        )

    def test_section_abandoned_pattern_is_found(self) -> None:
        text = (
            "## [0.78.0] — 2026-09-24\n\n"
            "> **Never published to crates.io; abandoned.** Tagged 2026-09-24.\n\n"
            "Some content.\n"
        )
        with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
            f.write(text)
            path = Path(f.name)
        self.addCleanup(path.unlink)
        self.assertEqual(cpv.abandoned_versions(path), {"0.78.0"})

    def test_list_abandoned_pattern_is_found(self) -> None:
        text = (
            "## [0.78.0] — 2026-09-24\n\n"
            "> 0.76.10, 0.76.11 and 0.76.12 are abandoned on the same ruling.\n"
        )
        with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
            f.write(text)
            path = Path(f.name)
        self.addCleanup(path.unlink)
        self.assertEqual(
            cpv.abandoned_versions(path), {"0.76.10", "0.76.11", "0.76.12"}
        )

    def test_a_version_merely_named_as_where_content_shipped_is_not_abandoned(self) -> None:
        # The exact trap: 0.78.0's own abandonment note names 0.79.0 as where
        # its content actually shipped. 0.79.0 must not be swept in.
        text = (
            "## [0.78.0] — 2026-09-24\n\n"
            "> **Never published to crates.io; abandoned.** The registry went\n"
            "> from 0.77.0 to 0.79.0, so the changes reached users in 0.79.0\n"
            "> rather than in a release of their own.\n"
        )
        with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
            f.write(text)
            path = Path(f.name)
        self.addCleanup(path.unlink)
        found = cpv.abandoned_versions(path)
        self.assertIn("0.78.0", found)
        self.assertNotIn("0.79.0", found)
        self.assertNotIn("0.77.0", found)

    def test_the_real_changelog_marks_exactly_the_four_known_gaps(self) -> None:
        found = cpv.abandoned_versions(REPO_ROOT / "CHANGELOG.md")
        self.assertEqual(found, {"0.78.0", "0.76.10", "0.76.11", "0.76.12"})


class NewestTag(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        init_repo(self.root)

    def commit(self, tag: str | None = None) -> None:
        (self.root / "f.txt").write_text(tag or "x", encoding="utf-8")
        git(self.root, "add", "-A")
        git(self.root, "commit", "-q", "-m", tag or "c")
        if tag:
            git(self.root, "tag", "-m", "t", tag)

    def test_picks_the_semver_newest_not_the_most_recent_commit(self) -> None:
        self.commit("0.9.0")
        self.commit("0.76.9")
        self.commit("0.76.10")
        self.assertEqual(cpv.newest_tag(self.root), "0.76.10")

    def test_no_tags_raises(self) -> None:
        self.commit()
        with self.assertRaises(RuntimeError):
            cpv.newest_tag(self.root)


class MainIntegration(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        init_repo(self.root)
        (self.root / "f.txt").write_text("x", encoding="utf-8")
        git(self.root, "add", "-A")
        git(self.root, "commit", "-q", "-m", "c")
        git(self.root, "tag", "-m", "t", "1.2.0")

    def write_changelog(self, text: str) -> None:
        (self.root / "CHANGELOG.md").write_text(text, encoding="utf-8")

    def test_every_crate_published_is_exit_zero(self) -> None:
        self.write_changelog("# Changelog\n")
        with mock.patch.object(cpv, "registry_max_version", return_value="1.2.0"):
            rc = cpv.main(["--root", str(self.root)])
        self.assertEqual(rc, 0)

    def test_one_crate_behind_is_exit_one(self) -> None:
        self.write_changelog("# Changelog\n")

        def fake(crate: str, timeout: float) -> str:
            return "1.1.0" if crate == "sui-id-core" else "1.2.0"

        with mock.patch.object(cpv, "registry_max_version", side_effect=fake):
            rc = cpv.main(["--root", str(self.root)])
        self.assertEqual(rc, 1)

    def test_an_abandoned_tag_is_exit_zero_without_querying_the_registry(self) -> None:
        self.write_changelog(
            "## [1.2.0] — 2026-01-01\n\n> **Never published to crates.io; abandoned.**\n"
        )
        with mock.patch.object(cpv, "registry_max_version") as fake:
            rc = cpv.main(["--root", str(self.root)])
        fake.assert_not_called()
        self.assertEqual(rc, 0)

    def test_a_missing_changelog_is_exit_two(self) -> None:
        rc = cpv.main(["--root", str(self.root)])
        self.assertEqual(rc, 2)

    def test_a_registry_error_is_exit_two(self) -> None:
        import urllib.error

        self.write_changelog("# Changelog\n")
        with mock.patch.object(
            cpv, "registry_max_version", side_effect=urllib.error.URLError("boom")
        ):
            rc = cpv.main(["--root", str(self.root)])
        self.assertEqual(rc, 2)


if __name__ == "__main__":
    unittest.main()
