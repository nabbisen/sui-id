# RFC 096-B2 stage 6b — the issuer host rule

> ## Whose turn it is: **the implementation role's.**
> Cycle L, 096-B2 S6b —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §6:
> *"096-B2 does not close without it."* **This document is the dispatch.**

**Dispatched.** 2026-11-12, by the architect.
**Governing RFC.** [RFC 096](../../accepted/096-upstream-oidc-federation-validation.md),
**Accepted**, `:355` (the issuer row: *"no IP literal; no `localhost`,
`.localhost`, `.local`, `.internal`, or `.home.arpa`"*) and `:374-377`
(*"Private, loopback, link-local, or literal-IP issuers are not supported in
M4. There is no configuration escape hatch."*). Building is permitted.
**Baseline.** The tip of `main` when you start (`git log -1`; record it). S6
must be in it.

## 0 — what this stage is, and why it was deferred

S2a built every issuer rule except the host rule (`federation_policy.rs`,
`TODO(096-B2 issuer host rule)`). Enforcing it would have refused every e2e
test's mock upstream, which is served at `https://127.0.0.1:PORT`
(`tests/e2e/tls_mock.rs`, `serve_https`). **This stage moves the mocks to a
name, then enforces the rule.** In that order, so the suite is green at each
step.

**The pieces already exist.** The mock certificate already covers the name
`fixture.test` (`tls_mock.rs:39`). `FixtureResolver` already resolves that
name to loopback (`:195-215`). And `federation_test_client` already takes
the caller's resolver (`:220`). `.test` is reserved for testing (RFC 6761),
and is not on RFC 096's refused list.

## A — measure first, then move the mocks

1. **Measure, and put the table in the return.** List every file and test
   that reaches a `serve_https` mock, and the resolver each client is built
   with. That is at least 11 files calling `serve_https`, and 20 naming
   `ValidatingResolver`. Separately, list every test that **asserts
   something about an IP-literal issuer or endpoint itself**, for example an
   RFC 134 egress row. Those test a rule and must keep testing it; they are
   not fixtures to move.
2. **`serve_https` returns `https://fixture.test:PORT`.** It still binds
   `127.0.0.1:0`. Check how reqwest takes the port before relying on it: with
   a resolver, the connection should use the URL's port, so one
   `FixtureResolver` that maps `fixture.test` to loopback serves any number
   of mocks in one test. **Prove it with a test that runs two mocks on two
   ports through one client.** If reqwest does not behave that way, stop and
   report.
3. **The clients that reach those mocks use the fixture resolver.**
   `ValidatingResolver` refuses loopback by design (RFC 134 D2). Keep it, and
   its own tests, wherever a test is about that refusal.
4. **The whole suite is green at this point**, before §B. Say so in the
   return, with the run.

## B — the rule

- In `federation_policy::validate_issuer`, at the `TODO`: refuse an issuer
  host that is:
  - an IPv4 or IPv6 literal;
  - `localhost`;
  - or a name ending `.localhost`, `.local`, `.internal` or `.home.arpa`.

  Compare case-insensitively, and treat a trailing dot as no dot. Its own
  refusal message names the rule and RFC 096 `:374-377`'s no-escape-hatch
  sentence, so an operator with an internal IdP learns this is deliberate,
  not a typo.
- **One issuer check, not two.** `discovery::validate_issuer`
  (`discovery.rs:511`) checks only the scheme. S3's review recorded that it
  should collapse into the policy's. Every path that validates an issuer
  uses the one rule.
- **Tests:** each refused form (`127.0.0.1`, `[::1]`, `10.0.0.1`,
  `localhost`, `idp.localhost`, `idp.local`, `idp.internal`, `idp.home.arpa`,
  `IDP.LOCAL`, `idp.local.`), and accepted ones that look close
  (`local.example.com`, `internal-idp.example.com`, `fixture.test`).
  **Mutation:** drop one suffix, and quote its test going red.
- **Startup.** A configuration whose issuer breaks the rule refuses to start,
  as S2a's other rules do. A stored row with such an issuer has no usable
  policy, so the callback and the preflight refuse it.

## C — records

- `CHANGELOG.md` `[Unreleased]`, `### Changed`: the issuer host rule joins
  S2a's list of newly enforced rules, worded for an operator with an internal
  IdP.
- `docs/src/reference/configuration.md`'s `issuer` row states the rule.
- Remove the `TODO(096-B2 issuer host rule)`. `grep -rn` must find nothing.

## D — constraints and what to return

- **No tag, no dispatch.**
- Run `bash scripts/local-matrix.sh --changed`, the complete matrix, and
  quote the scope line and any failing check names.
- **Don't edit files while the matrix reads them.**
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage6b-the-issuer-host-rule-2026-11-12.md`,
with:
- the baseline;
- §A.1's measurement table;
- the two-mocks, one-resolver test;
- the green run after §A;
- every test that still tests an IP-literal rule, and why it stays;
- the rule's tests, and its mutation;
- the refusal message's exact text.
