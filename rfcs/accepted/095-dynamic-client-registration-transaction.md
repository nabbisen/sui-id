# RFC 095 — Dynamic Client Registration Transaction and Validation

**Status.** Accepted
**Security review.** Required
**Accepted on.** 2026-07-18
**Approved by.** `@nabbisen`
**Independent design review.** `codex-independent-architecture-security-reviewer` (OpenAI Codex), [Accept with notes](../reviews/095-design-review-2026-07-18.md)
**Implementation owner.** `codex-developer` (OpenAI Codex), confirmed by `@nabbisen`; implementation start remains gated below
**Design prerequisites.** RFC 094 Accepted with baseline dynamic registration assigned to its Class-A transaction seam; RFC 7591 metadata scope and redirect/logout URI policy independently reviewed.
**Implementation prerequisites.** RFC 094 Implemented and its baseline dynamic-registration mutation/audit transaction passes; this RFC Accepted; adversarial test plan approved.
**Closure prerequisites.** Validate-first, single-transaction behavior passes invalid-input, rollback, retry, and concurrency tests; independent closure review accepts evidence.
**Tracks.** ROADMAP M3 — Atomic dynamic registration.
**Touches.** `crates/sui-id/src/http/handlers/dynamic_register.rs`, token-endpoint credential extraction, OIDC authorization redirect matching, client core/store models, `repos/client_registration_token.rs`, the RFC 094 C15 transaction/event seam, schema migration 0039 or its next available equivalent, discovery/registration documentation, and adversarial tests.
**Handoff.** [`../handoffs/095-dynamic-client-registration/README.md`](../handoffs/095-dynamic-client-registration/README.md)
**Validation matrix.** [`../handoffs/095-dynamic-client-registration/metadata-validation.md`](../handoffs/095-dynamic-client-registration/metadata-validation.md)
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Independent security and closure reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex).

## Summary

Complete dynamic registration on RFC 094's already-atomic baseline: validate
all supported metadata before entering that transaction, persist the complete
validated metadata set, and prove guarded limited-use concurrency/retry
semantics adversarially. RFC 094 already makes use consumption, disabled-client
creation, registration-source stamp, and typed audit one transaction, so no
production Class-A best-effort gap crosses the M2 boundary.

The supported surface is deliberately smaller than the full RFC 7591 registry.
It matches what sui-id can enforce at authorization, token, consent, and logout
time. Unknown extension metadata is ignored as RFC 7591 requires; understood
but unsupported standard metadata is rejected so a caller cannot mistake an
ignored security setting for an active one.

## Normative sources and local policy

This design applies:

- [RFC 7591](https://www.rfc-editor.org/rfc/rfc7591.html) for request metadata,
  defaults, response echoing, and registration errors;
- [RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html) for exact redirect
  matching and rejection of wildcard/pattern matching;
- [RFC 8252](https://www.rfc-editor.org/rfc/rfc8252.html) for numeric loopback
  redirects and the port-only comparison exception; and
- [OpenID Connect RP-Initiated Logout 1.0](https://openid.net/specs/openid-connect-rpinitiated-1_0.html)
  for `post_logout_redirect_uris`.

Where those specifications permit deployment policy, this RFC chooses the
closed policy below. Private-use custom schemes, software statements, client
JWK metadata, registration management, localized metadata, and open
registration are not supported by this milestone.

## Current defect and M2/M3 boundary

The current handler consumes a registration-token use before validating the
JSON metadata. It populates application-identity and registration-source values
in `ClientRow`, but the current `clients::create` statement omits those columns;
the audit append is a separate best-effort call. It accepts arbitrary
`grant_types` and any non-`none` `token_endpoint_auth_method`, returns those
values, and does not persist or enforce them. It also accepts logout URIs
without validating them.

RFC 094 owns the M2 correction that makes the existing baseline C15 command
Class-A atomic. RFC 095 begins only from that implemented seam and owns:

1. the exact supported metadata contract and bounded parser;
2. validation before any durable mutation;
3. persistence and runtime enforcement of grant/auth metadata;
4. typed redirect profiles, public-native authorization loopback handling, and
   exact authenticated logout redirection;
5. limited-use contention, rollback, and honest retry semantics; and
6. protocol response/error behavior and adversarial evidence.

RFC 095 does not reopen RFC 094's capability, actor-context, typed-event, or
audit-chain architecture.

## Request envelope

`POST /oauth2/register` remains authorized registration only.

- TLS termination is mandatory in deployed configurations. Open registration
  remains disabled.
- `Content-Type` must be `application/json`; other media types return 415.
- The decoded body limit is 64 KiB. Oversize input returns 413 before JSON
  allocation or database work.
- The JSON top level must be one object. A streaming duplicate-member detector
  rejects duplicate keys, including duplicate unknown keys. Nesting depth is at
  most 16 and the top-level object contains at most 128 members.
- The local initial access token format is exactly 64 lowercase hexadecimal
  characters, as emitted by the issuance command. Other bearer syntax follows
  the same generic invalid-token path without hashing or logging the input.
- JSON type errors and malformed JSON return `invalid_client_metadata` only
  after a valid initial access token has been established; malformed bearer
  syntax is rejected first.
- Unknown extension members are ignored and are neither persisted nor echoed.
  The 64 KiB limit still counts them.
- Localized `name#tag` members, `contacts`, `jwks_uri`, `jwks`, `software_id`,
  and `software_version` are understood but unsupported and return
  `invalid_client_metadata`. `software_statement` returns
  `unapproved_software_statement` because no trust anchor is configured.

The endpoint has an IP rate limit before body processing and a second limiter
keyed by a truncated process-local keyed hash of the token digest after bearer
parsing. The limiter key resets on process restart and is neither persisted nor
exported. Neither limiter key, token digest, nor raw bearer appears in logs or
metrics.

## Supported metadata contract

| Member | Presence/default | Accepted value | Persistence/enforcement |
|---|---|---|---|
| `redirect_uris` | Required | 1–16 unique canonical redirect URIs forming one closed redirect profile | Stored in request order with the derived typed profile; matched under that profile |
| `client_name` | Required by sui-id | Trimmed, 1–128 Unicode scalar values and at most 512 UTF-8 bytes; no control or bidi-override/isolate characters | Stored and safely escaped for UI; bounded sanitized form in audit |
| `token_endpoint_auth_method` | Default `client_secret_basic` | Exactly `none`, `client_secret_basic`, or `client_secret_post` | Stored as a typed value and enforced at `/oauth2/token` |
| `grant_types` | Default `["authorization_code"]` | `["authorization_code"]` or the canonical pair `["authorization_code","refresh_token"]` | Stored and checked before token grant dispatch |
| `response_types` | Default `["code"]` | Exactly `["code"]` | Stored; authorization endpoint already rejects other response types |
| `scope` | Default current catalog rows with `is_default=1` | 1–32 unique registered scope tokens, including `openid`; at most 1,024 bytes total | Canonically space-joined and enforced by existing scope policy |
| `post_logout_redirect_uris` | Default empty | 0–16 unique canonical HTTPS URIs, independent of the authorization redirect profile | Stored separately and always matched byte-for-byte |
| `logo_uri` | Optional | One canonical HTTPS application URI | Stored and echoed for protocol/operator inspection; never rendered or fetched for a dynamic client |
| `client_uri` | Optional | One canonical HTTPS application URI | Stored as `homepage_uri`; never fetched |
| `policy_uri` | Optional | One canonical HTTPS application URI | Stored as `privacy_policy_uri`; never fetched |
| `tos_uri` | Optional | One canonical HTTPS application URI | Stored; never fetched |

For new dynamic clients, an empty stored scope never means the legacy
“allow any scope” policy. An explicit empty `scope` is invalid. The default
catalog set must contain `openid`, or registration fails closed as a
configuration error without consuming the token.

`refresh_token` and `offline_access` are coupled: either both are present in
the normalized grant/scope policy or neither is. This prevents a registration
from advertising or receiving refresh capability that its persisted policy
does not authorize.

For a code-only dynamic client, authorization-code exchange returns no refresh
token and the token response omits that optional member. A code+refresh client
receives a refresh token only when the particular authorization request
included `offline_access`. Legacy clients retain their prior issuance behavior.

## Normalization and validation order

Validation produces one immutable `ValidatedRegistration` before the Class-A
write transaction:

1. apply the IP limiter, media-type/body bounds, bearer syntax, and token-keyed
   limiter;
2. run the read-only token predicate once as an early generic authorization
   probe; invalid states stop before parsing attacker-controlled metadata;
3. enforce duplicate-key, depth/member, collection, and scalar bounds;
4. parse supported fields and reject understood-but-unsupported fields;
5. validate and canonicalize grant, response, auth-method, and scope values;
6. derive the closed authorization redirect profile, validate authorization
   redirects against it, and independently validate logout/application-identity
   URIs;
7. trim and validate the client name;
8. read the scope catalog and resolve omitted/default or explicit scope policy;
9. generate a client ID and, for confidential methods, a 32-byte random client
   secret plus Argon2id hash outside the database transaction;
10. build and serialize the complete 201 response into a secret-bearing,
   zeroize-on-drop buffer before mutation; and
11. immediately before entering C15, repeat the read-only predicate and obtain
   one sealed, possession-bound registration-token authorization decision.

Sorting is used only for closed sets: grants are stored in the order
`authorization_code`, `refresh_token`, and scope tokens are ASCII-sorted.
Redirect and display URI strings are not semantically normalized after
validation; their canonical input strings and request order are preserved.
This avoids turning two different registered redirect strings into an
unexpected match.

## URI policy

Every URI is at most 2,048 ASCII bytes, parses as one absolute `url::Url`, and
must already equal the serializer's canonical form. Backslashes, malformed
percent escapes, Unicode host spellings, default-port aliases, and other
non-canonical spellings are rejected rather than rewritten.

### Authorization and logout redirects

Every new registration derives and persists exactly one typed
`RedirectProfile`; clients cannot mix profiles:

| Profile | Derivation | Authorization redirects | Runtime rule |
|---|---|---|---|
| `ConfidentialHttps` | Basic or POST authentication | HTTPS only | Exact byte-for-byte |
| `PublicHttps` | `none` and every redirect is HTTPS | HTTPS only; includes claimed-HTTPS native links in the supported subset | Exact byte-for-byte; PKCE mandatory |
| `PublicNativeLoopback` | `none` and every redirect is HTTP numeric loopback | `127.0.0.1` or `[::1]`, each with an explicit nonzero port | Only the port may vary; PKCE mandatory |

Thus a confidential client, including Basic or POST, can never register a
loopback HTTP redirect. A public registration cannot mix HTTPS and loopback
redirects. Claimed HTTPS native links are supported only as ordinary
`PublicHttps` redirects: sui-id performs no platform/app-link attestation and
grants them no relaxed matching. Custom schemes and `localhost` remain outside
the supported subset.

A redirect or post-logout redirect has no fragment, username, password,
wildcard, or empty host. Query and path components are permitted and form part
of the registered value. `localhost`, other loopback ranges/names, private-use
schemes, custom application schemes, `file`, `data`, and `javascript` are
rejected. Duplicate canonical strings in one collection are rejected.

At authorization time, `ConfidentialHttps` and `PublicHttps` comparison is
exact byte-for-byte. For `PublicNativeLoopback`, scheme, numeric IP literal,
path, query, and every other component must match exactly; only the port may
differ. The authorization code stores the actual redirect used, and token
exchange requires an exact match with that stored value. Prefix, suffix,
wildcard, decoded, case-folded, and query-subset comparisons are forbidden.

Post-logout redirects are independent of the authorization redirect profile
and stricter than every authorization profile: every new dynamic
post-logout URI is canonical HTTPS and always matches its registered string
byte-for-byte. A `PublicNativeLoopback` client may therefore register an
independent HTTPS post-logout URI or no post-logout URI; it cannot register an
HTTP loopback logout URI. No authorization loopback exception transfers to
logout.

Dynamic clients with no `post_logout_redirect_uris` have no logout redirect.
They do not inherit the legacy fallback to `redirect_uris`. Existing
non-dynamic clients retain that fallback until a separate compatibility
decision removes it.

A dynamic post-logout redirect additionally requires a cryptographically
verified `id_token_hint`. Signature, issuer, token shape, and audience are
validated under the issuer's ID-token rules; the audience must contain the same
client, `azp` must equal it when required by a multi-audience token, and a
supplied `client_id` must agree. With an active session, `sub` must match the
session principal and `sid`, when both exist, must match the session.

An unexpired valid hint without an active session may establish RP legitimacy
for an exact registered redirect; it does not manufacture a session. At
`exp <= now` the hint is expired. An expired hint is accepted only when the OP
still has an active session whose subject/applicable `sid` matches and
`0 <= now - exp <= 300` seconds. Exactly 300 seconds is accepted; 301 seconds
is rejected. `iat` must not be in the future. This intentionally narrows the
logout specification's expired-token recommendation because sui-id has no
separate trustworthy recent-session-to-client record: a five-minute,
current-session-bound window prevents an old signed ID token from remaining a
long-lived redirect capability. The grace authorizes only logout validation,
never API or token use.

`/oauth2/logout` accepts both GET query parameters and POST
`application/x-www-form-urlencoded` parameters under one bounded parser.
Duplicate security parameters, mixed query/body security parameters, unsupported
media types, and over-limit input are rejected before session mutation. A
verified hint associated with the current session may perform logout and then
use only its exact registered redirect. A verified unexpired hint with no
active session may use the exact redirect as an already-signed-out result.

A missing, invalid, over-grace, client-mismatched, or session-mismatched hint
never mutates the session and never redirects on the initial GET or POST.
Instead, an active session receives a local confirmation page containing a
single-use, short-lived CSRF token bound to that session and confirmation
purpose. Only a subsequent explicit POST confirmation with that token may
perform local logout; it ignores all RP redirect/client parameters and returns
only a local signed-out result. Invalid confirmation CSRF leaves the session
unchanged. With no active session, the handler returns only a local
already-signed-out/error result. No path falls back to bare `client_id`,
`redirect_uris`, an unverified URI, or a different RP.

### Application-identity URIs

`logo_uri`, `client_uri`, `policy_uri`, and `tos_uri` require canonical HTTPS,
an ASCII host, and no userinfo or fragment. They are display references only:
sui-id does not resolve, preflight, proxy, download, or otherwise fetch them.
Consent and other browser UI must not emit an image or other automatic request
for a dynamically registered `logo_uri`; the value is retained only for
protocol echo and operator inspection. `client_uri`, `policy_uri`, and
`tos_uri` may be user-activated links with `rel="noopener noreferrer"` and a
no-referrer policy, never automatic subresources. Existing
administrator-created client presentation is unchanged. This keeps server
validation outside SSRF and prevents a dynamic registrant from learning that a
user viewed consent merely by registering a tracking image.

## Scope, grant, and authentication compatibility

Scope tokens use the RFC 6749 `scope-token` ASCII character set, are 1–64 bytes
each, and are unique. Each must exist in the current `scope_definition`
catalog. For an explicit scope request, the transaction requires every
validated token still to exist. For an omitted scope, it recomputes the sorted
`is_default=1` set and requires exact equality with the validated snapshot.
Thus a concurrent catalog edit cannot create a client with stale or silently
expanded authority.

The only valid combinations are:

| Token auth | Client type | Grants | Required scope consequence |
|---|---|---|---|
| `none` | Public; no secret generated | authorization code, optionally refresh | PKCE remains mandatory; refresh requires `offline_access` |
| `client_secret_basic` | Confidential; one secret returned | authorization code, optionally refresh | Refresh requires `offline_access`; Basic is the only accepted token-endpoint credential transport |
| `client_secret_post` | Confidential; one secret returned | authorization code, optionally refresh | Refresh requires `offline_access`; body credentials are the only accepted transport |

`implicit`, `password`, `client_credentials`, device grants, JWT/SAML bearer
grants, response type `token`, and extension auth methods are rejected. A
request containing both HTTP Basic and body credentials is rejected; credential
sources are never merged. Public clients presenting a secret are rejected.

The token response therefore makes `refresh_token` optional. Grant enforcement
occurs before dispatch and again before issuance, so a disallowed refresh
credential is neither issued nor accepted.

## Persistence and compatibility migration

The next available migration after RFC 094 adds nullable
`token_endpoint_auth_method`, `grant_types`, `response_types`, and
`redirect_profile` columns to `clients`, with checks for valid JSON/type tags.
`NULL` is an explicit legacy state for administrator-created rows; new dynamic
clients cannot write NULL. It also inserts
`sui_meta.cors_policy_generation = '0'`. Its text value has exactly one
canonical nonnegative decimal encoding: `0` or `[1-9][0-9]*`, bounded by
`u64::MAX`.

Every mutation that can change an enabled client's redirect origins, enabled
state, or deleted state reads and strictly parses that value, computes
`checked_add(1)`, and performs a one-row guarded replacement from the exact old
canonical text to the exact new canonical text in the same transaction.
Missing, malformed, negative, noncanonical, concurrently changed, or overflowed
state aborts the client mutation; it is never coerced, defaulted, saturated, or
wrapped.

Rows correctly stamped `registered_via = 'dynamic'` by the implemented RFC 094
baseline cannot recover discarded auth/grant request metadata. The migration
backfills those rows conservatively:

- public rows become auth method `none`;
- confidential rows become internal `legacy_secret_any`, preserving the
  current Basic-or-body behavior without allowing new registrations to select
  it;
- grants become `["authorization_code","refresh_token"]`, matching the
  effective pre-RFC-095 token surface; and
- response types become `["code"]`; and
- redirect profiles are derived only for positively stamped dynamic rows when
  the stored auth method and complete redirect set derive exactly one valid
  profile. Public HTTPS/loopback and confidential HTTPS rows derive their
  corresponding profile. Any positively stamped row that derives zero or more
  than one profile—including public mixed HTTPS/loopback, confidential
  loopback, malformed, or noncanonical redirects—is reported for owner
  disposition, retains NULL, and remains runtime-ineligible even if its legacy
  enabled flag is set. It is never guessed into a weaker profile, and migration
  does not silently rewrite its enabled/deleted flags.

The migration does not enable, delete, or rotate any client. Backfilled
`legacy_secret_any` is visible to operators and covered by an upgrade test and
documentation; later narrowing requires an explicit operator choice. Existing
administrator-created NULL rows retain their current behavior.

Pre-RFC-094 dynamic registrations are a separate historical ambiguity:
`clients::create` discarded `registered_via`, so their rows look like
administrator-created clients. The migration must not infer provenance from
shape, timestamps, names, or missing application URIs. A read-only upgrade
report lists only positive `client.dynamic_register` audit targets whose client
row still says `admin`, labels them `legacy_unstamped_dynamic_candidate`, and
records when audit history is absent or unverifiable. It does not mutate them.
Before M3 closure, the owner reviews every reported candidate and durably
records disablement, explicit legacy acceptance, or a separately audited
metadata correction. No “zero candidates” claim is allowed when the historical
best-effort audit cannot prove completeness.

The C15 insert writes the base row, disabled/deleted flags, source,
application-identity fields, logout URIs, scopes, auth method, grants, response
types, consent policy, and timestamps in one statement. Separate
`set_registered_via` or `update_app_identity` writes are forbidden in the
dynamic path.

Dynamic clients remain disabled after C15. The redirect-origin CORS cache
excludes disabled and deleted clients and derives origins with the same URL
parser rather than string slicing. C15 does not publish a disabled client's
origin.

Each immutable cache snapshot contains the strictly parsed CORS-policy
generation read in the same database read transaction as its origin rows.
Before any cached origin can authorize a preflight or token request, middleware
strictly parses the current durable generation and requires exact typed
equality with the snapshot. A missing/malformed/negative/noncanonical value,
database read failure, mismatch, missing initial snapshot, or rebuild failure
denies the origin. Every audited enable, disable, delete, restore, or
redirect-origin change performs the guarded checked increment within its
Class-A mutation; only a successful post-commit rebuild can publish the
matching generation. Therefore stale state can deny newly enabled origins but
can never continue to allow a disabled, deleted, or changed origin. This
per-request generation read is intentional; an in-process notification alone
is not sufficient across crashes or multiple processes. The
current-generation read is the CORS authorization linearization point: a
mutation serialized after it may affect the next request, while a mutation
already committed necessarily produces a mismatch until rebuild.

The loopback port exception applies only to `PublicNativeLoopback`
authorization redirect comparison, not to logout or CORS. CORS always compares
the exact parsed origin, including port; native loopback clients normally do
not depend on browser CORS, and no wildcard-port origin is introduced.

## Registration-token authorization

The raw initial access token is accepted only as one 64-lowercase-hex Bearer
credential and is immediately reduced to its SHA-256 digest. A read-only
authorization query uses the indexed digest and the same predicate shape for
unknown, expired, revoked, exhausted, and structurally invalid counter states.
Failure returns one generic `invalid_token` response; no state-specific
description, metric label, or log field is emitted.

Successful read authorization consumes into a private
`AuthorizedCommandContext<C15DynamicRegistration>`. In addition to the
registration-token ID, request/correlation ID, and decision time, its sealed
C15 authority owns a `RegistrationTokenPossession` made from the successful
probe's SHA-256 digest. That proof:

- is constructed only by `RegistrationTokenAuthorizer` from the same query
  result as the token ID;
- is private, non-`Clone`, non-`Debug`, non-serializable, zeroize-on-drop, and
  inaccessible to HTTP/event construction;
- cannot be supplied separately from, or substituted into, a context; and
- is consumed by the guarded C15 helper to bind `token_hash` without exposing
  it to the handler or event.

The raw token never enters the context. Neither the raw token nor the
possession digest can be placed in a typed audit event, log, metric, error, or
response. Compile-negative tests must reject construction outside the
authorizer, token-ID/proof mixing, and event/log arguments of the proof type.

The decision time is not the final expiry authority. C15 permits at most five
seconds between the second probe and the within-transaction guard. After
`BEGIN IMMEDIATE`, the database worker obtains `guard_at` from its sealed,
test-injectable trusted transaction clock. Clock regression
(`guard_at < decision_at`) or decision age over five seconds aborts with a
confirmed-rollback retryable internal result. Expiry is exclusive:
`expires_at <= guard_at` is expired. The guarded statement is normatively
equivalent to:

```sql
UPDATE client_registration_token
SET used_count = used_count + 1, updated_at = :guard_at
WHERE id = :token_id
  AND token_hash = :possession_digest
  AND max_uses >= 0
  AND used_count >= 0
  AND revoked_at IS NULL
  AND (expires_at IS NULL OR expires_at > :guard_at)
  AND (max_uses = 0 OR used_count < max_uses)
```

Exactly one affected row is required. Zero rows returns the same generic
`invalid_token` result and aborts C15 without an event. More than one row is a
database invariant failure. Read-then-write arbitration or an unconditional
increment is forbidden.

## C15 Class-A transaction

The RFC 094 C15 runner receives only validated/prepared inputs and executes one
immediate SQLite write transaction:

```text
validate/prepare outside transaction
  -> sealed token authorization decision
  -> BEGIN IMMEDIATE
  -> capture trusted guard_at; enforce non-regression and <=5-second age
  -> recheck scope-catalog snapshot
  -> guarded registration-token consume using sealed possession (must affect 1)
  -> insert complete disabled dynamic client (must affect 1)
  -> build typed client.dynamic_register event from committed candidates
  -> append/hash event through RFC 094 runner
  -> COMMIT
  -> release pre-serialized 201 response
```

The typed event target is the new client ID. Its bounded attributes are
registration-token ID, sanitized client name, auth method, canonical grant
tags, and URI counts. It contains no bearer token, token digest, client secret,
secret hash, full redirect URI, application URI, or arbitrary metadata.

Any scope drift, guard loss, stale decision, uniqueness failure, insert
failure, event-build failure, audit append failure, caught pre-commit panic, or
commit failure rolls back the token count, client row, and event together.
Success is not exposed to HTTP until the RFC 094 runner returns `Audited`.

Cancellation has a narrower contract. Cancellation known before dispatch
performs no database work. Once work is dispatched to the blocking database
worker, dropping the async waiter does not prove that queued work was canceled:
the closure may start, commit, or finish while no HTTP response remains.
A cooperative cancellation flag observed inside the transaction before commit
causes rollback, but the disconnected caller cannot infer that observation.
Cancellation during the worker, around commit, or after commit is therefore an
ambiguous outcome to the client and must never trigger an automatic retry.
Only a still-connected handler that receives the worker's explicit
confirmed-rollback result may emit a retryable 503. Worker telemetry and
durable reconciliation may classify an abandoned operation operationally; they
do not convert a lost response into client-visible certainty.

For a token with `n > 0` remaining uses, at most `n` concurrent transactions
commit. Each committed increment has exactly one disabled client and one
`client.dynamic_register` event. Unlimited tokens (`max_uses = 0`) retain the
same one-use/one-client/one-event correspondence without a finite winner cap.

## Response and error contract

Every registration response carries `Cache-Control: no-store` and
`Pragma: no-cache`. Success is HTTP 201 and returns:

- `client_id` and `client_id_issued_at`;
- `client_secret` plus `client_secret_expires_at: 0` for confidential clients;
- every registered supported metadata value after defaults/canonicalization;
  and
- no ignored extension member or internal compatibility tag.

The server prepares the exact success body before C15. The plaintext secret
exists only in zeroize-on-drop preparation/response memory, is returned only
after commit, and is never stored or logged.

| Condition | HTTP/protocol result | Durable effect |
|---|---|---|
| Missing, malformed, unknown, expired, revoked, exhausted bearer; guarded race loss | 401 `invalid_token` plus one fixed `WWW-Authenticate: Bearer` shape | None |
| Invalid redirect or logout URI | 400 `invalid_redirect_uri` | None |
| Other invalid/unsupported metadata or duplicate JSON member | 400 `invalid_client_metadata` | None |
| Unsupported/untrusted software statement | 400 `unapproved_software_statement` | None |
| Wrong media type / oversize body | 415 / 413 under local HTTP envelope policy, not RFC 7591 error codes | None |
| SQLite busy, stale/clock-invalid authority, internal serialization preparation, catalog configuration, or confirmed transaction failure | 503 `temporarily_unavailable` under local availability policy with bounded `Retry-After` | None |
| Success | 201 JSON | One use, one client, one event |

Error descriptions are fixed ASCII templates naming at most a field and array
index. They never echo a bearer, secret, URI, name, scope value, SQL message, or
token state.

## Retry and idempotency semantics

RFC 7591 registration POST remains non-idempotent in M3. This RFC does not add
an idempotency key or store replayable plaintext client secrets.

- A client may retry after a complete protocol response that explicitly
  reports a pre-commit validation error or a 503 confirmed rollback.
- A connection loss or timeout without a complete response is ambiguous: the
  client must not automatically retry, because the first request may have
  committed and a second valid request would consume another use and create a
  second client.
- Cancellation after database-worker dispatch is the same ambiguous class,
  including cancellation while the work is queued. Only pre-dispatch
  cancellation or an explicit worker result delivered to the handler proves
  that no commit occurred.
- Two sequential valid requests are two registrations even when their bodies
  are identical.
- Adding replayable/idempotent registration later requires a separate RFC that
  defines secret-response custody, retention, and cleanup.

This is intentionally honest rather than claiming idempotency that the current
credential model cannot provide.

## Security invariants

- Invalid metadata never consumes a use.
- For a token with `n` remaining uses, at most `n` concurrent valid requests
  commit; every committed use corresponds to exactly one disabled client and
  one audit event.
- No client row is reachable with a partially validated URI set.
- Failure after guarded consumption but before commit leaves all three durable
  resources unchanged.
- Audit data never contains registration bearer/digest, generated secret/hash,
  or full metadata URIs.
- Runtime grant/auth/redirect behavior cannot exceed the metadata returned at
  registration.
- HTTP loopback relaxation belongs only to a PKCE public-native profile;
  confidential and public-HTTPS redirects remain exact.
- A dynamic client without registered logout URIs gains no logout redirect by
  legacy fallback.
- A dynamic logout redirect requires a verified client/session-bound ID-token
  hint and matches one independently registered HTTPS value exactly.
- Missing, invalid, expired-outside-policy, or mismatched logout hints cannot
  mutate a session; local logout then requires explicit CSRF-protected POST
  confirmation and never redirects to the RP.
- A stale CORS snapshot can deny but cannot authorize after any relevant client
  mutation.
- Dynamically supplied logo URIs cause no automatic server or browser request.
- An ambiguous transport retry is never represented as safe or idempotent.

## Adversarial verification contract

The companion
[verification plan](../handoffs/095-dynamic-client-registration/verification.md)
is normative for implementation and closure. At minimum it covers:

- every supported/defaulted/rejected field and every cross-field combination;
- wrong JSON types, duplicate keys, unknown extensions, bounds, controls, and
  response echo behavior;
- canonical HTTPS, typed public/confidential/native profiles, Basic/POST
  loopback rejection, IPv4/IPv6 loopback, authorization-only variable loopback
  port, exact logout/query/path matching, fragments, userinfo, wildcard,
  `localhost`, private schemes, Unicode/IDNA, malformed percent encoding,
  duplicate, and oversize URI cases;
- migration/backfill for public, confidential, admin, disabled, and deleted
  historical rows, every non-derivable stamped profile, canonical guarded CORS
  generation/overflow, plus non-mutating legacy unstamped-candidate reporting
  and owner disposition;
- enforcement of Basic-only, post-only, none, allowed grants, PKCE, scope
  catalog, optional refresh issuance, generation-checked fail-closed CORS,
  HTTPS-only exact ID-token-hint logout legitimacy, GET/POST parsing,
  invalid-hint CSRF confirmation, no-logout-fallback, and no dynamic-logo fetch
  behavior;
- injected failure before/after guard, client insert, event build/append, and
  commit, with exact three-resource rollback reconciliation;
- compile-negative possession-proof isolation, mixed-context rejection, fresh
  transaction-clock expiry, stale queued authority, and cancellation at each
  dispatch/worker/commit/response boundary;
- N-way contention for remaining-use counts 1, 3, exhausted, revoked, expiring,
  and unlimited, plus concurrent operator revocation;
- explicit rollback retry success, sequential duplicate registration, and
  ambiguous response-loss NO-RETRY behavior; and
- fixed invalid-token status/header/body, sanitized logs/metrics, response
  cache headers, and secret-bearing buffer handling.

All applicable RFC 093 gates and RFC 094 C15 structural/event tests pass on one
clean commit. Independent closure review reconciles token counts, client rows,
audit-chain rows, response metadata, and observed command output.

## Implementation stages

1. Land the compatibility migration and typed client metadata model with
   upgrade/round-trip tests.
2. Extract pure bounded parsing, normalization, URI, scope, and compatibility
   validation with the complete table-driven corpus.
3. Extend token/authorization/logout runtime enforcement before allowing the
   new metadata to be registered.
4. Replace the baseline C15 inputs with `ValidatedRegistration`, guarded token
   consumption, one complete client insert, and typed response preparation.
5. Add failure-injection, contention, retry, error-shape, and secret/log
   evidence; run the full clean-tree gate matrix.

No stage may temporarily accept metadata that runtime endpoints do not yet
enforce. Schema readers land before writers, and runtime enforcement lands
before the registration handler can persist the new values.

## Alternatives rejected

- **Keep consuming before validation.** Rejected because invalid requests burn
  limited authorization and enable denial of service.
- **Accept but ignore grant/auth metadata.** Rejected because the response
  would assert controls that runtime does not enforce.
- **Validate then perform separate writes.** Rejected because a crash or audit
  failure can split use count, client, metadata, and evidence.
- **Allow custom native schemes now.** Rejected because sui-id has no claimed
  application identity or collision policy; numeric loopback and claimed HTTPS
  cover the approved M3 surface.
- **Canonicalize and silently rewrite redirect URIs.** Rejected because security
  comparison must preserve the exact registered string.
- **Claim automatic retry safety.** Rejected because a lost 201 response leaves
  the client secret unrecoverable without a new secret-custody protocol.

## Acceptance and closure evidence

Before this RFC may become Accepted, independent design review confirms the
metadata subset, URI policy, migration compatibility, C15 context/guard
sequence, response/error mapping, and retry boundary.

Before implementation starts, RFC 094 is Implemented, its C15 baseline and
clean-tree evidence pass, the implementation handoff has non-overlapping file
ownership, and the owner records authorization.

Before movement to `done/`, the exact clean commit, migration evidence,
validation corpus, failure matrix, contention reconciliation, runtime
enforcement, legacy-candidate disposition, sanitized logs, response headers,
and independent adversarial closure approval are repository-visible.
