#!/usr/bin/env python3.14
"""RFC 098 G15: documentation-authority checker.

Three checks, in one script so one failure names one cause:

  (A) docs/src/SUMMARY.md is complete in both directions -- every page
      under docs/src/ is reachable from the book's table of contents, and
      every entry in the table of contents names a file that exists. An
      unreachable page is not published, so it drifts unread; an entry
      with no file breaks the build's navigation.

  (B) Every link takes the form RFC 098 rule 6 requires, in both
      directions. Outside docs/src/, an absolute
      https://github.com/nabbisen/sui-id/blob/ URL to this repository is
      a repository-relative link written wrongly: G10b and G14 skip
      external targets, so they cannot see whether it resolves, and the
      README's link to its own threat model was invisible to the gate
      for exactly that reason. Inside docs/src/ the rule inverts for
      targets outside the book, because no relative form works in both
      renderings -- mdBook rewrites `.md` to `.html` and emits
      `../../../ROADMAP.html`, which the build never produces. So a book
      page reaches an outside-book file by absolute URL and a book page
      by relative path, and never the other way round. The sanctioned
      absolute form is not gate-blind: its prefix is stripped and the
      remaining path must be a tracked file.

  (C) A document that declares the version it is current as of is either
      within the tolerance contracts/doc-authority.toml sets, or carries a
      staleness banner. RFC 098 rule 5 makes the banner the sanctioned
      state for a document that lags, so a bannered document passes. A
      document the policy lists that declares no version at all also
      fails: a pin that vanished is a claim that stopped being checkable.

  (D) The reader event reference lists exactly the events the audit
      coverage matrix registers, in both directions. The matrix is the
      copy G13 keeps true against the code; the reference is the copy
      operators read, and nothing kept it in step, so it fell nineteen
      events behind. A name counts only as the whole first cell of a table
      row, written as inline code: prose, other columns and fenced blocks
      may mention an event without registering it. Both paths come from
      [event_reference] in contracts/doc-authority.toml.

Exit 0 = pass. Exit 1 = any violation, each on stderr, prefixed
`check-doc-authority:` and naming its check and the file (with a line
number wherever the violation has one).
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path

# (B)'s scope. The complement of no gate: G10b covers README.md, ROADMAP.md
# and docs/; G11 covers the RFC files; G14 covers rfcs/handoffs/ and
# roadmap/. This check is about a link's *form* rather than its target, so
# it runs over all of them at once.
SELF_URL_SCOPE = ("README.md", "ROADMAP.md", "docs", "rfcs", "roadmap")
SELF_URL_PREFIX = "https://github.com/nabbisen/sui-id/blob/"
# The same URL with its `blob/<ref>/` head removed, leaving a repository
# path -- what makes the sanctioned form checkable rather than skipped.
SELF_URL_RE = re.compile(
    r"^https://github\.com/nabbisen/sui-id/blob/[^/]+/(.+)$"
)

BOOK_SRC = Path("docs/src")
SUMMARY = BOOK_SRC / "SUMMARY.md"

LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
INLINE_CODE_RE = re.compile(r"`[^`]*`")
FENCE_MARKER_RE = re.compile(r"^(`{3,}|~{3,})(.*)$")
FENCE_CLOSER_RE = re.compile(r"^(`+|~+)\s*$")
VERSION_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)")
EVENT_NAME_CELL_RE = re.compile(r"^`([a-z0-9_]+\.[a-z0-9_.]+)`$")
TABLE_SEPARATOR_RE = re.compile(r"^:?-{3,}:?$")
WORKSPACE_VERSION_RE = re.compile(r'^version\s*=\s*"([^"]+)"', re.M)


def iter_unfenced_lines(text: str) -> list[tuple[int, str]]:
    """(1-based lineno, line) for lines outside fenced code blocks, using
    the same real CommonMark fence-closing rule as check-rfc-integrity.py:
    a fence closes only on a line of the same fence character, repeated at
    least as many times as the opener, with no trailing text."""
    fence_char: str | None = None
    fence_len = 0
    out: list[tuple[int, str]] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if fence_char is None:
            m = FENCE_MARKER_RE.match(stripped)
            if m:
                fence_char = m.group(1)[0]
                fence_len = len(m.group(1))
                continue
            out.append((lineno, line))
        else:
            m = FENCE_CLOSER_RE.match(stripped)
            if m and m.group(1)[0] == fence_char and len(m.group(1)) >= fence_len:
                fence_char = None
                fence_len = 0
    return out


def extract_links(text: str) -> list[tuple[int, str]]:
    """(lineno, target) for every Markdown link outside fenced blocks and
    outside inline code spans. RFC 098 quotes the absolute-URL pattern in
    its own prose; masking inline code is what keeps check (B) from
    failing the document that defines it."""
    out: list[tuple[int, str]] = []
    for lineno, line in iter_unfenced_lines(text):
        masked = INLINE_CODE_RE.sub(lambda m: " " * len(m.group(0)), line)
        for m in LINK_RE.finditer(masked):
            out.append((lineno, m.group(1).strip()))
    return out


def local_target(target: str) -> str | None:
    """The path part of a link that points at a file in this repository,
    or None for an external URL, a bare anchor, or an empty target."""
    if target == "" or target.startswith("#"):
        return None
    if re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", target):
        return None
    path = target.split("#", 1)[0].split("?", 1)[0].strip()
    return path or None


def tracked_markdown(root: Path, scope: tuple[str, ...]) -> list[Path]:
    """Tracked *.md under the scope, as paths relative to root. Uses git
    so untracked scratch files and ignored build output never fail a gate."""
    present = [p for p in scope if (root / p).exists()]
    if not present:
        return []
    result = subprocess.run(
        ["git", "ls-files", "-z", "--", *present],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    )
    return sorted(
        Path(name)
        for name in result.stdout.split("\0")
        if name.endswith(".md")
    )


def check_summary_completeness(root: Path, failures: list[str]) -> None:
    summary_path = root / SUMMARY
    if not summary_path.is_file():
        failures.append(f"(A) {SUMMARY}: the book has no table of contents")
        return

    linked: set[Path] = set()
    for lineno, target in extract_links(summary_path.read_text(encoding="utf-8")):
        path = local_target(target)
        if path is None or not path.endswith(".md"):
            continue
        # Resolved, not compared as a string: `./guides/x.md` and
        # `guides/x.md` name the same page.
        resolved = (summary_path.parent / path).resolve()
        try:
            relative = resolved.relative_to(root.resolve())
        except ValueError:
            failures.append(
                f"(A) {SUMMARY}:{lineno}: entry points outside the repository: {target}"
            )
            continue
        linked.add(relative)
        if not resolved.is_file():
            failures.append(
                f"(A) {SUMMARY}:{lineno}: entry names a file that does not exist: {target}"
            )

    for page in sorted((root / BOOK_SRC).rglob("*.md")):
        relative = page.resolve().relative_to(root.resolve())
        if relative == SUMMARY:
            continue
        if relative not in linked:
            failures.append(
                f"(A) {relative}: page is not reachable from {SUMMARY}"
            )


def tracked_files(root: Path) -> set[str]:
    """Every tracked path in the repository, as posix strings relative to
    root. The sanctioned absolute form is checked against this, so a book
    page cannot link to a repository file that does not exist."""
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    )
    return {name for name in result.stdout.split("\0") if name}


def check_link_forms(root: Path, failures: list[str]) -> None:
    tracked = tracked_files(root)

    for relative in tracked_markdown(root, SELF_URL_SCOPE):
        in_book = relative.is_relative_to(BOOK_SRC)
        text = (root / relative).read_text(encoding="utf-8")

        for lineno, target in extract_links(text):
            if target.startswith(SELF_URL_PREFIX):
                if not in_book:
                    failures.append(
                        f"(B) {relative}:{lineno}: absolute link to this repository; "
                        f"use a repository-relative path: {target}"
                    )
                    continue
                # Rule 6's sanctioned form, and the reason it is not
                # gate-blind: strip `blob/<ref>/` and check what is left.
                m = SELF_URL_RE.match(target)
                if m is None:
                    failures.append(
                        f"(B) {relative}:{lineno}: absolute repository link with no "
                        f"file path after blob/<ref>/: {target}"
                    )
                    continue
                path = m.group(1).split("#", 1)[0].split("?", 1)[0]
                if Path(path).is_relative_to(BOOK_SRC):
                    failures.append(
                        f"(B) {relative}:{lineno}: rule 6 -- a book page links to a "
                        f"book page by relative path, not by absolute URL: {target}"
                    )
                elif path not in tracked:
                    failures.append(
                        f"(B) {relative}:{lineno}: absolute repository link names a "
                        f"path that is not tracked: {path}"
                    )
                continue

            if not in_book:
                continue

            # The other half of rule 6: a book page may not reach outside
            # the book relatively, because mdBook rewrites the target to a
            # page the build never produces.
            local = local_target(target)
            if local is None:
                continue
            resolved = ((root / relative).parent / local).resolve()
            if resolved.is_relative_to((root / BOOK_SRC).resolve()):
                continue
            failures.append(
                f"(B) {relative}:{lineno}: rule 6 -- a book page reaches an "
                f"outside-book file by absolute repository URL, not relatively; "
                f"mdBook rewrites this to a page the build never produces: {target}"
            )


def parse_version(text: str) -> tuple[int, int, int] | None:
    m = VERSION_RE.match(text.strip())
    if not m:
        return None
    return int(m.group(1)), int(m.group(2)), int(m.group(3))


def workspace_version(root: Path, failures: list[str]) -> tuple[int, int, int] | None:
    manifest = root / "Cargo.toml"
    if not manifest.is_file():
        failures.append("(C) Cargo.toml: not found; no workspace version to compare against")
        return None
    with manifest.open("rb") as handle:
        data = tomllib.load(handle)
    raw = data.get("workspace", {}).get("package", {}).get("version")
    if not isinstance(raw, str):
        failures.append("(C) Cargo.toml: [workspace.package] declares no version")
        return None
    parsed = parse_version(raw)
    if parsed is None:
        failures.append(f"(C) Cargo.toml: workspace version is not a semver triple: {raw}")
        return None
    return parsed


def check_version_freshness(root: Path, policy: dict, failures: list[str]) -> None:
    freshness = policy.get("freshness", {})
    documents = freshness.get("documents", [])
    if not documents:
        return

    current = workspace_version(root, failures)
    if current is None:
        return

    tolerance = int(freshness["tolerance_minor"])
    claim_re = re.compile(freshness["claim_regex"])
    banner_re = re.compile(freshness["banner_regex"], re.M)
    banner_lines = int(freshness["banner_within_lines"])

    for name in documents:
        relative = Path(name)
        path = root / relative
        if not path.is_file():
            failures.append(f"(C) {relative}: listed in the policy but does not exist")
            continue
        text = path.read_text(encoding="utf-8")

        claim: tuple[int, int, int] | None = None
        claim_line = 0
        for lineno, line in iter_unfenced_lines(text):
            m = claim_re.search(line)
            if not m:
                continue
            raw = next((g for g in m.groups() if g), None)
            if raw is None:
                continue
            claim = parse_version(raw)
            claim_line = lineno
            break

        if claim is None:
            failures.append(
                f"(C) {relative}: listed in the policy but declares no version it is "
                f"current as of; a pin that vanished is a claim that stopped being checkable"
            )
            continue

        if claim[0] == current[0]:
            lag = current[1] - claim[1]
        else:
            lag = tolerance + 1  # a major-version difference always exceeds

        if lag <= tolerance:
            continue

        head = "\n".join(text.splitlines()[:banner_lines])
        if banner_re.search(head):
            continue

        failures.append(
            f"(C) {relative}:{claim_line}: declares v{claim[0]}.{claim[1]}.{claim[2]} "
            f"against workspace v{current[0]}.{current[1]}.{current[2]} "
            f"(lag {lag} > tolerance {tolerance}) and carries no staleness banner"
        )


def table_event_names(text: str) -> dict[str, int]:
    """Event names in the first column of every Markdown table, mapped to
    the line each first appears on. Only a first cell that is exactly one
    inline-code dotted name counts; header and separator rows, prose, later
    columns and fenced blocks are ignored."""
    names: dict[str, int] = {}
    for lineno, line in iter_unfenced_lines(text):
        stripped = line.strip()
        if not stripped.startswith("|"):
            continue
        first = stripped[1:].split("|", 1)[0].strip()
        if TABLE_SEPARATOR_RE.match(first):
            continue
        m = EVENT_NAME_CELL_RE.match(first)
        if m:
            names.setdefault(m.group(1), lineno)
    return names


def check_event_reference(root: Path, policy: dict, failures: list[str]) -> None:
    section = policy.get("event_reference")
    if not isinstance(section, dict) or not {"matrix", "reference"} <= section.keys():
        failures.append(
            "(D) policy: [event_reference] must name both `matrix` and `reference`"
        )
        return
    matrix, reference = Path(section["matrix"]), Path(section["reference"])
    texts: dict[Path, str] = {}
    for relative in (matrix, reference):
        path = root / relative
        if path.is_file():
            texts[relative] = path.read_text(encoding="utf-8")
        else:
            failures.append(
                f"(D) {relative}: named in [event_reference] but does not exist"
            )
    if len(texts) < 2:
        return

    registered = table_event_names(texts[matrix])
    documented = table_event_names(texts[reference])
    for name in sorted(registered.keys() - documented.keys()):
        failures.append(
            f"(D) {reference}: missing an event registered at "
            f"{matrix}:{registered[name]}: {name}"
        )
    for name in sorted(documented.keys() - registered.keys()):
        failures.append(
            f"(D) {reference}:{documented[name]}: lists an event not "
            f"registered in {matrix}: {name}"
        )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--policy", required=True)
    args = parser.parse_args(argv)

    root = Path(args.root)
    with (root / args.policy).open("rb") as handle:
        policy = tomllib.load(handle)

    failures: list[str] = []
    check_summary_completeness(root, failures)
    check_link_forms(root, failures)
    check_version_freshness(root, policy, failures)
    check_event_reference(root, policy, failures)

    if failures:
        for line in failures:
            print(f"check-doc-authority: {line}", file=sys.stderr)
        print(
            f"check-doc-authority: failed with {len(failures)} violation(s)",
            file=sys.stderr,
        )
        return 1

    print("check-doc-authority: all conditions satisfied")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
