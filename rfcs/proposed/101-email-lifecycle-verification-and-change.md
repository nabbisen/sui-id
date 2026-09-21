# RFC 101 — Email lifecycle: verification and change

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None. This RFC picks up a deferral recorded in migration
`0020_user_identity_invariants.sql` §2, which added `email_verified_at` and stated
it would stay NULL "for every user until an email-verification flow ships (a
future RFC)".
**Implementation prerequisites.** RFC 103 (administrator-issued account recovery,
Accepted 2026-09-17) Implemented — owner ruling 2026-09-16, see the §6.3 correction. RFC 094 M2a
foundation Implemented — every
mutation defined here is Class-A and must land on the sealed runner, not beside
it. RFC 094's `U11` inventory row is the placeholder for the change command.
**Closure prerequisites.** A user can verify an address, change it, and the OIDC
`email_verified` claim reflects reality; recovery cannot be redirected by session
compromise alone; every mutation here is atomic with its audit event.
**Tracks.** Account recovery integrity. Not a ROADMAP milestone item — new scope
approved by `@nabbisen` on 2026-09-09 while scoping RFC 094's Wave C.
**Touches.** `crates/sui-id-store/src/repos/users.rs`, a new verification-token
repo, `crates/sui-id-core/src/account/`, `crates/sui-id/src/http/handlers/`,
`crates/sui-id-core/src/oidc/authorize.rs`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Independent security and closure reviewer.** Role independence per RFC 000 — the
reviewer must not have authored, implemented, or previously approved this RFC;
vendor is not a criterion. Design review routes to the implementation role for
implementability; the threat judgments in §6 route to `@nabbisen` where no other
role can adjudicate them.

## Summary

A user's email address is set once, at account creation, and can never change.
Nothing verifies it. It is simultaneously the password-recovery channel and the
source of the OIDC `email_verified` claim.

This RFC defines the missing lifecycle: **verify**, **change**, **re-verify**.

## Why this is a security RFC and not a feature request

Three consequences follow from the current state, and the third is the one that
makes this security-sensitive rather than merely incomplete.

**1. Recovery can be permanently misdirected and never corrected.** A typo at
creation, or a user leaving the domain that hosts their address, leaves password
recovery pointing somewhere the user does not control. No self-service path and
no admin path can fix it — `users::update_email` exists in the repository layer
with zero production callers.

**2. An address the account never proved it owns is a recovery channel.** Nothing
has ever set `email_verified_at`; verified at time of writing across the whole
workspace. The address is whatever was typed at creation.

**3. Address reuse transfers recovery.** Corporate domains change hands and
consumer providers recycle addresses. Whoever later controls a stale address
inherits the ability to request and complete a password reset for that account.
This needs no compromise of sui-id at all.

The OIDC consequence is real but secondary: `email_verified` is emitted `false`
for every user, so a relying party that requires a verified email rejects every
sui-id identity. Migration 0020 chose that deliberately — reporting `false`
honestly rather than omitting the claim — and it was the right call. This RFC is
what makes the claim able to be `true`.

## Design

### D1 — Verification is proof of control, and nothing else counts

`email_verified_at` is set only by consuming a single-use token delivered **to the
address being verified**. It is never set by an admin, never set at creation,
never inferred.

An admin can *set* an address (as today) but cannot mark it verified. Separating
who may change an address from what proves it is the whole point: an operator
typo must not produce a verified address.

### D2 — A change is a new address in a pending state, not a mutation of the live one

Changing an address must not replace the live one until the new address is
proven. Until then the account keeps its current address, and recovery keeps
working through it.

The pending change carries: the target address, a single-use token hashed at rest,
an expiry, and the initiating actor.

### D3 — Both addresses are told

Confirmation goes to the **new** address; a notification goes to the **old** one.
The new-address confirmation is what authorises the change. The old-address
notice is what lets a legitimate owner notice a change they did not initiate,
which is the only defence against an attacker who already holds a live session.

### D4 — Session compromise must not be sufficient

An attacker with a valid session should not be able to move the recovery channel
to an address they control. Confirmation at the new address raises the bar to
"also controls the target address", and the old-address notice gives the owner a
window to react.

**Settled 2026-09-10 (owner).** An email change is a **dangerous action under
RFC 058**, which defines the category as *"actions that meaningfully reduce the
security or availability of a principal"*. Moving the recovery address does
exactly that — and arguably more than anything in RFC 058's enumerated list:
disabling MFA weakens a control, while moving the recovery address converts
temporary session access into permanent account control.

**RFC 058 attaches three obligations to that category, not one.** All three apply
here:

1. **A confirm screen** explaining what happens and what is reversible
   (RFC 030 / RFC 040). For this action the screen must say which address the
   confirmation will be sent to and that the current address remains in effect
   until it is confirmed — the reversibility fact a user needs to judge the
   change.
2. **Step-up immediately before the action** (RFC 020 / RFC 021):
   `require_fresh_step_up`, within `STEP_UP_FRESHNESS_SECS` (300s). The same gate
   already guarding MFA disable and passkey removal, whose call site gives the
   reason verbatim: *"post-compromise attacker moves. Step-up is required."*
3. **An audit row with `note` populated** (RFC 045 pattern, RFC 060 rollout). This
   constrains §6.2 below: the events this RFC defines are not free to carry an
   empty note, and the note must be designed rather than left to the
   implementation.

*Correction, 2026-09-10.* The first version of this settlement specified only
obligation 2. It was reached by reading the code — the `require_fresh_step_up`
call sites and their `// RFC 058:` comments — rather than RFC 058 itself, so it
inherited exactly what the code makes visible at a call site and missed the two
obligations that are not visible there. The owner caught the method before the
gap. Answering a design question from the code ratifies whatever the code happens
to implement; the specification is what says whether that is the whole
requirement.

### D5 — Every mutation here is Class-A under RFC 094

Setting a pending change, consuming a verification token, and changing the live
address are durable security mutations. Each commits atomically with its audit
event on RFC 094's Class-A runner. RFC 094's `U11` row is the placeholder for the
change command; verification and pending-change consumption need rows of their own,
added to `command-inventory.md` when this RFC is accepted.

Event names follow the existing convention — `user.*` for administrative action on
a user, `auth.*` for authentication-flow events. Exact names are §6.2.

### D6 — Tokens follow the password-reset precedent

Single-use, expiring, hashed at rest, invalidated on consumption, and invalidated
when a competing change is initiated. `password_reset_tokens` is the existing
shape and there is no reason to invent a second one.

### D7 — An email change invalidates outstanding password-reset tokens

*Required by [RFC 103](../accepted/103-administrator-issued-account-recovery.md)
D3, recorded here 2026-09-17 because the email-change command (inventory `U11`)
arrives with this RFC and not with RFC 103.*

The command that changes an address revokes every outstanding
`password_reset_tokens` row for that user, in its own transaction. A reset link
mailed to the **old** address must not outlive the address: after a change, that
mailbox may belong to someone else, which is the reuse threat §2 exists to close.
RFC 103's stage 3 adds the same invalidation to U09, U10, U02 and U04, and marks
this one as this RFC's.

## Security considerations

**Recovery during a pending change.** The live address stays authoritative until
confirmation, so a pending change never weakens recovery — it cannot be used to
strand a user.

**Verification does not expire here.** Once verified, an address stays verified
until it changes. Periodic re-verification is out of scope; if it is ever wanted
it is its own RFC.

**Rate limiting.** Verification and change requests are email-sending endpoints
reachable by an authenticated user, and are abusable as a spam relay against a
third party. They need the same throttling the forgot-password flow already has.

**Enumeration.** Requesting a change to an address already in use must not reveal
that it is in use. The forgot-password flow already establishes the pattern of
responding identically regardless.

## Open questions

**§6.1 — Does an email change require re-authentication? Settled 2026-09-10
(owner).** Yes: an email change is a dangerous action under RFC 058, carrying all
three of its obligations — confirm screen, step-up immediately before, and an
audit row with `note` populated. The ruling and its reasoning are recorded under
D4 above. *This entry still read "`@nabbisen`'s call" until 2026-09-16; the
decision had been recorded in D4 but not here.*

**§6.2 — Exact event names and their note content.** Now constrained by RFC 058's
third obligation: each event here must carry a populated `note`, so the note's
content is part of this question rather than an implementation detail. Proposed: `user.email_change_requested`,
`user.email_changed`, `auth.email.verified`. The first two are administrative-shape
events on a user; the third is an authentication-flow proof. Wants a second reader
against the namespace split recorded in `audit-coverage-matrix.md`.

**§6.3 — Admin-initiated change. Settled 2026-09-10 (owner).** Permitted, and it
grants an admin no capability they lack.

An admin can already set any user's password directly —
`reset_user_password(…, new_password: &str)` takes the value — so full account
takeover by an admin is available today in one step. "Admin redirects recovery,
then resets the password" is strictly weaker than that. Refusing admin-initiated
change would therefore protect nothing.

**But it is not the recovery mechanism, and should not be described as one.** The
scenario that motivates this RFC — a user who has lost *both* password and email
access — is already solved by admin password reset, which involves no email at
all. An admin-set address is **unverified** (D1, without exception), so it does
not restore a recovery channel; it records a corrected address that the user then
proves.

> **Correction, 2026-09-16 (architect).** The premise above is false in
> production. `reset_user_password` exists in `sui-id-core` and as a Class-A store
> command, but **no route and no CLI subcommand reaches it** (RFC 098 dispatch 14).
> An admin cannot set another user's password today. The §6.3 conclusion still
> holds, for a different reason: an admin-set address is unverified, and the
> §6.4/§6.5 ruling lets only a verified address receive a reset. The ruling's
> "nobody is stranded" rests on admin password reset **being reachable**. Until
> it is, a user with no verified address and a forgotten password has no way back
> in. **Ruled 2026-09-16 (`@nabbisen`): build it**, with the web version's security
> risk addressed. It is designed as
> [RFC 103](../accepted/103-administrator-issued-account-recovery.md): an administrator issues a
> single-use recovery link and never sets or sees a password. RFC 103 is an
> implementation prerequisite of this RFC.

**A support path is structurally guaranteed to exist.** RFC 094's `U05` last-admin
guard rejects any demotion that would leave zero admins, in-transaction and proven
under concurrency. So "no one to manage users" is not a reachable state, and the
recovery-of-last-resort question does not arise.

**§6.5 — Must password reset require a *verified* address? Settled 2026-09-16
(owner) — see the ruling after §6.4.** Raised 2026-09-10 by
the §6.1/§6.3 answers, not present in the original draft. §2's third consequence —
address reuse transferring recovery — is only fully closed if recovery requires a
proven address. Requiring it strands every existing user, since none is verified;
not requiring it leaves the motivating threat open for anyone who never verifies.
This is §6.4's question and this one meeting: whatever is decided for the existing
population determines whether this can be required, and when. **Coupled, and both
are the owner's.**

**§6.4 — Existing users. Settled 2026-09-16 (owner) — see the ruling below.** Every
current address is unverified. Whether existing users are prompted, required, or
left alone until they change something.

**Ruling on §6.4 and §6.5 together — settled 2026-09-16 (owner).** `@nabbisen`
accepted the architect's proposal, under the standing principle of a finally clean,
safe and secure, robust and sophisticated design:

- **§6.5 — yes, from the day verification ships, with no transition period.** Password
  reset sends a link only to a verified address. A transition period was rejected: it
  is the motivating threat left open with a deadline someone must remember to end,
  and ending it would need a second migration and review.
- **§6.4 — existing users are prompted, never blocked.** Every existing address starts
  unverified. A signed-in user is asked to verify and can do so at any time; sign-in
  never depends on it. Blocking sign-in was rejected: a lockout pushes operators
  toward workarounds that weaken the system.
- **Nobody is stranded.** A user who cannot sign in and has no verified address
  recovers through admin password reset (§6.3), and an admin always exists (RFC 094
  `U05` last-admin guard).
- **No enumeration.** The forgot-password response is identical whether the account
  exists, and whether its address is verified.

## Alternatives rejected

**Admin-settable verified flag.** Would let an operator typo produce a verified
address and defeat D1's entire purpose.

**Change without confirmation at the new address.** Makes session compromise
sufficient to move the recovery channel — the specific outcome D4 exists to
prevent.

**Bolting this onto RFC 094's Wave C.** The conversion waves convert existing
behaviour. This is new behaviour with its own threat model, and smuggling it into
a conversion would give it none of the review it needs.
