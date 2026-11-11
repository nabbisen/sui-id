# RFC 096-B2 — how C17 "consumes" the preflight capability: one decision

> ## Approved 2026-11-11. `@nabbisen`: *"Approved."*, on §3 as revised.
> Applied to RFC 096 at `:390` and `:440`, and recorded in its header. As
> recommended, RFC 096 returned to `proposed/` and was re-accepted in one pass,
> and no line number moved. **S4 is dispatched** (2026-11-12, after S3 landed) —
> [`096-b2-stage4-c17-enable-and-disable-2026-11-12.md`](096-b2-stage4-c17-enable-and-disable-2026-11-12.md).
> *Corrected 2026-11-12:* the RFC 094 sentence is at `:515`, not `:514` (an
> off-by-one in the architect's read, fixed here and in RFC 096 `:440`).

**Prepared by.** The architect, 2026-11-11, before dispatching S3.

## 1 — what the accepted design says

- RFC 096 `:409-413`: preflight *"returns a private, non-cloneable,
  non-debuggable, non-serializable `ValidatedProviderPreflight`"*.
- RFC 096 `:440`: *"C17 enable consumes that capability"*. RFC 094
  (`rfcs/done/094-transactional-audit-registry.md:515`) says the same
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

## 3 — your decision (revised 2026-11-11, after checking it against both halves)

> `@nabbisen`, on the first version of §3: *"Your recommendations are almost
> approved. However, verify carefully they meet my fundamental philosophy."*
> §4 is that check. It changed the recommendation in five places, two of them
> corrections to the architect's own S3 dispatch. Below is the recommendation
> as it now stands.

**Recommended:**
1. **The compiler seals the capability** (S3). Nothing outside the preflight
   module can construct, copy, print or serialize a
   `ValidatedProviderPreflight`, and `trybuild` fixtures pin each of the four.
2. **One function both checks and enables, and nothing holds a capability
   between requests.** RFC 096 `:389-391` already asks for *"a successful
   discovery/JWKS preflight for that exact version immediately consumed by C17
   audited enablement"*. So the function that runs the preflight moves its
   capability straight into C17, in the same call. S5's administrator action is
   one button, "Check and enable", with an optional "Check only" for diagnosis.
3. **The store's enable command takes one named struct, not positional
   values.** Its fields are distinct types: the metadata and JWKS
   fingerprints, and the version and generation. A swap then fails to
   compile instead of being stored silently. Its name and doc state who may
   call it.
4. **A source-scope test, wider than F03's.** Across every crate's production
   source (`crates/*/src`), the store's enable command may be named in only
   two places: its definition, and the one file that holds the capability's
   private constructor. A mutation that adds a call anywhere else must turn
   it red.
5. **One clock for the 600-second window.** C17 reads `guard_at` from the
   store's own clock, inside the transaction, after `BEGIN IMMEDIATE`. The
   preflight reads `observed_at` from the same source, and not from the
   application's injectable clock. S4 adds a test-only seam on the store's
   clock for the 599/600-second boundary tests.
6. **Every refusal says what to do next.**
   - **The check's own refusals** (S3): not disabled, not fully configured,
     transport, discovery, JWKS, auth method unsupported, no common algorithm,
     changed during the check.
   - **C17's**: *"the provider changed after the check; run it again"*,
     *"the check is more than 10 minutes old; run it again"*, *"already
     enabled"*, and *"no longer in the configuration"*.
   - None of them collapses into a generic failure.
7. **The text.** It is material by your 2026-10-14 ruling, so RFC 096 returns
   to `proposed/` and is re-accepted in one pass, and nothing else is
   reopened. The amendment sits at `:440` and `:389-391`, and states how RFC
   096 `:443`'s *"still configured"* is held: the check requires the
   provider's configuration block, configuration is fixed for the life of
   the process, and consumption is immediate. **RFC 094 stays as written.**
   RFC 000 `:89` makes a done RFC a historical record, so RFC 096's amendment
   names RFC 094 `:515` and says how that sentence is now held.

**Still held by a test, not the compiler:** item 4, that only that one file
calls the store's enable command.

**The alternative**, moving discovery, JWKS and transport validation into
`sui-id-store` so `pub(crate)` holds both halves, is not recommended, for
the reason F03's amendment measured.

## 4 — the check against both halves

**Half 1 — clean, safe and secure, robust and sophisticated.**
- **Copying F03's test would not have held.** F03's test bans the store
  command's name from the whole `sui-id` crate. That works because F03's
  one legitimate caller is in a different crate, `sui-id-core`. For C17 the
  preflight, its consumer and S5's handlers are all in `sui-id`, so a
  crate-wide ban is impossible, and a loose "allowed somewhere in `sui-id`"
  would hold nothing. Item 4 narrows the scope to one file, across all crates.
- **The 600-second window would have measured clock skew.** My S3 dispatch
  takes `observed_at` from `app.clock`. The store reads `chrono::Utc::now()`
  (`registry.rs:1200`) and cannot name `sui-id-core`'s clock type. Two
  sources would make the window measure the difference between clocks, not
  the age of the check. That is item 5, and the S3 dispatch is corrected.
- **A capability held between requests was unaddressed.** It cannot be
  serialized, so carrying it from a "check" request to an "enable" request
  would have needed a server-side store of capabilities. That is a new place
  for state, with its own expiry and keying, all to support a flow RFC 096
  `:389-391` does not ask for. Item 2 removes it.
- **Two same-typed positional arguments are a silent-swap hazard.** Both
  fingerprints are `[u8; 32]`, and both counters are `i64`. That is item 3.

**Half 2 — APIs and UI/UX nobody misunderstands.**
- **The developer is a user of the store's API.** A public `enable` taking
  plain values reads as "the way to enable a provider". Items 3 and 4 make
  its contract visible in the signature and the doc, and enforced.
- **The operator.** Separate "check" and "enable" actions invite *"I checked,
  why won't it enable?"*, the stale-capability case. One action removes that
  case, and item 6 says what to do in every remaining one.
- **The record.** Offering you an immaterial option against your own
  precedent left two readings of one rule. Item 7 recommends one.
