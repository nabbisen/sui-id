"""RFC 143 stage 3 part A negative self-tests for
scripts/extract-changelog-section.py.

Run as: python3.14 -m unittest scripts.tests.test_extract_changelog_section
"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT_PATH = REPO_ROOT / "scripts" / "extract-changelog-section.py"

spec = importlib.util.spec_from_file_location("extract_changelog_section", SCRIPT_PATH)
ecs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ecs)  # type: ignore[union-attr]

SAMPLE = """\
# Changelog

## [Unreleased]

### Added

- an unreleased thing, see [docs](docs/x.md).

## [1.2.0] — 2026-01-02

### Fixed

- a fix with a [relative link](docs/x.md), an [absolute one](https://example.com),
  a [fragment](#section), a [root path](/abs),
  and here is a fenced block that must survive untouched:

  ```toml
  # [not-a-link](not/a/target.md)
  key = "value"
  ```

  and an inline code span: `[also-not-a-link](also/not.md)` stays.

### Added

- second item.

## [1.1.0] — 2026-01-01

- older release.

## [0.8.10] — 2025-12-31

- a release whose version starts with "0.8.1" but is not it.
"""


class SectionExtraction(unittest.TestCase):
    def test_extracts_exactly_one_section_and_drops_the_heading(self) -> None:
        lines = ecs.extract_section_lines(SAMPLE, "1.1.0")
        self.assertIsNotNone(lines)
        text = "\n".join(lines)
        self.assertNotIn("## [1.1.0]", text)
        self.assertIn("older release.", text)
        self.assertNotIn("0.8.10", text)

    def test_stops_at_the_next_level_2_heading_not_a_level_3_one(self) -> None:
        lines = ecs.extract_section_lines(SAMPLE, "1.2.0")
        text = "\n".join(lines)
        self.assertIn("### Added", text)
        self.assertIn("second item.", text)
        self.assertNotIn("## [1.1.0]", text)

    def test_version_must_be_a_whole_match_not_a_prefix(self) -> None:
        # "0.8.1" must not match the "## [0.8.10]" heading.
        lines = ecs.extract_section_lines(SAMPLE, "0.8.1")
        self.assertIsNone(lines, "0.8.1 must not match the 0.8.10 heading")

    def test_a_pre_release_suffix_does_not_match_its_base_version(self) -> None:
        sample = SAMPLE.replace("## [1.1.0]", "## [0.8.1-rc.1]", 1)
        lines = ecs.extract_section_lines(sample, "0.8.1")
        self.assertIsNone(lines, "0.8.1 must not match 0.8.1-rc.1")

    def test_unreleased_extracts_its_own_section(self) -> None:
        lines = ecs.extract_section_lines(SAMPLE, "Unreleased")
        self.assertIsNotNone(lines)
        self.assertIn("an unreleased thing", "\n".join(lines))

    def test_a_version_string_never_matches_unreleased(self) -> None:
        lines = ecs.extract_section_lines(SAMPLE, "1.2.0")
        self.assertNotIn("an unreleased thing", "\n".join(lines))


class LinkRewriting(unittest.TestCase):
    """`root` is a real checkout fixture with `docs/x.md` and
    `rfcs/done/y.md` present, and nothing else -- RFC 143 stage 4's own
    existence check reads real files, so the fixture must have them, not
    merely assert they're not checked."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        (self.root / "docs").mkdir()
        (self.root / "docs" / "x.md").write_text("x", encoding="utf-8")
        (self.root / "docs" / "y.md").write_text("y", encoding="utf-8")
        (self.root / "rfcs" / "done").mkdir(parents=True)
        (self.root / "rfcs" / "done" / "y.md").write_text("y", encoding="utf-8")
        self.empty_root = Path(self._tmp.name) / "empty"
        self.empty_root.mkdir()

    def test_a_repo_relative_target_is_rewritten_to_a_blob_url(self) -> None:
        out = ecs.rewrite_line(
            "see [docs](docs/x.md) and [rfc](rfcs/done/y.md).", "o/r", "v1", self.root
        )
        self.assertIn("[docs](https://github.com/o/r/blob/v1/docs/x.md)", out)
        self.assertIn("[rfc](https://github.com/o/r/blob/v1/rfcs/done/y.md)", out)

    def test_a_target_with_a_fragment_is_checked_by_its_file_and_keeps_the_fragment(self) -> None:
        out = ecs.rewrite_line("[s](docs/x.md#a-section)", "o/r", "v1", self.root)
        self.assertEqual(out, "[s](https://github.com/o/r/blob/v1/docs/x.md#a-section)")

    def test_a_fragment_does_not_excuse_a_missing_file(self) -> None:
        with self.assertRaises(ecs.RefuseError):
            ecs.rewrite_line("[s](docs/missing.md#a-section)", "o/r", "v1", self.root)

    def test_a_missing_target_is_refused(self) -> None:
        with self.assertRaises(ecs.RefuseError):
            ecs.rewrite_line("[x](docs/missing.md)", "o/r", "v1", self.root)

    def test_a_url_scheme_target_is_never_checked_against_the_filesystem(self) -> None:
        # `self.empty_root` has nothing under it at all -- if the
        # existence check ran on this target, it would refuse.
        out = ecs.rewrite_line("[x](https://example.com)", "o/r", "v1", self.empty_root)
        self.assertEqual(out, "[x](https://example.com)")

    def test_a_mailto_target_is_never_checked_against_the_filesystem(self) -> None:
        out = ecs.rewrite_line("[x](mailto:a@example.com)", "o/r", "v1", self.empty_root)
        self.assertEqual(out, "[x](mailto:a@example.com)")

    def test_a_fragment_only_target_is_never_checked_against_the_filesystem(self) -> None:
        out = ecs.rewrite_line("[x](#section)", "o/r", "v1", self.empty_root)
        self.assertEqual(out, "[x](#section)")

    def test_an_absolute_path_target_is_never_checked_against_the_filesystem(self) -> None:
        out = ecs.rewrite_line("[x](/abs/path.md)", "o/r", "v1", self.empty_root)
        self.assertEqual(out, "[x](/abs/path.md)")

    def test_a_dotdot_target_is_refused_before_any_existence_check(self) -> None:
        # Refused on the `..` rule alone, even against `self.empty_root`
        # where an existence check would refuse it anyway -- this proves
        # which rule actually fired.
        with self.assertRaises(ecs.RefuseError) as ctx:
            ecs.rewrite_line("[x](../escape.md)", "o/r", "v1", self.empty_root)
        self.assertIn("..", str(ctx.exception))

    def test_a_link_inside_a_code_span_is_untouched(self) -> None:
        # `docs/missing.md` does not exist under `self.root` either --
        # proving the span exclusion runs before any existence check,
        # not merely that an existing target happens to survive.
        out = ecs.rewrite_line(
            "text `[x](docs/missing.md)` more text", "o/r", "v1", self.root
        )
        self.assertEqual(out, "text `[x](docs/missing.md)` more text")

    def test_a_link_inside_a_fenced_block_is_untouched(self) -> None:
        lines = ["```toml", "# [x](docs/missing.md)", "```"]
        out = ecs.rewrite_links(lines, "o/r", "v1", self.root)
        self.assertEqual(out, lines)

    def test_a_link_inside_an_indented_fenced_block_is_untouched(self) -> None:
        # The shape CHANGELOG.md really has: a fence opened inside a list
        # item, indented to the item's content column.
        lines = ["1. In `Cargo.toml`:", "", "   ```toml", "   # [x](docs/missing.md)", "   ```"]
        out = ecs.rewrite_links(lines, "o/r", "v1", self.root)
        self.assertEqual(out, lines)

    def test_link_text_containing_a_code_span_survives(self) -> None:
        # The link's *text* may itself be a code span -- only the
        # target's own rewriting is at stake, the bracket/paren
        # structure around it is untouched.
        out = ecs.rewrite_line("[`ROADMAP.md`](docs/x.md)", "o/r", "v1", self.root)
        self.assertEqual(
            out, "[`ROADMAP.md`](https://github.com/o/r/blob/v1/docs/x.md)"
        )


class MainBehaviour(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.changelog = self.root / "CHANGELOG.md"
        self.changelog.write_text(SAMPLE, encoding="utf-8")
        # SAMPLE's own links all target docs/x.md -- present here, so a
        # root-driven refusal in these tests is never the real target's
        # existence, only the thing each test itself sets up.
        (self.root / "docs").mkdir()
        (self.root / "docs" / "x.md").write_text("x", encoding="utf-8")

    def run_main(self, version: str) -> tuple[int, str]:
        import io
        import contextlib

        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = ecs.main(
                [
                    "--changelog", str(self.changelog),
                    "--version", version,
                    "--repo", "o/r",
                    "--ref", "v1",
                    "--root", str(self.root),
                ]
            )
        return code, out.getvalue()

    def test_a_missing_version_exits_one(self) -> None:
        code, _ = self.run_main("9.9.9")
        self.assertEqual(code, 1)

    def test_an_empty_section_exits_one(self) -> None:
        sample = self.changelog.read_text(encoding="utf-8").replace(
            "## [1.1.0] — 2026-01-01\n\n- older release.\n",
            "## [1.1.0] — 2026-01-01\n\n",
        )
        self.changelog.write_text(sample, encoding="utf-8")
        code, _ = self.run_main("1.1.0")
        self.assertEqual(code, 1)

    def test_a_present_nonempty_section_exits_zero_with_rewritten_links(self) -> None:
        code, body = self.run_main("1.2.0")
        self.assertEqual(code, 0)
        self.assertIn("https://github.com/o/r/blob/v1/docs/x.md", body)
        self.assertIn("https://example.com", body)
        self.assertNotIn("## [1.2.0]", body)

    def test_a_dotdot_target_exits_one(self) -> None:
        sample = self.changelog.read_text(encoding="utf-8").replace(
            "[root path](/abs)", "[escape](../x.md)"
        )
        self.changelog.write_text(sample, encoding="utf-8")
        code, _ = self.run_main("1.2.0")
        self.assertEqual(code, 1)

    def test_a_target_missing_under_root_exits_one(self) -> None:
        sample = self.changelog.read_text(encoding="utf-8").replace(
            "[root path](/abs)", "[gone](docs/missing.md)"
        )
        self.changelog.write_text(sample, encoding="utf-8")
        code, _ = self.run_main("1.2.0")
        self.assertEqual(code, 1)


class RealChangelog(unittest.TestCase):
    """Against the real CHANGELOG.md: a positive integration check that
    does not depend on the sample fixture above."""

    def test_the_0_88_0_section_is_extracted_verbatim_apart_from_links(self) -> None:
        changelog = REPO_ROOT / "CHANGELOG.md"
        code = None
        import io
        import contextlib

        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = ecs.main(
                [
                    "--changelog", str(changelog),
                    "--version", "0.88.0",
                    "--repo", "o/r",
                    "--ref", "v1",
                    "--root", str(REPO_ROOT),
                ]
            )
        self.assertEqual(code, 0)
        body_lines = out.getvalue().splitlines()
        self.assertTrue(
            body_lines[0].startswith("**Upgrading is now previewable"),
            f"expected the bold summary first, got: {body_lines[0]!r}",
        )
        # The 0.88.0 section has no repo-relative links, so every line
        # must equal the real file's own text exactly.
        real_lines = changelog.read_text(encoding="utf-8").splitlines()
        start = real_lines.index("## [0.88.0] — 2026-11-08") + 1
        end = start
        while end < len(real_lines) and not real_lines[end].startswith("## "):
            end += 1
        expected = ecs.trim_blank(real_lines[start:end])
        self.assertEqual(body_lines, expected)


if __name__ == "__main__":
    unittest.main()
