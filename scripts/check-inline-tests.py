#!/usr/bin/env python3.14
"""RFC 137 D1 (G21): test modules live beside the module, not inside it.

A file under `crates/*/src/` must not declare its test module inline. The
construct flagged is an attribute `#[cfg(test)]`, optionally followed by other
attributes or comments, then `mod <name> {` with a body. The conforming form
is `#[cfg(test)] mod <name>;`, with the body in a sibling file.

The name of the module is not matched: `mod tests`, `mod lockout_tests` and
`mod redirect_uri_matches_tests` are the same construct.

Exemptions are a shrink-only list in `contracts/inline-test-exemptions.toml`.
Three things fail the gate:

  1. a file declares an inline test module and is not exempted;
  2. an exempted file no longer declares one (a stale exemption);
  3. an exempted path does not exist.

Each migration removes one line from the list, in the same commit.

**This is a textual scan, a proxy, not a proof.** It catches the construct as
written, with the attribute on the line directly above the `mod` line (other
attributes and comments may sit between them). A test module declared through
a macro, a `#[path]` pointing at a file that itself holds an inline module, or
a `cfg` predicate other than the bare `test` is not guaranteed to be caught.
It proves the construct is absent as written, not impossible.

Exit 0 = pass. Exit 1 = any violation, each on stderr, prefixed
`check-inline-tests:`. Exit 2 = the exemption list or a scanned file could not
be read or parsed.
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

CFG_TEST_RE = re.compile(r"^\s*#\[cfg\(test\)\]\s*$")
# Lines that may sit between the attribute and the `mod` line.
BETWEEN_RE = re.compile(r"^\s*(#\[|//|$)")
INLINE_MOD_RE = re.compile(r"^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*\{")

EXEMPTIONS = "contracts/inline-test-exemptions.toml"


def declares_inline_test_module(lines: list[str]) -> bool:
    for i, line in enumerate(lines):
        if not CFG_TEST_RE.match(line):
            continue
        j = i + 1
        while j < len(lines) and BETWEEN_RE.match(lines[j]):
            j += 1
        if j < len(lines) and INLINE_MOD_RE.match(lines[j]):
            return True
    return False


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    args = parser.parse_args(argv)
    root = Path(args.root)

    try:
        exempt_list = tomllib.loads((root / EXEMPTIONS).read_text(encoding="utf-8"))
        exempt = exempt_list["files"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as err:
        print(f"check-inline-tests: cannot read {EXEMPTIONS}: {err}", file=sys.stderr)
        return 2
    if exempt != sorted(set(exempt)):
        print(
            f"check-inline-tests: {EXEMPTIONS} must be sorted and free of duplicates",
            file=sys.stderr,
        )
        return 1
    exempt_set = set(exempt)

    violations: list[str] = []
    inline_now: set[str] = set()
    for path in sorted(root.glob("crates/*/src/**/*.rs")):
        rel = path.relative_to(root).as_posix()
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeDecodeError) as err:
            print(f"check-inline-tests: cannot read {rel}: {err}", file=sys.stderr)
            return 2
        if declares_inline_test_module(lines):
            inline_now.add(rel)
            if rel not in exempt_set:
                violations.append(f"{rel}: declares its test module inline; move it to a sibling file")

    for rel in sorted(exempt_set):
        if not (root / rel).is_file():
            violations.append(f"{rel}: listed in {EXEMPTIONS} but does not exist")
        elif rel not in inline_now:
            violations.append(f"{rel}: listed in {EXEMPTIONS} but no longer declares an inline test module; remove the line")

    for line in violations:
        print(f"check-inline-tests: {line}", file=sys.stderr)
    if violations:
        return 1
    print(f"check-inline-tests: {len(inline_now)} exempted inline test module(s); no new ones")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
