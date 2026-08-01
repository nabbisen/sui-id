#!/usr/bin/env python3.14
"""RFC 093 G10b: markdown link checker.

Scans the given files and directories (recursing into directories for
every ``*.md`` file) for inline Markdown links (``[text](target)``,
including the image form ``![alt](target)``) and verifies each local
target: the file exists at the exact case given, and if the link carries
a ``#fragment``, that fragment resolves to a heading in the target file
(the current file, for a bare ``#fragment`` link).

Four violation categories, matching RFC 093's negative self-test table:
  - missing target (no such file)
  - bad anchor (file exists, fragment does not resolve to any heading)
  - absolute local path (a path starting with "/" is not portable across
    clones/mirrors and is rejected outright, never resolved)
  - case-mismatched path (a case-insensitive match exists but the exact
    case given does not -- this only reproduces on a case-sensitive
    filesystem, which is why it is checked explicitly rather than left to
    plain existence)

External links (any target with a URL scheme, e.g. "https:", "mailto:")
are not checked. Fenced code blocks are skipped, so example link syntax
in documentation is not treated as a real link. Reference-style links
([text]: target) are not supported -- the scanned set (README.md,
ROADMAP.md, docs/) uses inline links exclusively as of this writing. An
optional CommonMark link title (`"..."` or `'...'`, e.g.
``[x](./t.md "the title")``) is stripped before resolving the target;
a parenthesised title (`(title)`) is not supported, since the link regex
below cannot distinguish it from the link's own closing parenthesis --
the scanned set has none of either form as of this writing.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*#*$")
FENCE_RE = re.compile(r"^(```|~~~)")
SCHEME_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
TITLE_RE = re.compile(r"""\s+(?:"[^"]*"|'[^']*')\s*$""")


def slugify(heading_text: str) -> str:
    """Approximate GitHub's heading-to-anchor-slug algorithm."""
    text = heading_text.strip().lower()
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"\s+", "-", text).strip("-")
    return text


def extract_heading_slugs(path: Path) -> set[str]:
    slugs: set[str] = set()
    seen_counts: dict[str, int] = {}
    in_fence = False
    for line in path.read_text(encoding="utf-8").splitlines():
        if FENCE_RE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        m = HEADING_RE.match(line)
        if not m:
            continue
        base = slugify(m.group(2))
        if base == "":
            continue
        count = seen_counts.get(base, 0)
        slug = base if count == 0 else f"{base}-{count}"
        seen_counts[base] = count + 1
        slugs.add(slug)
    return slugs


def extract_links(path: Path) -> list[tuple[int, str]]:
    """Returns (1-based line number, raw target) for every inline link,
    skipping fenced code blocks."""
    links: list[tuple[int, str]] = []
    in_fence = False
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if FENCE_RE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        for m in LINK_RE.finditer(line):
            target = m.group(1).strip()
            target = TITLE_RE.sub("", target)
            links.append((lineno, target))
    return links


def case_insensitive_match(target: Path) -> Path | None:
    """If target does not exist but a case-insensitive match does
    (walking each path component), return the actual on-disk path."""
    parts = target.parts
    current = Path(target.anchor) if target.is_absolute() else Path(".")
    for part in parts[1:] if target.is_absolute() else parts:
        if not current.is_dir():
            return None
        exact = current / part
        if exact.exists():
            current = exact
            continue
        match = None
        try:
            entries = list(current.iterdir())
        except OSError:
            return None
        for entry in entries:
            if entry.name.lower() == part.lower():
                match = entry
                break
        if match is None:
            return None
        current = match
    return current if current.exists() else None


def check_file(md_file: Path, root: Path, failures: list[str]) -> None:
    heading_cache: dict[Path, set[str]] = {}

    def headings_of(p: Path) -> set[str]:
        if p not in heading_cache:
            heading_cache[p] = extract_heading_slugs(p)
        return heading_cache[p]

    for lineno, raw_target in extract_links(md_file):
        if SCHEME_RE.match(raw_target):
            continue  # external link (http:, https:, mailto:, ...)

        path_part, _, fragment = raw_target.partition("#")

        if path_part == "":
            # Bare in-file anchor, e.g. "#section".
            if fragment and fragment not in headings_of(md_file):
                failures.append(
                    f"{md_file}:{lineno}: bad anchor: #{fragment} not found in {md_file}"
                )
            continue

        if path_part.startswith("/"):
            failures.append(
                f"{md_file}:{lineno}: absolute local path not allowed: {raw_target}"
            )
            continue

        target = (md_file.parent / path_part).resolve()
        try:
            target_rel = target.relative_to(root.resolve())
        except ValueError:
            target_rel = target

        if target.exists():
            resolved = target
        else:
            case_match = case_insensitive_match(target)
            if case_match is not None:
                try:
                    case_match_rel = case_match.relative_to(root.resolve())
                except ValueError:
                    case_match_rel = case_match
                failures.append(
                    f"{md_file}:{lineno}: case mismatch: link references "
                    f"'{path_part}' but the actual path is "
                    f"'{case_match_rel}'"
                )
                continue
            failures.append(
                f"{md_file}:{lineno}: target does not exist: {target_rel}"
            )
            continue

        if fragment and resolved.suffix == ".md":
            if fragment not in headings_of(resolved):
                failures.append(
                    f"{md_file}:{lineno}: bad anchor: #{fragment} not found in {target_rel}"
                )


def collect_markdown_files(root: Path, targets: list[str]) -> list[Path]:
    files: list[Path] = []
    for target in targets:
        p = root / target
        if p.is_dir():
            files.extend(sorted(p.rglob("*.md")))
        elif p.is_file():
            files.append(p)
        else:
            raise SystemExit(f"check-markdown-links: target not found: {p}")
    return files


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("targets", nargs="+")
    args = parser.parse_args(argv)

    root = Path(args.root)
    md_files = collect_markdown_files(root, args.targets)

    failures: list[str] = []
    for md_file in md_files:
        check_file(md_file, root, failures)

    if failures:
        for line in failures:
            print(f"check-markdown-links: {line}", file=sys.stderr)
        print(
            f"check-markdown-links: failed with {len(failures)} violation(s)",
            file=sys.stderr,
        )
        return 1

    print("check-markdown-links: all links resolve")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
