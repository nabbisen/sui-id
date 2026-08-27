# RFC 095 developer handoff

**Governing RFC:** [RFC 095](../../accepted/095-dynamic-client-registration-transaction.md)
**Audience:** `codex-developer` only after RFC re-acceptance and prerequisite evidence
**Status:** Planning companion; inherits the governing RFC's current status — returned to `proposed/` on 2026-07-28 for a material prerequisite amendment and pending fresh independent design review and re-acceptance — with implementation still blocked on the entry gate below

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
- the owner records `codex-developer`, the full clean baseline commit, and
  non-overlapping ownership of the files below;
- the metadata/URI/adversarial design has durable independent approval; and
- the frozen logout parser/confirmation constants and atomic consumption
  contract in [architecture.md](architecture.md) remain unchanged; and
- no competing migration claims the next schema number.

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
