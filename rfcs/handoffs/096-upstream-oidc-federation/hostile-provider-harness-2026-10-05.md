# RFC 096-A prerequisite — the hostile-provider harness

**Date:** 2026-10-05
**Status: APPROVED by `@nabbisen`, 2026-10-05 ("Approved."), and dispatched** —
see [`harness-build-2026-10-05.md`](harness-build-2026-10-05.md). **All four of
096-A's implementation prerequisites are now clear.**

**Originally written as: design, for approval, not dispatched.** RFC 096-A's implementation
prerequisites name *"the hostile-provider harness approved"* as one of four, so
this waits on his sign-off before any dispatch.
**By:** the architect. **This is the last of 096-A's four prerequisites** — the
`federation.rs` split cleared in `2f07862`; M1a and RFC 096's amended acceptance
were already met.

## The architecture is already decided, and not by me

RFC 096 `:496`:

> *"Production code has no test-only private-address switch. **The hostile
> fixture injects a transport trait beneath policy instead.**"*

That settles the shape. **Policy runs for real; the transport is swapped.** The
DNS address validation (RFC 134 D2), the TLS floor, the redirect refusal, the
timeouts and the response bounds all execute exactly as in production — only the
thing that moves bytes is a fixture.

**Why that matters and why the alternative was rejected:** a test-only
private-address switch would mean production code containing a branch that
disables the SSRF control. The control would then be one `cfg` away from off,
and the thing under test would no longer be the thing that ships.

## The seam exists — verified, not assumed

`reqwest` 0.13.4's `ClientBuilder::connector_layer` (`async_impl/client.rs:2458`)
takes a Tower `Layer` over the base connector `Service`. **That is beneath
policy by construction**: redirect refusal, timeouts, TLS bounds and the custom
`dns_resolver` are all configured above it and keep running.

**No new dependency.** `tower` is already in the graph via `reqwest`.

## Where it goes, and the G19 constraint

**G19 forbids a second `Client::builder()` anywhere on the federation path
except the egress module.** So the harness does not build its own client. The
seam is a **`#[cfg(test)]` constructor in `runtime/egress.rs` beside
`build_federation_client()`**, taking the layer and applying every production
setting unchanged.

That keeps one builder, in one place, with the gate still enforcing it — and a
test client that differs from the production client in exactly one respect,
stated in its own signature.

## The gating pattern is this project's own

RFC 094's fault injector is the model: *"`#[cfg(test)]`-gated end to end — zero
cost and zero behavior change in a normal build"* (`registry.rs:766-770`). The
same discipline applies here.

And RFC 094's argument for having a seam at all is the argument for this one:

> *"An injection seam is not optional hardening. It is the only thing that
> exercises these paths at all, and the runner was wrong for as long as it was
> missing."*

That RFC found a runner that committed on a domain error and had done since it
was written, because nothing exercised the path. **The federation transport
paths are in exactly that position today.**

## What this harness does that `tls_mock.rs` cannot

RFC 134 step 2 built `tests/e2e/tls_mock.rs` — a real self-signed-TLS server.
**It stays, and it is the right tool for most cases**: it exercises rustls and
hyper for real, which a fixture does not.

But a well-behaved `axum` server **cannot produce** the corpus RFC 096 requires:
bare LF and obs-fold, a declared length that disagrees with the body, stacked or
unknown transfer encodings, chunk extensions over the limit, a TLS version below
the floor, an oversized handshake, a response that stops mid-body, or a second
DNS answer. Those are protocol-level abuse, and they need a transport that is
not an HTTP server.

**So the two are complements and the dispatch will say which corpus rows belong
to which.** Replacing `tls_mock.rs` would lose real-stack coverage; replacing
the fixture would lose the abuse cases.

## What I am not deciding here

- **The corpus itself.** RFC 096's matrix and §"Hostile-provider" rows are the
  specification, and they are large. **This is the harness, which is the
  prerequisite — not the tests it will carry.**
- **Whether every abuse row is reachable through `connector_layer`.** The layer
  sits above the socket, so it can return arbitrary bytes; I believe that covers
  the HTTP-level rows. **The TLS-level rows — version floor, oversized
  handshake — may need the fixture to speak TLS itself**, and I have not
  verified that `connector_layer` is the right altitude for them. The dispatch
  must require that to be measured, not assumed.

## What I need

**Approval of the harness shape**: transport injected beneath policy via
`connector_layer`, through a `#[cfg(test)]` constructor in the egress module,
complementing rather than replacing `tls_mock.rs`.

With that, 096-A's four prerequisites are clear and I can dispatch the harness,
then 096-A itself in stages.
