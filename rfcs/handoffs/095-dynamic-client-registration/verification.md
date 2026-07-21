# RFC 095 verification and closure plan

## Migration evidence

- Upgrade schema-38 fixtures containing admin and dynamic, public and
  confidential, enabled/disabled/deleted clients.
- Assert admin NULL legacy semantics, dynamic public `none`, dynamic
  confidential `legacy_secret_any`, code+refresh grants, code response, and no
  status/secret/URI drift.
- Assert typed redirect-profile derivation for valid stamped dynamic rows.
  Public mixed HTTPS/loopback, confidential loopback, malformed, noncanonical,
  and every other zero/multi-profile stamped row retain NULL, are reported and
  runtime-ineligible without status mutation.
- Assert `cors_policy_generation` initializes as canonical `'0'`. Cover guarded
  increment from valid values and mutation rollback plus CORS denial for
  missing, malformed, negative, leading-zero/noncanonical, changed, and
  `u64::MAX` overflow state.
- Seed positive-audit unstamped candidates, unrelated admin rows, missing audit
  history, and unverifiable audit history. Assert reporting without mutation,
  no false completeness claim, and durable owner disposition before closure.
- Run migration twice through normal startup and restore a pre-migration backup.
- Reject corrupt tags/JSON and prove new dynamic inserts cannot write NULL or
  `legacy_secret_any`.

## Validation evidence

Execute every row and boundary in
[metadata-validation.md](metadata-validation.md), including wrong JSON types,
duplicate keys, unknown/unsupported members, name/scope bounds, canonical URI
corpus, cross-field grant/scope rules, response echo, and cache headers.

Pure validator property/fuzz targets assert:

- accepted URI input round-trips to the same canonical string;
- matcher symmetry and exactness;
- only `PublicNativeLoopback` authorization port can vary;
- validation never panics on arbitrary UTF-8/JSON values; and
- rejected input cannot construct `ValidatedRegistration`.

## Runtime enforcement evidence

- Basic-only, post-only, none, and legacy credentials cannot cross policy
  boundaries.
- Simultaneous credential sources fail.
- Public clients require PKCE and receive no client secret.
- Basic/POST clients reject every loopback HTTP registration; public HTTPS and
  public native-loopback profiles cannot be mixed.
- Authorization and refresh dispatch reject unregistered grant/response use;
  code-only clients receive no refresh token, and code+refresh clients receive
  one only for an authorization carrying `offline_access`.
- Explicit/default scopes cannot exceed the catalog or stored policy.
- Dynamic empty logout list never falls back; legacy admin fallback remains.
- Every dynamic logout URI is HTTPS independently of authorization profile;
  public-native plus HTTPS logout is accepted and every HTTP logout URI is
  rejected. Runtime matching is byte-exact.
- Dynamic logout redirects require a verified same-client ID-token hint and
  active-session subject/`sid` agreement when applicable. Cover valid
  unexpired hints with/without a session; expired matching-session hints at
  `now-exp` 0, 299, 300, and 301 seconds; future `iat`; missing/invalid/
  wrong-client/session-mismatch hints; and no bare-client/redirect fallback.
- Exercise GET query and form-encoded POST parsing, duplicate/mixed parameter
  rejection, bounds/media type, and identical hint decisions. Missing/invalid/
  mismatched initial requests must not mutate the session. Verify local
  confirmation, explicit subsequent POST, single-use/expiry/session/purpose
  CSRF binding, invalid-CSRF non-mutation, confirmed local-only logout, and
  removal of attacker-supplied RP parameters.
- Exercise the frozen parser bounds at 16,383/16,384/16,385 encoded bytes,
  7/8/9 decoded members, and 8,191/8,192/8,193 bytes per decoded scalar for
  both GET and POST through the same decision corpus.
- Exercise the 32-byte unpadded-base64url confirmation capability at
  `expires_at - 1`, `expires_at`, and `expires_at + 1`; replacement at the
  one-per-session cap; startup/periodic/opportunistic/cascade cleanup;
  cross-session and cross-purpose denial; standard CSRF failure; and at least
  32 simultaneous duplicate confirmation POSTs. Exactly one guarded
  consume/session mutation may win.
- Assert the raw confirmation value appears only in the no-store hidden form
  and POST body, its database digest cannot enter logs/events/errors, secret
  wrappers redact/zeroize, and confirmation/result responses retain
  `frame-ancestors 'none'` plus `X-Frame-Options: DENY`.
- HTTPS matching is byte-exact; only public-native authorization matching may
  vary loopback port; token exchange matches the actual redirect stored with
  the code.
- Disabled/deleted dynamic clients contribute no CORS origin. Enable, disable,
  delete, restore, and redirect changes use canonical parsing, checked
  increment, exact-old-value guard, and atomic generation replacement. Initial
  load failure, invalid representation, database generation-read failure,
  mismatch, and rebuild failure all deny. In particular,
  disable/delete/update followed by failed rebuild never permits the formerly
  allowed origin.
- Loopback CORS remains exact-port even though OAuth redirect comparison permits
  the registered numeric-loopback port exception.
- Consent rendering for a dynamic client produces no browser request to its
  `logo_uri`; application links are user-activated, noreferrer/noopener, and
  no-referrer.

## Possession and transaction-time evidence

- Compile-negative tests reject construction of
  `RegistrationTokenPossession` outside the authorizer, cloning/debugging/
  serializing it, substituting a proof or token ID across contexts, and passing
  it to event/log builders.
- The guarded SQL receives token ID and possession digest only through the
  consumed sealed context; event attributes expose token ID but not digest.
- Capture `guard_at` only after `BEGIN IMMEDIATE`. Exercise expiry before,
  equal to, and after `guard_at`, clock regression, and a queued decision age
  just below/equal/above five seconds. Stale/clock-invalid authority returns
  only a worker-confirmed rollback result.

## C15 rollback matrix

Inject before and after:

1. scope snapshot check;
2. guarded token update;
3. complete client insert;
4. typed event build;
5. audit append/hash update; and
6. commit.

For every injected failure, reconcile:

```text
token used_count = before
client row        = absent
event count       = before
audit chain head  = before
response          = no success/secret
```

Retry after a confirmed rollback must commit exactly one increment, client, and
event.

## Contention and state-race matrix

Run at least 32 contenders against:

- one remaining use: one commit;
- three remaining uses: three commits;
- exhausted token: zero commits;
- revoked token: zero commits;
- expiry immediately before/equal/after within-transaction `guard_at`;
- unlimited token: one commit per submitted valid request; and
- concurrent operator revocation: serialization order determines either the
  registration winner or revocation winner, never a split state.

For every run:

```text
committed use delta = new disabled clients = client.dynamic_register events
```

Event targets and token IDs must reconcile without exposing token hashes.

## Retry and transport evidence

- Two identical sequential successful requests create two registrations and
  consume two uses.
- A complete validation error consumes zero and may be corrected/retried.
- A complete 503 proves rollback and carries bounded `Retry-After`.
- Simulated response loss after commit is documented/observed as ambiguous; no
  automatic retry is performed and plaintext secret recovery is not claimed.
- Exercise cancellation before dispatch, while blocking work is queued, before
  the cooperative pre-commit check, around commit, and after commit/before
  response. Only pre-dispatch cancellation or a delivered worker-confirmed
  rollback is classified safe; every lost post-dispatch response remains
  ambiguous and is not automatically retried.

## Secret, logging, and error evidence

- Capture sanitized logs/metrics for missing, unknown, expired, revoked,
  exhausted, contention-loss, validation, database, and success paths.
- Assert no bearer, digest, client secret/hash, complete redirect/application
  URI, SQL text/value, or attacker-controlled name/scope is present.
- Assert all invalid-token variants have identical public status/header/body.
- Exercise zeroize-on-drop paths for hashing failure, scope drift, guard loss,
  insert/audit/commit failure, success handoff, and response construction.

## Required clean-commit gates

Record one full commit ID and observed results for:

- migration and store model tests;
- dynamic registration unit/integration/e2e tests;
- token/authorization/logout policy tests;
- validation property/fuzz corpus;
- RFC 094 C15 structural and failure-injection gates;
- RFC 093 G01–G12, LDAP smoke, mdBook, and RFC integrity; and
- independent adversarial closure review.

Results from different commits cannot be assembled into a passing closure
package.
