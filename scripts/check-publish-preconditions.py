#!/usr/bin/env python3.14
"""RFC 141 §3: verify Level B and release identity before the publish
workflow mints a crates.io Trusted Publishing token.

RFC 131 D2 makes a release cut require every `[gates]` lane green via
`workflow_dispatch` on the exact commit being tagged -- a person checks
that today. A publish workflow that does not check it would *weaken*
D2 by automating the publish without automating the check, removing
the person who was reading it with nothing in their place. This script
is that check, run from the workflow before `rust-lang/crates-io-auth-
action` mints a token, so a failure here stops the job before that
step ever runs (a nonzero exit from this step fails the job outright;
no later step executes).

Four things, independent, all checked and reported together rather
than stopping at the first (RFC 141 §3's own four bullets, in order):

  1. the tag resolves to a commit in this checkout, is an **annotated**
     tag (not lightweight), and that commit is an ancestor of `main`;
  2. the six `Cargo.toml` version fields ([workspace.package] plus the
     five [workspace.dependencies] entries with their own `path`)
     equal the tag;
  3. the most recent `CI` workflow run of event `workflow_dispatch` on
     that exact commit concluded `success`, with every job's own
     conclusion counted explicitly as `success`, `failure` or
     `skipped` -- anything else (most importantly a job still running,
     whose `conclusion` is `null`) is "not finished", never silently
     folded into one of the other three. 0 skipped is required: RFC
     130 D7 makes `workflow_dispatch` run the complete matrix
     regardless of what changed, so a skip on this trigger means
     something is wrong, not that a path filter correctly excluded it;
  4. the most recent `Security audit` workflow run (any trigger) on
     that same commit concluded `success`.

Checks 1 and 2 are local (git, and a local file); checks 3 and 4 read
the GitHub REST API via the four `*_for_sha`/`jobs_for_run` functions
below, each one wrapping exactly one API call so a test can replace it
with `unittest.mock.patch.object` (the same shape
`check-published-versions.py`'s own `registry_max_version` already
uses for the one other network call this project's own checkers make).

Exit 0 = all four conditions satisfied. Exit 1 = at least one refused
the publish (every refusal reported, not only the first). Exit 2 = a
condition could not be checked at all (network error, malformed API
response, unreadable file) -- distinct from a refusal, because "could
not verify" and "verified and it says no" must never read the same to
whoever is watching the job.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

# The five `[workspace.dependencies]` entries that carry this workspace's
# own `version` (and a `path`, distinguishing them from a third-party
# dependency of the same shape). `sui-id` itself has no entry here -- it
# inherits `[workspace.package].version` directly, which is the sixth site.
DEP_CRATES = ("sui-id-core", "sui-id-i18n", "sui-id-shared", "sui-id-store", "sui-id-web")

API_BASE = "https://api.github.com"
USER_AGENT = "sui-id-release-check (https://github.com/nabbisen/sui-id)"


class ApiError(Exception):
    """Wraps any failure reaching or parsing the GitHub API, so `main`
    can tell "could not check" (exit 2) apart from "checked, and it
    refuses" (exit 1) without re-deriving the distinction at each call
    site."""


def gh_api(path: str, token: str, timeout: float) -> dict:
    req = urllib.request.Request(
        f"{API_BASE}/{path}",
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": USER_AGENT,
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.load(resp)
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as exc:
        raise ApiError(f"{path}: {exc}") from exc


def ci_run_for_sha(repo: str, sha: str, token: str, timeout: float) -> dict | None:
    """The most recent `CI` workflow run of event `workflow_dispatch` on
    this exact commit, or `None` if there is none. Scoped to
    `event=workflow_dispatch` deliberately: RFC 141 §3's own wording is
    "not merely some run on that SHA" -- a green `push` or `pull_request`
    run on the same commit does not satisfy Level B, because `ci-gate.sh`
    behaves differently locally than it does hosted, and only
    `workflow_dispatch` is specified as the Level B trigger (RFC 131 D2,
    RFC 130 D7)."""
    data = gh_api(
        f"repos/{repo}/actions/workflows/ci.yml/runs"
        f"?head_sha={sha}&event=workflow_dispatch&per_page=1",
        token,
        timeout,
    )
    runs = data.get("workflow_runs") or []
    return runs[0] if runs else None


def jobs_for_run(repo: str, run_id: int, token: str, timeout: float) -> list[dict]:
    data = gh_api(f"repos/{repo}/actions/runs/{run_id}/jobs?per_page=100", token, timeout)
    return data.get("jobs") or []


def audit_run_for_sha(repo: str, sha: str, token: str, timeout: float) -> dict | None:
    """The most recent `Security audit` workflow run on this exact commit,
    any trigger event, or `None` if there is none. Unlike the CI check,
    RFC 141 §3's second bullet names no event restriction -- `audit.yml`
    already runs on every push to `main`, and `release-process.md`'s own
    step 0/step 5 readings take whichever run is current, not only a
    manually dispatched one."""
    data = gh_api(
        f"repos/{repo}/actions/workflows/audit.yml/runs?head_sha={sha}&per_page=1",
        token,
        timeout,
    )
    runs = data.get("workflow_runs") or []
    return runs[0] if runs else None


def check_ci(repo: str, sha: str, token: str, timeout: float) -> list[str]:
    run = ci_run_for_sha(repo, sha, token, timeout)
    if run is None:
        return [f"no CI workflow_dispatch run found for {sha}"]
    if run.get("conclusion") != "success":
        return [
            f"CI workflow_dispatch run {run.get('html_url', run.get('id'))} "
            f"concluded {run.get('conclusion')!r}, not success"
        ]
    jobs = jobs_for_run(repo, run["id"], token, timeout)
    if not jobs:
        return [f"CI run {run.get('id')} reports no jobs -- cannot confirm 0 skipped"]
    # Counted explicitly, never by subtraction from "the rest must be
    # success": a job still running has `conclusion = null`, which is
    # none of success/failure/skipped and must be "not finished", not
    # silently absorbed into one of the three named buckets. This is
    # the architect's own carried mistake (dispatch §2) -- reporting a
    # run complete while jobs were still in progress.
    success = sum(1 for j in jobs if j.get("conclusion") == "success")
    failure = sum(1 for j in jobs if j.get("conclusion") == "failure")
    skipped = sum(1 for j in jobs if j.get("conclusion") == "skipped")
    other = len(jobs) - success - failure - skipped
    errors = []
    if other > 0:
        errors.append(
            f"{other} job(s) in run {run.get('id')} are not finished "
            f"(conclusion is neither success, failure, nor skipped -- "
            f"still running, cancelled, or another non-terminal state)"
        )
    if skipped > 0:
        errors.append(
            f"{skipped} job(s) in run {run.get('id')} were skipped; a "
            f"workflow_dispatch run must execute the complete matrix "
            f"(RFC 130 D7) -- a skip here means something is wrong, not "
            f"that a path filter correctly excluded it"
        )
    if failure > 0:
        errors.append(f"{failure} job(s) in run {run.get('id')} failed")
    return errors


def check_audit(repo: str, sha: str, token: str, timeout: float) -> list[str]:
    run = audit_run_for_sha(repo, sha, token, timeout)
    if run is None:
        return [f"no Security audit run found for {sha}"]
    if run.get("conclusion") != "success":
        return [
            f"Security audit run {run.get('html_url', run.get('id'))} "
            f"concluded {run.get('conclusion')!r}, not success"
        ]
    return []


def git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(root), *args], capture_output=True, text=True
    )


def check_tag_identity(root: Path, tag: str, main_ref: str) -> tuple[list[str], str | None]:
    """Returns `(errors, resolved_commit_sha)`. `sha` is `None` only when
    the tag could not be resolved to a commit at all -- an ancestry
    failure still returns the resolved `sha` alongside its own error, so
    the CI/audit checks below can still report on that commit."""
    kind = git(root, "cat-file", "-t", tag)
    if kind.returncode != 0:
        return ([f"tag {tag!r} does not exist in this checkout"], None)
    if kind.stdout.strip() != "tag":
        return (
            [
                f"tag {tag!r} is not annotated (git cat-file -t reports "
                f"{kind.stdout.strip()!r}, not 'tag')"
            ],
            None,
        )
    rev = git(root, "rev-parse", f"{tag}^{{commit}}")
    if rev.returncode != 0:
        return ([f"could not resolve {tag!r} to a commit: {rev.stderr.strip()}"], None)
    sha = rev.stdout.strip()
    anc = git(root, "merge-base", "--is-ancestor", sha, main_ref)
    if anc.returncode != 0:
        return ([f"commit {sha} (tag {tag!r}) is not an ancestor of {main_ref}"], sha)
    return ([], sha)


def check_cargo_versions(root: Path, sha: str, tag: str) -> list[str]:
    # Read via `git show <sha>:Cargo.toml`, not the filesystem -- the
    # working tree currently checked out is not guaranteed to be the
    # tagged commit (a misconfigured checkout step would check out
    # `main` instead, for instance), and reading a value that could be
    # stale rather than the one this check exists to verify is exactly
    # the class of bug this round's own §1 (`origins_before`) and C10
    # (`logo_uri`) fixed. Found by testing this script against this
    # repository's own real `0.85.0` tag before trusting it.
    result = git(root, "show", f"{sha}:Cargo.toml")
    if result.returncode != 0:
        return [f"cannot read Cargo.toml at {sha}: {result.stderr.strip()}"]
    try:
        data = tomllib.loads(result.stdout)
    except tomllib.TOMLDecodeError as exc:
        return [f"cannot parse Cargo.toml at {sha}: {exc}"]
    errors = []
    pkg_version = data.get("workspace", {}).get("package", {}).get("version")
    if pkg_version != tag:
        errors.append(f"[workspace.package] version = {pkg_version!r}, expected {tag!r}")
    deps = data.get("workspace", {}).get("dependencies", {})
    for crate in DEP_CRATES:
        entry = deps.get(crate)
        v = entry.get("version") if isinstance(entry, dict) else None
        if v != tag:
            errors.append(
                f"[workspace.dependencies.{crate}] version = {v!r}, expected {tag!r}"
            )
    return errors


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--repo", required=True, help="owner/repo, e.g. nabbisen/sui-id")
    parser.add_argument("--tag", required=True)
    parser.add_argument("--token", required=True)
    parser.add_argument("--main-ref", default="origin/main")
    parser.add_argument("--timeout", type=float, default=30.0)
    args = parser.parse_args(argv)
    root = Path(args.root)

    refusals: list[str] = []
    problems: list[str] = []

    tag_errors, sha = check_tag_identity(root, args.tag, args.main_ref)
    refusals.extend(tag_errors)

    if sha is not None:
        refusals.extend(check_cargo_versions(root, sha, args.tag))
        try:
            refusals.extend(check_ci(args.repo, sha, args.token, args.timeout))
        except ApiError as exc:
            problems.append(f"CI check: {exc}")
        try:
            refusals.extend(check_audit(args.repo, sha, args.token, args.timeout))
        except ApiError as exc:
            problems.append(f"Security audit check: {exc}")
    else:
        problems.append(
            "Cargo.toml / CI / Security audit checks skipped: tag could not be resolved to a commit"
        )

    if problems:
        print("check-publish-preconditions: could not verify:", file=sys.stderr)
        for p in problems:
            print(f"  - {p}", file=sys.stderr)
        if refusals:
            print("check-publish-preconditions: also refused:", file=sys.stderr)
            for r in refusals:
                print(f"  - {r}", file=sys.stderr)
        return 2

    if refusals:
        print("check-publish-preconditions: refused:", file=sys.stderr)
        for r in refusals:
            print(f"  - {r}", file=sys.stderr)
        return 1

    print(f"check-publish-preconditions: {args.tag} ({sha}) satisfies all four preconditions")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
