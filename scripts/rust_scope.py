"""RFC 130 D1/D3/D8: the shared Rust scope, read once, and the one match
rule applied to it.

`contracts/gate-inputs.toml`'s `[lane_profiles]` declares a `paths` key for
every lane. G01-G09b (RFC 130 D2's scoped set) currently share one list
(D3); every other lane declares `["**"]`, meaning "always" -- a visible,
diffable statement rather than an implied default (D4). `load_rust_scope`
is the one place that reads the scoped lanes' list, so `scripts/compute-
changed-scope.py` (CI's `changes` job), `scripts/compute-local-changed-
scope.py` (cycle J stage 7d's local `--changed` chooser) and
`scripts/check-gate-inputs.sh` condition 9 (D8's completeness assertion)
can never read two different things.

`first_matching_path` is the one place both choosers decide whether a
changed path touches that scope, so there is one copy of the match rule
too, not a second `fnmatch` call each carries independently.
"""

from __future__ import annotations

import fnmatch
import tomllib
from pathlib import Path
from typing import Sequence

SCOPED_LANES = (
    "G01",
    "G02",
    "G03",
    "G04",
    "G05",
    "G06",
    "G07",
    "G07b",
    "G08",
    "G09a",
    "G09b",
)

ALWAYS = ("**",)


def load_rust_scope(policy_path: Path) -> tuple[str, ...]:
    """Return the one Rust-scope path list shared by every scoped lane.

    Raises ValueError, naming the problem, if a scoped lane has no `paths`,
    if a scoped lane declares `["**"]` (that means "always", which a lane
    meant to be scoped must not say), or if the scoped lanes do not all
    declare the same list. RFC 130 D4: a missing or inconsistent
    declaration must be loud, never an implied default.
    """
    with open(policy_path, "rb") as handle:
        manifest = tomllib.load(handle)
    profiles = manifest.get("lane_profiles", {})
    scopes: dict[str, tuple[str, ...]] = {}
    for lane in SCOPED_LANES:
        prof = profiles.get(lane, {})
        paths = prof.get("paths")
        if not paths:
            raise ValueError(f"lane {lane} has no `paths` declared in [lane_profiles]")
        paths = tuple(paths)
        if paths == ALWAYS:
            raise ValueError(
                f"lane {lane} is in RFC 130 D2's scoped set but declares paths = [\"**\"] "
                "(always) -- only an unscoped governance gate may say that"
            )
        scopes[lane] = paths
    distinct = set(scopes.values())
    if len(distinct) != 1:
        detail = "; ".join(f"{lane}={list(paths)}" for lane, paths in sorted(scopes.items()))
        raise ValueError(f"scoped lanes declare different `paths` lists: {detail}")
    return next(iter(distinct))


def first_matching_path(changed: Sequence[str], scope: Sequence[str]) -> str | None:
    """The first path in `changed` (in the given order) that matches any
    glob in `scope`, or `None` if none does.

    The one copy of the match rule itself (RFC 130 D1/D3, cycle J stage
    7d): `scripts/compute-changed-scope.py` (CI's `changes` job) and
    `scripts/compute-local-changed-scope.py` (the local `--changed`
    chooser) both call this rather than each repeating the `fnmatch`
    call, so there is one place that decides what "touches the Rust
    scope" means.
    """
    for path in changed:
        if any(fnmatch.fnmatch(path, pattern) for pattern in scope):
            return path
    return None
