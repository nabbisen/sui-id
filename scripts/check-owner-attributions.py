#!/usr/bin/env python3.14
"""RFC 117 stage 0 (G16): the owner-attribution census and its closed baseline.

This repository has recorded decisions as `@nabbisen`'s that he did not make,
three times (RFC 117 §Summary). Each was one commit that nobody looked at. This
gate makes such a sentence impossible to add *unseen*:

  1. **Census.** It finds every sentence, in every tracked text file, that
     attributes something to the owner: an owner token (`@nabbisen`, `owner`)
     and a decision-verb stem (`rul`, `decid`, `decision`, `authori[sz]`,
     `approv`, `accept`, `direct`, `instruct`) in the *same sentence*,
     **regardless of any date**. The match is deliberately generous: RFC 117's
     first draft used a narrow pattern that misses the 2026-09-09 incident, one
     of the three it was written to catch. A false positive costs a baseline
     line; a false negative is the failure this gate exists to prevent.
  2. **Every hit is printed**, to the log and to the GitHub step summary, new
     ones first.
  3. **Closed baseline.** `ci/owner-attribution-baseline.txt` records, per
     (path, normalised sentence hash), how many such sentences existed when the
     gate was adopted. The cutoff is the *tree*, not a date written in a
     sentence, which an author controls: an attribution that is not in the
     baseline fails whatever date it names, or none.
  4. **Fail** on any hit that is not in the baseline (or that occurs more times
     than the baseline records). Clearing a false positive is an edit to the
     baseline file, which shows up in the diff, not a marker an author types
     into their own paragraph.

What this does not do (stated because RFC 117 exists to stop overclaiming):

  * It does not verify that an owner decision is real. Nothing here can; that is
    what the ledger and its signatures (stages 1-3) are for.
  * **Citations are not honoured yet.** RFC 117 lets a sentence cite a ledger
    entry instead of being baselined. There is no ledger until stage 1, and a
    citation checked against an unsigned file would be a hole, so stage 0 has no
    citation path at all: a new attribution is baselined by a visible edit, or
    it fails.
  * It does not stop an agent that edits this script, its policy or the
    baseline in the same commit. The verifier lives in the tree it verifies;
    only review of changes to those paths closes that (RFC 117 §The claim).
  * It matches text. An attribution phrased with no owner token, or in an image,
    is not seen.

  5. **What the baseline edit changed is printed above the census** (stage 0b).
     A commit that adds an attribution and its baseline line passes with "0 new",
     so the only trace of the clearing is the diff of the baseline file. The gate
     therefore finds the base revision (a push's `before`, a pull request's base,
     or `--base`) and prints `git diff <base> -- <baseline>` under its own
     heading, first, so what clears a hit is shown beside the hit it clears. When
     there is no base (a first push, a new tag, a manual or detached run, a base
     that is not in this clone) it says so and why, rather than failing or
     printing nothing. It never changes the exit code: it is evidence, not a gate.

Usage:
  check-owner-attributions.py --root . --policy ci/owner-attributions.toml
  ... --base REV             the revision the baseline diff is taken against
                             (default: from the GitHub event payload, if any)
  ... --update-baseline      rewrite the baseline from the current tree
  ... --rev REV              scan the tree at REV instead of the working tree
  ... --baseline-rev REV0    take the baseline from the tree at REV0 (replaying a
                             historic commit: was this attribution *new*?)
Exit 0 = every attribution is baselined. 1 = a new one. 2 = usage or policy error.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import os
import re
import subprocess
import sys
import tomllib

MAX_FILE_BYTES = 2_000_000

# A comment line in a code or config file: `//`, `///`, `//!`, `#`, `--`.
_COMMENT_RE = re.compile(r"^\s*(?://+!?|#|--)\s?(.*)$")
# A sentence boundary: terminal punctuation, then a capital, quote, bracket or
# markdown span opener.
_SENTENCE_RE = re.compile(r"(?<=[.!?])\s+(?=[A-Z\"'`*(\[])")
_STRUCTURAL_PREFIXES = ("|", "#", "- ", "* ", "> ")
_LIST_NUMBER_RE = re.compile(r"^\s*\d+[.)]\s")


class PolicyError(Exception):
    pass


def load_policy(path: str) -> dict:
    try:
        with open(path, "rb") as fh:
            raw = tomllib.load(fh)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise PolicyError(f"cannot read policy {path}: {exc}") from exc
    try:
        policy = {
            "baseline": raw["baseline"],
            "markdown": tuple(raw["scan"]["markdown_extensions"]),
            "comments": tuple(raw["scan"]["comment_extensions"]),
            "owner": re.compile(raw["match"]["owner"]),
            "verb": re.compile(raw["match"]["verb"], re.IGNORECASE),
            "excerpt": int(raw["report"]["excerpt_chars"]),
        }
    except (KeyError, TypeError, ValueError, re.error) as exc:
        raise PolicyError(f"policy {path} is malformed: {exc!r}") from exc
    if raw.get("version") != 1:
        raise PolicyError("policy: version must be 1")
    return policy


def normalise(sentence: str) -> str:
    """The text a sentence is identified by: markdown decoration removed and
    whitespace collapsed, so re-wrapping or re-emphasising does not change it and
    a changed word does."""
    s = re.sub(r"[`*_>|]+", " ", sentence)
    s = _LIST_NUMBER_RE.sub("", s)
    s = re.sub(r"^\s*[-+]\s+", "", s)
    return re.sub(r"\s+", " ", s).strip()


def _units_from_paragraph(lines: list[str]) -> list[str]:
    """Split one blank-line-delimited block into units: a table row, a heading
    and a list item are each their own unit; wrapped prose lines are joined."""
    units: list[str] = []
    buf: list[str] = []

    def flush() -> None:
        if buf:
            units.append(" ".join(x.strip() for x in buf))
            buf.clear()

    for line in lines:
        stripped = line.lstrip()
        structural = stripped.startswith(_STRUCTURAL_PREFIXES) or bool(
            _LIST_NUMBER_RE.match(line)
        )
        if structural and buf:
            flush()
        buf.append(line)
        if stripped.startswith(("|", "#")):
            flush()
    flush()
    return units


def sentences_markdown(text: str) -> list[str]:
    out: list[str] = []
    for block in re.split(r"\n\s*\n", text):
        for unit in _units_from_paragraph(block.split("\n")):
            out.extend(_SENTENCE_RE.split(unit))
    return out


def sentences_comments(text: str) -> list[str]:
    """Sentences from the comment runs of a code or config file. Only comments
    are read: a string literal is not an attribution."""
    out: list[str] = []
    run: list[str] = []
    for line in text.split("\n") + [""]:
        m = _COMMENT_RE.match(line)
        if m:
            run.append(m.group(1))
            continue
        if run:
            joined = " ".join(x.strip() for x in run)
            out.extend(_SENTENCE_RE.split(joined))
            run = []
    return out


def census(files: dict[str, str], policy: dict) -> list[tuple[str, str]]:
    """Every attribution in `files`, as (path, normalised sentence), in a
    stable order. A sentence counts when an owner token and a decision-verb
    stem are both in it."""
    hits: list[tuple[str, str]] = []
    for path in sorted(files):
        ext = path.rsplit(".", 1)[-1] if "." in os.path.basename(path) else ""
        if ext in policy["markdown"]:
            sentences = sentences_markdown(files[path])
        elif ext in policy["comments"]:
            sentences = sentences_comments(files[path])
        else:
            continue
        for sentence in sentences:
            norm = normalise(sentence)
            if policy["owner"].search(norm) and policy["verb"].search(norm):
                hits.append((path, norm))
    return hits


def digest(sentence: str) -> str:
    return hashlib.sha256(sentence.encode("utf-8")).hexdigest()


def tally(hits: list[tuple[str, str]]) -> collections.Counter:
    return collections.Counter((p, digest(s)) for p, s in hits)


# ── reading a tree ────────────────────────────────────────────────────────


def _git(root: str, *args: str, stdin: bytes | None = None) -> bytes:
    res = subprocess.run(
        ["git", "-C", root, *args], input=stdin, capture_output=True, check=False
    )
    if res.returncode != 0:
        raise PolicyError(f"git {' '.join(args)} failed: {res.stderr.decode().strip()}")
    return res.stdout


def _decode(data: bytes) -> str | None:
    if len(data) > MAX_FILE_BYTES:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def read_worktree(root: str) -> dict[str, str]:
    """Tracked and untracked-but-not-ignored files: what a clean checkout of the
    commit about to be made would contain."""
    names = _git(
        root, "ls-files", "--cached", "--others", "--exclude-standard", "-z"
    ).split(b"\0")
    files: dict[str, str] = {}
    for raw in names:
        if not raw:
            continue
        rel = raw.decode("utf-8", "surrogateescape")
        full = os.path.join(root, rel)
        if not os.path.isfile(full):
            continue
        with open(full, "rb") as fh:
            text = _decode(fh.read())
        if text is not None:
            files[rel] = text
    return files


def read_rev(root: str, rev: str) -> dict[str, str]:
    listing = _git(root, "ls-tree", "-r", "-z", "--name-only", rev).split(b"\0")
    names = [n.decode("utf-8", "surrogateescape") for n in listing if n]
    if not names:
        return {}
    request = "".join(f"{rev}:{n}\n" for n in names).encode()
    blob = _git(root, "cat-file", "--batch", stdin=request)
    files: dict[str, str] = {}
    pos = 0
    for name in names:
        eol = blob.index(b"\n", pos)
        header = blob[pos:eol].split()
        if len(header) < 3:  # "<spec> missing"
            pos = eol + 1
            continue
        size = int(header[2])
        data = blob[eol + 1 : eol + 1 + size]
        pos = eol + 1 + size + 1
        text = _decode(data)
        if text is not None:
            files[name] = text
    return files


# ── the baseline ──────────────────────────────────────────────────────────

_BASELINE_HEADER = """\
# RFC 117 stage 0: the closed baseline of owner attributions.
#
# One line per (path, sentence): PATH <TAB> SHA-256 of the normalised sentence
# <TAB> how many times it occurs <TAB> the sentence's first characters, for
# people reading a diff. Only the first three fields are compared.
#
# This is the set of attributions that existed when the gate was adopted. It is
# CLOSED: a new attribution that is not here fails scripts/check-owner-
# attributions.py (G16). Adding a line is the way to clear a false positive, and
# it is a visible edit to this file, reviewed like any other. Do not add a line
# for an attribution you cannot point to the owner's own words for.
"""


def read_baseline(path: str) -> collections.Counter:
    counts: collections.Counter = collections.Counter()
    try:
        with open(path, encoding="utf-8") as fh:
            lines = fh.read().split("\n")
    except OSError as exc:
        raise PolicyError(f"cannot read baseline {path}: {exc}") from exc
    for n, line in enumerate(lines, 1):
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 3 or not re.fullmatch(r"[0-9a-f]{64}", parts[1]):
            raise PolicyError(f"{path}:{n}: malformed baseline line")
        try:
            count = int(parts[2])
        except ValueError as exc:
            raise PolicyError(f"{path}:{n}: count is not an integer") from exc
        key = (parts[0], parts[1])
        if key in counts:
            raise PolicyError(f"{path}:{n}: duplicate baseline entry")
        counts[key] = count
    return counts


def write_baseline(path: str, hits: list[tuple[str, str]], excerpt: int) -> int:
    first: dict[tuple[str, str], str] = {}
    for p, s in hits:
        first.setdefault((p, digest(s)), s)
    counts = tally(hits)
    lines = [_BASELINE_HEADER]
    for key in sorted(counts):
        text = first[key][:excerpt].replace("\t", " ")
        lines.append(f"{key[0]}\t{key[1]}\t{counts[key]}\t{text}\n")
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("".join(lines))
    return len(counts)


# ── what the baseline edit changed (stage 0b) ─────────────────────────────

_NULL_SHA = "0" * 40


def base_from_event(env: dict) -> tuple[str | None, str]:
    """The base revision named by the GitHub event payload, and how it was
    found. `(None, reason)` when the event names none: a first push or a new
    tag has a `before` of forty zeros, and a manual or scheduled run has no
    base at all."""
    name = env.get("GITHUB_EVENT_NAME", "")
    path = env.get("GITHUB_EVENT_PATH", "")
    if not name or not path:
        return None, "this is not a GitHub event run"
    try:
        with open(path, encoding="utf-8") as fh:
            event = json.load(fh)
    except (OSError, ValueError) as exc:
        return None, f"the event payload could not be read ({exc.__class__.__name__})"
    if name == "pull_request":
        sha = (event.get("pull_request") or {}).get("base", {}).get("sha")
        return (sha, "the pull request's base") if sha else (None, "the pull request payload names no base")
    if name == "push":
        sha = event.get("before")
        if not sha or sha == _NULL_SHA:
            return None, "this push has no previous commit (a first push, a new branch or a new tag)"
        return sha, "the push's previous commit"
    return None, f"a `{name}` event has no base revision"


def resolve_base(root: str, explicit: str | None, env: dict) -> tuple[str | None, str]:
    """(revision, how) or (None, why not). The revision is verified to be a
    commit in this clone: a base that is not here (a shallow checkout, a rewritten
    history) cannot be diffed against, and saying so is the point."""
    if explicit:
        sha, how = explicit, "`--base`"
    else:
        sha, how = base_from_event(env)
        if sha is None:
            return None, how
    res = subprocess.run(
        ["git", "-C", root, "rev-parse", "--verify", "--quiet", f"{sha}^{{commit}}"],
        capture_output=True,
        check=False,
    )
    if res.returncode != 0:
        return None, f"the base {sha[:12]} ({how}) is not a commit in this clone (a shallow checkout, or a rewritten history)"
    return res.stdout.decode().strip(), how


def render_baseline_section(root: str, baseline_rel: str, base: str | None, how: str) -> str:
    """The `## Baseline changes` section: the diff of the baseline file against
    the base, an explicit 'unchanged' when there is none, or a stated reason when
    there is no base."""
    head = ["## Baseline changes (RFC 117 stage 0b)", ""]
    if base is None:
        return "\n".join(
            head
            + [
                f"**No base revision, so what this commit changed in `{baseline_rel}` cannot be shown:** {how}.",
                "The census below is complete, but a baseline edit made in this change is not "
                "displayed here. Read the diff of that file directly.",
                "",
            ]
        )
    diff = _git(root, "diff", "--no-color", base, "--", baseline_rel).decode(
        "utf-8", "replace"
    )
    if not diff.strip():
        return "\n".join(
            head
            + [
                f"`{baseline_rel}` is unchanged since `{base[:12]}` ({how}). No attribution was cleared by an edit to the baseline.",
                "",
            ]
        )
    lines = diff.split("\n")
    added = sum(1 for l in lines if l.startswith("+") and not l.startswith("+++"))
    removed = sum(1 for l in lines if l.startswith("-") and not l.startswith("---"))
    return "\n".join(
        head
        + [
            f"`{baseline_rel}` **changed** since `{base[:12]}` ({how}): "
            f"**{added} line(s) added**, {removed} removed. Each added line clears an "
            "attribution that would otherwise fail this gate. **Read them.** The text after "
            "the third tab is the sentence.",
            "",
            "```diff",
            diff.rstrip("\n"),
            "```",
            "",
        ]
    )


# ── the verdict ───────────────────────────────────────────────────────────


def classify(hits, baseline):
    """Split hits into (new, baselined). A sentence occurring more often than
    the baseline records is new for the surplus."""
    remaining = collections.Counter(baseline)
    new, known = [], []
    for path, sentence in hits:
        key = (path, digest(sentence))
        if remaining[key] > 0:
            remaining[key] -= 1
            known.append((path, sentence))
        else:
            new.append((path, sentence))
    return new, known


def render(new, known, stale: int, excerpt: int) -> str:
    out = [
        "## Owner attributions (RFC 117 stage 0, G16)",
        "",
        f"{len(new) + len(known)} attributions found: **{len(new)} new**, "
        f"{len(known)} in the closed baseline"
        + (f", {stale} baseline entries no longer present (prune when convenient)" if stale else "")
        + ".",
        "",
    ]
    if new:
        out += [
            "### New: not in the baseline",
            "",
            "Each of these attributes something to the owner and was not there when "
            "the gate was adopted. **Read them.** If the owner said it, it belongs "
            "in the ledger once one exists; until then the sentence is added to "
            "`ci/owner-attribution-baseline.txt` in a visible edit, or removed.",
            "",
        ]
        out += [f"- `{p}`: {s[:excerpt]}" for p, s in new]
        out.append("")
    out += ["<details><summary>Every attribution in the baseline</summary>", ""]
    out += [f"- `{p}`: {s[:excerpt]}" for p, s in known]
    out += ["", "</details>", ""]
    return "\n".join(out)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--root", default=".")
    ap.add_argument("--policy", required=True)
    ap.add_argument("--rev", help="scan the tree at this revision")
    ap.add_argument("--baseline-rev", help="take the baseline from the tree at this revision")
    ap.add_argument("--base", help="the revision the baseline diff is taken against")
    ap.add_argument("--update-baseline", action="store_true")
    args = ap.parse_args(argv)

    try:
        policy = load_policy(os.path.join(args.root, args.policy))
        files = read_rev(args.root, args.rev) if args.rev else read_worktree(args.root)
        hits = census(files, policy)
        baseline_path = os.path.join(args.root, policy["baseline"])

        if args.update_baseline:
            n = write_baseline(baseline_path, hits, policy["excerpt"])
            print(f"owner-attributions: wrote {n} baseline entries ({len(hits)} attributions)")
            return 0

        if args.baseline_rev:
            base_files = read_rev(args.root, args.baseline_rev)
            baseline = tally(census(base_files, policy))
        else:
            baseline = read_baseline(baseline_path)
    except PolicyError as exc:
        print(f"owner-attributions: {exc}", file=sys.stderr)
        return 2

    new, known = classify(hits, baseline)
    seen = tally(hits)
    stale = sum(1 for key in baseline if key not in seen)
    report = render(new, known, stale, policy["excerpt"])
    if not args.rev and not args.baseline_rev:
        # The baseline diff comes first: what clears a hit is printed above the
        # census it changes (stage 0b). Evidence only; it never sets the exit code.
        base, how = resolve_base(args.root, args.base, os.environ)
        report = render_baseline_section(args.root, policy["baseline"], base, how) + "\n" + report
    print(report)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as fh:
            fh.write(report + "\n")
    if new:
        print(
            f"owner-attributions: FAIL, {len(new)} attribution(s) not in the baseline",
            file=sys.stderr,
        )
        return 1
    print(f"owner-attributions: all {len(known)} attributions are in the baseline")
    return 0


if __name__ == "__main__":
    sys.exit(main())
