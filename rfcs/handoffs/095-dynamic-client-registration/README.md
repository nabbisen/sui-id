# RFC 095 developer handoff

**Governing RFC:** [RFC 095](../../accepted/095-dynamic-client-registration-transaction.md)
**Audience:** the mid-capability model only after RFC re-acceptance and prerequisite evidence
**Stages 1 and 1b have both landed** — stage 1 `78a2dec`, stage 1b `99f83bb`
([`stage1b-loopback-port-matching-2026-10-05.md`](stage1b-loopback-port-matching-2026-10-05.md),
the one redirect-corpus row stage 1 deferred). *This line said 1b "is open"
until 2026-10-20; it had landed. The architect maintains this file and did
not update it when the round closed.*

**The remaining stages are planned**:
[`m3-stage-plan-2026-10-20.md`](m3-stage-plan-2026-10-20.md) — five stages,
not the two an older label named, because the remainder spans four file sets
and four kinds of verification. **Stage 2 landed** `7cc1488` (Level B, 27 jobs, 0 skipped) —
[`stage2-the-field-rules-2026-10-20.md`](stage2-the-field-rules-2026-10-20.md).
**Stage 3 landed** `2bdc6af` (Level B, 27 jobs, 0 skipped) —
[`stage3-the-registration-response-2026-10-21.md`](stage3-the-registration-response-2026-10-21.md).
**Stage 4's design is written and awaiting one ruling that is not the
architect's to make, not dispatched**:
[`stage4-design-the-authentication-grant-matrix-2026-10-21.md`](stage4-design-the-authentication-grant-matrix-2026-10-21.md).
**Stage 5 landed** `a8583e4` (Level B, 27 jobs, 0 skipped) —
[`stage5-browser-presentation-2026-10-21.md`](stage5-browser-presentation-2026-10-21.md)
— taken ahead of stage 4 because it depends on nothing stage 4 settles, and
idling the implementation role behind a ruling it does not need would cost
the cycle nothing but time. The order is now 2 → 3 → **5** → 4 → 6.

> **`0.83.0` is authorized and its preparation is dispatched** —
> [`../096-upstream-oidc-federation/release-0.83.0-preparation-2026-10-22.md`](../096-upstream-oidc-federation/release-0.83.0-preparation-2026-10-22.md).
> Every stage has landed: 2, 3, 4a, 4b, 5 and 6, each Level B 27/0. The
> mechanical half of the cut is the implementation role's; the tag and the
> publish are not. **M3 does not close with this release** — the
> legacy-candidate disposition and the evidence sign-off both remain
> `@nabbisen`'s.

> **Whose turn: the implementation role's.** Stage 4's measurement stands
> and its three decisions are settled. C3 is `@nabbisen`'s; C1 and C2 are
> the architect's, at his request, under `ROADMAP.md` §S1c. The design's
> §D is the record of which was whose. **Stage 4 is split in two. 4a landed** `d99b14a` (Level B, 27 jobs, 0
> skipped) —
> [`stage4a-persist-the-three-fields-2026-10-22.md`](stage4a-persist-the-three-fields-2026-10-22.md):
> migration `0049`, three nullable columns with no default, and the
> unrecorded-method marker. **No behaviour change at `/oauth2/token`.**
> **4b is dispatched and is the last unbuilt piece** —
> [`stage4b-enforce-the-matrix-2026-10-22.md`](stage4b-enforce-the-matrix-2026-10-22.md):
> the typed credential presentation, the channel check, RFC 123 D4
> compliance for `authorize::authenticate_client`, both-channels-present
> and the refresh half. `0.83.0` cuts when it lands; the scope was not
> re-cut around stage 4.

**Stage 6 landed** `4549c3b` (Level B, 27 jobs, 0 skipped) —
[`stage6-the-legacy-candidate-report-2026-10-21.md`](stage6-the-legacy-candidate-report-2026-10-21.md)
— the read-only legacy-candidate report, `sui-id admin
legacy-dynamic-candidates`. **Stage 4 is the only stage of `0.83.0` not
built**, and it is undispatched pending the self-review ruling.

**Found by stage 6's round; the repair was proposed and then withdrawn.**
C15's dynamic path calls `clients::create_within_tx` and then
`clients::set_registered_via_within_tx` (`commands.rs:4102-4103`), which
RFC 095 `:364-366` forbids by name — *"Separate `set_registered_via` or
`update_app_identity` writes are forbidden in the dynamic path."*

**The code is right and the RFC sentence is not.** Measured 2026-10-21:
`contracts/write-commands.toml` declares that two-call shape as C15's
mutation surface and G17 enforces it; C11 (`set_registered_via`) is class
`I` there — an internal mutation primitive, `sealed = false`, no audit
event — which `command-inventory.md` defines as an explicit exclusion from
Class A; and `create_leaves_registered_via_at_the_table_default` exists
specifically to fail the edit that was proposed. Atomicity is held by the
transaction either way, so nothing is at risk.

**What is actually needed is an erratum on RFC 095 `:362-366`.** That one
belongs to `@nabbisen`.

Amending an Accepted RFC's text is not the architect's to settle, and under
`ROADMAP.md` §S1a neither is reviewing such a change. Three
doc comments that described C11 as "separately-audited" are corrected;
nothing else in the code moves.

**Measured while writing that design, and it reshapes the stage:**
`token_endpoint_auth_method` is accepted at registration, validated by stage
2 and echoed by stage 3, but **never persisted** — `ClientRow` has no such
field and no migration adds the column. **Four** of the matrix's eight rows do not hold
today, two of which need a schema change. RFC 095 `:362-366` states the C15
insert writes the auth method, grants and response types; it writes none of
the three.

*Corrected 2026-10-21: an earlier version of this entry said three rows and
one field, and the design behind it reasoned about the wrong function —
`/oauth2/token`'s grant dispatch calls a private `authenticate_client` in
`authorize.rs`, not the public one serving introspection and revocation. It
also dispatched an "independent design review" to the implementation role,
which `ROADMAP.md` §S1a places with `@nabbisen` or an external architect, not with it. Both are recorded in the
design's own header.*

**Found while dispatching stage 5 and fixed by it:** the consent screen
emitted an `<img src>` for *any* client's `logo_uri` with no `registered_via`
check, so RFC 095 `:269`'s *"must not emit an image … for a dynamically
registered `logo_uri`"* did not hold at the emit level. No request was ever
made — the global CSP's `img-src 'self' data:` blocks it, now observed on the
consent response itself rather than only read from the middleware — so the
rule was met by a header the emitting code knew nothing about, and is now met
by construction.

**The same CSP also blocks an admin-registered client's logo**, which means
RFC 008's logo feature has never worked in a conforming browser, under a
caption asserting a logo was provided. Stage 5 left that exactly as it found
it. It is `@nabbisen`'s to dispose of, not the architect's and not the
implementation role's.

**`scope` is required at registration** as of stage 2 — a deliberate
departure from RFC 7591 §2's optionality, recorded in
[`metadata-validation.md`](metadata-validation.md) with its reason. The
integrator-facing statement of it is stage 3's.
`0.83.0` is M3's remaining implementation; closing M3 additionally needs the
legacy-candidate disposition and the evidence acceptance, **neither of which
is the architect's to give**.

Stage 1 was dispatched as —
[`stage1-envelope-and-redirect-profile-2026-10-05.md`](stage1-envelope-and-redirect-profile-2026-10-05.md):
the request envelope and the derived closed redirect profile. *That dispatch
said "stages 2 and 3"; the plan above supersedes that numbering with five.* The entry gate below is **open** — all six conditions met,
condition 4 discharged and the discharge approved 2026-10-05.

**Status:** Planning companion; inherits the governing RFC's current status — **Accepted 2026-08-27**, the 2026-07-28 return to `proposed/` for a material prerequisite amendment having closed. Implementation remains blocked on the entry gate below.

This handoff decomposes RFC 095 without authorizing coding or weakening RFC
094's C15 Class-A boundary.

## Companion files

- [architecture.md](architecture.md) — target components, schema compatibility,
  transaction flow, runtime enforcement, and ownership boundaries.
- [metadata-validation.md](metadata-validation.md) — exact supported metadata,
  limits, normalization, URI corpus, and error mapping.
- [verification.md](verification.md) — migration, rollback, contention,
  response, retry, and secret/log evidence.

## Entry gate

Implementation may start only when all are true:

- RFC 095 is under `rfcs/accepted/` with complete approval metadata;
- **RFC 094 M2a** is Implemented on the recorded clean baseline — **not** the
  whole of RFC 094 — and C15's guarded consume / client-create / typed-event
  transaction has observed rollback and exactly-once evidence there.
  C15 is pinned to M2a and may not be deferred to M2b (RFC 094 §M2a), so M2b's
  remaining conversion waves are **not** a prerequisite for this RFC.
  *Corrected 2026-08-12 after independent review finding B-095-1: this gate said
  "RFC 094 is Implemented", which contradicted the RFC's amended prerequisite and
  would have made this work wait for M2b unnecessarily.*
- RFC 093's current clean-tree matrix passes;
- *(withdrawn 2026-10-05 — this condition is discharged, see below);*
- the metadata/URI/adversarial design has durable independent approval; and
- the frozen logout parser/confirmation constants and atomic consumption
  contract in [architecture.md](architecture.md) remain unchanged; and
- no competing migration claims the next schema number.

> **Condition 4 is fully discharged, 2026-10-05.** It originally read: *"the
> owner records `codex-developer`, the full clean baseline commit, and
> non-overlapping ownership of the files below."* All three parts are withdrawn.
>
> **`codex-developer`** was a name the former architect invented;
> `@nabbisen` ruled on 2026-10-05, *"No `codex-developer` now or never"*, and
> this team's implementation role is the mid-capability model
> (`.git-exclude/roles/`). Recording it per-RFC records nothing.
>
> **Non-overlapping ownership** dates from the two-lane period when a second
> implementer was assigned; with one implementation role the collision it
> guarded cannot occur.
>
> **The baseline commit** was kept on 2026-10-05 and is now withdrawn too. It
> existed to serve [verification.md](verification.md)'s rule that *"results from
> different commits cannot be assembled into a passing closure package"* — which
> is correct, and is the same rule **RFC 131 D2** states as *"a claim about the
> shipped system requires Level B on the exact commit it cites"*. **But that
> binds at closure, not at start.** RFC 131 D2 enforces it mechanically on the
> commit the closure evidence names, and a baseline recorded before coding
> cannot contribute to it, because the commit that matters is the final one.
>
> **All six conditions are met and the entry gate needs no owner action.**
> **Approved by `@nabbisen`, 2026-10-05: "OK. Approved."** The entry gate is
> open; RFC 095 implementation may be dispatched.

## File ownership

The RFC 095 developer owns only the accepted implementation scope:

- dynamic registration HTTP parsing/error/response code;
- client registration-token authorization and C15 within-transaction helpers;
- client model/repository fields needed for registered metadata;
- the next migration and its upgrade tests;
- token auth/grant enforcement, typed redirect profiles, verified logout
  context, GET/POST logout confirmation/CSRF flow, and generation-checked CORS
  helpers;
- dynamic-client consent presentation hardening;
- RFC 095-specific documentation and tests; and
- necessary typed C15 descriptor/fixture updates under RFC 094 invariants.

Broad OIDC refactoring, unrelated client administration changes, RFC 094
capability redesign, RFC 096 federation work, and global URL-policy changes are
out of scope.

## Ordered delivery

1. Migration and backward-compatible readers.
2. Pure validators and complete negative corpus.
3. Runtime auth/grant/redirect/logout enforcement.
4. Prepared registration and C15 atomic write path.
5. Response/error hardening, concurrency/fault evidence, documentation.

Each stage must compile and preserve legacy behavior. New dynamic metadata must
not be writable until the corresponding runtime enforcement is active.

## Stop conditions

Stop and return to architecture/security review if:

- RFC 094's C15 context cannot represent a registration-token principal;
- a supported field cannot be persisted and returned exactly;
- secret plaintext must be durably stored for retry;
- loopback matching would loosen any component other than port;
- loopback HTTP would be allowed outside the PKCE public-native profile, or
  logout would inherit the authorization port relaxation;
- a new dynamic post-logout URI is not HTTPS, or logout URI validation depends
  on the authorization redirect profile;
- a dynamic logout redirect can proceed without a verified same-client
  ID-token hint and applicable session binding;
- a missing/invalid/mismatched-hint GET or initial POST can mutate the session,
  or local confirmation lacks a subsequent explicit CSRF-protected POST;
- logout parser/confirmation constants, raw-token exposure, expiry, maximum
  outstanding count, cleanup, or atomic duplicate-consumption behavior would
  differ from the frozen architecture contract;
- a stale CORS snapshot can authorize after disable, delete, or redirect
  mutation;
- the CORS generation can be coerced, defaulted, saturated, wrapped, or changed
  without a guarded checked increment in the client transaction;
- dynamic consent would automatically fetch a registrant-controlled logo;
- token ID and possession digest can be independently constructed/substituted,
  or token expiry is decided only before transaction acquisition;
- any positively stamped legacy dynamic row that does not derive exactly one
  valid redirect profile is assigned a profile or allowed at runtime;
- migration cannot distinguish pre-RFC-095 dynamic compatibility rows;
- implementation proposes guessing that an `admin` row was historically
  dynamic without positive durable audit evidence;
- any failure can commit token use without one client and one event;
- a new metadata field, redirect scheme, grant, response type, or auth method is
  proposed; or
- file ownership overlaps active RFC 093/094/096 implementation.

## Handoff completion

Return one clean commit identity, exact commands/results, migration fixtures,
validation cases, transaction fault/cancellation matrix, contention
reconciliation, possession/type-isolation evidence, CORS representation/
overflow/revocation evidence, GET/POST logout confirmation/CSRF and
browser-network evidence, legacy-unstamped candidate report and owner
disposition, sanitized log/metric samples, response headers, remaining risks,
and an independent closure-review request. A checklist without observed output
is not evidence.

## Review records

Relocated here on 2026-09-10 when `rfcs/reviews/` was retired — RFC 000
sanctions four lifecycle folders plus the optional `draft/` and this
non-lifecycle `handoffs/` companion folder, and per-RFC review records are
companion documents, not a sixth lifecycle.

| Record | What it is |
|---|---|
| [`095-design-review-2026-07-18.md`](095-design-review-2026-07-18.md) | Original independent design review, cited in RFC 095's lifecycle history |

RFC 095's current **Independent design review** and A6 sweep evidence are
the shared three-RFC records, held under
[`../094-transactional-audit/`](../094-transactional-audit/README.md#review-records).
