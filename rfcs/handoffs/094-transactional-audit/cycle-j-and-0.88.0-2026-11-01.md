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
4. **Seal the re-seal, and close criterion 2.** — **LANDED** `2b1bac5`,
   2026-11-04. K07, `admin.master_key.resealed`, Class A: the six-table
   re-seal and its audit record commit in one transaction. G17 102 → 103
   rows and 57 → 58 sealed; G20 declared 57 → 58. `rotate_master_key` is
   **deleted** from `sui-id-core` rather than kept as a delegating
   wrapper — the kept-wrapper trap closed categorically instead of
   tested around, after this cycle shipped one twice.
   `admin.master_key.rotated` is unchanged. Review —
   `.git-exclude/reviewed/cycle-j-stage4-seal-the-reseal-2026-11-04.md`.
   *The round corrected the dispatch: it cited K01 as the
   `system_principal: permitted`/no-actor shape and K01 is `forbidden`
   with a required human actor. K07 is built against U08, C22 and S05.
   The same wrong claim sits in three code comments
   (`commands.rs:1020`, `:6865`, `:6922`), left for a cleanup round.*
   *Folded in at review: four of the six re-sealed tables were exercised
   by no test in the workspace. Mutating `refresh_tokens::reseal_all` to
   `0` left **2212 tests passing, 0 failing** and the matrix 48/48 —
   and `cli.rs` archives the old key file as soon as K07 commits, so a
   skipped table leaves rows under a key the configured path no longer
   holds. `k07_reseals_every_one_of_the_six_tables` now pins all seven
   counts and fails by name under mutation.*
   **This closes criterion 2** — recorded in `ROADMAP.md` only after
   stage 5, because criterion 1 was reported met once before and was
   not, and the closure claim belongs with the whole of its evidence.
   **CI note, stated honestly:** `2b1bac5`'s own run
   (`41672489921`) was **red** — 27/28, `G06 (stable, all-features)` —
   on `r102_l02_totp_sign_in_commits_everything_and_is_fresh`, a
   load-sensitive test on a path this stage never touched. Fixed in
   `76a3005`, **CI 28 success / 0 failed / 0 skipped** (`41682794440`).
   The 28/28 belongs to the fix commit, not to the stage commit.
5. **Make a half-finished rotation legible.** — **LANDED** `8a0f61b`,
   2026-11-04, **CI 28 success / 0 failed / 0 skipped**
   (`41750549127`). The operator guide carries a table mapping what the
   audit log shows after an interrupted rotation to what happened and
   what to do, including the case that is **not** a half-finished
   rotation: a database rotated before the pair existed. The
   `(A required)` annotation is dropped from the coverage matrix with
   its reasoning recorded — a rename is not a SQL statement, so there is
   no transaction for that event to be atomic with — and the annotation
   is recorded as having been *correct when written* (`8741fa6`,
   2026-09-24), not as an error. A failed append is reported rather than
   discarded, without failing a command whose work has already
   committed. Review —
   `.git-exclude/reviewed/cycle-j-stage5-a-half-finished-rotation-is-legible-2026-11-04.md`.
   *Two fold-ins were the architect's own. The `--new-key` section the
   new table points into still described the pre-`0.87.1` world — that a
   generated key lived only in memory until one post-commit write, and
   that losing it made every encrypted column permanently undecryptable.
   `0.87.1` closed that window; closing it is that release's headline.
   The guide was therefore warning operators on `0.87.1`+ about a risk
   that no longer exists and recommending `--new-key` as protection
   against it — the inverse of this stage's purpose. Written in
   `15dc973`, **after** the fix landed in `1b3bc7d`: one false sentence
   was corrected and the section rewritten around it rather than
   re-derived against the code. And the new warning printed above
   sixteen lines of success output, so the one runtime signal of
   incompleteness scrolled past; it prints last now.*
   **Carried to the `0.88.0` preparation:** the guide and the reference
   doc hard-code **v0.88.0** as the version the pair starts at. That is
   correct under this plan and nothing checks it — G15 reads
   *"Stale as of vX"* banners, not version mentions in prose — and
   `0.87.1` was itself an unplanned mid-cycle patch. **The preparation
   must verify every doc naming the upcoming version against the version
   actually being cut.**
   **Carries three items found while reviewing stage 4**: whether
   `admin.master_key.rotated`'s `(A required)` annotation still means
   anything now that the atomic half has its own name; that event's
   matrix row still lists note fields as `keys_resealed=N` while the
   live append writes `"re-sealed N rows; old key archived at …"`; and
   the `step_now()` invariant. *Corrected 2026-11-04: this entry asked
   for a **sweep** of the remaining `step_now()` call sites, and
   measurement says no sweep is needed. `totp.rs:60` accepts a ±1 step
   window, so a code built inline survives one boundary crossing; the
   one site that challenges repeatedly expects refusal anyway. Only the
   assertion that re-evaluated `step_now()` was broken, and it is fixed.
   What stage 5 carries is a two-line comment recording that invariant at
   each `step_now()` definition. My own entry asked for more work than
   the facts support — the error this cycle keeps producing.*
6. **The `syn` AST boundary gate**, in a new `xtask` crate, **with the
   negative fixture that rejects an unregistered write** — the closure
   criterion names the fixture, not just the gate. **DISPATCHED**
   2026-11-04 —
   `rfcs/handoffs/094-transactional-audit/cycle-j-stage6-the-ast-boundary-gate-2026-11-04.md`.
   *Measurement before dispatch changed its shape three times. (i) RFC
   094 says registering the lane "requires a mechanism that does not
   exist yet"; it exists — A3.4's condition 7 and
   `contracts/gate-inputs.toml`'s `[gate_owners]`. (ii) G18 forbids the
   obvious split: `contract-paths.toml` requires every file in
   `contracts/` to carry a README row naming a gate already in
   `[gates]`, so `write-authority.toml` cannot land before the lane is
   registered — and RFC 094 requires G17 retired in the same change.
   One round, or a split that keeps those four together. (iii) G17 has
   **seven** lettered checks, not the four the RFC summarises; two of
   them postdate that summary. The dependency condition is **held out**
   — see §F.*
7. **The structural gate becomes the coverage authority** (§A item 3).
   **DISPATCHED** 2026-11-04 —
   `rfcs/handoffs/094-transactional-audit/cycle-j-stage7-the-structural-gate-becomes-the-authority-2026-11-04.md`.
   *Re-scoped at dispatch: **§A item 2 is already discharged.** G23 is a
   `[gates]` key and `continue-on-error` appears **zero** times in
   `ci.yml` — this project has no non-blocking lane mechanism, so
   landing the structural gate as a registered lane was making it
   blocking. This entry previously carried both items; only item 3
   remains, and it is not a documentation change: RFC 094 `:946-948`
   says `audit-structure` **becomes the authoritative coverage gate**,
   and today the bidirectional matrix↔source check is still G13's.
   Authority follows capability. G13 is **not** retired — the RFC makes
   that an amendment, which is the owner's — unlike G17, which the RFC
   required retired in the same change.*
7b. **The coverage check reads the `Event name` column, not every
   backtick.** — **LANDED** `61cc57d`, 2026-11-05. Closed a blind spot
   stage 7's review found: `matrix_event_names` filtered matrix names to
   namespaces the source declares, so a row in a namespace nothing
   declares passed the forward check silently. The filter is **deleted**,
   not widened. Review —
   `.git-exclude/reviewed/cycle-j-stage7b-the-matrix-column-not-the-backtick-2026-11-05.md`.
   *Inherited from G13, not introduced — G13 passes the same probe.*
   **Built without a dispatch, which was the architect's error**: stage
   7's review said "Dispatched as stage 7b" before any dispatch existed.
   No dispatch is written after the fact. **A review may recommend a
   stage; only a dispatch dispatches it.**
7c. **The column check moves onto the AST** (stage 7 §5(a), proposed by
   the implementation role and accepted at review). **DISPATCHED**
   2026-11-07 —
   `rfcs/handoffs/094-transactional-audit/cycle-j-stage7c-the-column-check-moves-2026-11-07.md`.
   `scripts/check-audit-matrix-columns.py` reads descriptors through
   `rust_registry.py`, whose own docstring says it works *"without an
   AST… nothing here can see a type"* — the defect RFC 094 exists to
   end. The descriptor side moves to `syn`; the matrix-cell side stays
   text, which is correct because a cell is text. G23's command is
   unchanged, so no A3.4 byte-match churn. — **LANDED** 2026-11-07;
   the check fires on the real matrix (a Class `A`→`B` and an Actor
   `—`→`admin user id` mutation of K07's row each fail by name), and the
   local matrix moves 48 → 47 because exactly one check — the deleted
   Python self-test step — left it. Review —
   `.git-exclude/reviewed/cycle-j-stage7c-the-column-check-moves-2026-11-07.md`.
   *Two corrections to the architect: the `audit-desync` fixture trap the
   dispatch warned of did not exist (its matrix has no Class or Actor
   column, so the Python step never ran on it), and stage 7's review had
   accepted a `CARGO_TARGET_DIR` explanation contradicting the
   architect's own stage 6b record. Process consequence:
   `.git-exclude/tmp/clones/` is the wrong place for a clone that builds
   the workspace, and `/tmp` is too small; a sibling of the tree on the
   main disk works.*
7c′. **Patch release `0.87.2`**, inserted ahead of 7d and authorized by
   `@nabbisen` 2026-11-07. It carries a `Security` entry: sign-in follows only
   a same-site return path (`8df3093`). It also carries stages 4–5's
   rotation records, so their docs' *"since v0.88.0"* becomes `v0.87.2`.
   **DISPATCHED** 2026-11-07 —
   `rfcs/handoffs/094-transactional-audit/release-0.87.2-preparation-2026-11-07.md`.
   Step 0 green (`Security audit` on `8df3093`, run `42356328349`).
   — **RELEASED AND PUBLISHED** 2026-11-07: tag `0.87.2` at `623ece8`, Level B
   `42363728891` (28/28, 0 skipped), `Security audit` green on the same commit,
   publish run `42366830929`, six crates confirmed on crates.io.
7d. **The local matrix learns what CI already knows: `--changed`.**
   **DISPATCHED** 2026-11-07 —
   `rfcs/handoffs/093-build-toolchain-release-gates/local-matrix-changed-mode-2026-11-07.md`.
   A full local run is 20.6 minutes, 93% of it in G02/G04/G05/G06, and a
   docs-only commit pays all of it although CI's `changes` job (RFC 130 D6)
   would skip those lanes. `--changed` selects by CI's rule, from the same
   `rust_scope.py`, fails open as D7 does, and states every lane it
   skipped. The full run stays the default and release preparation keeps
   it. It goes ahead of stages 8–9 because it speeds up every docs, RFC
   and review commit after it. Stages 8–9 are Rust changes, so they still
   run in full. *It reverses the script's "no fast subset" rule, which
   came from the architect's own dispatch and from nowhere else.
   The dispatch's §2 says why the defect that rule guarded against stays
   closed.* — **LANDED** 2026-11-07. A docs-and-scripts change now checks
   in about 80 seconds instead of about 21 minutes (36 run, 11 skipped,
   scope line quoted). Review —
   `.git-exclude/reviewed/cycle-j-stage7d-local-matrix-changed-mode-2026-11-07.md`.
   *The review found a defect the stage inherited from CI's own chooser*:
   git quotes a path holding a non-ASCII byte, a double quote, a backslash
   or a control character, the quoted form matches no scope glob, and
   **both** `compute-changed-scope.py` (CI's `changes` job, since RFC 130)
   and the new local chooser read such a Rust change as "no Rust change".
   That is the unsafe direction RFC 130 D7 forbids. Fixed in both with
   `-z`, with a test each that the old reading turns red.
8. **RFC 142 stage 4** — the preview subcommand. **DISPATCHED** 2026-11-07 —
   `rfcs/handoffs/142-upgrading-is-the-least-tested-thing-we-do/stage4-preview-the-upgrade-2026-11-07.md`.
   `sui-id preview-upgrade`: one definition of "pending" shared with the
   runner, migrations given their file names, and no key needed. Two things
   were measured before dispatch. A read-only open of a cleanly closed WAL
   database **leaves `-shm`/`-wal` files behind**, so a root-run preview
   would leave root-owned files beside the service's database; the dispatch
   prescribes `immutable=1` when no `-wal` exists, after a re-measure on the
   bundled SQLite 3.53.2. And the subcommand reference table it joins is
   wrong in several rows, including `admin rotate-key` described as a
   signing-key rotation; the dispatch corrects it. — **LANDED** 2026-11-07,
   matrix 47/47. *The review folded in two defects*: the `immutable` URI was
   not percent-encoded, so a `#` or `?` in a directory name made a populated
   database preview as **empty**; and the test said to prove the preview and
   the runner agree never observed the runner (a runner skipping 0045 passed
   it). It also moved `admin rotate-key`'s flags out of `setup`'s `--help`
   entry. Review —
   `.git-exclude/reviewed/cycle-j-stage8-rfc142-stage4-preview-upgrade-2026-11-07.md`.
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
3. **`rusqlite` under `[dev-dependencies]`: the RFC and the test that
   closed M2a disagree.** Opened 2026-11-04 while scoping stage 6, and
   **`@nabbisen`'s to settle.** RFC 094's structural-gate section
   requires that `rusqlite` appear in *"the `[dependencies]` or
   `[dev-dependencies]` table"* of no crate but `sui-id-store`. But
   `crates/sui-id/Cargo.toml:115` declares it under
   `[dev-dependencies]`, with a recorded reason, and **M2a's own
   criterion-7 test takes the opposite reading** —
   `tests/e2e/r094_m2a_db_access_scope.rs` states *"The clause is about
   production reach, not every Cargo edge … only a `[dependencies]`
   entry outside `sui-id-store`"* — and **M2a closed on that reading.**
   The strict reading would mean rewriting at least nine e2e files to
   inspect the database through `sui-id-store`'s API instead of raw SQL,
   including helpers stage 5 extended. The architect has **not** chosen:
   the condition is held out of stage 6 and nothing regresses while it
   waits, because the existing test already enforces the looser reading
   on every run. Settling it may be a material amendment to an Accepted
   RFC.
4. **Not in this scope, and still open**:
   [RFC 119](../../accepted/119-embedding-contract.md)'s review route
   and its two remaining questions,
   [RFC 143](../../accepted/143-installation-without-a-toolchain.md)'s
   two, RFC 140 stage 2 (the logo upload, unblocked since 2026-10-30),
   and the smaller carried items — the `test_id` resolution gate, the
   dead `consents` table, and the eight e2e files sharing the racy
   tracing idiom.
