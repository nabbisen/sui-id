#!/usr/bin/env python3.14
"""RFC 143 stage 2 part D: a stale vendored OpenSSL blocks a release.

**Why this exists** (cycle K stage 1, confirmed stage 2): RustSec has filed
no advisory against `openssl-src` since 2023-02-07, so a new OpenSSL
vulnerability in the static build's vendored copy would never turn
`Security audit` red. OpenSSL ships its own security fixes as patch
releases, and `openssl-src` (the crate `openssl-sys`'s `vendored` feature
builds, `crates/sui-id/Cargo.toml`'s `[target.'cfg(target_env = "musl")'
.dependencies]` table) follows each one with a new crate version. This
script is the thing that notices when `Cargo.lock`'s copy has fallen behind,
using data this project already reads (crates.io's own version list), the
same way `check-published-versions.py` does for sui-id's own crates.

**The crate's version scheme, measured, not assumed** (`cargo info
openssl-src` / crates.io, 2026-11-08): `<crate major>.<crate minor>.<crate
patch>+<openssl major>.<openssl minor>.<openssl patch>` -- for example
`300.6.1+3.6.3` is openssl-src's own 300.6.1, carrying OpenSSL 3.6.3. The
crate major tracks the OpenSSL *generation* (`300.x.y` ships OpenSSL 3.x;
a `400.x.y` series carrying OpenSSL 4.x already exists on crates.io as of
this writing) -- but the crate's own minor/patch numbering does **not**
track OpenSSL's one-for-one: `300.5.3+3.5.4` and `300.5.4+3.5.4` are two
different crate releases carrying the identical OpenSSL patch version
(a packaging-only crate bump). **So "the same OpenSSL series" is defined
from the embedded OpenSSL version's own `major.minor` (e.g. "3.6"), not
from the crate's.** Within one series, OpenSSL's own patch component is
what a security fix bumps; the newest published patch for the locked
series is what this check compares against.

**Deliberately scoped to "behind within the same series", not "a newer
series exists".** A new OpenSSL minor or major (3.7, 4.0, ...) is a
decision this project makes deliberately (an upgrade, not a drop-in patch);
this check reports one exists, informationally, but does not fail for it --
only for a locked series that itself has a newer patch published and
unapplied, which is unambiguously a security-maintenance release owed.

Exit 0 = current (the newest published patch for the locked series is the
one locked, or the locked series has no newer patch published). Exit 1 =
behind. Exit 2 = `Cargo.lock`, the registry, or the version strings
themselves could not be read/parsed.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

CRATE = "openssl-src"
USER_AGENT = "sui-id-release-check (https://github.com/nabbisen/sui-id)"

# `name = "openssl-src"` then, on the next `version = "..."` line, the
# locked version -- Cargo.lock's own `[[package]]` shape, read as text
# the same way `check-published-versions.py` reads tags rather than
# parsing full TOML for one field.
LOCK_VERSION_RE = re.compile(
    r'name\s*=\s*"openssl-src"\s*\nversion\s*=\s*"([^"]+)"', re.MULTILINE
)

# `<crate major>.<crate minor>.<crate patch>+<openssl major>.<openssl
# minor>.<openssl patch>`.
VERSION_RE = re.compile(
    r"^(\d+)\.(\d+)\.(\d+)\+(\d+)\.(\d+)\.(\d+)$"
)


class ParsedVersion:
    __slots__ = ("crate", "openssl", "raw")

    def __init__(self, raw: str) -> None:
        m = VERSION_RE.match(raw)
        if not m:
            raise ValueError(f"does not match <crate ver>+<openssl ver>: {raw!r}")
        self.raw = raw
        self.crate = (int(m.group(1)), int(m.group(2)), int(m.group(3)))
        self.openssl = (int(m.group(4)), int(m.group(5)), int(m.group(6)))

    def series(self) -> tuple[int, int]:
        return (self.openssl[0], self.openssl[1])

    def __repr__(self) -> str:
        return f"ParsedVersion({self.raw!r})"


def locked_version(cargo_lock_text: str) -> ParsedVersion:
    m = LOCK_VERSION_RE.search(cargo_lock_text)
    if not m:
        raise ValueError(f"no `{CRATE}` package found in Cargo.lock")
    return ParsedVersion(m.group(1))


def registry_versions(timeout: float) -> list[str]:
    """Every published version string of `openssl-src`, crates.io's own
    order (newest first), unfiltered. A thin seam so tests can supply a
    fixed list instead of a live fetch -- `check-published-versions.py`'s
    own `registry_max_version` is mocked the same way."""
    req = urllib.request.Request(
        f"https://crates.io/api/v1/crates/{CRATE}/versions",
        headers={"User-Agent": USER_AGENT},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        data = json.load(resp)
    return [v["num"] for v in data["versions"]]


def parse_all(raw_versions: list[str]) -> list[ParsedVersion]:
    """Parse every version string that matches the scheme; a crate
    version from before this scheme existed (none are known, but the
    registry is not this project's to control) is skipped rather than
    treated as a parse failure for the whole list."""
    out = []
    for raw in raw_versions:
        try:
            out.append(ParsedVersion(raw))
        except ValueError:
            continue
    return out


class Result:
    def __init__(
        self,
        locked: ParsedVersion,
        newest_in_series: ParsedVersion | None,
        newer_series_exists: bool,
    ) -> None:
        self.locked = locked
        self.newest_in_series = newest_in_series
        self.newer_series_exists = newer_series_exists

    @property
    def behind(self) -> bool:
        if self.newest_in_series is None:
            return False
        return self.newest_in_series.openssl > self.locked.openssl


def evaluate(locked: ParsedVersion, published: list[ParsedVersion]) -> Result:
    same_series = [p for p in published if p.series() == locked.series()]
    newest_in_series = max(same_series, key=lambda p: p.openssl, default=None)
    newer_series_exists = any(p.series() > locked.series() for p in published)
    return Result(locked, newest_in_series, newer_series_exists)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--lockfile", default="Cargo.lock")
    parser.add_argument("--timeout", type=float, default=15.0)
    args = parser.parse_args(argv)
    root = Path(args.root)

    try:
        text = (root / args.lockfile).read_text()
    except OSError as exc:
        print(f"check-openssl-src-freshness: cannot read {args.lockfile}: {exc}", file=sys.stderr)
        return 2

    try:
        locked = locked_version(text)
    except ValueError as exc:
        print(f"check-openssl-src-freshness: {exc}", file=sys.stderr)
        return 2

    try:
        raw_published = registry_versions(args.timeout)
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        print(f"check-openssl-src-freshness: could not reach crates.io: {exc}", file=sys.stderr)
        return 2

    published = parse_all(raw_published)
    if not published:
        print(
            "check-openssl-src-freshness: no parseable openssl-src versions "
            "returned by the registry",
            file=sys.stderr,
        )
        return 2

    result = evaluate(locked, published)

    series_str = f"{locked.series()[0]}.{locked.series()[1]}"
    locked_str = ".".join(str(n) for n in locked.openssl)
    if result.newer_series_exists:
        print(
            f"check-openssl-src-freshness: a newer OpenSSL series than "
            f"{series_str} is published (informational -- a series change "
            f"is a deliberate upgrade decision, not what this check enforces)"
        )

    if result.behind:
        newest_str = ".".join(str(n) for n in result.newest_in_series.openssl)  # type: ignore[union-attr]
        print(
            f"check-openssl-src-freshness: behind -- Cargo.lock has OpenSSL "
            f"{locked_str} ({locked.raw}), but {series_str}'s newest "
            f"published patch is {newest_str} "
            f"({result.newest_in_series.raw}). An OpenSSL security release "  # type: ignore[union-attr]
            f"means a sui-id release; see docs/src/contributing/"
            f"release-process.md.",
            file=sys.stderr,
        )
        return 1

    print(
        f"check-openssl-src-freshness: current -- OpenSSL {locked_str} "
        f"({locked.raw}) is {series_str}'s newest published patch"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
