#!/usr/bin/env python3.14
"""RFC 116 stage 6 (G18): what is written about the contracts directory must match
the directory.

The directory holds the machine-consumed contracts a gate compares against (RFC 098
D3). Before this gate, ~50 references to its files were backticked paths nothing
checked, so renaming it would have bought a better name at the price of fifty
silently wrong references. This gate is what makes a rename checkable. Two halves,
because they are one property:

  (paths)   Every `<directory>/<file>` path named in a file **in scope** exists. A
            file that still names a *former* directory's file is a violation that
            says where it moved. A name that is not a file in the directory is
            accepted only if the policy allows it, with a reason, and an allow entry
            no file uses any more is itself a violation.
  (readme)  Every file in the directory has a row in its README and every row names
            a file that exists; the Kind is in a closed vocabulary; every gate id in
            "Read by" is in `[gates]` **and** in the Gate Matrix (a row of the
            owning RFC's lane table); every script it names exists; every RFC it
            names exists; and the README names what the policy says it must (the
            developer's dispatcher, so it cannot describe the directory as CI's).
  (read)    A proxy for RFC 116 D2, "every file here is read by something that fails
            when it stops being true": every file is named by some file under
            `scripts/`, `.github/` or another file in the directory. It proves the
            file is referred to by a machine-read place, not that the reader fails
            when it should; that is each gate's own test.

**Scope** is data (contracts/contract-paths.toml), and it is by exemption: a file is
scanned unless a `[[record]]` entry exempts it, so a new top-level directory is
scanned by default. Records keep the path they had when they were written; rewriting
one to satisfy a gate would falsify it. Every exemption and every allowed name
carries a reason, and one without is a policy error.

Exit 0 = pass, 1 = any violation (each on stderr, prefixed `check-contracts:` and
naming the file and line), 2 = the policy or the tree cannot be read.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path

MAX_FILE_BYTES = 2_000_000
GATE_ID = re.compile(r"\bG\d+[a-z]?\b")
RFC_REF = re.compile(r"\bRFC (\d{3})\b")
BACKTICKED = re.compile(r"`([^`]+)`")


class PolicyError(Exception):
    pass


def load_policy(path: Path) -> dict:
    try:
        with path.open("rb") as fh:
            raw = tomllib.load(fh)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise PolicyError(f"cannot read policy {path}: {exc}") from exc
    try:
        policy = {
            "directory": str(raw["directory"]).strip("/"),
            "former": [str(d).strip("/") for d in raw.get("former_directories", [])],
            "manifest": raw["manifest"],
            "readme": raw["readme_file"],
            "extensions": tuple(raw["extensions"]),
            "kinds": tuple(raw_readme(raw)["kinds"]),
            "must_name": tuple(raw_readme(raw).get("must_name", [])),
            "records": list(raw.get("record", [])),
            "allow": list(raw.get("allow", [])),
        }
    except (KeyError, TypeError) as exc:
        raise PolicyError(f"policy {path} is malformed: {exc!r}") from exc
    if raw.get("version") != 1:
        raise PolicyError("policy: version must be 1")
    for kind, entries in (("record", policy["records"]), ("allow", policy["allow"])):
        for entry in entries:
            if not isinstance(entry.get("path"), str) or not entry["path"]:
                raise PolicyError(f"policy: a [[{kind}]] entry has no path")
            if not isinstance(entry.get("reason"), str) or not entry["reason"].strip():
                raise PolicyError(
                    f"policy: [[{kind}]] {entry['path']} has no reason; an exemption "
                    "without a reason is a hiding place, not an allow-list"
                )
    return policy


def raw_readme(raw: dict) -> dict:
    section = raw.get("readme")
    if not isinstance(section, dict):
        raise KeyError("readme")
    return section


# ── the tree ────────────────────────────────────────────────────────────


def read_tree(root: Path) -> dict[str, str]:
    """Tracked and untracked-but-not-ignored text files: what a clean checkout of
    the commit about to be made would contain."""
    res = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        capture_output=True,
    )
    if res.returncode != 0:
        raise PolicyError(f"git ls-files failed: {res.stderr.decode().strip()}")
    files: dict[str, str] = {}
    for raw in res.stdout.split(b"\0"):
        if not raw:
            continue
        rel = raw.decode("utf-8", "surrogateescape")
        full = root / rel
        if not full.is_file() or full.stat().st_size > MAX_FILE_BYTES:
            continue
        data = full.read_bytes()
        if b"\0" in data[:4096]:
            continue
        files[rel] = data.decode("utf-8", "replace")
    return files


def is_record(rel: str, records: list[dict]) -> bool:
    return any(rel == r["path"] or rel.startswith(r["path"].rstrip("/") + "/") for r in records)


# ── (paths) ─────────────────────────────────────────────────────────────


def reference_pattern(directories: list[str], extensions: tuple[str, ...]) -> re.Pattern[str]:
    dirs = "|".join(re.escape(d) for d in directories)
    exts = "|".join(re.escape(e) for e in extensions)
    # Not preceded by a path or word character, so `scripts/ci/x.toml`, `oci/x.toml`
    # and a URL's `/ci/x.toml` are other things, not this directory.
    return re.compile(rf"(?<![A-Za-z0-9_./~-])({dirs})/([A-Za-z0-9_.-]+\.(?:{exts}))\b")


def check_paths(policy: dict, files: dict[str, str], failures: list[str], own_policy: str) -> set[str]:
    directory, former = policy["directory"], policy["former"]
    pattern = reference_pattern([directory, *former], policy["extensions"])
    allowed = {a["path"]: a for a in policy["allow"]}
    used: set[str] = set()
    for rel in sorted(files):
        # The policy's own allow entries name the paths they allow; counting them as
        # uses would make every allowance permanently fresh.
        if is_record(rel, policy["records"]) or rel == own_policy:
            continue
        for n, line in enumerate(files[rel].split("\n"), start=1):
            for m in pattern.finditer(line):
                where, name = m.group(1), m.group(2)
                path = f"{where}/{name}"
                if path in allowed:
                    used.add(path)
                    continue
                if where in former:
                    moved = f"{directory}/{name}"
                    hint = f"; it moved to {moved}" if moved in files else "; and it is not in the directory either"
                    failures.append(f"(paths) {rel}:{n}: names {path}, a former directory{hint}")
                elif path not in files:
                    failures.append(
                        f"(paths) {rel}:{n}: names {path}, which does not exist "
                        "(add it to [[allow]] with a reason only if it is meant not to)"
                    )
    for entry in policy["allow"]:
        if entry["path"] not in used:
            failures.append(
                f"(paths) [[allow]] {entry['path']} is stale: no file in scope names it "
                f"(reason given: {entry['reason']})"
            )
    for entry in policy["records"]:
        prefix = entry["path"].rstrip("/")
        if not any(f == prefix or f.startswith(prefix + "/") for f in files):
            failures.append(f"(paths) [[record]] {entry['path']} matches no file; remove the exemption")
    return used


# ── (readme) ────────────────────────────────────────────────────────────


def split_row(line: str) -> list[str]:
    s = line.strip()
    s = s[1:] if s.startswith("|") else s
    s = s[:-1] if s.endswith("|") else s
    return [c.strip() for c in s.split("|")]


def readme_rows(text: str) -> list[tuple[int, list[str]]]:
    rows: list[tuple[int, list[str]]] = []
    header = False
    for n, line in enumerate(text.split("\n"), start=1):
        if not line.lstrip().startswith("|"):
            header = False
            continue
        cells = split_row(line)
        if re.fullmatch(r":?-{3,}:?", cells[0] if cells else ""):
            header = True
            continue
        if not header and cells and cells[0].lower() == "file":
            continue
        if header:
            rows.append((n, cells))
    return rows


def resolve_rfc(root: Path, num: str) -> list[Path]:
    found: list[Path] = []
    for folder in ("proposed", "accepted", "done", "archive"):
        found += sorted((root / "rfcs" / folder).glob(f"{num}-*.md"))
    return found


def gate_matrix_rows(root: Path, manifest: dict, lane: str, failures: list[str], where: str) -> bool:
    """Is `lane` in [gates] with an owner whose RFC table has a row for it?"""
    if lane not in manifest.get("gates", {}):
        failures.append(f"(readme) {where}: {lane} is not in [gates]")
        return False
    owner = manifest.get("gate_owners", {}).get(lane)
    heading = manifest.get("gate_lane_sources", {}).get(owner or "")
    if not owner or not heading:
        failures.append(f"(readme) {where}: {lane} has no owner in [gate_owners] or the owner is not a declared lane source")
        return False
    files = resolve_rfc(root, owner)
    if len(files) != 1:
        failures.append(f"(readme) {where}: {lane}'s owner RFC {owner} resolves to {len(files)} files")
        return False
    body, in_section = [], False
    for line in files[0].read_text(encoding="utf-8").split("\n"):
        if line.startswith("#"):
            in_section = line.lstrip("#").strip() == heading
            continue
        if in_section:
            body.append(line)
    if not any(re.match(rf"\|\s*{re.escape(lane)}\s*\|", l) for l in body):
        failures.append(
            f"(readme) {where}: {lane} is in [gates] but has no row in the Gate Matrix "
            f"(RFC {owner}, \"{heading}\")"
        )
        return False
    return True


def check_readme(policy: dict, root: Path, files: dict[str, str], failures: list[str]) -> None:
    directory = policy["directory"]
    readme = policy["readme"]
    if readme not in files:
        failures.append(f"(readme) {readme} does not exist")
        return
    text = files[readme]
    try:
        with (root / policy["manifest"]).open("rb") as fh:
            manifest = tomllib.load(fh)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        failures.append(f"(readme) cannot read the manifest {policy['manifest']}: {exc}")
        manifest = {}

    on_disk = sorted(
        f[len(directory) + 1 :]
        for f in files
        if f.startswith(directory + "/") and f != readme
    )
    rows = readme_rows(text)
    rowed: dict[str, int] = {}
    for n, cells in rows:
        where = f"{readme}:{n}"
        if len(cells) != 4:
            failures.append(f"(readme) {where}: {len(cells)} cells; a row is File | Kind | Read by | Owning RFC")
            continue
        file_cell, kind, read_by, owning = cells
        m = re.fullmatch(r"`([^`]+)`", file_cell)
        if not m:
            failures.append(f"(readme) {where}: the File cell is not one backticked file name: {file_cell!r}")
            continue
        name = m.group(1)
        if name in rowed:
            failures.append(f"(readme) {where}: {name} has a second row (first at line {rowed[name]})")
        rowed.setdefault(name, n)
        if f"{directory}/{name}" not in files:
            failures.append(f"(readme) {where}: names {name}, which is not in {directory}/")
        kinds = [k.strip() for k in kind.split(",")]
        for k in kinds:
            if k not in policy["kinds"]:
                failures.append(f"(readme) {where}: Kind {k!r} is not one of {list(policy['kinds'])}")
        if not read_by.strip():
            failures.append(f"(readme) {where}: {name} has an empty Read by cell (D2: every file is read by something)")
        for gate in GATE_ID.findall(read_by):
            gate_matrix_rows(root, manifest, gate, failures, where)
        for token in BACKTICKED.findall(read_by):
            if not (root / token).is_file():
                failures.append(f"(readme) {where}: Read by names {token}, which does not exist")
        if not RFC_REF.search(owning):
            failures.append(f"(readme) {where}: the Owning RFC cell names no RFC: {owning!r}")
        for num in RFC_REF.findall(owning):
            found = resolve_rfc(root, num)
            if len(found) != 1:
                failures.append(f"(readme) {where}: RFC {num} resolves to {len(found)} files")
    for name in on_disk:
        if name not in rowed:
            failures.append(f"(readme) {directory}/{name} has no row in {readme}")
    for needed in policy["must_name"]:
        if needed not in text:
            failures.append(f"(readme) {readme} does not name {needed}, which the policy requires it to")


# ── (read) ──────────────────────────────────────────────────────────────


def check_read(policy: dict, files: dict[str, str], failures: list[str]) -> None:
    directory, readme = policy["directory"], policy["readme"]
    machine = [
        f for f in files
        if (f.startswith("scripts/") or f.startswith(".github/") or f.startswith(directory + "/"))
        and f != readme
    ]
    for f in sorted(files):
        if not f.startswith(directory + "/") or f == readme:
            continue
        if not any(other != f and f in files[other] for other in machine):
            failures.append(
                f"(read) {f} is named by no file under scripts/, .github/ or {directory}/: nothing machine-read "
                "refers to it, so nothing can fail when it stops being true (RFC 116 D2)"
            )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--policy", required=True)
    args = parser.parse_args(argv)
    root = Path(args.root)
    try:
        policy = load_policy(root / args.policy)
        files = read_tree(root)
    except PolicyError as exc:
        print(f"check-contracts: {exc}", file=sys.stderr)
        return 2

    failures: list[str] = []
    check_paths(policy, files, failures, args.policy)
    check_readme(policy, root, files, failures)
    check_read(policy, files, failures)

    if failures:
        for line in failures:
            print(f"check-contracts: {line}", file=sys.stderr)
        print(f"check-contracts: failed with {len(failures)} violation(s)", file=sys.stderr)
        return 1
    scanned = sum(1 for f in files if not is_record(f, policy["records"]))
    print(
        f"check-contracts: all conditions satisfied ({scanned} files in scope, "
        f"{len(files) - scanned} records exempt, {sum(1 for f in files if f.startswith(policy['directory'] + '/')) - 1} contract files)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
