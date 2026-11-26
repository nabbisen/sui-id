"""RFC 097 (G24) self-tests for scripts/check-threat-model.py.

Run as: python3.14 -m unittest scripts.tests.test_check_threat_model

Each test builds a throwaway tree with a minimal contract, router, migration
set and CLI reference (not the real ones, in the style of
scripts/tests/test_check_inline_tests.py), then runs the real script as a
subprocess. The real tree is checked once, at the end, the same way every
other gate's test suite pins "the committed tree satisfies the gate it is
registered under".
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CHECKER = REPO_ROOT / "scripts" / "check-threat-model.py"

ROUTER_REL = "crates/sui-id/src/http/router.rs"
MIGRATIONS_REL = "crates/sui-id-store/src/migrations"
CLI_DOC_REL = "docs/src/reference/cli.md"
CONTRACT_REL = "contracts/threat-model.toml"

BASE_ROUTER = """\
use axum::Router;
use axum::routing::{get, post};

pub fn build_router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/widgets", get(list_widgets).post(create_widget))
}
"""

BASE_CLI_DOC = """\
# CLI reference

## Getting started

### `sui-id init` {#init}

### `sui-id serve` {#serve}
"""

BASE_MIGRATION_0001 = """\
CREATE TABLE IF NOT EXISTS widgets (
    id TEXT PRIMARY KEY
);
"""

BASE_CONTRACT = textwrap.dedent(
    """\
    schema_version = 1

    [[actor]]
    id = "visitor"
    name = "Unauthenticated visitor"

    [[asset]]
    id = "widget-data"
    name = "Widget rows"
    classification = "integrity"
    stores = ["widgets"]

    [[boundary]]
    id = "web"
    name = "Browser boundary"
    owner = "RFC 1"
    status = "inventoried"
    entry_points = ["GET /", "GET /widgets", "POST /widgets", "cli: sui-id init", "cli: sui-id serve"]
    outbound = []
    stores = ["widgets"]
    """
)


def make_tree(
    root: Path,
    *,
    contract: str = BASE_CONTRACT,
    router: str = BASE_ROUTER,
    migrations: dict[str, str] | None = None,
    cli_doc: str = BASE_CLI_DOC,
) -> None:
    (root / CONTRACT_REL).parent.mkdir(parents=True, exist_ok=True)
    (root / CONTRACT_REL).write_text(contract, encoding="utf-8")
    (root / ROUTER_REL).parent.mkdir(parents=True, exist_ok=True)
    (root / ROUTER_REL).write_text(router, encoding="utf-8")
    (root / MIGRATIONS_REL).mkdir(parents=True, exist_ok=True)
    for name, sql in (migrations or {"0001_widgets.sql": BASE_MIGRATION_0001}).items():
        (root / MIGRATIONS_REL / name).write_text(sql, encoding="utf-8")
    (root / CLI_DOC_REL).parent.mkdir(parents=True, exist_ok=True)
    (root / CLI_DOC_REL).write_text(cli_doc, encoding="utf-8")


def run(root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--root", str(root)],
        capture_output=True,
        text=True,
    )


class CheckThreatModel(unittest.TestCase):
    def test_a_minimal_consistent_tree_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(root)
            r = run(root)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            self.assertIn("all conditions satisfied", r.stdout)

    # ── 1: duplicate id; a control's boundary does not resolve ──────────

    def test_a_duplicate_boundary_id_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            contract = BASE_CONTRACT + textwrap.dedent(
                """
                [[boundary]]
                id = "web"
                name = "A second boundary reusing the same id"
                owner = "RFC 1"
                status = "inventoried"
                entry_points = []
                outbound = []
                stores = []
                """
            )
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("duplicate boundary id: 'web'", r.stderr)

    def test_a_controls_boundary_that_does_not_resolve_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            contract = BASE_CONTRACT + textwrap.dedent(
                """
                [[control]]
                id = "c1"
                boundary = "no-such-boundary"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["record:README.md"]
                """
            )
            make_tree(root, contract=contract)
            (root / "README.md").write_text("x", encoding="utf-8")
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("control 'c1': boundary 'no-such-boundary' does not resolve", r.stderr)

    # ── 2: analysed vs inventoried ───────────────────────────────────────

    def test_an_analysed_boundary_without_a_control_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            contract = BASE_CONTRACT.replace('status = "inventoried"', 'status = "analysed"')
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("boundary 'web' is 'analysed' but has no control", r.stderr)

    def test_an_inventoried_boundary_passes_and_is_listed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(root)
            r = run(root)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            self.assertIn("inventoried boundaries: web", r.stdout)

    def test_an_analysed_boundary_with_a_control_and_full_stride_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            stride_inline = ", ".join(
                f'{letter} = "{"covered by c1" if letter == "S" else "not applicable, read-only"}"'
                for letter in "STRIDE"
            )
            contract = BASE_CONTRACT.replace('status = "inventoried"', 'status = "analysed"').replace(
                'stores = ["widgets"]\n',
                f'stores = ["widgets"]\nstride = {{ {stride_inline} }}\n',
            ) + textwrap.dedent(
                """
                [[control]]
                id = "c1"
                boundary = "web"
                stride = ["S"]
                claim = "widgets require a session"
                keys_on = ["session"]
                evidence = ["record:README.md"]
                """
            )
            (root / "README.md").write_text("x", encoding="utf-8")
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    # ── 3: evidence forms ────────────────────────────────────────────────

    def test_evidence_forms_each_resolve_or_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            rfcs_dir = root / "rfcs" / "accepted"
            rfcs_dir.mkdir(parents=True)
            (rfcs_dir / "1-widgets.md").write_text("# RFC 1 — Widgets\n\n## Scope\n", encoding="utf-8")
            (root / "README.md").write_text("a run record", encoding="utf-8")
            crate_src = root / "crates" / "demo" / "src"
            crate_src.mkdir(parents=True)
            (crate_src / "lib.rs").write_text(
                "pub fn validate_widget(x: u32) -> bool { x > 0 }\n"
                "#[test]\nfn a_real_test() { assert!(validate_widget(1)); }\n",
                encoding="utf-8",
            )
            contract = BASE_CONTRACT + textwrap.dedent(
                """
                [[control]]
                id = "c-rfc"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["rfc:1#scope"]

                [[control]]
                id = "c-bad-rfc"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["rfc:999#nope"]

                [[control]]
                id = "c-test"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["test:demo::lib::a_real_test"]

                [[control]]
                id = "c-bad-test"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["test:demo::lib::no_such_test"]

                [[control]]
                id = "c-code"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["code:crates/demo/src/lib.rs::validate_widget"]

                [[control]]
                id = "c-bad-code"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["code:crates/demo/src/lib.rs::no_such_symbol"]

                [[control]]
                id = "c-record"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["record:README.md"]

                [[control]]
                id = "c-bad-record"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["record:no-such-file.md"]

                [[control]]
                id = "c-run"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["run:123456"]
                """
            )
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertNotIn("c-rfc", r.stderr)
            self.assertIn("c-bad-rfc", r.stderr)
            self.assertNotIn("c-test", r.stderr)
            self.assertIn("c-bad-test", r.stderr)
            self.assertNotIn("c-code", r.stderr)
            self.assertIn("c-bad-code", r.stderr)
            self.assertNotIn("c-record", r.stderr)
            self.assertIn("c-bad-record", r.stderr)
            self.assertNotIn("c-run", r.stderr)

    def test_a_control_with_only_rfc_evidence_is_listed_as_unevidenced(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            rfcs_dir = root / "rfcs" / "accepted"
            rfcs_dir.mkdir(parents=True)
            (rfcs_dir / "1-widgets.md").write_text("# RFC 1\n", encoding="utf-8")
            contract = BASE_CONTRACT + textwrap.dedent(
                """
                [[control]]
                id = "design-only"
                boundary = "web"
                stride = ["S"]
                claim = "..."
                keys_on = ["x"]
                evidence = ["rfc:1"]
                """
            )
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            self.assertIn("unevidenced controls: design-only", r.stdout)

    # ── 4: residual accepted_by / accepted_words pairing ─────────────────

    def test_a_residual_with_accepted_by_and_no_accepted_words_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            contract = BASE_CONTRACT + textwrap.dedent(
                """
                [[residual]]
                id = "r1"
                boundary = "web"
                risk = "..."
                rating = "low"
                accepted_by = "someone"
                accepted_words = ""
                revisit_when = "never"
                """
            )
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("residual 'r1': accepted_by and accepted_words must both be set", r.stderr)

    def test_a_residual_with_accepted_words_and_no_accepted_by_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            contract = BASE_CONTRACT + textwrap.dedent(
                """
                [[residual]]
                id = "r1"
                boundary = "web"
                risk = "..."
                rating = "low"
                accepted_by = ""
                accepted_words = "Accepted."
                revisit_when = "never"
                """
            )
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("residual 'r1': accepted_by and accepted_words must both be set", r.stderr)

    def test_a_pending_residual_with_neither_passes_and_is_listed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            contract = BASE_CONTRACT + textwrap.dedent(
                """
                [[residual]]
                id = "r1"
                boundary = "web"
                risk = "..."
                rating = "low"
                accepted_by = ""
                accepted_words = ""
                revisit_when = "never"
                """
            )
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            self.assertIn("pending residuals: r1", r.stdout)

    # ── 5: route derived from router.rs, not recalled from the contract ──

    def test_a_route_added_to_the_router_and_not_the_contract_fails_naming_it(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            router = BASE_ROUTER.replace(
                '.route("/widgets", get(list_widgets).post(create_widget))',
                '.route("/widgets", get(list_widgets).post(create_widget))\n'
                '        .route("/gadgets", get(list_gadgets))',
            )
            make_tree(root, router=router)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("route 'GET /gadgets' is in no boundary's entry_points", r.stderr)

    def test_an_entry_point_naming_a_route_the_router_does_not_have_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            contract = BASE_CONTRACT.replace(
                'entry_points = ["GET /", "GET /widgets", "POST /widgets", "cli: sui-id init", "cli: sui-id serve"]',
                'entry_points = ["GET /", "GET /widgets", "POST /widgets", "GET /ghost", '
                '"cli: sui-id init", "cli: sui-id serve"]',
            )
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("entry_points names route 'GET /ghost', which router.rs does not have", r.stderr)

    # ── 6: migrations -- CREATE and DROP both handled ────────────────────

    def test_a_table_created_and_not_in_the_contract_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            migrations = {
                "0001_widgets.sql": BASE_MIGRATION_0001,
                "0002_gizmos.sql": "CREATE TABLE gizmos (id TEXT PRIMARY KEY);\n",
            }
            make_tree(root, migrations=migrations)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("table 'gizmos' is in no boundary's stores", r.stderr)

    def test_a_table_created_then_dropped_is_not_required_in_the_contract(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            migrations = {
                "0001_widgets.sql": BASE_MIGRATION_0001,
                "0002_gizmos.sql": "CREATE TABLE gizmos (id TEXT PRIMARY KEY);\nDROP TABLE gizmos;\n",
            }
            make_tree(root, migrations=migrations)
            r = run(root)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    def test_a_renamed_table_is_tracked_under_its_new_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            migrations = {
                "0001_widgets.sql": BASE_MIGRATION_0001,
                "0002_rename.sql": (
                    "CREATE TABLE _widgets_new (id TEXT PRIMARY KEY);\n"
                    "DROP TABLE widgets;\n"
                    "ALTER TABLE _widgets_new RENAME TO widgets;\n"
                ),
            }
            make_tree(root, migrations=migrations)
            r = run(root)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    # ── 7: a CLI heading added and not in the contract fails ─────────────

    def test_a_cli_heading_added_and_not_in_the_contract_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cli_doc = BASE_CLI_DOC + "\n### `sui-id widgets list` {#widgets-list}\n"
            make_tree(root, cli_doc=cli_doc)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn(
                "CLI command 'sui-id widgets list' is in no boundary's entry_points "
                "(as 'cli: sui-id widgets list')",
                r.stderr,
            )

    # ── 8: an unparseable .route( line fails, naming file and line ───────

    def test_an_unparseable_route_call_fails_naming_file_and_line(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            router = BASE_ROUTER.replace(
                '.route("/widgets", get(list_widgets).post(create_widget))',
                '.route("/widgets", get(list_widgets).post(create_widget))\n'
                "        .route(no_string_literal_here)",
            )
            make_tree(root, router=router)
            r = run(root)
            self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
            self.assertIn(f"{ROUTER_REL}:", r.stderr)
            self.assertIn("no path string literal", r.stderr)

    # ── P6 review: the table list is the schema SQLite builds ────────────

    def test_a_table_created_after_a_block_comment_is_still_counted(self) -> None:
        # A leading /* ... */ is SQL SQLite accepts, and a text parser keyed
        # on "^CREATE TABLE" would skip the statement silently.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(
                root,
                migrations={
                    "0001_widgets.sql": BASE_MIGRATION_0001,
                    "0002_sprockets.sql": "/* why sprockets */ CREATE TABLE sprockets (id TEXT PRIMARY KEY);\n",
                },
            )
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("sprockets", r.stderr)

    def test_a_migration_sqlite_rejects_fails_closed_naming_the_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_tree(
                root,
                migrations={
                    "0001_widgets.sql": BASE_MIGRATION_0001,
                    "0002_broken.sql": "CREATE TABLE (no name here);\n",
                },
            )
            r = run(root)
            self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
            self.assertIn("0002_broken.sql", r.stderr)

    # ── P6 review: a routing call the parser does not read is refused ────

    def test_a_fallback_route_is_refused_until_the_gate_reads_it(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            router = BASE_ROUTER.replace(
                '.route("/widgets", get(list_widgets).post(create_widget))',
                '.route("/widgets", get(list_widgets).post(create_widget))\n'
                "        .fallback(not_found)",
            )
            make_tree(root, router=router)
            r = run(root)
            self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
            self.assertIn(".fallback(", r.stderr)
            self.assertIn(f"{ROUTER_REL}:", r.stderr)

    # ── asset/boundary cross-check ────────────────────────────────────────

    def test_an_asset_store_not_in_any_boundary_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            contract = BASE_CONTRACT.replace(
                'stores = ["widgets"]\n\n[[boundary]]',
                'stores = ["widgets", "ghost-store"]\n\n[[boundary]]',
            )
            make_tree(root, contract=contract)
            r = run(root)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("asset 'widget-data': stores entry 'ghost-store' is in no boundary's stores", r.stderr)

    # ── the real tree ──────────────────────────────────────────────────────

    def test_a_real_tree_passes(self) -> None:
        r = subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(REPO_ROOT)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)




if __name__ == "__main__":
    unittest.main()
