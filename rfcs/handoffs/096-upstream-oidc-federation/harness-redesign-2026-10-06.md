# Hostile-provider harness — design revision after the altitude finding

**Date:** 2026-10-06. **Supersedes the mechanism in
[`hostile-provider-harness-2026-10-05.md`](hostile-provider-harness-2026-10-05.md),
which `@nabbisen` approved on my evidence. That evidence was wrong.**
**Option A approved by `@nabbisen`, 2026-10-06 ("A. Your recommendation is
accepted"), and dispatched** — see
[`harness-build-v2-2026-10-06.md`](harness-build-v2-2026-10-06.md). RFC 096
`:496` is amended in place; the reasoning for treating that as non-material is
recorded there.

## What is now known

**`connector_layer` cannot carry a fixture.** Measured by the implementation
role in a compiled external probe: the response type is sealed, so a layer can
only forward, refuse, delay or count. No bytes in, none observable, at any
altitude.

**And the production client cannot reach a local fixture.** RFC 134 D2's
resolver denies loopback (`resolver.rs:92,205`). `tls_mock.rs` only works by
swapping in a separate `insecure_test_client()`.

**Together these close the shape RFC 096 asked for.** `:496` says *"the hostile
fixture injects a transport trait beneath policy"* and forbids a *"test-only
private-address switch"* in production. There is no transport trait to inject,
and every way of reaching a fixture needs the address policy relaxed somewhere.

## The decision

**Option A — a raw-byte TLS fixture, reached by a test client built from the
shared production constructor with only the resolver replaced.**

Extend `tls_mock.rs` to write hand-built bytes after a real rustls handshake
instead of serving through `axum`. Reach it with a client from the **same**
constructor as production, differing in exactly one documented respect.

- **Gets:** every HTTP-level abuse row, over a real handshake, through the real
  hyper parser and the real response bounds.
- **Costs:** the tested client is not byte-for-byte the production client. The
  difference is one argument, in one constructor, named in the signature.
- **On `:496`:** production code gains **no branch and no switch** — the
  difference lives entirely in test construction. I read that as satisfying the
  prohibition's purpose while not matching its stated mechanism. **That reading
  is the decision.**

**Option B — accept that these rows are not testable here, and say so.**

Drop the abuse rows from 096-A's corpus with the reason recorded, and rely on
`response_bounds`'s existing raw-TCP coverage plus hyper's own test suite.

- **Gets:** no new mechanism, no deviation from `:496`.
- **Costs:** RFC 096's hostile-provider corpus ships materially incomplete, and
  the rows it drops are the ones a hostile provider would actually use.

**I recommend A**, because B leaves the corpus claiming less than its name
promises. But A requires amending RFC 096 `:496`, which is an Accepted RFC, and
that is not mine to do.

## What I am not proposing

**Not a resolver exemption.** A test-only allowance for a private address in the
production resolver is the exact thing `:496` forbids, and it would put the
weakening in the control itself rather than beside it.

## My part in this

I verified `connector_layer` existed and wrote a design on its doc comment
without checking an external crate could use it. The owner approved that design
on my evidence. This is the sixth time I have written a requirement from an
unchecked source, and the second since recording the lesson.
