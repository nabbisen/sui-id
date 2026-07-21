# RFC 096 implementation architecture

This companion is normative where it adds precision to the governing RFC. It
does not authorize implementation while RFC 096 is Proposed.

## Component boundaries

| Component | Owns | Must not own |
|---|---|---|
| `FederationProviderPolicy` | parsed canonical issuer/origins/ports/auth/algs/scopes/version and private canonical digest | network I/O or raw secrets |
| `FederationResolver` | bounded DNS lookup and special-range decision | HTTP redirects or document caching |
| `FederationTransport` | pinned TLS request, deadlines, byte/media/status envelope | semantic JSON/JOSE decisions |
| `DiscoveryValidator` | bounded metadata parsing and exact provider binding | origin expansion |
| `FederationDocumentCache` | versioned single-flight TTL/cooldown state | stale authorization |
| `LoginAttemptRepo` | create, atomic claim, terminal transition, cleanup | token/claim parsing |
| `UpstreamTokenClient` | exact code exchange and zeroizing response envelope | identity mapping |
| `IdTokenValidator` | compact JOSE, key selection, signature and claims | user/session lookup |
| `VerifiedFederatedIdentity` | private proof-bearing validated values | raw token/access token/nonce |
| `FederationMapper` | linked lookup and closed provisioning decision | unsigned claims or email merge |
| RFC 094 command services | provider/link/user/session durable mutation plus audit | protocol parsing/network I/O |

Construction flows inward. HTTP handlers may compose these capabilities but
cannot construct `VerifiedFederatedIdentity`, a pinned destination, or an
RFC 094 command context directly.

## Provider state machine

```text
legacy/config change -> disabled + requires_review + version increment
validated config     -> disabled + requires_review
successful preflight -> ephemeral ValidatedProviderPreflight only; no write
C17 consumes <600s exact-generation capability -> generation++ + evidence + enabled in one audited transaction
trust-field change   -> version++ + generation++ + disabled + requires_review + attempts/cache invalidated
preflight expiry/restart -> capability unusable/lost; rerun preflight
operator disable     -> generation++ + disabled + evidence cleared + attempts invalidated
provider delete      -> generation++ + evidence/attempts invalidated + deleted
```

Preflight fetches and validates discovery plus JWKS through the production
policy. It proves current reachability/config compatibility, not future trust;
every runtime document remains independently validated. It returns a sealed
provider/version/activation-generation/policy-digest/time/fingerprint capability, never a durable
`preflight_ok` row. C17 consumes it, captures its transaction clock, requires
age in `[0,600s)`, rechecks exact disabled state/version/generation/policy,
checked-increments generation, and stores evidence bound to the new generation
with enable/event. Preflight is available only while disabled and rechecks that
state after network validation. Failure/cancellation/restart writes nothing;
enable invalidates sibling capabilities, and disable/C23/C18 each increment the
generation and clear evidence. Re-enable always needs a later capability.

The private policy module alone computes the governing RFC's domain-separated,
fixed-field SHA-256 digest and compares its fixed 32 bytes in constant time.
Handlers cannot supply, log, serialize, or inspect it.

All provider trust-field writes use amended RFC 094 C23 and one repository
method accepting typed policy. It increments `config_version` and
`activation_generation` with checked arithmetic. Both are strict canonical
decimal-text `u64` values. Overflow,
malformed legacy state, invalid configuration, failed audit append, or failed
attempt invalidation rolls back. Cache eviction is a post-commit availability
action; cache keys bind version plus activation generation and ensure eviction failure cannot authorize old
state.

## Login attempt transitions

```text
start: create pending (expires = created + 600s)

pending --matching upstream error/atomic consume--> failed
pending --matching code/atomic claim-------------> exchanging
pending --expiry/provider version change----------> unusable -> cleanup

exchanging --verified mapping + final command-----> completed
exchanging --any error/cancellation--------------- > failed/reconciled failed
completed/failed --24h cleanup---------------------> deleted
```

The pending-to-exchanging UPDATE contains all authorization predicates and
requires one row. Reading then updating is forbidden. State and binding inputs
are hashed before the query and compared as fixed bytes; caller-visible errors
do not identify which predicate failed. Decrypt PKCE only after winning. The
state row is never reset or copied.

If cancellation occurs after the claim, the attempt remains non-retryable.
Cleanup may classify old `exchanging` attempts as failed after their deadline;
it must never release them for exchange. A callback outcome is ambiguous to a
disconnected browser, but retry always means a new start.

## Callback pipeline

```text
bounded query parse
 -> path slug lookup (generic unknown/disabled response)
 -> exact response iss
 -> state/browser/version atomic claim
 -> dedicated token exchange
 -> discovery/JWKS fresh cache lookup
 -> JOSE header/key/signature validation
 -> issuer/audience/time/nonce/sub claim validation
 -> VerifiedFederatedIdentity
 -> link lookup / closed provisioning decision
 -> local MFA gate preserving Fed primary
 -> RFC 094 durable command(s)
 -> fixed local redirect
```

No later phase may call an earlier phase with raw strings once a typed value
exists. The callback clears the binding cookie in its response builder even
when the internal error mapper fails.

Upstream `error` consumes the attempt after exact issuer/state/browser binding
and performs no token exchange. Its name/description/URI are not surfaced or
logged. A callback without a usable attempt follows the same browser result as
replay, expiry, wrong provider, and wrong binding.

## URL and egress mechanics

Canonical input validation occurs once at configuration/discovery boundaries.
Endpoint fetch accepts only `ApprovedEndpoint`, which contains original host,
port, path/query, provider version, and operator-origin membership proof.

Resolution returns `PinnedDestination` with at most eight normalized public
addresses and an expiry no later than the current request. Only the dedicated
connector can consume it. Connector APIs preserve original authority for Host
and TLS SNI/certificate checks while dialing a listed address. Redirect
responses are returned as errors, never passed through a generic redirect
policy.

The vendored IANA snapshot generator produces explicit CIDR/category tables
for both families plus a hand-reviewed explicit transition/translation/anycast
deny table. `Globally Reachable` is descriptive input, never sufficient allow
authority. Any match in either IANA special registry is denied. IPv4-mapped
IPv6 is normalized then evaluated as IPv4; NAT64 well-known/local-use, 6to4,
Teredo, IPv4-compatible, and every other explicit transition assignment are
denied without embedded-address dialing. Unit tests cover both boundaries of
every prefix, ordinary public peers, mapped addresses, and public/private/
metadata-embedded NAT64 values.

The generator makes new/changed registry rows `undisposed-deny` and causes the
release policy check to fail until review records a disposition. It never
automatically converts a new globally reachable special row to allowed.

## Bounded HTTP envelope

The transport is a minimal HTTP/1.1-only client over the pinned rustls stream,
not reqwest/generic hyper response decoding. ALPN offers only `http/1.1`; every
other negotiated/prologue version, informational chain, or upgrade rejects.
Each request uses `Connection: close`.

The workspace pins the exact rustls version, crypto provider, roots, TLS
1.2/1.3-only configuration, disabled early data/client authentication, and
ALPN. A metered socket aborts before handshake completion after 256 KiB inbound
encrypted bytes; the pinned rustls single-handshake-message limit must be
recorded and no larger. Surfaced chains are at most 16 certificates/128 KiB
DER. The implementation allocation manifest records source/version-derived
handshake, certificate, record, plaintext/ciphertext, and connection buffer
bounds. An inability to establish those bounds blocks implementation. TCP
connect is 3 seconds, TLS handshake 5 seconds, and the whole request 10 seconds.

Before constructing a response envelope, it reads status and fields into one
fixed 32 KiB buffer and parses with exactly 64 preallocated field slots; there
is no grow/retry. Status line is at most 1 KiB, names 64 bytes, values 8 KiB,
and semantic accounting is status plus `name + value + 4` per field. Bare LF,
obs-fold, controls, invalid grammar, incomplete/excess data, and duplicate
singleton Content-Type/Age/Date/ETag reject. Bounded Cache-Control lines alone
combine as one list. ETag is at most 256 grammar-valid bytes.

Framing accepts one canonical Content-Length, sole `chunked` transfer coding,
or connection close. Duplicate/conflicting length, length plus transfer coding,
stacked/unknown coding, chunk extension, and size line over 128 bytes reject.
For a declared length, reads are capped to the remaining bytes; after the exact
body the one-use TLS connection is dropped immediately without waiting for EOF
or reading later bytes. No later bytes can become authority or a pooled second
response. Chunk/body parsing uses fixed 8 KiB scratch
and streams into the document cap. The zero chunk must be followed by the
immediate empty CRLF trailer section; every trailer rejects. HTTP/2/HPACK is
absent, so no advisory peer setting stands between the socket and this hard
allocation boundary.

## Bounded JSON

Use one streaming or raw-token prepass that rejects duplicate object members
at every depth before typed deserialization. Limits apply to decompressed
bytes; unsupported content encoding is rejected, avoiding decompression bombs.
Number handling preserves integer-vs-float distinction. Unknown metadata is
discarded only after it contributes to envelope depth/member/string limits.

Separate typed envelopes exist for discovery, JWKS, token response, JOSE
header, and claims. A permissive general `serde_json::from_slice` directly on
network bytes is not an approved boundary.

## Cache implementation

Each entry contains provider ID/version/activation generation, validated typed document, fetch
monotonic time, parsed freshness metadata, optional bounded ETag, and no raw
response headers/body once unneeded. Locks are never held across await except
the single-flight primitive.

The closed cache-control parser and 200/304 precedence are exactly those in the
governing RFC. Freshness metadata is a typed value, not retained arbitrary
headers. Zero freshness remains zero; only positive lifetime receives an upper
clamp. Age/Date and monotonic resident time decide freshness. No stale response
ever authorizes on error.

For 200, validate the entire body even if ETag is unchanged, then publish only
while provider/version/generation remains current. For 304, require the requested exact
validator plus a retained cacheable validated body of the same version and generation;
replace present Cache-Control/Age/Date metadata, inherit only absent
Cache-Control, and apply no-store/no-cache/zero freshness exactly. A malformed
or ambiguous known cache field rejects the response and cannot extend the old
entry.

Ordinary and forced requests follow the governing state table. One flight per
provider/document exists. Unknown kid joins any active flight; that result is
its only refresh opportunity. During cooldown it fails without consuming
forced budget. Otherwise a forced dispatch bypasses fresh reuse, starts the
60-second budget at network dispatch, and starts cooldown on failure. A 304
leaves the key set unchanged, so an unknown kid fails. State maps are bounded
by configured provider/document, never attacker kid.

## JOSE type isolation

Use wrapper types equivalent to:

```text
UntrustedCompactIdToken -> ParsedProtectedHeader
ParsedProtectedHeader + ValidatedJwks -> SignatureVerifiedPayload
SignatureVerifiedPayload + AttemptClaims -> VerifiedFederatedIdentity
```

Constructors after the first are private to the validator. Types containing
untrusted payloads are non-`Debug` and cannot implement the mapping trait. The
verified identity is also non-serializable and exposes bounded accessors only.
Compile-negative fixtures prove that handlers/mappers cannot construct or
substitute these stages.

## Persistence and RFC 094 boundary

Migration adds provider policy/version/status, canonical-decimal checked
`activation_generation`, and last-successful-enable
preflight-evidence columns plus the
attempt table, dedicated `federation_mfa_pending`, and
`federation_webauthn_ceremony` with status/count/unique-parent/check/FK indexes.
It rebuilds a table rather than
pretending SQLite checks can be retrofitted if necessary. Upgrade tests start
from real pre-migration fixtures with linked users and sealed secrets.

Evidence columns are nullable and writable only inside C17 enable; they are
cleared by disable/C23/C18 and never queried as re-enable authority. No
`pending_preflight` or independently writable `preflight_ok` state exists.

Exact RFC 094 integration is frozen by the accepted complete amended design:

- C17 enable consumes a private exact-policy/version/generation preflight
  younger than 600 seconds and atomically increments generation + stores
  evidence + enables + audits; disable increments generation and clears
  evidence/attempts. No preflight writer exists. C23 alone replaces trust
  policy, checked-increments version and generation, forces disable, invalidates
  old attempts, clears evidence, and audits atomically; C18 similarly increments
  generation before delete; cache eviction is post-commit;
- C19 remains link-authority create/update and is never used for login
  observation;
- F01 is linked direct Protocol promotion; F02 is Fed-bound MFA pending; F03 is
  federated MFA Protocol promotion; F04 is Class-A first provisioning; F05 is
  Protocol terminal failure/denial; F06 is bound WebAuthn ceremony state; and
- last-seen/email observation is a private primitive only beneath F01/F03/F04.

F01/F03 use post-commit must-attempt Class-B sign-in success under RFC 094's
previously reviewed base-design U24/U30 policy; they do not claim atomic audit. F04 commits one
`auth.federation.provisioned` event with attempt/user/link/session. No command
nests U01/C19/U30 or exposes their subordinate writers. Implementing stage 5
is forbidden until the complete amended RFC is represented in the implemented
manifest and its gates pass.

## MFA context

Extend pending local MFA with a typed primary method and bounded continuation,
not a second unsigned cookie. Only the federation callback can construct the
federated primary continuation after validated mapping. MFA verification
consumes it once and emits the exact local authentication-method vector. A
local password continuation cannot be substituted because variants are sealed
and their stored purpose/provider linkage differs.

The row has five-minute expiry, shared 0–5 failure count, terminal status, and
provider/version/activation-generation/link/user/browser/CSRF bindings. F03 serializes every bound
wrong/correct outcome: wrong 1–4 remains pending, wrong 5 exhausts, correct at
0–4 promotes, drift/expiry invalidates. Terminal rows retain no usable
continuation and purge after 24 hours. Restart/method switching cannot reset
count.

TOTP step, recovery hash, and WebAuthn counter are private F03 subordinate
writes. F06's one `FederatedLogin` ceremony is parent-bound and expires no
later than the F02 row. Wrong WebAuthn consumes that ceremony plus one shared
failure; successful proof consumes it only with pending/session/counter. Proof
types are disjoint and compile-negative fixtures reject substitution. A private
`FederatedMfaVerifier` first establishes browser/CSRF/pending authority, then
alone constructs sealed method-specific valid or `BoundRejected` candidates
and selects the closed reason. Binding failure produces no candidate. Handlers
cannot construct either form or choose a row-touching reason.

## Migration rollout and recovery

Upgrade leaves federation globally safe because every legacy provider is
disabled. The report includes internal ID, slug, whether a link exists, and
missing policy fields but no secret, issuer query, email, or subject. The
operator supplies new configuration, observes preflight, and enables one
provider at a time.

Operational rollback is provider disable plus session policy chosen by the
operator; existing local sessions are not silently reclassified. Database
downgrade is unsupported after migration. If the new binary must be rolled
back, federation remains disabled rather than invoking the old unsigned-token
path.
