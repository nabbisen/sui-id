"""Self-tests for scripts/wait-for-crate-download.py.

Run as: python3.14 -m unittest scripts.tests.test_wait_for_crate_download

No network: `wait` takes its probe, sleep and clock as arguments.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT_PATH = REPO_ROOT / "scripts" / "wait-for-crate-download.py"

spec = importlib.util.spec_from_file_location("wait_for_crate_download", SCRIPT_PATH)
wfcd = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wfcd)  # type: ignore[union-attr]


class FakeClock:
    def __init__(self) -> None:
        self.t = 0.0

    def now(self) -> float:
        return self.t

    def sleep(self, s: float) -> None:
        self.t += s


def run(statuses: list[int], timeout: float = 60.0, interval: float = 10.0) -> tuple[bool, list[str]]:
    seq = iter(statuses)
    seen: list[str] = []
    clock = FakeClock()

    def probe(url: str) -> int:
        seen.append(url)
        return next(seq)

    ok = wfcd.wait("https://example.invalid/x", timeout, interval, probe=probe,
                   sleep=clock.sleep, now=clock.now, log=lambda _m: None)
    return ok, seen


class TheUrlIsTheOneCargoFetches(unittest.TestCase):
    def test_exact_url_with_no_cache_busting(self) -> None:
        self.assertEqual(
            wfcd.download_url("sui-id-shared", "0.91.1"),
            "https://static.crates.io/crates/sui-id-shared/0.91.1/download",
        )

    def test_refuses_what_is_not_a_crate_or_a_release_version(self) -> None:
        for crate, version in [("sui-id-shared", "0.91"), ("sui-id-shared", "v0.91.1"),
                               ("sui-id-shared", "0.91.1; rm"), ("../x", "0.91.1"), ("", "0.91.1")]:
            with self.subTest(crate=crate, version=version):
                with self.assertRaises(ValueError):
                    wfcd.download_url(crate, version)


class ItWaitsUntilTheCdnServesTheCrate(unittest.TestCase):
    def test_a_cached_404_then_200_succeeds_after_retrying(self) -> None:
        ok, seen = run([404, 404, 200])
        self.assertTrue(ok)
        self.assertEqual(len(seen), 3)

    def test_a_range_answer_counts_as_ready(self) -> None:
        ok, _ = run([206])
        self.assertTrue(ok)

    def test_no_response_is_retried_not_taken_as_ready(self) -> None:
        ok, seen = run([0, 0, 200])
        self.assertTrue(ok)
        self.assertEqual(len(seen), 3)

    def test_it_gives_up_at_the_timeout_and_says_so(self) -> None:
        ok, seen = run([404] * 100, timeout=60.0, interval=10.0)
        self.assertFalse(ok)
        # attempts at t = 0, 10, …, 60: the deadline is checked after each probe
        self.assertEqual(len(seen), 7)

    def test_a_500_is_not_ready(self) -> None:
        ok, _ = run([500] * 100, timeout=20.0)
        self.assertFalse(ok)


class TheCommandLine(unittest.TestCase):
    def test_a_bad_version_exits_2_without_probing(self) -> None:
        self.assertEqual(wfcd.main(["--crate", "sui-id-shared", "--version", "latest"]), 2)


if __name__ == "__main__":
    unittest.main()
