# Creating a user without choosing their password

**Status.** **Proposed by the architect, 2026-09-22. Not authorized.** Awaiting
`@nabbisen`. Nothing here is an owner decision; it is written down so the
defect it describes is not lost while the decision is pending.
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
