# Federation sign-in: fail closed on the MFA read; no session for inactive users

**Authorized by.** [`ROADMAP.md`](../../ROADMAP.md) §Non-RFC work packages. Owner
authorization 2026-09-17: a live security defect, fixed ahead of RFC 102's L04 and
RFC 096-B1.
**Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Found by.** RFC 102/103 design review, H4.

## The defects
1. **MFA is skipped when the MFA read fails.** `complete_federated_signin`
   (`crates/sui-id/src/http/handlers/federation.rs`) computes
   `is_mfa_enabled(...).await.unwrap_or(false)`, so a read error skips local MFA
   and issues a session.
2. **Disabled and deleted users get sessions.** `federation.rs` has no
   `is_disabled` or `is_deleted` check. Such a session is refused at `/admin` and
   `/oauth2/authorize`, but `SessionContext` (`crates/sui-id/src/http/handlers.rs`)
   accepts it, so `/me/*` works.

## Required
- **The MFA decision comes from a successful read.** On error, refuse: redirect to
  `/admin/login?fed_error=signin_failed`, and log the cause at error level.
- **Refuse inactive users.** Before any session insert, re-read the user; a
  disabled or deleted user is refused with the same redirect.
- **Defense in depth.** `SessionContext` rejects a session whose user is disabled
  or deleted, like the admin extractors already do. It must not add a second
  database round trip where `session::resolve` can return the user state in the
  same read; measure it and say which.
- **Do not restructure** `federation.rs` beyond these checks. Its module split is
  withdrawn, and RFC 096-B1 replaces this path.

## Evidence
- Tests for each defect: an injected MFA read error gives no session; a disabled
  user and a deleted user with a federation link get no session; an existing
  session of a user disabled afterwards is refused on `/me/security`.
- Mutation: remove each check, and show its test failing.
- fmt, both clippy scopes, test count before and after, MSRV 1.95.
