"""RFC 143 stage 5 part D negative self-tests for
scripts/check-openssl-freshness.py.

Run as: python3.14 -m unittest scripts.tests.test_check_openssl_freshness

No network: `fetch_tags`/`fetch_series_table` are mocked throughout, the
same shape `check-publish-preconditions.py`'s own network functions
already use. The HTML fixture below is a trimmed, byte-faithful copy of
the real table's shape at https://openssl-library.org/source/ (fetched
2026-11-09), not a hand-simplified approximation -- the row regex has to
survive the real whitespace and nesting, not a cleaner stand-in for it.
"""

from __future__ import annotations

import importlib.util
import unittest
from datetime import date, timedelta
from unittest import mock

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT_PATH = REPO_ROOT / "scripts" / "check-openssl-freshness.py"

spec = importlib.util.spec_from_file_location("check_openssl_freshness", SCRIPT_PATH)
cof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cof)  # type: ignore[union-attr]


def ls_remote_line(tag: str) -> str:
    return f"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\trefs/tags/{tag}"


TAGS_FIXTURE = [
    ls_remote_line(t)
    for t in (
        "openssl-3.5.0",
        "openssl-3.5.0-alpha1",
        "openssl-3.5.5",
        "openssl-3.5.8",
        "openssl-3.5.9",
        "openssl-3.6.3",
        "openssl-3.6.4",
        "openssl-3.6.5",
        "openssl-4.0.3",
    )
]
# A peeled annotated-tag line: same name, `^{}` suffix, must not be
# double-counted or mistaken for a distinct patch.
TAGS_FIXTURE.append(ls_remote_line("openssl-3.5.9") + "^{}")

# Trimmed, real shape (fetched 2026-11-09): the thead is irrelevant to
# the row regex (it only matches `<tr>` bodies), so it is omitted here;
# the row structure -- six `<td>`s, the second holding a nested `<a>`,
# whitespace and blank lines between cells exactly as the real page has
# them -- is kept faithfully.
SOURCE_PAGE_FIXTURE = """
<table>
  <tbody>
      <tr>
        <td>4.1</td>
        <td><a href="https://example/openssl-4.1.0.tar.gz">openssl-4.1.0.tar.gz</a></td>
        <td>53.2MiB</td>
        <td>23 Oct 2026</td>
        <td>23 Nov 2027</td>
        <td class="text-base"></td>
      </tr>
      <tr>
        <td>3.5 [LTS]</td>
        <td><a href="https://example/openssl-3.5.9.tar.gz">openssl-3.5.9.tar.gz</a></td>
        <td>50.8MiB</td>
        <td>29 Sep 2026</td>
        <td>08 Apr 2030</td>
        <td class="text-base"></td>
      </tr>
  </tbody>
</table>
"""

NO_TABLE_FIXTURE = "<html><body><p>Sorry, this page moved.</p></body></html>"


class TagParsing(unittest.TestCase):
    def test_patches_in_series_excludes_other_series_and_prereleases(self) -> None:
        patches = cof.patches_in_series(
            [l.split("\t")[1][len("refs/tags/") :] for l in TAGS_FIXTURE if "\t" in l],
            "3.5",
        )
        self.assertEqual(patches, [0, 5, 8, 9])

    def test_a_peeled_tag_line_is_not_double_counted(self) -> None:
        # Mirrors `fetch_tags`'s own line parsing, since that function
        # itself calls `git` directly and has nothing to unit-test
        # against without a real subprocess.
        names = []
        for line in TAGS_FIXTURE:
            parts = line.split("\t")
            if len(parts) != 2 or not parts[1].startswith("refs/tags/"):
                continue
            name = parts[1][len("refs/tags/") :]
            if name.endswith("^{}"):
                continue
            names.append(name)
        self.assertEqual(names.count("openssl-3.5.9"), 1)


class SeriesTableParsing(unittest.TestCase):
    def test_parses_series_stripping_the_lts_marker(self) -> None:
        table = cof.parse_series_table(SOURCE_PAGE_FIXTURE)
        self.assertEqual(table["3.5"], date(2030, 4, 8))
        self.assertEqual(table["4.1"], date(2027, 11, 23))

    def test_a_page_with_no_rows_parses_to_an_empty_dict(self) -> None:
        table = cof.parse_series_table(NO_TABLE_FIXTURE)
        self.assertEqual(table, {})


class ContractConsistency(unittest.TestCase):
    def test_a_version_outside_the_series_is_refused(self) -> None:
        # The hole this guards: 3.6.9 under series 3.5 used to be judged
        # by 3.5's facts and reported "current".
        errors = cof.contract_errors("3.6.9", "3.5", [0, 5, 8, 9])
        self.assertTrue(any("is not in series 3.5" in e for e in errors), errors)

    def test_an_unpublished_patch_is_refused(self) -> None:
        errors = cof.contract_errors("3.5.10", "3.5", [0, 5, 8, 9])
        self.assertTrue(any("not a published upstream release" in e for e in errors), errors)

    def test_a_malformed_version_is_refused(self) -> None:
        self.assertTrue(cof.contract_errors("3.5", "3.5", [9]))

    def test_a_published_version_in_its_series_is_accepted(self) -> None:
        self.assertEqual(cof.contract_errors("3.5.9", "3.5", [0, 5, 8, 9]), [])


class Evaluate(unittest.TestCase):
    def setUp(self) -> None:
        self.table = cof.parse_series_table(SOURCE_PAGE_FIXTURE)

    def test_current_is_not_behind_and_not_dropped(self) -> None:
        result = cof.evaluate("3.5.9", "3.5", [0, 5, 8, 9], self.table, date(2026, 11, 9))
        self.assertFalse(result.behind)
        self.assertFalse(result.series_dropped)

    def test_behind_the_newest_published_patch(self) -> None:
        result = cof.evaluate("3.5.5", "3.5", [0, 5, 8, 9], self.table, date(2026, 11, 9))
        self.assertTrue(result.behind)
        self.assertEqual(result.newest_patch, 9)

    def test_a_series_dropped_from_the_list_is_flagged(self) -> None:
        # 3.6 has patches upstream but no row in the (fixture) table at all.
        result = cof.evaluate("3.6.3", "3.6", [3, 4, 5], self.table, date(2026, 11, 9))
        self.assertTrue(result.series_dropped)

    def test_eol_within_180_days_warns(self) -> None:
        # 3.5's EOL is 2030-04-08; 170 days before that is within the window.
        near = date(2030, 4, 8) - timedelta(days=170)
        result = cof.evaluate("3.5.9", "3.5", [9], self.table, near)
        self.assertTrue(result.eol_warning)

    def test_eol_beyond_180_days_does_not_warn(self) -> None:
        far = date(2026, 11, 9)  # 3.5's EOL (2030-04-08) is years away
        result = cof.evaluate("3.5.9", "3.5", [9], self.table, far)
        self.assertFalse(result.eol_warning)


class MainBehaviour(unittest.TestCase):
    def setUp(self) -> None:
        import tempfile

        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.contract = Path(self._tmp.name) / "openssl.toml"

    def write_contract(self, series: str, version: str) -> None:
        self.contract.write_text(
            f'version = 1\nseries = "{series}"\nopenssl_version = "{version}"\n'
            f'sha256 = "deadbeef"\nurl_template = "https://example.com/{{version}}"\n',
            encoding="utf-8",
        )

    def run_main(self) -> int:
        with mock.patch.object(cof, "fetch_tags", return_value=[
            l.split("\t")[1][len("refs/tags/") :] for l in TAGS_FIXTURE if "\t" in l
        ]), mock.patch.object(cof, "fetch_series_table", return_value=SOURCE_PAGE_FIXTURE):
            return cof.main(["--contract", str(self.contract)])

    def test_current_exits_zero(self) -> None:
        self.write_contract("3.5", "3.5.9")
        self.assertEqual(self.run_main(), 0)

    def test_behind_exits_one(self) -> None:
        self.write_contract("3.5", "3.5.5")
        self.assertEqual(self.run_main(), 1)

    def test_dropped_series_exits_one(self) -> None:
        self.write_contract("3.6", "3.6.3")
        self.assertEqual(self.run_main(), 1)

    def test_no_tag_in_series_exits_two(self) -> None:
        self.write_contract("9.9", "9.9.0")
        with mock.patch.object(cof, "fetch_tags", return_value=[
            l.split("\t")[1][len("refs/tags/") :] for l in TAGS_FIXTURE if "\t" in l
        ]), mock.patch.object(cof, "fetch_series_table", return_value=SOURCE_PAGE_FIXTURE):
            code = cof.main(["--contract", str(self.contract)])
        self.assertEqual(code, 2)

    def test_an_unparseable_page_exits_two(self) -> None:
        self.write_contract("3.5", "3.5.9")
        with mock.patch.object(cof, "fetch_tags", return_value=[
            l.split("\t")[1][len("refs/tags/") :] for l in TAGS_FIXTURE if "\t" in l
        ]), mock.patch.object(cof, "fetch_series_table", return_value=NO_TABLE_FIXTURE):
            code = cof.main(["--contract", str(self.contract)])
        self.assertEqual(code, 2)

    def test_a_network_error_exits_two(self) -> None:
        self.write_contract("3.5", "3.5.9")
        with mock.patch.object(
            cof, "fetch_tags", side_effect=cof.UnreadableSourceError("boom")
        ):
            code = cof.main(["--contract", str(self.contract)])
        self.assertEqual(code, 2)


if __name__ == "__main__":
    unittest.main()
