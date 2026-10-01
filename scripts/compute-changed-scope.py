#!/usr/bin/env python3.14
"""RFC 130 D6/D7: does this push touch the Rust scope?

Invoked by the generated `changes` job in `.github/workflows/ci.yml`. Prints
exactly `true` or `false` to stdout, nothing else, so the calling workflow
step can write it straight to `$GITHUB_OUTPUT`.

D7: fails open. Any condition under which the changed set is not known with
certainty prints `true` (run everything) rather than guessing: a non-`push`
event (`workflow_dispatch` above all -- RFC 130 D5/RFC 131 D2 depend on it
reliably obtaining the complete matrix), an absent or all-zero base ref
(`github.event.before` on a first push to a branch, or a force-push GitHub
cannot resolve), a base ref not present in the fetched history, or any `git`
error. Scoping is an optimisation applied only when the inputs are known
with certainty; it is never the fallback.

The Rust-scope path list itself is read through `rust_scope.py`, not
restated here -- the same module `scripts/check-gate-inputs.sh`'s condition
9 (D8) reads, so there is one copy of "what counts as Rust" rather than two
that can drift apart.
"""

from __future__ import annotations

import argparse
import fnmatch
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from rust_scope import load_rust_scope  # noqa: E402

ZERO_SHA_RE = re.compile(r"^0+$")


def fail_open(reason: str) -> None:
    print("true")
    print(f"compute-changed-scope: running the complete matrix: {reason}", file=sys.stderr)
    raise SystemExit(0)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--policy", required=True, help="path to contracts/gate-inputs.toml, relative to --root")
    parser.add_argument("--event-name", required=True)
    parser.add_argument("--before", default="", help="github.event.before; empty or all-zero means unresolvable")
    parser.add_argument("--after", required=True, help="the commit being evaluated; github.sha")
    args = parser.parse_args(argv)

    if args.event_name != "push":
        fail_open(f"non-push event ({args.event_name!r})")
    if not args.before or ZERO_SHA_RE.match(args.before):
        fail_open(
            "absent or all-zero base ref (first push on a branch, or a before "
            "GitHub could not resolve)"
        )

    root = Path(args.root)
    exists = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", args.before],
        capture_output=True,
    )
    if exists.returncode != 0:
        fail_open(f"base ref {args.before} is not present in the fetched history")

    diff = subprocess.run(
        ["git", "-C", str(root), "diff", "--name-only", args.before, args.after],
        capture_output=True,
        text=True,
    )
    if diff.returncode != 0:
        fail_open(f"git diff failed: {diff.stderr.strip()}")

    try:
        rust_scope = load_rust_scope(root / args.policy)
    except (OSError, ValueError) as exc:
        fail_open(f"could not read the declared Rust scope: {exc}")

    changed = [line for line in diff.stdout.splitlines() if line]
    rust_changed = any(
        any(fnmatch.fnmatch(path, pattern) for pattern in rust_scope) for path in changed
    )
    print("true" if rust_changed else "false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
