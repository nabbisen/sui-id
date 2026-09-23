# RFC 115 — Creating a user without choosing their password

**Status.** Proposed
**Security review.** Required
**Independent design review.** [Design review 2026-09-24](../handoffs/115-user-creation-without-a-password/design-review-2026-09-24.md) by the implementation role, which authored neither this RFC nor its handoff. One blocker, two high, six medium, three low; all resolved in this text. It also corrected two of this RFC's factual premises (§Corrections).
**Design prerequisites.** Three forks are open and belong to `@nabbisen`, stated in *Open questions*: creating a second administrator against D5's refusal of administrator targets; the five-per-hour throttle against bulk provisioning; and whether `must_change` is enforced or deleted. The design review gives a measured view on each.
**Implementation prerequisites.** RFC 103 Implemented — this reuses its recovery-link issuance as the replacement for the password field.
**Closure prerequisites.** No path lets an administrator **set** a user's password or **learn** one the user has chosen; the only route by which an administrator can reach a new account's password is issuing a recovery link through U37 — audited, fresh-step-up, second-factor-gated and throttled — and that an issuer can complete the link they issued is a stated residual, not a defect (D7, and RFC 103's threat-model entry). A created account cannot be activated by any unaudited path, including `/forgot-password` (D1). `must_change` is enforced or gone. No form's `Debug` can print a password or any other secret. `--dev` seeding is a named exception (D6).
**Tracks.** Account integrity. Found by the T2 re-review of 2026-09-22.
**Touches.** `crates/sui-id-core/src/identity/admin/users.rs`, `crates/sui-id-core/src/account/forgot_password.rs`, `crates/sui-id-core/src/authn/session.rs`, `crates/sui-id/src/http/handlers/admin/users.rs`, `crates/sui-id-web/src/pages/users.rs`, `crates/sui-id-store/src/commands.rs`, `crates/sui-id-store/src/registry.rs`, `crates/sui-id/src/http/handlers/me_security/forms.rs`, `crates/sui-id-shared/src/api.rs`, `ci/audit-coverage-matrix.md`, `ci/write-commands.toml`, `docs/src/reference/audit-events.md`, `docs/threat-model.md`. A migration if *Open question 2* is answered with a column.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/115-user-creation-without-a-password/README.md`](../handoffs/115-user-creation-without-a-password/README.md)

## Summary

RFC 103 removed U06, the path that let an administrator re-set an existing
user's password, and left untouched U01, which sets every local user's first
one. `/admin/users/new` carries a required password field, stored with
`must_change: false`, so an administrator knows a working password for every
account they created until its holder changes it, and using it later is an
ordinary sign-in. RFC 103's closure prerequisite 4 and threat T2 are therefore
not met.

The design review found that **removing the password field is not sufficient**:
an administrator also types the new user's **email address**, and nothing in
the forgot-password path requires that address to be verified. So an
administrator could create a password-less account with an address they
control, use the unauthenticated `/forgot-password` form, receive the link and
choose the password — with no step-up, no second factor, no throttle and no
`user.recovery_link.issued` event. D1 closes that.

Two dependent defects remain as first stated: `must_change` is read by nothing,
and every password-bearing HTTP form derives `Debug` over its plaintext field.

## Corrections to this RFC's own premises

Recorded rather than quietly fixed, because the first version of this RFC was
sent to `@nabbisen` with both errors in it.

1. **"Seven production writers of `credentials`" was wrong.** There are **four**
   production write sites: `setup.rs:202` (raw upsert, reached by both the
   wizard and headless `sui-id setup`), and the `within_tx` upserts in U01, U09
   and U10. The two `me_security.rs` sites counted as production are inside
   `#[cfg(test)] mod tests` (the module opens at `me_security.rs:148`), as is
   the `step_up.rs` site. **The conclusion is unchanged**: there is no fifth
   writer, and U01 is the gap. Independently re-verified by the architect.
2. **The eight `Debug` derives are complete for passwords and incomplete for
   secrets.** The review adds four dead DTOs in `crates/sui-id-shared/src/api.rs`
   that derive `Debug` over passwords, and a further set of forms carrying
   client secrets, TOTP codes, recovery codes, setup tokens and the rotated
   client secret. D5 covers all of them.
3. **One caller omitted:** `--dev` seeding calls U01 with plaintext passwords
   (`crates/sui-id/src/runtime/dev_mode.rs:424`). D6 names it.

## Decisions

**D1 — `/forgot-password` refuses an account that has never had a password.**
`request_reset` treats a local user with **no credential row** exactly as it
already treats a non-local one: the same neutral response, the same Class-B
event with no user, no token and no mail. A never-activated account is then
reachable only through an administrator-issued link, which is audited,
step-up-gated, second-factor-gated and throttled. This is the blocker fix; it
costs one lookup and touches no account that has ever held a password. RFC
101's verified addresses are the durable fix and may supersede it; they do not
exist, so this does not wait for them.

**D2 — `/admin/users/new` drops the password field.** The administrator creates
the account and is redirected to the existing recovery-link issuance at
`/admin/users/{id}/recovery-link-confirm`, which already requires a fresh
step-up, a reason and a confirm marker.

**D3 — Creation and issuance are two commands in sequence, not one
transaction.** U01 then U37, joined by a redirect. A single command would have
to re-implement U37's D5/D6/D8 checks and its B4 evidence, and two copies of
those controls is how they drift. The failure mode is benign: if the second
fails after the first commits there is a user with no credential, no token, no
session and nothing that can sign in; the administrator retries from the user's
page. The audit trail is two events, `user.create` then
`user.recovery_link.issued`, each recording what it did.

**D4 — The password parameter is removed, not passed as `None`.**
`CreateUserSpec.password`, `min_password_len`, the HIBP call at creation, and
`commands::create_user`'s `credential` parameter all go, so that a password at
creation becomes a **compile error** rather than a review finding — the
technique RFC 102 stage 9 used for the raw step-up write. A structural test
enumerates the writers of `credentials` against an allowlist, so a fifth writer
fails CI rather than review.

**D5 — Secrets are redacted by their type, not by remembering.** Every form
field holding a password, client secret, token, TOTP or recovery code becomes
`secrecy::SecretString` — already a workspace dependency — whose `Debug` is
redacted by construction. The four dead DTOs in `crates/sui-id-shared/src/api.rs`
are deleted. One test per form asserts the formatted output contains no secret.

**D6 — `--dev` is a named exception.** Dev seeding runs in the production
binary under a runtime flag, so it cannot use a `#[cfg(test)]` helper, and it
prints its passwords to stderr by design. The closure prerequisite is scoped
"outside `--dev`", and the RFC says so rather than claiming a rule it does not
keep. Tests use a store-side constructor compiled only for tests, the precedent
being `sessions::insert`.

**D7 — The property this RFC delivers is stated exactly.** The issuing
administrator receives the plaintext token and can open `/reset-password`
themself. That is unavoidable while the administrator is the courier, and RFC
103's threat-model entry already states it as a residual. What this RFC buys is
real and is what the closure prerequisite now claims: no silent, indefinite
knowledge of a working password, and every administrator-side activation is one
audited, throttled, step-up-gated, second-factor-gated event. `docs/threat-model.md`
is updated so the residual covers every account an administrator creates, not
only ones that already existed.

**D8 — A never-activated account fails sign-in like any other refusal.**
`login_with_mfa` currently returns before `verify_password` when
`credentials::get` is `NotFound`, skipping the dummy Argon2 verify that every
other refusal runs for timing equivalence, and writing no `auth.login.failure`.
Both are fixed in the same change: the dummy verify runs and the failure is
recorded.

**D9 — `user.create_warned_hibp` becomes unreachable and is retired, but the
warn outcome is not lost.** HIBP still runs where the user chooses the password
(U10), and today a `warn`-mode hit there is silently discarded. U10 records it.
Retiring the event follows RFC 103's U06 precedent: descriptor, variant,
manifest row, matrix row, reference row and the pinned event list, with a
grep-proof.

## Open questions

`@nabbisen` rules on each. The design review's measured view is recorded
beside each as an input, not as the answer.

1. **Creating a second administrator.** D5 of RFC 103 refuses administrator
   targets on the web. Options: keep a password field for that one case; route
   administrator creation through `sui-id admin issue-recovery-link`; or relax
   D5 for an account that has never held a credential.
   *Review's view:* the third, with the predicate `NOT EXISTS credentials AND
   last_login_at IS NULL` — the `last_login_at` clause keeps the rule correct
   if a later RFC introduces passwordless local accounts, where "no row" would
   stop meaning "never activated". It notes option 2 works today with no change
   at all, and recommends against option 1, which reintroduces the defect.
2. **The throttle against provisioning.** Five links per hour per issuer would
   cap bulk creation at five an hour.
   *Review's view:* exempt issuance-at-creation, marked by a column and
   evaluated inside U37's transaction on a target that is local, has no
   credential row and has no earlier token of any kind — so an existing account
   can never qualify. It adds two conditions to the architect's version:
   **do not exempt administrator targets** (an unthrottled stream of new
   administrators is the persistence primitive a stolen session would want),
   and give provisioning its own larger ceiling rather than making it free.
3. **`must_change`: enforce it or delete it.** The architect recommended
   enforcing it as "the smaller change".
   *Review's view:* that pricing is wrong, and the RFC should not repeat it.
   A check at the shared session extractor is not enough: `/authorize` and
   `/metrics` read sessions directly, so a forced-change account would still
   complete OIDC authorisation. Enforcing it means defining behaviour for every
   raw session reader, forbidding step-up and factor enrolment during the
   forced state, and storing the flag on the session row. **Its view is to
   delete it**, since after this RFC the only writer of `true` is `sui-id setup`
   for the operator's own account; if it is enforced instead, it is its own
   package, not a clause of this one.

## Risks

- **D1 changes the behaviour of an unauthenticated endpoint.** The response
  must stay byte-identical to the existing neutral one, or it becomes an
  account-enumeration oracle. That is the first thing to test.
- **D4 breaks around thirty callers** across core, store and e2e tests, plus
  `--dev`. That is the point — they are the surface being removed — but the
  change is wide and its review should check that no test was made to pass by
  weakening it.
- **D9 changes G13's count** and touches a file whose other rows are, per RFC
  116's design review, already wrong in twelve places. Sequence so that the two
  do not edit the same rows in the same week.
