"""RFC 130 D1/D3/D8: the shared Rust scope, read once.

`contracts/gate-inputs.toml`'s `[lane_profiles]` declares a `paths` key for
every lane. G01-G09b (RFC 130 D2's scoped set) currently share one list
(D3); every other lane declares `["**"]`, meaning "always" -- a visible,
diffable statement rather than an implied default (D4). This module is the
one place that reads the scoped lanes' list, so `scripts/compute-changed-
scope.py` (the CI `changes` job) and `scripts/check-gate-inputs.sh`
condition 9 (D8's completeness assertion) can never read two different
things.
"""

from __future__ import annotations

import tomllib
from pathlib import Path

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
