# RFC 134 — Federation egress is a policy, not a client

**Status.** Implemented
**Closure reviewed on.** 2026-10-05
**Closure approved by.** `@nabbisen` (accountable owner), 2026-10-05: "Approved." The closure review was performed by **the architect, which wrote this RFC, all six of its dispatches, and the closure criteria themselves**, and is therefore **not** independent of it — `@nabbisen` is the approver, which is what RFC 000 requires when no independent role exists. The implementation role verified each package's measurements separately; that corroboration is recorded beside them and is neither approval nor independence.
**Closure evidence.** [Closure review 2026-10-05](../handoffs/134-federation-egress-is-a-policy/closure-review-2026-10-05.md). **Level B:** `78a2decb`, run `37266203375` — 26 jobs, 0 skipped, all green; verified to cover all 23 entries in `contracts/gate-inputs.toml`'s `[gates]` (RFC 131 D2).
**Accepted on.** 2026-10-05
**Approved by.** `@nabbisen`, 2026-10-05: "Accepted." — **on D3's re-scope, which is the only thing that changed**: the origin set must gain a path by which an administrator can change it on a provider that already exists. Everything else, including the three security-review changes and the narrowed closure criterion 5, was settled by the earlier approvals and is not reopened. **This acceptance makes D3's maintenance path dispatchable; it is not yet built.**
**Lifecycle history.** **Returned to `proposed/` 2026-10-05**, under RFC 000's return-for-review rule, for a material re-scope of D3 — pre-approved by `@nabbisen` 2026-10-05 ("If re-scope is more reasonable, it will be approved"), on the architect's re-review finding the shipped D3 has no administrator-maintainable origin set. Previously: **Returned to `proposed/` 2026-10-04**, under RFC 000's return-for-review rule, for a material narrowing of closure criterion 5 — approved by `@nabbisen` 2026-10-04 ("Accepted.") on the architect's recommendation. The implementation is **complete and shipped** (steps 1–6, `981b228`/`842b75b`/`2425374`/`a89ea83`); nothing in the design was found wrong. What was wrong was a closure criterion the architect wrote, which imported RFC 096's unbuilt hostile-provider corpus into this RFC's bar — see [`../handoffs/134-federation-egress-is-a-policy/closure-readiness-2026-10-04.md`](../handoffs/134-federation-egress-is-a-policy/closure-readiness-2026-10-04.md). Previously: Accepted 2026-10-03 by `@nabbisen` ("Confirmed. Accepted.") on the text carrying the security review's three required changes. **Returned to `proposed/` the same day**, 2026-10-03, under RFC 000's return-for-review rule, following an owner-approved material change to D5's scope and to the closure prerequisites. The prior acceptance is preserved here; the design it approved was not found wrong, it was found improvable.
**Amended on.** 2026-10-05 — D3 re-scoped. The origin set it introduced is **write-once at provider creation**: `startup.rs:303-304` seeds only when the slug is absent, `repos/federation_provider.rs` has no `update`, and the only way to change the set is to delete the provider, which cascades (`0038_federation_link.sql:22`) and destroys every federation link to it. D3 now requires a maintenance path. The architect's earlier "admin UI → config field" edit was recorded as correcting a factual error; that was wrong, because a config file does not maintain an existing provider.
**Amended on.** 2026-10-04 — closure criterion 5 narrowed from "the hostile-provider corpus in RFC 096's matrix passes" to this RFC's own transport scope. The corpus is RFC 096's deliverable (096-A and 096-C), spanning JOSE, claims and state/nonce, none of which RFC 134 built or was dispatched for; as written the criterion could never be met by the RFC carrying it.
**Amended on.** 2026-10-03 — D5 restructured. The five rows are now split by **enforceability** rather than by difficulty, after `@nabbisen` asked whether they could be derived from measured reality. Three of the five stop requiring a matrix amendment: see [`../handoffs/134-federation-egress-is-a-policy/d5-can-the-rows-be-measured-2026-10-03.md`](../handoffs/134-federation-egress-is-a-policy/d5-can-the-rows-be-measured-2026-10-03.md). Closure prerequisites updated accordingly.
**Security review.** Required — [security review 2026-10-03](../handoffs/134-federation-egress-is-a-policy/security-review-2026-10-03.md), **by the architect, which authored this RFC, and therefore not independent.** Carried under `ROADMAP.md` R1's residual. It returned **three required changes** (R1 the missing origins column, R2 the `resolve()` bypass, R3 the per-request timeout override), all folded into the text below.
**Independent design review.** [Security review 2026-10-03](../handoffs/134-federation-egress-is-a-policy/security-review-2026-10-03.md) — same document, same limitation. **The field's name overstates it**, as on RFCs 124, 128, 130, 132 and 133.

**Design prerequisites.** None. RFC 096 is Accepted and its normative validation
matrix already states the required behaviour; this RFC decides *how* that
behaviour is obtained and which of its rows survive scrutiny.
**Implementation prerequisites.** This RFC Accepted. **No dependency on RFC 094
M2a** — nothing here is a Class-A durable mutation, so this work can run beside
M2a rather than behind it.
**Closure prerequisites.** Every control in D1–D4 is enforced at a single
construction site; the resolver rejects both edges of every vendored prefix; the
gate fails on a second egress client, on any `resolve`/`resolve_to_addrs`, and on
any per-request timeout in the federation path; every discovery-supplied endpoint
is checked against the provider's origin set; **the transport rows of RFC 096's
matrix pass — the resolver corpus, which D2 satisfies** (narrowed 2026-10-04 from
"the hostile-provider corpus in RFC 096's matrix passes", which named a
deliverable of RFC 096 rather than of this RFC); every Tier 1 bound in D5 is a recorded
measurement with its corpus and headroom, not a chosen number; D5's three Tier 2
toggles are set explicitly rather than inherited; the chain-size row is filed in
a tier with evidence; and the Tier 3 amendment is settled either way.
**Tracks.** ROADMAP M4-A — Federation validation and transport.
**Touches.** `crates/sui-id/src/runtime/state.rs`, a new egress module,
`crates/sui-id/src/http/handlers/federation.rs`, a `federation_provider` migration
and the matching `FederationProviderConfig` field (D3's origin set), `contracts/`, CI.
**Handoff.** [`../handoffs/134-federation-egress-is-a-policy/README.md`](../handoffs/134-federation-egress-is-a-policy/README.md)
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

**This is the highest-value control in this RFC. It is not the cheapest** — the
security review (R1) found that `federation_provider` has an `issuer` column and
**no origins column**, so there is currently nothing to validate against.

A discovery document is fetched from the provider but is not *from* the provider
in any sense the type system knows. Every URL taken from it — `token_endpoint`,
`userinfo_endpoint`, `jwks_uri`, `authorization_endpoint` — is validated before
use:

- it is canonical HTTPS;
- its **origin and port are in the provider's configured origin set**. A discovery
  document may not introduce an origin the administrator did not configure.

**D3 therefore carries a schema change:** a migration adding the origin set (RFC
096's matrix, Origins row: *1–8 explicit canonical origins including issuer*), the
`FederationProviderConfig` field an administrator sets it from, a documented
default for existing rows, **and a path by which an administrator can change the
set on a provider that already exists.**

> **The maintenance path is required, not optional (added 2026-10-05).** Without
> it the origin set is **write-once at provider creation**, and an administrator
> who needs to add a second origin — the Google case this very section cites —
> has only one option: delete the provider and recreate it. That cascades
> (`crates/sui-id-store/src/migrations/0038_federation_link.sql:22`,
> `ON DELETE CASCADE`) and **destroys every federation link to that provider**,
> so every federated user loses their account linkage.
>
> An administrator editing the config file sees no error and nothing happens
> (`crates/sui-id/src/runtime/startup.rs:303-304` is `Ok(_) => {}`), and the
> workaround that does work is destructive in a way nothing warns about. That is
> the confusion this project's standard explicitly rules out.

> **Corrected 2026-10-03, by the architect.** This sentence said "admin UI to
> maintain it". **There is no admin UI for federation providers** —
> `FederationProvider` appears nowhere in `crates/sui-id-web/src`,
> `crates/sui-id/src/http/router.rs` has no provider routes, and
> `crates/sui-id/src/runtime/startup.rs:303-333` seeds providers from
> `cfg.federation_providers` on boot. I wrote "admin UI" without checking how
> providers are configured.
>
> **Recorded as correcting a factual error about the system, not as changing
> scope:** the requirement was that an administrator can maintain the set, the
> config file is how an administrator maintains a provider, and nothing is added
> to or removed from what must hold. **That reading is mine and `@nabbisen` may
> overrule it**, in which case this is a material change and the RFC returns to
> `proposed/` under RFC 000. The shipped implementation (`842b75b`) already
> follows the corrected text; the dev team flagged the contradiction rather than
> amending this RFC themselves, which is the right boundary.

**Deriving the set from `issuer` alone is rejected, not deferred.** Real providers
serve endpoints from a second origin — Google's issuer is
`https://accounts.google.com` while its token endpoint is on
`https://oauth2.googleapis.com` — so an issuer-only rule would reject correct
configurations, and the pressure would then be to weaken the check.

The matrix already requires this (§"Configuration and URLs", Endpoint row:
*Reject: discovery-added origin*). It is stated separately here because it is the
control that makes the rest of the chain sound: the token request carries the
provider's `client_secret`, and whether that credential goes to the provider is
decided entirely by this check.

### D4 — One client, enforced structurally

A gate asserts three things about the federation path. The first is obvious; the
other two are the ones the security review caught, and both bypass the policy
**without** constructing a second client:

1. `reqwest::Client::builder()` appears in the federation egress module and
   nowhere else;
2. **`resolve(` and `resolve_to_addrs(` appear nowhere at all**, including in the
   egress module. reqwest documents that per-name overrides are applied *on top
   of* a custom `dns_resolver`, so one such line pins a hostname to a chosen
   address and D2's resolver never runs for that name;
3. **no per-request `.timeout(` on the federation path.** `RequestBuilder::timeout`
   overrides the client's, and `federation.rs:112` already sets one — so D1's
   total bound would be dead code on the discovery path from the day it lands,
   invisibly, because the two values happen to agree today.

**R2 and R3 are the same mistake as the one this RFC exists to fix:** a control
placed in a constructor is worth only as much as the guarantee that nothing
overrides it, and reasoning about the constructor is not the same as reasoning
about everything that can override it. Controls that live in a
constructor are only as good as the guarantee that nothing else constructs one,
and that guarantee is mechanical or it is nothing.

**HIBP (`crates/sui-id-core/src/authn/hibp.rs`) is out of scope** and keeps its
own client: it talks to one pinned, first-party-chosen endpoint with no
attacker-influenced URL, so the threat model differs. The gate must **name** that
exemption rather than pattern-match around it, so a third client cannot appear by
resembling the exempt one.

### D5 — The five rows, split by enforceability

The first draft of this RFC said these five rows "cannot be met through
configuration of the current stack" and proposed amending all of them. **That was
wrong for three of the five**, and the error was splitting them by how hard they
looked instead of by where they can be enforced. Enforceability is a property of
the stack; difficulty was an opinion.

#### Tier 1 — ours already. Derive each bound from a measured corpus

| Row | Where it is enforced |
|---|---|
| Body: endpoint byte cap, depth/member/string/array caps, duplicate keys | our reader and deserializer |
| Media/status: 200 with JSON or `+json`, conditional 304 under cache rules | our code, after the response returns |

These need no amendment and no library cooperation. Each numeric bound is
**measured, not chosen**: fetch the discovery document, JWKS and userinfo
response from a corpus of real providers — Google, Microsoft Entra, Okta, Auth0,
Keycloak, GitLab, Authentik — record the observed maxima, and set the bound at
observed maximum times a stated headroom.

**The corpus, its date, and each number's derivation are recorded beside the
constant.** A bound nobody can justify is the one that gets raised the first time
something legitimate trips it.

#### Tier 2 — strict by default; make it a requirement instead of an inheritance

`reqwest` 0.13.4 exposes three opt-in laxity toggles:

- `http1_allow_obsolete_multiline_headers_in_responses`
- `http1_ignore_invalid_headers_in_responses`
- `http1_allow_spaces_after_header_name_in_responses`

**All three are called explicitly with `false` in D1's constructor.** They are
named to be opted into, so the defaults are already strict — but calling them
states the requirement *in our source* rather than inheriting it. A future
release that relaxes a default then cannot reach us silently, and no one has to
verify what the default was.

**This removes the one cost the first draft recorded for amending.** The header
envelope row (bare LF, obs-fold, invalid header lines) needs no amendment.

#### Tier 3 — genuinely unreachable. Only these need an amendment

`reqwest` does not expose `hyper`'s header-slot or buffer sizing, and nothing
exposes the handshake byte bound or connection-close behaviour:

- fixed 32 KiB header buffer, 64 slots, 8 KiB scratch;
- handshake ≤256 KiB inbound;
- "declared length read exactly then one-use connection dropped without EOF wait".

For these, and only these, the rows are amended to state the property —
*bounded header allocation; strict framing; no request smuggling* — satisfied by
a dependency floor plus a recorded statement of the upstream behaviour relied
upon, **evidenced rather than asserted**. If a guarantee turns out to be absent,
that row returns as real work.

**Approved by `@nabbisen`, 2026-10-03: "amend".**

#### How the amendment takes effect: RFC 134 supersedes, RFC 096's matrix is not edited

**This RFC supersedes those three rows of RFC 096's validation matrix. The matrix
file is not edited**, beyond a pointer added to it that makes the supersession
visible to anyone reading it.

**Why, and this is a judgement `@nabbisen` should overrule if he meant otherwise.**
RFC 000 states that handoffs "remain companions whose state is inherited from
their RFC". The validation matrix is RFC 096's handoff, so **rewriting a normative
requirement inside it is a material change to RFC 096** — and RFC 096 is Accepted,
so under the same rule just applied to this RFC it would return **RFC 096** to
`proposed/`. That would block M4-A and M4-B outright and require a fresh
independent design review of a large RFC, as a side effect of a narrow transport
decision.

Supersession avoids that, and is better on its own terms rather than merely
cheaper: it keeps the change attached to the RFC and the security review that
produced it, with its date and reasoning. Editing the matrix in place would land
the same words with none of that provenance, and a later reader would find a
relaxed requirement and no account of who relaxed it or why.

**The cost, stated:** the matrix no longer reads as self-contained for those three
rows. The pointer is what pays it, and it is the whole reason the pointer is
mandatory rather than courteous.

#### Certificate chain ≤16/128 KiB — **filed in Tier 3, 2026-10-05**

**Approved by `@nabbisen`, 2026-10-05**, on the architect's recommendation. The
Tier 3 amendment therefore covers **four** rows, not the three approved on
2026-10-03.

**Why it cannot be Tier 1.** `reqwest`'s `TlsInfo::peer_certificate()` returns
`Option<&[u8]>` — **the leaf certificate only, not the chain** — so nothing we
can reach measures a 16-certificate / 128 KiB chain.

**Why the alternative was rejected.** A `rustls` `ServerCertVerifier` does see
the intermediates and could measure them before delegating to the real verifier.
**That puts our code in the certificate-verification path to enforce a byte
bound, and the cost of a mistake there is accepting a bad certificate** — far
worse than this bound being absent.

So the row joins the other three: amended to the property it exists to secure,
satisfied by the pinned dependency floor and the recorded statement of upstream
behaviour, evidenced rather than asserted.

## Multiple implementation steps

1. **D1 + D4, including D5 Tier 2.** The constructor and the gate that keeps it
   the only way in, with the `resolve`/`resolve_to_addrs` and
   per-request-timeout prohibitions, and the three `http1_*` strictness toggles
   set explicitly. Smallest step, and it closes redirects, proxy, ALPN, the TLS
   floor and the header-envelope row at once.
2. **D3.** Endpoint-origin validation, with its migration and config field.
   Highest value, and larger than step 1 — this ordering was reversed in the
   first draft on a cost claim the security review falsified. **Landed
   `842b75b`.**
3. **D2.** The validating resolver and its vendored prefix table — the largest
   piece, and the one whose tests are the matrix's resolver corpus.
4. **D5 Tier 1.** The provider corpus measurement, and the body and media/status
   bounds derived from it. Independent of the amendment question, so it does not
   wait on anything.
5. **The chain-size row.** Determine whether `tls_info` can evidence it, and file
   it in Tier 1 or Tier 3 with that evidence.
6. **D5 Tier 3.** Only after the amendment question is answered — the only step
   that waits.

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

## Gate Matrix lane owned by RFC 134

Registered through the multi-source lane registry (RFC 094 R10), as RFC 098,
116 and 117 do. The heading above is the recorded source heading and is
matched by plain equality; do not rename it without changing the manifest in
the same commit. Column layout mirrors RFC 093's table so one parser reads
both. Added with step 1 (D4).

| ID | Toolchain | Features | Blocking command / assertion |
|---|---|---|---|
| G19 | Python 3.14 | n/a | `python3.14 scripts/check-federation-egress.py --root .` |

## Open questions

**Q2 is withdrawn** — it asked whether D3 could be built before acceptance, and
the RFC is now Accepted, so the question has no content. **Q3 is withdrawn by me
as mistaken**, with the reasoning kept below because the mistake is instructive.
**Q1 is the only one still open, and it is not mine to settle.**

### Q1 — D5 **Tier 3** only. **Recommendation: amend.** Still open.

**Narrowed.** This question first covered five rows. After the enforceability
split it covers three: the header buffer and slot sizing, the handshake byte
bound, and dropping without EOF wait. Tier 1 and Tier 2 need no amendment, and
the chain-size row is unresolved rather than in scope here.

Amend those three to state the property — *bounded header allocation; strict
framing; no request smuggling* — and satisfy them by a dependency floor plus a
recorded statement of the upstream behaviour relied upon.

**Why.** Literal compliance means replacing `hyper` and `rustls` with our own
HTTP/1.1 parser and TLS bounds, to match numbers whose security value lies in a
bound existing rather than in its being exactly 32 KiB. Hand-written HTTP parsers
are a classic source of request-smuggling and memory-safety defects. Trading a
widely reviewed implementation for an unreviewed one makes the system less safe
while making the matrix look satisfied.

**The cost of amending is now much smaller than the first draft claimed.** That
draft recorded the risk as "we rely on an upstream guarantee, and a relaxed
default reaches us silently". Tier 2 removes that for every row where a setter
exists, because the requirement is stated in our own source. What remains is
genuine but narrow: for three rows with no setter, we rely on upstream behaviour
and must evidence it at a pinned floor rather than assert it.

**This is not mine to settle** because it amends RFC 096's normative matrix, and
RFC 096 is Accepted.

### Q3 — the 8-address answer limit. **Withdrawn: my question was wrong.**

I asked whether the limit should be 1, on the grounds that nothing in the design
needs more than one surviving address. **That conflated two different numbers.**

- **8 is the cap on the DNS *answer set*** — how many records may come back.
- **1 is the *dial* count** — how many we connect to, which D2 step 4 already
  fixes at exactly one.

Requiring the answer *set* to be 1 would reject ordinary round-robin DNS, which
nearly every real provider uses, and the pressure would then be to weaken the
check — the same failure mode D3's issuer-only shortcut would have had.

**Keep 8.** The security property does not come from the cap. It comes from D2
step 3: **every** returned address must validate, and the whole answer is rejected
if any one of them fails. That is what defeats an answer mixing public and private
addresses, and it is already specified. The cap is only a bound on work.
