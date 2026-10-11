# Decision request — F01 and F05 require a Class-B emission path that does not exist, and F01's recorded class is weaker than what ships today

**Raised.** 2026-10-11 JST, by the architect.
**Found.** Writing stage 6's dispatch, reading the two commands' recorded
shapes against the code they would use.
**Status.** **Open.** Stage 6's two retention items are dispatched as stage 6a
and depend on nothing here; **F05 and F01 wait on this.**
**Blocks.** F01, F05 — and, by the same mechanism, F03 at stage 10.

## 1. The measured facts

**F01 and F05 are recorded as Class P with a post-commit Class-B event.**
`contracts/write-commands.toml`: F01 `class = "P"`, *"post-commit
`auth.federation.signin.success` is Class B under U24/U30 policy"*; F05
`class = "P"`, *"exact Class-B sum: `auth.federation.signin.upstream_failure`,
`takeover_blocked`, `link_required`, or `signin.denied` with closed reason
enum"*. `architecture.md:268-269` states it directly: *"F01/F03 use post-commit
must-attempt Class-B sign-in success … they do not claim atomic audit."*

**The Class-B emission path does not exist.** `registry.rs:118-124`:

```rust
pub enum AuditClass {
    /// Mutation and event commit in one transaction ([`Database::class_a`]).
    Atomic,
    /// Best-effort, tracked emission (RFC 094 Class-B `emit_must_attempt`,
    /// not implemented in this Stage-1 slice — no slice command needs it).
    MustAttempt,
}
```

Grepped across `crates/` excluding `target/`: `emit_must_attempt` appears in
that one comment and nowhere else. `MustAttempt` appears in the enum, in one
`Display` arm, and in a test asserting **no descriptor uses it**
(`commands/tests.rs:161-169`, `every_descriptor_is_atomic_class`). RFC 094
`:292-298` specifies the API — *"Replace silent `let _ = audit::append(...)`
behavior with a named `emit_must_attempt` API … tests can inject failure and
assert that the failure signal occurred"* — and assigns it to no milestone.

**It is not a prerequisite 096-B1 can assume.** RFC 094 `:575-579` lists the
M2a **foundation** items, and this is not among them: the `WriteTx<Protocol>`
runner, the `WriteTx<AtomicAudit>` runner, sealed `declare_write_command!`,
`ReadConn`, the command manifest. M2a's seven closure criteria do not mention
it either. 096-B1's prerequisite is M2a foundation, which is met.

**So this is the third instance of one pattern.** RFC 094's prose declares a
control its closure criteria do not require, and work that depends on it
assumes it exists. Instance one was M2a's own exit-criteria divergence
(`../094-transactional-audit/m2a-exit-criteria-divergence-2026-10-03.md`);
instance two was `ReadConn`, declared as required M2a foundation and absent,
which became 096-B1's stage 0
([`readconn-prerequisite-decision-2026-10-09.md`](readconn-prerequisite-decision-2026-10-09.md)).
This is instance three, and it is the one the earlier two predicted.

## 2. The second fact, which points the other way for F01

**`auth.federation.signin.success` is already atomic in production, today.**
`commands.rs:2144-2148` declares `L04_SUCCESS` with
`class: AuditClass::Atomic`, and `sign_in_federated` (`:2209`) runs it on
`db.class_a` (`:2217`): `last_login_at`, the session insert, over-cap eviction and the
event commit in **one** transaction. `handlers/federation.rs`'s
`complete_federated_signin` calls it on every federated sign-in with no local
MFA — the exact path F01 is meant to take over.

**Building F01 as recorded would therefore replace an atomic audit with a
best-effort one, for the same event name, on the authentication success path.**
RFC 102 is titled *"authentication fails closed without audit"*. Downgrading
its achieved property to satisfy a row in RFC 094's manifest is the wrong way
round, and no reviewer reading the diff a year from now would guess it was
deliberate.

F05 is not in that position: its denial events are today plain
`let _ = audit::append(...)` with an unregistered action string, so **anything**
registered is an improvement.

## 3. What I recommend

**Split them, because the two commands are not in the same position.**

**F01 → Class A, reusing L04 rather than adding a parallel command.** Add
what F01 has and L04 lacks — the guarded `federation_login_attempt
exchanging -> completed` transition, and the enabled-provider and
same-active-link/user authority checks — *inside* L04's existing Class-A
transaction, and record F01 in the manifest as the Class-A command it then is.
Nothing is downgraded, no second command appears for one user-visible outcome,
and the attempt completion becomes atomic with the session it authorises, which
is a property the recorded design does not offer at all.

**F05 → build `emit_must_attempt` first, then F05 on it.** Here the Class-B
shape is the right one and RFC 094 `:120`'s A6 is the reason: failure to log a
denial must never prevent the denial. An atomic F05 would roll the
terminalization back when an append fails, which leaves the row un-terminalized
on exactly the path that most needs the record. `emit_must_attempt` is small —
a named API returning an outcome, structured error telemetry on failure, and
failure injection so a test can assert the signal fired — and it retires every
remaining `let _ = audit::append(...)` in the federation handler as a
by-product. **It becomes stage 6b, before F05.**

**And the reconciliation this makes overdue.** The authorized plan already
carries *"RFC 094's prose-versus-criteria reconciliation — a proposal, not a
decision"*, recommended after stage 0. Three instances in, I recommend taking
it now rather than after 096-B1: the amendment should state which declared
controls are closure-required and which are not, so a dependent RFC can tell
what it may assume. Both changes above touch RFC 094 — F01's manifest class and
`emit_must_attempt`'s milestone — so they belong in one amendment rather than
two.

## 4. What a decision needs to cover

1. **F01 as Class A, folded into L04** — or kept as recorded, accepting the
   audit downgrade.
2. **`emit_must_attempt` as stage 6b**, before F05 — or F05 built atomic
   instead, accepting that an append failure leaves the attempt row
   un-terminalized.
3. **The RFC 094 amendment** carrying both, now rather than after 096-B1.

Items 1 and 2 are independent; 3 follows from either being taken.

**Nothing here changes the authorized stage order** — it inserts 6b and names
F01's seam. Stage 6a is dispatched and unaffected either way.
