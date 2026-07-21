# RFC 095 implementation architecture

## Target components

| Component | Responsibility |
|---|---|
| `registration::parse` | Bounded JSON object parsing, duplicate-key detection, supported/unsupported member classification |
| `registration::validate` | Pure scalar, set, compatibility, and canonical URI validation |
| `RegistrationTokenAuthorizer` | Read-only opaque bearer decision that constructs sealed C15 authority |
| `PreparedDynamicRegistration` | Validated metadata, generated IDs, hashed secret, pre-serialized zeroizing response |
| C15 within-transaction helpers | Scope snapshot check, guarded use consumption, complete client insert |
| RFC 094 Class-A runner | Typed event construction, append/hash, commit, `Audited` return |
| Client runtime policy | Exact token auth method, allowed grants, typed redirect profiles, authenticated logout redirect |
| Redirect-origin cache | Generation-checked URL-parser-derived origins for enabled/non-deleted clients only |

The HTTP handler orchestrates these components but owns no raw SQL, transaction,
audit append, password-hash internals, or ad hoc URI policy.

## Typed data flow

```text
bounded body + bearer
  -> early opaque token probe
  -> ParsedRegistration
  -> ValidatedRegistration + ScopeCatalogSnapshot
  -> generated ClientId / optional Zeroizing<ClientSecret> / secret hash
  -> pre-serialized Zeroizing<ResponseBytes>
  -> AuthorizedCommandContext<C15DynamicRegistration>
  -> PreparedDynamicRegistration
  -> class_a<C15>
       scope snapshot equality
       guarded token UPDATE
       complete client INSERT
       typed DynamicRegistered event
       append/hash/commit
  -> Audited<RegistrationCommitted>
  -> release exact HTTP 201 bytes
```

Only the final response boundary exposes the raw client secret. Debug output is
redacted for every secret-bearing type.

## Schema contract

The next migration adds nullable columns:

```text
clients.token_endpoint_auth_method
clients.grant_types
clients.response_types
clients.redirect_profile
sui_meta.cors_policy_generation
```

Store types map them to:

```text
TokenAuthPolicy = LegacyAny | None | ClientSecretBasic | ClientSecretPost
GrantPolicy     = LegacyCodeAndRefresh | Code | CodeAndRefresh
ResponsePolicy  = LegacyCode | Code
RedirectProfile = ConfidentialHttps | PublicHttps | PublicNativeLoopback
```

`NULL` belongs to administrator-created legacy rows and reported legacy dynamic
rows whose original intent cannot be safely reconstructed. Pre-RFC-095 dynamic
rows receive the safe explicit backfill values described by the RFC. New
dynamic inserts must supply all four client-policy fields. Database checks
reject unknown tags and malformed JSON, while typed construction and round-trip
tests enforce the closed sets.

Migration tests cover upgrade from schema 38, repeated startup, backup restore,
public/confidential dynamic rows, every non-derivable stamped dynamic profile,
admin rows, disabled/deleted preservation, and corrupt/unknown metadata
rejection.

`sui_meta.cors_policy_generation` is canonical decimal text: `0` or
`[1-9][0-9]*`, at most `u64::MAX`. Relevant Class-A mutations strictly parse
it, use checked increment, and guard replacement on the exact old text.
Missing, malformed, negative, noncanonical, changed, or overflowed state aborts
the mutation. CORS reads treat the same states as denial; no coercion or wrap is
permitted.

Rows stamped `admin` before RFC 094 are never automatically reclassified.
A read-only report may correlate positive `client.dynamic_register` audit
targets to those rows and label candidates, but best-effort historical audit
absence cannot prove admin provenance or a complete zero set. Owner disposition
is separate audited work and closure evidence.

## Runtime ordering

Readers and compatibility enum variants land first. Token endpoint enforcement
then distinguishes:

- Basic credentials;
- body credentials;
- public `none`; and
- legacy any-secret transport.

It rejects simultaneous credential sources and checks the client's grant policy
before dispatch. Code-only dynamic clients omit refresh issuance; code+refresh
clients issue it only for an authorization carrying `offline_access`.
Authorization matching then adopts the persisted profile: only
`PublicNativeLoopback` relaxes a numeric-loopback port, while confidential and
public HTTPS remain exact. Basic/POST loopback registration is impossible.
Dynamic logout URIs are independently HTTPS-only and always exact; the
authorization profile never supplies their scheme or matching rule. Logout
never falls back and redirects only after a verified client-bound
`id_token_hint` (plus active-session subject/`sid` agreement when applicable).
GET query and form-encoded POST share one bounded parser. Missing/invalid/
mismatched hints cannot mutate a session: they render local confirmation, and
only a subsequent CSRF-protected explicit POST may perform local logout with no
RP redirect. The five-minute expired-hint grace is inclusive at 300 seconds,
logout-only, and requires a matching active session.

The CORS cache uses the same parsed origin model, excludes disabled/deleted
clients, and must match the current strictly parsed durable generation. Only
after those paths are active may the registration writer persist exact
metadata.

## Token authorization and C15

The read-only authorizer executes one indexed predicate for all invalid token
states. An early probe gates metadata parsing; a second probe immediately
before C15 returns either a private context or a generic denial. Its sealed C15
authority owns both token ID and a non-cloneable, non-debuggable,
non-serializable, zeroize-on-drop possession digest constructed from the same
query result. HTTP and event code cannot access or separately substitute it.
The raw bearer never enters the context.

After `BEGIN IMMEDIATE`, the worker's sealed injectable transaction clock
captures `guard_at`. It rejects clock regression or a decision older than five
seconds and uses `guard_at`, not the pre-transaction time, for expiry and
guarded update. The C15 closure requires:

1. every explicit scope still exists, or an omitted-scope default set still
   equals the validated snapshot exactly;
2. guarded token update affects exactly one row;
3. complete client insert affects exactly one row;
4. typed event construction succeeds; and
5. the RFC 094 runner appends and commits.

All application-identity/source fields are part of the single insert. The
dynamic path cannot call general post-insert setters.

## Error ownership

Pure validation returns typed field/index errors. The HTTP adapter maps them to
fixed RFC 7591 codes without attacker-controlled text. Store errors cross the
boundary only as opaque retryable/non-retryable categories. SQL, URI, name,
scope, token, and secret values are never formatted into public errors.

The adapter adds no-store headers to success and errors. A complete 503 means
the transaction is known rolled back. Transport loss is explicitly ambiguous.

## Cancellation and memory

Pre-dispatch cancellation performs no work. After dispatch, dropping the async
waiter does not prove that queued or running blocking work stopped. A
cooperative flag observed before commit rolls back, but cancellation during
the worker, around commit, or after commit remains ambiguous to a disconnected
caller. Only a worker-confirmed rollback delivered to the live handler permits
a retryable 503; no ambiguous cancellation is automatically retried. No async
suspension occurs inside the rusqlite transaction. Secret, possession digest,
secret-hash inputs where practical, and serialized response bytes use
redacting zeroize-on-drop wrappers. Audit/event/log types cannot accept them.

## Compatibility boundary

Existing admin rows keep legacy token behavior. Existing dynamic confidential
rows use the visible `legacy_secret_any` backfill because their original Basic
versus POST request was discarded. Existing dynamic public rows become `none`.
All remain disabled/enabled/deleted exactly as before.

For every positively stamped legacy dynamic row, the complete stored
auth/redirect set must derive exactly one profile. Any zero/multi-profile row,
including public mixed schemes, confidential loopback, malformed, or
noncanonical data, retains NULL, is reported, and is runtime-ineligible without
status mutation.

Here “existing dynamic” means a row durably stamped dynamic by the RFC 094
baseline. Older unstamped candidates remain legacy admin rows until owner
review; shape-based automatic conversion is forbidden.

New dynamic clients never receive a legacy policy. Operator-driven narrowing of
backfilled clients is follow-up administration work, not an automatic migration.

Because C15 creates a disabled client, it does not add the new origins to CORS.
Every enable, disable, delete, restore, or redirect-origin change performs the
guarded checked increment of the canonical durable generation in the same
audited transaction. A cache snapshot carries the strictly parsed generation
read with its rows, and middleware checks it against the database before
allowing every preflight/token origin. Missing/malformed/noncanonical values,
overflow during mutation, missing snapshots, generation mismatch, read
failure, and rebuild failure deny or abort as appropriate. This makes stale
enablement a denial and stale revocation incapable of authorizing.

CORS origin comparison always includes the exact port. The RFC 8252
variable-port exception belongs only to `PublicNativeLoopback` authorization
redirect matching and never applies to logout or CORS.

Dynamic `logo_uri` values are stored/echoed but never rendered as browser
subresources. Dynamic application links require an explicit user click,
`noopener noreferrer`, and no referrer.

## Frozen logout-confirmation constants

These names and boundaries are fixed before implementation:

```text
LOGOUT_REQUEST_MAX_ENCODED_BYTES = 16_384
LOGOUT_REQUEST_MAX_MEMBERS       = 8
LOGOUT_REQUEST_MAX_SCALAR_BYTES  = 8_192
LOGOUT_CONFIRMATION_TTL_SECS     = 300
LOGOUT_CONFIRMATIONS_PER_SESSION = 1
LOGOUT_CONFIRMATION_TOKEN_BYTES  = 32
```

The whole-request bound applies independently to the raw GET query or POST form
body before unbounded decoding. The member bound applies after form decoding;
the scalar bound applies to every decoded name/value. GET and POST then enter
the same duplicate/cardinality and hint-decision function. Query/body security
parameters cannot be combined.

The confirmation capability is 32 CSPRNG bytes encoded as unpadded base64url
for one hidden form value. The raw value appears only in that no-store HTML
form and the confirmation POST body: it is never placed in a URL, cookie,
database, log, metric, audit event, `Debug`, or error. The database stores only
its SHA-256 digest with session ID, fixed `local_logout_confirmation` purpose,
creation time, and expiry. Secret-bearing handler values redact and zeroize on
drop. The existing session cookie remains HttpOnly, and the form also carries
the normal same-origin CSRF field/cookie check.

`expires_at = created_at + LOGOUT_CONFIRMATION_TTL_SECS`; validity is
`now < expires_at`, so equality is expired. Creating a confirmation uses one
immediate transaction to delete any prior confirmation for that
session/purpose and insert the replacement, enforcing the maximum of one.
Expired rows are removed on startup, periodically, and opportunistically;
session deletion cascades their removal.

Confirmation consumption and the selected local session-logout mutation share
one immediate sealed protocol transaction. A guarded
`DELETE ... RETURNING` must match digest, session, purpose, and
`expires_at > now` exactly once before session mutation. Simultaneous duplicate
POSTs therefore have at most one winner; a zero-row loser performs no new
logout mutation and receives the same local expired/already-used result.
Neither the raw token nor its digest enters RFC 094 event/log payloads.

The confirmation page and result preserve `Cache-Control: no-store`,
`frame-ancestors 'none'`, and `X-Frame-Options: DENY`. Evidence covers exact
expiry, replacement, cleanup, cross-session/purpose attempts, duplicate
concurrency, header preservation, and secret absence.
