#!/usr/bin/env python3.14
"""RFC 131 D7: a tagged version is published, or recorded as abandoned or
superseded.

A release cut (RFC 131 D2) requires Level B on the commit it cites, but says
nothing about the cut *reaching anyone* -- and that turned out to be the gap
that mattered. Four tagged versions (0.76.10, 0.76.11, 0.76.12, 0.78.0) were
never published; the first three sat undetected for three months, in a file
that described the symptom as a caution about *how to verify the registry*,
not as an open defect.

This script is the detector: for each of sui-id's six published crates, the
newest git tag is compared against the registry's current `max_version`
(crates.io requires an explicit `User-Agent` -- without one it returns a
policy error that reads as "not published" for every crate, which is the
trap `docs/src/contributing/release-process.md` already records). A tag
ahead of the registry is a gap, unless `CHANGELOG.md` records that version as
abandoned or superseded -- abandoned for D7 point 1's own resolution, for
0.78.0 and 0.76.10-12; superseded for 0.80.0 (the `0.81.0` release
proposal's §G), the first version excused for a reason other than having
simply been missed.

**Corrected 2026-10-01, the same day RFC 131 was written**: the first draft
of D7 ran this weekly, in `audit.yml`'s shape. The RFC's own decision text
reversed that (`.github/workflows/fuzz.yml:3-5` already records why a weekly
schedule fails silently in a solo repo), but this script's own *handoff*
dispatch still said "weekly" -- a stale instruction that didn't get updated
when the RFC was amended. This implements the RFC's corrected decision, not
the handoff's stale one: a required release-time step, not a schedule.
`docs/src/contributing/release-process.md` is where it's wired in.

Exit 0 = every tagged version is published or recorded abandoned/superseded.
Exit 1 = a gap. Exit 2 = a tag, the registry, or CHANGELOG.md could not be
read.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

CRATES = (
    "sui-id-shared",
    "sui-id-i18n",
    "sui-id-store",
    "sui-id-core",
    "sui-id-web",
    "sui-id",
)
USER_AGENT = "sui-id-release-check (https://github.com/nabbisen/sui-id)"
VERSION_RE = re.compile(r"\b\d+\.\d+\.\d+\b")


def semver_key(v: str) -> tuple[int, int, int]:
    parts = v.split(".")
    return (int(parts[0]), int(parts[1]), int(parts[2]))


def newest_tag(root: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), "tag"], capture_output=True, text=True, check=True
    )
    tags = [t for t in result.stdout.splitlines() if VERSION_RE.fullmatch(t)]
    if not tags:
        raise RuntimeError("no version-shaped git tags found")
    return max(tags, key=semver_key)


SECTION_EXCUSED_RE = re.compile(
    r"^##\s*\[(\d+\.\d+\.\d+)\].*?\n+(?:>.*\n?)*?>\s*\*\*never published.*?(?:abandoned|superseded)",
    re.IGNORECASE | re.MULTILINE,
)
LIST_EXCUSED_RE = re.compile(
    r"((?:\d+\.\d+\.\d+(?:,\s*|\s+and\s+))*\d+\.\d+\.\d+)\s+(?:is|are)\s+(?:abandoned|superseded)",
    re.IGNORECASE,
)


def excused_versions(changelog_path: Path) -> set[str]:
    """Versions CHANGELOG.md records as deliberately not published --
    *abandoned* (never published, no good reason to) or *superseded*
    (never published, a later release carries the content and a specific
    reason -- e.g. security -- not to put the gap on the registry).

    Renamed from `abandoned_versions` (RFC 131 D7's own name) once a
    second, distinct reason to skip a version existed: `0.80.0` is the
    first version this script has ever had to recognise that is not
    abandoned at all, and naming the function after only one of the two
    reasons it now excuses would itself be the kind of mismatch between
    a name and what it actually does this project's own design
    philosophy exists to catch.

    Two patterns, both present today and both narrow on purpose -- a loose
    "this paragraph mentions the word somewhere" scan over-matched 0.79.0,
    which is neither abandoned nor superseded, merely *named* in 0.78.0's
    abandonment note as where its content actually shipped:

    1. A version's own section (`## [X.Y.Z]`) opens with a blockquote stating
       it was never published and is abandoned or superseded (0.78.0's shape;
       0.80.0's superseded record follows the same shape).
    2. An explicit list followed by "is/are abandoned" or "is/are superseded"
       (0.76.10-12's shape, recorded inside 0.78.0's own section rather than
       their own).
    """
    text = changelog_path.read_text(encoding="utf-8")
    found: set[str] = set()
    found.update(SECTION_EXCUSED_RE.findall(text))
    for m in LIST_EXCUSED_RE.finditer(text):
        found.update(VERSION_RE.findall(m.group(1)))
    return found


def registry_max_version(crate: str, timeout: float) -> str:
    req = urllib.request.Request(
        f"https://crates.io/api/v1/crates/{crate}",
        headers={"User-Agent": USER_AGENT},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        data = json.load(resp)
    return data["crate"]["max_version"]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--changelog", default="CHANGELOG.md")
    parser.add_argument("--timeout", type=float, default=15.0)
    args = parser.parse_args(argv)
    root = Path(args.root)

    try:
        tag = newest_tag(root)
    except (subprocess.CalledProcessError, RuntimeError) as exc:
        print(f"check-published-versions: cannot determine the newest tag: {exc}", file=sys.stderr)
        return 2

    try:
        excused = excused_versions(root / args.changelog)
    except OSError as exc:
        print(f"check-published-versions: cannot read {args.changelog}: {exc}", file=sys.stderr)
        return 2

    if tag in excused:
        print(
            f"check-published-versions: {tag} is tagged and recorded as "
            f"abandoned or superseded; nothing to check"
        )
        return 0

    gaps = []
    for crate in CRATES:
        try:
            max_version = registry_max_version(crate, args.timeout)
        except (urllib.error.URLError, TimeoutError, KeyError, json.JSONDecodeError) as exc:
            print(f"check-published-versions: cannot query the registry for {crate}: {exc}", file=sys.stderr)
            return 2
        if semver_key(tag) > semver_key(max_version):
            gaps.append(f"{crate}: tag {tag} is ahead of the registry's {max_version}")

    if gaps:
        for g in gaps:
            print(f"check-published-versions: {g}", file=sys.stderr)
        print(
            f"check-published-versions: {tag} is tagged but not fully published, "
            f"and CHANGELOG.md does not record it as abandoned or superseded",
            file=sys.stderr,
        )
        return 1

    print(f"check-published-versions: {tag} is published on all {len(CRATES)} crates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
