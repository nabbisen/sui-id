#!/usr/bin/env python3.14
"""RFC 134 D4 (G19): federation's one egress client, enforced structurally.

A textual scan over the federation path and the egress module, asserting
three things the security review found as ways to bypass the egress
client's policy without constructing a second client:

  1. `reqwest::Client::builder()` (or a bare `Client::builder()` reached
     through a `use reqwest::Client;` import) appears nowhere under the
     federation path except the egress module itself.
  2. `resolve(` and `resolve_to_addrs(` appear nowhere in the whole
     workspace -- not only the federation path. `reqwest` documents that a
     per-name DNS override applies *on top of* a custom `dns_resolver`, so
     one such call pins a hostname to a chosen address and a future
     resolver policy would never run for that name.
  3. no per-request `.timeout(` call on the federation path outside the
     egress module. `RequestBuilder::timeout` overrides the client's,
     silently making the client's own bound dead code on that request.

**This is a textual scan, a proxy, not a proof.** It catches each
construct spelled the obvious way; a call reached through a type alias, a
re-exported name, or a macro is not guaranteed to be caught. It proves
these bypasses are not present as written, not that they are impossible.

Exit 0 = pass. Exit 1 = any violation, each on stderr, prefixed
`check-federation-egress:`. Exit 2 = a scanned path could not be read.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# The federation path: the handler, the egress module it must be the only
# user of, and the two repos a federation flow reads/writes. A new
# federation-touching file should be added here -- this list is what
# makes this gate "a handful of files", not a whole-crate scan.
FEDERATION_PATH = (
    "crates/sui-id/src/http/handlers/federation.rs",
    "crates/sui-id/src/runtime/egress.rs",
    "crates/sui-id-store/src/repos/federation_link.rs",
    "crates/sui-id-store/src/repos/federation_provider.rs",
)
EGRESS_MODULE = "crates/sui-id/src/runtime/egress.rs"

BUILDER_RE = re.compile(r"\bClient::builder\s*\(\s*\)")
RESOLVE_RE = re.compile(r"\.resolve(?:_to_addrs)?\s*\(")
TIMEOUT_RE = re.compile(r"\.timeout\s*\(")
COMMENT_OR_STRING_RE = re.compile(r"//.*$|\"(?:[^\"\\]|\\.)*\"")


def strip_noise(line: str) -> str:
    """Comments and string literals blanked, so a mention of these
    patterns in prose (this script's own docstring, a doc comment
    describing the gate) cannot itself trigger a finding."""
    return COMMENT_OR_STRING_RE.sub("", line)


def scan_file(path: Path, pattern: re.Pattern, failures: list[str], label: str) -> None:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"check-federation-egress: cannot read {path}: {exc}", file=sys.stderr)
        raise SystemExit(2)
    for lineno, line in enumerate(text.splitlines(), start=1):
        if pattern.search(strip_noise(line)):
            failures.append(f"{path}:{lineno}: {label}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    args = parser.parse_args(argv)
    root = Path(args.root)

    failures: list[str] = []

    # Condition 1: the builder appears nowhere on the federation path
    # except the egress module.
    for rel in FEDERATION_PATH:
        if rel == EGRESS_MODULE:
            continue
        path = root / rel
        if not path.is_file():
            continue
        scan_file(
            path,
            BUILDER_RE,
            failures,
            "reqwest::Client::builder() outside the egress module (condition 1)",
        )

    # Condition 2: resolve()/resolve_to_addrs() appear nowhere at all --
    # including inside the egress module, so every .rs file is in scope.
    for path in sorted((root / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        scan_file(
            path,
            RESOLVE_RE,
            failures,
            "resolve(/resolve_to_addrs( call (condition 2) -- a per-name DNS "
            "override bypasses any future resolver policy for that name",
        )

    # Condition 3: no per-request .timeout( on the federation path
    # outside the egress module (where it sets the client's own bound).
    for rel in FEDERATION_PATH:
        if rel == EGRESS_MODULE:
            continue
        path = root / rel
        if not path.is_file():
            continue
        scan_file(
            path,
            TIMEOUT_RE,
            failures,
            "per-request .timeout( on the federation path (condition 3) -- "
            "RequestBuilder::timeout overrides the client's own bound",
        )

    if failures:
        for line in failures:
            print(f"check-federation-egress: {line}", file=sys.stderr)
        print(
            f"check-federation-egress: failed with {len(failures)} violation(s)",
            file=sys.stderr,
        )
        return 1

    print("check-federation-egress: all conditions satisfied")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
