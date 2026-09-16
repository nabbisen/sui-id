# A returning LDAP user can sign in

**Authorized by.** [`ROADMAP.md`](../../ROADMAP.md) §Non-RFC work packages. Owner
authorization 2026-09-17. **Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Found by.** RFC 102/103 design review, H3 and L4.

## The defects
1. **The second sign-in is refused.** The first LDAP sign-in creates a shadow user
   named with the typed username. On the second attempt, `login_with_mfa` finds
   that user locally, `credentials::get` returns NotFound → `InvalidCredentials`,
   and `try_login_with_cascade` treats the user as known locally and never
   consults the directory (`crates/sui-id/src/http/handlers/admin/auth.rs`). The
   design review measured it with `InMemoryUserSource`: the first sign-in gives
   303 with a cookie, the second 401 with no cookie.
2. **Any lookup error counts as "unknown locally".** `try_login_with_cascade`
   treats *any* `find_by_username` error that way (L4), so a database error falls
   through to the directory.

## Required
- **Route existing external users to their source.** A local user whose `source`
  is `Ldap` authenticates against the user sources, never against a local
  credential. Disabled or deleted users are refused before the directory is
  asked.
- **A wrong password is counted.** It counts on the shadow user exactly as a
  local wrong password does (U22), and gets the uniform 401.
- **Only NotFound means unknown.** Every other lookup error is an error: the
  uniform 401, logged (the R11 1b pattern).
- **The RFC 102 B7 re-bind uses the same mapping** (added 2026-09-17).
  `rebind_directory_user` (`crates/sui-id/src/http/handlers.rs`) authenticates with
  the local `username`. A shadow row whose name was suffixed on collision
  therefore fails to re-bind, and each attempt counts as a wrong password. Use the
  directory identity this package establishes, and add a test for a suffixed
  shadow user.
- **Keep existing names.** Preserve today's shadow-user upsert and session
  creation. RFC 102's L03 converts them later, so do not pre-empt its design.

## Evidence
- End-to-end with `InMemoryUserSource`: first sign-in, second sign-in, a wrong
  password on the second (counted, 401), directory user removed (401), shadow user
  disabled (401, directory not consulted).
- An injected lookup error gives 401, the directory is not consulted, and the log
  line is present.
- Mutation on the routing condition and the error mapping.
- fmt, both clippy scopes, test count before and after, MSRV 1.95, and the
  all-features lanes (the `ldap` feature).
