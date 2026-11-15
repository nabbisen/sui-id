#!/usr/bin/env python3.14
"""Wait until crates.io's download CDN serves a just-published crate.

Why it exists: `cargo publish` returns once the crate is in the registry
*index*, but the next crate's own `cargo publish` verifies by downloading
its dependencies from `static.crates.io`, which can still answer 404 for a
crate that was published seconds earlier. That race stopped `0.91.0`'s
publish (run 43708004082): `sui-id-shared` 0.91.0 was published, then
`sui-id-store`'s verification got a cached 404 for it (`x-cache: HIT`,
`NoSuchKey`), and the publish stopped with two of six crates on the
registry.

So this polls **the exact URL cargo fetches**, not a cache-busted one: a
cache-busted 200 would not prove that cargo's own request will succeed.

Exit 0 once the URL answers 200 or 206. Exit 1 after `--timeout` seconds
without one. Exit 2 on a bad argument.

    python3.14 scripts/wait-for-crate-download.py --crate sui-id-shared --version 0.91.1
"""

from __future__ import annotations

import argparse
import re
import sys
import time
import urllib.error
import urllib.request
from typing import Callable

URL = "https://static.crates.io/crates/{crate}/{version}/download"
CRATE_RE = re.compile(r"^[a-z][a-z0-9_-]{0,63}$")
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+$")
READY = (200, 206)


def download_url(crate: str, version: str) -> str:
    if not CRATE_RE.match(crate):
        raise ValueError(f"not a crate name: {crate!r}")
    if not VERSION_RE.match(version):
        raise ValueError(f"not a release version (MAJOR.MINOR.PATCH): {version!r}")
    return URL.format(crate=crate, version=version)


def probe(url: str) -> int:
    """One GET for the first byte only. Returns the HTTP status, or 0 when
    the request could not be completed at all (DNS, TLS, a timeout)."""
    req = urllib.request.Request(url, headers={"Range": "bytes=0-0", "User-Agent": "sui-id-release/1"})
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.status
    except urllib.error.HTTPError as e:
        return e.code
    except (urllib.error.URLError, TimeoutError, OSError):
        return 0


def wait(
    url: str,
    timeout: float,
    interval: float,
    probe: Callable[[str], int] = probe,
    sleep: Callable[[float], None] = time.sleep,
    now: Callable[[], float] = time.monotonic,
    log: Callable[[str], None] = print,
) -> bool:
    deadline = now() + timeout
    attempt = 0
    while True:
        attempt += 1
        status = probe(url)
        if status in READY:
            log(f"wait-for-crate-download: {url} answered {status} on attempt {attempt}")
            return True
        if now() >= deadline:
            log(f"wait-for-crate-download: {url} still answers {status or 'no response'} after {timeout:.0f}s ({attempt} attempts)")
            return False
        log(f"wait-for-crate-download: {url} answered {status or 'no response'}; retrying in {interval:.0f}s")
        sleep(interval)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--crate", required=True)
    ap.add_argument("--version", required=True)
    ap.add_argument("--timeout", type=float, default=1200.0, help="seconds (default: 1200)")
    ap.add_argument("--interval", type=float, default=10.0, help="seconds between attempts (default: 10)")
    args = ap.parse_args(argv)
    try:
        url = download_url(args.crate, args.version)
    except ValueError as e:
        print(f"wait-for-crate-download: {e}", file=sys.stderr)
        return 2
    return 0 if wait(url, args.timeout, args.interval) else 1


if __name__ == "__main__":
    sys.exit(main())
