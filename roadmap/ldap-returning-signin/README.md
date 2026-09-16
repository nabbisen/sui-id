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

## Rulings on the stop of 2026-09-17 (architect)

The owner's first preference was option A, stated in the implementer's session
and routed here. The architect confirms it, with the conditions below.

1. **Option A: authenticate by stable id.** Add
   `UserSource::authenticate_stable_id(stable_id, password)`. Routing for a local
   row with `source = ldap`, and RFC 102 B7's `rebind_directory_user`, both call
   it. There is no migration, and no change to the shadow upsert.
2. **The directory's own restrictions must still apply.** This condition is not
   optional. `user_search_filter` often carries authorization, such as a group,
   an objectClass or an account-status clause. A search by stable id alone would
   bypass it, and a user removed from the allowed group could still sign in. The
   LDAP search is therefore
   `(&(<stable_id_attribute>=<RFC 4515-escaped id>)<user_search_filter with {username} replaced by *>)`
   under `user_search_base`. The filter-building function is pure and
   unit-tested, including:
   - escaping;
   - a filter with a group clause;
   - a filter with `{username}` in more than one position.
3. **DN fallback.** When the stored stable id is a DN (the attribute was missing
   or binary at provisioning), use a base-object search on that DN, with the same
   filter using `*`. Only a DN that lies under `user_search_base` is searched,
   compared case-insensitively and normalised. Otherwise the result is `Ok(None)`.
   Unit-test the containment check.
4. **P3 is preserved.** A miss and a wrong password are both `Ok(None)`, and both
   run a search and a bind attempt.
5. **Live-directory coverage is not available in this lane.** Unit-test the pure
   parts, and state plainly that the search and bind are compile-checked only.
   Record it as input for RFC 099's live integration evidence; do not add a
   container fixture in this package.
6. **§3.1 MFA on the returning path: in scope.** It branches exactly like
   `login_with_mfa`: a pending-MFA row and
   `auth.login.password_ok_mfa_required`. Without it, a factor enrolled through
   B7 would be skipped at sign-in. That would be an MFA bypass, and it must not
   ship.
7. **§3.2 a record with a different stable id: agreed.** Refuse with the uniform
   401, do not count it, and log at warn with the source slug and user id (no
   password).

## Evidence
- End-to-end with `InMemoryUserSource`: first sign-in, second sign-in, a wrong
  password on the second (counted, 401), directory user removed (401), shadow user
  disabled (401, directory not consulted).
- An injected lookup error gives 401, the directory is not consulted, and the log
  line is present.
- Mutation on the routing condition and the error mapping.
- fmt, both clippy scopes, test count before and after, MSRV 1.95, and the
  all-features lanes (the `ldap` feature).
