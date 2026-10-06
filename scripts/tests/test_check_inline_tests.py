"""RFC 137 D1 (G21) self-tests for scripts/check-inline-tests.py.

Run as: python3.14 -m unittest scripts.tests.test_check_inline_tests

Each test builds a throwaway tree with one crate, one source file and one
exemption list, then runs the real script as a subprocess. Four behaviours are
pinned: an inline test module is detected; a sibling `mod tests;` is not
flagged; an exemption whose file no longer has an inline module is reported;
and an unexempted inline module fails the gate.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-inline-tests.py"

INLINE = """\
pub fn answer() -> u32 {
    42
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_answers() {
        assert_eq!(answer(), 42);
    }
}
"""

COMPLIANT = """\
pub fn answer() -> u32 {
    42
}

#[cfg(test)]
mod tests;
"""

# Other attributes and a plain comment may sit between the attribute and the module.
INLINE_WITH_ATTRIBUTES = """\
#[cfg(test)]
// explanation of the attribute below
#[allow(clippy::unwrap_used)]
pub mod checks_v2 {
    #[test]
    fn t() {}
}
"""

SRC = "crates/demo/src/lib.rs"


def make_tree(root: Path, source: str, exemptions: list[str]) -> None:
    src = root / SRC
    src.parent.mkdir(parents=True, exist_ok=True)
    src.write_text(source, encoding="utf-8")
    listing = "\n".join(f'  "{e}",' for e in exemptions)
    contracts = root / "contracts"
    contracts.mkdir(parents=True, exist_ok=True)
    (contracts / "inline-test-exemptions.toml").write_text(
        f"files = [\n{listing}\n]\n" if listing else "files = []\n", encoding="utf-8"
    )


def run(root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(root)],
        capture_output=True,
        text=True,
    )


class CheckInlineTests(unittest.TestCase):
    def test_an_inline_test_module_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(root, INLINE, exemptions=[])
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stderr)
            self.assertIn(f"{SRC}: declares its test module inline", r.stderr)

    def test_an_inline_module_with_intervening_attributes_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(root, INLINE_WITH_ATTRIBUTES, exemptions=[])
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stderr)
            self.assertIn(f"{SRC}: declares its test module inline", r.stderr)

    def test_a_compliant_sibling_declaration_is_not_flagged(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(root, COMPLIANT, exemptions=[])
            r = run(root)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertIn("0 exempted inline test module(s); no new ones", r.stdout)

    def test_an_exempted_inline_module_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(root, INLINE, exemptions=[SRC])
            r = run(root)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertIn("1 exempted inline test module(s); no new ones", r.stdout)

    def test_a_stale_exemption_is_reported(self) -> None:
        # The file was migrated (now a sibling declaration) but its line stayed.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(root, COMPLIANT, exemptions=[SRC])
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stderr)
            self.assertIn(
                f"{SRC}: listed in contracts/inline-test-exemptions.toml but no longer "
                "declares an inline test module",
                r.stderr,
            )

    def test_an_exemption_for_a_missing_file_is_reported(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(root, COMPLIANT, exemptions=["crates/demo/src/gone.rs"])
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stderr)
            self.assertIn("crates/demo/src/gone.rs: listed in", r.stderr)
            self.assertIn("but does not exist", r.stderr)

    def test_an_unsorted_exemption_list_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(root, INLINE, exemptions=["crates/demo/src/zz.rs", SRC])
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stderr)
            self.assertIn("must be sorted and free of duplicates", r.stderr)

    def test_a_real_tree_passes(self) -> None:
        # The committed tree must satisfy the gate it is registered under.
        r = subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(REPO_ROOT)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(r.returncode, 0, r.stderr)


if __name__ == "__main__":
    unittest.main()
