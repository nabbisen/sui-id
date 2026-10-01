#!/usr/bin/env python3.14
"""RFC 131 D4 (widened): a document may not restate a gate's command.

A document may *describe* what a gate does; it may not restate the command
in its own words, because a restatement drifts from the gate it imitates —
`.github/CONTRIBUTING.md` said `cargo fmt`, the gate runs
`cargo +stable fmt --all -- --check`, and a contributor following the
document passed locally while CI failed (G08 on `71bca90`).

The rule cannot be "no `cargo` command outside `[gates]`": a document also
needs narrow, focused commands for local iteration
(`cargo test -p sui-id-core --lib password`), and those must stay. So the
line is: a document may not state a command that *looks like* a gate's full
verification command for the whole workspace, differing only in flags. A
command scoped to one package (`-p`/`--package`) or one target
(`--lib`/`--bin <x>`/`--test <x>`/`--example <x>`) is focused local work, not
a claim about the verification bar, and is exempt. `cargo fmt` is scoped to
the whole workspace by convention even bare (no `--all` needed to read as
"the fmt command"); `clippy`/`test`/`build` must say `--workspace` explicitly
to be read as a whole-workspace claim at all.

Scans every `.md` file under `docs/` and `.github/` (not a fixed list of
three, because "any future one" is the rule, per RFC 131 D4's own text),
line by line -- both inline code spans (`` `cargo fmt` `` inside a bullet,
which is where `.github/CONTRIBUTING.md`'s and `release-process.md`'s actual
violations live) and fenced code blocks -- for a
`cargo <fmt|clippy|test|build>` invocation matching the shape above.

Exit 0 = no document restates a gate's command. Exit 1 = one or more do,
each named with its file and line. Exit 2 = an input cannot be read.
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

SCAN_ROOTS = ("docs/src", ".github")
# Not bare `docs/`: RFC 098's authority table treats `docs/` top-level files
# as dated engineering-specification/audit records (D2), distinct from the
# live, navigable product and contributor documentation under `docs/src/`
# (D1). A historical audit's prose discussing a *past* `cargo fmt` finding
# is not a live instruction to a contributor, and must not be read as one.
GATE_SUBCOMMANDS = ("fmt", "clippy", "test", "build")

# A `cargo ...` invocation anywhere on the line -- inline code span, fenced
# block, or bare -- env-var prefixes like `CARGO_BUILD_JOBS=1 cargo test ...`
# are common in these docs, so this does not anchor to line start.
CARGO_LINE_RE = re.compile(r"\bcargo\s+(\+\S+\s+)?(\S+)\b(.*)$")

NARROWING_FLAGS = re.compile(r"(^|\s)(-p\b|--package\b|--lib\b|--bin\b|--test\b|--example\b)")
# A `VAR=value cargo ...` prefix (e.g. `PROPTEST_CASES=4096 cargo test
# --workspace`) is a parameterised example for a specific purpose, not a
# claim about the verification bar -- the same kind of narrowing as `-p`.
ENV_PREFIX_RE = re.compile(r"\b[A-Z_][A-Z0-9_]*=\S+\s+cargo\b")


def iter_markdown_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for scan_root in SCAN_ROOTS:
        base = root / scan_root
        if base.exists():
            files.extend(sorted(base.rglob("*.md")))
    return files


def is_workspace_claim(subcommand: str, rest: str) -> bool:
    if NARROWING_FLAGS.search(rest):
        return False
    if subcommand == "fmt":
        return True  # bare `cargo fmt` reads as the whole-workspace command
    return "--workspace" in rest


def load_gate_commands(policy_path: Path) -> dict[str, list[str]]:
    with open(policy_path, "rb") as handle:
        manifest = tomllib.load(handle)
    by_subcommand: dict[str, list[str]] = {s: [] for s in GATE_SUBCOMMANDS}
    for lane, cmd in manifest.get("gates", {}).items():
        for sub in GATE_SUBCOMMANDS:
            if re.search(rf"\bcargo \+\S+ {sub}\b", cmd):
                by_subcommand[sub].append(cmd)
    return by_subcommand


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--policy", required=True)
    args = parser.parse_args(argv)
    root = Path(args.root)

    try:
        gate_commands = load_gate_commands(root / args.policy)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        print(f"check-verification-commands: cannot read {args.policy}: {exc}", file=sys.stderr)
        return 2

    violations: list[str] = []
    for md in iter_markdown_files(root):
        try:
            text = md.read_text(encoding="utf-8")
        except OSError as exc:
            print(f"check-verification-commands: cannot read {md}: {exc}", file=sys.stderr)
            return 2
        rel = md.relative_to(root)
        for lineno, line in enumerate(text.splitlines(), start=1):
            m = CARGO_LINE_RE.search(line)
            if not m:
                continue
            subcommand, rest = m.group(2), m.group(3)
            if subcommand not in GATE_SUBCOMMANDS:
                continue
            if ENV_PREFIX_RE.search(line):
                continue
            if not is_workspace_claim(subcommand, rest):
                continue
            if any(g in line for g in gate_commands[subcommand]):
                continue  # the real gate command, quoted verbatim, is fine
            violations.append(
                f"{rel}:{lineno}: states a {subcommand!r} command for the whole workspace "
                f"that is not a [gates] command: {line.strip()!r} "
                f"-- point at `scripts/ci-gate.sh <GATE_ID>` instead"
            )

    if violations:
        for v in violations:
            print(f"check-verification-commands: {v}", file=sys.stderr)
        print(f"check-verification-commands: failed with {len(violations)} violation(s)", file=sys.stderr)
        return 1

    print("check-verification-commands: no document restates a gate's command")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
