# Cycle G and `0.85.0` — scope proposal

> ## Whose turn it is: **yours, for the scope decision.**
> **`@nabbisen`, 2026-10-25:** *"Then scope 0.85."* This is the proposal;
> the authorization is his. One finding in §D needs a ruling before wave 3
> can ever be scheduled, and it is not blocking `0.85.0`.

**Measured 2026-10-25** against `47eabdf`, after `0.84.0` shipped. Every
file:line below was read today, not carried from the `0.84.0` scope
document.

## A — what M2b still owes

RFC 094 `:1093-1098` lists M2b as two things: the remaining conversion
waves, and the authority switch. After `0.84.0`:

| | | state after `0.84.0` |
|---|---|---|
| Wave 1 | server/security settings, S01–S06 | **done**, `f77c85b` |
| Wave 2 | pending sensitive settings, S07–S09 | **done**, `594c970` + `b971955` |
| Wave 3 | federation configuration, C16–C18 + C23 | **not done** — and see §D |
| Wave 4 | client administration, C01–C10, C21, C22, K02, K03 | **not done** |
| Item 6 | the authority switch | **not done**, and must come last |

M2b's closure field (RFC 094 `:54`): *"every remaining Class-A row
converted; no production Class-A best-effort append anywhere; the AST
boundary gate landed, **with a negative fixture that rejects an
unregistered write**"*, plus acceptance of the evidence on the architect's
adversarial assessment.

## B — wave 4, re-measured today

The `0.84.0` scope document called wave 4 *"the same shape, nine times"*.
That still holds, verified against today's tree rather than quoted:

`crates/sui-id-core/src/identity/admin.rs:31-51` — `audit_ok` delegates to
`audit_with_note`, whose body is `let _ = audit::append(...)`. The result
is discarded; there is no path by which a failed append reaches the
caller.

Nine live call sites, all after the mutation has already committed:

```
identity/admin/clients.rs:113   client.create
identity/admin/clients.rs:144   (audit_ok)
identity/admin/clients.rs:171   (audit_ok)
identity/admin/clients.rs:212   client.update
identity/admin/clients.rs:262   client.update
identity/admin/clients.rs:285   (audit_with_note)
identity/admin/clients.rs:321   (audit_with_note)
identity/admin/clients.rs:392   (audit_with_note)
identity/admin/signing_keys.rs:94  signing_key.retire
```

`crates/sui-id-store/src/repos/clients.rs` has **no** `append_within_tx`
— measured, not assumed. So every one of these is the defect wave 2 had:
the write commits, the record may not, and nothing tells anyone.

What that means operationally: **a client created, updated, disabled,
soft-deleted or re-enabled, a client secret rotated, a registration
authorization revoked, or a signing key retired or deleted, can each
succeed with no audit record.** Secret rotation and signing-key retirement
are the two that would matter most in an incident reconstruction.

## C — wave 3, re-measured today, and it is not what the table says

The inventory calls C16–C18 `target-legacy` and C23 `target-absent`, which
reads as *"nothing live is wrong, these are future semantics"*. That is
not what the code shows.

**`federation.provider.created` is emitted nowhere.** `git grep` for it
across `crates/` returns no writer at all — the same class of defect as
wave 1's four settings writes, which the `0.84.0` stage plan also
described wrongly until it was measured.

The production write surface today is **`runtime/startup.rs` only**:

- `:323` — `federation_provider::create`, when a configured provider is
  absent from the database. Logs `tracing::info!`, audits nothing.
- `:385` — `federation_provider::update_allowed_origins`, from
  `reconcile_federation_provider`, when the configured allowed-origins
  list differs from the stored one. Logs old and new values at `info`,
  audits nothing.

`set_enabled` and `delete` have **no production caller at all** — tests
only, verified by grep excluding `tests`.

So wave 3's live defect is narrower and different from wave 4's: not a
best-effort append that can be lost, but **an origin allowlist that can
change on a config edit and a restart, leaving only a log line**. It is
real, and it is smaller than wave 4.

## D — the finding: wave 3 is in a dependency cycle, and that must be ruled on

Three artefacts, read today:

1. RFC 094 `:611-613` — *"`federation configuration` in M2b remains the
   provider administration commands C17/C18/C23 … and is RFC 096-B2's
   dependency."* So 096-B2 waits on wave 3.
2. `rfcs/handoffs/096-upstream-oidc-federation/README.md:119` — *"**096-B2**
   — requires RFC 094 M2b."* Same direction.
3. `contracts/write-commands.toml`, C17 and C18 — *"depends on RFC 096-B2
   (M2b). Target-state row."* And C23: *"Wholly new, RFC 096-B2 (M2b)."*
   The **opposite** direction.

**Wave 3 cannot be built from 096-B2's semantics while 096-B2 waits on
wave 3.** This is a real deadlock in the records, not a wording slip, and
it is why wave 3 has never been schedulable.

**My recommendation, for a ruling:** M2b's obligation on these rows is
**atomicity of the record for the behaviour that exists today**, not
delivery of 096-B2's target behaviour. Concretely — seal
`federation_provider::create` and `update_allowed_origins` onto the
Class-A seam now, emitting `federation.provider.created` and a
policy/origins-changed event in the same transaction; leave the activation
generation, the sealed preflight and the evidence invalidation to 096-B2,
which extends a sealed command rather than converting a bare one.

That breaks the cycle in the only direction that is coherent: RFC 094 owns
*whether the write and its record commit together*, RFC 096 owns *what the
write does*. It also makes M2b's *"no production Class-A best-effort
append anywhere"* literally satisfiable, which today it is not.

**RULED by `@nabbisen`, 2026-10-25: *"Your recommendation is authorized."*** So:

> **RFC 094 owns whether the write and its record commit together. RFC 096
> owns what the write does.**

The cycle is broken in that direction. **Wave 3 is now schedulable**: it
seals `federation_provider::create` and `update_allowed_origins` — the two
live production writes, both today in `runtime/startup.rs` and both
emitting nothing — onto the Class-A seam, with the events the inventory
already names. The activation generation, the sealed preflight and the
evidence invalidation stay with **096-B2**, which then extends a sealed
command rather than converting a bare one.

Three consequences, all recorded rather than left to be re-derived:

- **M2b's closure field is satisfiable as written.** *"No production
  Class-A best-effort append anywhere"* was unreachable while wave 3 was
  blocked; it is not now.
- **096-B2's prerequisite narrows.** It needs wave 3's *sealing*, not all
  of M2b. `rfcs/handoffs/096-upstream-oidc-federation/README.md:119` said
  "requires RFC 094 M2b" and now says which part.
- **The inventory's notes were the wrong half of the cycle.** C16–C18 and
  C23 said their work depends on 096-B2; what depends on 096-B2 is their
  *target semantics*, not their conversion. The notes say so now, and the
  `status` values are unchanged — they still describe target-state rows.

**Wave 3 is not in `0.85.0`.** `0.85.0` is wave 4, authorized and under
way with one open dispatch; wave 3 is the next cycle's, ahead of the
authority switch, which must still come last.

## E — the proposal

**`0.85.0` = wave 4, complete. C01–C10, C21, C22, K02, K03.**

Three reasons, in the order they actually weigh:

1. **It is the largest concentration of live risk left** — nine
   after-the-fact appends on client administration and signing keys,
   including secret rotation.
2. **It has no cross-RFC dependency.** Wave 3 is blocked on §D's ruling;
   the authority switch must come last by RFC 094 `:1101-1105`, since it
   exists to catch a newly introduced bare write and needs the boundary to
   exist first. Wave 4 is the only thing that can start on authorization
   alone.
3. **The shape is uniform and already proven.** All nine go through one
   helper, and `0.84.0` established the conversion pattern on nine
   commands of the same class. This is the same work with a known method,
   which is the argument for doing it in one cycle rather than two.

**Stage plan, proposed — three stages, same rhythm as `0.84.0`:**

- **Stage 1 — the client lifecycle.** C01, C05, C06, C21 (create, disable,
  soft-delete, enable). `clients.rs:113, 285, 321` and the enable path.
  Soft-delete carries token revocation, so it is the one with a compound
  transaction; it goes in the first stage deliberately, where the pattern
  gets its hardest test while attention is highest.
- **Stage 2 — the credential and authorization writes.** C07, C08, C22,
  K02, K03 — secret rotation, dev secret, registration-token revocation,
  signing-key retire and delete. The highest-value records in the set.
- **Stage 3 — the metadata writes.** C02, C03, C04, C09, C10 — basic
  metadata, scopes, post-logout URIs, consent policy, app identity.

**G20's declared command set** should move 34 → 48 across the three
stages. I will state the expected count per stage in each dispatch, as at
`0.84.0`, so a wrong count is a visible failure rather than a silent one.

## F — what `0.85.0` would not do

- **It does not close M2b.** Wave 3 and the authority switch remain, and
  the authority switch's negative fixture is itself a stage.
- **It does not close M3** — that is `rfcs/handoffs/095-dynamic-client-registration/m3-closure-route-2026-10-25.md`,
  and if §D1 of that note holds, M3 can close independently and sooner.
- **It does not unblock 096-B2** unless §D is ruled as recommended.

## G — what I need from you

1. **`0.85.0` = wave 4?** Or a different cut — in particular, if the
   origin-allowlist gap in §C reads as more urgent than it does to me, I
   will take the two startup writes first instead, as a small stage ahead
   of wave 4. That is a risk judgement, and it is yours.
2. **§D's ruling** — does wave 3 seal today's behaviour now, or wait for
   096-B2? Not blocking `0.85.0`; blocking everything after it.
3. **Nothing else.** The stage plan, the dispatches and the release cycle
   are mine once the scope is authorized.
