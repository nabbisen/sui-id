#!/usr/bin/env python3.14
"""RFC 143 stage 5 part D: is the pinned OpenSSL still current, and still
supported?

This replaces `check-openssl-src-freshness.py` (stage 2 part D, withdrawn
2026-11-09), which compared `Cargo.lock`'s `openssl-src` version against
`openssl-src`'s own newest crates.io release in the same series -- "it
checked the supplier against itself" (the stage 5 decision's own words).
When `openssl-src` stopped following OpenSSL's 3.5/3.6 patch releases,
that check compared one stale value with an equally stale one and
reported "current". The crate is gone now (RFC 143 stage 5 part C):
the static build takes OpenSSL's own source directly, pinned in
`contracts/openssl.toml`, and this script reads OpenSSL itself --
never anything this project or any crate publishes about OpenSSL.

Two independent sources, both read fresh every run, no local cache:

  1. `git ls-remote --tags https://github.com/openssl/openssl.git` --
     every `openssl-X.Y.Z` tag (pre-releases, `-alphaN`/`-betaN`, are
     excluded by the exact-match pattern) OpenSSL has ever cut. The
     newest patch in the pinned series, compared against the pin.
  2. `https://openssl-library.org/source/` -- the Series / End-of-Life
     table that names every series OpenSSL currently supports. A series
     not in this table is out of support (this is the rule that would
     have caught 3.6, stage 5's own finding: the vendored series had
     already left this table, and nothing read it).

Four outcomes, deliberately not three:

  - **Fails** (exit 1) when the pinned version is behind the newest
    published patch in its own series.
  - **Fails** (exit 1) when the pinned series is not in OpenSSL's
    supported list at all.
  - **Warns**, exit 0, when the pinned series' end-of-life date is
    within 180 days -- time to plan a series move, not an emergency.
  - **Exits 2**, never 0, when either source cannot be read at all:
    a network failure, a tag list with no tag in the pinned series, or
    a page that parses to no Series/End-of-Life table. An unreadable
    source must turn this lane red, never pass it silently -- that is
    the lesson of the whole stage: the old check's real bug was never
    noticing its own comparison had gone stale.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
import urllib.error
import urllib.request
from datetime import date, datetime
from pathlib import Path

OPENSSL_REPO = "https://github.com/openssl/openssl.git"
SOURCE_PAGE = "https://openssl-library.org/source/"
USER_AGENT = "sui-id-release-check (https://github.com/nabbisen/sui-id)"

TAG_RE = re.compile(r"^openssl-(\d+)\.(\d+)\.(\d+)$")
# One row of the Series / End-of-Life table: Series, a <td> with a link
# (filename), Size, Release date, End-of-Life date, then the checksums
# cell. Series carries an optional " [LTS]" marker, stripped here.
ROW_RE = re.compile(
    r"<tr>\s*<td>([^<]+?)</td>\s*<td>.*?</td>\s*<td>[^<]*</td>\s*<td>[^<]*</td>\s*"
    r"<td>([^<]+)</td>",
    re.DOTALL,
)


class UnreadableSourceError(Exception):
    """Either source could not be read at all -- network failure, or a
    tag list / table that parsed to nothing usable. Always exit 2."""


def fetch_tags(timeout: float) -> list[str]:
    """Every tag name in the real OpenSSL repository, via `git
    ls-remote` -- no GitHub API call, so no rate limit and no
    authentication, for a check that runs on every push and weekly."""
    try:
        result = subprocess.run(
            ["git", "ls-remote", "--tags", OPENSSL_REPO],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=True,
        )
    except (subprocess.SubprocessError, OSError) as exc:
        raise UnreadableSourceError(f"git ls-remote --tags {OPENSSL_REPO}: {exc}") from exc
    tags = []
    for line in result.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) != 2 or not parts[1].startswith("refs/tags/"):
            continue
        name = parts[1][len("refs/tags/") :]
        if name.endswith("^{}"):  # a peeled annotated-tag line, same name again
            continue
        tags.append(name)
    return tags


def fetch_series_table(timeout: float) -> str:
    req = urllib.request.Request(SOURCE_PAGE, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.read().decode("utf-8", errors="replace")
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise UnreadableSourceError(f"{SOURCE_PAGE}: {exc}") from exc


def patches_in_series(tags: list[str], series: str) -> list[int]:
    major, minor = (int(p) for p in series.split("."))
    patches = []
    for tag in tags:
        m = TAG_RE.match(tag)
        if m and int(m.group(1)) == major and int(m.group(2)) == minor:
            patches.append(int(m.group(3)))
    return sorted(patches)


def parse_series_table(html: str) -> dict[str, date]:
    """Returns {series: end_of_life_date}, parsed from the first
    Series/End-of-Life table on the page. An empty dict (no rows found
    at all) means the page's layout has changed -- `main` treats that
    as unreadable, not as "no series supported"."""
    table: dict[str, date] = {}
    for m in ROW_RE.finditer(html):
        series_cell = m.group(1).strip()
        eol_cell = m.group(2).strip()
        series = series_cell.split()[0]  # drop a trailing "[LTS]"
        try:
            eol = datetime.strptime(eol_cell, "%d %b %Y").date()
        except ValueError:
            continue
        table[series] = eol
    return table


class Result:
    def __init__(
        self,
        behind: bool,
        newest_patch: int | None,
        series_dropped: bool,
        eol_date: date | None,
        eol_warning: bool,
    ) -> None:
        self.behind = behind
        self.newest_patch = newest_patch
        self.series_dropped = series_dropped
        self.eol_date = eol_date
        self.eol_warning = eol_warning


def contract_errors(pinned_version: str, series: str, patches: list[int]) -> list[str]:
    """The contract must describe what is built. The workflow builds
    `openssl_version`, while this check judges `series`. If the two
    disagree, a version outside the series would be judged by the
    series' facts: `openssl_version = "3.6.9"` under `series = "3.5"`
    compared patch 9 with 3.5.9 and reported "current" while the build
    shipped an unsupported 3.6. And a pinned patch upstream never
    published is a typo, not "ahead"."""
    m = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)", pinned_version)
    if m is None:
        return [f"openssl_version {pinned_version!r} is not MAJOR.MINOR.PATCH"]
    if f"{m.group(1)}.{m.group(2)}" != series:
        return [
            f"openssl_version {pinned_version} is not in series {series} -- the "
            f"workflow builds openssl_version, so both must name the same series"
        ]
    if int(m.group(3)) not in patches:
        return [f"openssl_version {pinned_version} is not a published upstream release"]
    return []


def evaluate(
    pinned_version: str,
    series: str,
    patches: list[int],
    eol_table: dict[str, date],
    today: date,
    warn_within_days: int = 180,
) -> Result:
    pinned_patch = int(pinned_version.split(".")[2])
    newest_patch = patches[-1] if patches else None
    behind = newest_patch is not None and pinned_patch < newest_patch

    eol_date = eol_table.get(series)
    series_dropped = eol_date is None
    eol_warning = eol_date is not None and (eol_date - today).days <= warn_within_days

    return Result(behind, newest_patch, series_dropped, eol_date, eol_warning)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", required=True, help="path to contracts/openssl.toml")
    parser.add_argument("--timeout", type=float, default=30.0)
    args = parser.parse_args(argv)

    with open(args.contract, "rb") as handle:
        contract = tomllib.load(handle)
    series = contract["series"]
    pinned_version = contract["openssl_version"]

    try:
        tags = fetch_tags(args.timeout)
        patches = patches_in_series(tags, series)
        if not patches:
            raise UnreadableSourceError(
                f"no 'openssl-{series}.N' tag found in the upstream tag list "
                f"({len(tags)} tags read in total) -- cannot confirm the pinned "
                f"version is current"
            )
        table_html = fetch_series_table(args.timeout)
        eol_table = parse_series_table(table_html)
        if not eol_table:
            raise UnreadableSourceError(
                f"{SOURCE_PAGE} parsed to no Series/End-of-Life rows at all -- "
                f"its layout may have changed"
            )
    except UnreadableSourceError as exc:
        print(f"check-openssl-freshness: could not verify: {exc}", file=sys.stderr)
        return 2

    errors = contract_errors(pinned_version, series, patches)
    if errors:
        for e in errors:
            print(f"check-openssl-freshness: {args.contract}: {e}", file=sys.stderr)
        return 1

    result = evaluate(pinned_version, series, patches, eol_table, date.today())

    refusals = []
    if result.behind:
        refusals.append(
            f"behind -- {series}'s newest published patch is "
            f"{series}.{result.newest_patch}, but {args.contract} pins "
            f"{pinned_version}"
        )
    if result.series_dropped:
        refusals.append(
            f"series {series} is no longer in OpenSSL's own supported list "
            f"({SOURCE_PAGE}) -- it must be moved to a series that is"
        )

    if refusals:
        for r in refusals:
            print(f"check-openssl-freshness: {r}", file=sys.stderr)
        return 1

    if result.eol_warning:
        print(
            f"check-openssl-freshness: series {series} reaches end-of-life on "
            f"{result.eol_date.isoformat()} -- plan the next series move",
            file=sys.stderr,
        )

    print(
        f"check-openssl-freshness: {pinned_version} is current for series {series} "
        f"(newest published patch: {series}.{result.newest_patch})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
