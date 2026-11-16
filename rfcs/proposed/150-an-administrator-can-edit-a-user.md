# RFC 150 — An administrator can edit a user

**Status.** Proposed
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
directory owns them. The username cannot be changed, and the page says why.
**Tracks.** The operator-experience track, Track D: one of two features
given a freeze exception.
**Touches.** `crates/sui-id-store/src/commands.rs`,
`crates/sui-id-store/src/repos/users.rs`, `crates/sui-id-web/src/pages/users.rs`,
`crates/sui-id/src/http/handlers/admin/users.rs`, `crates/sui-id/src/http/router.rs`,
`crates/sui-id-i18n/src/locale/*.rs`.
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

### 2 — the username is not editable, and the page says so

The username is how the user signs in, and it is what the audit log shows as
the actor of everything they did. Changing it would make that history name
someone who no longer signs in under that name. The page shows it read-only,
with one line: *"The username is how this person signs in, and cannot be
changed. To use a different one, create a new user."* Whether a username
should ever be changeable is §5's question.

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

1. **Should a username ever be changeable?** The architect recommends **no**,
   for the reason in §2. A mistyped username is fixed by creating the right
   user and disabling the wrong one, which keeps the history true.

## Alternatives rejected

- **Make `users::update_email` the admin's email edit.** Rejected. It writes
  an address no one has proved they control, with no record. That is the
  recovery-misdirection RFC 101 exists to prevent.
- **One "save user" command for every field.** Rejected. Each change gets its
  own audit record, so the log says exactly what changed. A combined command
  would record changes that were not made, or hide which one was.
