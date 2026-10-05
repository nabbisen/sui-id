# RFC 096-A prerequisite — build the hostile-provider harness

**Approved and dispatched 2026-10-05.** The design is
[`hostile-provider-harness-2026-10-05.md`](hostile-provider-harness-2026-10-05.md);
this is the build instruction. **096-A itself is still not dispatched** — this
is its last prerequisite, not its first stage.

## Step 1 — measure the altitude question before building anything

I flagged in the design that I had **not** verified whether every abuse class is
reachable at `connector_layer`'s altitude. **Settle that first and report it,
because it decides the rest.**

`ClientBuilder::connector_layer` (`reqwest-0.13.4/src/async_impl/client.rs:2458`)
wraps the base connector `Service`. A layer there yields the **byte stream**, so
HTTP-level abuse — bare LF, obs-fold, a lying `Content-Length`, stacked or
unknown transfer encodings, oversized chunk extensions, a body that stops
mid-stream — should be straightforward: return the bytes you choose.

**TLS-level abuse is the open question.** A sub-floor TLS version or an
oversized handshake has to be produced *by the thing speaking TLS*. Whether the
connector layer sits above or below `rustls` in this stack determines whether
the fixture can speak TLS itself or is handed an already-negotiated stream.

**Report which of these is true, with the evidence.** If TLS-level rows are out
of reach there, say so and stop — do not improvise a second mechanism. That
would be a design change and it is mine to make.

## Step 2 — the seam

A **`#[cfg(test)]`** constructor in `crates/sui-id/src/runtime/egress.rs`,
beside `build_federation_client()`, taking the layer and applying **every
production setting unchanged**.

**Share the settings, do not restate them.** If the two constructors list the
builder calls separately, they will drift, and the test client will stop being
the production client. Factor the common configuration so there is one list.

**G19 forbids a second `Client::builder()` on the federation path outside this
module**, so the constructor lives here and the harness calls it. Do not add a
builder in the test tree.

**Follow RFC 094's fault injector for the gating** — `#[cfg(test)]`-gated end to
end, zero cost and zero behaviour change in a normal build
(`registry.rs:766-770`). A release build must not contain the seam.

## Step 3 — the fixture, and only enough corpus to prove it works

Build the transport fixture and **two or three** tests — not the corpus. The
corpus is RFC 096's matrix and belongs to 096-A.

What the proof tests must show:

1. **The fixture is reached** — a response the fixture fabricates arrives at
   `fetch_discovery`, so the seam is actually in the path.
2. **Policy still runs above it** — the clearest demonstration is that the
   **DNS address validation still rejects a denied address even with the
   fixture installed**. If swapping the transport silently bypasses RFC 134 D2,
   the harness is worse than nothing and this test is how you find out.
3. **One protocol abuse the real TLS mock cannot produce** — a lying
   `Content-Length` is the cheapest, and RFC 134 D5 Tier 1 already requires the
   byte cap to hold on bytes read rather than on the header, so there is an
   existing assertion to point at.

## What stays as it is

**`tests/e2e/tls_mock.rs` is not replaced and not modified.** It exercises
rustls and hyper for real, which the fixture does not, and it carries the
semantic cases. The dispatch for 096-A's corpus will say which rows go where;
until then, nothing moves between them.

## Return

Per-hunk SHA-256 against a stated baseline, **the step 1 altitude finding with
its evidence**, confirmation that a release build contains no seam, and the full
local gate set — **including A3.2, G19 and G20**.
