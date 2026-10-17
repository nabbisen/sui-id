# RFC 096-B1 stage 12c — the token endpoint auth method is not in the profile

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. The direction is not a judgement call: RFC 096
> settles it, and the implementation contradicts it.

**Dispatched.** 2026-10-17 JST, by the architect.
**RFC.** 096 — **Status: Accepted**.
**Why it exists.** Stage 12a built the `supports_token_endpoint_auth_method`
cross-check, watched it refuse every provider the handler can reach, and
backed it out rather than patch the fixtures around it. That was the right
call, and what it exposed is this.
**Why now and not with 12b.** 12b is blocked on `@nabbisen`'s account-lock
call; this is not, and cycle D should not ship federated MFA on a token
exchange that cannot authenticate to a conformant provider.
**Baseline.** Read the tip with `git log -1`, hash against it, name it.

## 1 — what is wrong, and why it is not a design question

The handler sends the client secret in the token request's **form body** —
`handlers/federation.rs:852-853`, `form_params.push(("client_secret", …))`,
then `.form(&form_params)`. There is no `basic_auth` call anywhere in the
file. Confirmed unchanged at `ee57b76:790`, so this is shipped behaviour and
not something 12a introduced.

That is `client_secret_post`. **RFC 096 `:320`:**

> `token_auth_method` | Exactly `client_secret_basic` or `none`;
> `client_secret_post` is not in the M4 profile

So the implementation uses the one method the RFC's own profile table
excludes. `:570` then requires discovery's
`token_endpoint_auth_methods_supported` to **contain the configured method**,
which is the check 12a built — it refused correctly, and the thing it refused
was ours.

Your own corroboration stands and is worth keeping in the record: 13 of 13
`token_endpoint_auth_methods_supported` occurrences across
`crates/sui-id/tests/` advertise `client_secret_basic`, none advertise
`client_secret_post`; and RFC 6749 §2.3.1 makes Basic the one a confidential
client must support, with post the optional alternative.

**So there is no "which should we choose".** Build `client_secret_basic` for
confidential clients and `none` for public ones, per `:320`.

## 2 — what to build

- **`client_secret_basic`**: the credentials move from the form body to an
  `Authorization: Basic` header, with `client_id` and `client_secret`
  **URL-encoded before base64** as RFC 6749 §2.3.1 requires — that encoding
  step is the part implementations most often get wrong, and a secret with a
  reserved character is the test that catches it.
- **`none`**: a public client sends neither; confirm what the handler does
  today when `client_secret` is `None` and say whether it already matches.
- **Then wire the cross-check that started this** —
  `supports_token_endpoint_auth_method` against discovery, which should now
  pass against every fixture rather than refuse all of them. If it still
  refuses something, that is a finding, not a fixture to edit.

**Do not make the method provider-configurable.** `:320` says exactly two
values are in the profile; a configuration knob would re-admit the third.

## 3 — the fixtures are evidence, not obstacles

12a found 38 tests failing when the check went in. After this change they
should pass **because the handler now does what they advertise** — not
because anything in them moved. If you find yourself editing a fixture's
`token_endpoint_auth_methods_supported` to make a test pass, stop: that is
the fixture telling you the handler is still wrong.

## 4 — the CHANGELOG entry

A `### Fixed` or `### Security` entry, your call with the reason given. The
facts it has to carry: the federated token exchange authenticated with
`client_secret_post`, which RFC 096's profile excludes and which a conformant
provider advertising only `client_secret_basic` would reject; federation
against such a provider could not complete a sign-in; and it is fixed. Say
whether released versions are affected — `0.80.0` is the one to check, the
way the last two rounds' entries do.

## What this must not do

**No 12b work.** `/admin/login/mfa` is still waiting on the account-lock call.

**No new provider configuration surface.**

## What to return

1. **The change**, with the URL-encoding-before-base64 point addressed
   explicitly and a test for a secret containing a reserved character.
2. **The cross-check wired**, and the count of tests that failed before and
   pass after.
3. **The `none`/public-client path**, and whether it already behaved.
4. **The CHANGELOG entry.**
5. **Per-hunk SHA-256** against the tip you name, regenerated after your last
   commit.
6. **The full matrix, all 24 green.**
7. **Count reconciliation** from `57b4ead`: `sui-id-core --lib` **201**,
   `sui-id-store --lib` **529**, `sui-id --lib` **560**, `sui-id --test e2e`
   **537**, `sui-id --test compile_fail` **12**, `sui-id-i18n` **19**,
   `scripts/tests` **349**.
8. **Anything wrong with this dispatch.** You found this one by building a
   check and believing it when it refused everything, which is the whole
   reason it is now a stage instead of a surprise in production.
