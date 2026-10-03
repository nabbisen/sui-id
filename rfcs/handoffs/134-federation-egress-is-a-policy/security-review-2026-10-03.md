# RFC 134 — security review

**Date:** 2026-10-03
**Reviewed by:** the architect, **which wrote RFC 134.** Not independent of its
subject, as on RFCs 124, 128, 130, 132 and 133. Carried under `ROADMAP.md` R1.
**Ordering, stated plainly:** this review was written **after** `@nabbisen` said
"RFC 134 is accepted", not before. On RFC 133 the review preceded acceptance and
was input to it, which is the better order. **It returns three required changes,
so the amended text needs confirming before the RFC moves to `accepted/`.**

## Verdict

**Three required changes. The design's shape is sound; one of its cost claims is
false and two bypass routes are unguarded.**

### R1 — D3 cannot ship without a schema change, and the RFC says it can

**The RFC calls D3 "Smallest, highest value, no new dependency. Shippable by
itself" and makes it implementation step 1. There is nothing to validate
against.** `crates/sui-id-store/src/migrations/0037_federation_provider.sql`
defines `federation_provider` with `issuer`, `client_id`, `client_secret_enc`,
`scopes`, `provision_mode`, `enabled` — **and no origins column.**

Validating a discovery-supplied endpoint "against the provider's configured
policy" requires that policy to exist. RFC 096's matrix is explicit that it is a
set, not the issuer alone — §"Configuration and URLs", Origins row: *1–8 explicit
canonical origins including issuer*.

**Deriving the policy from `issuer` alone is not a workaround.** Real providers
serve endpoints from a second origin — Google's issuer is
`https://accounts.google.com` while its token endpoint is on
`https://oauth2.googleapis.com` — so an issuer-origin-only rule would reject
correct configurations, and the pressure would then be to relax the check.

**Required:** D3 carries a migration adding the origins set, admin UI to set it,
and a documented default for existing rows. Its step-1 billing as small and
dependency-free must be corrected — it is the highest-value control and it is
*not* the cheapest.

### R2 — `resolve()` / `resolve_to_addrs()` bypass D2, and D4's gate does not see it

`reqwest::ClientBuilder` has `resolve(domain, addr)` and
`resolve_to_addrs(domain, &[addr])` (`async_impl/client.rs:2291,2299`), and
reqwest's own documentation for `dns_resolver` states: *"Overrides for specific
names passed to `resolve` and `resolve_to_addrs` will still be applied **on top
of** this resolver."*

So a single added line pins a hostname to an address of the author's choosing and
**the validating resolver never runs for that name.** D4's gate searches for
`reqwest::Client::builder()`, which this does not introduce — the bypass lives on
the very builder D4 blesses.

**Required:** D4's gate also fails on `resolve(` and `resolve_to_addrs(` anywhere
in the federation path, including the egress module itself.

### R3 — a per-request timeout silently overrides the client policy, and one already does

`RequestBuilder::timeout` overrides the client's. **`federation.rs:112` already
sets `.timeout(10s)` per request**, so D1's `.timeout(10s)` would be dead code on
the discovery path from the day it lands — and nothing would reveal it, because
the two values currently agree. A later edit to either one silently decides which
bound applies.

**Required:** remove per-request timeouts from the federation path and let the
client policy be the only source; D4's gate asserts their absence. **The general
principle this RFC is built on — one place, enforced structurally — is exactly
what R2 and R3 violate**, and both were missed because I reasoned about the
constructor instead of about everything that can override it.

## Checked and sound

- **D2's placement inside the resolver** is right. Returning exactly one
  validated address removes the check-to-connect window; a pre-flight lookup
  followed by a connect would be the defect, and the RFC says so.
- **D1's builder calls all exist** in reqwest 0.13.4 under this workspace's
  feature set, `rustls → __rustls → __tls` traced rather than assumed.
- **The ID-token/TLS reasoning is correct.** OIDC Core §3.1.3.7 permits it, and
  the RFC correctly makes it conditional on D3 rather than treating it as free.
- **D5's recommendation is right.** Hand-writing an HTTP/1.1 parser to hit
  specific buffer numbers trades a reviewed implementation for an unreviewed one.
  The RFC's refusal to assert hyper's defaults without evidencing them is the
  right standard.

## Not covered

JOSE, claims, state/nonce and cache/rotation rows of RFC 096's matrix. This
review, like the RFC, is transport and endpoint-selection only. **Their state is
unknown, not assumed good.**
