# RFC 134 — Federation egress is a policy, not a client

**Status.** Proposed
**Security review.** Required

**Design prerequisites.** None. RFC 096 is Accepted and its normative validation
matrix already states the required behaviour; this RFC decides *how* that
behaviour is obtained and which of its rows survive scrutiny.
**Implementation prerequisites.** This RFC Accepted. **No dependency on RFC 094
M2a** — nothing here is a Class-A durable mutation, so this work can run beside
M2a rather than behind it.
**Closure prerequisites.** Every control in D1–D4 is enforced at a single
construction site; the resolver rejects both edges of every vendored prefix; a
gate fails if a second egress client appears in the federation path; and the
hostile-provider corpus in RFC 096's matrix passes. D5's matrix amendment is
settled either way — whichever answer comes back, those rows stop being
ambiguous.
**Tracks.** ROADMAP M4-A — Federation validation and transport.
**Touches.** `crates/sui-id/src/runtime/state.rs`, a new egress module,
`crates/sui-id/src/http/handlers/federation.rs`, `contracts/`, CI.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

## Summary

Federation's outbound HTTP client is built in one place with one setting on it, a
timeout. RFC 096-A's validation matrix specifies roughly a dozen transport
controls that the client is therefore not applying, and specifies them as
properties of *a client* — which is why they were easy to omit and would be easy
to omit again. This RFC makes egress **a policy object that is impossible to
bypass**: one constructor, one resolver, one response reader, and a gate that
fails if a second client appears. It also proposes that five of the matrix's rows
be amended, because they specify the internals of an HTTP parser rather than an
observable security property, and meeting them literally would mean replacing a
reviewed HTTP stack with our own.

## Background

The validation matrix was written by the previous architect. Most of it is
excellent and unusually concrete. A minority of it specifies implementation
internals — exact buffer sizes inside the header parser, certificate-chain byte
caps, "connection dropped without EOF wait" — that are not reachable through
`reqwest`, `hyper` or `rustls` configuration, and are not observable from outside
the process. Those rows are addressed in D5 rather than quietly skipped.

## Design

### D1 — One constructor, and it is the only one

A single function builds the federation egress client. Every setting below is
applied there; no call site may construct, clone-and-modify, or override it.

Verified available in `reqwest` 0.13.4 under this workspace's feature set
(`rustls` → `__rustls` → `__tls`):

| Control | Call | Matrix row |
|---|---|---|
| No redirects | `.redirect(redirect::Policy::none())` | Redirect: *none*; reject every 3xx |
| No proxy | `.no_proxy()` | Proxy/ambient state: *none* |
| No referer | `.referer(false)` | Proxy/ambient state |
| HTTP/1.1 only | `.http1_only()` | HTTP version: HTTP/1.1 ALPN only |
| TLS floor and ceiling | `.min_tls_version(TLS_1_2)`, `.max_tls_version(TLS_1_3)` | Connection: TLS 1.2–1.3 |
| Connect bound | `.connect_timeout(3s)` | Timing: connect ≤3s |
| Header bound | `.read_timeout(5s)` | Timing: headers ≤5s |
| Total bound | `.timeout(10s)` | Timing: total ≤10s |

**No cookie store.** The `cookies` feature is not enabled, so there is none to
disable. The implementation asserts this rather than assuming it: a compile-time
check that the feature is off, so enabling it later for an unrelated reason
cannot silently give federation a cookie jar.

### D2 — A resolver that validates, and returns exactly one address

The SSRF control belongs in DNS resolution, not in a check before or after it.
`reqwest::dns::Resolve` is implemented by an egress resolver that:

1. resolves the name through the system resolver;
2. rejects an empty answer, or more than 8;
3. normalizes every address and rejects any that is not ordinary public unicast —
   every IANA special-purpose prefix **regardless of its "globally reachable"
   flag**, plus the explicit deny list: NAT64 (both prefixes), 6to4, Teredo,
   IPv4-mapped, IPv4-compatible, and anycast;
4. returns **exactly one** surviving address.

Returning one address is what closes rebinding: the connector dials what the
resolver returned, so there is no window between the check and the connection,
and no second answer to fall back to. **Validating inside the resolver is the
design; a pre-flight lookup followed by a connection is the bug.**

The prefix table is **vendored, not computed**, with each row carrying its
registry source, so a change to it is a reviewable diff.

### D3 — Discovery is untrusted input

**This is the highest-value control in this RFC and the cheapest.**

A discovery document is fetched from the provider but is not *from* the provider
in any sense the type system knows. Every URL taken from it — `token_endpoint`,
`userinfo_endpoint`, `jwks_uri`, `authorization_endpoint` — is validated before
use:

- it is canonical HTTPS;
- its **origin and port are in the provider's configured policy**. A discovery
  document may not introduce an origin the administrator did not configure.

The matrix already requires this (§"Configuration and URLs", Endpoint row:
*Reject: discovery-added origin*). It is stated separately here because it is the
control that makes the rest of the chain sound: the token request carries the
provider's `client_secret`, and whether that credential goes to the provider is
decided entirely by this check.

### D4 — One client, enforced structurally

A gate asserts that `reqwest::Client::builder()` appears in the federation egress
module and nowhere else under the federation path. Controls that live in a
constructor are only as good as the guarantee that nothing else constructs one,
and that guarantee is mechanical or it is nothing.

**HIBP (`crates/sui-id-core/src/authn/hibp.rs`) is out of scope** and keeps its
own client: it talks to one pinned, first-party-chosen endpoint with no
attacker-influenced URL, so the threat model differs. The gate must **name** that
exemption rather than pattern-match around it, so a third client cannot appear by
resembling the exempt one.

### D5 — Five matrix rows specify a parser, not a property

These rows cannot be met through configuration of the current stack:

| Row | What it specifies |
|---|---|
| HTTP allocation | fixed 32 KiB header buffer, 64 slots, 8 KiB scratch |
| Connection | handshake ≤256 KiB inbound, chain ≤16/128 KiB |
| Header envelope | bare LF / obs-fold rejection, duplicate singleton |
| Body framing | sole chunked, empty trailer, "dropped without EOF wait" |
| Body | chunk extension/line >128 |

Meeting them literally means replacing `hyper` and `rustls` with our own HTTP/1.1
parser and TLS bounds. **I recommend against it.** A hand-written HTTP parser is
a classic source of request-smuggling and memory-safety defects, and we would be
trading a widely reviewed implementation for an unreviewed one in order to match
specific numbers whose security value is in the *bound existing*, not in its
being 32 KiB.

**Proposal:** amend these rows to state the property (*bounded header and body
allocation; strict framing; no request smuggling*) and satisfy them by a
dependency floor plus a recorded statement of what is relied upon.

**What I have not done:** verified `hyper` 1.10.1's exact defaults for each of
these. The implementation must *evidence* the guarantee it relies on — version,
and the upstream behaviour relied upon — not assert it. If a guarantee turns out
to be absent, that row returns as real work rather than being waved through.

## Multiple implementation steps

1. **D3 alone.** Endpoint-origin validation. Smallest, highest value, no new
   dependency. Shippable by itself.
2. **D1 + D4.** The constructor and the gate that keeps it singular.
3. **D2.** The validating resolver and its vendored prefix table — the largest
   piece, and the one whose tests are the matrix's resolver corpus.
4. **D5.** Only after the amendment question is answered.

## Tests

The matrix's own resolver corpus is the specification: both edges of every
vendored prefix, ordinary public v4/v6 peers, mixed public/private answers,
rebinding, address-order changes, IPv4-mapped and -compatible, IPv6 zone IDs,
metadata-service addresses, both NAT64 boundaries with public/private/link-local/
metadata embedded v4, 6to4, Teredo.

For D3, the hostile-provider corpus must include a discovery document whose
`token_endpoint` names an origin outside the provider's policy, and the test must
assert **no request is made**, not merely that sign-in fails.

Each control gets a test that fails when the control is removed. A test that
passes with the setting deleted is not evidence of the setting.

## Security considerations

The threat actor is the upstream provider itself, or a party able to influence
what its discovery document says. TLS addresses the passive-network case and is
not the control here. The standing bound is that a federation provider is
**administrator-configured**; no user-facing route accepts an arbitrary URL.

**One interaction deserves recording.** Federated ID-token signatures are not
verified; the code relies on receiving the token directly from the token endpoint
over TLS. **That is permitted** — OIDC Core §3.1.3.7 allows TLS server validation
in place of signature checking for a back-channel token response — and it is not
a defect. But it is sound only while the token endpoint is genuinely the
provider's, which is what D3 establishes. The two are safe together and should be
reasoned about together; if D3 is ever relaxed, this shortcut stops being
permissible and JWKS verification becomes mandatory.

## Open questions

1. **D5's amendment** — amend the five rows to state properties, or require
   literal compliance and accept a bespoke HTTP client? My recommendation is to
   amend. This one is not mine to settle, because it changes an accepted RFC's
   normative matrix.
2. **Should D3 ship ahead of this RFC's acceptance?** It is small and closes the
   most. I am *not* proposing that — RFC 000 prohibits building from `proposed/`,
   and I would rather ask than carve an exception.
3. **Is the 8-address answer limit right**, or should it be 1? Nothing in this
   design needs more than one surviving address, and a lower cap is a smaller
   attack surface. The matrix says 8; I see no reason for 8.
