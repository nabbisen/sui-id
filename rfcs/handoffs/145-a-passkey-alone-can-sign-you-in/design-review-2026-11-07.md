# RFC 145 — design review, 2026-11-07

**Reviewer.** The architect, **at `@nabbisen`'s request**: *"RFC 145 is
almost accepted. Review it carefully by yourself with my fundamental
philosophy."* This is `ROADMAP.md` §S1c's owner-invoked exception, as for
RFC 143 and RFC 144. **The architect wrote this RFC and is not independent
of it.** So every premise below was re-measured rather than re-read, and the
two design points that rested only on reading the library were built and
run.

**Scope.** The RFC as committed in `1ec121c`. The philosophy has two halves:
clean, safe, secure, robust and sophisticated design, **and** an interface
and API that users cannot misunderstand. Each finding says which half it
serves.

**Verdict.** The design holds: no decision is reversed and no open question
changes. **Thirteen amendments**, made in the same change as this record.
The most serious ones: the ceremony was not bound to the browser that
started it (F2), and the RFC described the sign-in button as new when
dormant code for it has been in the tree since RFC 034 (F1).

## What was built and run

A throwaway crate outside the tree (in the session scratchpad, offline,
`webauthn-rs = "=0.5.5"` with `danger-allow-state-serialisation` and
`conditional-ui`, and `webauthn-rs-proto` 0.5) printed:

```
forced mediation = Some(Conditional)
request JSON keys = ["publicKey"]
allowCredentials = []
userVerification = "required"
discoverable state serialises: yes
registration authenticatorSelection before = {"requireResidentKey":false,"residentKey":"discouraged","userVerification":"required"}
registration authenticatorSelection after  = {"requireResidentKey":false,"residentKey":"preferred","userVerification":"required"}
```

So the two library premises hold:
- **D1's modal ceremony.** Setting `mediation = None` removes `mediation`
  from the JSON, which leaves a plain modal request. `allowCredentials` is
  empty and user verification is required.
- **D8's registration edit.** It is typed, but it needs a direct dependency
  (F7). The state the ceremony stores serialises under the feature the
  workspace already enables.

**Not run, because no authenticator was available:** a full
register-then-finish round trip under `"preferred"`. That stays in stage 1,
with a software authenticator if one can be used offline, and otherwise in
the stage 4 browser check.

## Premises re-measured and confirmed

- **User verification is enforced, not merely requested.**
  `webauthn-rs-core-0.5.5/src/core.rs:546-548` (registration) and `:855-857`
  (authentication) return `UserNotVerified` when the flag is absent under
  `Required`. `start_passkey_authentication` (the existing second-factor
  path) uses `Required` too. D6's two-factor argument stands, and it is
  equally true of today's second-factor sessions.
- **Federated provisioning writes `source = ldap`.**
  `crates/sui-id-store/src/repos/users.rs:975` (`upsert_shadow`, which
  federation provisioning calls). So D2's `source = local` rule excludes
  both directory and federated-provisioned accounts.
- **`user_uuid` is not `sub`.** `oidc/tokens.rs:142` uses `UserId`.
- **`L09` is unused** anywhere in `rfcs/`, `contracts/`, `docs/` or `crates/`.
- **Expired ceremonies are purged.** `crates/sui-id/src/runtime/gc.rs:56`,
  `webauthn_pending::purge_expired`.
- **The sign-in limiter** allows 10 per 60 seconds per address
  (`crates/sui-id/src/runtime/ratelimit.rs:133`). See F12.
- **`DiscoverableKey: From<&Passkey>`** exists
  (`webauthn-rs-0.5.5/src/interface.rs:679`), so the stored passkey blobs
  serve the discoverable finish unchanged. There is no data migration.

## Findings

**F1 — the button is not new; dormant code for it has existed since RFC 034.**
*(Both halves.)* `render_login` already has `show_passkey_option: bool`,
which renders "Sign in with passkey" **above** the password form
(`crates/sui-id-web/src/pages/auth.rs`). **Every caller has passed `false`
since it landed on 2026-06-05** (`1b0080b`), so it never rendered. Three
things are wrong with it:
- its form posts to `/admin/login/webauthn/start`, the second-factor route,
  which needs a pending-MFA cookie, so it **could not have worked**;
- its divider label is a hardcoded English `"or"`;
- RFC 034 (Implemented) and its `CHANGELOG` entry describe a button that
  users never saw.

The newer design document (`sui-id-v0.64.2-uiux-handoff.md`, "Sign-in"
screen) places "Sign in with passkey" **below** the password form, which
is where D1 puts it.

→ **Amended.** D1 now reuses the flag and the existing
`login_passkey_primary` string, moves the block below the form, points it
at the new route, and replaces the literal `"or"` with a locale key. RFC
034's untrue sentence is recorded under *Findings recorded while scoping*,
and is not corrected silently.

**F2 — the ceremony was not bound to the browser that started it.**
*(Safe.)* D4 described the row, its expiry and its single use, but not who
may complete it. Without a binding, a completion could be submitted against
a ceremony some other browser started. The existing second-factor path binds
with an HttpOnly ceremony cookie plus the double-submit CSRF token. **The
sign-in page issues no CSRF token today** (`login_get`, `auth.rs:125`;
`login_post` enforces none).

→ **Amended.** The start sets an HttpOnly, `SameSite=Lax`, path-scoped
ceremony cookie. Start and completion both require the CSRF token. The
login page issues one whenever the switch is on. Completion consumes the row
only by its id taken from that cookie.

**F3 — D3's message had no way to reach the user.** *(Not misread.)* The
existing passkey second-factor refusal **redirects to `/admin/login` with no
message at all** (`handlers/admin/webauthn.rs:96-114`). Copying it would
leave a user looking at the same page with no idea what happened, which is
the worst outcome for the users RFC 144 is about.

→ **Amended.** A refusal renders the sign-in page with D3's sentence and
status `401`, as `login_post` does. It is not a bare redirect.

**F4 — D5 invented a write that already exists.** *(Clean.)* D5 put the
signature-counter update inside the new transaction. Today it is `U29`, a
Class-P write that the second-factor path performs before L02. The start's
row is `U33`, also Class P.

→ **Amended.** The counter update is `U29`, unchanged. The start is `U33`,
unchanged. `L09` consumes the ceremony inside its own transaction, which is
the step-up command's precedent (`webauthn_pending::consume_step_up_within_tx`).
There is one new command, not one new mechanism.

**F5 — step-up freshness was unstated.** *(Safe; not misread.)* L02 stamps
`last_step_up_at` when the second factor is a passkey
(`crates/sui-id-store/src/commands.rs:3506`), so a dangerous operation
right after such a sign-in does not ask again. The RFC did not say whether
`L09` does the same. Leaving it unsaid invites two different implementations.

→ **Amended.** `L09` stamps it, with method `Webauthn`, exactly as L02 does
for a passkey. The same proof gives the same freshness.

**F6 — D8 contradicted itself about registration.** *(Not misread.)* It said
that with the switch off *"nothing in this RFC is reachable"*, and also that
registration asks for `"preferred"`, without saying whether that depends on
the switch.

→ **Amended. Registration asks for `"preferred"` regardless of the switch**,
and D8 says so and why. Discoverability grants nothing while the switch is
off, because the server never runs a discoverable ceremony. Making it
conditional would mean every security key enrolled before an operator
turns the switch on is never offered by the browser afterwards. That is
silent, and it is exactly the confusion D8's operator-guide paragraph would
then have to explain. The closure prerequisite now names the registration
options as the one intended difference, pinned by a test.

**F7 — the typed edit needs a direct dependency.** *(Clean.)*
`ResidentKeyRequirement` is not re-exported by `webauthn-rs`. The prototype
needed `webauthn-rs-proto`, which resolved to the same 0.5.5 already in
`Cargo.lock`. The alternative, patching the JSON string, avoids the
dependency but loses the type.

→ **Amended.** It is a direct dependency on `webauthn-rs-proto`, at the
version `webauthn-rs` itself resolves. `Touches` names `Cargo.toml`
accordingly, and a test pins the serialised options.

**F8 — "Touches" missed the public claims table.** *(Not misread.)*
`docs/src/reference/oidc-api.md:369` has a sign-in path → `acr`/`amr`
table. A relying party's developer reads that, not the code.

→ **Amended.** It gains a row, and `Touches` lists the file.

**F9 — the switch's name can be misread.** *(Not misread.)*
`passkey_sign_in = false` reads as "passkeys cannot be used to sign in".
Passkeys already sign in, as a second factor, so an operator could set it
to `false` believing it disables passkeys altogether.

→ **Amended.** It is now **`passkey_sign_in_without_password`**. It is
longer, and it cannot be misread.

**F10 — "passkey" alone is jargon to the users this most helps.** *(Not
misread.)* Many older users have never heard the word.

→ **Amended.** One helper line under the button, in all three locales:
*"Uses your fingerprint, face or screen lock. No password needed."* The
passkey-page sentence loses *"on a device that offers it"*, which
qualified the sentence into vagueness.

**F11 — provider-linked local accounts.** *(Not misread.)* D2 said *"A
directory or federated account is refused"*. An administrator can link an
existing **local** account to a provider (`link_only`). Its source stays
`local`, the password path serves it, and so does this one. That is
consistent, but D2's sentence read as if it were excluded.

→ **Amended.** It says so explicitly.

**F12 — the rate limit meets a script that expects JSON.** *(Robust; not
misread.)* The start route is fetched by the script. A limiter refusal is
rendered as HTML (`ErrorAs::Html`), which the script would fail to parse,
leaving the user with nothing. At 10 per minute per address, a household or
office behind one address can reach the limit.

→ **Amended.** D9: the script treats any non-success start as *"Too many
attempts. Wait a minute, then try again."* when the status is 429, and as
D1's calm sentence otherwise. It never shows a raw error.

**F13 — the closure prerequisite's "counter reset" was ambiguous.**
*(Not misread.)* It could mean the signature counter or the failed-sign-in
counter.

→ **Amended.** It names the failed-sign-in counter, and lists the
ceremony's consumption.

## What did not change, and why

- **Q1–Q4 stand as written.** Nothing measured here moves a recommendation.
  Q1 is, if anything, strengthened: the user-verification enforcement that
  supports `["hwk","mfa"]` is confirmed in the library code, not only in
  its documentation.
- **Modal rather than autofill, usernameless rather than identifier-first**:
  confirmed by the prototype, and by the newer design document's placement.
- **No password removed, recovery untouched.** Nothing here touches RFC
  103.

## Acceptance

Not given here. `@nabbisen`'s *"almost accepted"* stays exactly that until
he reads this record and the amended RFC. The RFC's `Independent design
review` field cites this record, which G11 condition 9c requires before
any move to `accepted/`.
