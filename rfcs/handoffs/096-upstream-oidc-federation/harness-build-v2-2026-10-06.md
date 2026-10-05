# RFC 096-A prerequisite — build the hostile-provider harness (v2)

**Approved and dispatched 2026-10-06**, Option A of
[`harness-redesign-2026-10-06.md`](harness-redesign-2026-10-06.md). RFC 096
`:496` is amended to match. **Supersedes
[`harness-build-2026-10-05.md`](harness-build-2026-10-05.md), whose mechanism
does not exist** — your step 1 finding, which was right.

**096-A is still not dispatched.** This is its last prerequisite.

## The mechanism, corrected

**A raw-byte TLS fixture, reached by a client from the shared production
constructor with only the resolver replaced.**

Not `connector_layer` — you proved it cannot carry bytes. Not a resolver
exemption in production — RFC 096 `:496` forbids it and so do I.

## 1 — The fixture

Extend `tests/e2e/tls_mock.rs`: alongside the existing `axum` path, a mode that
completes the **real** rustls handshake and then writes **hand-built bytes**
from the test.

**`tls_mock.rs`'s existing behaviour is not changed.** It carries the semantic
cases and exercises hyper for real; this adds a second mode beside it.

## 2 — The client, and the one difference

A **`#[cfg(test)]`** constructor in `runtime/egress.rs` that applies **every
production setting through the same code path** and takes only a resolver
override.

**Share the configuration, do not restate it.** Two lists drift, and a drifted
test client stops being evidence about the production one. **If you cannot
factor it so both constructors provably apply the same settings, stop and say
so** — that would mean the difference is wider than one argument and the
evidence is weaker than this design claims.

**G19 still forbids a second `Client::builder()` outside the egress module.**

**Name the difference where it cannot be missed:** in the constructor's
signature, its doc comment, and the fixture's own module header. A reader must
not have to discover that these tests use a different resolver.

## 3 — Rows, and only enough to prove the mechanism

Two or three, not the corpus — the corpus is 096-A's.

1. **Bytes arrive**: a response the fixture fabricates reaches
   `fetch_discovery`.
2. **The bounds still apply**: a lying `Content-Length` is still capped on bytes
   read, through the production `response_bounds` path.
3. **Production policy is unweakened**: the real `build_federation_client()`
   still refuses a name resolving to loopback. `the_real_egress_client_refuses_a_name_that_resolves_to_loopback`
   already proves this — **cite it rather than duplicating it**, and say in the
   package that you checked it still passes.

## What you were right about, carried forward

Your step 1 report noted that two of the three proof tests in the previous
dispatch were already covered — the resolver one above, and
`response_bounds`'s raw-TCP `Content-Length` test. **That is why row 3 is a
citation and not new work.**

## Return

Per-hunk SHA-256 against a stated baseline, confirmation that a release build
contains no seam, evidence that both constructors apply the same settings, and
the full local gate set — **including A3.2, G19 and G20**.
