# RFC 096-B1 — the F03 seam: a decision request, before stage 10b

> **Answered 2026-10-14.** `@nabbisen` approved option A-plus-four on
> 2026-10-14, in his own words: *"OK. Approved."* The approval is on the revised
> recommendation below (option **A-plus-four**: the scope test, F03's own
> signature doc, the `write-commands.toml` note, and the RFC 096 amendment),
> not on the first draft, which he returned by asking whether it satisfied
> his fundamental philosophy. It did not: half 2 was unaddressed, and the
> four parts are what answer it. The architect wrote both the recommendation
> and this record, so this is not an independent review; `@nabbisen` is the
> approver, which is what RFC 000 requires when no independent role exists.
>
> **What this does not yet settle, raised after the approval and not covered
> by it — see "The lifecycle consequence" at the end of this document.** The
> amendment this approval contains is a material change to a security
> invariant, and RFC 000 `:43-47` makes that a round-trip through
> `proposed/` for RFC 096. Stage 10b's dispatch waits on it.


**Raised.** 2026-10-14 JST, by the architect, for `@nabbisen`.
**Why it is here and not in a dispatch.** The answer changes how an
**Accepted** RFC's sentence is satisfied. That is not the implementation
role's to choose, and I do not think it is mine alone either.
**Blocks.** Stage 10b (F03) only. Stage 10a is complete and landed.

## The sentence

RFC 096 `:758-762`:

> Only the verifier selects the closed rejection reason; **handlers can
> neither construct a candidate nor supply a row-touching reason.**

Stage 10a holds this where it can. `ValidTotp`, `ValidRecovery` and
`BoundRejected` have private fields and no public constructor, and five
`trybuild` fixtures pin it — a handler cannot build one, cannot choose a
rejection reason, cannot print, clone or serialize one.

## Why it stops at the next crate boundary

Three measured facts, none of them negotiable:

- `sui-id-core` depends on `sui-id-store`; `sui-id-store` depends on
  `sui-id-shared` only. The edge runs one way, so **`sui-id-store` cannot
  name a `sui-id-core` type in any signature**.
- F03 is Class P, so it must live in `sui-id-store`: `WriteTx::tx()` is
  `pub(crate)` to that crate (`registry.rs:105`).
- `sui_id_store::commands::*` is public and `sui-id` handlers already call
  it directly in **eight** places, stage 9's own F02 among them
  (`handlers/federation.rs:1670`).

So F03 will be a `pub` store command taking plain primitives — a step, a
sealed blob pair, a reason — and a handler can call it with values of its
own choosing without going near the verifier. The candidate family holds
`:758-762` at the `sui-id-core` → `sui-id` boundary and cannot carry it
across the `sui-id-store` one.

Rust offers no way out of this. A witness type would have to live in
`sui-id-store` to appear in F03's signature, and anything there that
`sui-id-core` can construct, `sui-id` can construct too; making its
constructor `pub(crate)` locks `sui-id-core` out as well. A sealed trait
fails for the same reason. I checked both before writing this.

## Revised 2026-10-14, before any answer

`@nabbisen` asked whether the recommendation below satisfies his
fundamental philosophy, and told me to review it carefully. It did not,
and the re-examination changed three things. The option is unchanged; the
argument for it and the shape of it are not.

**1. The enforcement is not a new gate. It is an existing idiom.** I wrote
"a contract file plus a gate", which undersells it and would have had
someone build something new. `crates/sui-id/tests/e2e/r094_m2a_db_access_scope.rs`
already does exactly this job for RFC 094 M2a: it reads every crate's
`Cargo.toml` and asserts that `rusqlite` appears outside `sui-id-store`
nowhere but `[dev-dependencies]`, with a failure message naming the rule and
the RFC it comes from. It is an ordinary test under `cargo test --workspace`,
so G02/G04/G05/G06 run it; no new gate, no new contract file. The F03 seam is
the same shape one level down — a source scan rather than a manifest scan.

**2. Option B's cost is bigger than I said, and measurably so.** I relayed
"already-reviewed, security-sensitive code" from the implementation role's
package without measuring it. Measured: `verify_password`
(`authn/password.rs:96-113`) takes a permit from `HASH_SEMAPHORE`, a
`static OnceLock<Semaphore>` bounded at `min(cores * 2, 16)` and shared with
`hash_password` — its own doc calls it "the live, process-wide semaphore".
Duplicating the primitive into `sui-id-store` gives the process **two
independent bounds**, so the cap silently becomes up to 32 concurrent Argon2
operations instead of 16 — at the costliest call site in the codebase, since
one recovery-code guess runs up to `RECOVERY_CODE_COUNT` sequential
verifications (`mfa.rs:366-372`). Moving the semaphore down instead means
`sui-id-store` owns password hashing, which is not "move a primitive" at all.
That is a robustness argument against B, not a sentiment about touching
reviewed code.

**3. The recommendation failed half 2 of the philosophy, and that is the
real correction.** Under A there are two ways to promote a federated second
factor that both compile: through the verifier, or by calling F03 directly
with primitives of one's own choosing. A developer reading `commands.rs`
sees F03 in a list of 103 commands, three of which (F02,
`provision_federated_user`, `sign_in_federated`) *are* called straight from
handlers, and nothing in F03's shape says it is the exception. "APIs and
UI/UX for users not to be confused or misunderstand" covers the next
developer, and a public function whose wrong use is as easy to write as its
right use is the failure that half names. A scope test catches it after the
fact; it does not stop the misunderstanding.

**So option A is now A-plus-four**, each part an idiom this project already
uses rather than anything invented:

- **the scope test**, in the shape of `r094_m2a_db_access_scope.rs`, with a
  failure message that names the rule, the RFC clause and what to call
  instead — teaching, not just refusing;
- **F03's own doc, at the signature**, stating that it trusts its caller,
  naming the one legitimate caller and naming the test that enforces it —
  exactly what stage 10a's `verify` already does for
  `presented_csrf_body_value`, which documents at the parameter that the
  caller must source it from the request body because the function "has no
  way to enforce that itself, and says so rather than silently assuming it";
- **a note on F03's row in `contracts/write-commands.toml`**, where someone
  auditing commands actually looks — the inventory already carries this kind
  of note in `rationale` (see U-row `:363`, which records that
  `sessions::insert` is crate-private so "nothing outside `sui-id-store` can
  name them", and `:839`);
- **the RFC 096 amendment, which is not optional.** Without it the RFC says
  handlers cannot and the code says they can, and "finally clean" forbids
  parking a known contradiction. With it, the document is true.

Half 1 is then satisfied by the seam being enforced on every commit by an
existing mechanism; half 2 by the wrong path being documented as wrong at
the three places a developer actually reads — the signature, the inventory
and the failure message — rather than only in a test they meet after writing
the bug.

## The two real options

**A — accept the seam, and make it mechanical rather than conventional.**
The type system holds the property up to the store boundary; a contract
file plus a gate holds it across. The project already works this way:
`contracts/write-commands.toml` enumerates 103 commands under G17, and
`check-federation-egress.py` is G19. A check that **no `sui-id` source
names the F03 command** is the same shape and would fail CI the moment a
handler reached past the verifier.

- *Cost.* `:758-762` is then satisfied by a gate at one seam rather than by
  the compiler at both. That is weaker, and the RFC does not say "a gate
  ensures"; it says handlers cannot.
- *Gain.* Nothing already reviewed is touched, and the rule is enforced on
  every commit rather than remembered.

**B — move the proof primitives into `sui-id-store`** so the verifier sits
beside F03 and `pub(crate)` becomes the whole enforcement.

- *Cost.* Recovery-code matching calls `password::verify_password` — argon2,
  `sui-id-core`'s own password-hashing surface, the same one the **local**
  password path uses. Moving or duplicating it is a change to reviewed,
  security-sensitive code to buy a compile-time property on the federated
  path.
- *And it does not generalize.* Stage 11 adds WebAuthn to this same
  candidate family. `webauthn-rs` is a `sui-id-core` dependency; under B the
  verifier is in the store crate, so stage 11 would have to add
  `webauthn-rs` to the crate that owns the database — the owner-visible
  dependency trade — or lose the property again for the third method. B
  therefore buys the guarantee for two methods of three and defers the same
  question to stage 11.

## What I recommend, and what is yours

**I recommend A**, on B's second cost rather than its first: an enforcement
that holds for TOTP and recovery and then breaks at WebAuthn is not an
enforcement, and B reaches that break in one stage's time. A holds all three
uniformly, at the price of being a gate rather than a type.

What I am not willing to decide alone is the concession inside it: under A,
an Accepted RFC's "handlers can neither" is true of the candidate and true
in CI of the command, and not true of the Rust type system at the store
seam. Recording that honestly is part of the choice — RFC 096 would want a
sentence saying so, which is an amendment to an Accepted RFC and yours under
RFC 000.

**The question, in one line:** A-plus-four — the scope test, the
signature doc, the inventory note and the RFC 096 amendment — or B with the
primitive move, the doubled Argon2 bound to solve, and the stage 11
dependency question brought forward?

**One thing I checked and will not offer as a third option.** Removing
`sui-id-store` from `sui-id`'s `[dependencies]` would make F03 unreachable
from handlers by the compiler, which is what both halves would most like.
`sui-id` names `sui_id_store` **272** times across thirteen module paths
(147 `repos`, 46 `models`), so that is its own RFC and not a thing stage 10b
can carry. Measured, not assumed, because the honesty of a two-option
framing depends on it.

Nothing is built either way until this is answered. If A, stage 10b's
dispatch carries the gate as part of its own scope.

## The lifecycle consequence, found after the approval

I checked RFC 000 before dispatching stage 10b, and the approval has a cost
neither of us named while deciding. It does not change the decision; it
changes when anything can be built on it.

**RFC 000 `:43-47`** (and again at `:398-401`):

> A material change to security invariants, public behavior, scope, or
> prerequisites **atomically returns an Accepted RFC to `proposed/`**, removes
> active-looking acceptance metadata, updates the index and inbound links, and
> preserves the old decision in history or a superseded-review note.

and `:49-50`: "Only `accepted/` makes implementation eligible."

RFC 096 `:759-760` currently reads "handlers can neither construct a candidate
nor supply a row-touching reason." Stage 10a holds the first half by the
compiler. The amendment records that the second half is held, at the
`sui-id-store` seam, by a scope test rather than by construction. **A
type-system guarantee and a test-enforced one are not the same assurance, so
this is a material change to a security invariant, not a clarification** — and
I would rather say so than argue it down to avoid the round-trip, which is the
same instinct as parking the contradiction.

So, in order:

1. RFC 096 moves to `proposed/`, atomically with the amendment, its acceptance
   metadata removed and the 2026-08-27 decision preserved as a superseded-review
   note. Index and inbound links in the same change.
2. It is reviewed and re-accepted in its amended form.
3. Only then is stage 10b dispatched, carrying the four parts in its scope.

**Between 1 and 2, no 096-B1 implementation is eligible** — not stage 10b,
and not 11, 12, 14 or 6d either, since all of them build against this RFC.
Stage 10a is already landed and unaffected.

**There is precedent and it is this RFC's own.** `ROADMAP.md`: "RFCs 094, 095
and 096 were returned to `proposed/` on 2026-07-28 for owner-approved material
amendments, and were re-accepted on 2026-08-27."

**The amendment text, drafted so the round-trip is one review rather than an
open gap.** Replacing the second half of `:759-760`:

> Only the verifier selects the closed rejection reason, and no handler can
> construct a candidate: the candidate types are `sui-id-core`'s with private
> fields and no public constructor, which `crates/sui-id/tests/compile_fail/`
> pins. A handler also cannot supply a row-touching reason, but that half is
> not held by the type system: F03 is Class P and must live in `sui-id-store`,
> which cannot name a `sui-id-core` type, so F03 is a `pub` command taking
> plain primitives and the rule that only the verifier's own promotion path
> may call it is asserted by a source-scope test in the shape of RFC 094 M2a's
> `r094_m2a_db_access_scope.rs`. The distinction is recorded here rather than
> left for a reader to infer from the crate graph.

**What I have not done.** I have not moved the file or edited the RFC. Doing
either makes every remaining 096-B1 stage build-ineligible the moment it
happens, which is a scheduling act, and scheduling is proposed here and
settled by `@nabbisen`. Say when, and whether the drafted sentence is the
one to carry.
