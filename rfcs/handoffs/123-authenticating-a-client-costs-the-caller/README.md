# An endpoint that authenticates a client costs the caller something

**RFC.** [RFC 123](../../proposed/123-authenticating-a-client-costs-the-caller.md), **Proposed**.
**Author.** High-capability model, requirements-architect role.
**Baseline.** `4ebf0f7` or later.

## Measured at `e1a251d` — confirm at the baseline

`oauth_token.rs:56` (introspect) and `:119` (revoke) call no
`enforce_rate_limit`; only `/oauth2/token` does (`oidc.rs:361`).
`authenticate_client` (`sui-id-core/src/oidc/oauth_token.rs:267-294`) runs
Argon2 at `:291` for a known confidential client and returns earlier for an
unknown, public or disabled one. Confirm the Argon2 parameters actually
configured, since D1's severity is the memory cost per call.

## What to build

RFC 123 D1–D4.

**State the limits you chose and the traffic you chose them for.** An
introspection endpoint is called by a resource server, possibly on every request
it serves; a limit sized for a browser breaks it. Say what shape you assumed and
why. If the honest answer is that the right bucket is per-client rather than
per-IP, say that, and note that it requires authenticating before limiting,
which is the ordering D2 is about — work the tension through rather than
picking one.

**D4 is a measurement, and it produces a committed record**, not a conclusion in
a chat. Time both paths, enough samples to say something, in a stated
environment; put the numbers and the method in the package and in a file under
this handoff. Then recommend. A measurement nobody can re-run is an opinion.

## Evidence

- A test that each endpoint refuses past its limit, and one that a legitimate
  caller within it is unaffected.
- The D4 measurement, as a committed file with its method.
- Mutations: remove each limit; move the hashing back before the cheap checks.
- fmt, both clippy scopes, the test count before and after, MSRV, every doc gate.

## What to return

The usual package, with the limits and the traffic assumption behind them, the
D4 numbers, and your D3 answer.
