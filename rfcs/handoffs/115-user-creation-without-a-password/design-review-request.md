# RFC 115 — independent design review request

**RFC.** [RFC 115 — Creating a user without choosing their password](../../accepted/115-user-creation-without-a-password.md). Proposed.
**Reviewer.** Mid-capability model, implementation role. It authored neither the
RFC nor its handoff.
**Route.** Owner decision of 2026-08-26 (`ROADMAP.md` §S1): a design is reviewed
by the role that must build against it.
**Why now.** `@nabbisen` accepted this RFC on 2026-09-24. RFC 000 requires a
named independent design reviewer for a security-sensitive RFC, and G11 refuses
an Accepted RFC whose `Independent design review` field has no durable
reference. **This RFC leads release cycle A**, so this review is the critical
path.
**Baseline.** `9287e0d` or later.
**Scope.** Read-only. Change no code and no RFC text. Report findings.

## Why this RFC exists, stated plainly

RFC 103 closed the path that let an administrator **re-set** an existing user's
password (U06) and left untouched the path that sets **every** local user's
first one (U01). So RFC 103's own closure prerequisite 4 — "no code path lets
anyone other than the account holder choose or learn a password" — is not met,
and `docs/threat-model.md` had to be narrowed on 2026-09-22 to stop claiming
otherwise. **RFC 103 cannot close until this lands.**

The findings below came from the architect re-tracing every production writer
of the `credentials` table. They have not been independently checked. Check
them.

## 1. The claims this RFC rests on — confirm or refute each

One row per claim: the claim, the `file:line` you read, and whether it holds.

1. **`/admin/users/new` carries a required password field**, which reaches
   `CreateUserSpec.password` → `hash_password` → `CredentialRow { must_change:
   false }` → U01. Give the chain with line numbers.
2. **There are exactly seven production writers of `credentials`**, and six of
   them are the account holder or the operator's own account: `setup.rs` (the
   first-run wizard), `cli.rs` (`sui-id setup`, headless), `me_security.rs`
   (twice), U09, U10 — and U01 is the seventh and the gap. **An eighth writer
   is a blocker.** Include raw SQL against the `credentials` table in your
   search, not only `credentials::upsert`.
3. **`must_change` is read by nothing.** No sign-in path consults it. The
   architect grepped `crates/sui-id-core/src/authn/` and
   `crates/sui-id/src/http/` and found only a test fixture. Search wider.
4. **`sui-id setup`'s doc comment promises a rotation requirement the product
   does not implement** — "stored with `must_change = true`, and printed ONCE
   to stdout".
5. **Eight password-bearing HTTP form structs derive `Debug` over plaintext
   fields**, and **none is logged today**. The eight: `SetupAdminForm`,
   `ResetPasswordForm`, `EmailSettingsForm` (the SMTP password), `LoginForm`,
   `PasswordChangeForm`, `PasskeyRegisterStartForm`, `MfaEnrollStartForm`,
   `CreateUserForm`. Confirm the list is complete and that none reaches a log
   line, a panic message, or an error body.
6. **`commands::create_user` already takes `credential:
   Option<CredentialRow>`** and skips the write on `None`, so a user with no
   credential row is representable today without a migration.
7. **Username reuse is not an impersonation path**: `username` is `NOT NULL
   UNIQUE`, deletion is soft, and the OIDC `sub` is the `UserId`.

## 2. Is the replacement buildable?

The design: `/admin/users/new` drops the password field; the administrator
creates the account and lands on RFC 103's recovery-link issuance.

8. **Can a user with no credential row exist end to end?** Trace what happens
   when one tries to sign in, requests a password reset, or is enumerated by
   the admin list. Name anything that assumes a credential row exists —
   `credentials::get(...).expect`, an `unwrap`, a join that drops the row, a
   page that renders "password set" unconditionally.
9. **What dies with the password field?** `check_password_policy`,
   `hibp::enforce_hibp` and `min_password_len` are currently called on the
   administrator's chosen password at creation. If no password is chosen there,
   say where those checks still run — they must still run on the password the
   *user* chooses at `/reset-password` — and confirm they do.
10. **Does U01's audit event change?** It currently records a creation that
    includes a credential. If creation no longer writes one, say whether
    `user.create` / `user.create_warned_hibp`'s closed result branch still
    makes sense, and whether `user.create_warned_hibp` becomes unreachable.
11. **Two commands or one?** Creating the user and issuing the link are two
    Class-A commands (U01 and U37). Say whether they should run in one
    transaction, in sequence with a failure mode, or as two separate operator
    actions — and what happens if the second fails after the first commits.
    **This is the question most likely to be got wrong.**

## 3. The three open questions — give your view, with reasons

These are `@nabbisen`'s to rule on. Your view is an input, not the decision.

12. **Creating a second administrator.** RFC 103 D5 refuses administrator
    targets on the web. So under this design, how is a second administrator
    created? Options: the web keeps a password field for that one case;
    administrator creation routes through `sui-id admin issue-recovery-link`;
    or D5 is relaxed for an account that has never held a credential. *The
    architect recommends the third: D5 exists to stop one administrator
    capturing another's live account, and an account that has never been used
    is not that.* Trace what D5 actually checks and say whether the relaxation
    is expressible without weakening the live-account case.
13. **The throttle against provisioning.** Five links per hour per issuer would
    cap bulk user creation at five an hour. *The architect recommends excluding
    issuance-at-creation from the counter.* Say whether that is expressible
    without giving an attacker a way to issue unthrottled links — for instance
    by creating a user in order to get an unthrottled issuance.
14. **`must_change`: enforce it or delete it.** *The architect recommends
    enforcing it*, since it keeps `sui-id setup`'s documented promise. Say what
    enforcing it costs: where the check goes, what a forced-change screen must
    not allow (skipping it, reaching any other page, completing OIDC), and
    whether the flag then needs an audit event of its own.

## 4. Anything else

15. **Any threat this RFC's design introduces or misses.** In particular: with
    no password at creation, an account exists that cannot be signed into until
    a link is handed over. Is a never-activated account a problem — for the
    admin list, for OIDC, for the concurrent-session cap, for anything that
    counts users?
16. **Anything in this RFC that cannot be built as described**, or that would
    be better built another way, including what it does not mention.

## What to return

A review-request package under `.git-exclude/review-requests/`, containing:
- the claim table for §1, one row per claim, with `file:line`;
- findings ranked blocker / high / medium / low;
- your answers to items 8–16, each citing what you read;
- your view on the three open questions, marked as a view.

Do not implement anything. This RFC is Proposed; implementation is not
authorized until it is Accepted, which this review enables.
