# `clients::create` drops `consent_policy` — the persistence bug

**Dispatched 2026-10-02** on `@nabbisen`'s *"Agreed. User consent should be
respected and carefully treated."* **This document is a dispatch. Start.**
**Found by.** The implementation role, while implementing C15, and flagged
correctly as out of that dispatch's scope.
**Why it is separate from M2b.** It affects **every** caller of
`clients::create`, admin and dynamic alike — not only C15's path.

## The bug

`clients::create`'s `INSERT` names eleven columns. **`consent_policy` is not one
of them**, so the column takes its migration default `'none'`
(`0025_consent.sql`). `dynamic_register.rs:198` sets
`consent_policy: ConsentPolicy::FirstTime` in the row it passes, and its own
header calls `first_time_only` *"the sensible default for third-party"*. **The
value is silently discarded.**

At the authorize endpoint (`handlers/oidc.rs:159`), `ConsentPolicy::None => false`
means `needs_consent` is false: **the consent screen is never shown.**

## The bound, measured — and it is why this is not an emergency

An earlier characterisation of this (the architect's, to `@nabbisen`) said a user
*"signs in and is redirected back to a third-party client, never having been
asked."* **That overstated it by omission, and the omission matters:**

- RFC 008 **P4** requires that *"every dynamically-registered client starts
  disabled pending admin enable."* **That control works** —
  `dynamic_register.rs:195` sets `is_disabled: true`, and `is_disabled` **is** in
  `create`'s `INSERT`.
- `authorize.rs` refuses a disabled client at `:80`, `:142` and `:346`, before the
  consent gate is reached.

**So the gap cannot bite until an administrator has explicitly enabled the
client.** The accurate statement is: *after an administrator enables a dynamically
registered client, its users are never asked for consent, and nothing tells the
administrator that.*

**No incident, and none was possible:** no deployments exist (crates.io held only
0.77.0 until 2026-10-02, 11–29 downloads per version, and `@nabbisen` confirms no
production use), and registration requires an administrator-issued token.

## What to build

**1. Persist what the caller passes.** `create`/`create_within_tx` write
`consent_policy` from the row rather than leaving it to the column default. This
is a bug fix: the struct field exists, the caller sets it, and the write drops it.

**2. The four application-identity URIs are the same bug.** `logo_uri`,
`homepage_uri`, `privacy_policy_uri`, `tos_uri` are dropped identically. Fix them
in the same change — they are one defect with five columns, and splitting it would
leave four of them broken with nobody looking.

**3. A test that fails first.** Create a client with a non-default
`consent_policy` and non-empty URIs, read it back, and assert every field
round-trips. **Run it against the current code and watch it fail** before trusting
it — as you did for the `rusqlite` clause and C15.

**4. End-to-end, for the behaviour that matters:** a dynamically registered client,
once enabled, **shows the consent screen on first authorization**. That is the
property, not the column value.

## The design question this does *not* decide

`create`'s doc comment says it *"does not set `consent_policy`, `registered_via`,
or the application-identity URIs"*, pointing at the separately-audited writers
`update_consent_policy` (C09) and `update_app_identity` (C10). **Do not remove
those setters or route creation through them.** Setting a value *at creation* is
not an update, and creation is already audited by its own command; C09 and C10
exist so a *later change* to policy is separately visible to an operator. Fixing
the `INSERT` does not disturb that.

**What is genuinely open, and is `@nabbisen`'s, not yours or mine:** no RFC states
that dynamically registered clients should get `first_time_only`. The intent lives
only in `dynamic_register.rs`'s header and line 198. RFC 008's third-party posture
clearly supports it and RFC 038 established the column, but **neither says it.**

So this dispatch fixes the **persistence bug** — the code failing to do what it
says — and does **not** settle whether `first_time_only` is the right policy, nor
whether an administrator enabling a dynamic client should be shown and asked to
confirm its consent policy. **If your work suggests either, report it; do not
decide it.**

Per RFC 127, once the policy is settled it belongs in `docs/`, not only in a
header comment — but that follows the ruling, not this fix.

## Protocol

Hand over a working tree; do not commit, do not push. State the parent commit as
the baseline. Declare every hunk's hash. Gates through
`scripts/ci-gate.sh <GATE_ID>` — this touches `crates/`, so the full Rust matrix
applies. Measure any number or line reference handed to you rather than applying
it.

**On the `CHANGELOG`:** this is a user-visible security-relevant fix, so it earns
an `[Unreleased]` entry. Describe it as the persistence bug and the consent
consequence, and **state the bound** — that a dynamically registered client starts
disabled, so an administrator had to enable it first. An entry that omits the bound
would overstate the exposure, which is the error this dispatch had to correct in
its own framing.
