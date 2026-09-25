"""Read the sealed-command registry out of the Rust source, without an AST.

Shared by the checks that hold a document to what `declare_write_command!`
declares (RFC 116 stage 2). It reads three things from a production source
file: the commands (`command X = "ID" { … enum E { V => &DESCRIPTOR, … } }`),
the `EventDescriptor` statics they map to, and the `AttributeSpec` constants
those descriptors name. That is the honest limit of a text read, and the
reason RFC 094's `audit-structure` exists: nothing here can see a type.

Comments and string contents are blanked before any pattern runs, keeping every
offset, so a doc comment quoting a declaration cannot declare one and a brace in
a string cannot unbalance a block. String *values* are read back from the raw
text at the blanked span.

Not imported by `check-write-commands.py` (RFC 116 stage 1, already landed and
green), which carries its own copy of `blank`, `is_test_file` and `block_end`.
That duplication is named in the stage 2 package as a follow-up rather than
removed here.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path

SRC_ROOT = Path("crates")


def blank(text: str) -> str:
    """Replace comments and string contents with spaces, keeping every newline
    and every offset."""
    out = list(text)
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif text.startswith("/*", i):
            j = text.find("*/", i + 2)
            j = n if j < 0 else j + 2
            for k in range(i, j):
                if text[k] != "\n":
                    out[k] = " "
            i = j
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            for k in range(i + 1, min(j, n)):
                if text[k] != "\n":
                    out[k] = " "
            i = j + 1
        else:
            i += 1
    return "".join(out)


def is_test_file(path: Path) -> bool:
    name = path.name
    return (
        name == "tests.rs"
        or (name.startswith("tests_") and name.endswith(".rs"))
        or "tests" in path.parts
    )


def match_close(text: str, open_at: int, open_ch: str, close_ch: str) -> int:
    depth = 0
    for i in range(open_at, len(text)):
        if text[i] == open_ch:
            depth += 1
        elif text[i] == close_ch:
            depth -= 1
            if depth == 0:
                return i
    return -1


@dataclass
class Descriptor:
    name: str  # the event name, e.g. "user.create"
    static: str  # the Rust static, e.g. "U01_CREATE"
    klass: str  # "Atomic" or "MustAttempt"
    actor: str  # "Required", "Optional" or "None"
    attributes: dict[str, bool] = field(default_factory=dict)  # name -> required
    file: str = ""
    line: int = 0


@dataclass
class Command:
    id: str
    file: str
    line: int
    events: list[str] = field(default_factory=list)  # event names, in variant order


INVENTORY_ID = re.compile(r"[A-Z][0-9]{2}")
INVOCATION = re.compile(r"\bdeclare_write_command!\s*\{")
COMMAND_ID = re.compile(r'\bcommand\s+\w+\s*=\s*"( *)"')
DESCRIPTOR_REF = re.compile(r"=>\s*&\s*([A-Z][A-Z0-9_]*)\b")
DESCRIPTOR_STATIC = re.compile(
    r"\bstatic\s+([A-Z][A-Z0-9_]*)\s*:\s*EventDescriptor\s*=\s*EventDescriptor\s*\{"
)
ATTRIBUTE_CONST = re.compile(
    r"\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*AttributeSpec\s*=\s*AttributeSpec\s*\{"
)
FIELD_NAME = re.compile(r'\bname\s*:\s*"( *)"')
FIELD_REQUIRED = re.compile(r"\brequired\s*:\s*(true|false)\b")
FIELD_CLASS = re.compile(r"\bclass\s*:\s*AuditClass::([A-Za-z]+)")
FIELD_ACTOR = re.compile(r"\bactor\s*:\s*ActorRequirement::([A-Za-z]+)")


def _attribute_from_body(raw: str, body_start: int, body_end: int, text: str) -> tuple[str, bool] | None:
    seg = text[body_start:body_end]
    m = FIELD_NAME.search(seg)
    r = FIELD_REQUIRED.search(seg)
    if not m or not r:
        return None
    name = raw[body_start + m.start(1) : body_start + m.end(1)]
    return name, r.group(1) == "true"


def read_file(root: Path, rel: Path, failures: list[str]) -> tuple[dict[str, Descriptor], list[Command]]:
    raw = (root / rel).read_text(encoding="utf-8")
    text = blank(raw)
    where = str(rel)

    consts: dict[str, tuple[str, bool]] = {}
    for m in ATTRIBUTE_CONST.finditer(text):
        open_at = m.end() - 1
        end = match_close(text, open_at, "{", "}")
        got = _attribute_from_body(raw, open_at, end, text) if end > 0 else None
        if got:
            consts[m.group(1)] = got

    descriptors: dict[str, Descriptor] = {}  # keyed by static name
    for m in DESCRIPTOR_STATIC.finditer(text):
        open_at = m.end() - 1
        end = match_close(text, open_at, "{", "}")
        line = raw.count("\n", 0, m.start()) + 1
        if end < 0:
            failures.append(f"{where}:{line}: unbalanced descriptor {m.group(1)}")
            continue
        body = text[open_at : end + 1]
        head = re.match(r"\{\s*kind\s*:[^,]+,\s*name\s*:\s*\"( *)\"", body)
        klass, actor = FIELD_CLASS.search(body), FIELD_ACTOR.search(body)
        if not head or not klass or not actor:
            failures.append(
                f"{where}:{line}: descriptor {m.group(1)} is not in the shape "
                "`kind, name, class, actor, …` this reader understands"
            )
            continue
        d = Descriptor(
            name=raw[open_at + head.start(1) : open_at + head.end(1)],
            static=m.group(1),
            klass=klass.group(1),
            actor=actor.group(1),
            file=where,
            line=line,
        )
        am = re.search(r"\battributes\s*:\s*&\s*\[", body)
        if am:
            a_open = open_at + am.end() - 1
            a_end = match_close(text, a_open, "[", "]")
            inner_start = a_open + 1
            pos = inner_start
            while a_end > 0 and pos < a_end:
                b = re.compile(r"AttributeSpec\s*\{").search(text, pos, a_end)
                i = re.compile(r"\b([A-Z][A-Z0-9_]*)\b").search(text, pos, a_end)
                if b and (not i or b.start() <= i.start()):
                    b_open = b.end() - 1
                    b_end = match_close(text, b_open, "{", "}")
                    got = _attribute_from_body(raw, b_open, b_end, text)
                    if got:
                        d.attributes[got[0]] = got[1]
                    pos = b_end + 1
                elif i:
                    ident = i.group(1)
                    if ident in consts:
                        d.attributes[consts[ident][0]] = consts[ident][1]
                    else:
                        failures.append(
                            f"{where}:{line}: descriptor {d.name} names attribute `{ident}`, "
                            "which is not an `AttributeSpec` const in this file"
                        )
                    pos = i.end()
                else:
                    break
        descriptors[d.static] = d

    commands: list[Command] = []
    for m in INVOCATION.finditer(text):
        open_at = text.index("{", m.start())
        end = match_close(text, open_at, "{", "}")
        line = raw.count("\n", 0, m.start()) + 1
        if end < 0:
            failures.append(f"{where}:{line}: unbalanced declare_write_command! block")
            continue
        body = text[open_at : end + 1]
        ids = [raw[open_at + i.start(1) : open_at + i.end(1)] for i in COMMAND_ID.finditer(body)]
        if len(ids) != 1:
            failures.append(f"{where}:{line}: expected one `command … = \"ID\"`, found {len(ids)}")
            continue
        cmd = Command(id=ids[0], file=where, line=line)
        for ref in DESCRIPTOR_REF.findall(body):
            if ref not in descriptors:
                failures.append(
                    f"{where}:{line}: command {cmd.id} maps to `{ref}`, "
                    "which is not an `EventDescriptor` static in the same file"
                )
            elif descriptors[ref].name not in cmd.events:
                cmd.events.append(descriptors[ref].name)
        commands.append(cmd)
    return descriptors, commands


def read_registry(root: Path, failures: list[str]) -> tuple[dict[str, Descriptor], list[Command]]:
    """Every descriptor **reachable from a sealed command** in a production
    source file under `crates/`, keyed by event name, and every command.

    A descriptor no command maps to is not on the Class-A seam and is not
    returned: registry.rs's proof-only descriptor is one.
    """
    base = root / SRC_ROOT
    reachable: dict[str, Descriptor] = {}
    commands: list[Command] = []
    if not base.is_dir():
        return reachable, commands
    for path in sorted(p for p in base.rglob("*.rs") if not is_test_file(p.relative_to(root))):
        rel = path.relative_to(root)
        descriptors, cmds = read_file(root, rel, failures)
        by_name = {d.name: d for d in descriptors.values()}
        for cmd in cmds:
            # registry.rs's proof-only command has an id that is deliberately
            # not an inventory code (`check-write-commands.py` tolerates it
            # there and nowhere else); it is not on the seam of any real event.
            if not INVENTORY_ID.fullmatch(cmd.id):
                continue
            commands.append(cmd)
            for name in cmd.events:
                if name in reachable and reachable[name].static != by_name[name].static:
                    failures.append(f"{rel}: event {name} is declared by two descriptors")
                reachable[name] = by_name[name]
    return reachable, commands
