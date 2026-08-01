#!/usr/bin/env python3.14
"""RFC 093 G11: RFC integrity checker.

Checks every Markdown RFC under rfcs/{proposed,accepted,done,archive}/ and
rfcs/README.md against RFC 093's RFC-integrity contract:

  1. one unique identifier across lifecycle folders (two independent
     numbering namespaces: standard `NNN-slug.md` and `RFC-MI-NNN-slug.md`);
  2. folder and Status agreement;
  3. every RFC indexed exactly once at a resolvable relative path in
     rfcs/README.md's "## Index" section;
  4. every relative Markdown link in an RFC (and in rfcs/README.md) resolves;
  5. no RFC file exists directly under rfcs/ (only inside a lifecycle
     folder);
  6. every standard numeric RFC with identifier >= --policy's
     metadata_required_from.min_number has the full required metadata
     field set, regardless of folder or git history;
  7. every RFC-MI-* identifier not in --policy's closed historical list is
     prospective and requires the same full field set as (6);
  9. Accepted RFCs have acceptance metadata, and when Security review is
     Required, an Independent design review field with a durable
     repository-relative reference;
  10. Done RFCs with identifier >= the same threshold and Security review
      Required have dated Closure metadata and a durable repository-
      relative Closure evidence reference;
  11. RFCs below the threshold are checked for (1)-(5) only -- no
      retrospective reviewer is invented for them.

(Numbering above matches RFC 093's own list; item 8 is not an
independently-checkable invariant -- it is subsumed by (9), which is what
actually fires once a Proposed RFC moves to Accepted.)

Metadata is recognized only from bold, period-terminated labels
(`**Label.** value`) in the RFC header -- from the title line up to but
excluding the first level-2 (`## `) heading. This deliberately excludes
illustrative template examples inside RFC bodies (rfcs/done/000 and
rfcs/done/018 both embed example metadata blocks as prose, not real
headers) and any content inside fenced code blocks.

One narrowed exception: for an RFC-MI-* identifier already on --policy's
closed historical list, a fenced ```toml front-matter block's `status`
key supplies the Status field (folder/status agreement only -- historical
MI identifiers remain exempt from the full metadata field set). This does
not generalize to RFC-MI-* identifiers off that list, so a *new* MI RFC
must still use the bold-label convention to pass.

`Independent design review` and `Closure evidence` carry an evidence rule:
the first Markdown link in the field's value must resolve relative to the
RFC, be tracked (`git ls-files --error-unmatch`), and not be ignored
(`git check-ignore`). An absolute local path, an external-only reference,
or a missing target all fail.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path

LIFECYCLE_FOLDERS = ("proposed", "accepted", "done", "archive")

STANDARD_RE = re.compile(r"^(\d{3})-.+\.md$")
MI_RE = re.compile(r"^RFC-MI-(\d{3})-.+\.md$")

LABEL_RE = re.compile(r"^\*\*([A-Za-z][A-Za-z0-9 /'-]*)\.\*\*[ \t]*(.*)$")

LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
INLINE_CODE_RE = re.compile(r"`[^`]*`")
HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*#*$")
SCHEME_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")

REQUIRED_METADATA_FIELDS = (
    "Security review",
    "Design prerequisites",
    "Implementation prerequisites",
    "Closure prerequisites",
    "Tracks",
    "Touches",
    "Accountable owner and approver",
)

FOLDER_STATUS_PREFIXES = {
    "proposed": ("Proposed",),
    "accepted": ("Accepted",),
    "done": ("Implemented",),
    "archive": ("Withdrawn", "Superseded"),
}


class Rfc:
    def __init__(self, path: Path, folder: str, namespace: str, number: int):
        self.path = path
        self.folder = folder
        self.namespace = namespace  # "standard" or "mi"
        self.number = number
        self.identifier = (
            f"RFC-MI-{number:03d}" if namespace == "mi" else f"{number:03d}"
        )
        self.header_lines: list[str] = []
        self.fields: dict[str, list[str]] = {}

    def field(self, label: str) -> str | None:
        """First occurrence's value, or None if the label is absent."""
        values = self.fields.get(label)
        return values[0] if values else None


def slugify(heading_text: str) -> str:
    text = heading_text.strip().lower()
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"\s+", "-", text).strip("-")
    return text


FENCE_MARKER_RE = re.compile(r"^(`{3,}|~{3,})(.*)$")
FENCE_CLOSER_RE = re.compile(r"^(`+|~+)\s*$")


def iter_unfenced_lines(text: str) -> list[tuple[int, str]]:
    """Yields (1-based lineno, line) for lines outside fenced code
    blocks, using real CommonMark fence-closing rules rather than a
    naive "any ``` line toggles" approach: a fence only closes on a line
    consisting solely of the same fence character, repeated at least as
    many times as the opener, with no trailing text. A line that merely
    *starts* with backticks/tildes but carries an info string (e.g. an
    inner ` ```sh ` example nested for illustration inside an outer
    ` ```markdown ` block, as in rfcs/done/076) is not a valid closer and
    does not end the block -- unlike a naive per-``` toggle, which would
    desync on exactly that pattern and silently stop scanning the rest
    of the file. Verified to produce identical results to the naive
    approach across every file check-markdown-links.py (G10b) scans
    today, so that shipped checker's results are unaffected; this file's
    corpus (rfcs/) does contain the nested-fence pattern that exposes
    the difference.
    """
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


def extract_heading_slugs(text: str) -> set[str]:
    slugs: set[str] = set()
    seen_counts: dict[str, int] = {}
    for _, line in iter_unfenced_lines(text):
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


def extract_links(text: str) -> list[tuple[int, str]]:
    """RFC prose sometimes contains regex/character-class snippets in
    inline code (e.g. `` `^[a-z0-9](-?[a-z0-9])*$` ``) that superficially
    match link syntax (`[...]`  immediately followed by `(...)`); inline
    code spans are masked out before searching a line for real links, on
    top of the existing fenced-code-block skip."""
    links: list[tuple[int, str]] = []
    for lineno, line in iter_unfenced_lines(text):
        line = INLINE_CODE_RE.sub("", line)
        for m in LINK_RE.finditer(line):
            links.append((lineno, m.group(1).strip()))
    return links


def parse_header(text: str) -> tuple[list[str], dict[str, list[str]]]:
    """Header = from the top of the file up to (excluding) the first
    level-2 heading. Returns the raw header lines and a label->values map
    built from `**Label.** value` lines within it."""
    header_lines: list[str] = []
    for line in text.splitlines():
        if re.match(r"^## ", line):
            break
        header_lines.append(line)

    fields: dict[str, list[str]] = {}
    for line in header_lines:
        m = LABEL_RE.match(line)
        if not m:
            continue
        label, value = m.group(1), m.group(2)
        fields.setdefault(label, []).append(value)
    return header_lines, fields


TOML_FENCE_RE = re.compile(r"```toml\n(.*?)\n```", re.DOTALL)


def parse_toml_frontmatter(header_text: str) -> dict | None:
    m = TOML_FENCE_RE.search(header_text)
    if not m:
        return None
    try:
        return tomllib.loads(m.group(1))
    except tomllib.TOMLDecodeError:
        return None


def discover_rfcs(root: Path, policy: dict, failures: list[str]) -> list[Rfc]:
    rfcs: list[Rfc] = []
    seen: dict[tuple[str, int], list[Path]] = {}

    rfcs_dir = root / "rfcs"
    for md in sorted(rfcs_dir.glob("*.md")):
        if md.name != "README.md":
            failures.append(f"stray RFC file directly under rfcs/: {md.relative_to(root)}")

    for folder in LIFECYCLE_FOLDERS:
        folder_dir = rfcs_dir / folder
        if not folder_dir.is_dir():
            continue
        for md in sorted(folder_dir.glob("*.md")):
            m_mi = MI_RE.match(md.name)
            m_std = STANDARD_RE.match(md.name)
            if m_mi:
                namespace, number = "mi", int(m_mi.group(1))
            elif m_std:
                namespace, number = "standard", int(m_std.group(1))
            else:
                failures.append(
                    f"unrecognized RFC filename (neither NNN-slug.md nor "
                    f"RFC-MI-NNN-slug.md): {md.relative_to(root)}"
                )
                continue
            rfc = Rfc(md, folder, namespace, number)
            text = md.read_text(encoding="utf-8")
            rfc.header_lines, rfc.fields = parse_header(text)
            # Narrowed TOML front-matter reading (design decision,
            # m1b-c2-rfc-integrity-checker-review-2026-08-01.md §4):
            # applies only to identifiers already on the closed
            # historical RFC-MI-* list, not to RFC-MI-* generally --
            # a *new* MI RFC must still use the bold-label convention.
            # This only ever supplies the Status field (folder/status
            # agreement, invariant 2); historical MI identifiers remain
            # exempt from the full metadata field set (invariant 7).
            if (
                rfc.namespace == "mi"
                and rfc.identifier in policy["historical_rfc_mi"]["ids"]
                and "Status" not in rfc.fields
            ):
                toml_data = parse_toml_frontmatter("\n".join(rfc.header_lines))
                if toml_data and "status" in toml_data:
                    rfc.fields["Status"] = [str(toml_data["status"])]
            rfcs.append(rfc)
            seen.setdefault((namespace, number), []).append(md)

    for (namespace, number), paths in seen.items():
        if len(paths) > 1:
            identifier = f"RFC-MI-{number:03d}" if namespace == "mi" else f"{number:03d}"
            rel = ", ".join(str(p.relative_to(root)) for p in paths)
            failures.append(
                f"duplicate identifier {identifier} across lifecycle folders: {rel}"
            )

    return rfcs


def check_folder_status_agreement(rfcs: list[Rfc], failures: list[str]) -> None:
    for rfc in rfcs:
        status = rfc.field("Status")
        rel = rfc.path
        if status is None:
            failures.append(f"{rel}: no Status field found in the RFC header")
            continue
        allowed = FOLDER_STATUS_PREFIXES[rfc.folder]
        if not any(status.startswith(prefix) for prefix in allowed):
            failures.append(
                f"{rel}: folder '{rfc.folder}' requires Status starting with "
                f"one of {allowed}, got '{status}'"
            )


TABLE_ROW_RE = re.compile(r"^\|\s*([^|]+?)\s*\|")


def check_index(root: Path, rfcs: list[Rfc], failures: list[str]) -> None:
    # RFC index tables are not confined to a single "## Index"-bounded
    # span: rfcs/README.md also carries a later, disjoint
    # "## UI-Security Contract" section with its own RFC table (RFCs
    # 088-092). Rather than track every such section by name, this counts
    # links inside table rows across the whole file.
    #
    # A table row is "this RFC's index entry" only when the row's own
    # first cell names its identifier -- a bare number ("025") for
    # standard RFCs, "MI-NNN" for RFC-MI-* ones. This is stricter than
    # "any link to the file anywhere in the document": prose
    # cross-references (e.g. "governed by [RFC 018](...)" outside any
    # table) and another RFC's own row incidentally linking to this one
    # (e.g. archive/007's "Superseded by [RFC 025](...)" cell) are not
    # index entries and must not be counted as one.
    readme = root / "rfcs" / "README.md"
    text = readme.read_text(encoding="utf-8")

    cell_key_to_rfc: dict[str, Rfc] = {}
    for rfc in rfcs:
        key = f"MI-{rfc.number:03d}" if rfc.namespace == "mi" else f"{rfc.number:03d}"
        cell_key_to_rfc[key] = rfc

    counts: dict[Path, int] = {rfc.path.resolve(): 0 for rfc in rfcs}

    for _, line in iter_unfenced_lines(text):
        if not line.lstrip().startswith("|"):
            continue
        m = TABLE_ROW_RE.match(line.strip())
        if not m:
            continue
        rfc = cell_key_to_rfc.get(m.group(1).strip())
        if rfc is None:
            continue
        row_line = INLINE_CODE_RE.sub("", line)
        for link_m in LINK_RE.finditer(row_line):
            raw_target = link_m.group(1).strip()
            if SCHEME_RE.match(raw_target):
                continue
            path_part = raw_target.split("#", 1)[0]
            if not path_part or path_part.startswith("/"):
                continue
            target = (readme.parent / path_part).resolve()
            if target == rfc.path.resolve():
                counts[target] += 1

    for rfc in rfcs:
        count = counts[rfc.path.resolve()]
        if count == 0:
            failures.append(f"{rfc.path}: not indexed in rfcs/README.md (no table row identifies it)")
        elif count > 1:
            failures.append(
                f"{rfc.path}: indexed {count} times in rfcs/README.md, expected exactly once"
            )


def check_links(root: Path, rfcs: list[Rfc], failures: list[str]) -> None:
    files = [rfc.path for rfc in rfcs] + [root / "rfcs" / "README.md"]
    for md_file in files:
        text = md_file.read_text(encoding="utf-8")
        heading_cache: dict[Path, set[str]] = {}

        def headings_of(p: Path) -> set[str]:
            if p not in heading_cache:
                heading_cache[p] = extract_heading_slugs(p.read_text(encoding="utf-8"))
            return heading_cache[p]

        for lineno, raw_target in extract_links(text):
            if SCHEME_RE.match(raw_target):
                continue
            path_part, _, fragment = raw_target.partition("#")
            if path_part == "":
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
            if not target.exists():
                failures.append(f"{md_file}:{lineno}: target does not exist: {path_part}")
                continue
            if fragment and target.suffix == ".md":
                if fragment not in headings_of(target):
                    failures.append(
                        f"{md_file}:{lineno}: bad anchor: #{fragment} not found in {path_part}"
                    )


def requires_full_metadata(rfc: Rfc, policy: dict) -> bool:
    if rfc.namespace == "standard":
        return rfc.number >= policy["metadata_required_from"]["min_number"]
    return rfc.identifier not in policy["historical_rfc_mi"]["ids"]


def check_required_metadata(rfcs: list[Rfc], policy: dict, failures: list[str]) -> None:
    for rfc in rfcs:
        if not requires_full_metadata(rfc, policy):
            continue
        for label in REQUIRED_METADATA_FIELDS:
            value = rfc.field(label)
            if value is None or value.strip() == "":
                failures.append(
                    f"{rfc.path}: missing required metadata field '{label}.' "
                    f"(identifier {rfc.identifier} requires full metadata)"
                )


def extract_evidence_link(value: str) -> str | None:
    m = LINK_RE.search(value)
    return m.group(1).strip() if m else None


def check_evidence_field(root: Path, rfc: Rfc, label: str, failures: list[str]) -> None:
    value = rfc.field(label)
    if value is None or value.strip() == "":
        return  # presence is checked by the caller; this only checks quality
    link = extract_evidence_link(value)
    if link is None:
        failures.append(
            f"{rfc.path}: '{label}.' has no repository-relative Markdown link "
            f"(external-only or plain-text references are not durable evidence)"
        )
        return
    if SCHEME_RE.match(link):
        failures.append(f"{rfc.path}: '{label}.' link is external-only: {link}")
        return
    if link.startswith("/"):
        failures.append(f"{rfc.path}: '{label}.' link is an absolute local path: {link}")
        return
    target = (rfc.path.parent / link).resolve()
    if not target.exists():
        failures.append(f"{rfc.path}: '{label}.' link target does not exist: {link}")
        return
    try:
        rel = target.relative_to(root.resolve())
    except ValueError:
        failures.append(f"{rfc.path}: '{label}.' link resolves outside the repository: {link}")
        return
    # check-ignore before ls-files, deliberately: git does not report an
    # already-tracked path as ignored even when a later .gitignore
    # pattern matches it (verified directly -- `git check-ignore` on a
    # force-added, now-pattern-matched file exits 1, "not ignored"), so
    # checking tracked-status first would make the ignored branch below
    # unreachable for the realistic case (an untracked path under
    # .git-exclude/, which check-ignore correctly flags before ls-files
    # would even get to say "not tracked").
    ignored = subprocess.run(
        ["git", "-C", str(root), "check-ignore", str(rel)],
        capture_output=True,
    )
    if ignored.returncode == 0:
        failures.append(f"{rfc.path}: '{label}.' target is gitignored: {rel}")
        return
    tracked = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--error-unmatch", str(rel)],
        capture_output=True,
    )
    if tracked.returncode != 0:
        failures.append(f"{rfc.path}: '{label}.' target is not tracked by git: {rel}")


def check_accepted_metadata(root: Path, rfcs: list[Rfc], failures: list[str]) -> None:
    for rfc in rfcs:
        status = rfc.field("Status")
        if status is None or not status.startswith("Accepted"):
            continue
        for label in ("Accepted on", "Approved by"):
            value = rfc.field(label)
            if value is None or value.strip() == "":
                failures.append(f"{rfc.path}: Accepted RFC is missing '{label}.'")
        security = rfc.field("Security review")
        if security is not None and security.startswith("Required"):
            value = rfc.field("Independent design review")
            if value is None or value.strip() == "":
                failures.append(
                    f"{rfc.path}: Security review is Required but "
                    f"'Independent design review.' is missing"
                )
            else:
                check_evidence_field(root, rfc, "Independent design review", failures)


def check_closure_metadata(root: Path, rfcs: list[Rfc], policy: dict, failures: list[str]) -> None:
    threshold = policy["metadata_required_from"]["min_number"]
    for rfc in rfcs:
        if rfc.namespace != "standard" or rfc.number < threshold:
            continue
        if rfc.folder != "done":
            continue
        security = rfc.field("Security review")
        if security is None or not security.startswith("Required"):
            continue
        for label in ("Closure reviewed on", "Closure approved by"):
            value = rfc.field(label)
            if value is None or value.strip() == "":
                failures.append(
                    f"{rfc.path}: Done, security-sensitive, identifier >= {threshold} "
                    f"but missing '{label}.'"
                )
        value = rfc.field("Closure evidence")
        if value is None or value.strip() == "":
            failures.append(
                f"{rfc.path}: Done, security-sensitive, identifier >= {threshold} "
                f"but missing 'Closure evidence.'"
            )
        else:
            check_evidence_field(root, rfc, "Closure evidence", failures)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--policy", required=True)
    args = parser.parse_args(argv)

    root = Path(args.root)
    policy_path = root / args.policy
    with policy_path.open("rb") as f:
        policy = tomllib.load(f)

    failures: list[str] = []
    rfcs = discover_rfcs(root, policy, failures)
    check_folder_status_agreement(rfcs, failures)
    check_index(root, rfcs, failures)
    check_links(root, rfcs, failures)
    check_required_metadata(rfcs, policy, failures)
    check_accepted_metadata(root, rfcs, failures)
    check_closure_metadata(root, rfcs, policy, failures)

    if failures:
        for line in failures:
            print(f"check-rfc-integrity: {line}", file=sys.stderr)
        print(
            f"check-rfc-integrity: failed with {len(failures)} violation(s)",
            file=sys.stderr,
        )
        return 1

    print("check-rfc-integrity: all conditions satisfied")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
