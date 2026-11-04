# Cycle J stage 7 — the structural gate becomes the coverage authority

> ## Whose turn it is: **the implementation role's.**
> **One of the two items this stage was scoped for is already done.**
> §1 says which and why, with the evidence. What remains is item 3, and
> it is not a documentation change: `audit-structure` must *perform* the
> coverage check before it can be the authority for it.

**Dispatched.** 2026-11-04, by the architect.
**Stage.** 7 of nine. §D of
`rfcs/handoffs/094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md`.
**Closes.** RFC 094 `:1105-1107` item 6's third clause —
*"downgrade/remove the string gate as authority"*.
**Scope.** Item 3 only. **Not** item 2 (§1). **Not** the adversarial
evidence package (item 5, stage 9's and the owner's).
**Baseline.** The tip of `main` when you start — `git log -1`, record it.
**Status check.** RFC 094 is **Accepted**. Building is permitted.

---

## 1 — item 2 is already discharged, and you should not do work for it

§D lists this stage as *"make the structural gate blocking; downgrade
the string gate from authority"*. **The first half is done**, measured
rather than assumed:

- G23 is a `[gates]` key (`contracts/gate-inputs.toml:111`), so it is a
  Level B lane and required for every release claim.
- **`continue-on-error` appears zero times in `.github/workflows/ci.yml`.**
  This project has no non-blocking lane mechanism at all, so a
  registered lane is a blocking lane by construction.

Landing the structural gate as a registered lane *was* making it
blocking. Record that in your return with the evidence; do not invent a
blocking switch that has no counterpart in the tree.

## 2 — what item 3 actually requires, in RFC 094's own words

> *"When `audit-structure` lands at M2b it becomes the authoritative
> coverage gate; G13 then remains for vocabulary-drift diagnosis or is
> retired, by amendment."* (`094-…md:946-948`)

G23 today checks **write-authority** and the **write-command
inventory**. The **coverage** check — every audit event literal in
`crates/` has a row in `contracts/audit-coverage-matrix.md`, and every
row has a literal — is still G13's, in `scripts/check-audit-matrix.sh`.

**So `audit-structure` must subsume the bidirectional coverage check.**
Authority follows capability: a gate cannot be the authority for a
property it does not check.

## 3 — the one instruction that matters: do not transcribe the string script

RFC 094 is explicit about why the switch exists:

> *"The list is now derived on every run from the three places the code
> writes an audit action. That closes the lag; it does not close the
> class. **A string gate can only see literals in namespaces the code
> already declares**, which is one more reason `audit-structure` is the
> authoritative coverage control at M2b."* (`:952-958`)

G13-b already fixed the hand-maintained namespace list — `:17` of the
script says so. **The residual defect is structural, not a stale
list**: string extraction can only find event names where it already
knows to look.

**Build it on the AST.** Event names reach the audit log through typed
paths you already parse for the write-command checks —
`EventDescriptor`'s `name` field, `declare_write_command!`'s mappings,
and the `AuditLogRow { action: … }` construction sites. A port that
greps `crates/` for `word.word` string literals inherits the exact
defect that justifies the switch, and would make the authority claim
false while the gate passes.

**If the AST cannot reach some emitter path, say so and name it.** A
named gap is this project's standard; a silent one is the failure mode
the whole cycle has been correcting.

## 4 — G13 stays, and retiring it is not yours or mine

RFC 094 offers two endings: G13 *"remains for vocabulary-drift
diagnosis **or** is retired, by amendment"*. Retirement needs an
amendment, which is `@nabbisen`'s.

**So: G13 remains.** Change what it *is*, not whether it runs — its
`[lane_profiles]` title already says `(interim, RFC 094)`, and the
documentation that currently points at it as the coverage control must
point at G23 instead. Leave the lane registered and green.

**Do not delete `scripts/check-audit-matrix.sh`.** Stage 6b correctly
deleted `check-write-commands.py` because RFC 094 required G17 retired
*"in the same change"*; it says the opposite here.

## 5 — two things to measure and propose, not to decide

**(a) G13's second step.** The lane also runs
`scripts/check-audit-matrix-columns.py` (RFC 116 stage 2), which holds
the matrix's `class`, `actor` and `step_up` columns against the sealed
descriptors and *"reads table rows, not names"*. That is column
agreement, a different property from coverage. **Measure whether it
belongs in `audit-structure` too, and propose** — the RFC's "coverage
gate" sentence does not obviously reach it, and I am not deciding it
from the outside.

**(b) the other event-name consumers.** RFC 094 `:942-944` says G13
*"compares the matrix against source literals and nothing else, which
is why three event-name consumers drifted past it"*. If that is the
stated weakness, an authoritative coverage control plausibly covers the
consumers too.

**Here is one, measured today, so the question is not abstract:**
`crates/sui-id-i18n/src/strings.rs` declares **29 `audit_event_*`
keys**, and `grep -rn "audit_event_" crates/ --include="*.rs"`
excluding the i18n crate returns **one** hit, which is an unrelated
test function name. Every one of those 29 label keys is dead, including
`audit_event_admin_master_key_rotated`, and **no gate notices** — a
consumer that has already drifted, exactly as the RFC predicts.

**Propose the consumer set; do not build it blind.** `docs/src/reference/audit-events.md`
is the other obvious one. Say what each consumer is for, which drift it
would catch, and what it costs. **If the honest answer is that this is
its own stage, say that** — I have mis-sized this cycle twice and your
measurement rounds have been better than my estimates.

## 6 — the constraints that bite

- **G23's command string is byte-matched** by A3.4 condition 7 against
  the row in RFC 094's *"Gate Matrix lanes owned by RFC 094"* table. If
  the command gains a flag or an input path, **both change in the same
  commit**, and the heading is not renamed.
- **Adding a contract input** means a `contracts/README.md` row naming a
  gate already in `[gates]` (G18, via `contract-paths.toml`) — G23
  qualifies, so this is satisfiable, unlike stage 6b's chicken-and-egg.
- **`ci.yml` is regenerated**, never hand-edited.
- The `.cargo/config.toml` alias now passes `--locked` to **cargo**
  (added at stage 6b's review). Leave it there; the binary's own
  `--locked` remains a documented no-op.
- No restating a gate command in prose (A3.4).
- No `audit.yml` run, no Level B claim. Both readings stay with me.

## 7 — `CHANGELOG`

**None, and say so.** Developer-facing tooling: a gate gains a check and
another stops being cited as the authority. Nothing an operator or a
library consumer observes changes.

## 8 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage7-the-structural-gate-becomes-the-authority-2026-11-04.md`:
the baseline; the item-2 evidence from §1; how the coverage check is
derived from the AST rather than from string extraction, with the
mechanism named; any emitter path the AST cannot reach; your proposals
for §5(a) and §5(b); per-hunk hashes; and the gate summary **run on the
real tree after copy-back**.

**And the equivalence bar from stage 6b applies again, for the same
reason.** You are taking over a check that is currently the only control
over the audit-coverage matrix. Show, per direction of the bidirectional
check, one case where the new implementation fails — and show that a
name G13 catches today is still caught. A coverage gate that passes
because it looked in fewer places is the precise failure this item
exists to end.
