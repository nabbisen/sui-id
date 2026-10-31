# `0.87.0` — release preparation

> ## Whose turn it is: **the implementation role's**, for the mechanical half.
> The tag and the publish dispatch stay with the architect, and **two
> prerequisites are `@nabbisen`'s** — see §0.

**Dispatched.** 2026-11-01 JST, by the architect.
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
— read **How a release is published** first; this is the first release
that does not publish by hand.
**Authorized.** `@nabbisen`, 2026-10-30.
**Contents.** RFC 094 cycle I in full — the census, the conversions
(stages 2, 2b, 3, 4) and the dispositions (stage 5) — plus RFC 142's
migration corpus and gate `G22`, RFC 140 §5's settlement, and two
load-sensitive tests made deterministic.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 0 — read this before touching anything

**This release does not publish by hand.** RFC 141 §7 settled `0.87.0`
as the first cut through `.github/workflows/publish.yml`. **You are not
publishing and you are not tagging** — §4 lists what is not yours, and
it is more than usual.

**Two prerequisites are outstanding and they are `@nabbisen`'s**: the
six per-crate Trusted Publishing configurations on crates.io, and the
`release` GitHub Environment with a required reviewer. **Your work does
not depend on either** — prepare the release and return it. If they are
still outstanding when you finish, say so and stop; do not work around
them.

## 1 — the version bump

**Six sites**, all in the root `Cargo.toml`, measured today:

```
:20  version = "0.86.0"                          → 0.87.0
:33  sui-id-core   = { version = "0.86.0", … }   → 0.87.0
:34  sui-id-i18n   = { version = "0.86.0", … }   → 0.87.0
:35  sui-id-shared = { version = "0.86.0", … }   → 0.87.0
:36  sui-id-store  = { version = "0.86.0", … }   → 0.87.0
:37  sui-id-web    = { version = "0.86.0", … }   → 0.87.0
```

**All six must equal the tag**, and `scripts/check-publish-preconditions.py`
now checks that mechanically before any token is minted — a miss is no
longer caught by a person noticing.

Refresh `Cargo.lock` **by letting cargo do it**. Confirm the six
workspace entries moved and **nothing else in the lock changed**.

### Must **not** be bumped

- **`ROADMAP.md` — do not touch it at all.** The cycle I row is already
  written; the release record is the architect's to add after the cut.
- `contracts/owner-attribution-baseline.txt` — never hand-edited.
- Everything under `rfcs/handoffs/` — records.

### Must be bumped — read each sentence, including the arithmetic

1. **`docs/development-specification.md:8-26`.** Measured today, it
   reads *"Stale as of v0.86.0 (2026-10-29) … reflects the v0.77.0
   codebase and the workspace is nine minor versions past it, which is
   seven more than … tolerance allows"*.
   - `Stale as of v0.86.0 (2026-10-29)` → `v0.87.0` and the cut's date.
   - `nine minor versions past it` → **ten** (v0.77.0 → v0.87.0).
   - `seven more than` → **eight** (`tolerance_minor = 2`,
     `contracts/doc-authority.toml:10`).
   - `unverified against v0.86.0` → `v0.87.0`.
   - **Widen the existing §15 divergence clause; do not add a fourth.**
     It already names M2b stages 1–3, wave 4 and wave 3. **Cycle I now
     joins them**, and it is the largest of the four: every remaining
     Class-A row — the MFA and session writes, consent, the
     registration token, and first-run setup, which now commits as one
     transaction — moved onto `db.class_a`. One clause, not a list of
     stages.
2. **`docs/threat-model.md:4`** — `the workspace is now **v0.86.0**` →
   `v0.87.0`. The `Stale as of 2026-09-10` line above it is that
   document's own staleness date — **leave it**.

## 2 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.87.0] — <the cut's date>`, with a fresh empty
`[Unreleased]` above it.

**One subsection exists — `Changed` — with four entries**, all already
written. Measured today: no `Added`, `Security`, `Removed` or `Fixed`
section, and **that is correct** — nothing in this cycle added a
feature or fixed a reported defect. **Do not invent a section** to make
the release look rounder.

**Write the one-line summary under the version heading**, and it must
name every subsection the release has. With one subsection that is
simple, but the summary still has to carry what an operator needs:
**three event names retire in this release** (`mfa.disable`,
`webauthn.credential.delete`, `auth.sessions.bulk_revoke_self`) and
**one more** (`setup.create_initial_admin`). Four retirements in one
release is the single most consequential thing in it for anyone with a
query or an alert, and the summary must say so rather than leaving it
to the entries.

**No link-reference block** — confirmed today, zero lines matching
`^\[0\.`.

## 3 — packaging verification

Level B covers compilation, tests and lints; it does not cover
packaging. Run:

```
cargo package -p sui-id-shared --allow-dirty
```

**Report the output verbatim**, including any warning. The other five
can only be verified end to end once `sui-id-shared` is on the index,
which is the publish workflow's job, not yours.

## 4 — what you must not do

- **No tag.** Annotated, on `main`, and the architect's.
- **No publish, and no `workflow_dispatch` of `publish.yml`.** That
  workflow runs behind an approval gate that does not exist yet (§0).
- **No `gh workflow run` of anything**, including `audit.yml` — the
  step 0 and step 5 readings of the `Security audit` lane are the
  architect's, per RFC 141 §5.
- **No Level B claim.** `scripts/local-matrix.sh` is not Level B and
  says so in its own banner.
- **Do not touch `ROADMAP.md`** (§1).
- **Do not restate a gate's command** in any document — RFC 131 D4, and
  `scripts/check-verification-commands.py` enforces it.

## 5 — your local matrix

**Run `bash scripts/local-matrix.sh`** and report the count. It derives
its set from `.github/workflows/ci.yml`, so it cannot go stale the way a
hand-enumerated list has, twice.

**If a lane fails, read it before re-running.** Two load-sensitive tests
were made deterministic in `e651e9c` specifically so this run is
trustworthy; a failure now is more likely to be real than it was last
week. If you do hit a flake, say which test and what you measured —
do not re-run until it passes and report the green one.

## 6 — what to return

- The usual package, against the baseline you recorded.
- **The six `Cargo.toml` sites and the `Cargo.lock` diff**, with
  confirmation that nothing else in the lock moved.
- **The promoted `CHANGELOG`**, with the one-line summary (§2).
- **`cargo package`'s verbatim output** (§3).
- The two documents from §1, with the arithmetic shown — **ten** and
  **eight**, not a copied-forward **nine** and **seven**.
- `bash scripts/local-matrix.sh`'s output and count.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.
- **Whether §0's two prerequisites are still outstanding**, as far as
  you can observe them. You cannot check the crates.io side; you can
  check the environment with
  `gh api repos/nabbisen/sui-id/environments`.

Return it as a review request in `.git-exclude/review-requests/`. Leave
the working tree uncommitted.
