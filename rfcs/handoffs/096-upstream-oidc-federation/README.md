# RFC 096 developer handoff

**Governing RFC:** [RFC 096](../../proposed/096-upstream-oidc-federation-validation.md)
**Audience:** `codex-developer` only after the applicable entry gate
**Status:** Planning companion; inherits the governing RFC's current status — returned to `proposed/` on 2026-07-28 for a material prerequisite and staging amendment and pending fresh independent design review and re-acceptance — with implementation still blocked on the entry gates below

This package decomposes the federation trust completion without reopening RFC
004's stable mapping policy or RFC 094's atomic-audit architecture.

## Companion files

- [architecture.md](architecture.md) — target components, state transitions,
  migration, cache, activation, and ownership.
- [validation-matrix.md](validation-matrix.md) — normative transport,
  metadata, JOSE, claim, mapping, and error rules.
- [verification.md](verification.md) — hostile fixtures, concurrency/fault
  evidence, live canary, and closure bundle.

## Entry gates

Pure stages 1–3 may start only when RFC 096 is Accepted, its design review has
no blocker/high finding, RFC 093 is Implemented with the clean-tree matrix, a
clean baseline commit is recorded, and file ownership is non-overlapping.

RFC 096's RFC 094 design was independently reviewed, the complete material RFC
was durably returned to Proposed in commit
`43085e38219e5eb1bfe11cc698b18f1fa5f5e4d7`, and `@nabbisen` explicitly
accepted the complete amended RFC on 2026-07-21. This prerequisite is satisfied.
Handler/mapping/session stage 5 requires amended RFC 094 to be Implemented with
all C17/C18/C19/C23/F01–F06 and applicable user/session fixtures passing. If RFC 095
implementation is active, its owner must release any shared OIDC/session/
migration file explicitly. The roadmap's second implementer and independent
reviewer requirement applies to any approved overlap.

## Frozen boundaries

- Public canonical HTTPS issuer and endpoint origins only; no private-network
  exception.
- All IANA special-purpose and explicit translation/tunnel/anycast prefixes,
  including both NAT64 prefixes, are denied regardless of embedded address.
- Deployments provide native IPv4 for IPv4-only upstreams or provider-native
  ordinary public IPv6; DNS64/NAT64 compatibility is intentionally absent.
- Provider-specific callback plus mandatory RFC 9207 `iss`.
- Durable one-winner attempt before token exchange; no cookie-only fallback.
- Mandatory signed ID token; no userinfo request.
- Closed asymmetric JOSE algorithms and public-key profiles.
- `(provider_id, sub)` lookup only; verified-email provisioning only; no email
  merge.
- Unknown `link_only` identity fails closed; self-service linking is not built.
- Upstream MFA never satisfies local MFA; federated primary method survives the
  local challenge.
- No stale JWKS/discovery acceptance; closed HTTP freshness/304 rules and
  bounded provider-wide unknown-`kid` refresh.
- HTTP/1.1-only bounded parser/framing with fixed pre-response allocation and
  all trailers rejected; HTTP/2 is not offered; rustls and handshake/chain/
  record bounds are pinned and evidenced.
- C17 consumes ephemeral exact-activation-generation preflight and increments
  generation on enable/disable; C23/C18 also invalidate generation; exact RFC
  094 C23/F01–F06 ownership; only
  C23/F04 are new Class A, while F01–F03/F05/F06 preserve Protocol/Class-B.

## File ownership

Expected scope includes federation configuration/types/repositories/handlers,
a dedicated egress module, JOSE/discovery/cache modules, the next migration,
MFA primary-method context, RFC 096 fixtures, and operator/security docs.

Do not refactor unrelated outbound HTTP, change RFC 094 capabilities/events,
expand dynamic registration, add upstream protocols/providers, build account
link UX, trust upstream groups/MFA, or alter general OIDC provider behavior.

Before coding, return the exact intended file list and migration number. Stop
if another owner holds one of them.

## Ordered delivery

1. Pure types, configuration validation, canonical URL rules, bounded JSON,
   JOSE header/key/claim validators.
2. Resolver policy, pinned transport, discovery/JWKS cache, injected hostile
   fixture.
3. Migration, typed repositories, version/generation invalidation, preflight and audited
   enable integration.
4. Attempt creation/claim, provider callback, token exchange, ID-token
   verification.
5. Verified identity mapping, provisioning, local MFA/session preservation,
   insecure legacy-path removal.
6. Full adversarial/fault/migration evidence, live canary, docs, handoff, and
   independent closure request.

Every stage must compile independently. A provider cannot be enabled until all
runtime enforcement for its stored configuration is present. Test-only network
injection must be structurally unavailable in production construction.

The operator preflight report records compatibility with the deliberate
HTTP/1.1 profile: ALPN presence/absence, connection-close bodies, chunked
responses, declared-length responses whose socket remains open, and rejection
of informational chains/trailers. It offers no generic-client fallback.

## Stop conditions

Stop and return for architecture/security review if implementation would:

- make canonicalization, special-address policy, algorithm list, cache TTL,
  skew, or attempt state weaker/dynamic;
- accept an IANA special-purpose, NAT64, 6to4, Teredo, mapped/compatible, or
  otherwise explicit transition/anycast destination;
- allow discovery to authorize a new endpoint origin or follow a redirect;
- perform DNS after address validation without revalidating and pinning it;
- decode claims into mapping code before signature validation;
- accept an absent nonce/ID token/`kid`/`iat`, or use a symmetric algorithm;
- retry a consumed attempt or return it to pending;
- use email for existing-account mapping or provision without verified email;
- restore the unsigned pending-link cookie or build linking without a new RFC;
- lose `Fed` as primary authentication across local MFA;
- accept stale keys after refresh/expiry or refresh once per attacker `kid`;
- exceed header/ETag bounds, raise zero/short freshness to a minimum, use 304
  without exact retained validated state, or leave cache-flight precedence
  discretionary;
- enable from persisted/replayed/stale/wrong-generation preflight authority,
  omit an activation-generation increment, accept HTTP/2, leave rustls bounds
  unrecorded, grow the header parser, or accept any response trailer;
- log upstream credentials, claims, URLs, bodies, DNS answers, email, or sub;
- infer safe trust configuration for a legacy provider;
- bypass/overload RFC 094 C17/C18/C23/F01–F06, nest Class-A commands, split their
  compound rows, or call link-observation primitives directly;
- allow more than five bound MFA failures, reset count by restart/method
  switch, or consume method anti-replay state without the session; or
- overlap an active RFC 094/095 implementation without recorded ownership and
  independent capacity.

## Completion return

Return a release handoff that cites the governing RFC and all three companion
files. Include clean commit identity, exact commands and observed results,
migration fixtures/report, IANA registry snapshot, transport construction
proof, hostile-provider corpus, concurrency/fault reconciliation, cache/clock
evidence, sanitized telemetry, live-canary procedure/result, remaining risks,
and the independent closure-review path. Do not claim a gate from a checklist.
