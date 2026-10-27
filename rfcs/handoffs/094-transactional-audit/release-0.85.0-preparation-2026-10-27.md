# `0.85.0` — release preparation

> ## Whose turn it is: **the implementation role's, for the mechanical half.**
> **`@nabbisen`, 2026-10-27: _"Authorized."_** The cut and the publish
> stay with him and the architect; everything below is the preparation
> that does not.

> ## LIVE again — re-measured 2026-10-27 and ready to start.
> This was **held** on 2026-10-27 for one round:
> [`registration-token-listing-2026-10-27.md`](registration-token-listing-2026-10-27.md),
> which closed the gap where `revoke-registration-token --id ID` had no
> way for an operator to learn an id. **That round landed `d0c1924`,
> Level B 27/0**, so the hold is lifted.
>
> **Re-measured rather than trusted**, as the hold note promised:
> §1's six version sites are unchanged (`Cargo.toml:20,33-37`); the
> `0.84.0` mentions outside the lockfile are the same six; and
> `[Unreleased]` now carries **five** subsections — `Added`, `Changed`,
> `Removed`, `Fixed`, `Security` — not the four it had when §2 was
> written. §2 is amended accordingly.

**Dispatched.** 2026-10-27 JST, by the architect, held the same day and
**released on 2026-10-27** — see above.
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
§*Pre-publish checklist* — items 1, 2 and 3 are this dispatch. **Step 0
is already done and is not yours**: the `Security audit` lane was read
on `main` before this was written — `workflow_dispatch` `40482260166` on
`65c49e1`, green. Two commits have landed since, and **neither touched
`Cargo.toml` or `Cargo.lock`** (measured: `git diff 65c49e1 d0c1924 --
Cargo.lock Cargo.toml` is empty), so that reading still holds. The lane
is read again on the exact commit being tagged, which is step 5 and is
the architect's. Items 4 and 5, Level B, the tag and the publish are
**not** yours either; see §4.
**Contents.** RFC 094 M2b wave 4, four stages, each already Level B on
its own commit. Plus M3's closure, recorded 2026-10-25.
**Baseline.** The tip of `main` when you start — `git log -1`, and record
the hash in your return.

## 0 — what this release is, so the entry reads right

`0.85.0` is **RFC 094 M2b's client-administration wave**: every
remaining administrator write on clients and signing keys now commits
atomically with its audit record. Fourteen inventory rows, four stages,
G20 from 34 to 44.

**It does not close M2b.** Wave 3 (federation configuration) and the
authority switch remain. Wave 3 is no longer blocked — its dependency
cycle was ruled on 2026-10-25 — but it is not in this cut.

## 1 — the version bump

**Six sites**, all in the root `Cargo.toml`, measured today:

```
:20  version = "0.84.0"                          → 0.85.0
:33  sui-id-core   = { version = "0.84.0", … }   → 0.85.0
:34  sui-id-i18n   = { version = "0.84.0", … }   → 0.85.0
:35  sui-id-shared = { version = "0.84.0", … }   → 0.85.0
:36  sui-id-store  = { version = "0.84.0", … }   → 0.85.0
:37  sui-id-web    = { version = "0.84.0", … }   → 0.85.0
```

Then refresh `Cargo.lock` **by letting cargo do it** — not by editing
it. Confirm the six workspace entries moved and that **nothing else in
the lock changed**.

### What mentions `0.84.0` and must **not** be bumped

- `ROADMAP.md:926` — the cycle F release record. **Do not touch
  `ROADMAP.md` at all**; the cycle G row above it is the architect's and
  is already written.
- `contracts/owner-attribution-baseline.txt` — never hand-edited; it is
  written by `check-owner-attributions.py --update-baseline`.
- Everything under `rfcs/handoffs/` — all records.

### What mentions `0.84.0` and **must** be bumped

**Not a find-and-replace: read each sentence and keep it true**,
arithmetic included.

1. `docs/development-specification.md:8-23`.
   - *"Stale as of v0.84.0 (2026-10-25)"* → `v0.85.0` and the cut's date.
   - *"seven minor versions past it"* → **eight** (v0.77.0 → v0.85.0).
   - *"which is five more than … tolerance allows"* → **six**
     (`tolerance_minor = 2`, `contracts/doc-authority.toml:10`).
   - *"unverified against v0.84.0"* → `v0.85.0`.
   - **Extend the existing divergence clause.** It currently says §15
     predates the Class-A settings commands "(RFC 094 M2b stages 1–3)".
     Wave 4 added client administration and signing keys. Widen that
     clause rather than adding a fourth — one divergence, now larger.
2. `docs/threat-model.md:4` — *"the workspace is now **v0.84.0**"* →
   `v0.85.0`. The *"Stale as of 2026-09-10"* line above it is the
   document's own staleness date, **not** a release date: leave it.

## 2 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.85.0] — <the cut's date>`, with a fresh empty
`[Unreleased]` above it carrying no subsections.

**The section is already written and reviewed — summary, `Added`,
`Changed`, `Removed`, `Fixed` and `Security`, five subsections. Do not
rewrite, reorder, merge, summarise or shorten any of it**, and in
particular:

- **Do not touch the three severity paragraphs in `Security`.** Their
  wording is `@nabbisen`'s under `.github/SECURITY.md` and is not yours
  or mine to adjust.
- **Do not touch the summary paragraph.** It carries three clauses this
  release owes the public record — M3's closure, the two retired event
  names, and the breaking API changes — and the first two have wording
  bounds set deliberately.

Your job here is the promotion and nothing else: the heading, the date,
and the empty `[Unreleased]` above it.

**There is no link-reference block** — measured: zero lines matching
`^\[0\.`. Stated so you do not go looking.

## 3 — packaging verification

```
cargo package -p sui-id-shared --allow-dirty
```

The package must build and its verify step must succeed. **Report the
output verbatim**, including any warning, even if it succeeds.

## 4 — what you must not do, and it is most of the risk

- **No tag.** Not `0.85.0`, not any other.
- **No publish.**
- **No `workflow_dispatch`**, and no Level B claim. Level B for a
  release must be obtained on the exact commit being tagged, which does
  not exist yet, and reading it is the architect's.
- **No `Security audit` judgement.** Step 0 is done and the lane is
  green; running `cargo audit` locally is welcome and judging it is not.
  **Use the lane's own flags** — read them from
  `.github/workflows/audit.yml`'s `Run cargo audit` step, not from here
  and not from memory. A bare run exits 0 on classes that lane denies.
- **Do not edit any `Security` severity paragraph, or the summary.**

## 5 — your local matrix, which is larger than you have been running

Stage 4 found that `gate-matrix-fixtures` runs
`python3.14 -m unittest scripts.tests.test_*` steps that no bash fixture
script wraps. **Derive that list, do not transcribe it** — stage 4's own
enumeration came back thirteen when there are fourteen:

```
grep -o "python3.14 -m unittest scripts\.tests\.test_[a-z_0-9]*" \
  .github/workflows/ci.yml | sed 's/.*tests\.//' | sort -u
```

Run what that returns. **Run the Rust store tests before them**:
`test_check_m2a_rollback_coverage` shells out to a checker that reads
`target/rfc094-rollback-coverage.txt`, a build artefact the store test
run produces, so on a cold tree it reports a false failure. I hit that
once this week deciding whether it was real.

## 6 — what to return

- The diff of the six version sites, and confirmation that `Cargo.lock`
  changed only in the six workspace entries.
- What you changed in each staleness marker — including both numbers and
  the widened divergence clause — and what you deliberately left alone.
- The `cargo package` output, verbatim.
- The full local matrix, **as the local matrix**, with the `+stable`
  caveat, and the derived unittest list you actually ran.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.

**No `CHANGELOG` entry for this round itself.** A version bump is not a
change to describe.

Leave the working tree uncommitted. One open dispatch at a time; this is
it.
