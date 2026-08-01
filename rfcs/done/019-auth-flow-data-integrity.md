# RFC 019 — Auth flow data integrity hardening

**Status.** Implemented
**Priority.** High. Three concrete defects in the token-issuance
path: a window where a disabled user can still receive tokens,
a refresh-token lookup that performs a full-table decrypt scan,
and a GC policy that contradicts the refresh-token theft-detection
design that shipped in migration `0008`.
**Tracks.** v0.29.5 data-model review — high-priority findings
#1, #2, #3, #4. Single-realm scope (RFC 022) is unaffected by
this work.
**Touches.** `crates/sui-id-store/src/migrations/` (one new
SQL file with three logical sections; details below),
`crates/sui-id-store/src/repos/auth_codes.rs`,
`crates/sui-id-store/src/repos/refresh_tokens.rs`,
`crates/sui-id-core/src/authorize.rs` (`exchange_code`,
refresh rotation), `crates/sui-id-core/src/admin.rs` (user
disable/delete invalidates outstanding auth codes).

## Summary

The data-model review identifies a cluster of defects that all
sit on the OIDC token-issuance path:

1. **`auth_codes` has no FK to `users` or `clients`**, and
   `exchange_code()` does not re-check user state at exchange
   time. A user disabled or soft-deleted in the 60-second
   window between authorization and exchange still receives an
   access token, ID token, and refresh token. Sessions and
   refresh tokens are revoked on disable; auth codes are not.
2. **`refresh_tokens` has no `token_hash` column.** Lookup is
   "decrypt every candidate row, constant-time compare" —
   `O(n_active_refresh_tokens)` per `/oauth2/token` call.
3. **The refresh-token GC contradicts the theft-detection
   design.** Migration `0008` added `family_id` so that a
   replayed (already-rotated) refresh token reveals theft. The
   current GC deletes any row with `revoked_at IS NOT NULL`,
   which means a replayed token can be GC'd before it gets
   replayed, returning a benign `NotFound` instead of firing
   `theft_detected`.

Each defect is small in isolation. They are bundled in one RFC
because they share a single migration plus a tightly coupled
set of repo-function changes. Splitting them would force the
implementer to write three migrations against `auth_codes` /
`refresh_tokens`, three test fixtures, and three audit-event
emissions for what is logically one piece of work: closing the
gaps in token-issuance integrity.

## Why high priority

These are real, exploitable correctness gaps, not theoretical
ones:

- An admin who disables a compromised account does not actually
  cut off all in-flight authorizations. The window is 60s wide
  but the effect is real: a token issued in that window outlives
  the disable.
- The full-decrypt-scan is acceptable at 50 active refresh
  tokens. At 5,000 it is a 100ms `/oauth2/token` p99 floor with
  no observable cause. sui-id has no benchmarks (RFC 014) so
  this would not surface until a deployment hit the wall.
- The GC behaviour means that the *most security-relevant*
  scenario the family_id mechanism was built for — a stolen
  refresh token replayed days after its rotation — is the
  scenario the GC silently disarms.

These compound: a disabled user whose session is preserved
through the auth-code window, whose refresh token then survives
because nothing detects its replay, gives an attacker meaningful
post-revocation persistence.

## Requirements

After this RFC ships:

1. `exchange_code()` rejects token issuance when the bound user
   is disabled or soft-deleted at exchange time, with the same
   `invalid_grant` semantics it uses for already-consumed codes.
2. `auth_codes` carries `ON DELETE CASCADE` foreign keys to
   `users(id)` and `clients(id)`. Hard-deletion of either
   clears outstanding codes automatically.
3. Soft-disable and soft-delete of a user invalidate that
   user's outstanding auth codes synchronously, the same way
   they invalidate sessions and refresh tokens today.
4. `refresh_tokens` carries an indexed `token_hash` column.
   Lookup by token plaintext is `O(log n)` via index, not a
   full-table decrypt scan.
5. The refresh-token GC retains revoked rows until their
   original `expires_at` passes. Replays of recently-rotated
   tokens reach the theft-detection branch.
6. Existing tests pass without behaviour changes for the cases
   they covered. New tests cover each of the three integrity
   gaps.

## Design

### § 1. Auth-code user-state recheck

`exchange_code()` already calls `clients::get(client_id)` and
rejects on a disabled / deleted client. The symmetric check on
the bound user does not exist. Add it directly after the
existing client check, before any token issuance:

```rust
// crates/sui-id-core/src/authorize.rs, in exchange_code()
let user = sui_id_store::repos::users::get(db, &row.user_id)
    .map_err(map_user_lookup_err)?;
if user.is_disabled || user.is_deleted {
    sui_id_store::repos::auth_codes::mark_consumed(db, &code_hash)?;
    audit_emit(db, "oauth2.exchange_code.user_revoked", ...);
    return Err(AuthorizeError::InvalidGrant);
}
```

The auth code is marked consumed even on rejection so that
re-presenting it after the user is re-enabled is also blocked
(consistent with how a disabled-then-re-enabled user must
re-authenticate to obtain a fresh code anyway).

The audit event uses a distinct kind (`user_revoked`) from the
existing `client_revoked` and `code_consumed` paths so that
forensics can distinguish "user was disabled mid-flow" from
the other invalid_grant causes.

### § 2. `auth_codes` foreign keys

The current schema (migration `0001`):

```sql
CREATE TABLE auth_codes (
    code_hash TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    ...
);
```

SQLite cannot add FKs to an existing table; the table must be
rebuilt. Rebuild is safe because auth codes have a 60-second
TTL — any rows currently in the table at migration time are
either expired (will be cleaned by the next GC pass) or about
to be exchanged or fail. The rebuild discards them all,
identical to a server restart taking longer than 60 seconds.

```sql
-- migration 0019, § 1
PRAGMA foreign_keys = OFF;
DROP TABLE auth_codes;
CREATE TABLE auth_codes (
    code_hash TEXT PRIMARY KEY,
    client_id TEXT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    user_id   TEXT NOT NULL REFERENCES users(id)   ON DELETE CASCADE,
    redirect_uri      TEXT NOT NULL,
    code_challenge    TEXT NOT NULL,
    code_challenge_method TEXT NOT NULL,
    scope             TEXT NOT NULL,
    nonce             TEXT,
    issued_at         TEXT NOT NULL,
    expires_at        TEXT NOT NULL,
    consumed          INTEGER NOT NULL DEFAULT 0
        CHECK (consumed IN (0, 1))
);
PRAGMA foreign_keys = ON;
```

The DROP-then-CREATE pattern is the only SQLite-portable shape
that picks up FK constraints on existing tables. The
`PRAGMA foreign_keys` toggle is required because the rebuild
itself transiently violates the FK self-consistency rule SQLite
checks at end-of-statement.

Note that `consumed`'s `CHECK (consumed IN (0, 1))` is
included here as a side benefit — it costs nothing while the
table is being rebuilt anyway. The same pattern for other
tables is in scope for RFC 021, not this one.

### § 3. Auth-code invalidation on user disable / delete

FK cascade fires only on hard-delete, but sui-id soft-deletes
users. Add explicit invalidation in `admin::disable_user()`
and `admin::soft_delete_user()`, alongside the existing
`sessions::revoke_all_for_user()` and
`refresh_tokens::revoke_all_for_user()` calls:

```rust
// crates/sui-id-core/src/admin.rs
sui_id_store::repos::sessions::revoke_all_for_user(db, &user_id)?;
sui_id_store::repos::refresh_tokens::revoke_all_for_user(db, &user_id)?;
sui_id_store::repos::auth_codes::invalidate_all_for_user(db, &user_id)?;  // new
```

`auth_codes::invalidate_all_for_user(user_id)` issues a single
SQL `UPDATE auth_codes SET consumed = 1 WHERE user_id = ?1
AND consumed = 0`. The FK cascade still applies on a future
hard-delete; this adds the soft-delete coverage.

The same call site applies to admin password reset (currently
revokes sessions and refresh tokens). Whether mid-flow auth
codes survive a password reset is a judgement call: this RFC
treats password reset the same as disable for consistency.

### § 4. `refresh_tokens.token_hash`

The current row carries `token_enc` (sealed plaintext) plus
metadata. Add `token_hash` as an indexed lookup key:

```sql
-- migration 0019, § 2
ALTER TABLE refresh_tokens
    ADD COLUMN token_hash BLOB;

CREATE UNIQUE INDEX idx_refresh_tokens_token_hash
    ON refresh_tokens(token_hash)
    WHERE token_hash IS NOT NULL;
```

The index is a *partial* unique index because of the backfill
window: existing rows have `NULL` until the worker described
below fills them. Once backfill completes, the column becomes
effectively NOT NULL by application invariant; the partial
predicate is retained because retrofitting `NOT NULL` would
require another rebuild for limited benefit.

**Hash function.** SHA-256(token) with no pepper. Refresh
tokens are 32-byte CSPRNG outputs, so the input has full
entropy; salted Argon2 is unnecessary and would defeat the
performance goal. A server-side HMAC pepper would give defence
against an attacker who reads the database file *but does not
have the master key*, which is not a realistic threat in
sui-id's model — the master key seals the credentials column,
HIBP secret, SMTP password, etc., and a database file without
the master key is already inert.

**Issue path.** `refresh_tokens::insert(...)` now writes both
`token_enc` and `token_hash`. The hash is computed from the
same plaintext used to seal `token_enc`, so the columns are
written atomically.

**Lookup path.** `refresh_tokens::find_active(plaintext)`
becomes:

```rust
let hash = sha256(plaintext);
let row = conn.query_row(
    "SELECT ... FROM refresh_tokens
     WHERE token_hash = ?1 AND revoked_at IS NULL
     AND expires_at > ?2",
    (hash, now),
    map,
)?;
// optional: verify token_enc decrypts and matches plaintext as a defence
// in depth against hash collisions (will never trip in practice).
```

The optional belt-and-braces verify-by-decrypt is left in for
defence in depth at zero cost on the happy path (one row, one
decrypt). Removing it would also be defensible.

The `find_any(plaintext)` variant used by the theft-detection
branch becomes the same shape but without the
`revoked_at IS NULL` predicate.

### § 5. Refresh-token GC vs theft detection

Today's GC:

```sql
DELETE FROM refresh_tokens
WHERE expires_at < ?1
   OR revoked_at IS NOT NULL;
```

This is wrong. A token revoked at 09:00:00 with `expires_at =
09:30:00` should remain in the table until 09:30:00 — that is
the window in which a replay would surface theft.

Replace with:

```sql
DELETE FROM refresh_tokens
WHERE expires_at < ?1;
```

Revoked rows live until their original `expires_at`. The
theft-detection branch in
`crates/sui-id-core/src/authorize.rs` finds replays through
`find_any()` and detects them by `revoked_at IS NOT NULL`, as
already implemented.

There is a secondary case worth guarding against: a revoked
token whose `expires_at` has passed should not silently
disappear when its replay would have been detectable. This is
an unavoidable trade-off — the alternative is keeping revoked
rows forever, which is a slow leak. The chosen behaviour
(retain until original expiry) gives at least the original
token's lifetime of detection coverage, which matches the
original token's authorisation horizon and is the natural
boundary.

### § 6. Backfill

Existing deployments have rows in `refresh_tokens` with
`token_hash IS NULL`. Two backfill strategies:

- **Online (chosen).** On startup, after migrations apply, a
  one-shot backfill task iterates rows with `token_hash IS NULL`,
  decrypts `token_enc`, computes hash, writes hash. Runs in
  the background via `tokio::spawn`. Lookup falls back to the
  legacy decrypt-scan for any row that hasn't been backfilled
  yet (so the system is correct throughout).
- **Offline.** A `sui-id admin migrate-refresh-token-hashes`
  CLI subcommand. Operator runs it after upgrading. Simpler
  but adds an upgrade step.

The online strategy is preferred. The fallback path is small
(the existing decrypt-scan is preserved for one release) and
disappears as soon as the backfill task is complete. After the
backfill is done, the next release can drop the fallback and
treat `token_hash IS NULL` as a bug.

The fallback removal is **not** in scope for this RFC. A
follow-up tagged "v0.30.x" or whatever release is one cycle
beyond this one removes the fallback path. This RFC's design
explicitly carries a deprecation pointer at the fallback.

### § 7. Migration file

The three SQL changes (auth_codes rebuild, refresh_tokens
column + index, no SQL change for the GC fix — that's
code-only) are bundled in one migration file:

```
crates/sui-id-store/src/migrations/0019_auth_flow_integrity.sql
```

Migration `0019` is the next unused number after `0018_session_
limits.sql`. RFC 020 and RFC 021 will use `0020`, `0021`. If
the implementation order shifts, the numbers shift to match.

The file is divided into two `-- §` comments matching the
sections above. SQLite executes the statements in order; the
auth_codes rebuild needs `PRAGMA foreign_keys = OFF` first and
`= ON` after. Per RFC 021's requirement that migrations run
in transactions, the migration runner wraps the whole file in
a transaction; the `PRAGMA foreign_keys` pragmas operate
within that transaction.

## Tests

The three defects each get an e2e regression test, plus unit
tests on the smaller building blocks.

1. **Auth-code user-state recheck.**
   `crates/sui-id/tests/e2e/exchange_code_user_revoked.rs`:
   - Authorize, get a code.
   - Disable the user before exchange.
   - Exchange call returns `invalid_grant`.
   - Auth-code row is `consumed = 1`.
   - Audit log contains `oauth2.exchange_code.user_revoked`.

2. **Auth-code FK cascade.**
   `crates/sui-id-store/src/repos/auth_codes.rs` unit test:
   - Insert auth code linked to a user.
   - Hard-delete user via `users::hard_delete()` (a CLI-only
     path).
   - Verify auth_codes row disappears.

3. **Auth-code invalidation on disable.**
   Unit test on `admin::disable_user()`:
   - Insert auth codes, sessions, refresh tokens for user.
   - Call `disable_user()`.
   - All three are revoked / consumed.

4. **Refresh-token hash lookup.**
   `crates/sui-id-store/src/repos/refresh_tokens.rs` unit
   test:
   - Insert N refresh tokens with `token_hash` populated.
   - Look up one by plaintext.
   - Returns the right row in O(log n) — explicit benchmark
     deferred to RFC 014's harness.

5. **Refresh-token theft detection survives GC.**
   `crates/sui-id/tests/e2e/refresh_theft_after_gc.rs`:
   - Issue refresh token A, rotate to B (revoking A).
   - Run GC.
   - Verify A's row still exists in DB (`expires_at > now`).
   - Replay A.
   - Family is fully revoked, `theft_detected` audit row is
     written.

6. **Backfill correctness.**
   Unit test:
   - Pre-populate `refresh_tokens` with `token_hash = NULL` and
     known `token_enc`.
   - Run backfill once.
   - All rows have `token_hash` matching `sha256(decrypt(token_enc))`.
   - Subsequent lookups by plaintext succeed via the index path.

## Security considerations

- **Hash leakage.** `token_hash` is not a secret in the same
  way the plaintext token is, but a leaked database with hashes
  + a precomputed table of issued tokens (which the attacker
  would have to construct from logs that don't contain
  plaintext tokens) is theoretically a reduction. In practice
  the attacker needs the master key to read anything else
  useful from the file, and at that point they have the
  `token_enc` plaintexts directly. No new attack surface.
- **Backfill window.** During backfill, `token_hash` is NULL on
  some rows and the lookup must fall back to the decrypt scan.
  The fallback is the *current* behaviour, so security is
  unchanged from the pre-RFC state during the window.
- **Disabled-user race.** The recheck closes a 60-second
  window. It does not close the window between an attacker
  acquiring an auth code and the human admin clicking "disable"
  — that is bounded by code TTL and is a separate concern. The
  recheck is purely a defence against the race admin/system
  *did* win but the data-flow lost.
- **Theft detection completeness.** A revoked token whose
  original `expires_at` has already passed is GC'd, and a replay
  of it returns `NotFound`, not `theft_detected`. This is the
  documented behaviour; expanding it (e.g., a separate
  `revoked_tombstones` table that outlives `expires_at`) is a
  larger design with more invariants and is not in scope.
  Recorded as a future-work note in `docs/threat-model.md`.

## Multiple implementation steps

These can ship in two pieces if the maintainer prefers smaller
PRs:

- **Step 1 (smallest possible fix, ships first).** Sections § 1
  + § 3 + § 5: the user-state recheck in `exchange_code`, the
  invalidation on disable/delete, and the GC fix. Code-only,
  no migration. Closes the highest-impact gap (token issuance
  to disabled users) and the theft-detection regression
  immediately.
- **Step 2.** Sections § 2 + § 4 + § 6: the migration with
  auth_codes rebuild + refresh_tokens token_hash + backfill.
  Schema change, larger surface area, requires the
  RFC-021-style transactional migration runner to be in place.

If RFC 021 lands first the transactional migration runner is
already there; otherwise this RFC's Step 2 either depends on
RFC 021 § 6 or carries its own minimal transactional wrapper.

## Open questions

1. **Hard-delete CLI path.** § 2's FK cascade only fires on
   hard-delete. Today there is no admin UI for hard-delete; it
   is a CLI subcommand referenced from RFC 017's user-management
   contracts. Verify that hard-delete actually issues
   `DELETE FROM users WHERE id = ?` rather than a soft-delete,
   so the cascade triggers as expected. If it doesn't, the FK
   semantics become decorative.
2. **Backfill error policy.** If backfill encounters a row whose
   `token_enc` does not decrypt (key-rotation gone wrong, byte
   corruption), what does it do? Options: skip with a warning,
   delete the row, fail startup. Recommend skip-with-warning
   for graceful operation; revisit if a real deployment hits
   this and prefers loud failure.
3. **`token_hash` in audit logs.** Should `token_hash` (or a
   short prefix of it) appear in audit rows for refresh-token
   events, to aid forensics? RFC 016's redaction list does not
   list it explicitly. If the maintainer wants a forensic
   handle, add the first 8 bytes of the hash. If not, hashes
   stay out of audit. Default: stay out (simpler).
