# Architecture

## Crate graph

```
sui-id (binary)
 ├── sui-id-web     (Leptos SSR, pages, layout, components)
 │    └── sui-id-i18n   (Locale enum, Strings, Formatters)
 ├── sui-id-core    (domain logic: passwords, JWT, OIDC, sessions, email)
 │    ├── sui-id-store  (SQLite, migrations, repositories, column encryption)
 │    │    └── sui-id-shared  (DTOs, typed IDs, error types)
 │    └── sui-id-i18n
 └── sui-id-store
```

There are no circular dependencies. `sui-id-shared` sits at the bottom;
`sui-id` sits at the top.

## Request lifecycle

```
Browser request
 → Axum router (crates/sui-id/src/http/router.rs)
   → middleware (HSTS, request ID, rate limiter, session resolver)
   → handler (crates/sui-id/src/http/handlers/)
     → core use-case function (crates/sui-id-core/src/)
       → repository (crates/sui-id-store/src/repos/)
         → SQLite (single connection, WAL mode)
     → render_* function (crates/sui-id-web/src/pages.rs)
       → Leptos SSR → HTML string
   → Response
```

## Storage model

One SQLite database file, one master key file. `Database` is a handle over
an `Arc<dyn Backend>` (`crates/sui-id-store/src/backend.rs`): the storage
engine sits behind a trait, and `SqliteBackend` is the implementation that
owns the connection and dispatches blocking I/O to a dedicated thread pool
via `spawn_blocking`. The seam is deliberate — RFC 009 step 1 introduced it
so an alternative SQL backend becomes an implementation of `Backend` rather
than a rewrite of every repository. SQLite is the only implementation today,
and RFC 009's remaining steps are unscheduled.

### Column encryption

Sensitive columns (refresh tokens, TOTP secrets, SMTP password, passkey
bytes, signing-key private bytes) are sealed with XChaCha20-Poly1305.
Each seal call takes `(key, plaintext, aad)` where `aad` is a
context-specific Additional Authenticated Data byte string. This binds
the ciphertext to its table and column, preventing cross-column
transplantation attacks.

### Master key resolution

```
1. SUI_ID_MASTER_KEY env var (base64-encoded 32 bytes)
2. key_file path from config
3. If neither: generate a new key and write to key_file
```

## Session lifecycle

```
Login (POST /admin/login)
 → verify password → verify MFA (if enrolled)
 → insert SessionRow, set session cookie
 → redirect to /admin

Authenticated request
 → session_cookie extractor reads cookie
 → sessions::resolve checks: not revoked, not expired, idle timeout
 → CurrentAdmin / CurrentUser extractor

Logout (POST /admin/logout)
 → sessions::revoke
 → clear session cookie
```

## Audit log

A Class-A mutation and its audit row commit in one transaction. The write
command is declared with `declare_write_command!`, which seals the event's
name, class, actor requirement and attributes into a typed descriptor, and
the runner hands the domain function a `WriteTx<AtomicAudit>` — so the
mutation cannot land without its audit row, and the audit row cannot name an
event the registry does not declare. RFC 094 is the design; the registered
event set is `ci/audit-coverage-matrix.md`, which lane G13 checks against the
source literals in both directions.

`events::emit(db, clock, ctx, event)` remains for Class-B events — the
best-effort, non-transactional ones, six call sites, all in
`crates/sui-id-core/src/account/forgot_password.rs`. It is no longer the
path every mutation takes, as it was before RFC 094.

Either way a row lands in `audit_log` with a SHA-256 hash chained to the
previous row. The audit page verifies the chain's tail on load.

## i18n

All user-visible strings pass through the `Strings` struct in `sui-id-i18n`.
The struct is fully populated at compile time for each supported locale;
missing fields are compile errors. The locale resolution chain for admin pages:

1. Admin user's `preferred_lang` (from `users` table).
2. `server_settings.default_lang` (operator-configured).
3. `Locale::Ja` fallback.

For end-user pages (login, MFA, etc.), the chain additionally considers
the `sui_id_lang` cookie and the `Accept-Language` header.

## Security invariants

- `unsafe_code = "forbid"` is enforced workspace-wide.
- Authentication failure responses always run a full Argon2id verification
  to maintain timing equality (dummy verify on non-existent users).
- Refresh token theft detection: every rotation stores the new family; a
  second presentation of the old token revokes the entire family.
- PKCE S256 is mandatory for all authorization code flows; Implicit flow
  is not implemented.
- `redirect_uri` requires exact byte-for-byte match; wildcards are not
  supported.
