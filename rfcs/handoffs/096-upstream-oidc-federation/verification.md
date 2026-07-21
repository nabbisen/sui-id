# RFC 096 verification and closure plan

This plan defines evidence required after implementation. Its presence is not
evidence and no command is claimed until observed on the implementation
baseline.

## Test architecture

Build a deterministic hostile-provider fixture beneath the production policy
interfaces. It supplies scripted DNS answers, pinned-connect observations,
TLS outcomes, headers/chunks/delays, discovery, JWKS, token responses, clock,
and cancellations. It must not require a production private-network allow
switch and cannot be constructed by production startup.

Use real JOSE operations and independent fixture keys. Golden tokens cover
each supported algorithm. Mutation helpers generate one defect at a time plus
combined ambiguity cases. Random/property tests remain bounded and record
seeds on failure.

## Evidence suites

### 1. Static/type isolation

- production construction uses dedicated no-redirect/no-proxy transport;
- only validated/pinned destinations reach the connector;
- only `IdTokenValidator` constructs `VerifiedFederatedIdentity`;
- unverified payload/token types are non-debuggable and cannot reach mapping;
- test-only private transport is absent from production features;
- all provider/link/user/session writes map to the RFC 094 manifest; and
- C17/C18 activation handling and C23/F01–F06 descriptors/capabilities and subordinate-write mappings exist,
  with no nested Class-A or direct observation writer; and
- no legacy unverified decoder, userinfo fallback, shared callback exchange,
  unsigned pending-link cookie, or `Fed` second-factor branch remains.

Compile-negative fixtures attempt to construct/substitute every sealed stage.
Structural searches are supporting evidence, not the only proof.

### 2. Unit/property corpus

Execute every row and boundary in `validation-matrix.md`. Record case IDs,
expected typed result, observed result, clock, algorithm, and fixture version.
Canonical URL, discovery derivation, IANA/explicit transition ranges, bounded
HTTP/JSON, NumericDate/optional claims, state-machine, cache, and redaction
properties require generated edge cases plus stable regression cases.

### 3. Transport integration

Observe dialed IP, SNI/Host, request path, redirects, proxy environment, DNS
calls, deadlines, response bytes, and credential headers. Prove there is no
second DNS resolution and no request to an unapproved redirect/origin. Test
mixed answers and DNS changes between discovery, JWKS, and token requests.
Observe denials for both NAT64 prefixes with public and denied embedded IPv4,
mapped/compatible IPv6, 6to4, Teredo, anycast/special rows marked globally
reachable, and undisposed registry drift. Assert the final dial list contains
only ordinary public addresses outside every deny table.

At raw TLS/wire level, prove ALPN offers only HTTP/1.1 and generate 0/1/exact/
over limits for fixed header buffer/slots, status line, semantic total,
name/value, ETag, bare LF/obs-fold, invalid grammar, partial/slow headers,
duplicate/conflicting Content-Length, Content-Length+Transfer-Encoding,
stacked coding, chunk-size line/extensions, body cap, and
trailers. Limit failures occur before a normal response object or growable
header allocation. Attempt HTTP/2 negotiation, continuation/compression-table
payloads, HTTP/1.0, informational chains, and upgrade; all reject without HPACK
field-list allocation.

Record the exact locked rustls version/configuration/source constants and
allocation manifest. Exercise handshake encrypted-byte equality/overflow,
rustls message-limit equality/overflow, 16/17 certificates, 128 KiB/over DER,
record fragmentation, TCP/TLS/total deadline equality, and allocation under a
hostile peer. For Content-Length, prove reads never request beyond the
remaining length, exact completion drops the one-use connection immediately
without EOF wait, deliberately appended bytes are neither read nor reused, and
a live canary that keeps its socket open after the declared body still works.

Never print Basic credentials. Assertions inspect redacted fixture state or
one-way equality indicators.

### 4. Attempt concurrency/cancellation

For 2, 8, and 64 callbacks racing one attempt, reconcile:

- one pending-to-exchanging winner;
- exactly one token request;
- no attempt reset;
- terminal status after injected success/failure; and
- one permitted final session/mutation outcome.

Inject cancellation before claim, after claim/before request, during response,
during validation, before RFC 094 dispatch, within the Class-A transaction,
around commit, and before HTTP response. A disconnected browser must never make
the same attempt retryable. Reconcile database rows/events/sessions rather than
inferring from HTTP delivery.

Preflight races cover failed/cancelled/restarted preflight, negative/zero/
599.999/600-second age, policy/version/generation mutation, disable/delete,
preflight attempted while enabled, two distinct same-generation capabilities,
replay/substitution, evidence-write/event/commit faults, and post-enable
disable/re-enable. The required sequence is P1/P2 -> P1 enable -> disable -> P2
re-enable attempt, which must fail before mutation/event. Also run two complete
fresh-preflight enable/disable cycles, generation 0/max/malformed/noncanonical
states, overflow in C17/C18/C23, cancellation at each transaction boundary,
audit rollback, restart, and C23/delete races. Reconcile exact old/new
generation, exactly one enable/evidence/event per fresh generation, no
standalone write, and mandatory post-disable fresh preflight.

Digest vectors cover every included field, absent versus empty, every list
permutation, secret-envelope change, field-order/domain/schema mutations,
canonical equivalence rejection, fixed 32-byte constant-time comparison, and
compile/structural denial of caller construction, serialization, logging,
auditing, or metrics exposure.

Provider lifecycle cases prove C16 IDs are globally fresh across create/delete/
recreate and cannot be supplied or deterministically recovered. Startup
reconciliation with an unchanged secret preserves the exact existing envelope,
while an actual secret change reseals and advances policy only inside zeroizing,
non-debuggable handling. Neither plaintext nor envelope digest enters evidence.

### 5. Cache and rotation

Use trusted wall and monotonic fake time to prove defaults/upper caps, exact
short/zero max-age, no-store, no-cache, must-revalidate, ignored s-maxage,
Age/Date/resident-age arithmetic, expiry equality, clock regression, duplicate/
malformed/overflow directives, failure cooldown, single-flight,
provider-version-and-activation-generation invalidation, unknown-kid 60-second dispatch suppression,
bounded memory, ETag/304 precedence, and no stale acceptance.

Run a deterministic table for ordinary/forced lookup × fresh/stale/miss ×
no-flight/ordinary-flight/forced-flight × cooldown/window state. Reconcile
network count, joiners, budget timestamp, cooldown, publication, and caller
result. Exercise 304 with no retained body, no-store history, mismatched ETag,
same/different version or generation, expired retained body, replacement/preserved
Cache-Control, Age/Date, no-store/no-cache/zero freshness, and forced unknown
kid. Every same-ETag 200 body is reparsed, including changed valid and invalid
bodies.

Rotation sequences include old only; overlap; new only; unknown key before and
after permitted refresh; malicious duplicate kid; changed key under same kid;
refresh failure; emergency removal; and provider disable mid-flight.

### 6. Mapping, MFA, and atomicity

Cover existing link, disabled/deleted user, absent/changed email, collision,
verified provisioning, unverified denial, link-only denial, username conflicts,
and provider disable/version races. Assert no email lookup creates authority.

For no MFA and each local MFA method, inspect resulting session/OIDC
authentication context and prove `Fed` remains primary and upstream MFA claims
change nothing. Attempt continuation substitution and replay.

Inject every C17-preflight/C18/C23/F01–F06 predicate, domain write, anti-replay, cap,
event-build/audit-chain where applicable, and commit failure. Reconcile
attempt, pending MFA, user, link observation/authority, session, bookkeeping,
event, and chain state. Prove C23 audit rollback and post-commit eviction
failure; F01/F03 Class-B success occurs only after protocol commit; F02/F06
have no success; F04 has exactly one atomic event; F05 changes no authority. Any split
compound write, nested Class-A command, or atomic-audit claim for F01/F03 is a
blocker.

For TOTP, recovery, and WebAuthn, run 2/8/64-way wrong/correct races at counts
0, 3, and 4; exact fifth failure; correct-at-five; restart; expiry equality;
method substitution; malformed bound proof; invalid binding; F06 concurrent
replace; WebAuthn parent/RP/origin/challenge mismatch; and post-limit replay.
Reconcile shared count/status, method ceremony, TOTP step, recovery hashes,
passkey counter, pending row, session, link observation, and exact closed
Class-B event/reason. No wrong proof consumes valid anti-replay authority.

Compile-negative fixtures prove handlers cannot construct valid or
`BoundRejected` candidates or choose an F03 rejection reason. Runtime cases
separate invalid browser, CSRF, pending ID, provider/version/generation, user,
and method binding (no row touch/candidate) from each bound malformed/wrong/
substituted proof (private verifier candidate and exactly one guarded count).
Prove raw proof buffers are dropped/zeroized before F03 dispatch.

### 7. Migration and upgrade

Upgrade fixtures include zero providers; enabled/disabled legacy providers;
each provision mode; existing links/users; sealed secrets; malformed rows; and
interrupted/repeated migration. Prove providers become disabled/review-required,
links and IDs survive, no policy is guessed, old attempts/cookies cannot be
used, reports are redacted, and rerun behavior is deterministic.

Fresh-install schema, upgrade schema, repository model, and documented schema
must agree. Record the actual migration number chosen after preceding RFC work.
Assert preflight evidence is nullable, has no standalone writer, is written
only with C17 enable and bound to the resulting activation generation, clears
on disable/C23/C18, and cannot authorize re-enable. Fresh/upgrade schema and
repository types use strict canonical-decimal generation zero and reject
malformed/overflowed values.

### 8. HTTP/browser and observability

Exercise provider enumeration, start, callback success/error/replay, legacy
callback, invalid next, link-only denial, MFA, and provisioning through the
real router. Verify cookie flags/path/expiry, fixed browser taxonomy, no open
redirect, no cacheable credential response, and no upstream text reflection.

Capture logs, metrics, audit, tracing, panic/error output, and fixture request
history for all cases. Run forbidden-fragment scanning and inspect bounded
cardinality. Browser/network evidence must show no userinfo request and no
unexpected origin.

### 9. Representative live canary

After hostile evidence passes, configure one owner-approved public upstream in
a disposable non-production realm. Credentials remain in environment/secret
storage and outside commands, logs, CI, screenshots, and artifacts. Observe
discovery/JWKS/token hostnames, callback issuer binding, valid login, state
replay denial, local MFA, provider disable, and key-refresh behavior available
from that provider.

Record provider product/version, date, sanitized policy, expected endpoints,
test-account disposal, ALPN presence/absence, connection-close/chunked behavior,
whether a declared-length socket remains open, informational/trailer behavior,
and limitations. Live availability or provider behavior
does not waive any closed profile rule.

Operator documentation and a configuration test state that IPv4-only upstreams
need native IPv4 egress and otherwise need provider-native ordinary public
IPv6; an IPv6-only DNS64/NAT64 deployment fails closed with no compatibility
switch.

## Suggested gate sequence

Run the repository-mandated format, lint, unit, integration, migration,
all-feature, documentation, supply-chain, and packaging gates from RFC 093,
then RFC 096-specific suites. Exact commands come from the implemented RFC 093
gate manifest; this planning document does not freeze stale command spelling.

Run relevant suites from a clean tree and repeat security-critical concurrency/
cache suites enough to expose scheduling sensitivity. Record repetitions,
seeds, duration, platform, toolchain, and whether network was intentionally
enabled. The live canary is separate from deterministic CI.

## Closure bundle

The implementation review request must reference:

- governing RFC 096 file;
- this README, architecture, validation matrix, and verification plan;
- RFC 004 and the implemented RFC 094 command/inventory evidence;
- exact implementation handoff and clean commit;
- diff/file ownership and chosen migration;
- observed command results and artifact locations;
- hostile fixture version, IANA snapshot, clock/cache constants;
- migration report and rollback/disable procedure;
- concurrency/fault durable reconciliation;
- sanitized telemetry/browser/live-canary evidence; and
- remaining risks and explicit non-goals.

The implementation reviewer may differ from the design reviewer; file paths
and approved design-review disposition must therefore be sufficient to
reconstruct every security decision without relying on conversation history.

## Immediate blockers

Closure is NO-GO for any blocker/high finding, missing hostile class, stale-key
acceptance, private/unpinned egress, unsigned/unbound identity, state replay,
email merge, local-MFA bypass, insecure legacy fallback, unreviewed RFC 094
mutation seam, guessed legacy provider policy, secret/claim leakage, missing
live canary, dirty/unknown baseline, or unobserved required gate.
