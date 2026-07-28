# RFC 096 — Upstream OIDC Federation Validation

**Status.** Proposed
**Security review.** Required
**Lifecycle history.** Accepted 2026-07-21 after [independent review](../reviews/096-design-review-2026-07-21.md); **returned to Proposed on 2026-07-28** for the prerequisite and staging amendment below, per RFC 000's return-for-review rule for material prerequisite changes. The 2026-07-21 acceptance is preserved in history and is superseded, not withdrawn. No validation, transport, JOSE, or claim-handling design is reopened.
**Amendment summary (2026-07-28).** Implementation prerequisite re-pointed from full RFC 093 to M1a; the previously inline validation-versus-mutation caveat promoted into two normative stages, 096-A and 096-B; preparatory `federation.rs` split added as a prerequisite; file ownership against RFC 094 named. Requested by `@nabbisen` on 2026-07-28 on the recommendation of the requirements architect, to allow federation work to run as an independent lane.
**Design prerequisites.** RFC 093 Accepted; amended RFC 094 Accepted including C17/C18/C23/F01–F06 federation commands; the federation threat delta, discovery/egress policy, and hostile-provider plan require independent design approval.
**Implementation prerequisites.** M1a complete — RFC 093's Rust gate lanes G01–G09 pass on one clean commit under the amended Gate Matrix; this RFC Accepted in its amended form; the hostile-provider harness approved; the preparatory `federation.rs` split committed and independently reviewed. Stage 096-A may then begin. Stage 096-B additionally requires RFC 094 **M2a** Implemented with the C17/C18/C23 seam proven. **M1b is not a prerequisite for either stage** — no part of this RFC depends on the documentation or RFC-integrity gates.
**Closure prerequisites.** Per stage. **096-A:** discovery, transport, JOSE, claims, state/nonce, and cache/rotation evidence, including the hostile-provider corpus, pass independent review. **096-B:** federation mutation commands on the Class-A seam with rollback evidence, session integration, migration, and representative live-integration evidence pass independent closure review.
**Tracks.** ROADMAP M4-A — Federation validation and transport; M4-B — Federation mutation and session integration.
**Touches.** Federation provider configuration and handlers; dedicated outbound HTTP; discovery/JWKS cache; JOSE/ID-token validation; login attempts and MFA context; provider/link repositories; RFC 094 C19 and amended C17/C18/C23/F01–F06 seams; migrations; threat model; integration tests.
**Handoff.** [`../handoffs/096-upstream-oidc-federation/README.md`](../handoffs/096-upstream-oidc-federation/README.md)
**Validation matrix.** [`../handoffs/096-upstream-oidc-federation/validation-matrix.md`](../handoffs/096-upstream-oidc-federation/validation-matrix.md)
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Implementation owner.** `codex-developer` (OpenAI Codex); implementation start remains gated below.
**Independent security and closure reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex).

## Summary

Replace the shipped federation path's trust-on-TLS token decoding and
cookie-replay state with a complete OIDC relying-party trust boundary. Every
enabled provider has a reviewed canonical issuer, endpoint-origin allowlist,
token authentication method, and asymmetric ID-token algorithm allowlist.
Discovery and JWKS travel through a dedicated redirect-free, proxy-free,
DNS-pinned public-Internet transport. A callback wins one durable login attempt
before code exchange, requires provider-specific and issuer response binding,
and produces a typed verified identity only after JOSE and all required claims
pass.

The milestone intentionally narrows RFC 004. ID tokens are mandatory; userinfo
fallback is removed. An unknown `link_only` identity receives a generic denial
until a separate RFC supplies a complete local-authentication, CSRF, and
approval flow. No private-network issuer exception, upstream-MFA trust, group
sync, or new provider protocol is introduced.

## Implementation stages

Added by the 2026-07-28 amendment. The staging is not new policy: the previous
prerequisite line already permitted validation and transport work to precede
RFC 094 implementation. This makes the boundary normative and reviewable rather
than an inline caveat, so federation can run as an independent lane.

### 096-A — validation and transport

Discovery and egress/SSRF policy; HTTPS enforcement and exact issuer binding;
JWKS retrieval, key selection, algorithm constraint and signature verification;
required ID-token claim validation; the mandatory one-time nonce; bounded cache
and key-rotation behaviour; and the hostile-provider and token-substitution
corpus.

**096-A performs no durable mutation.** It may not create, enable, disable or
delete a federation provider or link, and it may not establish a session. If a
change appears to require one, it is 096-B work and waits for the RFC 094 seam.

This stage carries essentially all of the security value of this RFC: it closes
the current defect in which the callback accepts an ID token whose signature was
never verified and whose nonce may be absent. It needs no part of the audit
transaction seam, which is why it is separable.

### 096-B — mutation and session integration

Federation provider and link commands (C17/C18/C23) implemented on the RFC 094
Class-A transaction seam, and session establishment from a verified assertion.
Requires RFC 094 M2a Implemented.

### File ownership

`crates/sui-id/src/http/handlers/federation.rs` currently contains both callback
validation and provider/link mutation logic. In its combined form it is owned by
neither this RFC nor RFC 094, and concurrent editing by both lanes risks a merge
that silently drops a security check. It is split by a preparatory change owned
by neither lane, with zero behaviour change, before either lane begins.

After the split, this RFC owns the discovery, JOSE/JWKS, claim-validation and
callback modules; RFC 094 owns the federation mutation commands. The split is a
prerequisite recorded in the implementation prerequisites above.

## Standards and local policy

The normative protocol basis is:

- [OpenID Connect Core 1.0](https://openid.net/specs/openid-connect-core-1_0-18.html)
  for authorization-code flow, ID-token validation, nonce, audience, and
  authorized-party rules;
- [OpenID Connect Discovery 1.0](https://openid.net/specs/openid-connect-discovery-1_0-final.html)
  for exact issuer metadata and endpoint discovery;
- [RFC 8725](https://www.rfc-editor.org/rfc/rfc8725.html) for explicit algorithm
  verification, key separation, and hostile JOSE input;
- [RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html) for current OAuth
  security practice and authorization-server mix-up defence; and
- [RFC 9207](https://www.rfc-editor.org/rfc/rfc9207.html) for authorization
  response issuer identification.

Those specifications permit broader deployment choices. This RFC defines a
closed sui-id profile. A compliant upstream that cannot satisfy the profile is
not enabled; interoperability pressure does not create a runtime bypass.

## Current implementation defect

The current callback base64-decodes ID-token claims without verifying a JOSE
signature and treats TLS delivery as token authenticity. It accepts an absent
ID token and falls back to userinfo. Nonce is checked only when present, while
issuer, audience, `azp`, expiry, issuance time, algorithm, key, and signature
are not checked. Discovery/token/userinfo use the shared HTTP client with
default redirect/proxy/DNS behavior and unbounded JSON decoding.

State, nonce, and PKCE verifier are recoverable from a signed but reusable
browser cookie for ten minutes. Callback consumption is not a durable
single-winner operation. A shared callback path lacks a provider-specific
redirect boundary. The incomplete link path stores unsigned identity JSON in
a browser cookie. Generic pending MFA changes a federated primary method into
password authentication after challenge completion, and `AuthMethod::Fed` is
currently treated as a second factor.

RFC 004's durable mapping key and non-merging principles remain correct. This
RFC supersedes its transport, validation, state-cookie, optional-userinfo,
link-skeleton, and authentication-method mechanics where they conflict.

## Security invariants

- **F1 — verified identity only.** Mapping, provisioning, MFA, and session code
  can receive only a private `VerifiedFederatedIdentity` constructed after
  successful signature and claim validation.
- **F2 — exact provider binding.** Configured issuer, discovered issuer,
  callback path provider, response `iss`, login-attempt provider, and ID-token
  `iss` identify the same provider exactly.
- **F3 — constrained egress.** Production discovery, JWKS, and token exchange
  connect only to operator-approved public HTTPS origins at DNS answers checked
  and pinned for that request. Redirects and ambient proxies are absent.
- **F4 — one callback winner.** State, browser binding, nonce, PKCE verifier,
  provider version, and redirect URI belong to one expiring durable attempt.
  Exactly one callback may claim it, and any post-claim failure requires a new
  login attempt.
- **F5 — closed JOSE profile.** Only configured asymmetric algorithms and
  compatible local JWKS keys are accepted. Unsigned, symmetric, encrypted,
  indirectly keyed, ambiguous, or malformed tokens fail closed.
- **F6 — stable identity.** `(provider_id, sub)` is the only automatic lookup
  key. Email never merges accounts. Provisioning requires present verified
  email and a non-collision.
- **F7 — local MFA remains local.** Federation is a primary authentication
  method, not a local second factor. Upstream `amr`, `acr`, and `auth_time`
  never satisfy local MFA or step-up policy.
- **F8 — bounded cache failure.** Cache expiry, rotation, and unknown `kid`
  cannot create stale-key acceptance or an attacker-controlled fetch loop.
- **F9 — no upstream credential custody.** Authorization codes, client
  secrets, PKCE verifiers, ID tokens, and access tokens are never logged;
  upstream tokens are never persisted.
- **F10 — atomic durable effects.** User/link/provider/session mutations use
  the applicable RFC 094 command transaction. A Class-A security mutation
  cannot commit without its required typed audit event; F01–F03/F05/F06 retain
  their explicitly reviewed Protocol/Class-B classification without claiming
  atomic audit.

## Provider trust configuration

An enabled provider is a versioned immutable security snapshot containing:

| Field | Closed rule |
|---|---|
| `issuer` | 1–2,048 ASCII bytes; canonical absolute HTTPS URL; no userinfo, query, or fragment; nonempty host; path allowed; no IP literal; no `localhost`, `.localhost`, `.local`, `.internal`, or `.home.arpa`; exact serialized value is authoritative |
| `client_id` | 1–512 UTF-8 bytes; no controls; compared byte-for-byte |
| `client_secret` | Environment-indirected, AAD-sealed at rest; required only for a confidential provider |
| `token_auth_method` | Exactly `client_secret_basic` or `none`; `client_secret_post` is not in the M4 profile |
| `scopes` | Unique closed subset of `openid email profile`; `openid` mandatory; at most three tokens |
| `id_token_algs` | Nonempty subset of `RS256`, `PS256`, `ES256`, `EdDSA`; default and recommended singleton `RS256`; at most four |
| `endpoint_origins` | 1–8 canonical HTTPS origins; issuer origin included; explicit operator trust, never learned as authority from discovery |
| `https_ports` | Nonempty subset of 1–65535, at most eight; default `[443]`; every issuer/endpoint origin port included |
| `provision_mode` | Exactly `link_only` or `provision_on_first_login` |
| `config_version` | Checked monotonic `u64`; any trust-field change increments it and invalidates attempts/caches |
| `activation_generation` | Durable checked monotonic `u64`, independent of `config_version`; every enable, disable, policy replacement, and delete invalidates earlier activation authority |

Canonical URL comparison is performed by one typed parser. Inputs must already
equal the parser's ASCII serialization; the system rejects rather than silently
rewrites case, default ports, dot segments, Unicode host spellings, malformed
escapes, backslashes, or trailing-slash aliases. An origin is exactly scheme,
ASCII host, and effective port. Endpoint paths and queries may differ; endpoint
fragments and userinfo are forbidden.

Private, loopback, link-local, or literal-IP issuers are not supported in M4.
There is no configuration escape hatch. A deployment needing an internal IdP
requires a separate RFC defining its network trust, certificate, DNS, and
operator-risk model.

Provider configuration is authoritative from validated application config;
admin UI may enable or disable but may not mutate trust fields. Startup sync
must update changed rows rather than silently preserving older values. A
change uses amended RFC 094 C23: it writes a checked new version, disables the
provider, checked-increments its activation generation, clears preflight
evidence, invalidates outstanding old-version login attempts plus
federated-MFA continuations/ceremonies, and appends its typed
audit event in one Class-A database transaction. Cache eviction is a
post-commit availability side effect. Version/generation/enabled-state checks ensure an
eviction failure cannot authorize an old entry. Re-enabling requires a
successful discovery/JWKS preflight for that exact version immediately
consumed by C17 audited enablement.

Both counters use canonical decimal text (`0` or `[1-9][0-9]*`) in SQLite,
strictly parse to `u64`, and use checked increment plus an exact-old-text guard;
malformed, noncanonical, missing, or `u64::MAX` state fails closed without a
mutation or event. New and migrated providers start at generation zero. The
generation is activation/revocation authority, whereas `config_version`
identifies the trust policy; neither counter substitutes for the other.
Provider IDs are generated from a globally fresh, unpredictable identity space
and are never reused after C18 deletion. Operator-supplied or deterministic
provider IDs are forbidden. Any future recreation scheme requires an RFC that
preserves a durable tombstone/incarnation binding.

Startup reconciliation preserves the existing sealed client-secret envelope
when the configured plaintext secret is unchanged. It compares/reseals only
inside non-debuggable zeroizing types and never exports plaintext or digest
material. Randomized resealing is not itself a policy change and must not cause
version/generation churn on every startup.

Preflight has no separately authoritative durable state. Successful production
transport/metadata/JWKS validation returns a private, non-cloneable,
non-debuggable, non-serializable `ValidatedProviderPreflight` containing
provider ID/version, the exact disabled-state activation generation, a digest
of the complete durable policy, trusted
`observed_at`, and bounded validated metadata/key-set fingerprints. Failure or
cancellation returns no capability and performs no write.

Preflight is permitted only for a non-deleted, fully configured, disabled provider. It
captures version, generation, and policy before network I/O and, after all
validation, re-reads them and the disabled state before its private constructor
can return a capability. An enable racing that final read is still rejected by
C17's authoritative transaction guard. An enabled provider therefore cannot
produce a preflight capability.

The policy digest is SHA-256 over the domain
`sui-id/federation-provider-policy/v1\0` followed by a fixed-order binary
encoding of issuer, client ID, sealed-client-secret-envelope SHA-256 (explicit
absent/present tag), token-auth-method tag, scopes, ID-token algorithms,
endpoint origins, HTTPS ports, and provision mode. Each scalar is encoded as a
one-byte field tag, a big-endian `u32` byte length, and exact canonical bytes;
lists use their field tag, a big-endian `u16` item count, and independently
length-prefixed canonical items sorted by the closed enum order or canonical
bytes; ports are sorted big-endian `u16` values. Empty and absent are distinct.
Enabled/review status, both counters, evidence, and display labels are excluded
because they are bound separately or are not trust policy. The digest is
computed only by the private policy module from typed durable/proposed policy,
compared as exactly 32 bytes in constant time, and is never accepted from a
caller, serialized, logged, audited, or exposed through metrics. Changing this
schema requires an RFC amendment and disables/reviews affected providers.

C17 enable consumes that capability. After `BEGIN IMMEDIATE`, its trusted clock
requires `0 <= guard_at - observed_at < 600 seconds`; exactly 600 is stale. It
rechecks the provider is disabled, non-deleted, exact version/policy digest,
exact activation generation, and still configured. It checked-increments that
generation and atomically stores evidence bound to the new generation, enables,
and appends the event. Thus the first of multiple same-generation capabilities
invalidates every sibling capability. Disable requires a currently enabled row,
checked-increments generation, disables, clears evidence, invalidates attempts,
federated-MFA continuations, and ceremonies, and appends its event in the same
transaction. Disable of an already-disabled row is a non-committing guarded
result and cannot burn generations.

C23 checked-increments both policy version and activation generation. C18
delete checked-increments activation generation before marking/removing the row
and invalidates the same evidence and flows; a missing/deleted row cannot match
a capability. Events contain only old/new version or generation and bounded
internal counts, not policy/upstream data. Any counter overflow, predicate loss,
invalidation failure, event/audit failure, cancellation, or commit failure
rolls back every effect. Concurrent preflights may exist in memory, but exactly
one same-generation enable can win; stale/replayed/wrong-policy/wrong-generation
input rolls back without an enable event. Restart loses the capability. Stored
evidence documents only the winning generation and cannot authorize re-enable.

## Dedicated federation transport

Federation does not use the general-purpose `AppState.http_client`. A dedicated
client has:

- TLS certificate and hostname verification enabled, minimum TLS 1.2;
- the exact `rustls` crate/version, crypto provider, root store, and configuration
  are workspace-locked; TLS 1.2/1.3 only, early data and client authentication
  disabled, and the implementation evidence records the effective handshake,
  certificate-chain, message, record, and plaintext/ciphertext buffer bounds;
- HTTP/1.1 only: TLS ALPN offers only `http/1.1`; HTTP/2, HTTP/1.0, upgrade,
  informational response chains, and protocol mismatch are rejected;
- redirects disabled for discovery, JWKS, and token exchange;
- environment/system proxies disabled and no cookie store, ambient
  authentication, client certificate, or global authorization header;
- connect timeout 3 seconds, response-header timeout 5 seconds, and total
  request deadline 10 seconds; and
- a fixed 32 KiB HTTP/1.1 status/header read buffer and fixed 64-field parser
  slots, with no grow/retry allocation; status line at most 1 KiB, field name
  64 bytes, field value 8 KiB, and semantic total status plus
  `name_bytes + value_bytes + 4` per field at most 32 KiB; and
- response streaming into the endpoint-specific byte limit before parsing.

The bounded HTTP/1.1 parser operates before a normal response object is
constructed. It reads only until the first complete `CRLF CRLF` within the
fixed buffer and rejects bare LF, obsolete folding, controls, invalid syntax,
too many fields, or any over-limit line/value. The client sends
`Connection: close`, performs no pooling, and supports exactly one final
response.

Body framing is closed: exactly one canonical `Content-Length`, exactly one
`Transfer-Encoding: chunked`, or connection-close delimitation; duplicate
Content-Length, Content-Length plus Transfer-Encoding, other/stacked transfer
codings, and invalid framing reject. For Content-Length, each read is capped at
the remaining declared bytes; immediately after exactly that many bytes the
one-use TLS connection is dropped without waiting for EOF or reading any later
bytes. Such discarded bytes are never response authority and cannot become a
second response because pooling/reuse is absent. The chunk parser has a
128-byte size-line cap, accepts canonical hexadecimal size
without extensions, streams decoded bytes into the document cap, and after the
zero chunk requires the immediate empty `CRLF` trailer section. Every trailer,
even a declared or otherwise valid one, is rejected. Read scratch is fixed at
8 KiB. A metered transport aborts before TLS establishment after 256 KiB total
encrypted inbound handshake bytes. The pinned rustls source limit for one
handshake message must be recorded and must not exceed that cap; otherwise the
implementation is blocked pending a lower bounded configuration/library.
After handshake, its fixed/library record buffers and the 8 KiB application
scratch are recorded in the transport allocation manifest. The surfaced peer
certificate chain is additionally limited to 16 certificates and 128 KiB DER.
TCP connect is at most 3 seconds, TLS handshake at most 5 seconds, and the
existing 10-second total plus header/body deadlines apply. Wire-level limit
rejection therefore occurs before
unbounded library header/HPACK/field-list allocation. HTTP/2 is deliberately
unsupported rather than relying on advisory peer settings.

Before each request, the resolver obtains at most eight A/AAAA answers for the
exact endpoint host. IPv4-mapped IPv6 is normalized to IPv4 and evaluated by
the IPv4 policy. Every answer—not merely one chosen answer—must pass this
closed predicate:

1. it is valid unicast and matches no prefix at all in the vendored IANA IPv4
   or IPv6 special-purpose registry (all registry flags, including globally
   reachable rows, are denied); and
2. it belongs to no separately vendored translation, tunnelling, transition,
   anycast, benchmarking, or protocol-assignment deny prefix.

The second table is defence in depth and includes at least IPv4-mapped and
IPv4-compatible IPv6, both NAT64 prefixes `64:ff9b::/96` and
`64:ff9b:1::/48`, 6to4 `2002::/16`, Teredo `2001::/32`, documentation,
discard-only, benchmarking, shared/CGNAT, loopback, link-local, private-use,
multicast, unspecified, reserved, and known metadata ranges. M4 rejects both
public-embedded and private/link-local-embedded NAT64 addresses; it does not
attempt translation-aware extraction/dialing. Ordinary public IPv4 and IPv6
outside all special/explicit prefixes are accepted.

IANA `Globally Reachable=true` is not by itself an allow decision. A newly
added or changed special allocation enters the generated policy as denied
until an explicit reviewed disposition changes that generated row; the
registry-update check fails the release gate on undisposed drift. Snapshot
identity, generator, explicit table, and dispositions are reviewable. This
conservatively denies some globally reachable special anycast/transition
services that are unsuitable as federation web origins.

Operator documentation must state the deployment consequence: sui-id needs
native IPv4 egress for an IPv4-only provider or provider-native ordinary public
IPv6. IPv6-only/DNS64/NAT64 egress cannot reach an IPv4-only upstream under M4,
and there is intentionally no NAT64 compatibility switch.

The connection is pinned to one of those already validated addresses while
retaining the original hostname for TLS SNI and certificate validation. The
HTTP stack must not perform a second unvalidated DNS lookup. Failure of all
validated addresses fails the request. DNS is re-resolved and revalidated for
every network fetch; cache entries cache documents, never permission to reuse
an old address. Production code has no test-only private-address switch. The
hostile fixture injects a transport trait beneath policy instead.

## Discovery profile

Discovery appends `/.well-known/openid-configuration` after the issuer path as
OIDC Discovery specifies. Thus `https://id.example` derives
`https://id.example/.well-known/openid-configuration`, while
`https://id.example/tenant/a` derives
`https://id.example/tenant/a/.well-known/openid-configuration`. The RFC
8414-style `https://id.example/.well-known/openid-configuration/tenant/a` form
is rejected for that path issuer. Only a 200 response with `application/json`
or an equivalent `+json`
media type is parsed. The decoded body is at most 64 KiB, depth 16, 128 object
members per object, 32 array members, and 2,048 bytes per string; duplicate
keys at any depth are rejected.

Required metadata and validation are:

| Member | Rule |
|---|---|
| `issuer` | Required and byte-exact with configured canonical issuer |
| `authorization_endpoint` | Required canonical HTTPS URL; approved origin/port |
| `token_endpoint` | Required canonical HTTPS URL; approved origin/port |
| `jwks_uri` | Required canonical HTTPS URL; approved origin/port |
| `response_types_supported` | Contains exactly usable `code`; advertised extras ignored within bounds |
| `grant_types_supported` | If present, contains `authorization_code` |
| `code_challenge_methods_supported` | Required and contains `S256` |
| `authorization_response_iss_parameter_supported` | Required and exactly `true` |
| `token_endpoint_auth_methods_supported` | Contains the configured method; `none` required for public clients |
| `id_token_signing_alg_values_supported` | Intersects configured algorithms; runtime uses the intersection only |
| `subject_types_supported` | Contains `public`; pairwise-only providers are outside M4 |

Discovered endpoints do not expand trust. Every endpoint must match an
operator-configured origin and port. Discovery redirect, origin substitution,
scheme downgrade, credential-bearing URL, or noncanonical URL fails the whole
document. Unknown bounded metadata is ignored.

## Authorization start and callback binding

The authorization request uses a provider-specific redirect URI:
`/auth/federated/{slug}/callback`. The legacy shared callback never exchanges a
code; after migration it returns only the generic local failure response.

Start creates these independent 32-byte CSPRNG values, base64url without
padding: `state`, `nonce`, PKCE verifier, and browser binding. PKCE is always
S256. The browser receives only the opaque binding in a Secure, HttpOnly,
SameSite=Lax cookie scoped to the provider callback. Authorization parameters
are constructed with a URL serializer and include `response_type=code`, exact
client ID/redirect URI, configured scopes, state, nonce, and S256 challenge.

`next` is optional and stored only after parsing as a canonical same-origin
absolute-path reference. It must start with one `/`, contain no scheme,
authority, backslash, fragment, control, or encoded separator ambiguity, and
serialize identically. Invalid input becomes the fixed local post-login target;
it is never reflected to the upstream.

The next migration creates a `federation_login_attempt` equivalent to:

```text
id, provider_id, provider_config_version, provider_activation_generation,
state_sha256 UNIQUE, nonce_sha256, browser_binding_sha256,
pkce_verifier_sealed, exact_redirect_uri, next_path,
created_at, expires_at, status(pending|exchanging|completed|failed), claimed_at
```

The verifier is sealed with fresh nonce and AAD binding attempt ID, provider ID,
config version, and activation generation. Hashes are raw fixed-size values. Attempt lifetime is 600
seconds: `expires_at <= now` is expired. Wall-clock access is injected and
clock regression fails closed.

Callback query is at most 8 KiB and 32 parameters. Duplicate security
parameters, malformed percent encoding, mixed `code` and `error`, or missing
`state`/`iss` fail before network access. A valid response has exactly one of
`code` or `error`, plus exactly one state and issuer. `iss` must byte-match the
configured issuer for the callback slug and attempt.

For a code response, one immediate database transaction matches state digest,
provider ID/version/generation, pending state, unexpired time, and constant-time browser
binding; it changes `pending` to `exchanging` before token exchange. Exactly
one affected row is required. The callback cookie is expired on every terminal
path. No attempt returns to pending: transport, token, validation, disabled
provider, or mapping failure changes exchanging to failed or leaves it safely
reconcilable as failed; the browser must start again. Error responses consume
the matching attempt without token exchange. Completed/failed/expired attempts
are periodically deleted after 24 hours.

## Token exchange

The code is 1–2,048 bytes without controls. The exchange uses the attempt's
exact redirect URI and decrypted verifier. Confidential providers use HTTP
Basic only; public providers send `client_id` and no secret. Credentials are
never placed in a URL. The transport and origin policy above applies.

The response must be 200 JSON, at most 64 KiB, depth 16, 128 members, with
duplicate keys rejected. `id_token` is mandatory and at most 16 KiB. If an
`access_token` is returned it is at most 8 KiB and held only in a
zeroize-on-drop value until the response is discarded; it is neither used nor
persisted. Refresh tokens, if returned despite the request, are bounded,
immediately zeroized, and ignored. OAuth error fields are bounded and mapped to
local reason enums without logging their attacker-controlled text.

Userinfo fallback is removed. Email/profile data must be present in the
verified ID token when needed. This eliminates a bearer-token egress and a
second identity-claim source.

## JOSE and JWKS validation

An ID token is a compact signed JWS with exactly three nonempty segments and a
decoded size at most 16 KiB. JWE, detached payload, unencoded payload, and
general/flattened JSON serializations are rejected. A maintained JOSE library,
not local base64-and-JSON code, performs cryptographic verification.

Protected header rules are:

- `alg` is present and belongs to both provider configuration and currently
  discovered metadata; `none`, every `HS*`, and any unlisted value fail;
- `kid` is required, unique in the JWKS, visible ASCII, and 1–128 bytes;
- `jku`, `x5u`, embedded `jwk`, `x5c`, `crit`, and `b64` are rejected;
- `cty` is absent; `typ` is absent or exactly `JWT`; and
- duplicate header members or unknown critical behavior fail.

JWKS is 200 JSON, at most 64 KiB, depth 16, 128 members, and at most 32 keys,
with duplicate JSON members and duplicate nonempty `kid` rejected. The selected
key must match `kid`, algorithm family, curve/size, optional `alg`, `use=sig`
when `use` is present, and contain `verify` when `key_ops` is present. Accepted
keys are RSA at least 2,048 bits with a valid exponent, P-256 EC for ES256, and
Ed25519 OKP for EdDSA. Private key members and incompatible/multi-use keys are
rejected. One and only one compatible key must result.

Signature verification completes before payload claims are exposed outside
the validator. On failure, no partially decoded claim object reaches mapping,
logs, metrics, or audit.

## Required claim matrix

| Claim | Rule |
|---|---|
| `iss` | Required string, byte-exact configured issuer |
| `sub` | Required string, 1–255 UTF-8 bytes, no control; preserved exactly |
| `aud` | Required string or 1–8 unique strings; contains exact client ID |
| `azp` | Required and exact client ID when `aud` has multiple values; when present for one audience, also exact |
| `exp` | Required integer NumericDate; valid only while `now < exp + 60s` |
| `iat` | Required integer; `created_at - 60s <= iat <= now + 60s` |
| `nbf` | Optional integer; `nbf <= now + 60s` |
| `nonce` | Required string; digest compared in constant time to the attempt's nonce digest |
| `email` | Optional valid mailbox-shaped UTF-8 string, at most 254 bytes; metadata only |
| `email_verified` | Optional boolean; email is provisioning-authoritative only when exactly `true` |
| `preferred_username` | Optional string, at most 128 scalars/512 UTF-8 bytes, no control or bidi override/isolate; valid value is a derivation hint only |
| `name` | Optional string, at most 256 scalars/1,024 UTF-8 bytes, no control or bidi override/isolate; display hint only |
| `amr` | Optional array of at most 16 unique visible-ASCII strings, each 1–64 bytes; valid value ignored for authority and not persisted |
| `acr` | Optional visible-ASCII string, 1–256 bytes; valid value ignored for authority and not persisted |
| `auth_time` | Optional integer NumericDate in the representable clock range; valid value ignored for authority and not persisted |
| `at_hash` | Optional visible-ASCII string, 1–256 bytes; valid value ignored because the access token supplies no identity/API authority |

NumericDate values must be JSON integers in a range safely representable by
the application clock; floats, strings, overflow, and duplicate claims fail.
The 60-second skew is symmetric only where stated and is fixed, not provider
controlled. `iat` is additionally bound to this attempt so an otherwise valid
old token cannot be substituted.

Every optional claim listed above is deterministic: if present with the wrong
type, duplicate member, invalid character, or exceeded bound, the ID token is
rejected rather than partially ignored. `at_hash` validation is intentionally
not required in this code flow because the access token is never used for
identity, userinfo, or an API and is immediately zeroized. A well-formed
`at_hash` therefore grants nothing; a malformed one still fails the bounded
claims envelope.

Successful validation returns a non-cloneable construction capability holding
provider ID/version/activation generation, exact `sub`, verified email state, bounded display hints,
and validation time. It contains no raw token, nonce, or upstream access token.

## Identity, provisioning, MFA, and session

Lookup is exclusively `(provider_id, sub)`. An existing link may authenticate
without email. Last-seen email is stored only when syntactically valid and is
never a lookup key.

For an unknown identity:

- `link_only` returns one generic “local account link required” result. The
  unsigned pending-link cookie and incomplete `/auth/federated/link` skeleton
  are removed/disabled. M4 does not create a link. A later design may restore
  self-service linking with fresh local authentication, local MFA, CSRF,
  single-use durable intent, explicit approval, and C19 atomicity.
- `provision_on_first_login` requires a present email and
  `email_verified=true`. If any local user already has that normalized email,
  deny as a takeover collision; never auto-link. Otherwise derive a bounded
  local username under RFC 004 P7 and execute amended RFC 094 F04: attempt
  completion, passwordless non-admin user, link, `[Fed]` session/cap, and
  `auth.federation.provisioned` commit in one Class-A transaction. An absent or
  unverified email denies; M4 does not invent a held-account state.

Raw email, `sub`, ID token, or upstream error text never appears in log or
metric labels. Audit uses internal provider/user/link IDs and bounded reason
enums. Notification of a collision, if retained, must not include upstream
credentials and must not be required for the denial to succeed.

Federation is a primary method. A user without local MFA receives an
authentication context containing `Fed` and ACR1. A user with local MFA enters
a durable/bounded pending challenge that preserves `primary_method=Fed`,
provider/link identity, next target, creation/expiry, and one-time consumption.
Successful TOTP, recovery, or WebAuthn produces `[Fed, Totp]`,
`[Fed, Recovery]`, or `[Fed, Webauthn]`; it never substitutes `Pwd`.
`Fed` must not implement the local-second-factor predicate. Upstream claims do
not change this rule.

The migration creates a dedicated `federation_mfa_pending` row rather than
weakening the current generic user-only pending-MFA shape. It
stores provider/version/activation generation/link/user, sealed continuation/next target, browser and
CSRF binding digests, `primary_method=Fed`, `failure_count` constrained 0–5,
`status` in `pending|completed|exhausted|invalidated`, `created_at`, and
`expires_at=created_at+300 seconds`. Exactly `expires_at <= guard_at` is
expired. Terminal rows retain no usable continuation and are purged after 24
hours. A restart preserves count/status; it grants no reset. A separate
`federation_webauthn_ceremony` table has a unique parent-pending key, exact
provider/version/activation generation/link/user binding, sealed ceremony state, RP/origin, and
no-later expiry; generic password/WebAuthn pending rows cannot reference it.

Five is the fixed maximum across all local methods combined. After browser/
CSRF/pending binding succeeds, every supported-method submission with malformed
or cryptographically wrong proof, unsupported method, or method/ceremony
substitution is one failure. Pre-binding garbage cannot select/touch a row. In
one F03 Protocol transaction, a wrong proof guarded on `status=pending`,
`failure_count < 5`, and unexpired unchanged authority increments by one. New
count 1–4 returns `RejectedStillPending`; new count exactly 5 sets
`status=exhausted`, destroys the continuation, deletes bound method ceremonies,
and returns `AttemptsExhausted`. No sixth increment exists. A correct proof may
promote only while count is 0–4; whichever guarded wrong/correct transaction
serializes first determines the equality boundary.

Proof verification is owned by a private `FederatedMfaVerifier`, whose only
entry point first validates the browser cookie, CSRF token, pending ID, user,
provider/version/generation, method, and pending-row binding. Binding failure
returns only an unbound generic error and cannot construct an F03 input or touch
the row. After binding, the verifier consumes bounded raw input and returns a
sealed, non-cloneable, non-debuggable, non-serializable method-specific
`ValidTotp`, `ValidRecovery`, `ValidWebAuthn`, or `BoundRejected` candidate.
Only the verifier selects the closed rejection reason; handlers can neither
construct a candidate nor supply a row-touching reason. Raw proofs are
zeroized/dropped before the candidate crosses into F03.

Authoritative consumption remains inside F03. TOTP promotion requires a step
strictly newer than durable `last_used_step` and updates it with the session.
Recovery promotion rechecks and removes exactly the matched stored hash with
the session; wrong recovery never removes a hash. WebAuthn uses F06 to create
or atomically replace one `FederatedLogin` ceremony bound to pending ID,
provider/version/activation generation/link/user, RP/origin/challenge, and expiry no later than the
pending row. F03 requires that exact ceremony and credential, revalidates the
sealed WebAuthn result, then consumes ceremony/pending and updates the passkey
counter with the session. Wrong WebAuthn completion increments the shared
failure count and consumes only that failed ceremony; retry requires F06.
TOTP/recovery proof cannot enter the WebAuthn branch or vice versa.

The exact durable branches under the independently reviewed and owner-approved
RFC 094 amendment are:

- F01 atomically completes an existing-link/no-MFA attempt, updates bounded
  link observation/login bookkeeping, inserts `[Fed]` session, and enforces the
  cap as Protocol state; its success event is Class B after commit.
- F02 atomically completes the upstream attempt and creates one Fed-bound
  pending-MFA continuation, without session, link observation, or success.
- F03 has four guarded Protocol outcomes: `Promoted` performs method-specific
  anti-replay/pending consumption, link observation/bookkeeping,
  `[Fed, local_method]` session and cap; `RejectedStillPending` increments to
  1–4; `AttemptsExhausted` increments to exactly 5 and terminalizes;
  `Invalidated` terminalizes on authority drift/expiry. Only Promoted emits
  post-commit Class-B sign-in success. Rejection/exhaustion/invalidation emit
  their fixed Class-B observations after their transaction.
- F04 is the Class-A first-provision branch described above. A new user cannot
  already have local MFA, so it has no pending-MFA variant.
- F05 terminalizes a failed/denied attempt as Protocol state, changes no
  user/link/session observation, and emits only its fixed Class-B observation.
- F06 creates/replaces the one method-bound WebAuthn ceremony as Protocol
  state and grants no authentication/session authority by itself.

The Class-B observation vocabulary is closed. F03 uses
`auth.federation.mfa_rejected` with reason
`totp|recovery|webauthn|malformed|method_mismatch`,
`auth.federation.mfa_exhausted` with `attempt_limit`, or
`auth.federation.mfa_invalidated` with
`expired|provider_changed|provider_disabled|link_changed|user_inactive|factor_changed|ceremony_invalid`.
F05 selects exactly one of:

- `auth.federation.signin.upstream_failure` with
  `upstream_error|transport|discovery|token_response|jose|claims|timeout|cancelled`;
- `auth.federation.takeover_blocked` with `email_collision`;
- `auth.federation.link_required` with `link_only`; or
- `auth.federation.signin.denied` with
  `attempt_expired|provider_changed|provider_disabled|user_inactive|link_changed|provision_policy|internal_failure`.

Only these enum tags and internal IDs enter observations; upstream strings,
method proof, counter value, email, subject, and endpoints do not.

`last_seen_at` and bounded verified-email observation are private primitives
inside F01/F03/F04, not C19 link-authority updates. F01–F03/F05/F06 deliberately retain
RFC 094's accepted session/login Protocol/Class-B classification; the design
does not mislabel their login event as atomically audited. F04 and C23 are the
only new Class-A commands. The amendment was independently design-reviewed,
the materially amended RFC was durably returned to Proposed in commit
`43085e38219e5eb1bfe11cc698b18f1fa5f5e4d7`, and `@nabbisen` explicitly
accepted the complete amended RFC on 2026-07-21. This RFC's own explicit
acceptance transition remains required.

## Cache and rotation algorithm

This is a federation security-document cache profile, not a general RFC 9111
cache. It deliberately supports only ETag revalidation and the closed
directives below, ignores stale extensions, forbids stale use, and defines
stricter custom 304 metadata defaults. Generic HTTP cache middleware must not
replace it.

Cache keys include provider ID and config version. Configuration mutation,
disable, or delete makes old entries non-authoritative by durable version/state
comparison, invalidates attempts in the owning database command, and requests
post-commit eviction.

| Cache | Default freshness when no `max-age` | Maximum freshness |
|---|---|---|
| Discovery | 1 hour | 24 hours |
| JWKS | 15 minutes | 6 hours |

The cache parses a closed case-insensitive directive set from bounded
`Cache-Control` fields: `no-store`, `no-cache`, `must-revalidate`, `max-age`,
and `s-maxage`. Multiple field lines are combined as an HTTP list, but a
recognized directive may occur only once; conflicting/duplicate recognized
directives, quoted or noncanonical decimal delta-seconds, overflow, invalid
list syntax, or obsolete folding reject the response. Unknown bounded
directives are ignored. `s-maxage` is parsed but ignored because this is a
private cache. `must-revalidate` is recorded but adds no behavior because stale
use is always forbidden.

`no-store` permits use of a fully validated 200 representation only for the
current operation and evicts/retains no entry or ETag. `no-cache` permits
retention but gives zero reusable freshness. `max-age=0` also gives zero
freshness. Positive `max-age` is honored exactly up to the per-cache maximum;
only the upper bound is clamped. It is never raised to a minimum. With no
`max-age`, the table default applies unless `no-cache` is present.

Exactly zero or one `Age`, `Date`, `ETag`, and `Content-Type` field is accepted;
multiple `Cache-Control` fields alone may use HTTP list combination. `Age` is
canonical decimal seconds bounded by `u32::MAX`; `Date` is IMF-fixdate. Duplicate/malformed `Age`
or `Date`, or `Date` later than the trusted wall clock plus 60 seconds rejects
the response. Initial current age is the maximum of `Age` (zero when absent)
and nonnegative trusted-now minus `Date` (zero when absent), then monotonic
resident time is added. Freshness is
`lifetime > current_age`; equality is stale. Clock regression expires the
entry. An ETag, when present, is one strong or weak entity-tag matching HTTP
grammar, at most 256 visible/quoted bytes; duplicate, malformed, control, or
oversize ETag rejects the response.

Every 200 body is bounded, parsed, and semantically validated even when its
ETag equals the retained validator. A valid 200 atomically replaces the old
entry only after full validation. An invalid 200 never refreshes or preserves
fresh authority beyond the old entry's independent original expiry.

A 304 is accepted only for a conditional request naming the exact retained
ETag of a validated, cacheable, exact-provider-version-and-activation-generation representation. The
retained body may be locally stale; successful network revalidation is what
permits current use. A 304 without such a body/validator, after `no-store`, or
for a different version or activation generation is rejected. A returned ETag must equal the requested
validator byte-for-byte. Cache-Control present on 304 replaces the retained
recognized directive set; if absent it is inherited. Present Age/Date replace
their retained metadata; absent Age becomes zero and absent Date becomes the
trusted 304 receipt time, so prior freshness age is not silently reused.
`no-store` on 304 allows the retained validated body for that
one waiting operation and then evicts it. `no-cache`/`max-age=0` allow that
operation but make the entry immediately stale. 304 never validates a
different body and never bypasses document semantic validation already bound
to the retained representation.

An entry is usable only while fresh and for the exact enabled
provider/version/activation generation. Cache entries and single-flight keys
bind all three values, so disable/re-enable cannot revive documents fetched in
an older activation even when the trust policy is unchanged.
There is no stale-if-error acceptance. Cache directives such as
`stale-if-error` or `stale-while-revalidate` are ignored and cannot extend
authority.

The request/cache state order is closed:

| Situation | Network/result rule |
|---|---|
| Ordinary lookup, fresh entry | Return it; no network |
| Ordinary lookup, stale/miss, active 30s failure cooldown | Fail; no network |
| Ordinary lookup, stale/miss, active flight | Join that one flight |
| Ordinary lookup otherwise | Become one conditional/unconditional flight leader |
| Unknown `kid`, active cooldown | Fail; do not consume forced-refresh budget |
| Unknown `kid`, active ordinary/forced flight | Join it; that result is the sole refresh opportunity for this callback |
| Unknown `kid`, no flight, forced dispatch within prior 60s | Fail; no network |
| Unknown `kid` otherwise | Mark dispatch time and become one forced JWKS flight leader, bypassing fresh-document reuse |

The 60-second forced window begins when its network request is dispatched,
whether it returns 200, 304, or failure. A failed flight starts the 30-second
document/provider cooldown; joined callers observe the same result. A forced
304 proves the key set unchanged, so the unknown key still fails without a
second request. Publication occurs only if the provider remains enabled at the
same version. Maps are bounded by configured providers/document types, never
attacker `kid` values.

Rotation requires the upstream to publish overlap long enough for cached
tokens or tolerate the bounded refresh. Once a refreshed valid JWKS omits a
key, that key is not retained as a fallback. Emergency removal therefore
fails closed after refresh; operators may disable the provider immediately.

## Error and observability contract

Browser responses use a small fixed local taxonomy: provider unavailable,
federation failed, link required, or local account action required. They do not
echo provider errors, issuer/endpoint URLs, email, subject, code, state, nonce,
or parser details and do not distinguish unknown from disabled providers.

Internal structured telemetry may contain request ID, internal provider ID,
phase (`start`, `discovery`, `callback`, `token`, `jwks`, `claims`, `mapping`,
`mfa`), and a closed reason enum. It may contain duration and bounded counts.
It must not contain full URLs, query strings, response bodies, DNS answers,
tokens, secrets, cryptographic hashes, subject, email, or arbitrary upstream
text. Provider slug may be a configured bounded field in operator logs but not
a metric label.

Start is rate-limited by source/IP and provider; callback is rate-limited by
source/IP and a process-local keyed digest of state after syntax validation.
Limiter keys are neither durable nor exported. Limits are configuration
bounded and availability controls only; protocol correctness never depends on
them.

## Migration and rollout

The next available migration adds typed/versioned provider trust fields, the
canonical checked `activation_generation`,
nullable last-successful-C17 preflight evidence (with no standalone writer),
`federation_login_attempt`, `federation_mfa_pending`, and
`federation_webauthn_ceremony`. Existing provider rows lack enough durable evidence
to infer endpoint origins, algorithm policy, authentication method, or a safe
preflight. Migration therefore:

1. preserves provider IDs, sealed secrets, and federation links;
2. sets every provider disabled, `validation_status=requires_review`, and
   `activation_generation=0`;
3. writes no guessed origin, algorithm, auth method, or preflight evidence;
4. reports each provider for operator configuration; and
5. deletes/invalidates legacy browser-state and pending-link flows without
   unlinking users.

Startup refuses malformed new federation configuration but does not prevent
the rest of sui-id from starting merely because a disabled provider cannot
reach its upstream. An enabled legacy-shaped or unvalidated row is
runtime-ineligible. Operator configuration, successful preflight, audited
enable, and one-provider canary precede wider rollout.

No compatibility flag re-enables unsigned ID-token decoding, userinfo fallback,
shared callback exchange, private egress, optional nonce, or reusable cookie
state. Rollback disables providers; it does not downgrade the database or
restore insecure validation.

## Implementation stages and ownership

1. Pure configuration/URL/JSON/JOSE claim validators and adversarial corpus.
2. Injectable resolver/transport plus discovery/JWKS cache.
3. Schema migration, repository types, provider preflight/versioning.
4. Durable attempt start/callback claim and token exchange.
5. Verified-identity mapping, RFC 094 mutation integration, MFA/session method
   preservation, and removal of insecure legacy paths.
6. Hostile-provider matrix, migration/rollback tests, representative live
   canary, documentation, and independent closure review.

Stages 1–3 may be prepared in files not owned by an active RFC 094/095
implementer. Stage 5 is blocked until RFC 094 is Implemented. Any overlap in a
shared handler, state, repository, migration sequence, audit registry, or OIDC
session code requires explicit file ownership plus a second independent
implementer/reviewer as required by the roadmap.

## Verification and closure evidence

The normative adversarial cases are in the companion validation matrix and
verification handoff. At minimum, evidence covers:

- issuer/path/response mix-up, discovery endpoint substitution, redirects,
  DNS rebinding, private/special IPs, proxy inheritance, TLS failures, and
  recorded rustls handshake/certificate/record allocation bounds;
- oversized/slow/duplicate/ambiguous JSON and URL canonicalization;
- `none`/HMAC/wrong algorithm, indirect key references, duplicate `kid`, bad
  key type/size/use, signature failure, and key rotation/removal;
- missing/mismatched nonce, replayed state, callback contention, provider
  change/disable mid-flight, activation-generation replay/overflow, the
  P1/P2-enable-disable-P2 race, and code-exchange failure after claim;
- issuer/audience/`azp`/time/sub/email claim boundaries;
- email collision, unverified provisioning, unknown link-only identity,
  local-MFA enforcement, and correct `amr`/ACR output;
- cache expiry, unknown-`kid` storms, single-flight, cooldown, and no stale
  acceptance;
- migration quarantine, log/audit/metric redaction, and fault-injected RFC 094
  rollback, private MFA-candidate construction/binding, and policy-digest
  canonical vectors; and
- one representative public upstream integration using owner-supplied test
  credentials outside CI, with secrets absent from artifacts.

Closure records exact clean commit, commands/output, fixture identity, clock
and IANA registry snapshot, cache constants, migration report, sanitized
telemetry samples, and independent finding disposition. A live happy path does
not substitute for hostile-provider evidence.

## Rejected alternatives

- **Trust ID-token claims because token exchange used TLS:** TLS authenticates
  an endpoint, not the token's issuer, audience, key, or replay context.
- **Retain userinfo fallback:** it expands bearer-token handling and permits
  identity data from a second response; M4 requires one signed source.
- **Cookie-only state:** integrity does not provide single-use consumption or
  server-side provider/version binding.
- **Follow discovery/JWKS redirects:** redirects let a trusted origin delegate
  egress at request time outside operator policy.
- **Allow a private-network exception switch:** its DNS, routing, certificate,
  and operator trust model is materially different and requires its own RFC.
- **Use email to recover an existing link:** this violates RFC 004 P1/P2 and
  creates an account-takeover path.
- **Complete linking incidentally in M4:** a safe local reauthentication and
  approval flow is a separate security design, not a callback detail.

## Open issues resolved by this design

The proposal has no implementation-discretion open question about private
issuers, cache bounds, algorithm sets, state lifetime, userinfo, callback
binding, or link-only behavior. Changes to those closed choices require an RFC
amendment and independent security review.
