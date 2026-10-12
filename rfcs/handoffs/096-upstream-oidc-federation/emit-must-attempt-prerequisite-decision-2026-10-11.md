# `emit_must_attempt` — a second declared prerequisite that does not exist, and the decision taken

**Recorded.** 2026-10-11 JST, by the architect.
**Status.** **Settled on the build.** `@nabbisen`, 2026-10-11: "Your
recommendation is accepted." Built as 096-B1's **stage 6b**, before F05.
**One part is not settled and is flagged in §4** — the RFC 094 amendment the
same decision called for may, under RFC 000, return RFC 094 to `proposed/`,
which I did not name when recommending it.
**Why it is in git.** The gap is scheduled work rather than an unfixed
weakness, so the reasoning belongs where a reader can find it — the same
reasoning as [`readconn-prerequisite-decision-2026-10-09.md`](readconn-prerequisite-decision-2026-10-09.md).
**Placement note.** In the handoff package rather than amended into RFC 096's
body, for the same reason as the `ReadConn` record: an amendment shifts every
subsequent line and invalidates the bare `:NNN` citations this project uses
heavily.

## 1. The finding

RFC 094 `:292-298` specifies the API:

> Replace silent `let _ = audit::append(...)` behavior with a named
> `emit_must_attempt` API. It returns/records an outcome and always emits
> structured error telemetry on failure. Callers may preserve the primary
> denial or detection response, but tests can inject failure and assert that
> the failure signal occurred. Raw append remains internal to the audit
> subsystem.

Measured 2026-10-11:

| Piece | State |
|---|---|
| `AuditClass::MustAttempt` | declared, `registry.rs:118-124`, with the comment *"not implemented in this Stage-1 slice — no slice command needs it"* |
| `emit_must_attempt` | **absent** — the identifier appears in that one comment and nowhere else in `crates/` |
| any descriptor of class `MustAttempt` | **none**, and `commands/tests.rs:162`'s `every_descriptor_is_atomic_class` asserts there are none |
| best-effort `let _ = … audit::append(…)` sites in production | **21**, across 13 files; **4** of them in `handlers/federation.rs` |

F01, F03 and F05 each specify a post-commit Class-B event
(`contracts/write-commands.toml`, and `architecture.md:268-269`). **None of the
three can be built as recorded until this exists.** F05 is 096-B1's next
command, so the gap is immediate rather than theoretical.

**It is not something 096-B1 could have assumed.** RFC 094 `:575-579` lists the
M2a **foundation** items — the `WriteTx<Protocol>` runner, the
`WriteTx<AtomicAudit>` runner, sealed `declare_write_command!`, `ReadConn`, the
command manifest — and this is not among them. M2a's seven closure criteria do
not mention it either. 096-B1's prerequisite is M2a foundation, and M2a
foundation is met.

**This is the third instance of one pattern**, and the second to land on
096-B1: RFC 094's prose declares a control its closure criteria do not require,
and work that depends on it assumes it exists. Instance one was M2a's own
exit-criteria divergence
(`../094-transactional-audit/m2a-exit-criteria-divergence-2026-10-03.md`),
instance two was `ReadConn` (stage 0), and this is instance three.

## 2. Why build it here rather than wait for RFC 094

The same reasoning stage 0 settled, and the precedent is now established:
096-B1 is the RFC that implements F01–F06 (`:92-93`), the piece is small and
fully specified, and waiting would mean either building F05 on a weaker shape
or stalling 096-B1 behind a milestone that does not list the piece.

**Scope line, so this does not become an audit refactor.** 096-B1 needs the
mechanism and the events F05 emits. It does **not** need the workspace-wide
conversion: *"raw append remains internal to the audit subsystem"* can only be
true once all 21 sites are converted, and RFC 094's **M2b** already owns *"no
production Class-A best-effort append anywhere"*. So stage 6b builds the
mechanism and proves it on one site; the remaining 20 are M2b's, listed in the
stage 6b dispatch so M2b's inventory does not have to rediscover them.

## 3. What the decision covers

1. **`emit_must_attempt` is built as stage 6b**, before F05 (stage 6c).
2. **F05 is then built on it** rather than built atomic — because RFC 094
   `:120`'s A6 is the reason the Class-B shape is right *here*: failure to log a
   denial must never prevent the denial, and an atomic F05 would roll the
   terminalization back on exactly the path that most needs the record.
3. **F01 goes the other way — Class A, folded into L04** — because
   `auth.federation.signin.success` is already atomic in production
   (`commands.rs:2144-2148`, `sign_in_federated` on `db.class_a` at `:2209`,
   `:2217`), and building F01 as recorded would downgrade RFC 102's achieved
   property on the authentication success path.

Items 1 and 2 need no RFC amendment: they implement accepted text. **Item 3
does**, and §4 is about what that costs.

## 4. The consequence I did not name when I recommended the amendment

> **Superseded 2026-10-12** by
> [`five-open-decisions-reviewed-2026-10-12.md`](five-open-decisions-reviewed-2026-10-12.md).
> This section's recommendation — build F01 now, record the divergence, amend
> RFC 094 later — rested on *"no gate ties the manifest's class to RFC 094's
> prose"*, which is a statement about what is checked and not about what is
> true. Measuring the round-trip also found it blocks far less than claimed
> (M2b only, which is unstarted) and one thing not claimed at all (RFC 096's
> own *Design prerequisites* line becomes false while RFC 094 sits in
> `proposed/`). The replacement sequences the amendment into a gap with nothing
> in flight. The text below is kept as the original reasoning.

The decision document recommended that one RFC 094 amendment carry F01's class
and `emit_must_attempt`'s milestone. **I did not say what amending RFC 094 may
cost, and it may cost a great deal.** RFC 000 (`rfcs/done/000-rfc-lifecycle-policy.md:43-47`,
`:398-401`):

> A material change to security invariants, public behavior, scope, or
> prerequisites atomically returns an Accepted RFC to `proposed/`, removes
> active-looking acceptance metadata, updates the index and inbound links …

F01's class determines whether the sign-in audit record commits atomically with
the session it authorises. On the honest reading **that is a security
invariant**, so the amendment is material, and a material amendment returns RFC
094 to `proposed/` — which is **build-prohibited**. RFC 094 gates 096-B1 and
every other lane that depends on the command registry, so the amendment as I
framed it could halt the project it was meant to tidy.

**Two facts narrow the problem.**

- **No gate ties the manifest's class to RFC 094's prose.** Measured:
  `scripts/check-write-commands.py` requires `sealed = true ⟹ class = "A"` and
  nothing more; it reads no RFC. `check-audit-matrix-columns.py` rejects a
  Class-B matrix row only when a sealed ***Atomic*** descriptor carries the
  name, so a `MustAttempt` descriptor is compatible with a B row. So the gates
  pass with F01 as Class A **without** any amendment; what would be wrong is
  the *record*, not the build.
- **F01 is two stages out** (6b, then 6c/F05, then F01), so there is runway to
  settle this without blocking anything.

**What I recommend now**, having found the cost:

1. **Build 6b and F05 without touching RFC 094.** Both implement accepted text.
2. **For F01, record the class decision in RFC 096's own handoff** — RFC 096 is
   the RFC that implements F01 — and update `contracts/write-commands.toml`'s
   F01 row with a `rationale` that names the conflict and its resolution. The
   divergence from RFC 094's prose is then *documented in the open*, which is
   the thing this project refuses to leave implicit, without a lifecycle move.
3. **Then take the RFC 094 amendment deliberately, at a moment when a
   `proposed/` round-trip is affordable** — it needs re-acceptance, not
   re-design, and it should carry all three recorded divergences at once (the
   M2a exit-criteria consolidation's successor, `ReadConn`, and this). My view
   is that the right moment is after 096-B1 closes, not during it.

**This reverses the "amendment now" half of what was accepted**, and it does so
because of a consequence I failed to put in front of you the first time. The
other two items stand as accepted. If you would rather amend now and accept the
round-trip, say so and I will sequence 096-B1 around it.
