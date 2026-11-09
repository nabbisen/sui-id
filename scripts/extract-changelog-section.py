#!/usr/bin/env python3.14
"""RFC 143 stage 3 part A: extract one version's `CHANGELOG.md` section,
verbatim, for use as a GitHub Release body.

RFC 143 D5 says the Release body is "the `CHANGELOG` section for that
version, verbatim". That cannot be literal: `CHANGELOG.md`'s own links
are repo-relative (`rfcs/...`, `docs/...`), and GitHub's renderer leaves
a relative link exactly as written, so on a Release page
(`/<repo>/releases/tag/X.Y.Z`) it resolves against that URL, not the
repository root -- a 404. Measured 2026-11-09:

    gh api markdown -f mode=gfm -f context=<repo> -f text='[x](docs/y.md)'
    -> <a href="docs/y.md">x</a>

The architect's reading of D5: the *words* are verbatim; the *link
targets* are made to resolve, by rewriting each repo-relative target to
`https://github.com/<repo>/blob/<ref>/<target>`, pinned to the given
ref (normally the tag itself) so a link to e.g. `rfcs/accepted/...`
still resolves correctly after that RFC later moves to `rfcs/done/...`.
Nothing else in the text changes.

A target is rewritten only when it has no URL scheme (`https:`,
`mailto:`, ...), does not start with `#` or `/`, and does not contain
`..` -- a `..` target is refused outright (exit 1) rather than resolved
by guessing what it points at relative to the blob URL. Rewriting never
touches a fenced code block or an inline code span: `CHANGELOG.md` has
fenced ```toml`` blocks (example manifests) whose own bracket-paren text
must survive untouched.

RFC 143 stage 4, carried from stage 3's review: `G10b`/`G14` (this
project's markdown-link gates) do not cover `CHANGELOG.md`, so nothing
previously caught a section linking a file that had since moved or been
deleted -- the rewritten blob URL would simply 404 once a Release shipped
it. `--root` is the checkout to resolve a rewritten target against (on a
real release, the checkout *is* the tag, so this reads the same tree the
link will be pinned to); a target with no file or directory under it is
refused (exit 1) the same way a `..` target is, before the body is ever
printed. A target with a URL scheme, a bare `#fragment`, or an absolute
`/path` is never checked against the filesystem: `should_rewrite` already
excludes all three from rewriting, and this check only runs on what would
otherwise be rewritten. A repo-relative target with a fragment
(`docs/x.md#section`) is checked by its file part, and keeps the fragment.

Exit 0 = the section was found, is non-empty, and every link target
needing rewrite both had no `..` and resolved under `--root`; its
(possibly rewritten) body is printed to stdout. Exit 1 = the section is
missing, is empty after trimming, a link target contained `..`, or a
link target does not exist under `--root` -- each refusal is one line
on stderr naming the version (or target) and the file.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# A fence may be indented: CHANGELOG.md opens one inside a list item
# (`   ```toml`, at :1592 on 2026-11-09), and a column-0-only pattern
# would treat that block's contents as prose.
FENCE_RE = re.compile(r"^\s*(```|~~~)")
SCHEME_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
# CommonMark inline code span: a run of backticks, matching run to close.
CODE_SPAN_RE = re.compile(r"(?P<ticks>`+).*?(?P=ticks)")
LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")


class RefuseError(Exception):
    """A link target this script refuses to rewrite: contains `..`, or
    (RFC 143 stage 4) resolves to nothing under `--root`. The message is
    already complete; `main` prints `str(exc)` as-is rather than
    re-asserting which of the two reasons applies."""


def section_heading_re(version: str) -> re.Pattern[str]:
    return re.compile(r"^## \[" + re.escape(version) + r"\](?:\s*—\s*.*)?$")


def extract_section_lines(text: str, version: str) -> list[str] | None:
    """Returns the raw lines between the matching `## [version]` heading
    (excluded) and the next `## ` heading (excluded) or EOF, or `None` if
    no heading matches. Trimming and emptiness are the caller's concern."""
    lines = text.splitlines()
    heading_re = section_heading_re(version)
    start = None
    for i, line in enumerate(lines):
        if heading_re.match(line):
            start = i + 1
            break
    if start is None:
        return None
    end = start
    while end < len(lines) and not lines[end].startswith("## "):
        end += 1
    return lines[start:end]


def trim_blank(lines: list[str]) -> list[str]:
    i, j = 0, len(lines)
    while i < j and lines[i].strip() == "":
        i += 1
    while j > i and lines[j - 1].strip() == "":
        j -= 1
    return lines[i:j]


def should_rewrite(target: str) -> bool:
    return not (
        SCHEME_RE.match(target)
        or target.startswith("#")
        or target.startswith("/")
    )


def rewrite_line(line: str, repo: str, ref: str, root: Path) -> str:
    """Rewrites qualifying link targets on one line, skipping any match
    that falls inside an inline code span (computed once, from the
    original line, before any substitution -- so the replacement
    function's own output never perturbs which ranges count as "inside
    a code span")."""
    code_spans = [m.span() for m in CODE_SPAN_RE.finditer(line)]

    def in_code_span(pos: int) -> bool:
        return any(a <= pos < b for a, b in code_spans)

    def replace(m: re.Match[str]) -> str:
        if in_code_span(m.start()):
            return m.group(0)
        target = m.group(1)
        if not should_rewrite(target):
            return m.group(0)
        if ".." in target:
            raise RefuseError(f"refusing to rewrite link target {target!r} (contains '..')")
        # The file is the part before any `#fragment` or `?query`:
        # `docs/x.md#section` is a valid link to an existing file, and
        # checking the whole string would refuse it. The URL keeps both.
        path_part = target.split("#", 1)[0].split("?", 1)[0]
        if not (root / path_part).exists():
            raise RefuseError(
                f"refusing to rewrite link target {target!r}: "
                f"no such file or directory under {root}"
            )
        new_target = f"https://github.com/{repo}/blob/{ref}/{target}"
        whole = m.group(0)
        # Replace only the parenthesised target, preserving the link
        # text (and the leading "!" for an image link) exactly.
        open_paren = whole.rindex("(")
        return f"{whole[:open_paren]}({new_target})"

    return LINK_RE.sub(replace, line)


def rewrite_links(lines: list[str], repo: str, ref: str, root: Path) -> list[str]:
    out = []
    in_fence = False
    for line in lines:
        if FENCE_RE.match(line):
            in_fence = not in_fence
            out.append(line)
            continue
        if in_fence:
            out.append(line)
            continue
        out.append(rewrite_line(line, repo, ref, root))
    return out


def extract(changelog: Path, version: str, repo: str, ref: str, root: Path) -> str:
    """Returns the rewritten section body, or raises `RefuseError` /
    `LookupError` (missing/empty section) for `main` to report."""
    text = changelog.read_text(encoding="utf-8")
    lines = extract_section_lines(text, version)
    if lines is None:
        raise LookupError(f"no '## [{version}]' section found in {changelog}")
    lines = trim_blank(lines)
    if not lines:
        raise LookupError(f"the '## [{version}]' section in {changelog} is empty")
    lines = rewrite_links(lines, repo, ref, root)
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--changelog", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--repo", required=True, help="owner/repo, e.g. nabbisen/sui-id")
    parser.add_argument("--ref", required=True, help="git ref link targets are pinned to")
    parser.add_argument(
        "--root", required=True, help="checkout root a rewritten target must exist under"
    )
    args = parser.parse_args(argv)

    try:
        body = extract(Path(args.changelog), args.version, args.repo, args.ref, Path(args.root))
    except LookupError as exc:
        print(f"extract-changelog-section: {exc}", file=sys.stderr)
        return 1
    except RefuseError as exc:
        print(
            f"extract-changelog-section: {exc} -- "
            f"{args.changelog}, version {args.version!r}",
            file=sys.stderr,
        )
        return 1

    sys.stdout.write(body)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
