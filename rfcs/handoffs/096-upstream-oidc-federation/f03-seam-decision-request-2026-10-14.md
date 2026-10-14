# RFC 096-B1 — the F03 seam: a decision request, before stage 10b

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

**The question, in one line:** A with the gate and an RFC 096 sentence
recording the seam, or B with the primitive move and the stage 11
dependency question brought forward?

Nothing is built either way until this is answered. If A, stage 10b's
dispatch carries the gate as part of its own scope.
