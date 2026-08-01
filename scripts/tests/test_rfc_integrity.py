"""RFC 093 G11 negative self-tests for scripts/check-rfc-integrity.py.

Run as: python3.14 -m unittest scripts.tests.test_rfc_integrity

Each fixture is a throwaway, git-backed synthetic rfcs/ tree (git-backed
because the evidence-field rules for Independent design review / Closure
evidence require `git ls-files`/`git check-ignore` to resolve against a
real repository) with a minimal valid baseline RFC, mutated one way per
test. Invoked against the real script as a subprocess, matching this
project's convention elsewhere (check-markdown-links.py, the bash
checkers) of testing checkers as black boxes.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-rfc-integrity.py"

POLICY = """\
version = 1

[historical_rfc_mi]
ids = ["RFC-MI-999"]

[metadata_required_from]
min_number = 100
"""

VALID_RFC = """\
# RFC 100 — Example

**Status.** Accepted
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** None.
**Tracks.** Example.
**Touches.** nothing.
**Accepted on.** 2026-01-01
**Approved by.** `@owner`
**Independent design review.** `reviewer`, [Review](../reviews/100-review.md)
**Accountable owner and approver.** `@owner`.

## Summary

Example RFC used as a fixture baseline.
"""

VALID_REVIEW = """\
# Review of RFC 100

Accept.
"""

VALID_README = """\
# sui-id RFCs

## Index

| RFC | Title |
|---|---|
| 100 | [Example](./accepted/100-example.md) |
"""


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def make_baseline(root: Path) -> None:
    write(root / "ci" / "rfc-policy.toml", POLICY)
    write(root / "rfcs" / "README.md", VALID_README)
    write(root / "rfcs" / "accepted" / "100-example.md", VALID_RFC)
    write(root / "rfcs" / "reviews" / "100-review.md", VALID_REVIEW)


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
        [sys.executable, str(CHECKER), "--root", str(root), "--policy", "ci/rfc-policy.toml"],
        capture_output=True,
        text=True,
    )


class RfcIntegrityTest(unittest.TestCase):
    def test_valid_baseline_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_duplicate_identifier_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "rfcs" / "proposed" / "100-duplicate.md", VALID_RFC)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("duplicate identifier 100", result.stderr)

    def test_status_folder_mismatch_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC.replace("**Status.** Accepted", "**Status.** Proposed")
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("requires Status starting with", result.stderr)

    def test_no_status_field_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC.replace("**Status.** Accepted\n", "")
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("no Status field found", result.stderr)

    def test_missing_index_row_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "rfcs" / "README.md", "# sui-id RFCs\n\n## Index\n\nNothing here.\n")
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("not indexed", result.stderr)

    def test_duplicate_index_row_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            doubled = VALID_README + "| 100 | [Example again](./accepted/100-example.md) |\n"
            write(root / "rfcs" / "README.md", doubled)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("indexed 2 times", result.stderr)

    def test_index_row_in_other_rfcs_row_not_counted(self):
        # A link to RFC 100 appearing inside a *different* RFC's own
        # table row (e.g. a "superseded by" cell) must not count as
        # RFC 100's index entry, and prose links outside any table row
        # must not either -- both are real patterns in the live
        # rfcs/README.md (RFC 018/025 cross-references).
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            readme = (
                VALID_README
                + "\nSee also [RFC 100](./accepted/100-example.md) in prose.\n\n"
                + "| 200 | [Other, see RFC 100](./accepted/100-example.md) |\n"
            )
            write(root / "rfcs" / "README.md", readme)
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_broken_link_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC + "\nSee [nope](./does-not-exist.md) for details.\n"
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("target does not exist", result.stderr)

    def test_nested_fence_does_not_desync_link_scanning(self):
        # A ```lang``` fence nested for illustration inside an outer
        # fenced block (no valid CommonMark closer in between) must not
        # desync a naive per-``` toggle and hide a real broken link
        # afterward (rfcs/done/076's actual shape).
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC + (
                "\n```markdown\nexample\n```sh\ncommand\n```\n\n"
                "See [nope](./does-not-exist.md) after the nested fence.\n"
            )
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("target does not exist", result.stderr)

    def test_stray_file_directly_under_rfcs_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "rfcs" / "999-stray.md", "# stray\n")
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("stray RFC file directly under rfcs/", result.stderr)

    def test_missing_prospective_metadata_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC.replace("**Tracks.** Example.\n", "")
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing required metadata field 'Tracks.'", result.stderr)

    def test_prospective_mi_missing_metadata_rejected(self):
        # An MI identifier NOT on the closed historical list is
        # prospective and needs the full field set, same as a standard
        # RFC >= the threshold.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "rfcs" / "proposed" / "RFC-MI-500-new-epic-item.md",
                "# RFC-MI-500 — New epic item\n\n**Status.** Proposed\n\n## Summary\n\nNew.\n",
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("RFC-MI-500-new-epic-item.md: missing required metadata field", result.stderr)
            self.assertIn("identifier RFC-MI-500 requires full metadata", result.stderr)

    def test_historical_mi_needs_no_invented_metadata(self):
        # The required boundary-valid case: an MI identifier that IS on
        # the closed historical list is exempt from the full metadata
        # field set and must not be flagged for lacking fields real
        # historical MI RFCs never had. Real RFC-MI-* files use TOML
        # front-matter, not the bold-label convention -- this fixture
        # matches that real shape exactly (design decision,
        # m1b-c2-rfc-integrity-checker-review-2026-08-01.md §4), so it
        # exercises the narrowed TOML reader, not just the metadata
        # exemption.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "rfcs" / "done" / "RFC-MI-999-historical-item.md",
                '# RFC-MI-999: Historical item\n\n'
                '```toml\n'
                'id = "RFC-MI-999"\n'
                'title = "Historical item"\n'
                'status = "Implemented (v0.49.1)"\n'
                '```\n\n## Summary\n\nShipped long ago.\n',
            )
            readme = VALID_README + "| MI-999 | [Historical item](./done/RFC-MI-999-historical-item.md) |\n"
            write(root / "rfcs" / "README.md", readme)
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_off_list_mi_toml_status_not_read(self):
        # The narrowing itself: an MI identifier NOT on the closed
        # historical list must not have its TOML status read, even
        # though the file is otherwise shaped exactly like a historical
        # one -- only identifiers already on the list get the TOML
        # reader. Off-list, it must still fail as having no recognized
        # Status field (on top of the separate missing-metadata failure
        # already covered by test_prospective_mi_missing_metadata_rejected).
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "rfcs" / "done" / "RFC-MI-501-new-epic-item.md",
                '# RFC-MI-501: New epic item\n\n'
                '```toml\n'
                'id = "RFC-MI-501"\n'
                'status = "Implemented (v1.0.0)"\n'
                '```\n\n## Summary\n\nNot on the historical list.\n',
            )
            readme = VALID_README + "| MI-501 | [New epic item](./done/RFC-MI-501-new-epic-item.md) |\n"
            write(root / "rfcs" / "README.md", readme)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(
                "RFC-MI-501-new-epic-item.md: no Status field found in the RFC header",
                result.stderr,
            )

    def test_accepted_missing_acceptance_metadata_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC.replace("**Approved by.** `@owner`\n", "")
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing 'Approved by.'", result.stderr)

    def test_accepted_security_required_missing_review_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC.replace(
                "**Independent design review.** `reviewer`, [Review](../reviews/100-review.md)\n",
                "",
            )
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Independent design review.' is missing", result.stderr)

    def test_evidence_link_missing_target_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC.replace(
                "[Review](../reviews/100-review.md)", "[Review](../reviews/does-not-exist.md)"
            )
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("link target does not exist", result.stderr)

    def test_evidence_link_absolute_path_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC.replace(
                "[Review](../reviews/100-review.md)", "[Review](/etc/passwd)"
            )
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("absolute local path", result.stderr)

    def test_evidence_link_untracked_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            # The review file must never have been added/committed at all
            # -- rewriting a path that *was* committed doesn't untrack it,
            # since `git ls-files` reads the index, not file mtimes.
            (root / "rfcs" / "reviews" / "100-review.md").unlink()
            git_commit(root)
            write(root / "rfcs" / "reviews" / "100-review.md", VALID_REVIEW)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("not tracked by git", result.stderr)

    def test_evidence_link_gitignored_rejected(self):
        # An untracked path under .git-exclude/ -- the realistic shape
        # RFC 093 names explicitly. check-ignore is checked before
        # ls-files precisely so this reports "gitignored" rather than
        # the less specific "not tracked" (git does not report an
        # already-tracked path as ignored even when a later .gitignore
        # pattern matches it, so ordering the other way would make this
        # branch unreachable for a force-added file -- but this fixture
        # exercises the ordinary, never-tracked case that RFC 093 means).
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC.replace(
                "[Review](../reviews/100-review.md)",
                "[Review](../../.git-exclude/100-review.md)",
            )
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            write(root / ".gitignore", "/.git-exclude/\n")
            git_commit(root)
            write(root / ".git-exclude" / "100-review.md", VALID_REVIEW)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("gitignored", result.stderr)

    def test_evidence_link_external_only_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = VALID_RFC.replace(
                "[Review](../reviews/100-review.md)",
                "[Review](https://example.invalid/review)",
            )
            write(root / "rfcs" / "accepted" / "100-example.md", bad)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("external-only", result.stderr)

    def test_done_security_sensitive_missing_closure_metadata_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            done_rfc = VALID_RFC.replace("**Status.** Accepted", "**Status.** Implemented (v1.0.0)")
            write(root / "rfcs" / "done" / "100-example.md", done_rfc)
            (root / "rfcs" / "accepted" / "100-example.md").unlink()
            write(
                root / "rfcs" / "README.md",
                VALID_README.replace("./accepted/100-example.md", "./done/100-example.md"),
            )
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing 'Closure reviewed on.'", result.stderr)
            self.assertIn("missing 'Closure evidence.'", result.stderr)

    def test_historical_pre_threshold_rfc_needs_no_metadata(self):
        # Invariant 11: an RFC below metadata_required_from.min_number
        # gets only the structural checks -- no reviewer is invented.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "rfcs" / "done" / "001-old.md",
                "# RFC 001 — Old\n\n**Status.** Implemented (v0.1.0)\n\n## Summary\n\nShipped long ago, minimal header.\n",
            )
            readme = VALID_README + "| 001 | [Old](./done/001-old.md) |\n"
            write(root / "rfcs" / "README.md", readme)
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
