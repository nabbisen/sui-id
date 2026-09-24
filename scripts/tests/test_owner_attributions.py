"""RFC 117 stage 0 (G16) negative self-tests for scripts/check-owner-attributions.py.

Run as: python3.14 -m unittest scripts.tests.test_owner_attributions

Each fixture is a throwaway git repository carrying the **real**
`ci/owner-attributions.toml` (so the patterns under test are the shipped ones),
mutated one way per test. The checker is invoked as a subprocess, matching this
project's convention of testing checkers as black boxes; a few pure-function
tests import it directly for the phrasing cases.
"""

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-owner-attributions.py"
REAL_POLICY = (REPO_ROOT / "ci" / "owner-attributions.toml").read_text()

_spec = importlib.util.spec_from_file_location("check_owner_attributions", CHECKER)
coa = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(coa)
POLICY = coa.load_policy(str(REPO_ROOT / "ci" / "owner-attributions.toml"))

# The three sentences RFC 117 was written to catch, as they appeared.
JULY = "**Vendor independence required** by the owner's 2026-07-28 S1 ruling."
JULY_ROADMAP = "Owner ruling, 2026-07-28: a two-tier scheme."
SEPTEMBER_098 = (
    "`@nabbisen` ruled on 2026-09-09 that the implementation role is not a reviewer."
)
ROADMAP_DIR = 'The directory justified itself as an "Owner ruling, 2026-09-10".'


def run(*args, cwd, env=None):
    return subprocess.run(
        ["python3.14", str(CHECKER), *args],
        cwd=cwd,
        capture_output=True,
        text=True,
        env=env,
    )


class Repo:
    """A synthetic git repository with the real policy and a baseline taken
    from whatever is in it at `adopt()`."""

    def __init__(self, tmp):
        self.root = Path(tmp)
        self._git("init", "-q", "-b", "main")
        self._git("config", "user.email", "t@example.invalid")
        self._git("config", "user.name", "t")
        self._git("config", "commit.gpgsign", "false")
        self.write("ci/owner-attributions.toml", REAL_POLICY)

    def _git(self, *args):
        subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)

    def write(self, rel, text):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self, msg="c"):
        self._git("add", "-A")
        self._git("commit", "-q", "-m", msg)

    def adopt(self):
        r = run(
            "--root", ".", "--policy", "ci/owner-attributions.toml", "--update-baseline",
            cwd=self.root,
        )
        assert r.returncode == 0, r.stderr
        return r

    def check(self, *extra, env=None):
        return run(
            "--root", ".", "--policy", "ci/owner-attributions.toml", *extra,
            cwd=self.root, env=env,
        )


class Phrasing(unittest.TestCase):
    """What counts as an attribution."""

    def hits(self, text, path="doc.md"):
        return coa.census({path: text}, POLICY)

    def test_the_narrow_pattern_misses_september_and_this_one_does_not(self):
        # `@nabbisen ruled on <date>` has no "owner ruling" in it: the pattern
        # the RFC's first draft used does not match it.
        self.assertEqual(len(self.hits(SEPTEMBER_098)), 1)

    def test_the_two_other_incidents_are_found(self):
        self.assertEqual(len(self.hits(JULY)), 1)
        self.assertEqual(len(self.hits(JULY_ROADMAP)), 1)
        self.assertEqual(len(self.hits(ROADMAP_DIR)), 1)

    def test_an_undated_attribution_is_found(self):
        self.assertEqual(len(self.hits("The owner decided that reviews are optional.")), 1)

    def test_a_sentence_wrapped_across_lines_is_found(self):
        wrapped = "It is recorded that `@nabbisen`\nruled on the matter last week.\n"
        self.assertEqual(len(self.hits(wrapped)), 1)

    def test_a_table_row_and_a_list_item_are_their_own_units(self):
        text = (
            "| Date | Ruling |\n|---|---|\n"
            "| 2026-09-10 | The owner ruled the directory should exist |\n"
            "\n- unrelated item\n- `@nabbisen` approved the plan\n"
        )
        self.assertEqual(len(self.hits(text)), 2)

    def test_a_url_is_not_the_owner(self):
        self.assertEqual(
            self.hits("See https://github.com/nabbisen/sui-id for the decision log."), []
        )

    def test_an_owner_without_a_verb_is_not_an_attribution(self):
        self.assertEqual(self.hits("The client owner is listed on the page."), [])

    def test_only_comments_are_read_in_code(self):
        code = (
            'let s = "the owner decided this";\n'
            "// The owner ruled that this stays.\n"
            "fn f() {}\n"
        )
        hits = coa.census({"src/lib.rs": code}, POLICY)
        self.assertEqual([h[1] for h in hits], ["The owner ruled that this stays."])

    def test_re_wrapping_or_re_emphasising_does_not_change_the_identity(self):
        a = coa.census({"d.md": "The owner **ruled** on it today.\n"}, POLICY)
        b = coa.census({"d.md": "The owner\nruled on   it today.\n"}, POLICY)
        self.assertEqual(coa.tally(a), coa.tally(b))

    def test_a_changed_word_changes_the_identity(self):
        a = coa.census({"d.md": "The owner ruled on it today.\n"}, POLICY)
        b = coa.census({"d.md": "The owner ruled on it yesterday.\n"}, POLICY)
        self.assertNotEqual(coa.tally(a), coa.tally(b))


class Gate(unittest.TestCase):
    """The verdict, end to end."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.repo = Repo(self._tmp.name)

    def test_a_baselined_attribution_passes(self):
        self.repo.write("doc.md", f"# Doc\n\n{SEPTEMBER_098}\n")
        self.repo.adopt()
        self.repo.commit()
        r = self.repo.check()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        # (the policy file's own comment is an attribution too, and is baselined)
        self.assertIn("**0 new**", r.stdout)
        self.assertIn("`doc.md`", r.stdout)

    def test_a_new_attribution_fails_whatever_date_it_names(self):
        self.repo.write("doc.md", "# Doc\n\nNothing here.\n")
        self.repo.adopt()
        for label, sentence in [
            ("dated after adoption", "The owner ruled on 2026-12-01 that this is required."),
            ("back-dated before adoption", "The owner ruled on 2026-08-01 that this is required."),
            ("no date at all", "The owner decided that this is required."),
        ]:
            with self.subTest(label):
                self.repo.write("new.md", f"# New\n\n{sentence}\n")
                r = self.repo.check()
                self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
                self.assertIn("### New: not in the baseline", r.stdout)
                self.assertIn(sentence, r.stdout)

    def test_a_new_attribution_in_a_code_comment_fails(self):
        self.repo.write("doc.md", "# Doc\n")
        self.repo.adopt()
        self.repo.write("src/lib.rs", "// The owner approved skipping the check.\nfn f() {}\n")
        self.assertEqual(self.repo.check().returncode, 1)

    def test_editing_a_baselined_attribution_makes_it_new(self):
        self.repo.write("doc.md", f"# Doc\n\n{SEPTEMBER_098}\n")
        self.repo.adopt()
        self.repo.write(
            "doc.md", "# Doc\n\n`@nabbisen` ruled on 2026-09-09 that the role is optional.\n"
        )
        r = self.repo.check()
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("1 baseline entries no longer present", r.stdout)

    def test_a_copy_of_a_baselined_sentence_is_new_for_the_surplus(self):
        self.repo.write("doc.md", f"# Doc\n\n{SEPTEMBER_098}\n")
        self.repo.adopt()
        self.repo.write("doc.md", f"# Doc\n\n{SEPTEMBER_098}\n\nMore.\n\n{SEPTEMBER_098}\n")
        r = self.repo.check()
        self.assertEqual(r.returncode, 1, r.stdout)

    def test_the_same_sentence_in_another_file_is_new(self):
        self.repo.write("a.md", f"# A\n\n{SEPTEMBER_098}\n")
        self.repo.adopt()
        self.repo.write("b.md", f"# B\n\n{SEPTEMBER_098}\n")
        self.assertEqual(self.repo.check().returncode, 1)

    def test_a_ledger_citation_does_not_clear_a_new_attribution_in_stage_0(self):
        # There is no ledger before stage 1, and a citation checked against an
        # unsigned file would be a hole. A citation, or a fabricated ledger
        # file, must not rescue a new attribution.
        self.repo.write("doc.md", "# Doc\n")
        self.repo.adopt()
        self.repo.write("ci/decisions.txt", "=== DECISION D-0001 ===\nthe owner ruled\n=== END D-0001 ===\n")
        self.repo.write("new.md", "# New\n\nThe owner ruled on this (D-0001).\n")
        r = self.repo.check()
        self.assertEqual(r.returncode, 1, r.stdout)

    def test_removing_an_attribution_passes_and_is_reported_as_stale(self):
        self.repo.write("doc.md", f"# Doc\n\n{SEPTEMBER_098}\n")
        self.repo.adopt()
        self.repo.write("doc.md", "# Doc\n")
        r = self.repo.check()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("1 baseline entries no longer present", r.stdout)

    def test_an_untracked_but_not_ignored_file_is_scanned(self):
        self.repo.write("doc.md", "# Doc\n")
        self.repo.adopt()
        self.repo.commit()
        self.repo.write("later.md", "# Later\n\nThe owner ruled on this.\n")  # untracked
        self.assertEqual(self.repo.check().returncode, 1)

    def test_every_hit_is_printed_and_written_to_the_step_summary(self):
        self.repo.write("doc.md", f"# Doc\n\n{SEPTEMBER_098}\n\nThe owner decided X.\n")
        self.repo.adopt()
        summary = Path(self._tmp.name) / "summary.md"
        r = self.repo.check(env={"GITHUB_STEP_SUMMARY": str(summary), "PATH": _path()})
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        for text in (SEPTEMBER_098.replace("`", ""), "The owner decided X."):
            self.assertIn(text, r.stdout)
            self.assertIn(text, summary.read_text())

    def test_a_malformed_baseline_line_is_a_usage_error_not_a_pass(self):
        self.repo.write("doc.md", f"# Doc\n\n{SEPTEMBER_098}\n")
        self.repo.adopt()
        baseline = self.repo.root / "ci" / "owner-attribution-baseline.txt"
        baseline.write_text(baseline.read_text() + "doc.md\tnot-a-hash\t1\tx\n")
        r = self.repo.check()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)

    def test_a_missing_baseline_is_a_usage_error_not_a_pass(self):
        self.repo.write("doc.md", f"# Doc\n\n{SEPTEMBER_098}\n")
        r = self.repo.check()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)

    def test_a_malformed_policy_is_a_usage_error(self):
        self.repo.write("ci/owner-attributions.toml", "version = 1\n")
        r = self.repo.check()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)


class Replay(unittest.TestCase):
    """The historic-commit check: was the attribution new at that commit?"""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.repo = Repo(self._tmp.name)

    def test_a_commit_that_introduces_an_attribution_is_rejected_against_its_parent(self):
        self.repo.write("ROADMAP.md", "# Roadmap\n\nNothing decided.\n")
        self.repo.commit("before")
        self.repo.write("ROADMAP.md", f"# Roadmap\n\n{JULY_ROADMAP}\n")
        self.repo.commit("introduces the attribution")
        r = self.repo.check("--rev", "HEAD", "--baseline-rev", "HEAD^")
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("Owner ruling, 2026-07-28", r.stdout)

    def test_a_commit_that_adds_none_passes_against_its_parent(self):
        self.repo.write("ROADMAP.md", f"# Roadmap\n\n{JULY_ROADMAP}\n")
        self.repo.commit("has it")
        self.repo.write("other.md", "# Other\n")
        self.repo.commit("unrelated")
        r = self.repo.check("--rev", "HEAD", "--baseline-rev", "HEAD^")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)


class BaselineDiff(unittest.TestCase):
    """Stage 0b: what a baseline edit changed is printed above the census."""

    NEW = "The owner ruled on 2026-12-01 that this is required."

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.repo = Repo(self._tmp.name)
        self.repo.write("doc.md", "# Doc\n\nNothing here.\n")
        self.repo.adopt()
        self.repo.commit("adopt")

    def _launder(self):
        """A branch that adds an attribution AND baselines it, in one commit."""
        self.repo.write("new.md", f"# New\n\n{self.NEW}\n")
        self.repo.adopt()
        self.repo.commit("add an attribution and clear it")

    def _event(self, name, payload):
        path = Path(self._tmp.name) / "event.json"
        path.write_text(__import__("json").dumps(payload))
        return {"GITHUB_EVENT_NAME": name, "GITHUB_EVENT_PATH": str(path), "PATH": _path()}

    def test_a_branch_that_adds_and_baselines_shows_the_added_line_above_the_census(self):
        self._launder()
        r = self.repo.check("--base", "HEAD^")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)  # the gate itself passes
        out = r.stdout
        self.assertIn("## Baseline changes (RFC 117 stage 0b)", out)
        self.assertIn("**changed**", out)
        self.assertIn("1 line(s) added", out)
        self.assertRegex(out, r"\+new\.md\t[0-9a-f]{64}\t1\t" + self.NEW[:40])
        # ... above the census, so what clears a hit is beside the hit it clears.
        self.assertLess(
            out.index("## Baseline changes"), out.index("## Owner attributions")
        )

    def test_a_branch_that_touches_neither_shows_an_empty_section_not_a_broken_one(self):
        self.repo.write("other.md", "# Other\n")
        self.repo.commit("unrelated")
        r = self.repo.check("--base", "HEAD^")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("## Baseline changes (RFC 117 stage 0b)", r.stdout)
        self.assertIn("is unchanged since", r.stdout)
        self.assertNotIn("```diff", r.stdout)

    def test_a_removed_baseline_line_is_shown_too(self):
        # Deleting a baseline entry (and the attribution with it) is also a
        # change to the closed set.
        self.repo.write("old.md", "# Old\n\nThe owner decided the old thing.\n")
        self.repo.adopt()
        self.repo.commit("baseline an old attribution")
        self.repo.write("old.md", "# Old\n")
        self.repo.adopt()
        self.repo.commit("remove it and prune the baseline")
        r = self.repo.check("--base", "HEAD^")
        self.assertIn("1 removed", r.stdout)

    def test_no_base_is_a_stated_message_not_a_failure(self):
        self._launder()
        # No `--base` and no event payload: a manual or detached run.
        r = self.repo.check(env={"PATH": _path()})
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("**No base revision", r.stdout)
        self.assertIn("this is not a GitHub event run", r.stdout)
        self.assertIn("## Owner attributions", r.stdout, "the census is still printed")

    def test_a_first_push_has_no_base_and_says_why(self):
        self._launder()
        env = self._event("push", {"before": "0" * 40, "after": "abc"})
        r = self.repo.check(env=env)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("**No base revision", r.stdout)
        self.assertIn("a first push, a new branch or a new tag", r.stdout)

    def test_a_push_uses_the_previous_commit(self):
        self._launder()
        before = subprocess.run(
            ["git", "rev-parse", "HEAD^"], cwd=self.repo.root, capture_output=True, text=True
        ).stdout.strip()
        r = self.repo.check(env=self._event("push", {"before": before}))
        self.assertIn("the push's previous commit", r.stdout)
        self.assertIn("1 line(s) added", r.stdout)

    def test_a_pull_request_uses_its_base(self):
        self._launder()
        base = subprocess.run(
            ["git", "rev-parse", "HEAD^"], cwd=self.repo.root, capture_output=True, text=True
        ).stdout.strip()
        env = self._event("pull_request", {"pull_request": {"base": {"sha": base}}})
        r = self.repo.check(env=env)
        self.assertIn("the pull request's base", r.stdout)
        self.assertIn("1 line(s) added", r.stdout)

    def test_a_base_that_is_not_in_this_clone_is_a_stated_message(self):
        # A shallow checkout, or a history that was rewritten: the sha is not a commit here.
        self._launder()
        r = self.repo.check("--base", "1" * 40)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("**No base revision", r.stdout)
        self.assertIn("is not a commit in this clone", r.stdout)

    def test_a_manual_run_names_the_event_that_has_no_base(self):
        r = self.repo.check(env=self._event("workflow_dispatch", {}))
        self.assertIn("a `workflow_dispatch` event has no base revision", r.stdout)

    def test_an_unreadable_event_payload_degrades_to_a_message(self):
        env = {"GITHUB_EVENT_NAME": "push", "GITHUB_EVENT_PATH": "/nonexistent/event.json", "PATH": _path()}
        r = self.repo.check(env=env)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("the event payload could not be read", r.stdout)

    def test_the_step_summary_carries_the_section_first(self):
        self._launder()
        summary = Path(self._tmp.name) / "summary.md"
        env = {"GITHUB_STEP_SUMMARY": str(summary), "PATH": _path()}
        r = self.repo.check("--base", "HEAD^", env=env)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        text = summary.read_text()
        self.assertLess(text.index("## Baseline changes"), text.index("## Owner attributions"))
        self.assertIn("1 line(s) added", text)

    def test_the_section_never_changes_the_verdict(self):
        # A new, un-baselined attribution still fails, whatever the diff says.
        self.repo.write("new.md", f"# New\n\n{self.NEW}\n")
        r = self.repo.check("--base", "HEAD")
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("### New: not in the baseline", r.stdout)

    def test_replay_modes_do_not_print_the_section(self):
        # `--rev` / `--baseline-rev` replay a historic commit against its parent;
        # there is no "this change" to diff, so the section is not printed.
        r = self.repo.check("--rev", "HEAD", "--baseline-rev", "HEAD")
        self.assertNotIn("## Baseline changes", r.stdout)


def _path():
    import os

    return os.environ.get("PATH", "")


if __name__ == "__main__":
    unittest.main()
