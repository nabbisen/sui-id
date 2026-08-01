# RFC 021 — Schema invariant CHECKs and migration safety

**Status.** Implemented
**Priority.** Medium. No live correctness defects today; this RFC
strengthens DB-layer guarantees so the application can rely on
data shape without re-validating in every repo function. Also
includes a real operational safety fix for migrations.
**Tracks.** v0.29.5 data-model review — medium-priority findings
#7 (boolean CHECKs), #6 (clients confidential/secret_hash),
#8 (consents redesign), #9 (signing_keys active-key invariant),
#10 (JSON validity), and findings #11 (sessions index) and
#12 (migration transactions) from the same review.
**Touches.** A single migration file
`crates/sui-id-store/src/migrations/0021_schema_invariants.sql`,
the migration runner in
`crates/sui-id-store/src/migrations.rs` (transactional
wrapping), the signing-key rotation path in
`crates/sui-id-core/src/keys.rs` (or wherever rotation lives) to
match the new partial unique index, and an application-layer
JSON validator helper used at all write paths into the JSON-
TEXT columns listed below.

## Summary

The data-model review enumerates a set of DB-layer invariants
that today live as application invariants only:

- `clients.confidential` is allowed values `0` and `1` by
  convention; `secret_hash` must be present iff
  `confidential = 1`. Neither rule is enforced at the DB.
- A pile of boolean-shaped `INTEGER` columns (`users.is_admin`,
  `users.is_disabled`, `is_deleted`, `credentials.must_change`,
  `clients.is_disabled`, `clients.is_deleted`,
  `signing_keys.is_active`, `user_totp.enabled`, etc.) accept
  `2`, `-1`, etc.
- `signing_keys` allows multiple `is_active = 1` rows. The
  application picks the most recent. The "exactly one active"
  invariant is enforced by ordering, not by the DB.
- `consents` has no FK and an ambiguous primary key for an
  ambiguous semantics ("scope" as one scope or as a scope set?).
- JSON-TEXT columns (`clients.redirect_uris`, etc.) carry no
  validity check; a corrupted row triggers `serde_json::Error`
  at the next read.
- Migrations are not wrapped in transactions: a partial
  failure leaves `schema_version` un-bumped while the partially
  applied DDL remains, requiring manual recovery on the next
  run.
- The `sessions(user_id, created_at)` partial index is good for
  FIFO eviction but not the most natural index for the
  active-and-not-expired query that runs on every authenticated
  request.

These are bundled in one RFC because:

1. They share a single migration. SQLite can `ALTER TABLE` for
   adding columns but not for adding `CHECK` constraints; the
   tables involved (clients, users, credentials, signing_keys,
   user_totp, ...) need rebuild-style migration. A single
   migration that does the rebuilds end-to-end avoids paying
   the rebuild cost N times.
2. The transactional-migration fix is a precondition for
   reliably running this migration itself.
3. Each individual fix is too small to justify a standalone
   RFC, but together they materially raise the DB-layer
   guarantee.

## Why medium priority, not high

The application code today writes only valid values. The risk
is from out-of-band data sources: a maintainer running ad-hoc
SQL, a corrupted backup, a partial migration. Those scenarios
produce data the application fails on at the next read. The
fix prevents the data from existing in the first place.

It is *not* high-priority because no shipping deployment is
known to have such data. RFC 019 and RFC 020 close real
defects; this one closes hypothetical ones.

The migration-safety fix (§ 6 below) is an exception: it is a
real operational safety hole and would be high-priority on its
own. It rides along here because it touches the same migration
runner and adds one well-bounded change.

## Requirements

After this RFC ships:

1. Boolean-shaped INTEGER columns carry `CHECK (col IN (0, 1))`.
2. `clients` enforces `confidential IN (0, 1)` and
   `(confidential = 1) = (secret_hash IS NOT NULL)`.
3. `signing_keys` allows at most one `is_active = 1` row at
   any time, enforced by partial unique index.
4. `consents` has FKs to `users(id)` and `clients(id)`, an
   `updated_at` column, a `granted_scopes` column with a
   single-row-per-pair semantics, and a primary key of
   `(user_id, client_id)`.
5. JSON-TEXT columns are validated at every write call site
   in the repo layer; corrupted reads return a typed error
   that can be handled rather than an opaque
   `serde_json::Error`.
6. Migration runs are wrapped in a transaction per migration
   file. A failed migration leaves the DB unchanged.
7. The active-session query path uses an index that matches
   its WHERE clause shape.
8. Existing tests pass. New tests cover each invariant
   addition (one CHECK violation per new constraint).

## Design

### § 1. Boolean CHECKs

The columns:

```
users.is_admin
users.is_disabled
users.is_deleted
credentials.must_change
clients.is_disabled
clients.is_deleted
signing_keys.is_active
user_totp.enabled
auth_codes.consumed             (already added by RFC 019)
```

For each of `users`, `clients`, `credentials`, `signing_keys`,
`user_totp`, the table is rebuilt:

```
CREATE TABLE _new ( ... CHECK (col IN (0, 1)) ... );
INSERT INTO _new SELECT * FROM old;
DROP TABLE old;
ALTER TABLE _new RENAME TO old;
```

Indices and triggers on each old table must be re-created on
the new table. The migration file lists each rebuild block
explicitly; auto-generation is not used because the explicit
form is auditable.

`refresh_tokens.revoked_at IS NOT NULL` is already a
boolean-by-NULLability and does not need a CHECK.

### § 2. `clients.confidential` ↔ `secret_hash` consistency

Inside the `clients` rebuild from § 1:

```sql
CREATE TABLE _clients_new (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    confidential INTEGER NOT NULL
        CHECK (confidential IN (0, 1)),
    secret_hash TEXT,
    redirect_uris TEXT NOT NULL,
    post_logout_redirect_uris TEXT NOT NULL,
    allowed_scopes TEXT NOT NULL,
    is_disabled INTEGER NOT NULL DEFAULT 0
        CHECK (is_disabled IN (0, 1)),
    is_deleted INTEGER NOT NULL DEFAULT 0
        CHECK (is_deleted IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (
        (confidential = 1 AND secret_hash IS NOT NULL)
        OR
        (confidential = 0 AND secret_hash IS NULL)
    )
);
```

Pre-flight pre-migration query (operator runs before upgrade,
documented in operators.md):

```sql
SELECT id FROM clients
 WHERE (confidential = 1 AND secret_hash IS NULL)
    OR (confidential = 0 AND secret_hash IS NOT NULL);
```

If rows return, they are repaired before upgrade — a stale
secret_hash on a public client is dropped, a missing
secret_hash on a confidential client is regenerated by an admin
through the UI before upgrade. This is the same
pre-flight pattern as RFC 020.

### § 3. `signing_keys` single-active invariant

The current rotation path: insert new key with `is_active=1`,
then update old key to `is_active=0`. Briefly there are two
active keys; the read path tolerates this by ordering on
`created_at DESC LIMIT 1`.

To enforce "exactly one active" at the DB:

```sql
CREATE UNIQUE INDEX idx_signing_keys_single_active
    ON signing_keys(is_active)
    WHERE is_active = 1;
```

This means rotation must reverse order: retire old first, then
insert new. Briefly there are zero active keys. The window is
inside one transaction, so external readers never see the
intermediate state.

```rust
// crates/sui-id-core/src/keys.rs (or signing.rs)
pub fn rotate(db: &Database) -> CoreResult<SigningKeyRow> {
    db.with_tx(|tx| {
        let new_id = generate_key_id();
        let (priv_enc, public) = make_ed25519_keypair_sealed(db.key())?;
        // 1. retire any current active key
        tx.execute(
            "UPDATE signing_keys SET is_active = 0, rotated_at = ?1
             WHERE is_active = 1",
            (&now,),
        )?;
        // 2. insert new active
        tx.execute(
            "INSERT INTO signing_keys
                (id, algorithm, private_key_enc, public_key,
                 is_active, created_at, rotated_at)
             VALUES (?1, 'EdDSA', ?2, ?3, 1, ?4, NULL)",
            (&new_id, &priv_enc, &public, &now),
        )?;
        // 3. fetch and return
        signing_keys::active(tx)
    })
}
```

This requires the migration runner to expose `with_tx`. RFC 013
proposes a broader refactor; for this RFC, a minimal
`Database::with_tx<F>` helper is sufficient.

JWKS publication continues to publish "active and recently
retired" keys as a Vec; nothing in that path requires multiple
*active* keys.

### § 4. `consents` redesign

Drop and rebuild:

```sql
DROP TABLE consents;
CREATE TABLE consents (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    granted_scopes TEXT NOT NULL,    -- space-separated scope set, like access tokens
    granted_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, client_id),
    CHECK (length(granted_scopes) > 0)
);
```

The data-review notes the alternative shape with `scope` in
the primary key (one row per scope per (user, client)). The
chosen shape (one row per (user, client), scopes joined as
space-separated string, matching how access tokens carry
scope) keeps the row count bounded and avoids the
delete-then-insert pattern when scopes change.

The table has no production users yet (RFC 008 third-party
posture is the consumer), so DROP-then-CREATE is acceptable.

If RFC 008 has not yet defined consent semantics by the time
this RFC ships, the rebuild here is the placeholder shape;
RFC 008 may further amend the table when it lands. The
amendment will be a small additive ALTER, not a full rebuild,
because the keys and FKs are correct.

### § 5. JSON validity

SQLite JSON1 may or may not be available depending on build
profile. Rather than depend on it, validate at the application
layer in repository write paths.

A small helper in `sui-id-store::repos::util`:

```rust
fn require_valid_json<T: DeserializeOwned>(s: &str) -> StoreResult<()> {
    serde_json::from_str::<T>(s).map(|_| ()).map_err(|e| {
        StoreError::CorruptJson { context: "repo write", source: e }
    })
}
```

Each write into a JSON-TEXT column passes through this check
with the expected target type:

- `clients.redirect_uris` → `Vec<String>`
- `clients.post_logout_redirect_uris` → `Vec<String>`
- `clients.allowed_scopes` → `Vec<String>`
- `auth_methods` (location TBD per code review) → expected
  shape known at call site
- `webauthn_pending.state_json` → `webauthn_rs::PasskeyRegistration`
  or analogue.

`user_totp.recovery_codes_enc` is encrypted JSON, validated
after decrypt.

A `sui-id admin verify-json` CLI subcommand is added to
back-walk the existing rows in each JSON-TEXT column and
report any that fail to deserialize. This is the "repair
command" alternative to a `CHECK (json_valid(...))`. Operator
runs it after upgrade if they have reason to suspect old
corruption.

### § 6. Migration transactions

Today's runner:

```rust
for m in pending {
    conn.execute_batch(m.sql)?;
    conn.execute("INSERT OR REPLACE INTO sui_meta(...)", ...)?;
}
```

Replace with:

```rust
for m in pending {
    let tx = conn.transaction()?;
    tx.execute_batch(m.sql)?;
    tx.execute(
        "INSERT OR REPLACE INTO sui_meta(key, value) VALUES(?1, ?2)",
        (META_KEY_SCHEMA_VERSION, m.version.to_string()),
    )?;
    tx.commit()?;
}
```

A migration that fails partway leaves the DB at the previous
schema version. The next run reattempts the same migration
cleanly.

Caveat: SQLite does not support some DDL statements inside a
transaction, notably `VACUUM`. None of sui-id's existing
migrations use those, and any future migration that needs them
is responsible for its own recovery story (and probably should
not be a versioned migration).

The boolean PRAGMA `foreign_keys` is settable inside a
transaction. The migration-internal `PRAGMA foreign_keys = OFF`
that RFC 019 § 2 uses for `auth_codes` rebuild operates within
the surrounding transaction.

### § 7. Sessions index improvement

Today (migration `0018`):

```sql
CREATE INDEX idx_sessions_user_active
    ON sessions(user_id, created_at)
    WHERE revoked_at IS NULL;
```

The active-session query is roughly:

```sql
SELECT * FROM sessions
 WHERE user_id = ?1
   AND revoked_at IS NULL
   AND expires_at > ?2
 ORDER BY created_at;
```

The current index supports the `user_id` + `revoked_at IS NULL`
filter and the `created_at` order, but not the `expires_at >
?` filter. Add a more specific index:

```sql
CREATE INDEX idx_sessions_user_active_alive
    ON sessions(user_id, expires_at, created_at)
    WHERE revoked_at IS NULL;
```

The original index is **not** dropped — FIFO eviction
(`ORDER BY created_at LIMIT N`) without an `expires_at` filter
benefits from the existing one. Carrying both is acceptable
for a query path this hot. Once benchmarks (RFC 014) show one
of them is unused, drop the unused one.

### § 8. Pre-flight queries (collected)

The migration assumes the existing data is consistent with
the new constraints. Operators should run these pre-flight
queries in a maintenance window before upgrading:

```sql
-- boolean values out of {0, 1}
SELECT 'users.is_admin' AS col, count(*) AS bad
  FROM users WHERE is_admin NOT IN (0, 1)
 UNION ALL ...

-- clients confidential/secret_hash mismatch
SELECT id FROM clients
 WHERE (confidential = 1 AND secret_hash IS NULL)
    OR (confidential = 0 AND secret_hash IS NOT NULL);

-- signing_keys multiple active
SELECT count(*) AS active_count FROM signing_keys WHERE is_active = 1;
-- expected: 0 or 1
```

These ship as a single SQL file at `docs/operators/preflight-
0021.sql` referenced from `docs/operators.md`'s upgrade
section. Operators paste it into their `sqlite3` shell.

The migration file itself does **not** run these checks; if
data fails them the migration aborts on the constraint, which
is the correct outcome (the operator must repair the data).
A bonus check could be a `BEFORE` verification step printed by
the runner; left as future work.

## Tests

For each new constraint, a unit test that violates it and
expects an error:

- Insert `users.is_admin = 2` → CHECK violation.
- Insert client with `confidential = 1, secret_hash = NULL` →
  CHECK violation.
- Insert two `signing_keys` with `is_active = 1` → unique
  violation.
- Insert `consents` with non-existent `user_id` → FK violation.
- Repository write of malformed JSON to `redirect_uris` →
  `StoreError::CorruptJson`.

Migration-runner tests:

- A migration whose SQL fails partway leaves
  `schema_version` un-bumped and the DB unchanged.
- Two pending migrations applied in sequence; the second one
  fails; the first one's effects are committed; the second
  one is retryable.

Sessions-index test: an EXPLAIN QUERY PLAN comparison from
before/after, covered indirectly by the RFC 014 benchmark
once that lands.

## Security considerations

The CHECK constraints reduce, not increase, the surface area
where a malformed row can mislead the application. There is
no new attack surface.

The `signing_keys` partial unique index closes a small but
real risk: a code path that inserts a new active key without
retiring the old one results in the JWKS endpoint publishing
two active keys, neither of which is wrong, but which
complicates rollback. The fix tightens the invariant.

The migration-transaction fix prevents the failure mode where
a partial migration is committed to disk but the version
metadata is not, leading to a duplicate-column / orphaned-
table situation that requires manual operator intervention.

## Multiple implementation steps

This RFC is large for a "medium-priority cleanup" and may
land in two stages if reviewer bandwidth is the constraint:

- **Step 1.** § 6 (transactional migrations), § 5 (JSON
  validation helper). No schema change. Safe to ship
  immediately.
- **Step 2.** § 1, § 2, § 3, § 4, § 7. Requires the
  pre-flight pass and is the bulk of the migration file.

If RFC 019 and RFC 020 land first, their migrations will
already exist; this RFC's migration `0021` follows.

## Open questions

1. **JSON1 availability check at startup.** sui-id could
   detect JSON1 at startup and use `CHECK (json_valid(...))`
   when present, falling back to app-level only otherwise.
   Adds complexity for no functional gain unless build
   environments diverge enough that the question matters.
   Recommend: stay with app-level only for now.
2. **Hardening `user_uuid` to `NOT NULL` and removing the
   `''` default.** Out of scope here; tracked under RFC 020's
   open questions.
3. **`consents` shape stability.** RFC 008 (third-party-
   posture) may amend `consents` further when it lands. The
   shape here is intended as the floor that RFC 008 builds on,
   not the ceiling. If RFC 008 needs per-scope rows or a
   history table, that is additive.
