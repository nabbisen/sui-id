#!/usr/bin/env python3.14
"""RFC 116 stage 2 (D3, D3b): the audit matrix's `class` and `actor` columns,
and its `step_up` marker, are checked against the code.

`scripts/check-audit-matrix.sh` (G13) compares event-name *strings*. This is
the other half of the same lane, run by that script as its last step: it holds
the columns a reader trusts to the sealed descriptors in `crates/`.

The parse reads **table rows**, not names. G13's extraction takes every
backticked `word.word` from anywhere in the file, so a whole row can be deleted
without it noticing whenever the name also appears in prose (measured: 24 of 56
rows). Everything here keys on the first cell of a table line under a header
whose first cell is `Event name`; a table line whose first cell names an event
under any other header is a violation (`schema`), so a new table shape cannot
silently drop its rows out of the check.

What is checked, and against what:

  (class)  A row that claims Class A names an event carried by a sealed
           descriptor of class `Atomic`; a row that claims Class B names none.
           And every sealed `Atomic` descriptor has a row that claims A: the
           reverse direction, which is what makes "every Class-A command has a
           row" true rather than hoped.
  (actor)  For a row whose event is sealed: `ActorRequirement::None` is a cell
           that starts with an em dash; `Optional` is a cell naming an actor
           *and* saying `none` (as in "admin user id; none for the CLI");
           `Required` is a cell naming an actor and no `none`.
  (step_up) For a row whose event is sealed: the note cell says `step_up`
           (required) exactly when the descriptor has a `step_up` attribute that
           is required. Exact over the five rows that carry it.

What is deliberately **not** checked, and why (RFC 116 D3): the `target` column,
which passes by construction (every cell says "user id" or "—" in prose the
descriptor's `TargetRequirement` cannot contradict), and attribute *names* in the
note column, which is free prose whose backticks yield values rather than
names. A check that is right most of the time teaches people to ignore it.

Rows whose event is not sealed are not checked for `actor`: there is no
descriptor to check them against, and saying so is better than approximating.

Exit 0 = pass, 1 = any violation (each on stderr, prefixed
`check-audit-matrix-columns:` and naming its check and the row's line), 2 = the
matrix or the source cannot be read. A tree with **no** `commands.rs` (the G13
fixture repositories are such trees) is skipped, and says so.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rust_registry  # noqa: E402

COMMANDS_RS = Path("crates/sui-id-store/src/commands.rs")

EVENT_CELL = re.compile(r"^`([a-z0-9_]+(?:\.[a-z0-9_]+)+)`$")
NAMEISH = re.compile(r"^`?[a-z0-9_]+(?:\.[a-z0-9_]+)+`?$")
STEP_UP_REQUIRED = re.compile(r"`step_up`\s*\(required\)")


@dataclass
class Row:
    line: int
    event: str
    cells: dict[str, str]  # lower-cased header -> cell text

    @property
    def klass(self) -> str:
        return self.cells.get("class", "")

    @property
    def actor(self) -> str:
        return self.cells.get("actor", "")

    @property
    def note(self) -> str:
        return self.cells.get("note fields", "")


def split_row(line: str) -> list[str]:
    """Cells of a markdown table line, honouring `\\|` and pipes inside inline
    code."""
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|") and not s.endswith("\\|"):
        s = s[:-1]
    cells, cur, in_code, i = [], [], False, 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s) and s[i + 1] == "|":
            cur.append("|")
            i += 2
            continue
        if c == "`":
            in_code = not in_code
        if c == "|" and not in_code:
            cells.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
        i += 1
    cells.append("".join(cur).strip())
    return cells


def is_separator(cells: list[str]) -> bool:
    return bool(cells) and all(re.fullmatch(r":?-{3,}:?", c) for c in cells)


def read_matrix(text: str, failures: list[str]) -> list[Row]:
    rows: list[Row] = []
    header: list[str] | None = None
    in_fence = False
    lines = text.splitlines()
    for n, line in enumerate(lines, start=1):
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            header = None
            continue
        if in_fence or not line.lstrip().startswith("|"):
            header = None
            continue
        cells = split_row(line)
        if header is None:
            # The first table line is the header; the next must be the rule.
            nxt = split_row(lines[n]) if n < len(lines) else []
            if is_separator(nxt):
                header = [c.strip().lower() for c in cells]
            continue
        if is_separator(cells):
            continue
        first = cells[0] if cells else ""
        m = EVENT_CELL.match(first)
        if header[0] == "event name":
            if not m:
                failures.append(
                    f"(schema) line {n}: the Event name cell is not a single backticked "
                    f"event name: {first!r}"
                )
                continue
            if len(cells) != len(header):
                failures.append(
                    f"(schema) line {n}: {len(cells)} cells under a {len(header)}-column header"
                )
                continue
            rows.append(Row(n, m.group(1), dict(zip(header, cells))))
        elif NAMEISH.match(first):
            failures.append(
                f"(schema) line {n}: a table row whose first cell names an event ({first}) "
                f"under a header that does not start with `Event name` ({header[0]!r}); "
                "its columns cannot be checked, so it must not be there"
            )
    return rows


def claimed_class(cell: str) -> str | None:
    s = cell.replace("*", "").strip()
    if re.match(r"^A\b", s):
        return "A"
    if re.match(r"^B\b", s):
        return "B"
    return None


def actor_shape(cell: str) -> str | None:
    s = cell.strip()
    if not s:
        return None
    if s.startswith("—") or s.startswith("-"):
        return "None"
    if re.search(r"\bnone\b", s, re.IGNORECASE):
        return "Optional"
    return "Required"


def check(root: Path, matrix: Path) -> tuple[list[str], str]:
    failures: list[str] = []
    text = matrix.read_text(encoding="utf-8")
    rows = read_matrix(text, failures)

    seen: dict[str, int] = {}
    for row in rows:
        if row.event in seen:
            failures.append(
                f"(schema) line {row.line}: {row.event} has a second row (first at line {seen[row.event]})"
            )
        seen.setdefault(row.event, row.line)

    src_failures: list[str] = []
    descriptors, _commands = rust_registry.read_registry(root, src_failures)
    failures += [f"(source) {f}" for f in src_failures]
    sealed_atomic = {n for n, d in descriptors.items() if d.klass == "Atomic"}

    by_event = {r.event: r for r in rows}
    for row in rows:
        claim = claimed_class(row.klass)
        if claim is None:
            failures.append(
                f"(class) line {row.line}: {row.event}: the Class cell {row.klass!r} does not start with A or B"
            )
            continue
        if claim == "A" and row.event not in sealed_atomic:
            failures.append(
                f"(class) line {row.line}: {row.event} claims Class A but no sealed "
                "`EventDescriptor` of class `Atomic` carries that name"
            )
        if claim == "B" and row.event in sealed_atomic:
            failures.append(
                f"(class) line {row.line}: {row.event} claims Class B but a sealed "
                "Class-A descriptor carries that name"
            )
        d = descriptors.get(row.event)
        if d is None:
            continue
        want = d.actor
        got = actor_shape(row.actor)
        if got is None:
            failures.append(f"(actor) line {row.line}: {row.event}: the Actor cell is empty or absent")
        elif got != want:
            failures.append(
                f"(actor) line {row.line}: {row.event}: the descriptor says "
                f"`ActorRequirement::{want}`, the Actor cell {row.actor!r} reads as {got}"
            )
        if "note fields" in row.cells:
            in_doc = bool(STEP_UP_REQUIRED.search(row.note))
            in_code = d.attributes.get("step_up") is True
            if in_doc and not in_code:
                failures.append(
                    f"(step_up) line {row.line}: {row.event}: the note says `step_up` (required); "
                    "the descriptor has no required `step_up` attribute"
                )
            if in_code and not in_doc:
                failures.append(
                    f"(step_up) line {row.line}: {row.event}: the descriptor requires `step_up`; "
                    "the note does not say so"
                )

    for name in sorted(sealed_atomic):
        if name not in by_event:
            d = descriptors[name]
            failures.append(
                f"(class) {name} is a sealed Class-A event ({d.file}:{d.line}) with no table row "
                "claiming it; a mention in prose does not count"
            )

    summary = (
        f"{len(rows)} table rows, {len(sealed_atomic)} sealed Class-A events; "
        f"{sum(1 for r in rows if r.event in descriptors)} rows checked for actor and step_up"
    )
    return failures, summary


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--matrix", required=True)
    args = parser.parse_args(argv)

    root = Path(args.root)
    matrix = root / args.matrix
    if not (root / COMMANDS_RS).is_file():
        print(f"check-audit-matrix-columns: skipped, no {COMMANDS_RS} under {root} (a fixture tree)")
        return 0
    try:
        failures, summary = check(root, matrix)
    except OSError as err:
        print(f"check-audit-matrix-columns: cannot read {args.matrix}: {err}", file=sys.stderr)
        return 2
    if failures:
        for line in failures:
            print(f"check-audit-matrix-columns: {line}", file=sys.stderr)
        print(f"check-audit-matrix-columns: failed with {len(failures)} violation(s)", file=sys.stderr)
        return 1
    print(f"check-audit-matrix-columns: all conditions satisfied ({summary})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
