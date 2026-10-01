"""RFC 130 D1/D3/D8 negative self-tests for scripts/rust_scope.py.

Run as: python3.14 -m unittest scripts.tests.test_rust_scope
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from rust_scope import SCOPED_LANES, load_rust_scope  # noqa: E402

GOOD_SCOPE = '["crates/**", "Cargo.toml", "Cargo.lock", "rust-toolchain*", "scripts/ci-gate.sh", "contracts/gate-inputs.toml", "contracts/workflow-template.toml", ".github/workflows/ci.yml"]'


def manifest_with(paths_expr: str, missing: str | None = None) -> str:
    lines = ["version = 1", "gate_matrix_version = 1", "", "[lane_profiles]"]
    for lane in SCOPED_LANES:
        if lane == missing:
            continue
        lines.append(f"{lane} = {{ paths = {paths_expr} }}")
    return "\n".join(lines) + "\n"


class RustScope(unittest.TestCase):
    def write(self, text: str) -> Path:
        tmp = tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False, encoding="utf-8")
        self.addCleanup(lambda: Path(tmp.name).unlink(missing_ok=True))
        tmp.write(text)
        tmp.close()
        return Path(tmp.name)

    def test_the_shared_list_round_trips(self) -> None:
        path = self.write(manifest_with(GOOD_SCOPE))
        scope = load_rust_scope(path)
        self.assertEqual(list(scope), eval(GOOD_SCOPE))  # noqa: S307 -- fixture literal, not input

    def test_a_missing_lane_is_named(self) -> None:
        path = self.write(manifest_with(GOOD_SCOPE, missing="G05"))
        with self.assertRaisesRegex(ValueError, r"lane G05 has no `paths`"):
            load_rust_scope(path)

    def test_a_scoped_lane_declaring_always_is_named(self) -> None:
        text = manifest_with(GOOD_SCOPE).replace('G06 = { paths = ' + GOOD_SCOPE + ' }', 'G06 = { paths = ["**"] }')
        path = self.write(text)
        with self.assertRaisesRegex(ValueError, r"lane G06 is in RFC 130 D2's scoped set"):
            load_rust_scope(path)

    def test_disagreeing_lanes_are_named(self) -> None:
        text = manifest_with(GOOD_SCOPE).replace(
            'G09a = { paths = ' + GOOD_SCOPE + ' }', 'G09a = { paths = ["docs/**"] }'
        )
        path = self.write(text)
        with self.assertRaisesRegex(ValueError, r"scoped lanes declare different `paths` lists"):
            load_rust_scope(path)

    def test_an_unreadable_manifest_raises_oserror(self) -> None:
        with self.assertRaises(OSError):
            load_rust_scope(Path("/nonexistent/gate-inputs.toml"))


if __name__ == "__main__":
    unittest.main()
