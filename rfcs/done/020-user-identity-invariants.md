# RFC 020 — User identity invariants and OIDC claim consistency

**Status.** Implemented
**Priority.** High. The defects here have user-visible
consequences (forgot-password silently failing for capitalised
emails) and an OIDC-conformance gap (advertising the `email`
scope and not returning the claim).
**Tracks.** v0.29.5 data-model review — high-priority finding
#5 (`users.email` case sensitivity) and #6 (`users.user_uuid`
uniqueness); v0.29.5 review-2 §2 (OIDC `email` scope vs
userinfo); v0.29.5 review-2 §1 closing item (`email_verified_at`).
**Touches.** `crates/sui-id-store/src/migrations/0020_user_identity_invariants.sql`,
`crates/sui-id-store/src/repos/users.rs` (new lookup-by-
normalized-email path; existing `lookup_by_email` becomes a
thin wrapper), `crates/sui-id-core/src/admin.rs`,
`crates/sui-id-core/src/setup.rs`,
`crates/sui-id-core/src/forgot_password.rs`,
`crates/sui-id-core/src/userinfo.rs` (or wherever
`UserInfo` is built), `crates/sui-id-core/src/discovery.rs`
(unchanged but verified consistent).

## Summary

Three coupled defects in user identity:

1. **`users.email` is case-sensitive at the unique index, but
   forgot-password is case-insensitive at lookup.** A user
   created with `Alice@example.com` cannot reset their password
   by typing `alice@example.com`. Two separate users can be
   created differing only in case.
2. **`users.user_uuid` has no UNIQUE constraint.** WebAuthn
   uses `user_uuid` as the stable user handle; a duplicate
   would conflate two users at the credential layer.
3. **The OIDC `email` scope is advertised in discovery but
   userinfo does not return `email`.** RP integrators see a
   missing-claim and have no documented reason for it.
   `email_verified_at` is also absent from the data model, so
   `email_verified` cannot be returned even if `email` is.

Each defect is small. They are bundled because all three sit
on the same `users` row, all three need a single migration,
and the OIDC fix depends on having `email_normalized` and
`email_verified_at` columns to be honest about claims.

## Why high priority

- **Defect 1 is a real user-facing bug.** A user who registers
  with `Alice@…` and tries to reset later by typing the
  lowercase form gets a "we sent you an email if it exists"
  message, but no email arrives. This is indistinguishable from
  a bug in SMTP, and it is undebuggable without DB access.
- **Defect 2 is a credential-layer integrity gap.** WebAuthn
  expects `user_handle` to be globally unique. If two rows
  share `user_uuid`, the WebAuthn assertion path resolves the
  wrong user. This has not happened in production because
  user_uuid generation is via UUIDv4 and collisions are
  vanishingly unlikely, but the *DB* invariant is decorative.
- **Defect 3 is an OIDC conformance gap.** Returning
  `scopes_supported = ["openid", "profile", "email", "offline_access"]`
  while never returning `email` in userinfo is not technically
  forbidden, but it leads RPs to expect a claim that doesn't
  arrive. The discovery document and userinfo response should
  agree.

## Requirements

After this RFC ships:

1. `users.email_normalized` exists, populated by
   `email.trim().to_lowercase()` at write time.
2. The unique index for email lives on `email_normalized`,
   not on `email`.
3. `users.user_uuid` has a unique index. New user creation that
   would conflict fails with a clear error.
4. `users.email_verified_at` exists. It is `NULL` initially
   for all existing users.
5. The OIDC userinfo endpoint, when the access token's scope
   includes `email`, returns `email` (the original case-
   preserved form) and `email_verified` (boolean derived from
   `email_verified_at IS NOT NULL`).
6. The OIDC discovery document continues to advertise the
   `email` scope. It is now consistent with userinfo.
7. Forgot-password lookup is case-insensitive via
   `email_normalized`; the existing repository comment about
   "caller normalizes" is removed.
8. Existing tests pass. New tests cover the case-fold
   round-trip, the uuid uniqueness, and the userinfo email
   return.

## Design

### § 1. Schema changes

Migration `0020`:

```sql
-- 0020_user_identity_invariants.sql

-- email_normalized: separate column; backfill from existing email column
ALTER TABLE users ADD COLUMN email_normalized TEXT;
UPDATE users
   SET email_normalized = lower(trim(email))
 WHERE email IS NOT NULL;

-- swap the unique index from email to email_normalized
DROP INDEX IF EXISTS idx_users_email;
CREATE UNIQUE INDEX idx_users_email_normalized
    ON users(email_normalized)
    WHERE email_normalized IS NOT NULL;

-- email_verified_at: tracks when the address was verified, NULL otherwise
ALTER TABLE users ADD COLUMN email_verified_at TEXT;

-- user_uuid: UNIQUE constraint via partial index on non-empty values
-- (the column has DEFAULT '' from migration 0004, so partial predicate
-- excludes the legacy empty-string sentinel; once all rows are non-empty
-- the partial predicate becomes effectively a full UNIQUE)
CREATE UNIQUE INDEX idx_users_user_uuid
    ON users(user_uuid)
    WHERE user_uuid <> '';
```

The `WHERE user_uuid <> ''` partial predicate is conservative:
historically `user_uuid` carried `DEFAULT ''` (migration `0004`
added it as a backfill column), and it is possible some early
rows still hold `''`. If they do, the unique constraint would
fire on every new user with the empty default. The partial
predicate excludes those rows. New code paths always set a
non-empty UUID, so as legacy rows are touched and re-saved the
partial predicate's exception domain shrinks to zero.

A future migration (out of scope for this RFC) can backfill
empty `user_uuid` values to fresh UUIDs, after which the
partial predicate can be tightened to a full UNIQUE.

### § 2. Repository changes

`users::lookup_by_email(email)` is renamed to
`users::lookup_by_email_normalized(normalized: &str)` and
operates on `email_normalized`. A new helper:

```rust
pub fn normalize_email(input: &str) -> String {
    input.trim().to_lowercase()
}
```

lives in `sui-id-shared` so all call sites import the same
function (no per-site reinvention).

Write paths:

- `users::create(...)` and `users::update_email(...)` write
  both `email` (original case) and `email_normalized` (via
  `normalize_email`) atomically in the same SQL.
- The existing comment in `users.rs` ("case-sensitive; caller
  normalises") is deleted; the new contract is that the
  repository normalises and the caller passes the original
  form.

Read paths:

- `forgot_password::request_reset(email)` calls
  `normalize_email` and then `lookup_by_email_normalized`.
- Any other caller that takes user-input email
  (admin user creation form, setup) goes through the same
  helper.

### § 3. UserInfo response shape

Today's `UserInfo`:

```rust
pub struct UserInfo {
    pub sub: String,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
}
```

Becomes:

```rust
pub struct UserInfo {
    pub sub: String,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
    /// Original-case email. Returned only when access token scope
    /// contains `email`. None if the user has no email on record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Whether the email has been confirmed. Derived from
    /// `email_verified_at IS NOT NULL`. Returned only when `email`
    /// is also returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
}
```

The userinfo handler reads the access token, recovers the
granted scope set, and:

- If `email` is in scope and `users.email IS NOT NULL`,
  populates both fields.
- If `email` is in scope but the user has no email, omits both
  fields (this is the documented behaviour for missing claims).
- If `email` is not in scope, omits both fields regardless.

`email_verified` is `Some(true)` iff `email_verified_at IS NOT
NULL`, else `Some(false)`. It is omitted entirely when
`email` itself is omitted, per OIDC convention that
`email_verified` is meaningless without `email`.

`email_verified_at` is *not* set anywhere by code that ships
in this RFC. It will be `NULL` for every user — equivalently,
`email_verified = false` for every userinfo response — until a
future RFC adds an email-verification flow. This is honest:
sui-id has not verified anyone's email; saying so is correct.
Returning `email_verified = false` is informative for RPs
that condition on it.

### § 4. Discovery document

No change. The `scopes_supported` list already includes `email`.
The fix is on the userinfo side, not the discovery side. (The
review's option B — drop `email` from scopes_supported — is
rejected: now that userinfo will return `email`, advertising it
is correct.)

### § 5. ID token claims

The data review notes that `IdTokenClaims` is also minimal.
`email` and `email_verified` could be added to the ID token
when scope=email. This RFC does **not** add them to the ID
token, intentionally: ID tokens are passed around between
client and IdP and frequently logged; reducing the email
exposure to userinfo (which requires presenting an access
token) is the more conservative shape and matches OIDC's
recommended practice. RPs that want the email on every
ID-token receive should call userinfo once after token
exchange and cache the result.

If a maintainer decides differently in the future, adding
`email` / `email_verified` to `IdTokenClaims` is a small
follow-up. The shape designed here does not preclude it.

## Tests

1. **Email case-fold round-trip.** Unit test in
   `users.rs`: create user `Alice@Example.com`, assert
   `lookup_by_email_normalized("alice@example.com")` returns
   the row. Asserts the original `email` column preserves
   `Alice@Example.com`.

2. **Email uniqueness across cases.** Unit test:
   create user with email `Alice@example.com`, attempt to
   create another with `alice@EXAMPLE.com`. Second create
   fails with the unique-index error.

3. **Forgot-password case-insensitivity.** e2e in
   `crates/sui-id/tests/e2e/forgot_password_case.rs`: register
   `Alice@example.com`, request reset with
   `alice@example.com`, verify a reset email is generated for
   the correct user.

4. **`user_uuid` uniqueness.** Unit test in `users.rs`:
   insert user with explicit `user_uuid = "uuid-A"`, attempt
   second insert with same UUID. Second insert fails.

5. **UserInfo with `email` scope.** e2e: token issued with
   scope `openid email`, userinfo returns `email` and
   `email_verified: false`. Token issued with scope `openid`
   only, userinfo omits both.

6. **UserInfo with no email on record.** e2e: user has no
   email, token has scope=email, userinfo omits both fields
   (does not return `email: null`).

## Security considerations

- **Email enumeration via case differences.** Pre-RFC,
  registering `Alice@…` and `alice@…` was technically possible
  and could be used to bypass uniqueness checks. Post-RFC the
  unique constraint catches it. There is a brief window during
  the migration where two such rows could already exist; the
  migration's `UPDATE users SET email_normalized = ...` would
  produce a UNIQUE-violation if so. The migration in § 1 does
  not handle this — it lands as a hard error during upgrade,
  which is the right outcome (operator must resolve the
  duplicate manually before proceeding). A pre-flight check
  belongs in the operator-facing upgrade docs:

  ```sql
  SELECT lower(trim(email)) AS norm, count(*) AS n
    FROM users WHERE email IS NOT NULL
    GROUP BY norm HAVING n > 1;
  ```

  Documented in `docs/operators.md` as part of the upgrade
  notes for the release that ships this RFC.
- **Userinfo leak via email.** The userinfo endpoint is
  authenticated (Bearer access token). Returning `email`
  there is no different from returning `name` there in terms
  of who can see it. RP integrators and audit logs already
  treat the access token as the authority-to-read-claims.
- **`email_verified = false` honesty.** Returning a literal
  `false` is more conservative than omitting the claim; an RP
  that conditions on `email_verified === true` makes the right
  decision (refuse to treat the email as authoritative). RPs
  that misread `false` as "no claim returned" are buggy.
- **`user_uuid` partial UNIQUE.** The partial predicate
  excludes empty-string rows, which means in principle a rogue
  code path that wrote `user_uuid = ''` could create
  duplicates. Code paths that write `user_uuid` are limited
  to user creation (which sets a fresh UUIDv4) and migration
  `0004` (which set the default). No normal path can write
  `''`. The risk is limited to a misuse of the lower-level
  repository, which is in-process and reviewable.

## Open questions

1. **Email verification flow.** This RFC adds the column and
   wires the claim, but does not add the flow that actually
   verifies an email. Verification requires SMTP configured
   (RFC 001's email outbox helps here), an "email confirmation"
   token table (similar shape to `password_reset_tokens`), and
   a `/me/security` UI affordance. Recommend tracking as a
   separate medium-priority RFC after RFC 001 lands.
2. **Migration error if duplicates exist.** The pre-flight
   query in `docs/operators.md` lets operators detect
   duplicates before upgrading. If a duplicate is found, what
   does the operator do? The right answer depends on which row
   they consider authoritative — that is a per-deployment
   decision. Document the SQL for resolving (UPDATE one row's
   email to a placeholder, or DELETE) and let the operator
   choose.
3. **Should `name` use the same case-preservation rule?**
   `display_name` is already case-preserving. The pattern from
   email — preserve original, normalise for lookup — could be
   extended to `username`, but `username` is not a search key
   (it's an exact identifier), so it doesn't currently need
   normalisation. Out of scope unless usability research
   suggests otherwise.
