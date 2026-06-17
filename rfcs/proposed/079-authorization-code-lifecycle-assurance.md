# RFC 079 — Authorization Code Lifecycle Assurance

**Status.** Proposed
**Tracks.** Strategy theme 2 (audit gap G4). Category A.
**Touches.** `sui-id-store/src/repos/auth_codes.rs`,
`sui-id-core/src/authorize.rs::exchange_code`, migration
`0031_auth_code_consumed_guard.sql` (comment-only / index, see
below), tests in both crates.

## Summary

Make single-use consumption of authorization codes enforced *by
the SQL statement itself*, independent of the connection model:
add the `AND consumed = 0 AND expires_at > now` predicate to the
consume UPDATE, check rows-affected `== 1`, and restructure the
exchange path as a smart-constructor pipeline
(`ValidatedCodeExchange`) so token issuance is unreachable
before client authentication, binding checks, and PKCE all pass.

## Motivation

Today `auth_codes::consume` is correct only because `Database`
serializes every closure behind one mutex: the transaction does
SELECT → Rust-side checks → unconditional `UPDATE … SET consumed
= 1`, never inspecting rows-affected. The moment the storage
layer gains real concurrency (connection pool; the SQL-backends
exploration in proposed RFC 009; even a second process on the
same WAL file), single-use enforcement silently evaporates —
the worst kind of regression, with no failing test. Strategy
§5.1 names "code consumption must be atomic at the storage
layer" as a core invariant; it should hold by construction, not
by an implicit property of `Mutex<Connection>`.

## Background

The current exchange order (client auth → consume → bindings →
PKCE → user-state re-check) is deliberate and good: a code is
burned even when a later check fails, so a partially-observed
code can never be retried. This RFC keeps that order and the
consume-first, fail-safe semantics exactly; it changes *how*
consumption is guaranteed and gives the post-consume validation
sequence a type-level shape.

## Target code areas

- `repos/auth_codes.rs::consume` — rewrite as a single guarded
  UPDATE + fetch:

  1. `UPDATE auth_codes SET consumed = 1
      WHERE code_hash = ?1 AND consumed = 0 AND expires_at > ?2`
  2. rows-affected `== 1` ⇒ SELECT the row and return it;
     `== 0` ⇒ `StoreError::NotFound` (same opaque outcome for
     unknown / expired / replayed, preserving non-disclosure).

  Both statements stay inside the existing transaction.
- `authorize.rs::exchange_code` — introduce the validated
  pipeline (in-crate, private):

  ```rust
  struct ConsumedCode(AuthorizationCodeRow);          // step 1
  struct BoundCode(AuthorizationCodeRow);             // step 2: client + redirect_uri verified
  struct PkceVerifiedCode(AuthorizationCodeRow);      // step 3
  struct IssuableGrant { row: AuthorizationCodeRow,
                         user: UserRow }              // step 4: user re-check
  ```

  Each step is a function consuming the previous state and
  returning `CoreResult<NextState>`; `issue_for` takes
  `IssuableGrant` only. This is the strategy's typestate
  recommendation applied at minimal scope: a linear pipeline of
  four states, no enum explosion, fully private to the module.

## Security properties / invariants

- **P1 (one-shot).** For a given `code_hash`, at most one
  `consume` call across the process lifetime (and across
  processes sharing the DB) returns `Ok`.
- **P2 (expiry).** A code with `expires_at <= now` can never be
  consumed, decided inside the same SQL statement that flips the
  flag (no TOCTOU between check and write).
- **P3 (ordering).** `issue_for` is not callable (does not
  type-check) without `PkceVerifiedCode` provenance; PKCE is not
  checkable without `BoundCode`; nothing is checkable without
  `ConsumedCode`. Token issuance before validation is a compile
  error, not a code-review finding.
- **P4 (burn on failure).** A consumed code whose later checks
  fail stays consumed (already true; pinned by test).
- **P5 (opacity).** Unknown, expired, and replayed codes are
  indistinguishable to the caller (`invalid_grant`, same wording).

## Non-goals

- No change to code TTL, generation, or hashing.
- No typestate exposure outside `authorize.rs` (states stay
  private; handlers see the same `exchange_code` signature).
- No detection/alerting on code replay in this RFC (an audit
  event for replay attempts can ride RFC 085 if wanted).

## Proposed design

The repo function becomes the *only* arbiter of one-shot-ness;
core no longer re-checks `row.consumed` (the row returned is by
definition freshly consumed). The Rust-side expiry check is
removed in favour of the SQL predicate; clock source is `Utc::now`
passed as a parameter for testability, mirroring `SharedClock`
usage in core.

Migration `0031` adds a covering index
`idx_auth_codes_expiry ON auth_codes(expires_at)` (purge speed,
optional) and a header comment documenting that single-use is
statement-enforced from this version. No schema shape change —
`code_hash` PRIMARY KEY + the guarded UPDATE are sufficient.

## Data model impact

None structural. One additive index.

## API impact

None at HTTP level. `repos::auth_codes::consume` signature gains
a `now: DateTime<Utc>` parameter; with RFC 078, `code_hash`
becomes `&CodeHash`.

## Testing strategy

- Repo test: two sequential `consume` calls — first `Ok`, second
  `NotFound` (P1 sequential form).
- Repo test: expired row — `NotFound`, and `consumed` remains 0
  in the DB (P2; proves the predicate, not Rust, rejected it).
- proptest: interleaving generator issues N codes and a shuffled
  multiset of consume attempts (including duplicates); property:
  per code, `Ok` count ≤ 1, and `Ok` only if attempted before
  expiry. (Full state-machine harness lands in RFC 083; this is
  a focused precursor.)
- Core test: pipeline order pinned by `compile_fail` doctest
  (calling `verify_pkce_step` with a `ConsumedCode`).
- Existing exchange-path tests stay green unchanged.

## Migration strategy

None for data. Behaviour-compatible for all conforming clients.

## Rollout plan

Ships with RFC 080 in one release (suggested v0.65.0) — both
restructure grant-side storage discipline and share test
infrastructure.

## Risks and mitigations

- *Risk:* UPDATE-then-SELECT changes the returned-row read point.
  Mitigated: both run in the same transaction; the row is
  immutable after consumption.
- *Risk:* typestate friction for future maintainers. Mitigated:
  four private structs, one file, documented inline; strategy
  §9's "keep typestate localized" is the explicit bound.

## Acceptance criteria

- `consume` enforces P1/P2 via statement predicate + rows-affected
  guard; no Rust-side `consumed`/expiry re-check remains.
- The pipeline types exist, are private, and `issue_for` requires
  the final state.
- New tests above pass; 0 warnings; baseline green.

## Open questions

- Whether to record a `auth.code.replay_denied` audit event on
  rows-affected = 0 with an existing row present. Deferred to
  RFC 085's completeness matrix to keep this RFC storage-focused.
