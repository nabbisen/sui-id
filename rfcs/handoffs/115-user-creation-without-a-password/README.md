# Creating a user without choosing their password

**Governing RFC.** [RFC 115](../../accepted/115-user-creation-without-a-password.md),
**Accepted 2026-09-24**, after an independent design review that returned a
blocker. The RFC is the authority; where this page and the RFC disagree, the
RFC wins and this page is corrected.
**Status.** **Dispatched 2026-09-24.** Release cycle A, item 1, window to
2026-10-10. Stages are below; the sections that follow them are the original
package text, kept as the record of how the defect was found.
**Implementer.** Mid-capability model.
**Found by.** The T2 re-review of 2026-09-22, tracing every production writer of
the `credentials` table rather than re-reading RFC 103's checklist.
**Bears on.** [RFC 103](../../accepted/103-administrator-issued-account-recovery.md)
closure prerequisite 4, threat T2, and `docs/threat-model.md`'s RFC 103 entry.

## The defect

RFC 103 removed U06, the path that let an administrator re-set an **existing**
user's password. It left untouched U01, the path that sets **every** local
user's first one.

`/admin/users/new` carries a required password field. `CreateUserForm.password`
→ `CreateUserSpec.password` → `hash_password` → `CredentialRow { must_change:
false }` → U01. So an administrator chooses the initial password of every local
account, knows it indefinitely, and nothing ever requires the holder to replace
it. Signing in with it later is an ordinary sign-in: no audit event
distinguishes the administrator from the user.

Therefore, as the tree stands:

- RFC 103 **closure prerequisite 4** — "no code path lets anyone other than the
  account holder choose or learn a password" — is **not met**;
- **T2** ("an administrator impersonates a user: sets a password they know") is
  narrowed to accounts the administrator created, which on a single-realm
  instance is all of them, not closed;
- `docs/threat-model.md` has been corrected to scope its claim to the recovery
  operation and to point here.

## Two dependent defects found with it

1. **`must_change` is a control that does not exist.** The column is written in
   three places and cleared on self-change, and **read by nothing** — no
   sign-in path consults it. `sui-id setup`'s own documentation
   (`crates/sui-id/src/cli.rs`, the `run_setup` doc comment) promises that a
   generated password is "stored with `must_change = true`", implying a
   rotation requirement the product does not implement.
2. **Every password-bearing HTTP form derives `Debug` over plaintext fields** —
   `SetupAdminForm`, `ResetPasswordForm`, `EmailSettingsForm` (the SMTP
   password), `LoginForm`, `PasswordChangeForm`, `PasskeyRegisterStartForm`,
   `MfaEnrollStartForm`, `CreateUserForm`. **None is logged today**; this is
   latent, not live. RFC 103 gave `RecoveryToken` a redacted `Debug` and a
   zeroizing `Drop`; the passwords that token exists to avoid handling get
   neither, and `CreateUserForm.password` is a plain `String` where the CLI's
   equivalent is `Zeroizing<String>`.

## The shape a fix would take

The store already permits it: `commands::create_user` takes `credential:
Option<CredentialRow>` and skips the write on `None`. A user can exist with no
credential today.

- `/admin/users/new` drops the password field. The administrator creates the
  account and lands on RFC 103's recovery-link issuance — same confirm screen,
  required reason, fresh step-up, throttle, handover procedure and audit event.
- One rule, no exceptions: nobody but the account holder ever chooses a
  password.

**Three questions the owner has to settle before this is buildable**, each a
real fork rather than a detail:

1. **Creating another administrator.** D5 refuses administrator targets on the
   web. Either the web keeps a password for that one case, or administrator
   creation routes through the CLI, or D5 is relaxed for an account that has
   never held a credential. The architect's view: relax it for the
   never-credentialed case — D5 exists to stop one administrator capturing
   another's live account, and an account that has never been used is not that.
2. **The throttle meets provisioning.** Five links per hour per issuer caps bulk
   user creation at five an hour. Either first-issuance-at-creation is excluded
   from the counter, or provisioning uses the CLI.
3. **`must_change`: enforce it or delete it.** Enforcing it (force a change at
   next sign-in) is the smaller change and keeps `sui-id setup`'s promise.
   Deleting it is cleaner if creation no longer sets passwords at all. Leaving
   it as it is, is not an option: a flag no code reads reads as a control in
   review, and did to this reviewer until it was checked.

## Evidence a package would have to carry

- A test that a user created through the web has **no credential row**, and that
  a sign-in attempt for them fails.
- A test that the created user sets their own password through the issued link
  and signs in with it.
- Whichever `must_change` branch is chosen: a test that a flagged account is
  forced to change at next sign-in, or a grep-proof that the column and every
  mention of it are gone.
- For the `Debug` derives: a redacted `Debug` per form, each with a test that
  the formatted output does not contain the password.

---

# Stages — dispatched 2026-09-24

**Read [RFC 115](../../accepted/115-user-creation-without-a-password.md) first**,
including its *Corrections* section: two of this page's original factual
premises were wrong, and the RFC records which. Then read the
[design review](design-review-2026-09-24.md); its findings are why the RFC has
twelve decisions instead of three.

**Baseline.** `368d278` or later. **Return** a review-request package per stage
under `.git-exclude/review-requests/`, in the form the RFC 102 and 103 stages
used: what was built, the evidence table, the mutations with their results, the
gates on the final tree, and per-hunk SHA-256 hashes against the stated
baseline.

| Stage | Decisions | Why this order |
|---|---|---|
| 1 | D1, D8 | Closes the hole and fixes the sign-in branch **before** the change that creates password-less accounts |
| 2 | D2, D3, D4, D9, D10 | Creation without a password, end to end. D10 lands with it or the web cannot create a second administrator at all |
| 3 | D11, D5, D12 | The throttle column and migration, the secret-redaction sweep, and removing `must_change` |

## Stage 1 — close the hole, fix the branch

**D1.** `request_reset` treats a local user with **no credential row** exactly
as it already treats a non-local one: the same neutral response, the same
Class-B event with no user, no token, no mail. The block RFC 103 D13 added is
the seam; put it beside that, not in a new place.

> **The response must be byte-identical to the existing neutral one.** If it
> differs by a character, a status code, a header or a timing, `/forgot-password`
> becomes an oracle for "this account has never been activated", which is worse
> than the hole being closed. Test that equality directly, not by inspection.

**D8.** `login_with_mfa` returns `InvalidCredentials` before `verify_password`
when `credentials::get` is `NotFound`, so it skips the dummy Argon2 verify that
every other refusal runs for timing equivalence, and no `record_login_failure`
runs — no `auth.login.failure`, no counter, no lockout. Run the dummy verify and
record the failure on that branch.

**Evidence.**
- A user with an email and no credential row gets **no mail** from
  `/forgot-password`, and the response equals the one a non-local user gets.
  Mutation: remove the check — caught.
- Sign-in for such a user: the dummy verify ran, `auth.login.failure` is
  written, the counter moves, and the response is identical to a wrong
  password. Mutation each way — caught.
- **This stage can be built and tested today**, because test fixtures can
  already create a user with no credential row (`r103_stage4.rs` does).

## Stage 2 — creation without a password

**D2.** `/admin/users/new` drops the password field. **D3.** Creation redirects
to `/admin/users/{id}/recovery-link-confirm`; two commands in sequence, U01 then
U37, never one transaction — the RFC says why. **D4.** Remove
`CreateUserSpec.password`, `min_password_len`, the HIBP call at creation, and
`commands::create_user`'s `credential` parameter, so a password at creation is a
**compile error**. **D9.** `user.create_warned_hibp` becomes unreachable and is
retired on the U06 precedent, and U10 records the `warn`-mode outcome that is
currently discarded. **D10.** RFC 103's D5 refuses an administrator target only
when it **has** a credential row: `NOT EXISTS credentials AND last_login_at IS
NULL`, read inside U37's transaction on the same fresh read D5 already performs.
`can_issue_recovery` changes in lockstep or the button stays hidden.

**Evidence.**
- A user created on the web has **no credential row**; a sign-in attempt for
  them fails; they activate through the issued link and sign in with the
  password **they** chose.
- The password policy and HIBP still run where the user chooses the password —
  they already do, at `forgot_password.rs`; prove it rather than assume it.
- A second administrator can be created and activated on the web. Mutation:
  drop the `last_login_at` clause — the test that a *live* administrator is
  still refused must catch it.
- A structural test enumerating writers of `credentials` against an allowlist,
  so a fifth writer fails CI rather than review.
- The grep-proof for `user.create_warned_hibp`, and G13's count updated.
- **Around thirty call sites break** — core `step_up.rs` tests, store
  `runner/user_admin.rs`, six e2e files, and `--dev` seeding. Tests get a
  store-side constructor compiled only for tests, the precedent being
  `sessions::insert`. **`--dev` is a runtime flag in the production binary and
  cannot use a cfg-gated helper — it is the named exception (D6); say so in the
  package rather than working around it.**
- **Do not weaken a test to make it pass.** If a test can only pass by
  asserting less, say so in the package and leave it failing for the review.

## Stage 3 — throttle, secrets, `must_change`

**D11.** The throttle exemption: a column, a migration on the 0041 precedent
(`ALTER TABLE … ADD COLUMN`, since `issued_via` cannot gain a value without a
table rebuild), and the predicate evaluated **inside U37's transaction** — local,
no credential row, no earlier token of any kind. **Administrator targets are
not exempt.** Provisioning gets its own larger ceiling rather than being free.

**D5.** Every form field holding a password, client secret, token, TOTP or
recovery code becomes `secrecy::SecretString` — already a workspace dependency.
The four dead DTOs in `crates/sui-id-shared/src/api.rs` are deleted. One test
per form asserts the formatted output contains no secret.

**D12.** `must_change` is deleted: the column, `CredentialRow.must_change`, the
three function parameters, one test, and one migration. `cli.rs`'s doc comment
is corrected to say what is true.

**Evidence.**
- The exemption predicate: a live account can **never** qualify — that is the
  property to test and to mutate. Creating a user to obtain an unthrottled
  issuance yields a link only to the account just created.
- An administrator target still counts against the normal five.
- Per-form `Debug` redaction, one test each.
- A grep-proof that `must_change` is gone, and that `DROP COLUMN` works on the
  pinned SQLite — **verify that before writing the migration**, not after.

## Standing instructions for all three stages

1. **Report, do not fix, anything outside the stage.** RFC 116's review found
   twelve false rows in `ci/audit-coverage-matrix.md`; they are corrected
   already, but if you find another, name it and leave it.
2. **Say what you could not do.** Every stage of RFCs 102 and 103 that
   disclosed a limit was accepted on that disclosure; none was penalised for it.
3. **If a decision in the RFC turns out to be wrong when you build it, stop and
   say so.** Two architect rulings have already been corrected by measurement
   in this programme, and both corrections were right.
