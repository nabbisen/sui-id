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
**Independent design review.** `reviewer`, [Review](../handoffs/100-example/100-review.md)
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
    write(root / "contracts" / "rfc-policy.toml", POLICY)
    write(root / "rfcs" / "README.md", VALID_README)
    write(root / "rfcs" / "accepted" / "100-example.md", VALID_RFC)
    write(root / "rfcs" / "handoffs" / "100-example" / "100-review.md", VALID_REVIEW)


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
        [sys.executable, str(CHECKER), "--root", str(root), "--policy", "contracts/rfc-policy.toml"],
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
        # rfcs/README.md (RFC 000/025 cross-references).
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
                "**Independent design review.** `reviewer`, [Review](../handoffs/100-example/100-review.md)\n",
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
                "[Review](../handoffs/100-example/100-review.md)", "[Review](../handoffs/100-example/does-not-exist.md)"
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
                "[Review](../handoffs/100-example/100-review.md)", "[Review](/etc/passwd)"
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
            (root / "rfcs" / "handoffs" / "100-example" / "100-review.md").unlink()
            git_commit(root)
            write(root / "rfcs" / "handoffs" / "100-example" / "100-review.md", VALID_REVIEW)
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
                "[Review](../handoffs/100-example/100-review.md)",
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
                "[Review](../handoffs/100-example/100-review.md)",
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


    def test_unsanctioned_folder_under_rfcs_rejected(self):
        # Invariant 12: RFC 000's layout is five folders, four holding RFCs,
        # plus optional draft/ and the handoffs/ companion folder. A sixth
        # top-level folder is a second place for lifecycle-looking documents
        # to accumulate. Before this check nothing noticed one -- discover_rfcs
        # iterates LIFECYCLE_FOLDERS and is blind to everything else, which is
        # how rfcs/reviews/ survived months of green runs.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "rfcs" / "reviews" / "100-review.md", VALID_REVIEW)
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertIn("unsanctioned folder under rfcs/: rfcs/reviews/", result.stderr)

    def test_sanctioned_optional_folders_accepted(self):
        # The same invariant must not fire on the layout RFC 000 does allow:
        # draft/ and handoffs/ are both optional and both legitimate. Without
        # this the check would pass for the wrong reason -- by forbidding
        # everything, including what the policy permits.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "rfcs" / "draft" / "101-wip.md", VALID_RFC)
            write(root / "rfcs" / "handoffs" / "100-example" / "README.md", "# Handoff\n")
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)


    def test_handoff_without_an_rfc_rejected(self):
        # Invariant 13: RFC 000 defines a handoff as a companion *to an RFC*
        # and requires every rfcs/handoffs/NNN-slug/ to correspond to an
        # existing RFC number. Work no RFC governs inherits a lifecycle status
        # that is not its own -- live work filed under a closed RFC reads as
        # historical. Six such directories accumulated unseen before
        # 2026-09-10; they now live in roadmap/, authorised by ROADMAP.md.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "rfcs" / "handoffs" / "tidy-up-the-tests" / "README.md", "# Package\n")
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertIn("handoff directory does not name an RFC", result.stderr)
            self.assertIn("tidy-up-the-tests", result.stderr)

    def test_handoff_naming_an_absent_rfc_rejected(self):
        # The rule is correspondence, not spelling: a directory can be shaped
        # NNN-slug and still name an RFC that does not exist. Renaming a stray
        # package to look numbered must not buy it a pass.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "rfcs" / "handoffs" / "742-invented" / "README.md", "# Package\n")
            git_commit(root)
            result = run_checker(root)
            self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertIn("resolves to 0 RFCs", result.stderr)

    def test_handoff_matching_an_existing_rfc_accepted(self):
        # And it must not fire on the case RFC 000 sanctions, or it would pass
        # by forbidding everything -- the baseline already carries RFC 100.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "rfcs" / "handoffs" / "100-example" / "README.md", "# Handoff\n")
            git_commit(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)


ARCHIVED_RFC = """\
# RFC 007 — Old design

**Status.** Withdrawn

## Summary

An archived RFC.
"""

DONE_RFC = """\
# RFC 050 — Older design

**Status.** Implemented (v0.1.0)

## Summary

A done RFC.
"""


class ReviewRuleGuardTest(unittest.TestCase):
    """RFC 110, conditions 14 and 15: an RFC header may record who reviewed
    something and may not rule on who is allowed to, and may not rest on an
    archived RFC. One invalid and one boundary-valid fixture per branch."""

    def run_tree(
        self,
        extra_header: str = "",
        body: str = "Example RFC used as a fixture baseline.\n",
        folder: str = "accepted",
        policy: str = POLICY,
        archived: bool = False,
        done: bool = False,
        header_replace: tuple[str, str] | None = None,
    ) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(root / "contracts" / "rfc-policy.toml", policy)
            header = VALID_RFC.split("## Summary")[0]
            if header_replace:
                header = header.replace(*header_replace)
            if extra_header:
                header = header.rstrip("\n") + "\n" + extra_header.rstrip("\n") + "\n\n"
            text = header + "## Summary\n\n" + body
            if folder == "accepted":
                write(root / "rfcs" / "accepted" / "100-example.md", text)
            else:
                # Move the RFC under test to another live/archive folder.
                (root / "rfcs" / "accepted" / "100-example.md").unlink()
                status = {"proposed": "Proposed", "done": "Implemented (v0.1.0)", "archive": "Withdrawn"}[folder]
                text = text.replace("**Status.** Accepted", f"**Status.** {status}")
                write(root / "rfcs" / folder / "100-example.md", text)
                readme = VALID_README.replace("./accepted/100-example.md", f"./{folder}/100-example.md")
                write(root / "rfcs" / "README.md", readme)
            readme = (root / "rfcs" / "README.md").read_text()
            if archived:
                write(root / "rfcs" / "archive" / "007-old.md", ARCHIVED_RFC)
                readme += "| 007 | [Old](./archive/007-old.md) |\n"
            if done:
                write(root / "rfcs" / "done" / "050-older.md", DONE_RFC)
                readme += "| 050 | [Older](./done/050-older.md) |\n"
            write(root / "rfcs" / "README.md", readme)
            git_commit(root)
            return run_checker(root)

    def assertRejected(self, result, *needles):
        self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
        for needle in needles:
            self.assertIn(needle, result.stderr)

    def assertPasses(self, result):
        self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    # ---- condition 14: phrases ------------------------------------------

    def test_each_review_rule_phrase_is_rejected_and_named(self):
        for phrase in (
            "must not have authored",
            "Role independence",
            "Independence means",
            "vendor is not a criterion",
        ):
            with self.subTest(phrase):
                r = self.run_tree(extra_header=f"Rule: the reviewer {phrase} this RFC.")
                self.assertRejected(
                    r, "100-example.md", "(condition 14)", phrase.lower(),
                    "may not rule on who is allowed to",
                )

    def test_message_names_the_line_of_the_phrase(self):
        r = self.run_tree(extra_header="filler\nThe reviewer must not have authored it.")
        # header: title(1), blank(2), Status(3) ... the phrase is the last header line.
        last_field = len(VALID_RFC.split("## Summary")[0].rstrip().splitlines())
        self.assertRejected(r, f"100-example.md:{last_field + 2}:")

    def test_phrase_wrapped_across_a_line_break_is_rejected(self):
        r = self.run_tree(extra_header="The reviewer must not\nhave authored the design.")
        self.assertRejected(r, "must not have authored")

    def test_phrase_split_by_emphasis_or_code_is_rejected(self):
        for text in (
            "The reviewer **must not** have authored it.",
            "The reviewer `must not have` authored it.",
            "The reviewer _must not_ have authored it.",
        ):
            with self.subTest(text):
                self.assertRejected(self.run_tree(extra_header=text), "condition 14")

    def test_invisible_characters_cannot_split_a_phrase(self):
        # A zero-width space or a bidi control is invisible in the rendered header.
        for text in ("The reviewer must not\u200b have authored it.", "Role\u202e independence per RFC 000."):
            with self.subTest(repr(text)):
                self.assertRejected(self.run_tree(extra_header=text), "condition 14")

    def test_phrase_case_variants_are_rejected(self):
        for text in ("ROLE INDEPENDENCE per RFC 000.", "role Independence per RFC 000."):
            with self.subTest(text):
                self.assertRejected(self.run_tree(extra_header=text), "role independence")

    def test_phrase_in_a_fenced_block_inside_the_header_is_still_header_text(self):
        r = self.run_tree(extra_header="```text\nRole independence per RFC 000\n```")
        self.assertRejected(r, "role independence")

    def test_a_fenced_heading_does_not_cut_the_header_short_and_hide_a_clause(self):
        # `parse_header` ends at the first `## `, even inside a fence; the guard's
        # boundary is fence-aware, so a clause after such a fence is still seen.
        hidden = "```text\n## not a heading\n```\nThe reviewer must not have authored it."
        self.assertRejected(self.run_tree(extra_header=hidden), "must not have authored")

    def test_the_same_phrase_in_the_body_is_accepted(self):
        r = self.run_tree(body="Role independence per RFC 000, and the reviewer must not have authored it.\n")
        self.assertPasses(r)

    def test_a_header_in_the_archive_is_out_of_scope(self):
        r = self.run_tree(extra_header="Role independence per RFC 000.", folder="archive")
        self.assertPasses(r)

    def test_a_proposed_and_a_done_header_are_in_scope(self):
        for folder in ("proposed", "done"):
            with self.subTest(folder):
                r = self.run_tree(extra_header="Role independence per RFC 000.", folder=folder)
                self.assertRejected(r, "condition 14")

    def test_rfc_000s_own_sentence_may_be_quoted_in_a_header(self):
        # D3: "cannot be the sole approver" is RFC 000's rule, not an invention.
        r = self.run_tree(
            extra_header="RFC 000: the implementer cannot be the sole approver of a design."
        )
        self.assertPasses(r)

    def test_sole_and_approver_separately_are_accepted(self):
        self.assertPasses(self.run_tree(extra_header="The sole owner is the approver of record."))

    # ---- condition 14: labels -------------------------------------------

    def test_the_field_that_carried_the_eleven_clauses_is_rejected(self):
        r = self.run_tree(
            extra_header="**Independent security and closure reviewer.** Per RFC 000 the reviewer is external."
        )
        self.assertRejected(
            r, "100-example.md", "'Independent security and closure reviewer.'",
            "(condition 14)", "may not rule on who is allowed to",
        )

    def test_a_renamed_review_field_is_rejected(self):
        for label in (
            "Reviewer requirements",
            "Independence",
            "Independent security reviewer",
            "Review authority",
            "Authorised reviewers",
        ):
            with self.subTest(label):
                r = self.run_tree(extra_header=f"**{label}.** Someone outside the author.")
                self.assertRejected(r, f"'{label}.'", "(condition 14)")

    def test_an_exemption_field_is_not_a_thing(self):
        # D5: there is no `Reviewer-rule exemption`; the label is itself rejected,
        # with or without a date and a link.
        for value in ("necessary.", "necessary, approved by the owner 2026-09-22, [x](../handoffs/100-example/100-review.md)."):
            with self.subTest(value):
                r = self.run_tree(extra_header=f"**Reviewer-rule exemption.** {value}")
                self.assertRejected(r, "'Reviewer-rule exemption.'")

    def test_the_six_recorded_review_labels_are_accepted(self):
        r = self.run_tree(
            extra_header=(
                "**Closure reviewed on.** 2026-01-02\n"
                "**Closure approved by.** `@owner`"
            )
        )
        self.assertPasses(r)

    def test_a_label_unrelated_to_review_is_accepted(self):
        self.assertPasses(self.run_tree(extra_header="**Amended on.** 2026-01-02 - why."))

    # ---- condition 15 ---------------------------------------------------

    def test_a_header_citing_an_archived_rfc_is_rejected(self):
        r = self.run_tree(extra_header="**Amended on.** 2026-01-02 - per RFC 007.", archived=True)
        self.assertRejected(r, "100-example.md", "header cites archived RFC 007", "(condition 15)")

    def test_plural_list_and_range_forms_are_rejected(self):
        for text in ("See RFCs 050 and 007.", "See RFCs 007, 050.", "See RFCs 005-010.", "See RFC-007."):
            with self.subTest(text):
                r = self.run_tree(extra_header=f"**Amended on.** {text}", archived=True, done=True)
                self.assertRejected(r, "header cites archived RFC 007")

    def test_a_markdown_link_into_the_archive_is_rejected(self):
        r = self.run_tree(
            extra_header="**Amended on.** 2026-01-02 - see [the old design](../archive/007-old.md).",
            archived=True,
        )
        self.assertRejected(r, "header cites archived RFC 007")

    def test_a_header_citing_a_done_rfc_is_accepted(self):
        r = self.run_tree(extra_header="**Amended on.** 2026-01-02 - per RFC 050.", archived=True, done=True)
        self.assertPasses(r)

    def test_a_bare_number_is_not_a_citation(self):
        self.assertPasses(self.run_tree(extra_header="**Amended on.** i18n 007 and commit 007abc.", archived=True))

    def test_an_rfcs_own_number_and_title_are_not_citations(self):
        self.assertPasses(self.run_tree(extra_header="**Amended on.** RFC 100 amended itself.", archived=True))

    def test_the_title_line_is_not_read_for_citations(self):
        # D6: the title line names the RFC itself (and an archived RFC's own title
        # is what put two archive files in the red before the scope fix); it is
        # skipped whole, not only when it names its own number.
        r = self.run_tree(header_replace=("# RFC 100 — Example", "# RFC 100 — The successor to RFC 007"), archived=True)
        self.assertPasses(r)

    def test_the_same_citation_in_the_body_is_accepted(self):
        r = self.run_tree(body="This supersedes RFC 007 historically.\n", archived=True)
        self.assertPasses(r)

    def test_an_archived_header_may_name_an_archived_rfc(self):
        r = self.run_tree(extra_header="**Amended on.** per RFC 007.", folder="archive", archived=True)
        self.assertPasses(r)

    def test_the_policy_allowlist_admits_exactly_its_entry(self):
        header = "**Amended on.** 2026-01-02 - Supersedes RFC 007."
        without = self.run_tree(extra_header=header, archived=True)
        self.assertRejected(without, "header cites archived RFC 007")
        allowed = POLICY + '\n[archive_citations]\n"100" = ["007"]\n'
        self.assertPasses(self.run_tree(extra_header=header, archived=True, policy=allowed))
        # An entry for another RFC, or for another cited number, admits nothing.
        for entry in ('"101" = ["007"]', '"100" = ["018"]'):
            with self.subTest(entry):
                policy = POLICY + f"\n[archive_citations]\n{entry}\n"
                self.assertRejected(
                    self.run_tree(extra_header=header, archived=True, policy=policy),
                    "header cites archived RFC 007",
                )

    def test_the_allowlist_does_not_silence_condition_14(self):
        policy = POLICY + '\n[archive_citations]\n"100" = ["007"]\n'
        r = self.run_tree(extra_header="Role independence per RFC 007.", archived=True, policy=policy)
        self.assertRejected(r, "(condition 14)")

    # ---- the repository as it stands ------------------------------------

    def test_the_repository_as_it_stands_passes_both_conditions(self):
        result = subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(REPO_ROOT), "--policy", "contracts/rfc-policy.toml"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
        self.assertNotIn("condition 14", result.stderr)
        self.assertNotIn("condition 15", result.stderr)

    def test_the_real_policy_names_the_one_legitimate_citation(self):
        import tomllib

        with (REPO_ROOT / "contracts" / "rfc-policy.toml").open("rb") as f:
            policy = tomllib.load(f)
        self.assertEqual(policy["archive_citations"], {"025": ["007"]})


if __name__ == "__main__":
    unittest.main()
