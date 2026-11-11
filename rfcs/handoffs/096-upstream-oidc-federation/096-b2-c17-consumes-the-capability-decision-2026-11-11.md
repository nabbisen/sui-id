# RFC 096-B2 — how C17 "consumes" the preflight capability: one decision

> ## Whose turn it is: **yours**, for one decision (§3).
> **S3 is not blocked by it** and is dispatched now, because it builds only
> the capability, not its consumer. **S4 (C17) waits on it.**

**Prepared by.** The architect, 2026-11-11, before dispatching S3.

## 1 — what the accepted design says

- RFC 096 `:409-413`: preflight *"returns a private, non-cloneable,
  non-debuggable, non-serializable `ValidatedProviderPreflight`"*.
- RFC 096 `:440`: *"C17 enable consumes that capability"*. RFC 094
  (`rfcs/done/094-transactional-audit-registry.md:514`) says the same
  thing of *"the C17 enable Class-A transaction"*.

## 2 — why the type system can hold only half of it

This is **the F03 seam again**, for the same reason (RFC 096 `:17-35`, your
2026-10-14 approval):
- **The capability must be built in `sui-id`.** That is where the transport,
  discovery and JWKS validation live, and moving them into the database
  crate would pull the network and JOSE stack into it. That move was
  rejected for F03 on measured cost.
- **The enable transaction must be in `sui-id-store`.** Every Class-A
  command is declared there.
- **`sui-id-store` cannot name a `sui-id` type**, because the dependency
  runs the other way. So the store's enable command takes plain values,
  as F04 and F03 already do. A witness type and a sealed trait both fail
  for the reason F03's amendment records: whoever implements or constructs
  the witness is in `sui-id`, so it seals nothing against `sui-id`'s own
  code.

**What the compiler can still hold:** that nothing outside the preflight
module can make, copy, print or serialize a `ValidatedProviderPreflight`.
That is S3, with `trybuild` fixtures like the federated-MFA ones.

**What it cannot hold:** that the store's enable command is reached *only*
by unpacking such a capability.

**What already limits that residue.** C17's own transaction rechecks every
bound value against the stored row: the policy digest, version, generation
and disabled state (RFC 096 `:440-443`). So input not taken from a real
capability can only ever enable the policy that is already stored. What it
could do is skip the network check itself, the reachability proof, and
record evidence (fingerprints, observation time) for a check that never ran.
That residue is what the test in §3 holds.

## 3 — your decision

**Recommended: apply the F03 precedent exactly.**
- The capability is sealed by the compiler in `sui-id`, with fixtures.
- The store's enable command takes the capability's values.
- **A source-scope test in the shape of
  `crates/sui-id/tests/e2e/r096_b1_stage10b_f03_scope.rs` pins its one
  production caller**: the `sui-id` function that takes
  `ValidatedProviderPreflight` by value.
- RFC 096 `:440` gains a sentence saying so.

**Materiality.** By your 2026-10-14 ruling, a guarantee held by a test
instead of the type system is material. So, as then, RFC 096 returns to
`proposed/` and is re-accepted in the same pass, and nothing else is
reopened. If you would rather treat this as the F03 precedent applied, and
so immaterial, say so; the text is the same either way.

**The alternative**, moving discovery, JWKS and transport validation into
`sui-id-store` so `pub(crate)` holds both halves, is not recommended, for
the reason F03's amendment measured.
