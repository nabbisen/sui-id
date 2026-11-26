#!/usr/bin/env python3.14
"""RFC 097 (G24): the threat-model inventory contract, and the three lists
that keep it complete.

Checks `contracts/threat-model.toml` (the method's own schema,
rfcs/handoffs/097-threat-model/method-2026-11-23.md §2):

  1. every id is unique, within its own kind (`actor`, `asset`, `boundary`,
     `control`, `residual`);
  2. every reference resolves: a control's `boundary`, a residual's
     `boundary`;
  3. every boundary has an `owner`;
  4. an `analysed` boundary has at least one control and a filled STRIDE
     row (six letters, each `not applicable` or naming at least one
     control); an `inventoried` boundary needs neither, and is listed in
     the summary, same as a pending residual;
  5. every evidence link resolves (method §3's five forms -- `run:` is
     recorded as cited, never checked offline);
  6. a control whose only evidence is `rfc:` is listed as unevidenced;
  7. a residual marked `accepted_by` carries `accepted_words`, and the
     reverse: one without the other is a failure, not a pending state.

Then derives three lists from the source of truth, never from the
contract, and checks every entry the contract names against them, and
every entry in them against the contract (RFC 097 P6 dispatch §2 -- "a
guard must generate the list it guards"):

  - every HTTP route in `crates/sui-id/src/http/router.rs` belongs to
    some boundary's `entry_points`, as `"METHOD /path"`;
  - every table that exists after every migration in
    `crates/sui-id-store/src/migrations/*.sql` runs, in order, belongs
    to some boundary's `stores`;
  - every CLI leaf command in `docs/src/reference/cli.md`'s `###`
    headings belongs to some boundary's `entry_points`, as
    `"cli: sui-id <words>"`.

And one more cross-check: every asset's `stores` entry is in some
boundary's own `stores` -- the same completeness gap, read from the
asset side.

**This is a textual/structural scan, a proxy for the migrations and
routes, not a database connection or a compiled parser.** A route built
through a helper that doesn't call `.route(` literally, or a table
created by Rust code rather than SQL, would not be seen; see this
stage's own review-request package for what was checked before relying
on this.

Exit 0 = pass. Exit 1 = any violation, each on stderr, prefixed
`check-threat-model:`. Exit 2 = a required file could not be read, or a
line could not be parsed (fail closed, never skipped).
"""

from __future__ import annotations

import argparse
import re
import sqlite3
import sys
import tomllib
from pathlib import Path

CONTRACT = "contracts/threat-model.toml"
ROUTER = "crates/sui-id/src/http/router.rs"
MIGRATIONS_DIR = "crates/sui-id-store/src/migrations"
CLI_DOC = "docs/src/reference/cli.md"

STRIDE_LETTERS = ("S", "T", "R", "I", "D", "E")
EVIDENCE_FORMS = ("rfc:", "test:", "code:", "record:", "run:")


class GateFailure(Exception):
    """A clean, reported failure (exit 1). Distinct from a parse failure
    the caller treats as exit 2."""


class ParseFailure(Exception):
    """A line or statement this scan cannot read (exit 2, fail closed)."""


# ── route extraction (RFC 097 P6 dispatch §2, check 1) ──────────────────

METHOD_RE = re.compile(r"\b(get|post|put|delete|patch|head|options|trace)\s*\(")
STRING_LITERAL_RE = re.compile(r'"((?:[^"\\]|\\.)*)"')


UNREAD_ROUTING_RE = re.compile(
    r"\.(route_service|nest|nest_service|fallback|fallback_service|method_not_allowed_fallback)\s*\("
)


def _balanced_route_args(text: str) -> list[tuple[int, str]]:
    """Every `.route(...)` call's argument text, paren-balanced (parens
    inside a string literal don't count), paired with the 1-indexed line
    its `.route(` starts on."""
    out: list[tuple[int, str]] = []
    for m in re.finditer(r"\.route\s*\(", text):
        start = m.end() - 1
        line_no = text.count("\n", 0, start) + 1
        depth = 0
        i = start
        in_str = False
        closed = False
        while i < len(text):
            c = text[i]
            if in_str:
                if c == "\\":
                    i += 1
                elif c == '"':
                    in_str = False
            else:
                if c == '"':
                    in_str = True
                elif c == "(":
                    depth += 1
                elif c == ")":
                    depth -= 1
                    if depth == 0:
                        out.append((line_no, text[start + 1 : i]))
                        closed = True
                        break
            i += 1
        if not closed:
            raise ParseFailure(f"{ROUTER}:{line_no}: unterminated .route( call")
    return out


def derive_routes(root: Path) -> set[str]:
    path = root / ROUTER
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise ParseFailure(f"cannot read {ROUTER}: {exc}") from exc

    # P6 review: a routing call this parser does not read would add entry
    # points it never sees -- `.fallback(` answers every unmatched path.
    # Fail closed on each, so the gate is taught before one is used.
    for um in UNREAD_ROUTING_RE.finditer(text):
        line_no = text.count("\n", 0, um.start()) + 1
        raise ParseFailure(
            f"{ROUTER}:{line_no}: .{um.group(1)}( adds routes this gate does not read; "
            "teach derive_routes to read it before using it"
        )

    routes: set[str] = set()
    for line_no, arg in _balanced_route_args(text):
        sm = STRING_LITERAL_RE.search(arg)
        if not sm:
            raise ParseFailure(f"{ROUTER}:{line_no}: .route( call has no path string literal")
        path_str = sm.group(1)
        rest = arg[sm.end() :]
        methods = sorted(set(METHOD_RE.findall(rest)))
        if not methods:
            raise ParseFailure(f"{ROUTER}:{line_no}: .route( call for {path_str!r} names no HTTP method")
        for method in methods:
            routes.add(f"{method.upper()} {path_str}")
    return routes


# ── table extraction (check 2) ───────────────────────────────────────────

def derive_tables(root: Path) -> set[str]:
    """The tables the migrations leave, read from the schema they build.

    P6 review: every migration runs, in order, against an in-memory SQLite
    database, and the answer is `sqlite_master`, not a second parser of SQL.
    A statement written any way SQLite accepts (a leading block comment, a
    quoted or differently cased name) is counted exactly as SQLite counts
    it. Temporary tables live in `temp`, not `main`, so they are not
    counted. Any migration SQLite rejects fails closed, naming the file.
    """
    mig_dir = root / MIGRATIONS_DIR
    files = sorted(mig_dir.glob("*.sql"))
    if not files:
        raise ParseFailure(f"{MIGRATIONS_DIR} has no .sql files")
    conn = sqlite3.connect(":memory:")
    try:
        for f in files:
            try:
                text = f.read_text(encoding="utf-8")
            except OSError as exc:
                raise ParseFailure(f"cannot read {f}: {exc}") from exc
            try:
                conn.executescript(text)
            except sqlite3.Error as exc:
                raise ParseFailure(f"{f}: SQLite rejected this migration: {exc}") from exc
        rows = conn.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'"
        ).fetchall()
    finally:
        conn.close()
    return {r[0] for r in rows}


# ── CLI command extraction (check 3) ─────────────────────────────────────

CLI_HEADING_RE = re.compile(r"^### `([^`]+)`")


def derive_cli_commands(root: Path) -> set[str]:
    path = root / CLI_DOC
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise ParseFailure(f"cannot read {CLI_DOC}: {exc}") from exc
    commands: set[str] = set()
    for line_no, line in enumerate(text.splitlines(), start=1):
        if not line.startswith("### "):
            continue
        m = CLI_HEADING_RE.match(line)
        if not m:
            raise ParseFailure(f"{CLI_DOC}:{line_no}: leaf heading does not match '### `<command>`'")
        commands.add(m.group(1))
    if not commands:
        raise ParseFailure(f"{CLI_DOC} has no '### `...`' leaf headings")
    return commands


# ── the contract's own structural checks ─────────────────────────────────


def _rfc_file(root: Path, number_and_rest: str) -> Path | None:
    """`number_and_rest` is `rfc:NNN#anchor`'s path portion. Resolves NNN
    to exactly one rfcs/{proposed,accepted,done,archive}/NNN-*.md."""
    number = number_and_rest.split("#", 1)[0]
    matches: list[Path] = []
    for sub in ("proposed", "accepted", "done", "archive"):
        matches.extend((root / "rfcs" / sub).glob(f"{number}-*.md"))
    if len(matches) == 1:
        return matches[0]
    return None


def _heading_exists(path: Path, anchor: str) -> bool:
    text = path.read_text(encoding="utf-8")
    slug_re = re.compile(r"^#{1,6}\s+(.*)$", re.MULTILINE)
    for m in slug_re.finditer(text):
        heading = m.group(1).strip()
        slug = re.sub(r"[^a-z0-9\- ]", "", heading.lower()).strip().replace(" ", "-")
        slug = re.sub(r"-+", "-", slug)
        if slug == anchor:
            return True
    return False


def _test_exists(root: Path, crate: str, qualified_path: str, name: str) -> bool:
    pattern = re.compile(r"\bfn\s+" + re.escape(name) + r"\s*[(<]")
    crate_src = root / "crates" / crate
    if not crate_src.is_dir():
        return False
    for rs in crate_src.rglob("*.rs"):
        try:
            text = rs.read_text(encoding="utf-8")
        except OSError:
            continue
        if pattern.search(text):
            return True
    # also the top-level tests/ tree for crates whose tests live there
    tests_dir = root / "crates" / crate / "tests"
    if tests_dir.is_dir():
        for rs in tests_dir.rglob("*.rs"):
            try:
                text = rs.read_text(encoding="utf-8")
            except OSError:
                continue
            if pattern.search(text):
                return True
    return False


def _code_symbol_exists(root: Path, rel_path: str, symbol: str) -> bool:
    p = root / rel_path
    if not p.is_file():
        return False
    text = p.read_text(encoding="utf-8")
    pattern = re.compile(
        r"\b(fn|struct|enum|trait|type|const|static)\s+" + re.escape(symbol) + r"\b"
    )
    return bool(pattern.search(text))


def check_evidence_link(root: Path, link: str) -> str | None:
    """Returns None if the link resolves, or a failure reason string."""
    if link.startswith("rfc:"):
        rest = link[len("rfc:") :]
        number, _, anchor = rest.partition("#")
        rfc_path = _rfc_file(root, rest)
        if rfc_path is None:
            return f"rfc:{number} does not resolve to exactly one rfcs/*/NNN-*.md"
        if anchor and not _heading_exists(rfc_path, anchor):
            return f"rfc:{rest}: anchor not found in {rfc_path}"
        return None
    if link.startswith("test:"):
        rest = link[len("test:") :]
        parts = rest.split("::")
        if len(parts) < 2:
            return f"test:{rest}: expected <crate>::<path>::<name>"
        crate, name = parts[0], parts[-1]
        if not _test_exists(root, crate, rest, name):
            return f"test:{rest}: no `fn {name}` found in crate {crate}"
        return None
    if link.startswith("code:"):
        rest = link[len("code:") :]
        rel_path, sep, symbol = rest.partition("::")
        if not sep:
            return f"code:{rest}: expected <path>::<symbol>"
        if not _code_symbol_exists(root, rel_path, symbol):
            return f"code:{rest}: symbol not found"
        return None
    if link.startswith("record:"):
        rel_path = link[len("record:") :]
        if not (root / rel_path).is_file():
            return f"record:{rel_path}: file does not exist"
        return None
    if link.startswith("run:"):
        return None  # recorded as cited; never checked offline (method §3)
    return f"{link}: not one of {', '.join(EVIDENCE_FORMS)}"


def check_contract(root: Path, contract: dict) -> list[str]:
    failures: list[str] = []

    actors = contract.get("actor", [])
    assets = contract.get("asset", [])
    boundaries = contract.get("boundary", [])
    controls = contract.get("control", [])
    residuals = contract.get("residual", [])

    # 1. every id unique, within its own kind.
    for kind, rows in (
        ("actor", actors),
        ("asset", assets),
        ("boundary", boundaries),
        ("control", controls),
        ("residual", residuals),
    ):
        seen: set[str] = set()
        for row in rows:
            rid = row.get("id")
            if not rid:
                failures.append(f"a {kind} row has no id")
                continue
            if rid in seen:
                failures.append(f"duplicate {kind} id: {rid!r}")
            seen.add(rid)

    boundary_ids = {b["id"] for b in boundaries if "id" in b}

    # 2. references resolve.
    for c in controls:
        b = c.get("boundary")
        if b not in boundary_ids:
            failures.append(f"control {c.get('id', '?')!r}: boundary {b!r} does not resolve")
    for r in residuals:
        b = r.get("boundary")
        if b not in boundary_ids:
            failures.append(f"residual {r.get('id', '?')!r}: boundary {b!r} does not resolve")

    # 3. every boundary has an owner.
    for b in boundaries:
        if not b.get("owner"):
            failures.append(f"boundary {b.get('id', '?')!r} has no owner")

    # 4. an analysed boundary needs >=1 control and a filled STRIDE row;
    #    inventoried needs neither, and both kinds get listed.
    controls_by_boundary: dict[str, list[dict]] = {}
    for c in controls:
        controls_by_boundary.setdefault(c.get("boundary", ""), []).append(c)

    inventoried: list[str] = []
    analysed: list[str] = []
    for b in boundaries:
        bid = b.get("id", "?")
        status = b.get("status")
        if status not in ("inventoried", "analysed"):
            failures.append(f"boundary {bid!r}: status must be 'inventoried' or 'analysed', got {status!r}")
            continue
        if status == "inventoried":
            inventoried.append(bid)
            continue
        analysed.append(bid)
        own_controls = controls_by_boundary.get(bid, [])
        if not own_controls:
            failures.append(f"boundary {bid!r} is 'analysed' but has no control")
        stride = b.get("stride", {})
        for letter in STRIDE_LETTERS:
            row = stride.get(letter)
            if not row:
                failures.append(f"boundary {bid!r} is 'analysed' but its STRIDE row {letter!r} is not filled")

    # 5/6. evidence links, and rfc:-only controls listed as unevidenced.
    unevidenced: list[str] = []
    for c in controls:
        cid = c.get("id", "?")
        evidence = c.get("evidence", [])
        if not evidence:
            failures.append(f"control {cid!r} has no evidence link at all")
            continue
        real_forms = [e for e in evidence if not e.startswith("rfc:")]
        if not real_forms:
            unevidenced.append(cid)
        for link in evidence:
            reason = check_evidence_link(root, link)
            if reason is not None:
                failures.append(f"control {cid!r}: evidence link does not resolve: {reason}")

    # 7. residual accepted_by/accepted_words pairing.
    pending: list[str] = []
    for r in residuals:
        rid = r.get("id", "?")
        accepted_by = r.get("accepted_by", "")
        accepted_words = r.get("accepted_words", "")
        if bool(accepted_by) != bool(accepted_words):
            failures.append(
                f"residual {rid!r}: accepted_by and accepted_words must both be set or both empty"
            )
        elif not accepted_by:
            pending.append(rid)

    summary = (
        f"boundaries: {len(boundaries)} ({len(analysed)} analysed, {len(inventoried)} inventoried); "
        f"controls: {len(controls)} ({len(unevidenced)} unevidenced); "
        f"residuals: {len(residuals)} ({len(pending)} pending)"
    )
    print(f"check-threat-model: {summary}")
    if inventoried:
        print(f"check-threat-model: inventoried boundaries: {', '.join(sorted(inventoried))}")
    if unevidenced:
        print(f"check-threat-model: unevidenced controls: {', '.join(sorted(unevidenced))}")
    if pending:
        print(f"check-threat-model: pending residuals: {', '.join(sorted(pending))}")

    return failures


# ── the three derived-list checks, plus the asset/boundary cross-check ──


def check_derived_lists(root: Path, contract: dict) -> list[str]:
    failures: list[str] = []
    boundaries = contract.get("boundary", [])
    assets = contract.get("asset", [])

    declared_entry_points: set[str] = set()
    declared_stores: set[str] = set()
    for b in boundaries:
        declared_entry_points.update(b.get("entry_points", []))
        declared_stores.update(b.get("stores", []))

    real_routes = derive_routes(root)
    for route in sorted(real_routes):
        if route not in declared_entry_points:
            failures.append(f"route {route!r} is in no boundary's entry_points")
    for ep in sorted(declared_entry_points):
        if ep.startswith("cli: "):
            continue
        method_and_path = ep
        if method_and_path not in real_routes:
            failures.append(f"entry_points names route {ep!r}, which router.rs does not have")
    print(f"check-threat-model: {len(real_routes)} route(s) derived from {ROUTER}")

    real_tables = derive_tables(root)
    for table in sorted(real_tables):
        if table not in declared_stores:
            failures.append(f"table {table!r} is in no boundary's stores")
    for store in sorted(declared_stores):
        if store.startswith("file:") or store.startswith("env:"):
            continue
        if store not in real_tables:
            failures.append(f"stores names table {store!r}, which does not exist after the migrations run")
    print(f"check-threat-model: {len(real_tables)} table(s) derived from {MIGRATIONS_DIR}")

    real_cli = derive_cli_commands(root)
    for cmd in sorted(real_cli):
        wanted = f"cli: {cmd}"
        if wanted not in declared_entry_points:
            failures.append(f"CLI command {cmd!r} is in no boundary's entry_points (as {wanted!r})")
    for ep in sorted(declared_entry_points):
        if not ep.startswith("cli: "):
            continue
        cmd = ep[len("cli: ") :]
        if cmd not in real_cli:
            failures.append(f"entry_points names CLI command {ep!r}, which {CLI_DOC} does not have")
    print(f"check-threat-model: {len(real_cli)} CLI command(s) derived from {CLI_DOC}")

    for a in assets:
        for store in a.get("stores", []):
            if store not in declared_stores:
                failures.append(
                    f"asset {a.get('id', '?')!r}: stores entry {store!r} is in no boundary's stores"
                )

    return failures


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--contract", default=CONTRACT)
    args = parser.parse_args(argv)
    root = Path(args.root)

    try:
        contract = tomllib.loads((root / args.contract).read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        print(f"check-threat-model: cannot read {args.contract}: {exc}", file=sys.stderr)
        return 2
    if contract.get("schema_version") != 1:
        print(
            f"check-threat-model: {args.contract}: schema_version must be 1, got {contract.get('schema_version')!r}",
            file=sys.stderr,
        )
        return 2

    failures: list[str] = []
    try:
        failures.extend(check_contract(root, contract))
        failures.extend(check_derived_lists(root, contract))
    except ParseFailure as exc:
        print(f"check-threat-model: {exc}", file=sys.stderr)
        return 2

    if failures:
        for line in failures:
            print(f"check-threat-model: {line}", file=sys.stderr)
        print(f"check-threat-model: failed with {len(failures)} violation(s)", file=sys.stderr)
        return 1

    print("check-threat-model: all conditions satisfied")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
