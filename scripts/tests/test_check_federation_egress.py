"""RFC 134 D4 (G19) negative self-tests for scripts/check-federation-egress.py.

Run as: python3.14 -m unittest scripts.tests.test_check_federation_egress

Each fixture is a throwaway synthetic tree mirroring the real federation
path's layout, mutated one way per test. Invoked against the real script as
a subprocess, matching this project's convention elsewhere.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-federation-egress.py"

GOOD_HANDLER = """\
async fn fetch_discovery(client: &reqwest::Client, issuer: &str) -> Result<OidcDiscovery, String> {
    let resp = client.get(&url).send().await?;
    Ok(resp)
}
"""

GOOD_EGRESS = """\
pub fn build_federation_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build federation HTTP client")
}
"""


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def make_baseline(root: Path) -> None:
    write(root / "crates/sui-id/src/http/handlers/federation.rs", GOOD_HANDLER)
    write(root / "crates/sui-id/src/runtime/egress.rs", GOOD_EGRESS)
    write(root / "crates/sui-id-store/src/repos/federation_link.rs", "// nothing here\n")
    write(root / "crates/sui-id-store/src/repos/federation_provider.rs", "// nothing here\n")


def run_checker(root: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(root)],
        capture_output=True,
        text=True,
    )


class FederationEgressTest(unittest.TestCase):
    def test_valid_baseline_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("all conditions satisfied", result.stdout)

    def test_builder_outside_egress_module_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = GOOD_HANDLER + "\nfn second() { let _ = reqwest::Client::builder(); }\n"
            write(root / "crates/sui-id/src/http/handlers/federation.rs", bad)
            result = run_checker(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("condition 1", result.stderr)

    def test_a_bare_client_builder_via_use_import_is_also_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = "use reqwest::Client;\nfn second() { let _ = Client::builder(); }\n"
            write(root / "crates/sui-id/src/http/handlers/federation.rs", bad)
            result = run_checker(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("condition 1", result.stderr)

    def test_resolve_call_anywhere_in_the_tree_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            # Not even on the federation path: condition 2 is whole-tree.
            write(
                root / "crates/sui-id-core/src/somewhere_else.rs",
                'fn f() { builder.resolve("example.com", addr); }\n',
            )
            result = run_checker(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("condition 2", result.stderr)

    def test_resolve_to_addrs_call_is_also_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "crates/sui-id/src/runtime/egress.rs",
                GOOD_EGRESS + '\nfn f() { b.resolve_to_addrs("x", &addrs); }\n',
            )
            result = run_checker(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("condition 2", result.stderr)

    def test_a_free_function_named_resolve_is_not_a_false_positive(self):
        # The exact trap: this project has unrelated `resolve(...)` free
        # functions (locale resolution, session resolution) that must not
        # trip condition 2 -- only a method call (`.resolve(`) does.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "crates/sui-id-core/src/i18n.rs",
                "pub async fn resolve(db: &Database) -> Locale { todo!() }\n",
            )
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_per_request_timeout_on_the_federation_path_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            bad = GOOD_HANDLER.replace(
                "client.get(&url).send().await?;",
                "client.get(&url).timeout(std::time::Duration::from_secs(10)).send().await?;",
            )
            write(root / "crates/sui-id/src/http/handlers/federation.rs", bad)
            result = run_checker(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("condition 3", result.stderr)

    def test_the_egress_modules_own_total_timeout_is_not_a_false_positive(self):
        # GOOD_EGRESS itself calls `.timeout(` -- the client's total bound,
        # the one place this is legitimate. Already exercised by
        # test_valid_baseline_passes; this test pins the reason explicitly.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            self.assertIn(".timeout(", GOOD_EGRESS)
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_mention_in_a_comment_is_not_a_false_positive(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            write(
                root / "crates/sui-id/src/http/handlers/federation.rs",
                GOOD_HANDLER
                + "\n// do not call reqwest::Client::builder() here, "
                + "or .resolve(\"x\", a), or .timeout(d)\n",
            )
            result = run_checker(root)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_an_unreadable_path_is_exit_2(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            # A directory where the checker expects a file.
            bad_path = root / "crates/sui-id/src/http/handlers/federation.rs"
            bad_path.unlink()
            bad_path.mkdir()
            result = run_checker(root)
            self.assertEqual(result.returncode, 2)

    def test_the_repository_as_it_stands_passes(self):
        result = subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(REPO_ROOT)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
