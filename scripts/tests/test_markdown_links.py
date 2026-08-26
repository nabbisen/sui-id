"""RFC 093 G10b negative self-tests for scripts/check-markdown-links.py.

Run as: python3.14 -m unittest scripts.tests.test_markdown_links

Each fixture is a throwaway directory containing one or two Markdown files
with a single deliberate violation, invoked against the real script as a
subprocess (matching this project's convention elsewhere of testing
checkers as black boxes rather than importing their internals) -- the
script's own filename is not a valid Python module name (hyphens), which
this also sidesteps.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-markdown-links.py"


def run_checker(root: Path, *targets: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(root), *targets],
        capture_output=True,
        text=True,
    )


class MarkdownLinksTest(unittest.TestCase):
    def test_valid_links_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "target.md").write_text("# Target\n\nSome content.\n")
            (root / "source.md").write_text(
                "# Source\n\n"
                "[link](./target.md) and [anchor](./target.md#target) "
                "and [self](#source).\n"
            )
            result = run_checker(root, "source.md", "target.md")
            self.assertEqual(
                result.returncode, 0, msg=result.stdout + result.stderr
            )

    def test_missing_file_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "source.md").write_text(
                "# Source\n\n[broken](./does-not-exist.md)\n"
            )
            result = run_checker(root, "source.md")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("target does not exist", result.stderr)

    def test_bad_anchor_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "target.md").write_text("# Target\n\nNo such section.\n")
            (root / "source.md").write_text(
                "# Source\n\n[broken](./target.md#does-not-exist)\n"
            )
            result = run_checker(root, "source.md", "target.md")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("bad anchor", result.stderr)

    def test_absolute_local_path_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "source.md").write_text(
                "# Source\n\n[broken](/etc/passwd)\n"
            )
            result = run_checker(root, "source.md")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("absolute local path not allowed", result.stderr)

    def test_case_mismatched_path_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "Target.md").write_text("# Target\n")
            (root / "source.md").write_text(
                "# Source\n\n[broken](./target.md)\n"
            )
            result = run_checker(root, "source.md", "Target.md")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("case mismatch", result.stderr)

    def test_case_mismatch_outside_root_reports_not_crashes(self):
        # Review correction 1: a case-insensitive match that resolves
        # outside --root must still be reported as a case-mismatch
        # violation, not crash with an unhandled ValueError from
        # Path.relative_to.
        with tempfile.TemporaryDirectory() as tmp:
            outer = Path(tmp)
            (outer / "Outside.md").write_text("# Outside\n")
            root = outer / "root"
            root.mkdir()
            (root / "source.md").write_text(
                "# Source\n\n[broken](../outside.md)\n"
            )
            result = run_checker(root, "source.md")
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn("Traceback", result.stderr)
            self.assertIn("case mismatch", result.stderr)

    def test_link_title_stripped_before_resolving(self):
        # Review correction 2: a standard CommonMark link title must not
        # be treated as part of the target path.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "target.md").write_text("# Target\n")
            (root / "source.md").write_text(
                '# Source\n\n[link](./target.md "the title") and '
                "[single](./target.md 'the title').\n"
            )
            result = run_checker(root, "source.md", "target.md")
            self.assertEqual(
                result.returncode, 0, msg=result.stdout + result.stderr
            )

    def test_duplicate_heading_slugs_get_github_suffix(self):
        # GitHub de-duplicates repeated heading slugs by appending -1,
        # -2, ... to the second and later occurrences.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "target.md").write_text(
                "# Target\n\n## Foo\n\ntext\n\n## Foo\n\nmore text\n\n## Foo\n\nyet more\n"
            )
            (root / "source.md").write_text(
                "# Source\n\n"
                "[first](./target.md#foo) and "
                "[second](./target.md#foo-1) and "
                "[third](./target.md#foo-2).\n"
            )
            result = run_checker(root, "source.md", "target.md")
            self.assertEqual(
                result.returncode, 0, msg=result.stdout + result.stderr
            )

    def test_external_links_not_checked(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "source.md").write_text(
                "# Source\n\n"
                "[web](https://example.invalid/nothing) and "
                "[mail](mailto:nobody@example.invalid).\n"
            )
            result = run_checker(root, "source.md")
            self.assertEqual(
                result.returncode, 0, msg=result.stdout + result.stderr
            )

    def test_links_in_fenced_code_blocks_ignored(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "source.md").write_text(
                "# Source\n\n"
                "```markdown\n"
                "[example](./does-not-exist.md)\n"
                "```\n"
            )
            result = run_checker(root, "source.md")
            self.assertEqual(
                result.returncode, 0, msg=result.stdout + result.stderr
            )

    def test_regex_in_inline_code_span_is_not_read_as_a_link(self):
        """`^[a-z0-9](-?[a-z0-9])*$` parses as [a-z0-9](-?[a-z0-9]) unless
        inline code spans are stripped. Two real occurrences in
        rfcs/proposed/025-multi-tenant-expansion.md failed the checker until
        2026-08-27; G10b's scope excluded rfcs/proposed/, which is the only
        reason CI never saw it."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "source.md").write_text(
                "# Source\n\n"
                "`slug` matches `^[a-z0-9](-?[a-z0-9])*$`, length 2..64.\n"
            )
            result = run_checker(root, "source.md")
            self.assertEqual(
                result.returncode, 0, msg=result.stdout + result.stderr
            )

    def test_link_syntax_inside_a_code_span_is_not_checked(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "source.md").write_text(
                "# Source\n\nWrite `[text](./does-not-exist.md)` like this.\n"
            )
            result = run_checker(root, "source.md")
            self.assertEqual(
                result.returncode, 0, msg=result.stdout + result.stderr
            )

    def test_code_span_inside_link_text_still_resolves(self):
        """The guard must not make the checker blind: [`NAME`](./target.md) is
        used widely in this repository and its target must still be checked."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "source.md").write_text(
                "# Source\n\nSee [`ROADMAP.md`](./does-not-exist.md).\n"
            )
            result = run_checker(root, "source.md")
            self.assertEqual(
                result.returncode, 1, msg=result.stdout + result.stderr
            )
            self.assertIn("does-not-exist.md", result.stdout + result.stderr)

    def test_missing_target_argument_errors(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            result = run_checker(root, "does-not-exist.md")
            self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
