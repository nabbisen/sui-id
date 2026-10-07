#!/usr/bin/env python3.14
"""RFC 138 D1: the one definition of a per-hunk hash, as a tool rather than a
sentence copied between dispatches.

    python3.14 scripts/hunk-hashes.py --baseline <rev> [--root <path>] [paths...]

Prints, per changed path, Markdown ready to paste into a package's "Hashes"
section: the hunk count and one SHA-256 per hunk for a modified file, and a
labelled full-content hash for every file this change adds.

**The rule.** A hunk is its `@@` header line followed by its body lines, each
line including its terminating newline, the last one included. Hunks are cut
at `@@` headers; no newline belongs to a boundary. The diff is `-U3 -M`
against the stated baseline; `-M` detects a rename regardless of the caller's
own `diff.renames` setting. The `diff --git`/`index`/`---`/`+++` preamble belongs to
no hunk.

**Why this rule and not the retired one.** The retired method split the raw
diff text on the literal string `"\\n@@"`. That consumes the newline ending
every hunk except the last, so a hunk's hash depended on whether another hunk
followed it in the same file -- exactly the property a reproducible hash must
not have. Hashing each hunk as a self-contained slice (header line plus body
lines, each kept whole) means a hunk's bytes are the same whether it is first,
last, or alone.

**What each shape of change gets, stated rather than left to be found:**

- **Modified** (tracked, existing at the baseline and now): per-hunk hashes,
  under "Per-hunk SHA-256".
- **Added** (new at the baseline -- whether already `git add`ed or still
  untracked): one full-content SHA-256 of the current working-tree bytes,
  under "Full-content SHA-256 — new files". Hunks do not apply to a file that
  did not exist to diff against.
- **Deleted**: one SHA-256 of the file's content *at the baseline*, under
  "Deleted files", so a reviewer can confirm what was removed without the
  file being present to inspect.
- **Renamed, content unchanged** (`similarity index 100%`, no hunks in the
  diff): one SHA-256 of the unchanged content, under "Renamed files", citing
  both paths. The content hash is deliberately the same number a reviewer
  would get from the file at either path.
- **Renamed, content also changed**: per-hunk hashes under "Per-hunk
  SHA-256", same as a plain modification, with the old path noted in the
  heading.
- **Binary**: git's diff carries no hunks for a binary file (`git diff`
  without `--binary` only reports that the files differ). Full-content
  SHA-256 of the current bytes (or the baseline bytes, if deleted), under
  "Binary files".
- **A file whose last line has no trailing newline**: `git diff` emits a
  literal line `\\ No newline at end of file` immediately after the affected
  content line, inside the hunk. That line is part of the hunk's bytes under
  this rule, with no exception carved out for it -- so a file that gains or
  loses a trailing newline changes that hunk's hash, which is correct: the
  bytes changed.
- **CRLF content**: this tool reads every git subprocess's stdout as bytes,
  never through a text-mode decode that could normalise line endings. A `\\r`
  in the diff is hashed as a `\\r`.

No CI gate reads this tool's output (RFC 138's non-goal): it is run by a
person or role producing or checking a package, not a lane.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
from pathlib import Path


def git(root: Path, *args: str) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        capture_output=True,
        check=True,
    )
    return result.stdout


def git_text(root: Path, *args: str) -> str:
    return git(root, *args).decode("utf-8", errors="surrogateescape")


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def split_hunks(body: bytes) -> list[bytes]:
    """Split one file's diff body (everything after the +++ line) into
    self-contained hunks: each a `@@` header line plus its body lines, every
    line keeping its own trailing newline. The final hunk keeps whatever the
    diff gave it, trailing newline or not."""
    lines = body.splitlines(keepends=True)
    hunks: list[list[bytes]] = []
    for line in lines:
        if line.startswith(b"@@"):
            hunks.append([line])
        elif hunks:
            hunks[-1].append(line)
        # A line before the first "@@" inside this slice should not occur;
        # if it does (a malformed diff), it is silently dropped rather than
        # mis-attributed to a hunk it is not part of.
    return [b"".join(h) for h in hunks]


class FileChange:
    def __init__(self, path: str) -> None:
        self.path = path
        self.old_path: str | None = None
        self.status: str = "modified"  # modified | added | deleted | renamed | binary
        self.hunks: list[bytes] = []
        self.binary = False


def parse_diff(raw: bytes) -> list[FileChange]:
    """Split a full `git diff` into per-file sections and classify each."""
    lines = raw.splitlines(keepends=True)
    sections: list[list[bytes]] = []
    for line in lines:
        if line.startswith(b"diff --git "):
            sections.append([line])
        elif sections:
            sections[-1].append(line)

    changes: list[FileChange] = []
    for section in sections:
        text = b"".join(section)
        new_path = None
        old_path = None
        # Every marker below is on its own line, and the section always
        # starts with "diff --git ", so none of them can be the first bytes
        # of `text` -- a plain substring search is exact, not an
        # approximation.
        is_new = b"\nnew file mode " in text
        is_deleted = b"\ndeleted file mode " in text
        is_binary = b"Binary files " in text
        rename_from = None
        rename_to = None
        header_old = None
        header_new = None

        for line in section:
            if line.startswith(b"diff --git a/"):
                rest = line[len(b"diff --git a/") :].rstrip(b"\n")
                # " b/" is the separator git itself uses between the two
                # sides. A path containing the literal bytes " b/" defeats
                # this, the same limitation git's own unquoted header has.
                sep = rest.find(b" b/")
                if sep != -1:
                    header_old = rest[:sep].decode("utf-8")
                    header_new = rest[sep + 3 :].decode("utf-8")
            elif line.startswith(b"rename from "):
                rename_from = line[len(b"rename from ") :].rstrip(b"\n").decode("utf-8")
            elif line.startswith(b"rename to "):
                rename_to = line[len(b"rename to ") :].rstrip(b"\n").decode("utf-8")
            elif line.startswith(b"+++ "):
                raw_path = line[4:].rstrip(b"\n")
                if raw_path != b"/dev/null":
                    new_path = raw_path[2:].decode("utf-8")  # strip "b/"
            elif line.startswith(b"--- "):
                raw_path = line[4:].rstrip(b"\n")
                if raw_path != b"/dev/null":
                    old_path = raw_path[2:].decode("utf-8")  # strip "a/"

        # A binary diff has no "---"/"+++" lines at all, so the header line
        # is the only source for its path; it is also a safe fallback for
        # anything else that reaches here without one.
        new_path = new_path or header_new
        old_path = old_path or header_old

        change = FileChange(new_path or rename_to or old_path or "")
        if rename_from and rename_to:
            change.old_path = rename_from
            change.path = rename_to
            change.status = "renamed"
        elif is_deleted:
            change.status = "deleted"
            change.path = old_path or change.path
        elif is_new:
            change.status = "added"
        else:
            change.status = "modified"

        idx = text.find(b"\n@@")
        if idx != -1:
            # Slice right after the newline, so the slice starts at "@@"
            # itself -- the preamble before it belongs to no hunk.
            change.hunks = split_hunks(text[idx + 1 :])

        if is_binary:
            change.binary = True

        changes.append(change)
    return changes


def render_markdown(
    baseline: str,
    modified: list[FileChange],
    added_paths: list[tuple[str, bytes]],
    deleted: list[tuple[str, bytes]],
    renamed_unchanged: list[tuple[str, str, bytes]],
    binary: list[tuple[str, bytes, str]],
) -> str:
    out: list[str] = []

    out.append(f"## Per-hunk SHA-256 — modified files, against `{baseline}`")
    out.append("")
    if modified:
        blocks = []
        for change in modified:
            heading = f"**`{change.path}`**"
            if change.old_path:
                heading += f" (renamed from `{change.old_path}`)"
            heading += f" — {len(change.hunks)} hunk(s)"
            rows = [f"| {i} | `{sha256_hex(h)}` |" for i, h in enumerate(change.hunks, 1)]
            blocks.append(
                "\n\n".join([heading, "| hunk | SHA-256 |\n|---|---|\n" + "\n".join(rows)])
            )
        out.append("\n\n".join(blocks))
    else:
        out.append("None.")
    out.append("")

    out.append("## Full-content SHA-256 — new files")
    out.append("")
    if added_paths:
        out.append("| file | SHA-256 |\n|---|---|")
        for path, data in added_paths:
            out.append(f"| `{path}` | `{sha256_hex(data)}` |")
    else:
        out.append("None.")
    out.append("")

    out.append("## Deleted files")
    out.append("")
    if deleted:
        out.append("| file | SHA-256 (baseline content) |\n|---|---|")
        for path, data in deleted:
            out.append(f"| `{path}` | `{sha256_hex(data)}` |")
    else:
        out.append("None.")
    out.append("")

    out.append("## Renamed files, content unchanged")
    out.append("")
    if renamed_unchanged:
        out.append("| from | to | SHA-256 |\n|---|---|---|")
        for old, new, data in renamed_unchanged:
            out.append(f"| `{old}` | `{new}` | `{sha256_hex(data)}` |")
    else:
        out.append("None.")
    out.append("")

    out.append("## Binary files, full content")
    out.append("")
    if binary:
        out.append("| file | SHA-256 | state |\n|---|---|---|")
        for path, data, state in binary:
            out.append(f"| `{path}` | `{sha256_hex(data)}` | {state} |")
    else:
        out.append("None.")

    return "\n".join(out).rstrip("\n") + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--root", default=".")
    parser.add_argument("paths", nargs="*")
    args = parser.parse_args(argv)
    root = Path(args.root)

    # -M: detect renames regardless of the caller's own `diff.renames`
    # config, so a rename is reported as one, not as a delete plus an add.
    diff_args = ["diff", "-U3", "-M", args.baseline, "--"]
    if args.paths:
        diff_args += args.paths
    else:
        diff_args += ["."]
    raw = git(root, *diff_args)
    changes = parse_diff(raw)

    modified: list[FileChange] = []
    added_paths: list[tuple[str, bytes]] = []
    deleted: list[tuple[str, bytes]] = []
    renamed_unchanged: list[tuple[str, str, bytes]] = []
    binary: list[tuple[str, bytes, str]] = []

    for change in changes:
        if change.binary:
            if change.status == "deleted":
                data = git(root, "show", f"{args.baseline}:{change.path}")
                binary.append((change.path, data, "deleted"))
            else:
                data = (root / change.path).read_bytes()
                binary.append((change.path, data, change.status))
            continue
        if change.status == "deleted":
            data = git(root, "show", f"{args.baseline}:{change.path}")
            deleted.append((change.path, data))
        elif change.status == "added":
            data = (root / change.path).read_bytes()
            added_paths.append((change.path, data))
        elif change.status == "renamed" and not change.hunks:
            data = (root / change.path).read_bytes()
            renamed_unchanged.append((change.old_path or "", change.path, data))
        else:
            modified.append(change)

    # Untracked files never appear in `git diff <baseline>`, no matter what
    # pathspec is given -- they are new relative to every baseline. Found
    # separately and reported as added, exactly like a new tracked file.
    ls_args = ["ls-files", "--others", "--exclude-standard", "--"]
    ls_args += args.paths if args.paths else ["."]
    untracked = git_text(root, *ls_args).splitlines()
    known = {p for p, _ in added_paths}
    for rel in untracked:
        if rel and rel not in known:
            added_paths.append((rel, (root / rel).read_bytes()))

    added_paths.sort(key=lambda pair: pair[0])
    modified.sort(key=lambda c: c.path)
    deleted.sort(key=lambda pair: pair[0])
    renamed_unchanged.sort(key=lambda t: t[1])
    binary.sort(key=lambda t: t[0])

    print(
        render_markdown(args.baseline, modified, added_paths, deleted, renamed_unchanged, binary),
        end="",
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
