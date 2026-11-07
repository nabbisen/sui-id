#!/usr/bin/env python3.14
"""RFC 093/RFC 130, cycle J stage 7d: `scripts/local-matrix.sh --changed`'s
chooser -- the local counterpart to `scripts/compute-changed-scope.py` (the
CI `changes` job), reusing its selection rule (`rust_scope.py`) rather than
inventing a second one.

**Output contract, matching CI's:** exactly `true` or `false` on stdout
(`true` means the Rust-scoped lanes must run; `false` means they may be
skipped), and the reason on stderr -- one line, the complete `scope: ...`
message `local-matrix.sh` prints verbatim under its banner. Exit 2 is a
usage error: an explicit `--base` that does not resolve. Every other
condition under which the changed set is not known with certainty fails
open (`true`, with the reason), the same posture `compute-changed-scope.py`
takes (RFC 130 D7) -- scoping is an optimisation applied only when the
inputs are known for certain, never the fallback.

**What "changed" means locally, and why it differs from CI's `git diff
<before> <after>`:** the union of `git diff --name-only --no-renames
<base>` (base against the *working tree*, not `HEAD` -- this is what
makes staged, unstaged and committed-unpushed edits all visible) and
`git ls-files --others --exclude-standard` (untracked, not ignored --
the same listing G18 uses; an untracked file is the trap that gave a
throwaway clone 48/48 while the real tree was 47/1).

**The cross-check (RFC 093 cycle J stage 7d §3d):** `local-matrix.sh`
derives, from `.github/workflows/ci.yml` itself, which gate ids carry
`if: needs.changes.outputs.rust == 'true'` and passes that set here as
`--ci-scoped-lanes`. If it does not equal `rust_scope.SCOPED_LANES`,
something has drifted between what CI actually conditions and what this
module believes the scoped set is -- failing open and naming the
difference is the only safe response; guessing which one is wrong is not.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from rust_scope import SCOPED_LANES, first_matching_path, load_rust_scope  # noqa: E402


def run(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(root), *args],
        capture_output=True,
        text=True,
    )


def fail_open(reason: str) -> None:
    print("true")
    print(f"scope: --changed could not determine the changed set ({reason}) — complete matrix", file=sys.stderr)
    raise SystemExit(0)


def usage_error(message: str) -> None:
    print(f"compute-local-changed-scope: {message}", file=sys.stderr)
    raise SystemExit(2)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--policy", required=True, help="path to contracts/gate-inputs.toml, relative to --root")
    parser.add_argument(
        "--ci-scoped-lanes",
        required=True,
        help="comma-separated gate ids local-matrix.sh derived from ci.yml's "
        "own `if: needs.changes.outputs.rust == 'true'` condition",
    )
    parser.add_argument("--base", default=None, help="override the default `git merge-base HEAD @{upstream}`")
    args = parser.parse_args(argv)

    root = Path(args.root)

    ci_scoped = frozenset(x for x in args.ci_scoped_lanes.split(",") if x)
    declared = frozenset(SCOPED_LANES)
    if ci_scoped != declared:
        only_ci = sorted(ci_scoped - declared)
        only_declared = sorted(declared - ci_scoped)
        detail = (
            f"ci.yml conditions {sorted(ci_scoped)} on needs.changes.outputs.rust, "
            f"rust_scope.SCOPED_LANES declares {sorted(declared)}"
            + (f"; only in ci.yml: {only_ci}" if only_ci else "")
            + (f"; only in SCOPED_LANES: {only_declared}" if only_declared else "")
        )
        fail_open(f"scoped-lane sets disagree: {detail}")

    not_repo = run(root, "rev-parse", "--is-inside-work-tree")
    if not_repo.returncode != 0 or not_repo.stdout.strip() != "true":
        fail_open(f"{root} is not a git repository")

    if args.base is not None:
        ref_label = args.base
        resolved = run(root, "rev-parse", "--verify", f"{args.base}^{{commit}}")
        if resolved.returncode != 0:
            usage_error(f"--base {args.base!r} does not resolve to a commit: {resolved.stderr.strip()}")
        base_sha = resolved.stdout.strip()
    else:
        ref_label = "@{upstream}"
        symbolic = run(root, "symbolic-ref", "-q", "HEAD")
        if symbolic.returncode != 0:
            fail_open("HEAD is detached and no --base was given")
        upstream = run(root, "rev-parse", "--verify", "@{upstream}")
        if upstream.returncode != 0:
            fail_open("no upstream is configured for the current branch and no --base was given")
        merge_base = run(root, "merge-base", "HEAD", "@{upstream}")
        if merge_base.returncode != 0:
            fail_open(f"git merge-base failed: {merge_base.stderr.strip()}")
        base_sha = merge_base.stdout.strip()

    short = run(root, "rev-parse", "--short", base_sha)
    short_sha = short.stdout.strip() if short.returncode == 0 else base_sha[:7]

    diff = run(root, "diff", "--name-only", "--no-renames", "-z", base_sha)
    if diff.returncode != 0:
        fail_open(f"git diff failed: {diff.stderr.strip()}")
    untracked = run(root, "ls-files", "--others", "--exclude-standard", "-z")
    if untracked.returncode != 0:
        fail_open(f"git ls-files failed: {untracked.stderr.strip()}")

    # `-z`: paths verbatim, NUL-terminated. Without it git quotes any path
    # holding a non-ASCII byte, a double quote, a backslash or a control
    # character (`core.quotePath`), e.g. crates/a/é.rs becomes
    # "crates/a/\303\251.rs". No scope glob matches the quoted form, so a
    # Rust change would read as no Rust change and every scoped lane would
    # be skipped: the unsafe direction RFC 130 D7 forbids.
    changed = [path for path in diff.stdout.split("\0") if path]
    changed += [path for path in untracked.stdout.split("\0") if path]

    try:
        rust_scope = load_rust_scope(root / args.policy)
    except (OSError, ValueError) as exc:
        fail_open(f"could not read the declared Rust scope: {exc}")

    match = first_matching_path(changed, rust_scope)
    prefix = f"scope: --changed against {ref_label} ({short_sha}):"
    if match is not None:
        print("true")
        print(f"{prefix} Rust scope touched (first: {match}) — complete matrix", file=sys.stderr)
    else:
        lanes = " ".join(sorted(ci_scoped))
        print("false")
        print(
            f"{prefix} {len(changed)} changed path(s); no Rust-scope path — "
            f"{len(ci_scoped)} lane(s) will be skipped: {lanes}",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
