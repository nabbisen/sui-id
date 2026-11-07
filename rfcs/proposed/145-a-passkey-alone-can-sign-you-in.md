# RFC 145 — A passkey alone can sign you in

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** With `[security] passkey_sign_in_without_password` off, every sign-in behaves byte-for-byte as before (the login page, the passkey page, the claims), proven by tests that run with it off, and the only intended difference anywhere, the registration options' `residentKey: "preferred"` (§D8), is pinned by its own test; with it on, a passkey sign-in is one sealed Class-A command whose ceremony consumption, session, failed-sign-in counter reset, step-up stamp and `auth.login.success` row commit together or not at all; a ceremony completes only from the browser that started it; every refusal on the passkey path is the same response, whatever its cause; an account whose source is not `local` cannot complete it; an unauthenticated start writes nothing that outlives its expiry, and is bounded by the sign-in rate limiter; the session's `acr`/`amr` are the values §D6 settles, asserted end to end in an ID token; the login page offers the passkey button only when the switch is on, with its text in all three locales; and the passkey page tells the user, in plain words, that a passkey can sign them in by itself.
**Tracks.** Authentication. §Q carries four open questions for the owner, the last of them the feature freeze.
**Touches.** `Cargo.toml` (one `webauthn-rs` feature, and a direct `webauthn-rs-proto` dependency at the version `webauthn-rs` resolves), `crates/sui-id-core/src/authn/webauthn.rs`, `crates/sui-id-core/src/authn/session.rs`, `crates/sui-id-shared/src/auth_method.rs`, `crates/sui-id-store/src/commands.rs`, one migration, `crates/sui-id/src/http/handlers/admin/webauthn.rs`, `crates/sui-id/src/http/router.rs`, `crates/sui-id/static/` (one script), `crates/sui-id-web/src/pages/auth.rs`, the passkey page, `crates/sui-id-i18n`, `contracts/write-commands.toml`, `contracts/audit-coverage-matrix.md`, `docs/src/reference/configuration.md`, `docs/src/reference/audit-events.md`, `docs/src/reference/oidc-api.md`, `docs/src/guides/operators.md`.
**Independent design review.** [Design review record, 2026-11-07](../handoffs/145-a-passkey-alone-can-sign-you-in/design-review-2026-11-07.md). **By the architect, at `@nabbisen`'s request** — *"Review it carefully by yourself with my fundamental philosophy"* — `ROADMAP.md` §S1c's owner-invoked exception, and the record says he asked. **The architect wrote this RFC and is not independent of it**; the review re-measured every premise and ran the two that rested on reading the library, and its most serious finding is that the ceremony was not bound to the browser that started it.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/145-a-passkey-alone-can-sign-you-in/README.md`](../handoffs/145-a-passkey-alone-can-sign-you-in/README.md)

## Summary

Asked by `@nabbisen`, 2026-11-07, whether sui-id supports passkeys or plans
to. It supports them **only as a second factor**: every passkey sign-in
begins with a username and a correct password. This RFC lets a user sign in
with **a passkey alone**. They press one button, their device asks for their
fingerprint, face or screen-lock PIN, and they are in. There is nothing to
remember and nothing to type.

It is the strongest sign-in sui-id can offer, because a passkey cannot be
phished or guessed. For the users RFC 144 is written for, it is also the
simplest. **But it changes what a passkey already enrolled means**: from
"the second half of my sign-in" to "a key to my account". So it is behind
an operator switch that is **off** by default, and when it is on, the passkey
page says so in plain words.

**What it does not do.** It removes no password and creates no account
without one. Recovery (RFC 103) is untouched. A user who loses their passkey
is exactly where they are today: password, then an administrator's MFA
reset.

## Why this is an RFC

It adds a sign-in path that verifies one credential instead of two, and it
changes the claims relying parties receive. Those are design decisions
about the threat model and an external contract, not a repair.

## Measured, 2026-11-07

**Today's ceremony needs a known user.**
`crates/sui-id-core/src/authn/webauthn.rs:203` `start_authentication` takes a
`user_id` and offers only that user's credentials. The only callers come
after the password step (`/admin/login/webauthn/*`), during step-up
(`/me/security/step-up/webauthn/*`), and in the federated MFA continuation.

**The library can do usernameless sign-in, behind a preview feature, and
advises against the autofill form of it.** `webauthn-rs` 0.5.5 (pinned in
`Cargo.lock`) exposes `start_discoverable_authentication`,
`identify_discoverable_authentication` and `finish_discoverable_authentication`
only under feature `conditional-ui` (`webauthn-rs-0.5.5/src/lib.rs:1310`),
which belongs to its `preview-features` group (`Cargo.toml:61`).
`start_discoverable_authentication` **forces** `mediation = Conditional`,
the browser-autofill form. The crate's own documentation (`src/lib.rs`,
"Conditional UI / Username Autocompletion") says user testing found that
form *"hard to activate and may be confusing to users"*, and does not
recommend it. The finish step requires user verification
(`UserVerificationPolicy::Required`) and documents the result as trustworthy
*"as a MFA interaction"*.

**`mediation` is a public field.** `RequestChallengeResponse.mediation:
Option<Mediation>` (`webauthn-rs-proto-0.5.5/src/auth.rs:69`). Setting it to
`None` turns the same challenge into an ordinary **modal** request: the
browser shows its own passkey chooser, opened by a button. No library fork
is needed.

**Today's registrations discourage discoverable credentials.**
`start_passkey_registration` (`webauthn-rs-0.5.5/src/lib.rs:534`) sets
`require_resident_key(false)`, which `webauthn-rs-core` sends as
`residentKey: "discouraged"` (`core.rs`, the builder's `resident_key`
mapping). It does require user verification, and asks for
`credProtect: UserVerificationRequired`. Many platform authenticators make a
discoverable credential anyway; many security keys do not. **So some
already-enrolled passkeys will work with this RFC and some will not, and the
server cannot know in advance which.** `authenticator_selection.resident_key`
is also public (`webauthn-rs-proto-0.5.5/src/options.rs:314`).

**The user handle is `user_uuid`, which is not the OIDC `sub`.** Registration
passes `user.user_uuid` (`webauthn.rs:123`); ID tokens carry `sub =
UserId` (`crates/sui-id-core/src/oidc/tokens.rs:142`). Credentials are found
by `user_webauthn_credentials::find_by_credential_id`, whose column is
`UNIQUE` (migration `0004`).

**An unauthenticated ceremony needs a new pending kind.** `webauthn_pending`
allows a NULL `user_id`, but `kind` is CHECKed to `register`, `authenticate`
and `step_up` (migration `0013`).

**Claims today.** `crates/sui-id-shared/src/auth_method.rs`: a passkey maps to
`amr` `"hwk"`; `acr` is `"3"` when any method is phishing-resistant;
`"mfa"` is added only when two **distinct** `amr` tokens are present, and the
code comment says *"WebAuthn alone … does not earn `mfa`"*. Under today's
code, a passkey-alone session would get `acr "3"` and `amr ["hwk"]`. The
documentation on `acr "3"` describes multi-factor, so a reading that
contradicts itself is already one line away (§D6).

**Who signs in where.** `try_login_with_cascade`
(`handlers/admin/auth.rs:147`) sends `source = ldap` accounts to the directory
on every sign-in. Federation provisioning writes `source = ldap` rows too
(the comment at `:243`). The local password path serves `source = local`
accounts only.

## Decision

### D1 — one button, a modal chooser, no username

The sign-in page shows **"Sign in with passkey"** below the password form,
separated by "or", with one helper line under it in plain words: *"Uses
your fingerprint, face or screen lock. No password needed."* Many of the
users this helps most have never heard the word "passkey", so the line says
what will happen instead of naming the technology. Pressing it starts a discoverable ceremony with
`mediation = None`: the **browser's own** chooser lists the passkeys this
device holds for this site. The user picks one and verifies with their
device. The server learns who they are only from a valid assertion.

- **Not autofill** (conditional mediation). The library's authors advise
  against it on usability grounds. It is also invisible until a user happens
  to focus the username field, which is exactly the uncertainty this project
  avoids.
- **Not "type your username first"** (identifier-first). Answering a username
  with "this account has a passkey" tells a stranger which accounts exist
  and how they are protected. The modal ceremony asks the server nothing
  before the assertion, so there is nothing to enumerate.
- **The password form stays first.** Most users will not have a discoverable
  passkey, and a page whose first control fails for most of its readers is
  the wrong page. The newer design document
  (`sui-id-v0.64.2-uiux-handoff.md`, "Sign-in" screen) places it the same
  way. The button meets RFC 144's target size and contrast.
- **The block already exists, dormant.** RFC 034 added `render_login`'s
  `show_passkey_option` flag and the `login_passkey_primary` string. Every
  caller has passed `false` since 2026-06-05, so it never rendered. It sat
  *above* the form, posted to the second-factor route (which needs a
  pending-MFA cookie, so it could not have worked), and labelled its divider
  with a hardcoded English `"or"`. **This RFC reuses the flag and the
  string**: it moves the block below the form, points it at the new route,
  and replaces the literal with a locale key. A second, parallel button is
  not added.
- **When the browser offers nothing**, it shows its own "no passkeys"
  message, or the user cancels. The browser reports both as the same error,
  so the page shows one calm sentence that covers both: *"No passkey was
  used. You can try again, or sign in with your password."* It is not an
  error banner, because nothing went wrong.

### D2 — who may complete it: the accounts the local password path serves

A passkey-alone sign-in completes only for an account that is **active, not
deleted, and `source = local`**, the same accounts whose password is checked
locally. A directory or federated account is refused, because its authority
is consulted at **every** sign-in. A passkey would let it in after the
directory disabled it. This is RFC 103 T10/D13's rule ("an external account
never receives a local credential that bypasses its directory"), applied to
the new path. Federation provisioning writes `source = ldap` rows
(`crates/sui-id-store/src/repos/users.rs:975`), so this covers provisioned
federated accounts too. **A local account an administrator has linked to a
provider (`link_only`) keeps `source = local`**: the password path serves it,
and so does this one.

**Administrators are included.** A passkey with user verification is
phishing-resistant multi-factor. That is stronger than the password-and-TOTP
sign-in an administrator may use today, so excluding them would be
backwards. RFC 102 A7's audience rule applies unchanged: a non-reader bound
for `/admin` is refused before anything is written.

### D3 — every refusal looks the same

Unknown credential, a credential whose user handle does not match its row,
a rejected assertion, an expired or reused ceremony, an ineligible account
(D2), a disabled, deleted or locked account (§Q3), and the switch being
off: **one response**, *"Sign-in failed. Try again, or sign in with your
password."* Each cause is still recorded distinctly in the audit trail
(D7), where operators look. **The sentence is rendered, not implied**: a
refusal answers with the sign-in page, D3's sentence and status `401`, as
`login_post` does. The existing passkey second-factor refusal redirects to
the sign-in page with no message (`handlers/admin/webauthn.rs:96-114`), and
that is not copied. The redirect target (`next`) is validated exactly as the
password path validates it.

### D4 — the ceremony's state: single use, bounded, unauthenticated by necessity

Starting a ceremony is **an unauthenticated write**: there is no user yet.
It writes a `webauthn_pending` row of a new kind, `discover`, with a NULL
`user_id`. That needs a migration rebuilding the CHECK, as `0013` did, and
it falls under RFC 142's upgrade path.

- **It is bound to the browser that started it.** The start sets an
  HttpOnly, `SameSite=Lax`, path-scoped cookie holding the ceremony's id,
  and completion consumes the row only by that id. Start and completion
  both require the double-submit CSRF token, the binding the second-factor
  path already uses. The sign-in page issues no CSRF token today
  (`login_get`), so it issues one whenever the switch is on.
- The row is written by `U33`, the existing Class-P ceremony command.
  Its `owner` and `rationale` gain the new kind; no new command is needed.
- Expiry is **five minutes**. The existing purge removes it
  (`crates/sui-id/src/runtime/gc.rs:56`).
- The start route takes the **sign-in rate limiter**
  (`RateLimitKey::Login`), the same budget as a password attempt from that
  address.
- Completion **consumes the row inside the sign-in transaction** with a
  guarded delete. Zero rows means refuse (D3). A ceremony completes at most
  once, under concurrency too (the RFC 103 D13 pattern).
- **Stage 1 measures** whether the per-address limiter bounds the table under
  a many-address flood. If it does not, it adds a global ceiling on live
  `discover` rows that refuses new ceremonies (with D1's calm message)
  rather than growing without limit.

A sealed, stateless cookie was considered and rejected: it cannot be single
use, and this is a sign-in, not a confirmation of something already
approved.

### D5 — the write is one sealed Class-A command

A new sign-in command (`L09` in `contracts/write-commands.toml`, sealed,
Atomic). It re-reads the user inside the transaction (active, not deleted,
`source = local`, and the §Q3 lock rule), consumes the ceremony (the step-up
command's precedent, `webauthn_pending::consume_step_up_within_tx`), creates
the session (with L01's concurrent-session-cap eviction), resets the
failed-sign-in counter and stale lock as L01 does, stamps `last_step_up_at`
with method `Webauthn` exactly as L02 does for a passkey
(`crates/sui-id-store/src/commands.rs:3506`), and appends
`auth.login.success`, **or none of these**. It is the shape L01 and L02
already have. RFC 094's structural gate (G23) enforces the registration.

**The signature counter is `U29`, unchanged**: the existing Class-P write
the second-factor path performs before L02. It is checked as that path
checks it, and a synchronised passkey reports zero and is accepted, as
today. The same proof gives the same step-up freshness. A dangerous operation
straight after a passkey sign-in does not ask again, which matches today's
passkey second factor.

### D6 — what relying parties are told

Recommended (§Q1): **`acr "3"`, `amr ["hwk", "mfa"]`**. The session records a
distinct method, so a passkey-alone session is never confused with a
"password + passkey" one, in the audit trail or in a later reading.

The reasoning: every passkey ceremony here requires user verification, at
registration and at sign-in. The authenticator proves possession of the key
**and** that the person unlocked it with a PIN or biometric, and
`webauthn-rs` verifies that flag. That is two factors in RFC 8176's sense,
held in one device. Emitting `["hwk"]` alone would make a relying party that
requires MFA reject the strongest sign-in sui-id offers, and the user would
see a failure with no explanation at the relying party's end.

`auth_method.rs`'s comments on `acr "3"` and on `mfa` are rewritten to match,
and an `acr_amr` e2e test asserts the claims from a real ID token. The
public table relying parties read, `docs/src/reference/oidc-api.md`'s
sign-in path → `acr`/`amr` rows, gains the passkey-alone row.

### D7 — the audit trail, and what operators see

- **Success:** `auth.login.success` with the note `passkey`, so a query for
  "every sign-in" still finds it. The session's methods distinguish it.
- **Refusal:** `auth.login.failure` with a reason per D3 cause, for example
  `passkey: unknown credential`, `passkey: account not eligible`,
  `passkey: ceremony expired or used`. The target is the username when the
  credential identified one, and empty otherwise.
- `contracts/audit-coverage-matrix.md` gains the rows. G15 holds
  `docs/src/reference/audit-events.md` to them.
- **Failed passkey-alone assertions do not count toward the account
  lockout.** There is no knowledge factor being guessed: a signature cannot
  be brute-forced. Counting it would let anyone who learned a credential id
  lock the account, gaining nothing but the lock. The address rate limiter
  still applies, and every refusal is audited.

### D8 — the switch, and what users are told

- **`[security] passkey_sign_in_without_password = false`** in the config
  file, documented in
  `docs/src/reference/configuration.md`. **The config file, not a web
  setting**: no web path, a stolen administrator session included, can turn
  a second factor into a sole factor. The name is long on purpose. Passkeys
  already sign users in, as a second factor, and a shorter
  `passkey_sign_in = false` reads as if it disabled them. With it off, no
  sign-in path in this RFC is reachable: no button, and both routes give
  D3's response.
- **Registration asks for a discoverable credential when it can, whether
  or not the switch is on.** The creation options are sent with
  `residentKey: "preferred"`, set through the typed field
  (`webauthn_rs_proto::ResidentKeyRequirement`, a direct dependency, since
  `webauthn-rs` does not re-export it) after the library builds them. The
  library's server-side state still does not *require* one, so a security
  key that cannot store a discoverable credential still enrols as a second
  factor. **Regardless of the switch**, because discoverability grants
  nothing while the switch is off: the server never runs a discoverable
  ceremony. Making it conditional would mean every security key enrolled
  before an operator turns the switch on is never offered by the browser
  afterwards, which fails silently. **This is a deliberate edit to the
  library's output**, so a test pins the serialised options. The
  architect's prototype showed `"discouraged"` → `"preferred"` with
  `requireResidentKey: false` kept. Stage 1 confirms that verification
  accepts both outcomes.
- **The passkey page says what a passkey can do.** With the switch on, one
  sentence above the list: *"Any passkey on this list can sign you in by
  itself, without your password. Remove any passkey you do not
  recognise."* With it off, today's text, unchanged.
  **That sentence is the consent this RFC relies on**: an existing user's
  passkey changes meaning when the operator turns the switch on, and the
  user must be able to see that without reading release notes.
- **The operator guide** gets a section: what the switch does, what users
  will see, which accounts are eligible (D2), and that some existing passkeys
  will not be offered by the browser (*Measured*) and need re-registering to
  be.

### D9 — the script

One small hand-written script, `crates/sui-id/static/passkey-sign-in.js`, in
the style of the existing `webauthn.js`: the same encoding helpers, no
bundle, `script-src 'self'`. `README.md` says JavaScript is *"confined to
four small hand-written files"*, but `crates/sui-id/static/` already holds
five (`reset-password.js` is the one its list omits). This makes six, and
the README sentence is corrected to list them rather than count them.

**The script never shows a raw error.** The start route is fetched, and a
limiter refusal is rendered as HTML (`ErrorAs::Html`), which the script
cannot parse. A `429` shows *"Too many attempts. Wait a minute, then try
again."*. The sign-in limiter allows 10 per minute per address
(`crates/sui-id/src/runtime/ratelimit.rs:133`), which a household or office
behind one address can reach. Any other non-success shows D1's calm
sentence.

## Non-goals

- **Accounts with no password at all**, and passkey-first account
  activation. The recovery story for an account whose only credential is a
  device is a different design (RFC 103 D4 assumes a password exists). If
  wanted, it is its own RFC.
- **Autofill (conditional) passkey sign-in** (D1).
- **Attestation or authenticator allowlists.** No `attested_passkey` API, no
  AAGUID policy.

## Stages

1. **Prototype and measure, no product change.** The architect's review
   already showed the serialised request (`mediation` absent,
   `allowCredentials: []`, `userVerification: "required"`) and the creation
   options' `"preferred"`. What remains: a full register-then-finish round
   trip under `"preferred"`, discoverable and not, with a software
   authenticator if one is usable offline. Then measure D4's table bound.
   Report whether the existing `sui_id_signin_via_passkey_total` counter
   counts second-factor passkey sign-ins today, and **do not change its
   meaning**. Anything that overturns D1–D8 comes back here before stage 2.
2. **Store.** Migration (`discover` kind), the `L09` sealed command, its
   `write-commands.toml` row, the audit matrix rows. Tests that each
   refusal rolls back everything.
3. **Core and HTTP.** The two routes, D2's eligibility, D3's single
   response, D6's claims, the switch. An e2e test per D3 cause asserting the
   identical response **and** the distinct audit reason.
4. **Interface.** The button, the passkey page sentence, the three locales,
   the script, the docs (configuration, audit events, operator guide,
   README). A test, run with the switch off, that the login page and the
   passkey page are byte-identical to before.

**Real-browser confirmation** (Chrome, Firefox and Safari, a platform
passkey and a security key) cannot run in CI. It is a manual check recorded
at stage 4, on hardware the owner chooses. Nothing in it needs elevated
privileges.

## Security considerations

| # | Threat | Control |
|---|---|---|
| T1 | A stranger learns which accounts exist, or how they are protected | D1 (nothing asked before the assertion), D3 |
| T2 | A directory-disabled account signs in with a local passkey | D2 |
| T3 | A stranger locks an account through the passkey path | D7 (not counted) |
| T4 | A captured assertion or ceremony is replayed | D4 (single use, consumed in the transaction), the challenge itself |
| T5 | Unauthenticated starts fill the database | D4 (rate limit, expiry, measured ceiling) |
| T6 | A session exists without its audit row, or the reverse | D5 |
| T7 | A user's second factor silently becomes a sole factor | D8 (off by default; config file only; the passkey page says so) |
| T8 | A stolen administrator session turns the feature on | D8 (no web path) |
| T9 | A relying party misreads the assurance level | D6, §Q1 |
| T10 | A stolen, unlocked device signs in | Inherent to passkeys: the device's own verification is the control. Users remove a lost device's passkey from the passkey page, or an administrator resets MFA (RFC 103) |

## Findings recorded while scoping — not in scope

- **`"hwk"` for synchronised passkeys.** RFC 8176's `hwk` is a hardware-secured
  key. A passkey synchronised through a platform account is not bound to
  one device, and the authenticator's backup-eligible flag says which kind
  it is. Today's code emits `hwk` for both, and has since passkeys shipped.
  Whether to emit `swk` for a backup-eligible credential is a separate
  question about an existing claim. It is recorded here because a sole
  factor makes the claim matter more.

- **RFC 034 Part A never shipped.** RFC 034 (Implemented, v0.36.0) and its
  `CHANGELOG` entry describe a "Sign in with passkey" button above the
  password form. It has never rendered (D1). This RFC makes the button real.
  The historical records are left as they are, and this note is the
  correction.

## Open questions (§Q)

**Q1 — the claims (D6).** Recommended: `acr "3"`, `amr ["hwk","mfa"]`. The
alternative, `["hwk"]` alone, is defensible on the old comment's reading,
but makes MFA-requiring relying parties reject a sign-in stronger than one
they accept. This is an external contract, so it is your call.

**Q2 — the default of the switch (D8).** Recommended: **off**, so that no
upgrade changes what a user's enrolled passkey can do. Revisit after a
release has shipped with it on somewhere real.

**Q3 — a locked account.** The password lockout is set by failed password
guesses. Recommended: **a locked account is refused on this path too**,
because "locked" should mean one thing everywhere, and L01 already re-reads
"active and unlocked". A successful passkey sign-in then resets the counter
and the stale lock, as L01 does. The cost is RFC 118's known limitation: a
stranger who guesses passwords can lock the owner out of the passkey path
as well. The alternative (a passkey bypasses a guessing lock, because it is
not a guess) is sound security reasoning, but leaves two meanings of
"locked" for users and operators to keep straight.

**Q4 — the feature freeze.** `ROADMAP.md` admits four kinds of work during
the freeze: urgent security fixes, remediation-enabling refactors,
dependency and security maintenance, and repairs that keep approved gates
executable. **This RFC is none of them.** It adds routes, a config key and a
claim value. RFC 143's exception was argued on its adding no product
surface, and that argument does not transfer here. Extending a closed
exception list is not the architect's to infer. Recommended: **accept the
design whenever it is ready, and schedule the build only after RFC 094 M2b
closes (`0.88.0`)**, with your explicit exception at that point. M2b is
the work everything else waits on, and this should not displace it. Built
after RFC 144, the two together make one release about older users.
