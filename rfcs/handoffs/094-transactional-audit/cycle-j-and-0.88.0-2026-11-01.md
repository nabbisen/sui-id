# Cycle J and `0.88.0` — scope proposal

> ## Whose turn it is: **yours, to authorize or argue with.**
> Scope proposals and release cycles are the architect's to make; the
> decision on them is not, per `ROADMAP.md` §S1a.

**Measured 2026-11-01**, on `cebb169`, after `0.87.0` shipped.
**Settled already.** `@nabbisen`, 2026-10-29: *"Two."* — `0.88.0` is the
authority switch, where M2b closes.
**Input.** Cycle I complete; `0.87.0` released and published.

> **Status, 2026-11-01.** The scope was authorized, then held: to a
> prompt offering to dispatch stage 1 at once, *"Will be authorized when
> ready."* — the architect had reported it as ready to go, which
> overstated it, and that is recorded rather than quietly fixed.
> **The start was then called the same day: *"You can start 0.88.0 when
> ready."*** So the cycle is open.
>
> **Stage 1 is dispatched** —
> `.git-exclude/staging/cycle-j-stage1-criterion-2-and-the-chain-claims-2026-11-01.md`.
> It is a measurement and a documentation round, deliberately ahead of
> the gates: stages 2–3 build gates **over** a boundary, and this stage
> fixes that boundary's definition, because a gate over an ambiguous
> criterion enforces whichever reading its author happened to hold.
>
> **§A item 4 turns out to be narrower and sharper than "correct
> audit-chain claims" suggested.** RFC 094 `:1191-1194` already states
> the honest position *and makes it normative* — the chain *"does not
> provide external notarization or protection against an attacker who
> can rewrite the database … **Documentation must say so plainly.**"*
> So this was never a disagreement about the truth: one document had
> **diverged from a requirement of its own RFC**, claiming a raw-SQL
> rewrite could *"never"* avoid detection. Corrected 2026-11-01 in
> `docs/src/guides/operators.md`; stage 1 checks every other place the
> chain is described against that same sentence, the admin UI's own
> strings included, since those have the least room to qualify.

---

## A — "the authority switch alone" is one item in the settlement and five in the RFC

The 2026-10-29 settlement says `0.88.0` is *"the authority switch
alone"*. **RFC 094 `:1100-1101` says the authority switch is five
things**, and the architect has been quoting the settlement rather than
the RFC:

1. land the `syn` AST boundary gate;
2. **make the structural gate blocking**;
3. **downgrade or remove the string gate as authority**;
4. **correct audit-chain claims**;
5. collect the full adversarial evidence package.

Only (1) has been described in any handoff to date. **This is the same
error cycle I opened with** — scoping a milestone from a summary of
itself rather than from its own closure field — and it is recorded here
rather than quietly corrected, because that is twice now.

## B — M2b's closure has four criteria, and one of them is not yet checkable

RFC 094's closure field, M2b: *"every remaining Class-A row converted;
no production Class-A best-effort append anywhere; the AST boundary
gate landed, **with a negative fixture that rejects an unregistered
write**; and `@nabbisen` accepts the durable evidence, **on the
architect's adversarial assessment of it**."*

| Criterion | State |
|---|---|
| 1. Every remaining Class-A row converted | **NOT met — corrected 2026-11-03, see below.** This cell read *"Met by cycle I — eleven converted, the rest dispositioned"* and was wrong twice |
| 2. No production Class-A best-effort append anywhere | **Not checkable as written.** See below |
| 3. AST gate landed with a negative fixture | Not started — needs a new `xtask` crate; there is no `xtask` in the workspace |
| 4. Acceptance on the architect's adversarial assessment | Cannot begin until 1–3 are done |

### Criterion 1 was reported met, and is not — corrected 2026-11-03

**Found by the dev team in stage 1, against the architect's own record.**
Of cycle I's eleven live Class-A rows, **nine were converted** — U13,
U16, U17, U18, U19, U20, U21, C14, S10 — not eleven. Of the remaining
two, **C19 was dispositioned** in stage 5; **U26 was neither converted
nor dispositioned.** Stage 5's dispositions were U11, C12, C13, C19,
C20. U26 appears in no stage's deliverable.

**U26 is a live, unconverted Class-A mutation with no event of its
own.** Verified: `class = "A"`, `sealed = false`, and its write at
`admin/auth.rs:352` is a bare
`users::upsert_ldap_shadow(&app.db, shadow, app.clock.now()).await?`
outside any sealed command.

**How the error happened, stated so it is recognisable next time.**
Cycle I stage 1 measured U26 and found its rationale *accurate*; the
architect read that as settling the row. **A measurement confirming
that a row has no command is not a decision that it needs none** — the
dev team's phrasing is better than the architect's: *"a measurement
confirmation, not a disposition decision."* **This is the third time in
this cycle a restatement stood in for the source**, after the authority
switch read from a settlement rather than RFC 094's own list, and
criterion 2 read as checkable when it was not.

**So M2b's closure needs U26 disposed of or converted**, and criterion
1 must not be reported met again until it is.

### Criterion 2 is ambiguous, and the ambiguity is load-bearing

Measured today: **nine `let _ = audit::append(...)` sites in production
code** (two further grep hits are doc comments describing the pattern).
Their events:

| Site | Event | In the inventory? |
|---|---|---|
| `authn/session.rs:61` | `auth.login.failure` | **yes, and the matrix classes it A** |
| `authn/session.rs:235` | `auth.login.password_ok_mfa_required` | yes |
| `admin/auth.rs:354` | `auth.login.password_ok_mfa_required` | yes |
| `http/handlers/oauth_token.rs:144` | `token.revoke` | yes |
| `audit_chain.rs:150` | `audit.chain.verification_failed` | no |
| `oidc/authorize.rs:587` | `oauth2.exchange_code.user_revoked` | no |
| `cli.rs:483` | `admin.master_key.rotated` | no |
| `oauth_token.rs:91` | `token.introspect` | no |
| `federation.rs:1920` | (not inline; to be established) | — |

**The first row looks like a violation and is not one.**
`record_refused_login` (`session.rs:48-73`) emits `auth.login.failure`
for a sign-in *refused before any credential check* — unknown,
disabled, deleted or locked. **It mutates nothing.** There is no
transaction for the append to be atomic with. U22, the sealed command
for the same event name, covers the case that *does* mutate: the
failure count and the lockout threshold.

**So one event name is emitted both by a Class-A command and by a
mutation-free path, and the criterion classes *events* while being
phrased about *appends*.** As written it can be read as already
violated, or as trivially satisfied, and the difference decides whether
`0.88.0` contains conversion work at all.

**The architect's proposed reading, for you to settle:** criterion 2
means **no append accompanying a Class-A mutation is best-effort**. A
record of something that wrote nothing has no transaction to join, is
outside the criterion by construction, and should be *classified* as
such explicitly rather than left looking like a gap. **Settling this is
item (4) of §A — "correct audit-chain claims" — and it should come
first, not last.**

## B′ — stage 1 landed, and stage 2 closes what it found

**Stage 1 LANDED** `ce3e0cc`, 2026-11-03. A measurement and
documentation round: `docs/src/reference/audit-events.md` now states
what a verified chain proves, and three records it exposed were
corrected — criterion 1's claim above, **K06's rationale**, and the
coverage matrix's own class-corrections narrative, which was stale in
both directions. Review —
`.git-exclude/reviewed/cycle-j-stage1-criterion-2-and-the-chain-claims-2026-11-03.md`.

**Criterion 2 fails on exactly one of nine sites.** Master-key rotation
reseals six tables in one transaction and records itself **outside** it,
best-effort, with **no command row anywhere** in the inventory.

**And that absence had a cause, which is the finding to keep.** K06's
rationale said the reseal primitives are *"subordinate to S10 or K01
only"*. `rotate_master_key` is a **third, independent, top-level
caller** of them. So the row was never written because the inventory
told the next reader there was no top-level command to look for — **a
wrong rationale does not merely mislead, it suppresses the search.**

**Stage 2 is dispatched** —
`.git-exclude/staging/cycle-j-stage2-the-two-gaps-2026-11-03.md`. One
conversion and one measurement, split on whether the work is
mechanical:

- **U26 is a conversion, and L03 is its template.** The no-MFA
  directory sign-in is fully sealed; the MFA-required path does the
  same shadow write bare. One new `L` command covers U26's upsert and
  U32's pending insert together, emitting the **existing** event name,
  so nothing retires.
- **Master-key rotation is a measurement**, because the operation spans
  the database *and* the filesystem — the key file is rewritten after
  the reseal commits — so no transaction covers both and sealing it is
  a question about *what the record is atomic with*. The architect's
  candidate is two facts and two records. **A separate question is
  flagged ahead of it**: if the reseal commits and the key-file write
  then fails, the database is sealed with a key that is not on disk, and
  whether a documented recovery path exists is being established. That
  may matter more than the audit gap.

## B″ — stage 2 landed, and it found something that jumped the queue

**Stage 2 LANDED** `7c42c23`, 2026-11-03 — **CI 28 success / 0 failed /
0 skipped**. **L08 seals the MFA-required directory sign-in**, closing
U26 — the row criterion 1 was reported to have converted and had not.
G20 56 → 57 declared, G17 56 → 57 sealed. Review —
`.git-exclude/reviewed/cycle-j-stage2-the-two-gaps-2026-11-03.md`.

**So criterion 1 is met, said carefully this time.** U32 deliberately
did *not* get `sealed_by`: its mutation surface still has a live
unsealed caller in the local-password branch. Claiming otherwise would
have been the tidy answer and the false one.

**Criterion 2 remains open on master-key rotation alone** — and the
measurement for it found a worse problem than the audit gap it was
sent after. **Dispatched ahead of stage 3, as its own round** —
[`urgent-master-key-ordering-2026-11-03.md`](urgent-master-key-ordering-2026-11-03.md).

**The re-seal commits before either key file is touched**, and without
`--new-key` the key is generated in memory, never printed, and
persisted only by the write that follows the commit. A disk error there
destroys it, and every sealed column with it, permanently. The guide
had promised *"There is no half-rotated state to recover from"*;
corrected 2026-11-03, with `--new-key` documented as the safe
procedure, which protects the operator who reads it. The round protects
the one who does not, by staging the key to `<path>.new` before the
re-seal — and by **refusing when that file already exists**, because a
rerun that overwrites a `.new` left by a crashed rotation would destroy
the key the database is already using and would look like a retry.

**The split was the architect's call under the standing arrangement**:
the ordering fix is small and independent, while the sealed command
needs a descriptor, an id and matrix work, so bundling them would have
shipped the urgent half at the slower half's pace.

**Two by-products carried into stage 3** rather than lost: the
`with_tx_sync`-on-an-async-path mismatch, and that today's
`let _ = audit::append(...)` swallows even the append's own failure.

## C — RFC 142 stages 4 and 5 are outstanding, and I nearly let them slide

[RFC 142](../../accepted/142-upgrading-is-the-least-tested-thing-we-do.md)
is **Accepted** with five stages. Stages 1–3 landed in `0.87.0`;
**stages 4 and 5 have no LANDED note and were in no cycle**:

- **Stage 4 — D3.** The read-only preview subcommand: what the stored
  schema version is, which release last migrated the file, which
  migrations *would* apply, and the version it would end at. **It must
  not write.**
- **Stage 5 — D4.** The automatic pre-upgrade backup, with a test that
  it happens and a test that it is skipped when nothing is pending.

RFC 142's closure needs both. **Stage 5 is the one an operator feels**:
`upgrade.md` tells them to back up manually, and D4 makes it automatic
for the case where forgetting is most expensive.

## D — the proposal

**`0.88.0` = M2b closes, and RFC 142 closes.**

> **Renumbered 2026-11-03.** This section listed five stages and the
> work has not matched them since stage 1. Two rounds happened that
> appeared nowhere in it — the criterion 1 gap and the master-key
> ordering fix, both discovered rather than planned — while the
> architect went on calling the next round "stage 3" in a list whose
> stage 3 is the gate work. That is the same stale-plan failure this
> cycle's §A records, committed again in the document that records it.
> The numbering below is against what happened. **Discovered rounds are
> numbered in place, not appended**, so the list keeps answering "what
> is next?" correctly.

1. **Settle criterion 2 and correct the audit-chain claims** (§B, and
   §A item 4). Measurement and documentation; no behaviour change. It
   came first because the later gates are gates *over* a boundary whose
   definition this fixes. — **LANDED** `ce3e0cc`, 2026-11-03.
2. **Close criterion 1's gap: seal the MFA-required directory sign-in**
   (L08, U26). *Discovered by stage 1*, which found criterion 1 had
   been reported met and was not. — **LANDED** `7c42c23`, 2026-11-03.
3. **The master-key ordering fix.** *Discovered by stage 2's
   measurement*, which went looking for an audit gap and found a
   permanent-data-loss window instead. Dispatched as its own round
   ahead of the audit work, because the fix was small and independent
   and the sealed command is neither. — **LANDED** `1b3bc7d`, shipped
   in **`0.87.1`**, 2026-11-03.
4. **Seal the re-seal, and close criterion 2.** The last outstanding
   Class-A best-effort append with a top-level operation behind it, and
   the operation with **no command row anywhere** — see K06's corrected
   rationale for why the row was never written. Larger than "add a
   command": `declare_write_command!` lives in `sui-id-store`
   (`registry.rs:1367`) and the orchestration lives in `sui-id-core`
   (`oidc/key_rotation.rs:99`, public API with three live callers), so
   it carries a crate move, a public-API decision and two test
   rewrites. **This closes criterion 2.**
5. **Make a half-finished rotation legible.** One operation now
   produces two records — the re-seal, atomic with its transaction, and
   the key-file replacement, which cannot be atomic with anything. That
   is *better* than one record, but only if the names and the docs say
   so, which is operator-facing work in its own right. Depends on 4.
6. **The `syn` AST boundary gate**, in a new `xtask` crate, **with the
   negative fixture that rejects an unregistered write** — the closure
   criterion names the fixture, not just the gate.
7. **Make the structural gate blocking; downgrade the string gate from
   authority** (§A items 2 and 3).
8. **RFC 142 stage 4** — the preview subcommand.
9. **RFC 142 stage 5** — the automatic pre-upgrade backup.

**Then M2b closes** on the adversarial evidence package and your
acceptance — which, per §B, the architect cannot give for its own work.

**Stages 4 and 5 were one stage until 2026-11-03**, when a self-review
the owner asked for
(`.git-exclude/reviewed/cycle-j-stage3-self-review-2026-11-03.md`)
found it was four things wearing one name. The split is recorded
because the underestimate, not the work, is the recurring defect.

## E — the honest alternative

**This is two closures in one cycle, and the architect's size estimates
for RFC 094 have now been wrong twice.** If you would rather split:

- **`0.88.0`** = stages 1–3, M2b closes.
- **`0.89.0`** = RFC 142 stages 4–5, RFC 142 closes — alongside
  [RFC 143](../../accepted/143-installation-without-a-toolchain.md),
  which §D of the ROADMAP's re-proposal already targets there.

**The architect recommends the split**, for the same reason the last one
was recommended and accepted: stage 2 needs a new workspace member, its
size is not yet stated, and pairing it with work that has an operator
deadline is how a cycle becomes open-ended.

## F — open questions

1. **Criterion 2's reading** (§B). The architect proposes "no
   best-effort append accompanying a Class-A mutation"; this is the
   decisive one, and it is item (4) of the switch itself.
2. **One cycle or two** (§E). The architect recommends two.
3. **Not in this scope, and still open**:
   [RFC 119](../../accepted/119-embedding-contract.md)'s review route
   and its two remaining questions,
   [RFC 143](../../accepted/143-installation-without-a-toolchain.md)'s
   two, RFC 140 stage 2 (the logo upload, unblocked since 2026-10-30),
   and the smaller carried items — the `test_id` resolution gate, the
   dead `consents` table, and the eight e2e files sharing the racy
   tracing idiom.
