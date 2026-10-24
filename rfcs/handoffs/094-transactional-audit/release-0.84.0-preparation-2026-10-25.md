# `0.84.0` — release preparation

> ## Whose turn it is: **the implementation role's, for the mechanical half.**
> **`@nabbisen`, 2026-10-24: _"Authorized to cut."_** — and, a minute
> later, _"Excuse me but I mistook to tell you. Proceed on the Stage 3
> instead of cutting it now."_ Stage 3 landed in `f77c85b` and its CI is
> green, so the authorization is live again as written. The cut and the
> publish stay with him and the architect; everything below is the
> preparation that does not.

**Dispatched.** 2026-10-25 JST, by the architect.
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
§*Pre-publish checklist* — items 1, 2 and 3 are this dispatch. Step 0,
items 4 and 5, Level B, the tag and the publish are **not**; see §4.
**Contents.** RFC 094 M2b stages 1, 2 and 3, plus RFC 123 D4 and the
`ctutils` advisory fix. All landed and green.
**Baseline.** The tip of `main` when you start — `git log -1`, and record
the hash in your return. `e5b732c` is the commit carrying this dispatch; if
the tip has moved past it, use the tip.

## 0 — what this release is, so the entry reads right

`0.84.0` is **RFC 094 M2b's settings waves**: the pending-settings-change
commands made atomic with their audit events (stages 1 and 2), and the six
settings writes that previously left a partial trace or none at all (stage
3). It also carries RFC 123 D4 — the token endpoint's pre-authentication
timing-and-response oracle — and the `ctutils` bump that closes
RUSTSEC-2026-0385.

**It does not finish M2b.** Waves 3 and 4 — the nine best-effort sites and
the authority switch — are `0.85.0` and later. Do not write anything in
the `CHANGELOG` or elsewhere that reads as M2b being complete.

## 1 — the version bump

**Six sites**, all in the root `Cargo.toml`, measured today on the
baseline:

```
:20  version = "0.83.0"                          → 0.84.0
:33  sui-id-core   = { version = "0.83.0", … }   → 0.84.0
:34  sui-id-i18n   = { version = "0.83.0", … }   → 0.84.0
:35  sui-id-shared = { version = "0.83.0", … }   → 0.84.0
:36  sui-id-store  = { version = "0.83.0", … }   → 0.84.0
:37  sui-id-web    = { version = "0.83.0", … }   → 0.84.0
```

Then refresh `Cargo.lock` **by letting cargo do it** — `cargo check` or
any build — not by editing it. Confirm the six workspace entries moved and
that nothing else in the lock changed. A third-party dependency that moved
in the same commit as a version bump is a second change hiding inside the
first; `ctutils 0.4.3 → 0.4.5` already landed separately in `5ed327d` for
exactly that reason, and nothing else should follow it here.

### What mentions `0.83.0` and must **not** be bumped

Each is a statement about the past, not a version spec. Measured today
with `git grep -n '0\.83\.0'`:

- `ROADMAP.md:926` — the cycle E release record. **Do not touch
  `ROADMAP.md` at all this round**; the cycle F row above it is the
  architect's and is already written.
- `docs/src/contributing/release-process.md:132` — a sentence about what
  `0.83.0`'s own preparation got wrong. Bumping it would make it false.
- `contracts/owner-attribution-baseline.txt:425,475` — baseline lines.
  **Never hand-edit this file**; it is written by
  `check-owner-attributions.py --update-baseline` and the hashes are the
  point of it.
- Everything under `rfcs/handoffs/` — all records.

### What mentions `0.83.0` and **must** be bumped

Two staleness markers. **Not a find-and-replace: read each sentence and
keep it true**, including the arithmetic.

1. `docs/development-specification.md:8-21`.
   - *"Stale as of v0.83.0 (2026-10-22)"* → `v0.84.0` and the cut's date.
   - *"six minor versions past it"* → **seven** (v0.77.0 → v0.84.0).
   - *"which is four more than … tolerance allows"* → **five**
     (`tolerance_minor = 2`, `contracts/doc-authority.toml:10`).
   - *"Everything else is unverified against v0.80.0"* — **this is
     already wrong on the baseline**, a leftover from when the banner read
     v0.80.0; it should name the same version the banner does. Fix it to
     `v0.84.0` in the same pass and say in your return that you did.
   - **Add one known divergence** to the list, since this release creates
     it: §15's logging and audit account now also predates the Class-A
     settings commands (RFC 094 M2b stages 1–3), which moved six settings
     writes and the pending-change lifecycle onto `db.class_a`. One clause
     in the shape of the three already there; do not restructure the list.
2. `docs/threat-model.md:4` — *"the workspace is now **v0.83.0**"* →
   `v0.84.0`. The *"Stale as of 2026-09-10"* date on the line above is the
   document's own staleness date, **not** a release date: leave it.

`G15` passes either way — a bannered document passes however far behind it
is (`doc-authority.toml:11-13`) — so nothing catches a stale staleness
marker but you.

## 2 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.84.0] — <the cut's date>`, with a fresh empty
`[Unreleased]` above it carrying no subsections.

The section has three subsections already written and reviewed —
`Changed`, `Fixed`, `Security`. **Do not rewrite, reorder, merge or
summarise them.** Both entries in `Security` carry severity paragraphs
`@nabbisen` settled; they are not yours or mine to adjust.

**Add a one-line summary under the version heading**, in the shape
`0.83.0` and `0.82.0` use — read those two first and match them. For this
release it says what the cut contains: RFC 094 M2b's settings waves, and
that M2b itself does not finish here.

**There is no link-reference block** — measured: zero lines matching
`^\[0\.`. The "Older releases" table at the foot covers 0.30.x and earlier
and needs nothing. Both stated so you do not go looking.

## 3 — packaging verification

Checklist item 1:

```
cargo package -p sui-id-shared --allow-dirty
```

The package must build and its verify step must succeed. The other five
crates cannot be verified end to end until `sui-id-shared` is on the
index, so this is the only one you can prove before the publish.

**Report the output**, not a summary of it. If it warns about anything —
included files, a missing field, a size — say so verbatim even if it
succeeds.

## 4 — what you must not do, and it is most of the risk

- **No tag.** Not `0.84.0`, not any other.
- **No publish.** Nothing reaches crates.io from this round.
- **No `workflow_dispatch`**, and no claim of Level B. Level B for a
  release must be obtained on the exact commit being tagged, which does
  not exist yet, and reading it is the architect's.
- **No `Security audit` judgement.** The reachability assessment and what
  follows from each finding are reserved; `.github/SECURITY.md` places
  severity with `@nabbisen`.
- **Do not edit either severity paragraph in the `Security` section.**

### If you run `cargo audit` locally — and the one thing to get right

Running it is useful and welcome. **Run it with the lane's own flags, not
bare**:

> `.github/workflows/audit.yml`, the `Run cargo audit` step — copy the
> flags from that line. Not from here, and not from memory.

A bare `cargo audit` **exits 0 on classes that lane denies**, a yanked
transitive crate among them. That is not hypothetical: `0.83.0`'s
preparation ran the bare form, reported clean, and `tokio` 1.54.0 was
yanked the whole time. Report the raw output either way; judging it is
still not yours.

The lane itself has already been read on the baseline for this release —
that is step 0 of the checklist and it is the architect's. It is why
`5ed327d` exists and why this dispatch is not carrying an unfixed
advisory.

## 5 — what to return

- The diff of the six version sites, and confirmation that `Cargo.lock`
  changed only in the six workspace entries.
- What you changed in each of the two staleness markers — including the
  two numbers and the `v0.80.0` correction — and what you deliberately
  left alone in `ROADMAP.md` and `release-process.md`.
- The `cargo package` output, verbatim.
- The full matrix, run local, as usual. **Report it as the local matrix,
  not as Level B** — the distinction is the whole of §4's third bullet.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.

**No `CHANGELOG` entry for this round itself.** A version bump is not a
change to describe; it is the act of naming the changes already described.

Leave the working tree uncommitted. One open dispatch at a time; this is
it.
