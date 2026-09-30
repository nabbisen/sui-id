# RFC 123 D4 — measured, before and after

**Method.** `authenticate_client` (`crates/sui-id-core/src/oidc/oauth_token.rs`)
called directly, in-process, against an in-memory SQLite database — no HTTP,
no socket. 300 timed samples per branch after a 5-iteration warm-up, release
profile. p10/median/max reported (not mean) so a few scheduler-jitter outliers
don't understate or overstate the gap.

## Before (the design review's numbers, reproduced for the record)

`authenticate_client` returned before hashing for every branch except a
known, confidential, enabled client.

| Branch | p10 | median | max |
|---|---|---|---|
| known confidential+enabled, wrong secret (hashes) | 34.01ms | 34.24ms | 40.62ms |
| unknown client id (no hash) | 81µs | 99µs | 165µs |
| disabled confidential client (no hash) | 36µs | 62µs | 108µs |
| public client (no hash) | 17µs | 42µs | 90µs |

Entire hashing-branch range sat 200-2000× above every non-hashing branch's
maximum — trivially distinguishable, even over a real network with jitter.

## After (this implementation, same method, plus a fifth branch)

Every branch that used to return early now runs a dummy `verify_password`
against a fixed decoy PHC hash first (`password::DUMMY_PHC` — the same decoy
`authn::session::login_with_mfa` already used for login; not a new idiom).

| Branch | p10 | median | max |
|---|---|---|---|
| known confidential+enabled, wrong secret (real hash) | 34.30ms | 34.86ms | 38.87ms |
| unknown client id (dummy hash) | 34.31ms | 34.84ms | 37.90ms |
| disabled confidential client (dummy hash) | 34.32ms | 34.84ms | 38.04ms |
| public client (dummy hash) | 34.28ms | 34.86ms | 38.38ms |
| malformed client id, not even a valid UUID (dummy hash) | 34.00ms | 34.45ms | 37.83ms |

All five branches' medians fall within 0.41ms of each other (34.45-34.86ms),
and every branch's full range overlaps every other branch's. The added fifth
branch (a malformed, non-UUID `client_id`) was not in the original
measurement — included here because it is also an early-return path in
`authenticate_client` and needed the same treatment; it did not need a
separate design-review question because it is the same "reject before
hashing" class D4 already covers.

**Conclusion: closed.** The gap that let a caller distinguish "known,
confidential, enabled" from everything else by timing is gone; every
rejection now costs the same wall-clock time as a real, failed verification.

## Method notes and limitations

- Measured in-process, not over a real socket, for the same reason the design
  review gave: a gap of this size (a full order of magnitude before the fix,
  now closed to sub-millisecond) does not need network-level measurement to
  be dispositive — ordinary jitter is smaller than what it would need to hide.
- This file's numbers come from a temporary test added to this branch's
  working tree for the duration of the measurement and removed afterward —
  it is not part of the shipped diff. The method is fully described above so
  the measurement can be reproduced from any commit that includes this
  RFC's changes to `authenticate_client`.
- Not re-measured against a live, loaded server; this closes the *timing
  side channel* D4 was about, not a claim about throughput or latency under
  production load, which [RFC 126](../../accepted/126-password-hashing-must-not-block-the-runtime.md)
  addresses separately (RFC 123 closes the volume-over-time half of the
  exposure; it does not close the concurrency half).
