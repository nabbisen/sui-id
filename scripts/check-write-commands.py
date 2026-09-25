#!/usr/bin/env python3.14
"""RFC 116 stage 1: the write-command inventory is checked against the code.

contracts/write-commands.toml is the one inventory of durable-write commands
(RFC 116 D1). It records *planned* conversions as well as sealed ones, so
the gate cannot ask every row to name a command (RFC 116 D2a). Each row says
which it is with `sealed`, and the directions follow from that:

  (A) code -> inventory, complete. Every command declared with
      `declare_write_command!` in a production source file under crates/ has a
      row, and that row says `sealed = true`.
  (B) inventory -> code, for sealed rows only. A row that says
      `sealed = true` names a command that is declared. A row that says
      `sealed = false` names none: a planned row whose command has since been
      sealed is out of date, and the gate says so.
  (C) `event` and `descriptor` are derived, not written. A sealed row lists
      the event names and the descriptor statics its command's events map to,
      and they must equal what the code says; a planned row carries neither.
      `--fix` rewrites them from the code.
  (D) `files`. Every path a row names exists, and every row that is not
      `target-absent` names at least one. A `target-absent` row names none.
  (E) The rows are well formed: known keys only, required keys present, an
      id of the inventory's shape, ids unique, `status` and `class` in their
      closed sets, a sealed row `implemented` and Class A.
  (F) No hand-written count. The file carries none; the counts this gate
      prints are derived from the rows on every run.

A `declare_write_command!` whose id is not of the inventory's shape is
tolerated only in registry.rs, whose proof-only command exists to give a
compile-fail test something to name and is "not an inventory row" by its own
comment. Test files are excluded by name, as G13 excludes them.

Not checked here, deliberately: the `syn` boundary and the presence of a
failure test per command, which RFC 094's `audit-structure` owns (RFC 116 D6).
This gate takes the conditions available without an AST.

Exit 0 = pass. Exit 1 = any violation, each on stderr, prefixed
`check-write-commands:`. Exit 2 = the inventory or the source cannot be read.
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

SRC_ROOT = Path("crates")
REGISTRY_RS = Path("crates/sui-id-store/src/registry.rs")

ID_SHAPE = re.compile(r"^[A-Z][0-9]{2}$")
STATUSES = ("implemented", "target-absent", "target-legacy")
CLASSES = ("A", "P", "O", "I", "X")
REQUIRED = (
    "id",
    "class",
    "status",
    "sealed",
    "owner",
    "rationale",
    "mutation_surface",
    "files",
    "test_id",
)
DERIVED = ("event", "descriptor")
ALLOWED = set(REQUIRED) | set(DERIVED)

# What a header must not say: a count is a claim a row edit falsifies. Matches
# the `(82)` after a status, and any "N rows/entries/commands" sentence.
HAND_COUNT = re.compile(
    r"\(\d+[:)]|\b\d+\s+(?:rows|entries|commands|implemented)\b", re.IGNORECASE
)


# ── reading the source ──────────────────────────────────────────────────


def blank(text: str) -> str:
    """Replace comments and string contents with spaces, keeping every
    newline and every offset, so a brace count and a line number both hold."""
    out = list(text)
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif text.startswith("/*", i):
            j = text.find("*/", i + 2)
            j = n if j < 0 else j + 2
            for k in range(i, j):
                if text[k] != "\n":
                    out[k] = " "
            i = j
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            for k in range(i + 1, min(j, n)):
                if text[k] != "\n":
                    out[k] = " "
            i = j + 1
        else:
            i += 1
    return "".join(out)


def is_test_file(path: Path) -> bool:
    name = path.name
    return (
        name == "tests.rs"
        or (name.startswith("tests_") and name.endswith(".rs"))
        or "tests" in path.parts
    )


def source_files(root: Path) -> list[Path]:
    base = root / SRC_ROOT
    if not base.is_dir():
        return []
    return sorted(p for p in base.rglob("*.rs") if not is_test_file(p.relative_to(root)))


INVOCATION = re.compile(r"\bdeclare_write_command!\s*\{")
# Patterns run over the blanked text, where every string's contents are
# spaces; the wanted literal is read back from the raw text at the same span.
COMMAND_ID = re.compile(r'\bcommand\s+\w+\s*=\s*"( *)"')
DESCRIPTOR_REF = re.compile(r"=>\s*&\s*([A-Z][A-Z0-9_]*)\b")
DESCRIPTOR_STATIC = re.compile(
    r"\bstatic\s+([A-Z][A-Z0-9_]*)\s*:\s*EventDescriptor\s*=\s*EventDescriptor\s*\{"
    r'\s*kind\s*:[^,]+,\s*name\s*:\s*"( *)"'
)


def block_end(blanked: str, open_at: int) -> int:
    depth = 0
    for i in range(open_at, len(blanked)):
        if blanked[i] == "{":
            depth += 1
        elif blanked[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    return -1


def read_commands(root: Path, failures: list[str]) -> dict[str, dict]:
    """Every declared command: id -> {file, line, events, descriptors}."""
    found: dict[str, dict] = {}
    for path in source_files(root):
        rel = path.relative_to(root)
        raw = path.read_text(encoding="utf-8")
        text = blank(raw)
        statics = {m.group(1): raw[m.start(2) : m.end(2)] for m in DESCRIPTOR_STATIC.finditer(text)}
        # A `macro_rules!` definition is not an invocation, and neither is a
        # doc comment showing one: both are gone or unmatched after `blank`.
        for m in INVOCATION.finditer(text):
            open_at = text.index("{", m.start())
            end = block_end(text, open_at)
            line = raw.count("\n", 0, m.start()) + 1
            if end < 0:
                failures.append(f"(A) {rel}:{line}: unbalanced declare_write_command! block")
                continue
            body = text[open_at : end + 1]
            ids = [raw[open_at + m2.start(1) : open_at + m2.end(1)] for m2 in COMMAND_ID.finditer(body)]
            if len(ids) != 1:
                failures.append(
                    f"(A) {rel}:{line}: expected one `command … = \"ID\"` in the block, found {len(ids)}"
                )
                continue
            cid = ids[0]
            if not ID_SHAPE.match(cid):
                if rel != REGISTRY_RS:
                    failures.append(
                        f"(A) {rel}:{line}: command id {cid!r} is not of the inventory's shape "
                        f"([A-Z][0-9][0-9]); only {REGISTRY_RS}'s proof-only command may be"
                    )
                continue
            if cid in found:
                failures.append(
                    f"(A) {rel}:{line}: command {cid} is declared twice "
                    f"(first at {found[cid]['file']}:{found[cid]['line']})"
                )
                continue
            events: list[str] = []
            descriptors: list[str] = []
            for ref in DESCRIPTOR_REF.findall(body):
                if ref not in statics:
                    failures.append(
                        f"(C) {rel}:{line}: command {cid} maps to `{ref}`, "
                        "which is not an `EventDescriptor` static in the same file"
                    )
                    continue
                if ref not in descriptors:
                    descriptors.append(ref)
                    events.append(statics[ref])
            if not descriptors:
                failures.append(f"(C) {rel}:{line}: command {cid} maps to no descriptor")
            found[cid] = {
                "file": str(rel),
                "line": line,
                "events": events,
                "descriptors": descriptors,
            }
    return found


# ── reading the inventory ───────────────────────────────────────────────


def check_shape(rows: list[dict], failures: list[str]) -> dict[str, dict]:
    by_id: dict[str, dict] = {}
    for index, row in enumerate(rows, start=1):
        rid = row.get("id", f"<row {index}>")
        where = f"(E) row {rid}"
        for key in row:
            if key not in ALLOWED:
                failures.append(f"{where}: unknown key `{key}`")
        for key in REQUIRED:
            if key not in row:
                failures.append(f"{where}: missing `{key}`")
        if "id" not in row:
            continue
        if not isinstance(rid, str) or not ID_SHAPE.match(rid):
            failures.append(f"{where}: id is not of the shape [A-Z][0-9][0-9]")
        if rid in by_id:
            failures.append(f"{where}: id appears twice")
            continue
        by_id[rid] = row
        if row.get("status") not in STATUSES:
            failures.append(f"{where}: status {row.get('status')!r} is not one of {STATUSES}")
        if row.get("class") not in CLASSES:
            failures.append(f"{where}: class {row.get('class')!r} is not one of {CLASSES}")
        if not isinstance(row.get("sealed"), bool):
            failures.append(f"{where}: `sealed` must be true or false")
        if not isinstance(row.get("files"), list) or not all(
            isinstance(f, str) for f in row.get("files", [])
        ):
            failures.append(f"{where}: `files` must be a list of paths")
        if row.get("sealed") is True:
            if row.get("status") != "implemented":
                failures.append(f"{where}: a sealed row must be `implemented`")
            if row.get("class") != "A":
                failures.append(f"{where}: a sealed command is Class A by construction")
    return by_id


def check_direction(by_id: dict[str, dict], code: dict[str, dict], failures: list[str]) -> None:
    for cid, info in sorted(code.items()):
        row = by_id.get(cid)
        if row is None:
            failures.append(
                f"(A) command {cid} is declared at {info['file']}:{info['line']} "
                "but has no row in the inventory"
            )
        elif row.get("sealed") is not True:
            failures.append(
                f"(A) command {cid} is declared at {info['file']}:{info['line']} "
                "but its row says `sealed = false`"
            )
    for rid, row in sorted(by_id.items()):
        if row.get("sealed") is True and rid not in code:
            failures.append(f"(B) row {rid} says `sealed = true` but no command {rid} is declared")


def check_derived(by_id: dict[str, dict], code: dict[str, dict], failures: list[str]) -> None:
    for rid, row in sorted(by_id.items()):
        if rid in code and row.get("sealed") is True:
            for key, derived in (("event", code[rid]["events"]), ("descriptor", code[rid]["descriptors"])):
                if key not in row:
                    failures.append(f"(C) row {rid}: `{key}` is missing; the code says {derived}")
                elif row[key] != derived:
                    failures.append(f"(C) row {rid}: `{key}` is {row[key]}; the code says {derived}")
        elif row.get("sealed") is False:
            for key in DERIVED:
                if key in row:
                    failures.append(f"(C) row {rid}: a planned row carries no `{key}`")


def check_files(root: Path, by_id: dict[str, dict], failures: list[str]) -> None:
    for rid, row in sorted(by_id.items()):
        files = row.get("files")
        if not isinstance(files, list):
            continue
        if row.get("status") == "target-absent":
            if files:
                failures.append(f"(D) row {rid}: `target-absent` names no file, but lists {files}")
            continue
        if not files:
            failures.append(f"(D) row {rid}: status {row.get('status')!r} names no file")
        for f in files:
            if not (root / f).is_file():
                failures.append(f"(D) row {rid}: `files` names {f}, which does not exist")


def check_header(text: str, failures: list[str]) -> None:
    header = []
    for line in text.splitlines():
        if line.startswith("[[") or (line and not line.startswith("#")):
            break
        header.append(line)
    for n, line in enumerate(header, start=1):
        if HAND_COUNT.search(line):
            failures.append(
                f"(F) header line {n}: a hand-written count ({line.strip()!r}); "
                "the gate derives counts on every run"
            )


# ── --fix: rewrite the derived fields from the code ─────────────────────


def toml_list(items: list[str]) -> str:
    return "[" + ", ".join('"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"' for s in items) + "]"


def fix_text(text: str, code: dict[str, dict]) -> str:
    """Rewrite `event`/`descriptor` on every sealed row and drop them from every
    planned one. Line-based, so every other byte, comment included, is kept."""
    out: list[str] = []
    block: list[str] = []

    def flush() -> None:
        if not block:
            return
        rid = next((re.match(r'id = "([^"]+)"', l).group(1) for l in block if l.startswith("id = ")), None)
        sealed = any(re.match(r"sealed = true\b", l) for l in block)
        kept = [l for l in block if not re.match(r"(event|descriptor) = ", l)]
        while kept and kept[-1] == "":
            kept.pop()
        if sealed and rid in code:
            kept += [
                f"event = {toml_list(code[rid]['events'])}",
                f"descriptor = {toml_list(code[rid]['descriptors'])}",
            ]
        out.extend(kept + [""])
        block.clear()

    seen_first = False
    for line in text.splitlines():
        if line.startswith("[[command]]"):
            flush()
            seen_first = True
            block.append(line)
        elif seen_first:
            block.append(line)
        else:
            out.append(line)
    flush()
    while out and out[-1] == "":
        out.pop()
    return "\n".join(out) + "\n"


# ── main ────────────────────────────────────────────────────────────────


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--inventory", required=True)
    parser.add_argument(
        "--fix",
        action="store_true",
        help="rewrite `event` and `descriptor` from the code, then check",
    )
    args = parser.parse_args(argv)

    root = Path(args.root)
    inventory = root / args.inventory
    try:
        text = inventory.read_text(encoding="utf-8")
        data = tomllib.loads(text)
    except (OSError, tomllib.TOMLDecodeError) as err:
        print(f"check-write-commands: cannot read {args.inventory}: {err}", file=sys.stderr)
        return 2
    rows = data.get("command")
    if not isinstance(rows, list) or not rows:
        print(f"check-write-commands: {args.inventory} has no [[command]] rows", file=sys.stderr)
        return 2

    failures: list[str] = []
    code = read_commands(root, failures)
    if not code:
        print("check-write-commands: no sealed command found under crates/ (fails closed)", file=sys.stderr)
        return 2

    if args.fix:
        fixed = fix_text(text, code)
        if fixed != text:
            inventory.write_text(fixed, encoding="utf-8")
            data = tomllib.loads(fixed)
            rows = data["command"]
            text = fixed

    by_id = check_shape(rows, failures)
    check_direction(by_id, code, failures)
    check_derived(by_id, code, failures)
    check_files(root, by_id, failures)
    check_header(text, failures)

    if failures:
        for line in failures:
            print(f"check-write-commands: {line}", file=sys.stderr)
        print(f"check-write-commands: failed with {len(failures)} violation(s)", file=sys.stderr)
        return 1

    sealed = sum(1 for r in rows if r["sealed"])
    implemented = sum(1 for r in rows if r["status"] == "implemented")
    print(
        f"check-write-commands: all conditions satisfied "
        f"({len(rows)} rows: {sealed} sealed, {len(rows) - sealed} planned; "
        f"{implemented} implemented)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
