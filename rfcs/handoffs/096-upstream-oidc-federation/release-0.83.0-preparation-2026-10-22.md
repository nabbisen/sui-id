# `0.83.0` — release preparation

> ## Whose turn it is: **the implementation role's, for the mechanical half.**
> **`@nabbisen`, 2026-10-22: _"Authorized."_** The cut and the publish
> stay with him and the architect; everything below is the preparation
> that does not.

**Dispatched.** 2026-10-22 JST, by the architect.
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
§*Pre-publish checklist* — items 1, 2 and 3 are this dispatch. Items 4 and
5, Level B, the tag and the publish are **not**; see §4.
**Contents.** M3 stages 2, 3, 4a, 4b, 5 and 6. All landed and green.
**Baseline.** Read the tip with `git log -1`.

## 0 — what this release is, so the entry reads right

`0.83.0` is RFC 095 M3's remaining implementation: the field rules, the
registration response, persistence of the three metadata fields, the
authentication/grant matrix, browser presentation, and the read-only
legacy-candidate report.

**It does not close M3.** Two prerequisites remain, and both belong to
`@nabbisen`. One is a durable disposition of every candidate `sui-id admin
legacy-dynamic-candidates` reports; the other is his sign-off on the
evidence. Do not write anything in the `CHANGELOG` or elsewhere that reads
as M3 being finished.

## 1 — the version bump

**Six sites**, all in the root `Cargo.toml`, measured today:

```
:20  version = "0.82.0"                          → 0.83.0
:33  sui-id-core   = { version = "0.82.0", … }   → 0.83.0
:34  sui-id-i18n   = { version = "0.82.0", … }   → 0.83.0
:35  sui-id-shared = { version = "0.82.0", … }   → 0.83.0
:36  sui-id-store  = { version = "0.82.0", … }   → 0.83.0
:37  sui-id-web    = { version = "0.82.0", … }   → 0.83.0
```

Then refresh `Cargo.lock` **by letting cargo do it** — `cargo check` or
any build — not by editing it. Confirm the six workspace entries moved and
that nothing else in the lock changed; a dependency that moved in the same
commit as a version bump is a second change hiding inside the first.

**Three files mention `0.82.0` and must not be bumped**, because each is a
statement about the past, not a version spec:

- `ROADMAP.md:926` — the `0.82.0` release record.
- `ROADMAP.md:1007` — a milestone row naming v0.82.0 as a boundary.
- `docs/development-specification.md:8` — *"Stale as of v0.82.0
  (2026-10-20)"* — and `docs/threat-model.md:4` — *"the workspace is now
  **v0.82.0**"*. **These two do want updating**, to `v0.83.0` and
  2026-10-22, because each is a marker of how far behind the document has
  fallen and a stale staleness marker is worse than none. Not a
  find-and-replace: read each sentence and keep it true.

## 2 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.83.0] — 2026-10-22`, with a fresh empty
`[Unreleased]` above it carrying no subsections.

The section has four subsections already written and reviewed — `Added`,
`Changed`, `Fixed`, `Security`. **Do not rewrite, reorder, merge or
summarise them.** The `Security` entry's severity paragraph carries a
wording `@nabbisen` settled today; it is not yours or mine to adjust.

**Add a one-line summary under the version heading**, in the shape
`0.82.0` and `0.81.0` use — read those two first and match them. For this
release it says what the cut contains: RFC 095 M3's remaining
implementation, and that M3 itself does not close here.

**There is no link-reference block** — measured: zero lines matching
`^[0.8`. The "Older releases" table at the foot covers 0.30.x and earlier
and needs nothing. Both stated so you do not go looking.

## 3 — packaging verification

Checklist item 1:

```
cargo package -p sui-id-shared --allow-dirty
```

The package must build and its verify step must succeed. The other five
crates cannot be verified end to end until `sui-id-shared` is on the index,
so this is the only one you can prove before the publish.

**Report the output**, not a summary of it. If it warns about anything —
included files, a missing field, a size — say so verbatim even if it
succeeds.

## 4 — what you must not do, and it is most of the risk

- **No tag.** Not `0.83.0`, not any other.
- **No publish.** Nothing reaches crates.io from this round.
- **No `workflow_dispatch`**, and no claim of Level B. Level B for a
  release must be obtained on the exact commit being tagged, which does
  not exist yet, and reading it is the architect's.
- **No `Security audit` judgement.** You may run `cargo audit` locally and
  report its raw output if you want to — it is useful. The reachability
  assessment and what follows from each finding are reserved;
  `.github/SECURITY.md` places severity with `@nabbisen`.
- **Do not edit the `Security` section's severity paragraph.**

## 5 — what to return

- The diff of the six version sites, and confirmation that `Cargo.lock`
  changed only in the six workspace entries.
- What you changed in the two staleness markers, and what you deliberately
  left alone in `ROADMAP.md`.
- The `cargo package` output, verbatim.
- The full matrix, run local, as usual. **Report it as the local matrix,
  not as Level B** — the distinction is the whole of §4's third bullet.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>`.

**No `CHANGELOG` entry for this round itself.** A version bump is not a
change to describe; it is the act of naming the changes already described.

Leave the working tree uncommitted. One open dispatch at a time; this is it.
