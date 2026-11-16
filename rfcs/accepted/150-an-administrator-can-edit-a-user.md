# RFC 150 — An administrator can edit a user

**Status.** Accepted
**Accepted on.** 2026-11-16
**Approved by.** `@nabbisen`, 2026-11-16: *"All accepted."*
**Security review.** Required
**Design prerequisites.** RFC 101 Accepted, for the email half (§3).
**Implementation prerequisites.** This RFC Accepted. Built in cycle P, as a
freeze exception decided 2026-11-15 (ROADMAP, cycle P's row). The email
half also needs RFC 101 implemented, so it waits for RFC 101's own build.
**Closure prerequisites.** An administrator can change a user's display name
and language from the user's own page. Each change is one Class-A command
whose audit record names the old and new value, and is tested with a
mutation. An email change goes through RFC 101's lifecycle, never a direct
write. A directory (LDAP) user's name and email are read-only, because the
directory owns them. An administrator can rename a user, with step-up and an audit record of the old and new name; an old or deleted user's name is reserved, released only by an administrator's own audited, step-up action on one name, and a test fails if any other path frees it.
**Tracks.** The operator-experience track, Track D: one of two features
given a freeze exception.
**Touches.** `crates/sui-id-store/src/commands.rs`,
`crates/sui-id-store/src/repos/users.rs`, `crates/sui-id-web/src/pages/users.rs`,
`crates/sui-id/src/http/handlers/admin/users.rs`, `crates/sui-id/src/http/router.rs`,
`crates/sui-id-i18n/src/locale/*.rs`.
**Independent design review.** [Design review record, 2026-11-16](../handoffs/150-an-administrator-can-edit-a-user/design-review-2026-11-16.md). By `@nabbisen`, who did not author it.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/150-an-administrator-can-edit-a-user/README.md`](../handoffs/150-an-administrator-can-edit-a-user/README.md)

## Summary

Raised by the owner, using the console on 2026-11-15: *"Unable to edit or add
users in user management."* Adding exists, but cannot be found (RFC 147 §1).
**Editing does not exist.** Measured on `ddcf4b0`, an administrator can
change a user's role, disable them, reset their MFA, issue a recovery link,
or delete them (`crates/sui-id/src/http/router.rs:159-195`). Nothing edits a
display name, a language or an email.

`users::update_email` exists (`crates/sui-id-store/src/repos/users.rs:577`),
but **only tests call it**, to seed data. It writes the address directly, with
no verification and no audit record. It must not become the production path.

## Decision

### 1 — the display name and the language: one command each

- **`admin.user.display_name.changed`**: a Class-A command on RFC 094's seam,
  in the shape of the existing settings commands (for example
  `change_idle_session_timeout`, `commands.rs:5400`). Its audit note records
  the old and new value. A display name is not a credential, so **no
  step-up** is required.
- **`admin.user.language.changed`**: the same shape, over the existing
  `users::set_preferred_lang` (`users.rs:137`). It becomes the only write
  path for an administrator.
- Both are edited **on the user's own page**, in one form with the current
  values filled in. Each changed field produces its own command and its own
  audit record, and an unchanged field produces none.

### 1a — a directory user's name and email belong to the directory

An LDAP user (RFC 005, `source = 'ldap'`) has its display name and email
**refreshed from the directory on every sign-in**: `upsert_ldap_shadow`
(`crates/sui-id-store/src/repos/users.rs:920`) *"refreshes `display_name`
and `email` if they have changed upstream"*. An administrator's edit there
would be silently overwritten at the user's next sign-in, so it would look
saved and then come undone.

**So for an LDAP user, both are read-only**, with one line: *"This person's
name and email come from your directory, and are updated each time they sign
in. Change them there."* The language stays editable, because the directory
does not supply it.
**Test.** The edit form offers no display-name or email field for an LDAP
user, and the command refuses one. **Mutation:** let the command accept it.

### 2 — the username: renamed, never reused

**Corrected 2026-11-16.** This section first said the username *"is what the
audit log shows as the actor of everything they did"*. **That was false**:
the audit log records and shows the actor's internal ID
(`crates/sui-id/src/http/handlers/admin/audit.rs:84`,
`crates/sui-id-web/src/pages/audit.rs:28`). The architect asserted it without
measuring it.

What is true:
- **Applications never see the username.** The OIDC `sub` is the user's ID
  (`crates/sui-id-core/src/oidc/tokens.rs:142`, `:174`), and no
  `preferred_username` claim is issued.
- **A name is never released.** `username` is `UNIQUE`
  (`0022_boolean_checks.sql:60`), and deletion only marks the row
  (`repos/users.rs:287-290`), so a deleted user's name stays taken for ever.
  RFC 115's review recorded that as why *"username reuse is not an
  impersonation path"*
  (`rfcs/handoffs/115-user-creation-without-a-password/design-review-2026-09-24.md`,
  finding 7). It is a **consequence** of keeping deleted rows, which keeps
  every audit record's actor resolvable. It is not a rule anyone set on its
  own.

**§5 decides it:** an administrator may rename a user, and every old or deleted
name stays reserved. The reservation is recorded in a table of retired names,
checked when a user is created or renamed.

### 3 — the email goes through RFC 101, and only through RFC 101

RFC 101 settled on 2026-09-10 that **an administrator may initiate a change**
(its §6.3). RFC 101 also requires that a change is a new address, pending
until it is verified (D2), and that both addresses are told (D3). So this RFC
adds **no email write of its own**. The user's page shows the current
address and its verification state, and offers *"Change email"*, which starts
RFC 101's administrator-initiated flow. Until RFC 101 is built, the page
shows the address read-only, with that flow named as coming.

**Test.** No production code path calls `users::update_email`. A test scans
for a call outside `tests/`, and fails on one. Mutate it by adding one.

### 4 — what it does not change

Role changes, disabling, MFA reset, recovery links and deletion keep their own
pages, confirmations and step-up as they are. The page's layout follows RFC
149, if that has been built by then.

## 5 — questions for the owner

1. **Should a username ever be changeable?** **Answered 2026-11-16: as proposed below**
   (`@nabbisen`: *"Proceed."*, in reply to the proposal). Asked 2026-11-16:
   *"Partially agreed. However, it brings limitation: Removed names can not
   be used in the future. Never once registered. Is it for security ?"*

   **Decided: an administrator may rename a user, and an old name stays
   reserved.** (First proposed by the architect.)
   - A rename is one Class-A command, `admin.user.username.changed`, with
     step-up, because it changes how a person signs in. Its audit record
     names the old and new name.
   - The renamed user signs in with the new name from then on. Sessions
     already open continue: they are bound to the ID, not the name.
   - **The old name stays reserved**, and so does every deleted user's name.
     No one else can ever be given a name someone once signed in with. That
     is the protection RFC 115 relied on, kept.
   - A **mistyped** name is then fixed by renaming the account, not by
     creating a second one, so the right name is never lost to a typo.
   - Not for a directory (LDAP) user, whose username comes from the
     directory (§1a).

   **The one thing it does not change: a removed name is still never reused.**
   That is the safety part, and it is narrower than it sounds. A reused name
   would **not** hand over the old account's credentials: a password belongs
   to the old row, not to the name. What it would hand over is **what people
   and tools attach to a name outside sui-id**: a ticket, an access list or a
   log search for *"alice"*, or an application that wrongly keys accounts on
   the name instead of `sub`. Those would then point at a different person,
   silently. sui-id itself keys on IDs, but an operator's habits and tools
   often key on names.

2. **What happens to a retired name?** **Decided 2026-11-16** (`@nabbisen`:
   *"Agreed."*, on the architect's option D): a retired name is **reserved by
   default, and an administrator may release one specific name
   deliberately.**
   - **Retired names**: a deleted user's name, held today by `username`'s
     `UNIQUE` across all rows; and a renamed user's old name, held by the
     table of retired names of §2.
   - **Release** is one Class-A command, `admin.username.released`, with
     step-up, for one name at a time. Its confirmation page states who last
     held the name (their ID), when it was retired, and plainly that
     *anything outside sui-id that still refers to this name will then refer
     to the next user given it*. The audit record names the released name and
     the retired holder's ID.
   - **Never automatic.** No time limit releases a name, and nothing else
     does.
   - **Why not reuse by default.** Measured on `e27895d`: sign-in finds the
     account by username (`crates/sui-id-core/src/authn/session.rs:157`), and
     a wrong password counts toward **that** row's lockout
     (`crates/sui-id-store/src/repos/users.rs:349`). A reused name lets a
     former user's old login, or a password manager still holding it, lock
     the new holder out. And references by name outside sui-id would point
     at someone else, silently.
   - **Rejected:** reserving names for ever with no release (a departed
     person's name could never be given again); freeing them at once (both
     harms above, unannounced); freeing them after a waiting period (the
     same harms, later and unannounced); deleting the row (it breaks audit
     history).
   - Not for a directory (LDAP) user.

## Alternatives rejected

- **Make `users::update_email` the admin's email edit.** Rejected. It writes
  an address no one has proved they control, with no record. That is the
  recovery-misdirection RFC 101 exists to prevent.
- **One "save user" command for every field.** Rejected. Each change gets its
  own audit record, so the log says exactly what changed. A combined command
  would record changes that were not made, or hide which one was.
