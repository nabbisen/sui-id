# RFC 080 — Refresh Token Rotation Atomicity and Reuse Detection Assurance

**Status.** Proposed
**Tracks.** Strategy theme 3 (audit gap G1 — highest-ranked
finding). Category A.
**Touches.** `sui-id-store/src/repos/refresh_tokens.rs`,
`sui-id-core/src/authorize.rs::exchange_refresh`, tests in both
crates, `docs/threat-model.md` (one paragraph).

## Summary

Collapse the refresh-grant's read-check-revoke-insert sequence
into one storage transaction with a rows-affected guard, so that
of any number of concurrent exchanges presenting the same token,
**exactly one** succeeds; every loser lands on the
theft-detection branch. Specify reuse-detection behaviour as an
explicit, documented outcome enum rather than emergent control
flow.

## Motivation

Audit gap G1: `exchange_refresh` spans three separate `with_conn`
closures (find → revoke → insert). Two concurrent requests with
the same active token can both observe `revoked_at IS NULL`, both
run the idempotent revoke, and both insert children — **two
active tokens from one parent in the same family**. This breaks
the strategy §5.2 invariants "a rotated refresh token must not
remain usable" (transitively: the rotation chain forks) and
"concurrent refresh requests must not produce inconsistent active
tokens". It also degrades theft detection: a forked family makes
the "exactly one live token per chain" mental model false, so an
operator auditing `auth.refresh.theft_detected` events can't
trust family state. Single-mutex serialization narrows but does
not close the window, and the window widens under any future
storage evolution (RFC 009).

## Background

Current flow (correct aspects to preserve): `find_any` returns
revoked rows so replay of a rotated token is *detectable*;
replay triggers `revoke_family` + audit, with an error
indistinguishable from ordinary failure; `purge_expired` retains
revoked-unexpired rows so detection works for the token's whole
nominal lifetime; rotation revokes-before-issue so a crash never
leaves both old and new valid. All of this is kept; only the
atomicity and the win/lose arbitration change.

## Target code areas

New repo function, replacing the find/revoke pair in the grant
path (`find_any` remains for introspection):

```rust
pub enum RotationLookup {
    /// Caller won the race: the row was active and is now revoked
    /// by this call. Proceed to issue the successor.
    RotatedHere(RefreshTokenRow),
    /// Row exists but was already revoked (by an earlier rotation
    /// or a concurrent winner): reuse signal. Caller must treat
    /// as theft: family revocation has ALREADY been performed
    /// inside this transaction; the family size revoked is given.
    ReuseDetected { row: RefreshTokenRow, family_revoked: usize },
    /// Row exists but is expired (and not revoked).
    Expired(RefreshTokenRow),
    /// No such token.
    Unknown,
}

pub async fn begin_rotation(
    db: &Database,
    token_hash: &RefreshTokenHash,   // RFC 078 type
    now: DateTime<Utc>,
) -> StoreResult<RotationLookup>
```

Implemented with `with_tx`:

1. SELECT row by `token_hash` (no filters) — absent ⇒ `Unknown`.
2. Expired and not revoked ⇒ `Expired`.
3. `UPDATE refresh_tokens SET revoked_at = ?now
    WHERE id = ?id AND revoked_at IS NULL` —
   rows-affected `1` ⇒ `RotatedHere`;
   rows-affected `0` ⇒ already revoked ⇒ run family revocation
   `UPDATE … WHERE family_id = ? AND revoked_at IS NULL` in the
   same transaction ⇒ `ReuseDetected`.
4. Commit.

A second function `insert_successor_within_tx` is *not* added:
the successor insert stays a separate statement, because losing a
crash between revoke-commit and insert only costs the user a
re-login (fail-safe), exactly as today. What must be atomic is
the *arbitration*, and step 3's guard provides it. (Folding the
insert into the same transaction is a one-line follow-up if soak
shows crash-window friction; noted as open question.)

Core (`exchange_refresh`) becomes a match on `RotationLookup`:

- `RotatedHere` → client-binding check **before** anything else
  remains in place at the SELECT result; issue successor with the
  same `family_id`.
- `ReuseDetected` → append `auth.refresh.theft_detected` audit
  (with family id and `family_revoked` count) and return
  `invalid_grant` with the standard opaque description.
- `Expired` / `Unknown` → opaque `invalid_grant`.

Client-binding subtlety: the binding check (`row.client_id ==
req.client_id`) must run on the SELECTed row *before* step 3's
revoke takes effect from the caller's perspective. To keep the
storage function policy-free, `begin_rotation` gains a
`expected_client: ClientId` parameter; a mismatch short-circuits
to a `WrongClient` variant *without* revoking (matching today's
behaviour where mismatch is rejected before any state change).

## Security properties / invariants

- **P1 (single winner).** For one token value and any concurrent
  set of exchanges, exactly one observes `RotatedHere`.
- **P2 (loser ⇒ theft path).** Every concurrent loser observes
  `ReuseDetected`, and family revocation has occurred before
  their response is computed.
- **P3 (no fork).** At all times, per family, the set of
  non-revoked rows has size ≤ 1 (the newest issued successor).
- **P4 (irreversibility).** No code path sets `revoked_at` back
  to NULL (greppable invariant; pinned by test).
- **P5 (opacity).** `RotatedHere` failure modes and
  `ReuseDetected`/`Expired`/`Unknown` all surface as
  `invalid_grant` with non-distinguishing wording and
  timing-equivalent handling.
- **P6 (monotone clock discipline).** Expiry comparisons use the
  caller-passed `now` so tests and `SharedClock` agree.

## Non-goals

- No change to token format, TTLs, family-id scheme, or the
  decrypt-scan backfill fallback (it remains for `find_any`
  introspection until the NOT NULL follow-up migration).
- No sequence/version column: SQLite's serialized writes plus the
  rows-affected guard already provide the ordering the strategy's
  "token sequence must not move backward" invariant asks for.
- No TLA+ model here (candidate in RFC 086).

## Proposed design

(See target code areas — design and target are one in this RFC.)
`RotationLookup` is exported from the store crate; it is the
explicit specification of reuse-detection behaviour the strategy
asks for ("reuse detection behaviour is explicitly specified and
tested").

## Data model impact

None. Existing columns and the partial UNIQUE `token_hash` index
suffice.

## API impact

None at HTTP level. Store API: `begin_rotation` added;
`find_active` retained for introspection; the grant path stops
calling `find_any`/`revoke` directly.

## Testing strategy

- Repo test (sequential P1): two `begin_rotation` calls, same
  hash — first `RotatedHere`, second `ReuseDetected` with
  family revoked.
- Repo test (concurrent P1/P2): spawn N tasks calling
  `begin_rotation` on one token over the real `Database`;
  assert exactly one `RotatedHere`.
- Repo test (P3): after a chain of rotations with injected
  concurrent replays, assert per-family active-count ≤ 1 by
  direct SQL.
- Core test: `ReuseDetected` emits exactly one
  `auth.refresh.theft_detected` audit row per detection.
- proptest precursor: random interleavings of
  {rotate, replay-old, expire-clock} against an oracle model
  asserting P1–P3 (full harness in RFC 083).
- Threat-model doc gains a paragraph describing the concurrency
  guarantee.

## Migration strategy

None for data; behaviour change is invisible to conforming
clients. Non-conforming clients that fire parallel refreshes
will now deterministically lose all-but-one and re-authenticate
— which is the OAuth 2.1 §6.1-intended outcome.

## Rollout plan

Ships with RFC 079 (suggested v0.65.0). Verification-phase soak
should specifically watch `auth.refresh.theft_detected`
frequency for false-positive spikes from legitimate clients with
retry storms; the audit note's family size aids triage.

## Risks and mitigations

- *Risk:* legitimate clients with aggressive retry logic see
  family revocations. Mitigation: this is spec-intended; soak
  monitoring; the error remains opaque and recovery is
  re-authentication.
- *Risk:* holding the revoke+family-revoke in one tx lengthens
  the critical section. Mitigation: both are single indexed
  UPDATEs; negligible under the current mutex model.

## Acceptance criteria

- `exchange_refresh` contains no read-then-write across closure
  boundaries; arbitration is rows-affected-guarded in one
  transaction.
- `RotationLookup` exists, is documented, and all four/five
  variants are covered by tests.
- Concurrent test (N ≥ 8 tasks) passes deterministically; P3
  SQL assertion holds; baseline green; 0 warnings.

## Open questions

- Fold the successor insert into the same transaction? Default
  no (crash window is fail-safe); revisit after soak.
- Should `Expired` presented-token also be treated as a reuse
  signal when the row is *revoked and* expired? Current design
  says revoked wins (checked first); confirm in review.
