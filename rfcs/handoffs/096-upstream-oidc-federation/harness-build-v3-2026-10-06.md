# RFC 096-A prerequisite — harness v3: two differences, and the seam G19 missed

**Dispatched 2026-10-06**, on your step-2 finding. **Supersedes
[`harness-build-v2-2026-10-06.md`](harness-build-v2-2026-10-06.md)'s "exactly
one difference", which was never achievable.** You were right twice.

## The decision: A1

The test client differs from production in **two** named respects: the
**resolver** and **one extra trusted root**. Both in the constructor's
signature.

**A2 is rejected.** `danger_accept_invalid_certs(true)` throws away the property
most worth keeping — that certificate verification runs — to test a byte
fixture. It is the shape of switch RFC 096 `:496` exists to prevent, and being
in test code does not change that.

**Verification stays on. The test client trusts one extra root; it does not
trust anything.**

## 1 — Factor the shared configuration, then the two constructors

One place applies every production setting. `build_federation_client()` passes
no resolver override and no extra root; the `#[cfg(test)]` constructor passes
both.

**If the factoring cannot make both provably apply the same settings, stop
again.** That instruction stands and you have now used it twice correctly.

## 2 — Replace `insecure_test_client()`, do not leave it beside the new one

Your finding: `tls_mock.rs:86-95` omits **six** production settings — the D2
resolver, `no_proxy`, `referer(false)`, `read_timeout`, the TLS version bounds,
and the three `http1_*` flags — and four e2e files use it.

**Move all four to the shared constructor and delete `insecure_test_client()`.**
A weaker client left in the tree is the one the next test will reach for.

**Expect some of those tests to need their expectations revisited** — they have
been running without the resolver and without the TLS bounds. **If any fails
under the production settings, that is a finding, not a chore: report it and do
not loosen the client to make it pass.**

## 3 — Extend G19 to `crates/sui-id/tests/`

G19 scans `crates/sui-id/src/` only, which is why the hand-rolled builder went
unseen. A `Client::builder()` in the test tree is exactly what it exists to
catch. Extend the scan; the shared constructor is the single permitted site.

## 4 — Then the fixture, and the three rows from v2

Unchanged from v2: the raw-byte TLS mode beside `tls_mock.rs`'s existing axum
path, and the three proof rows — bytes arrive, the bounds still apply, and the
production policy row **cited** rather than rebuilt.

## Return

Per-hunk SHA-256 against a stated baseline, evidence that both constructors
apply the same settings, confirmation that a release build contains no seam,
**the result of moving the four files** including anything that failed, and the
full local gate set — **including A3.2, G19 and G20**.
