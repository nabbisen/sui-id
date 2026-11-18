# 096-B1 stage 14 — live run record, 2026-11-18

**Commit.** `c30056b67aa89756e849a1ff219cee9032748a2f`; `sui-id --version` →
`sui-id 0.92.0`. Built `--release --locked` in the clean clone
`../sui-id-stage14`; instance in `../sui-id-live/` on `127.0.0.1:8801`.
**Upstream.** `https://demo.duendesoftware.com` (Duende's public demo).
**Who.** `@nabbisen` clicks in the browser, including both sign-ins at the
demo; the architect checks each step against `server.log` and the database.

| # | Result | Observed (owner) | Second record (architect) |
|---|---|---|---|
| 3 | PASS | — | `preview-upgrade`: *"No database exists … Starting this sui-id creates its schema and runs all 51 migrations. Nothing was changed."* No configuration error |
| 1 | PASS | — | `WARN sui_id::startup: federation provider "duende-demo" (id b445daf3-…) requires review before federated sign-in through it is re-enabled -- has no linked user, missing policy fields: none -- visit /admin/federation-providers and press "Check and enable"`. The only other `WARN`: the fresh master key, expected |
| 2 | PASS | *"Account created"* | `admin.setup.completed` 05:42:19, then `auth.login.success`. The admin is named `alice`, role `admin`, no email. (`WARN … setup-wizard admin password was found in HIBP breaches; accepted because hibp_mode = warn`: a throwaway password chosen for the run; warn mode, as configured) |
| 3–5 | PASS | *"the configuration enabled"* | `federation_provider` `duende-demo`: `enabled = 1`, `validation_status = validated`, `evidence_observed_at` 05:43:08, `id_token_algs = RS256`, `activation_generation = 1`; audit `federation.provider.enabled` 05:43:08. (Also, by the owner: `settings.default_language.changed`, `settings.hibp_mode.changed`) |

**Before step 7, measured:** the admin holds the username `alice`, which the
demo's `alice` will also propose. Collision is checked by **email** only
(`identity_resolution.rs:86-91`); the admin has none, so provisioning is
eligible, and `resolve_shadow_username` (`federation_identity.rs:52-69`)
gives the new user **`alice2`**, role `User`. Step 7 therefore also shows
that a federated identity named like the local admin gets its own
non-admin account.

## Restarted 2026-11-18 05:45, at the owner's request, before step 6

`@nabbisen`: *"To be clearer on test record, I might restart with another
name like `bob`."* The first attempt's admin shared the demo user's name.
Nothing failed. The instance was stopped, and its database, key and log
deleted; the same binary and configuration were reused. **The table below is
the record**; the rows above are kept as what was observed.

| # | Result | Observed (owner) | Second record (architect) |
|---|---|---|---|
| 3 | PASS | — | configuration unchanged from the first attempt, which `preview-upgrade` accepted |
| 1 | PASS | — | `WARN sui_id::startup: federation provider "duende-demo" (id 3c61f005-…) requires review before federated sign-in through it is re-enabled -- has no linked user, missing policy fields: none -- visit /admin/federation-providers and press "Check and enable"`; the only other `WARN`, the fresh master key |
| 2 | PASS | *"Done again."* | `admin.setup.completed` 05:45:37, then `auth.login.success`. Admin `bob`, role `admin`, no email. (`WARN … admin password was found in HIBP breaches; accepted because hibp_mode = warn`: a throwaway password chosen for the run) |
| 4 | PASS | as expected | *Check only* is read-only by design: it runs the preflight and shows a flash, writing nothing (`handlers/admin/federation_providers.rs:279-320`), so no audit record is expected or present |
| 5 | PASS | as expected | `duende-demo`: `enabled = 1`, `validation_status = validated`, `evidence_observed_at` 05:45:57, `id_token_algs = RS256`, `activation_generation = 1`; audit `federation.provider.enabled` 05:45:57. (Also, by the owner: `settings.default_language.changed`, `settings.hibp_mode.changed`) |
| 6 | PASS | signed out; the login page showed the Duende button | `POST /admin/logout` 200; audit `auth.session.revoked`, `trigger=self_signout`, 05:46:42 |
| 7 | **FAIL (designed refusal)** | signed in at the demo as `alice`; back at sui-id, not signed in; login page: *"Your identity provider reports that your email address is not verified. Verify it with your identity provider, then try again."* | `GET /auth/federated/duende-demo/start` 303 (05:46:51); `GET /auth/federated/callback` (05:47:08): `INFO … provision held: absent or unverified email slug=duende-demo`, then 303 to the login page; audit `auth.federation.signin.denied`, `result=denied`, `reason=unverified_email`; **no user created** (`users`: `bob` only) |

### Step 7, analysed

**What it proves.** The refusal is decided in identity resolution
(`identity_resolution.rs:83-85`, `DeniedUnverified`), which runs only on a
`VerifiedIdTokenClaims`. So against a real certified upstream, every
security-critical stage before it **passed**: discovery and JWKS fetch, the
authorization request with PKCE S256, state and nonce, the code exchange
with `client_secret_basic`, the RS256 signature against Duende's JWKS, and
the required and optional claim checks. The denial path then behaved as
designed: refused, audited, nothing provisioned.

**Why it refused.** RFC 096 (accepted): *"Userinfo fallback is removed.
Email/profile data must be present in the verified ID token when needed."*
Duende IdentityServer's default for the authorization-code flow returns
`email` and `email_verified` from the userinfo endpoint, not in the ID
token, unless the client sets `AlwaysIncludeUserClaimsInIdToken`. sui-id
reads only the ID token, so no verified email arrived. **Not a sui-id
protocol defect.** sui-id's log does not distinguish *absent* from
*unverified* (`provision held: absent or unverified email`).

**Two findings it exposed:**
- **R1, the message is not true here.** *"Your identity provider reports that
  your email address is not verified"*: the provider reported no such thing;
  most likely it sent no email in the ID token at all. *"Verify it with your
  identity provider"* sends the person on an errand that cannot help. The
  reason code (`unverified_email`) conflates the two cases too.
- **R2, the requirement is undocumented where an operator would look.** The
  configuration reference and the operator guide say provisioning *"requires
  `email_verified = true` from the upstream"*, never that it must be **in the
  ID token**. An IdentityServer-based upstream fails exactly like this.

**Not provable with this demo:** the provisioning happy path (steps 7–10),
because the demo's client is not ours to configure.
| 8–10 | NOT RUN | — | depend on a provisioned, linked user, which step 7 could not create with this demo |
| 11 | PASS | *"It was disabled."* | `duende-demo`: `enabled = 0`; audit `federation.provider.disabled` 05:51:45, `old_activation_generation=1 new_activation_generation=2`; the login page no longer renders the Duende button |
| 12 | PASS | *"Shown as you had described. Deleted."* | restarted without the provider block: `WARN … has no matching [[federation_providers]] block in this configuration; it is disabled and keeps its existing federation links -- restore the block, or delete the provider`. Then `federation_provider` empty, `federation_link` 0 rows; audit `federation.provider.deleted` 05:53:49, `deleted_links=0`, `old_activation_generation=2 new_activation_generation=3` |

**Log totals:** 0 `ERROR` lines in `server.log` across both starts. No token,
code, state or cookie is quoted in this record.

**Result:** steps 1–6, 11 and 12 PASS; step 7 a designed refusal (no verified
email in the ID token, from this demo's client); steps 8–10 not run.

## Decided 2026-11-18, on this record

The owner was asked two questions, and chose the architect's recommendation
for each:
1. **`0.92.0`'s tag proceeds on this record; M4-B stays open** until account
   creation is proven live against an upstream that puts `email` and
   `email_verified` in the ID token (Google does by default), run early in
   cycle O, so RFC 097 can still start there.
2. **R1 and R2 are fixed before the tag**, in one small dispatch (N2d): an
   honest refusal message, true whether the email was absent or unverified,
   and the operator documentation's ID-token requirement.

**The tag's commit will follow `c30056b`** by N2d's change, which touches
only that message and the documentation. Nothing this run exercised
changes.

## Afterwards (§7)

sui-id stopped; `../sui-id-live/` deleted. The release build in
`../sui-id-stage14/` is kept for cycle O's account-creation run.
