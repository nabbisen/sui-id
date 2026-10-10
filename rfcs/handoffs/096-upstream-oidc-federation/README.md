# RFC 096 developer handoff

**Governing RFC:** [RFC 096](../../accepted/096-upstream-oidc-federation-validation.md)
**Audience:** the mid-capability model only after the applicable entry gate
**Status:** Planning companion; inherits the governing RFC's current status — **Accepted 2026-08-27**, the 2026-07-28 return to `proposed/` for a material prerequisite and staging amendment having closed. **096-A is under way as of 2026-10-07**: all four entry gates are satisfied. Stages 1–5 have landed (stage 5 at `2a6c35d`, Level B); stage 6a is the open work, and **096-A does not close until stages 6a–9 land** — see **Open work** below, which is the current index. See **Open work** below, which is the current index.

This package decomposes the federation trust completion without reopening RFC
004's stable mapping policy or RFC 094's atomic-audit architecture.

## Companion files

- [architecture.md](architecture.md) — target components, state transitions,
  migration, cache, activation, and ownership.
- [validation-matrix.md](validation-matrix.md) — normative transport,
  metadata, JOSE, claim, mapping, and error rules.
- [verification.md](verification.md) — hostile fixtures, concurrency/fault
  evidence, live canary, and closure bundle.

## Entry gates

**096-A** may start only when RFC 096 is Accepted, its design review has no
blocker/high finding, **RFC 093's M1a lanes pass on a clean commit** (M1b is not
a prerequisite), a clean baseline commit is recorded, the preparatory
`federation.rs` split has landed with its observational-equivalence record
independently reviewed, and file ownership is non-overlapping.

*Corrected 2026-08-12 after independent review finding B-096-1. This gate
previously named "pure stages 1–3", but stage 3 was schema migration and provider
preflight — durable mutation, which 096-A prohibits. See RFC 096's work map.*

RFC 096's RFC 094 design was independently reviewed, the complete material RFC
was durably returned to Proposed in commit
`43085e38219e5eb1bfe11cc698b18f1fa5f5e4d7`, and `@nabbisen` explicitly
accepted the complete amended RFC on 2026-07-21. This prerequisite is satisfied.
**096-B1** (attempt state, nonce consumption, session establishment) requires
amended RFC 094 **M2a**'s **runner foundation** — sealed
`declare_write_command!`, `ReadConn`, the command manifest, and both the
`WriteTx<Protocol>` and `WriteTx<AtomicAudit>` runners — which M2a delivers
before its conversion waves. It does **not** require membership of any
conversion wave: F01–F06 belong to none, and 096-B1 implements them against the
seam. F04 is Class-A, so the Class-A runner is required, not optional. **096-B2** (provider and link commands C17/C18/C23, preflight, audited
enablement) requires RFC 094 **M2b**, with C17/C18/C19/C23/F01–F06 passing. If RFC 095
implementation is active, its owner must release any shared OIDC/session/
migration file explicitly. The roadmap's second implementer and independent
reviewer requirement applies to any approved overlap.

**Hold on F01 and F03 (2026-09-16).** They establish sessions. `@nabbisen` ruled
that a sign-in which cannot be audited does not succeed, and
[RFC 102](../../done/102-authentication-fails-closed-without-audit.md) carries the
ruling: session establishment becomes a Class-A command, not RFC 096's
Protocol-plus-Class-B shape. F01 and F03 do not start until RFC 102 is Accepted,
so they are built fail-closed once rather than rebuilt. F02, F04, F05 and F06 are
unaffected. **Released 2026-09-17:** RFC 102 is Accepted; F01 and F03 are built to
it (its L04 is their predecessor).

**Finding for 096-B1 (2026-09-17, RFC 102 stage 4).** The shipped callback stores
`FedState.next` at start but never reads it: every federated sign-in lands on
`/admin`, or `/admin/login/mfa`. A non-admin federated user therefore cannot
complete an OIDC authorize flow through federation. When B1 honours `next`,
apply RFC 102 A7 before F01 commits: refuse an admin-only destination for a
user who cannot read it, before any write.

## Frozen boundaries

- Public canonical HTTPS issuer and endpoint origins only; no private-network
  exception.
- All IANA special-purpose and explicit translation/tunnel/anycast prefixes,
  including both NAT64 prefixes, are denied regardless of embedded address.
- Deployments provide native IPv4 for IPv4-only upstreams or provider-native
  ordinary public IPv6; DNS64/NAT64 compatibility is intentionally absent.
- Provider-specific callback plus mandatory RFC 9207 `iss`.
- Durable one-winner attempt before token exchange; no cookie-only fallback.
- Mandatory signed ID token; no userinfo request.
- Closed asymmetric JOSE algorithms and public-key profiles.
- `(provider_id, sub)` lookup only; verified-email provisioning only; no email
  merge.
- Unknown `link_only` identity fails closed; self-service linking is not built.
- Upstream MFA never satisfies local MFA; federated primary method survives the
  local challenge.
- No stale JWKS/discovery acceptance; closed HTTP freshness/304 rules and
  bounded provider-wide unknown-`kid` refresh.
- HTTP/1.1-only bounded parser/framing with fixed pre-response allocation and
  all trailers rejected; HTTP/2 is not offered; rustls and handshake/chain/
  record bounds are pinned and evidenced.
- C17 consumes ephemeral exact-activation-generation preflight and increments
  generation on enable/disable; C23/C18 also invalidate generation; exact RFC
  094 C23/F01–F06 ownership; only
  C23/F04 are new Class A, while F01–F03/F05/F06 preserve Protocol/Class-B.

## File ownership

Expected scope includes federation configuration/types/repositories/handlers,
a dedicated egress module, JOSE/discovery/cache modules, the next migration,
MFA primary-method context, RFC 096 fixtures, and operator/security docs.

Do not refactor unrelated outbound HTTP, change RFC 094 capabilities/events,
expand dynamic registration, add upstream protocols/providers, build account
link UX, trust upstream groups/MFA, or alter general OIDC provider behavior.

Before coding, return the exact intended file list and migration number. Stop
if another owner holds one of them.

## Ordered delivery

**096-A** — no durable state is created in this stage.

1. Pure types, configuration validation, canonical URL rules, bounded JSON,
   JOSE header/key/claim validators.
2. Resolver policy, pinned transport, discovery/JWKS cache, injected hostile
   fixture, ID-token verification and nonce validation rule.

**096-B1** — requires RFC 094 M2a.

3. Migration for attempt state, typed repositories, attempt creation/claim,
   provider callback, token exchange.
4. Verified identity mapping, provisioning, local MFA/session preservation,
   session establishment, insecure legacy-path removal.

**096-B2** — requires RFC 094 M2b.

5. Provider/link commands on the Class-A seam, version/generation invalidation,
   preflight and audited enable integration.

**096-C**

6. Full adversarial/fault/migration evidence, live canary, docs, handoff, and
   independent closure request.

*Re-cut 2026-08-12 to match RFC 096's work map. The previous ordering placed
migration and audited enablement in the "pure" prefix and bundled M2a- and
M2b-dependent work into one stage.*

Every stage must compile independently. A provider cannot be enabled until all
runtime enforcement for its stored configuration is present. Test-only network
injection must be structurally unavailable in production construction.

The operator preflight report records compatibility with the deliberate
HTTP/1.1 profile: ALPN presence/absence, connection-close bodies, chunked
responses, declared-length responses whose socket remains open, and rejection
of informational chains/trailers. It offers no generic-client fallback.

## Stop conditions

Stop and return for architecture/security review if implementation would:

- make canonicalization, special-address policy, algorithm list, cache TTL,
  skew, or attempt state weaker/dynamic;
- accept an IANA special-purpose, NAT64, 6to4, Teredo, mapped/compatible, or
  otherwise explicit transition/anycast destination;
- allow discovery to authorize a new endpoint origin or follow a redirect;
- perform DNS after address validation without revalidating and pinning it;
- decode claims into mapping code before signature validation;
- accept an absent nonce/ID token/`kid`/`iat`, or use a symmetric algorithm;
- retry a consumed attempt or return it to pending;
- use email for existing-account mapping or provision without verified email;
- restore the unsigned pending-link cookie or build linking without a new RFC;
- lose `Fed` as primary authentication across local MFA;
- accept stale keys after refresh/expiry or refresh once per attacker `kid`;
- exceed header/ETag bounds, raise zero/short freshness to a minimum, use 304
  without exact retained validated state, or leave cache-flight precedence
  discretionary;
- enable from persisted/replayed/stale/wrong-generation preflight authority,
  omit an activation-generation increment, accept HTTP/2, leave rustls bounds
  unrecorded, grow the header parser, or accept any response trailer;
- log upstream credentials, claims, URLs, bodies, DNS answers, email, or sub;
- infer safe trust configuration for a legacy provider;
- bypass/overload RFC 094 C17/C18/C23/F01–F06, nest Class-A commands, split their
  compound rows, or call link-observation primitives directly;
- allow more than five bound MFA failures, reset count by restart/method
  switch, or consume method anti-replay state without the session; or
- overlap an active RFC 094/095 implementation without recorded ownership and
  independent capacity.

## Completion return

Return a release handoff that cites the governing RFC and all three companion
files. Include clean commit identity, exact commands and observed results,
migration fixtures/report, IANA registry snapshot, transport construction
proof, hostile-provider corpus, concurrency/fault reconciliation, cache/clock
evidence, sanitized telemetry, live-canary procedure/result, remaining risks,
and the independent closure-review path. Do not claim a gate from a checklist.

## Review records

Relocated here on 2026-09-10 when `rfcs/reviews/` was retired — RFC 000
sanctions four lifecycle folders plus the optional `draft/` and this
non-lifecycle `handoffs/` companion folder, and per-RFC review records are
companion documents, not a sixth lifecycle.

| Record | What it is |
|---|---|
| [`096-design-review-2026-07-21.md`](096-design-review-2026-07-21.md) | Original independent design review, cited in RFC 096's lifecycle history |

RFC 096's current **Independent design review** and A6 sweep evidence are
the shared three-RFC records, held under
[`../094-transactional-audit/`](../094-transactional-audit/README.md#review-records).

## Open work

**Updated 2026-10-07.** This section was stale: it still described 096-A as
blocked on the JOSE decision and named harness v3 as the open work, both of
which were overtaken days ago, and it did not mention stages 1, 2 or 3a at all.
A reader of this file would have concluded that 096-A had not started. **That
was my omission** — three dispatches written without updating the index the
implementation role reads.

### Dispatched and open

**096-A's fifteen stages are complete.** The architect's assessment is
[`096-a-completion-assessment-2026-10-09.md`](096-a-completion-assessment-2026-10-09.md)
— Level B on CI run `37911563157` (commit `56c91de`, 27 jobs, 0 skipped,
covering all 24 `[gate_owners]` entries). It **recommends** acceptance and does
not grant it. `:23` reserves that to `@nabbisen`, *"on the architect's
assessment"*. **RFC 096 does not
close here** — closure is per stage, 096-B1/B2/C remain, and the RFC stays
Accepted.

Three limitations are part of what is being accepted, not caveats to it: stages
6a–7 are **entirely dormant** (zero production callers, which is `:74-75`
working as written); discovery is live for nine of eleven metadata rules while
the two cross-checks and the tightened bounds are inert; and 096-A does not
close the shipped defect.

### Landed

- [`stage9-the-remaining-corpus-rows-2026-10-09.md`](stage9-the-remaining-corpus-rows-2026-10-09.md)
  — **landed `56c91de`, Level B.** The Claims and Discovery sections of
  `validation-matrix.md`, each row carrying the layer that proves it, plus the
  three rows the matrix demanded and nothing tested: the exponential NumericDate
  spelling, a multi-byte Unicode `sub` proven to return byte-identical, and the
  well-known derivation vectors driven end to end through real TLS — including
  the RFC 8414 insert-before-path form, which must never be requested.

### Dispatched and open

- [`stage1-attempt-state-migration-2026-10-10.md`](stage1-attempt-state-migration-2026-10-10.md)
  — **DISPATCHED, this is the open work.** Migration `0046` and the
  `federation_login_attempt` schema of RFC 096 `:577-589`. Schema only: creating
  an attempt is stage 2, claiming it stage 3. `:23` names migration evidence as
  a closure item, so it is produced here rather than reconstructed later.

### Landed

- [`stage0-readconn-2026-10-09.md`](stage0-readconn-2026-10-09.md)
  and
  [`stage0-fix-side-effecting-readonly-statements-2026-10-10.md`](stage0-fix-side-effecting-readonly-statements-2026-10-10.md)
  — **landed together as `e3ccb76`, Level B, 24/24 gates green.**
  **RFC 094's `ReadConn` — declared a required M2a control and absent for weeks
  — is now built.** The write surface is absent from `ReadStatement`'s type
  rather than refused at runtime; three `compile_fail` fixtures assert the
  `functions`/`vtab`/`load_extension` features stay off by probing the
  *resolved* feature set, which manifest parsing cannot do because Cargo's
  unification can enable a feature from anywhere in the graph; and 67 of 68 read
  sites are converted, the one exception reported.

  Returned once, because `sqlite3_stmt_readonly` is insufficient on its own —
  rusqlite's own source says so above the binding. SQLite's documentation names
  seven statements it reports read-only; the transaction grammar adds `END`;
  `EXPLAIN` reports whatever it wraps. **The returned fix also closed a hole in
  the part already accepted:** the original check trimmed whitespace but not
  comments, and twelve state-changing assignment PRAGMAs report read-only, so
  `/* x */ PRAGMA foreign_keys = OFF` was bypassed by the comment, passed by the
  interrogation, and would have disabled foreign-key enforcement on a pooled
  connection. Nothing vulnerable reached the repository — the holed check existed
  only in an uncommitted tree.

### Landed

- [`stage9-the-remaining-corpus-rows-2026-10-09.md`](stage9-the-remaining-corpus-rows-2026-10-09.md)
  — **landed `56c91de`, Level B.** The Claims and Discovery sections of
  `validation-matrix.md`, each row carrying the layer that proves it, plus the
  three rows the matrix demanded and nothing tested: the exponential NumericDate
  spelling, a multi-byte Unicode `sub` proven to return byte-identical, and the
  well-known derivation vectors driven end to end through real TLS — including
  the RFC 8414 insert-before-path form, which must never be requested.

### Dispatched and open

- [`stage0-fix-side-effecting-readonly-statements-2026-10-10.md`](stage0-fix-side-effecting-readonly-statements-2026-10-10.md)
  — **DISPATCHED, this is the open work.** One item on an otherwise accepted
  stage 0. `ReadConn` accepts a class of statements `sqlite3_stmt_readonly`
  reports read-only that nonetheless have effects — `ATTACH`, `DETACH`, `BEGIN`,
  `COMMIT`, `ROLLBACK`, `SAVEPOINT`, `RELEASE`. Proven by executing `ATTACH`
  through a legitimate `&ReadConn` and watching the file appear. Transaction
  control is the sharper half: a `BEGIN` inside a `with_read` closure outlives
  the closure on a pooled connection. The fix is the first-token refusal the
  stage already built for `PRAGMA`, extended to the class — their reasoning one
  step further, not a correction of it.
- [`stage0-readconn-2026-10-09.md`](stage0-readconn-2026-10-09.md)
  — **RETURNED**, everything but that item accepted: the `ReadConn`/`ReadStatement`
  types with `execute` absent from the type rather than refused at runtime, the
  `sqlite3_stmt_readonly` interrogation, the unconditional `PRAGMA` refusal
  (found by reading `rusqlite`'s own `// does not work for PRAGMA` at
  `raw_statement.rs:239`), five `compile_fail` fixtures, three feature
  assertions that probe the *resolved* feature set rather than the declared one —
  stronger than manifest parsing, because Cargo's unification can enable a
  feature from anywhere in the graph — and 67 of 68 READ sites converted with
  the one exception reported.

  **It also corrected the dispatch's site count**, which had repeated a figure
  the audit's own review had flagged as unreliable: 68 READ across 26 files, not
  77 across 23. The audit's WRITE figure was wrong too, 95 rather than 86, and
  68 + 95 = 163 reconciles exactly to its own non-test total.

### Landed

- [`stage9-the-remaining-corpus-rows-2026-10-09.md`](stage9-the-remaining-corpus-rows-2026-10-09.md)
  — **landed `56c91de`, Level B.** The Claims and Discovery sections of
  `validation-matrix.md`, each row carrying the layer that proves it, plus the
  three rows the matrix demanded and nothing tested: the exponential NumericDate
  spelling, a multi-byte Unicode `sub` proven to return byte-identical, and the
  well-known derivation vectors driven end to end through real TLS — including
  the RFC 8414 insert-before-path form, which must never be requested.

### Dispatched and open

- [`stage0-readconn-2026-10-09.md`](stage0-readconn-2026-10-09.md)
  — **DISPATCHED, this is the open work.** 096-B1's stage 0: `ReadConn` as a
  typed read-only handle, the per-statement `sqlite3_stmt_readonly`
  interrogation, the `functions`/`vtab`/`load_extension` feature assertion, and
  the conversion of the 77 READ sites the audit identified. The 86 WRITE sites
  are not candidates and stay as they are.

### Landed

- [`read-path-audit-result-2026-10-09.md`](read-path-audit-result-2026-10-09.md)
  — **answered: no.** No non-test call site reached through
  `with_conn`/`with_conn_sync` whose name signals a read also writes. Recorded in
  git because it is not a security finding, per its dispatch's own rule. The
  verdict survived an independent check: a mechanical hunt for the defect shape
  across every lookup-named non-test function produced two candidates, both
  false positives of the architect's own regex. The audit's own unreconciled
  count was the architect's error — the dispatch's 332/184 was inflated by 19
  non-call-sites, and 313 is correct. Two of its observations went into stage 0:
  the `sqlite3_stmt_readonly` check must run continuously rather than be an
  audit result, and every WRITE site is a single-statement `with_conn` write, so
  stage 0 converts only the READ list.

### Next: 096-B1

- [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md)
  — **AUTHORIZED 2026-10-09.** Nine stages, checked against
  `:23` and `:83-85` **before** stage 1 rather than after stage 9, which is how
  096-A's plan came to be nine stages for fifteen stages of work.
  **096-B1 is the stage that closes the shipped defect** (`:87`): everything
  096-A built in stages 6a–7 has zero production callers, and stage 4 is where
  every validator gains its first.
  `:104-105`'s open question is resolved in M2a's favour — `Database::protocol()`
  is a complete runner — so 096-B1 does **not** move to M2b.
- [`readconn-prerequisite-decision-2026-10-09.md`](readconn-prerequisite-decision-2026-10-09.md)
  — **settled.** `ReadConn`, one of the four M2a foundation pieces `:89-93` names
  as required, did not exist; reads go through an untyped connection and neither
  control RFC 094's amendments attach to it was built. M2a closed legitimately
  against its seven criteria, which never mention it — the second instance of a
  prose-versus-criteria divergence in that RFC. **Building it is 096-B1's stage
  0.**

### Open with the owner

- [`exp-boundary-strictness-2026-10-08.md`](exp-boundary-strictness-2026-10-08.md)
  — **DECISION REQUEST, blocking nothing.** RFC 096 `:662` states `exp`'s rule
  as the strict `now < exp + 60s`, while `:663` and `:664` state `iat`'s and
  `nbf`'s non-strictly. `jsonwebtoken` accepts at `now <= exp + 60`, so the RFC
  refuses one instant that the implementation accepts. Recommendation: amend
  `:662` to non-strict, matching its own two siblings and what has always been
  built. Enforcing it in our own code instead would put one rule in two places,
  which is the reasoning this RFC's `aud` design already rejected.

### Landed

- [`stage8-the-discovery-profile-2026-10-09.md`](stage8-the-discovery-profile-2026-10-09.md)
  and
  [`stage8-fix-pin-the-shared-caps-and-canonical-urls-2026-10-09.md`](stage8-fix-pin-the-shared-caps-and-canonical-urls-2026-10-09.md)
  — **landed together, Level B.** The eleven metadata rows, the document bounds,
  the canonical-URL rule, and `check_caps` parameterised so one walker serves
  two sets of numbers with JWKS's provably unchanged. Returned once: nothing
  pinned which caps the *shared* path passed, and a conforming ID token at RFC
  096's own claim bounds is 3,023 bytes, so a mix-up would have rejected tokens
  the RFC declares valid. `:555`'s *"noncanonical URL"* was also unenforced —
  `check_endpoint` returned the raw string, so an explicit `:445`, an uppercase
  host and a `/a/../token` path all survived unnormalised.
  The IP-literal/reserved-hostname reversal is **confirmed**: an IP-literal URL
  is canonical, `:296`'s reserved-host rule guards the operator's own issuer
  rather than an upstream's claim, and loopback is already denied by RFC 134
  D2's resolver. Probing the canonical predicate found only one divergence, an
  IDN host, where refusal is correct — a non-ASCII host is an IRI and the
  A-label is the canonical URI.

  **Discovery's attack category closes at the validation layer, not on the live
  path.** `validate` has one non-test caller, in a file 096-A may not touch, so
  nine of eleven rows are live and the two cross-checks plus the tightened
  bounds are not. A standing limitation for the closure assessment.

- [`stage7-the-nonce-rule-2026-10-09.md`](stage7-the-nonce-rule-2026-10-09.md)
  — **landed, Level B. Token substitution closes with it:** `aud`, `iss` and the
  nonce are all bound now, which is what defeats a substituted token. The rule
  only — the expected digest is a parameter, since what makes a nonce genuinely
  one-time is a mutation and belongs to 096-B1 (`:67-68`). It corrected this
  dispatch's own import path: `tokens` is declared `#[path = "oidc/tokens.rs"]
  pub mod tokens` off `sui-id-core`'s root, so `oidc` is a directory, not a
  module segment. The `ct_eq` -> `==` mutation survives and is reported as
  surviving: a constant-time property is not observable from a return value, and
  a test contrived to appear to catch it would be worse than the honest report.

- [`stage6c-optional-claims-and-the-capability-2026-10-08.md`](stage6c-optional-claims-and-the-capability-2026-10-08.md)
  and
  [`stage6c-fix-bound-boundaries-and-mailbox-shape-2026-10-09.md`](stage6c-fix-bound-boundaries-and-mailbox-shape-2026-10-09.md)
  — **landed together, Level B.** The eight bounded optional claims and the
  sealed construction capability of RFC 096 `:689-691`, which carries no raw
  token, nonce or access token. Returned once: every numeric bound could be
  tightened by one with all 487 tests passing, and because `:681-683` rejects
  the whole token for a malformed optional claim, a bound one too tight fails
  the login rather than truncating a hint. Nine accepting-side tests now pin
  each bound at its limit — nine rather than the seven I asked for, because
  `preferred_username` and `name` are different constants and one test would
  have killed the mutation while proving nothing about the other. The mandatory
  dot also came out of the mailbox shape: `user@intranet` must not cost a user
  their login over a claim that is metadata only and never a lookup key.
  Its byte-before-scalar check order is derived, not guessed — both limit pairs
  are exactly `4 x scalar_limit` and UTF-8 caps a scalar at four bytes, so
  checking scalars first would leave the byte branch unreachable.

- [`stage6b-time-claims-2026-10-08.md`](stage6b-time-claims-2026-10-08.md)
  — **landed, Level B.** `nbf` turned on, `iat` written with `created_at` as a
  parameter so its lower bound refuses a token minted before the attempt began,
  and a NumericDate shape pre-check for `exp`/`nbf` before `decode` — because
  `jsonwebtoken`'s `numeric_type` *rounds* a float into an accepted integer and
  degrades a string to "not present". An expired token is now `Expired` rather
  than `SignatureInvalid`, closing stage 6a's defect one claim over. Corrected
  two assertions in my dispatch: the library's `exp` boundary is non-strict, not
  "exactly the rule", and "`exp` is done" held for the window but not the shape.
  Their own mutation also found a hole in their own tests — the `chrono`-range
  branch was unreachable from both existing tests, which used `u64::MAX` and
  failed earlier at `i64::try_from`.

### Landed

- [`stage6a-required-identity-claims-2026-10-08.md`](stage6a-required-identity-claims-2026-10-08.md)
  and
  [`stage6a-fix-duplicate-members-2026-10-08.md`](stage6a-fix-duplicate-members-2026-10-08.md)
  — **landed together as `259e1fc`, Level B confirmed on the tip `78a223d`, 24/24 gates green.** `iss`, `sub`,
  `aud`, `azp`: byte-exact issuer binding, client-ID containment, the
  multi-audience `azp` rule, and the `aud` check stage 4a deferred — kept in our
  own validator because `jsonwebtoken`'s `Validation::aud` is an intersection
  test with no count bound, no uniqueness check and no concept of `azp`.
  Returned once: a repeated `iss`/`sub`/`aud` was refused as `SignatureInvalid`
  on a token whose signature was valid, because `jsonwebtoken` parses the
  payload a second time into a struct that rejects those three duplicates
  itself. The scan moved into `verify_id_token_against_jwks` before `decode`
  with its own error variant, which deleted `raw_payload` and four other things
  — the fix was net-negative in lines, and stopped retaining nonce-bearing
  bytes that RFC 096 `:689-691` forbids the capability to hold.
  **One of 096-A's four open attack categories closes: issuer.**

**096-A is fifteen stages, not nine.** Stage 5's closure assessment
(`.git-exclude/reviewed/rfc-096-a-stage5-hostile-provider-corpus-2026-10-08.md`)
checked 096-A against RFC 096 `:23`'s closure prerequisite rather than against
the stage plan, and found two whole areas of its scope sentence (`:59-63`)
unimplemented:

- **Claim validation and the nonce rule.** Four of the prerequisite's nine
  attack categories are open — token substitution, nonce, issuer and audience.
  `validate_aud = false` at `id_token.rs:399`, no issuer comparison anywhere,
  and `nonce()` at `:378` exposes the claim without comparing it.
- **The upstream discovery profile.** `discovery.rs` validates the configured
  issuer and every endpoint origin (RFC 134), but the metadata table at
  `:539-551` requires twelve members and `RawDiscovery` deserializes four. The
  document's own `issuer` is never read, and nor are
  `code_challenge_methods_supported`,
  `authorization_response_iss_parameter_supported` or
  `id_token_signing_alg_values_supported`.

**Both omissions were the architect's stage plan, not the RFC**, which required
this work throughout — so no amendment is needed, only the missing stages.
Remaining after 6a — **6b** time claims, **6c** bounded optional claims and the
construction capability, **7** the nonce rule, **8** the discovery profile,
**9** the corpus rows stage 5 could not cover.

### Landed

- [`stage5-hostile-provider-corpus-2026-10-08.md`](stage5-hostile-provider-corpus-2026-10-08.md)
  — **landed `2a6c35d`, Level B, 24/24 gates green.** The audit found the corpus
  already built by the per-stage mutation discipline: 101 tests mapped to the
  matrix rows in a checkable table, every name verified to exist in the module
  claimed. One real gap closed — RFC 096's *"Expired + network failure: Reject;
  no stale acceptance"* had only a cooldown test with nothing in the slot. The
  new `an_expired_entry_is_never_served_when_its_refresh_fails` is the sole
  test of the fourteen that catches removal of the freshness guard.

- [`stage4d-flights-and-cooldowns-2026-10-08.md`](stage4d-flights-and-cooldowns-2026-10-08.md)
  and
  [`stage4d-fix-authority-on-the-read-paths-2026-10-08.md`](stage4d-fix-authority-on-the-read-paths-2026-10-08.md)
  — **landed together.** The eight-row state table, single flight, the 30-second
  cooldown, the 60-second forced window measured from dispatch, rotation's
  no-fallback rule — plus the returned-and-fixed authority check on both read
  paths, and a `flight_generation` counter closing the ABA case where a
  recurring `CacheKey` would let a superseded flight clobber a newer one.
  **096-A's cache work is complete.**

### Landed

- [`stage4c-revalidation-and-version-binding-2026-10-08.md`](stage4c-revalidation-and-version-binding-2026-10-08.md)
  — **landed.** 200/304 revalidation, the 304 metadata rules, and `CacheKey`
  binding provider, version and activation generation — with `RetainedEntry`
  unconstructible outside the success paths, and `apply_304` taking no body
  parameter so it cannot validate a different one. 55 tests.

- [`stage4b-cache-directives-and-freshness-2026-10-07.md`](stage4b-cache-directives-and-freshness-2026-10-07.md)
  — **landed, with one required change carried into 4c.** The closed directive
  set, `Age`/`Date`/`ETag` validation and freshness arithmetic, as pure
  functions reusing `sui_id_core::time::Clock` rather than a new abstraction.

- [`stage4a-signature-verification-2026-10-07.md`](stage4a-signature-verification-2026-10-07.md)
  — **landed `8a697b1`.** Signature verification, the crypto error variants,
  and the two deferred refusals. `VerifiedIdTokenClaims` is unconstructible
  outside the verifying functions, proven by a compile-fail fixture.

- [`stage3b-jwks-key-selection-2026-10-07.md`](stage3b-jwks-key-selection-2026-10-07.md)
  — **landed `911dfed`.** RFC 096 `:643-648`'s selection rules, 13 refusal
  variants, 28 new tests. The private-key rule reads the raw JSON, because the
  library's typed struct silently drops `d`/`p`/`q`; proven by mutation.

- [`stage3a-jwks-transport-and-bounds-2026-10-07.md`](stage3a-jwks-transport-and-bounds-2026-10-07.md)
  — **landed `2526313`.** `parse_compact_jws`'s own 24 KiB input bound,
  `jwks_uri` in discovery (optional, because a required field would change
  live deserialization), the JWKS fetch on the RFC 134 federation client, and
  the four JWKS limits `response_bounds` does not already provide.

- [`stage2-compact-jws-and-header-2026-10-07.md`](stage2-compact-jws-and-header-2026-10-07.md)
  — **landed `5ed86e9`.** Compact-JWS structural rules and header hygiene, 35
  tests, reachable only by tests.
- [`stage1-id-token-algs-config-2026-10-07.md`](stage1-id-token-algs-config-2026-10-07.md)
  — **landed `9baf106`.** The `jsonwebtoken` dependency with the `aws_lc_rs`
  backend, `clippy.toml` barring the two unsafe entry points, and
  `id_token_algs` with six refusals at config load.
- [`jose-strategy-2026-10-06.md`](jose-strategy-2026-10-06.md) — **settled
  2026-10-07, no longer blocking.** `jsonwebtoken` with the `aws_lc_rs`
  backend for the upstream path only; `oidc/jwt.rs` stays EdDSA-only and
  untouched. Kept for the reasoning and the three rejected alternatives.
- [`harness-build-v3-2026-10-06.md`](harness-build-v3-2026-10-06.md) —
  **landed `7c068f8`.** Two named differences (resolver and one extra trusted
  root), `insecure_test_client()` replaced, G19 extended to the test tree.
- [`federation-split-2026-10-05.md`](federation-split-2026-10-05.md) —
  **landed `2f07862`.** The preparatory `federation.rs` split, one of 096-A's
  four implementation prerequisites.

### Planned, not yet dispatched


  *Stage 4's cache work was split into 4b/4c/4d on 2026-10-07, after reading
  `:825-926` line by line rather than by its heading. It is three separable
  concerns, and one dispatch covering all of them would be the largest of this
  RFC and effectively unreviewable. **Nothing in 096-A routes to production**;
  096-B1 does that.*
- **Stage 5** — the hostile-provider corpus driving every negative row of the
  validation matrix.
- **Stages 6a, 6b, 6c** — the required claim matrix (RFC 096 `:654-690`):
  identity claims, time claims, then the bounded optional claims and the
  construction capability that carries no raw token or nonce.
- **Stage 7** — the nonce validation *rule*: constant-time digest comparison
  against a supplied expected digest. The durable attempt state that makes a
  nonce genuinely one-time is 096-B1, per RFC 096 `:67-68`.
- **Stage 8** — the upstream discovery profile: the twelve-member metadata
  table at RFC 096 `:539-551`, the document bounds at `:532-535`, and the
  `id_token_signing_alg_values_supported` intersection that constrains the
  runtime algorithm set.
- **Stage 9** — the corpus rows stage 5 could not cover because nothing
  implemented them: substitution, nonce, issuer, audience, time, discovery.

  *Added 2026-10-08. The stage plan was checked against 096-A's closure
  prerequisite and its scope sentence term by term, and was found short a
  claims stage and a discovery stage. The plan was short, not the RFC.*

### Superseded, kept for the record

- [`harness-build-v2-2026-10-06.md`](harness-build-v2-2026-10-06.md) — its
  "exactly one difference" was not achievable; TLS trust is a second.
- [`harness-build-2026-10-05.md`](harness-build-2026-10-05.md) — its mechanism
  does not exist. (This entry previously carried a contradictory "DISPATCHED —
  this is the open work" line left behind by an earlier edit; removed.)
- [`harness-redesign-2026-10-06.md`](harness-redesign-2026-10-06.md),
  [`hostile-provider-harness-2026-10-05.md`](hostile-provider-harness-2026-10-05.md)
  — the design these builds implement, approved 2026-10-05.
