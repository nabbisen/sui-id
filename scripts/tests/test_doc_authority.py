"""RFC 098 G15 negative self-tests for scripts/check-doc-authority.py.

Run as: python3.14 -m unittest scripts.tests.test_doc_authority

Each fixture is a throwaway, git-backed synthetic tree (git-backed because
check (B) enumerates its scope with `git ls-files`, so an untracked file
must not be able to fail a gate) carrying a minimal valid baseline,
mutated one way per test. The checker is invoked as a subprocess, matching
this project's convention of testing checkers as black boxes.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-doc-authority.py"

POLICY = """\
version = 1

[freshness]
tolerance_minor = 2
banner_regex = '^> \\*\\*Stale as of'
banner_within_lines = 40
claim_regex = 'current as of \\*\\*?v(\\d+\\.\\d+\\.\\d+)|reflecting the v(\\d+\\.\\d+\\.\\d+) codebase'
documents = ["docs/pinned.md"]
"""

CARGO_TOML = """\
[workspace]
members = ["crates/example"]

[workspace.package]
version = "0.77.0"
"""

SUMMARY = """\
# Summary

[Introduction](./introduction.md)

- [Overview](./getting-started/overview.md)
"""

INTRODUCTION = "# Introduction\n\nBaseline page.\n"
OVERVIEW = "# Overview\n\nBaseline page.\n"

# Within tolerance: 0.77.0 workspace, claim 0.76.0, lag 1 <= 2.
PINNED_FRESH = """\
# Pinned document

It is current as of **v0.76.0** and describes the shipped behaviour.
"""

README = "# Example\n\nSee [the roadmap](ROADMAP.md).\n"
ROADMAP = "# Roadmap\n\nNothing planned.\n"


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def make_baseline(root: Path) -> None:
    write(root / "ci" / "doc-authority.toml", POLICY)
    write(root / "Cargo.toml", CARGO_TOML)
    write(root / "README.md", README)
    write(root / "ROADMAP.md", ROADMAP)
    write(root / "docs" / "src" / "SUMMARY.md", SUMMARY)
    write(root / "docs" / "src" / "introduction.md", INTRODUCTION)
    write(root / "docs" / "src" / "getting-started" / "overview.md", OVERVIEW)
    write(root / "docs" / "pinned.md", PINNED_FRESH)


def git_commit(root: Path) -> None:
    subprocess.run(
        ["git", "-C", str(root), "-c", "init.defaultBranch=main", "init", "-q"],
        check=True,
    )
    subprocess.run(["git", "-C", str(root), "add", "-A"], check=True)
    subprocess.run(
        [
            "git", "-C", str(root),
            "-c", "user.email=fixture@example.invalid", "-c", "user.name=fixture",
            "commit", "-q", "-m", "fixture baseline",
        ],
        check=True,
    )


def run_checker(root: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [
            sys.executable, str(CHECKER),
            "--root", str(root),
            "--policy", "ci/doc-authority.toml",
        ],
        capture_output=True,
        text=True,
    )


class DocAuthorityTest(unittest.TestCase):
    def test_valid_baseline_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    # --- (A) SUMMARY.md completeness, both directions --------------------

    def test_orphan_page_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "docs" / "src" / "guides" / "orphan.md", "# Orphan\n")
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("(A) docs/src/guides/orphan.md", result.stderr)
            self.assertIn("not reachable", result.stderr)

    def test_summary_entry_without_file_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "src" / "SUMMARY.md",
                SUMMARY + "- [Missing](./guides/missing.md)\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("(A) docs/src/SUMMARY.md:", result.stderr)
            self.assertIn("does not exist", result.stderr)

    def test_summary_entry_path_compared_resolved_not_textually(self):
        """`guides/x.md` and `./guides/x.md` name the same page; writing an
        entry the other way must not turn its target into an orphan."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "src" / "SUMMARY.md",
                SUMMARY.replace(
                    "./getting-started/overview.md",
                    "getting-started/../getting-started/overview.md",
                ),
            )
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    # --- (B) no self-referential absolute URL ----------------------------

    def test_absolute_self_url_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "README.md",
                "# Example\n\nSee [the roadmap]"
                "(https://github.com/nabbisen/sui-id/blob/main/ROADMAP.md).\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("(B) README.md:3", result.stderr)
            self.assertIn("absolute link to this repository", result.stderr)

    def test_absolute_self_url_in_code_span_passes(self):
        """RFC 098 quotes the pattern it forbids; a document that describes
        the rule must not fail it."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "README.md",
                "# Example\n\nDo not write "
                "`[x](https://github.com/nabbisen/sui-id/blob/main/ROADMAP.md)` "
                "in a document.\n\nSee [the roadmap](ROADMAP.md).\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_absolute_self_url_in_fenced_block_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "README.md",
                "# Example\n\n```markdown\n"
                "[x](https://github.com/nabbisen/sui-id/blob/main/ROADMAP.md)\n"
                "```\n\nSee [the roadmap](ROADMAP.md).\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_untracked_file_is_not_scanned(self):
        """git ls-files is the scope, so a scratch file dropped in the tree
        cannot turn a gate red."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            git_commit(root)
            write(
                root / "docs" / "scratch.md",
                "[x](https://github.com/nabbisen/sui-id/blob/main/ROADMAP.md)\n",
            )
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    # --- (B) rule 6: the two directions a link may take ------------------

    def test_book_page_absolute_outside_book_passes(self):
        """Rule 6's sanctioned form: no relative spelling works in both
        mdBook and GitHub, so a book page reaches outside by absolute URL."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "src" / "introduction.md",
                "# Introduction\n\nSee [the roadmap]"
                "(https://github.com/nabbisen/sui-id/blob/main/ROADMAP.md).\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_book_page_absolute_to_missing_file_rejected(self):
        """The sanctioned form is not gate-blind: the prefix is stripped and
        the remaining path must be tracked."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "src" / "introduction.md",
                "# Introduction\n\nSee [the plan]"
                "(https://github.com/nabbisen/sui-id/blob/main/PLAN.md).\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("(B) docs/src/introduction.md:3", result.stderr)
            self.assertIn("not tracked", result.stderr)

    def test_book_page_absolute_to_book_page_rejected(self):
        """Inside the book the relative form works in both renderings, so the
        absolute one is a link that stops following a page when it moves."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "src" / "introduction.md",
                "# Introduction\n\nSee [the overview]"
                "(https://github.com/nabbisen/sui-id/blob/main/docs/src/"
                "getting-started/overview.md).\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("(B) docs/src/introduction.md:3", result.stderr)
            self.assertIn("book page by relative path", result.stderr)

    def test_book_page_relative_outside_book_rejected(self):
        """The other half of rule 6. mdBook renders this as
        `../ROADMAP.html`, which the build never produces."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "src" / "introduction.md",
                "# Introduction\n\nSee [the roadmap](../../ROADMAP.md).\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("(B) docs/src/introduction.md:3", result.stderr)
            self.assertIn("rule 6", result.stderr)
            self.assertIn("never produces", result.stderr)

    def test_book_page_relative_inside_book_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "src" / "introduction.md",
                "# Introduction\n\nSee [the overview](./getting-started/overview.md).\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    # --- (C) version-claim freshness -------------------------------------

    def test_stale_claim_without_banner_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "pinned.md",
                "# Pinned document\n\nIt is current as of **v0.26.0**.\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("(C) docs/pinned.md:3", result.stderr)
            self.assertIn("carries no staleness banner", result.stderr)

    def test_stale_claim_with_banner_passes(self):
        """RFC 098 rule 5: the banner is the sanctioned state for a document
        that lags, so a bannered document passes however far behind it is."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "pinned.md",
                "# Pinned document\n\n"
                "> **Stale as of 2026-09-12.** Known to lag.\n\n"
                "It is current as of **v0.26.0**.\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_banner_below_the_window_does_not_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            filler = "\n".join(f"Line {n}." for n in range(1, 60))
            write(
                root / "docs" / "pinned.md",
                "# Pinned document\n\nIt is current as of **v0.26.0**.\n\n"
                + filler
                + "\n\n> **Stale as of 2026-09-12.** Too late to count.\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("carries no staleness banner", result.stderr)

    def test_listed_document_without_any_claim_rejected(self):
        """A pin that vanished is a claim that stopped being checkable."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "docs" / "pinned.md", "# Pinned document\n\nNo version here.\n")
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("(C) docs/pinned.md", result.stderr)
            self.assertIn("declares no version", result.stderr)

    def test_listed_document_missing_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            (root / "docs" / "pinned.md").unlink()
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("listed in the policy but does not exist", result.stderr)

    def test_claim_within_tolerance_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "pinned.md",
                "# Pinned document\n\nIt is current as of **v0.75.0**.\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_second_claim_form_is_recognised(self):
        """The policy's regex carries two alternatives; both must be read."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "docs" / "pinned.md",
                "# Pinned document\n\n*v3 — reflecting the v0.26.0 codebase.*\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("declares v0.26.0", result.stderr)


if __name__ == "__main__":
    unittest.main()
