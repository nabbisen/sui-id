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

**Updated 2026-10-14.** **`0.80.0` is released**, **stage 9 landed
`f5b4035`** (one fix round), and **stage 10a has landed** after one fix
round. The F03 seam question is **answered** — option A-plus-four, in
[`f03-seam-decision-request-2026-10-14.md`](f03-seam-decision-request-2026-10-14.md)
— and its amendment has **landed**: RFC 096 returned to `proposed/` and was
re-accepted the same day, in one pass, on `@nabbisen`'s approval. `:759-760`
now says which half of "handlers can neither" the compiler holds and which a
source-scope test does. **Stage 10b is dispatched**, carrying that test in its
own scope. A second dispatch, the CHANGELOG catch-up, is open alongside it:
cycle D is gated on stage 12, so the entries for stages 7–10a are written now
while what each one does and does not deliver is still easy to say honestly.
Stage 10
is split into **10a** (the `FederatedMfaVerifier`, its binding gate and its
candidates, no writes) and **10b** (F03's four guarded outcomes), a
scheduling change the architect proposed and the stage plan records as
awaiting authorization. Stage 6d is
deferred to after stage 11 by the 2026-10-12 authorization. Its RFC 094 record
question is taken in the gap before stage 12; the section further down carries
it.

### Dispatched and open

### Landed, 2026-10-15

- [`stage10b-fix-a-name-of-its-own-2026-10-15.md`](stage10b-fix-a-name-of-its-own-2026-10-15.md)
  — **accepted and landed `f3e6925`, nothing folded in. Level B: 24/24 locally, CI run `38665382301`, 27 jobs, 0 skipped**, all 24 `[gate_owners]` entries matched both ways. Stage 10b's fix round, four items, two blocking. F03's
  `Promoted` success needs a name of its own — the architect's dispatch told
  them to reuse `auth.federation.signin.success`, which cannot carry both a
  Class-A and a Class-B guarantee, and the local path has answered this all
  along with `auth.mfa.success` beside `auth.login.success`. And `LinkChanged`
  is declared but unproducible because nothing checks the link, so an
  administrator who unlinks inside the 300-second window still gets a session.
  Three of the round's findings were the architect's. **The second of those two
  was itself half wrong**: the unlink scenario does not reproduce, because
  migration 0047's composite foreign key cascades the pending row away with the
  link. The gap was real and the check earns its place, but for the narrower
  case the fix round measured — a link missing without a cascade propagates a
  raw store error instead of a clean `Invalidated`.

**Stage 11 is dispatched** —
[`stage11-the-third-method-2026-10-15.md`](stage11-the-third-method-2026-10-15.md):
F06, the `ValidWebAuthn` candidate, the verifier's WebAuthn path and F03's
WebAuthn branch, as one stage. The plan's own line said F06 alone; the drift
was the architect's and is corrected there. F06 by itself would write a
ceremony nothing reads, and stage 10's split bought a decision that here is
already made.

### Returned, 2026-10-15

- [`stage10b-f03-the-four-guarded-outcomes-2026-10-15.md`](stage10b-f03-the-four-guarded-outcomes-2026-10-15.md)
  — **returned.** F03's four guarded Protocol outcomes, the failure-count
  machine at `:766-772` including the equality boundary when a wrong and a
  correct proof race at count 4, the three new Class-B events (none of which
  exists in either the matrix or the reference yet), and the source-scope test
  the seam decision made part of this stage. No WebAuthn branch — F06 is stage
  11's — and no handler; stage 12 wires it.
- [`changelog-catch-up-2026-10-15.md`](changelog-catch-up-2026-10-15.md) —
  **accepted and landed `b33f5d5`.** It also struck one of the three release
  notes the architect had been carrying as owed: the directory-bind guard
  resolves to a pre-federation LDAP fix, so nothing changed and there is no
  note to write. Originally: Stage 7 is the
  only one of them an operator gains anything from; the entries have to say
  that, and the audit-event change has to be unmissable because it is the only
  one that can break something already relied on.

### Settled — the F03 seam

- [`f03-seam-decision-request-2026-10-14.md`](f03-seam-decision-request-2026-10-14.md)
  — **answered 2026-10-14: option A-plus-four.** What is still open is the
  lifecycle step the answer entails, drafted at the end of the same file: the
  RFC 096 amendment is material under RFC 000 `:43-47`, so RFC 096 returns to
  `proposed/` and is re-accepted before stage 10b, 11, 12, 14 or 6d can be
  built. Stage 10a is landed and unaffected. The question the document
  originally asked: F03 is Class P and must live
  in `sui-id-store`, which cannot name a `sui-id-core` type, so it will be a
  `pub` command taking plain primitives — and `sui-id` handlers already call
  `sui_id_store::commands::*` directly in eight places. RFC 096 `:758-762`'s
  "handlers can neither construct a candidate nor supply a row-touching
  reason" therefore holds at the `sui-id-core` → `sui-id` boundary and not at
  the store one. Two options, a recommendation (hold the seam with a contract
  and a gate rather than moving argon2 into the database crate, because the
  move breaks again at stage 11's WebAuthn), and one question. Stage 10b is
  not dispatched until it is answered.

### Landed, 2026-10-14 (second)

- [`stage10a-fix-a-row-the-verifier-can-actually-read-2026-10-14.md`](stage10a-fix-a-row-the-verifier-can-actually-read-2026-10-14.md)
  — **accepted and landed.** Stage 10a's fix round, seven items,
  three blocking. The package is accepted in substance — the placement
  argument, the candidate family, the two binding properties, the four
  `trybuild` fixtures and a self-found `Malformed` fix all stand. What does
  not: migration 0047's own CHECK forces `continuation_sealed IS NULL` once
  `status` leaves `'pending'`, while the row type holds a `Vec<u8>`, so
  **`find_by_id` cannot read a terminal row at all** — measured
  (`InvalidColumnType(5, "continuation_sealed", Null)`), which makes the
  verifier's status guard unreachable and blocks stage 10b. Stage 9's accepted
  R3 was also reversed without mention, leaving its defaults test true for one
  of the three columns it names. And seven `map_err(|_| BindingFailure)` turn
  a database outage into the same value as a wrong cookie — which is why the
  first defect was invisible. All seven returned correct, with one test folded
  in by the architect at acceptance: collapsing `classify_store_error` so
  every store error became a binding refusal — the defect the round existed
  to remove — left all twelve tests passing, and a corrupted `secret_enc` now
  fails exactly that mutation. Result:
  `.git-exclude/reviewed/rfc-096-b1-stage10a-fix-a-row-the-verifier-can-actually-read-2026-10-14.md`.

### Reviewed and returned, 2026-10-14

- [`stage10a-the-federated-mfa-verifier-and-its-candidates-2026-10-14.md`](stage10a-the-federated-mfa-verifier-and-its-candidates-2026-10-14.md)
  — **DISPATCHED, this is the open work.** The gate in front of every
  federated second-factor submission (RFC 096 `:752-762`), and **it writes
  nothing**. Three things the dispatch asks for before code. **Where the
  verifier can live is decided by the crate graph, not by taste**:
  `sui-id-core` depends on `sui-id-store` and not the reverse, `webauthn-rs`
  is core's, and F03 must be in the store crate because `WriteTx::tx()` is
  `pub(crate)` — so a candidate constructible only by the verifier and not by
  a handler needs an answer, and three are offered. **The binding gate must
  fail indistinguishably** — the same error value for a wrong cookie, a wrong
  CSRF token, an unknown pending id and someone else's pending id — and must
  leave the row untouched, both proven with a control. **The candidate family
  earns a compile-fail test**: `:758-762` says handlers can neither construct
  a candidate nor supply a row-touching reason, and the existing `trybuild`
  harness is how this project states "by construction". The contrast to name
  is `SecondFactorProof`, L02's own analogue, which is a `pub enum` anything
  can build. **Returned for a fix round** — two of the round's items were the
  architect's own errors: the dispatch named `sui-id-store`'s `trybuild`
  harness, which cannot name a `sui-id-core` type at all, and gave
  `totp::verify`'s signature without its `async`.

### Landed, 2026-10-14

- [`stage9-fix-a-binding-the-browser-can-still-present-2026-10-14.md`](stage9-fix-a-binding-the-browser-can-still-present-2026-10-14.md)
  — **landed `f5b4035`, Level B 24/24** plus A3.2, A3.4, the four fixture
  suites and a fresh workflow check, **CI run `38491673983`, 27 jobs, 0
  skipped**, all 24 `[gate_owners]` entries matched both ways. Stage 9's fix
  round, six changes on the returned working tree. The transaction, the purge, the audit
  replacement and the returned package's own `Database::protocol`/
  `operational` fix (a two-write Protocol transaction committed its first
  write when the second failed — `Ok(Err(e))` never fired `with_tx`'s `?`)
  are all **accepted as returned**. The one defect is the binding:
  `browser_binding_sha256` stores a digest of `sui_id_fed_browser_binding`,
  and the same response expires that cookie — measured off the real
  router's `Set-Cookie` headers — so no later request can present the value
  the column commits to. The rest are a missing sentence on how each binding
  reaches its consumer, two missing assertions (the three columns the insert
  leaves to `DEFAULT`, and the 24-hour purge boundary the sibling table
  already pins), the absent per-hunk hash table, and two prose sites this
  change falsifies, which are the architect's own omission.
- [`stage9-f02-first-mfa-continuation-and-the-purge-2026-10-14.md`](stage9-f02-first-mfa-continuation-and-the-purge-2026-10-14.md)
  — **returned 2026-10-14, superseded by the fix round above; landed with it
  in `f5b4035`.** F02, the first writer of stage 8's two tables: one
  `db.protocol` transaction holding the guarded attempt completion
  (`exchanging -> completed`) and exactly one `federation_mfa_pending` row.
  Three things beyond the transaction. **The
  purge** `:734` requires is this stage's, with the first writer, and the
  dispatch asks where it is *called from* — a purge nobody calls is the growth
  leak with extra steps. **The rechecks RFC 094 `:625-627` requires** are not
  absorbed by stage 8's new link `FOREIGN KEY`, which gives link existence and
  nothing else — not an active user, not an enabled provider at the attempt's
  exact version and activation generation. And **the audit record is currently
  false**: the legacy branch writes `auth.login.password_ok_mfa_required` for a
  federated sign-in, which for an F04-provisioned user asserts a password that
  does not exist. The replacement is argued, not dictated; the architect leans
  to a federation-specific must-attempt observation and says why. **G20 does
  not apply** — F02 is Class P, so no gate will ask for rollback evidence,
  which is why the dispatch asks for it by hand, with a control.

### Landed, 2026-10-13

- [`stage8-fix-bind-the-link-and-the-parent-row-2026-10-13.md`](stage8-fix-bind-the-link-and-the-parent-row-2026-10-13.md)
  — **landed `d7d5523`, Level B 24/24** plus A3.2 and A3.4, **CI run
  `38418809623`, 27 jobs, 0 skipped.** The returned migration was sound and
  nothing in it was wrong; the round existed because **three obligations its
  clause table assigned to later stages' code are enforceable in the schema**,
  established by building and running them rather than by reasoning. A
  composite `FOREIGN KEY` compares two tables where a `CHECK` cannot, so
  `:736-737`'s exact ceremony/parent binding agreement and `:737-738`'s
  no-later expiry both become constraints; `federation_link`'s own
  `PRIMARY KEY (user_id, provider_id)` binds `:722`'s *link* element, which
  was stored but not held — a pending row was valid for a pair that had never
  been linked, and survived an unlink. Also: an unparseable timestamp silently
  disables the `expires_at = created_at + 300` `CHECK`, because `strftime`
  returns `NULL` and a `CHECK` passes on `NULL`. The migration was edited in
  place: `0047` had never been committed. *Returned and accepted the same day,
  with two cascade tests added by the architect — the package's own evidence
  table claimed the ceremony was removed "transitively" by a user or provider
  delete, but the two tests it named create no ceremony at all. Both
  replacements seed two full chains and assert the survivors by joining the
  three tables, so removing too much and removing too little fail
  differently.*

- [`stage8-migration-0047-mfa-pending-and-ceremony-2026-10-13.md`](stage8-migration-0047-mfa-pending-and-ceremony-2026-10-13.md)
  — **returned, fix round open** (above). Migration `0047`:
  `federation_mfa_pending` and `federation_webauthn_ceremony`, **schema
  only**, the way stage 1 was. What the dispatch asks for beyond the DDL is a
  **clause-by-clause table of `:728-738`** saying, for each obligation,
  whether the schema enforces it or which stage's code must — because "the
  code will check it" is how a constraint becomes nobody's. Four clauses need
  judgement rather than transcription: whether a `CHECK` can express
  `expires_at = created_at + 300`, whether a one-valued `primary_method`
  column should exist, where the ceremony's no-later-expiry rule can live
  given a `CHECK` cannot see another table, and `:738`'s negative requirement
  about two *other* tables, which may not be schema-enforceable at all — the
  architect said so and invited the finding. **The purge is recorded as stage
  9's**, with the first writer. *Returned 2026-10-13: the clause table, the
  `STRICT` demonstration and the fresh-vs-forward identity evidence are all
  sound, and both findings it raised are accepted — including that this
  dispatch's **"G19 is the one to watch — migrations are its subject" was
  false**. G19 is federation egress (RFC 134); no gate in the manifest checks
  a migration structurally, and the architect has recorded that.*

- [`stage7-f04-first-provisioning-2026-10-13.md`](stage7-f04-first-provisioning-2026-10-13.md)
  — **landed `e9d5001`, Level B 24/24** plus A3.2 and A3.4, **CI run
  `38378360118`, 27 jobs, 0 skipped.** Federation's first Class-A
  transaction: attempt completion, the passwordless non-admin user, exactly
  one provider/sub link, the `[Fed]` session, the cap and the sole
  `auth.federation.provisioned` event, all on `tx.tx()`. It replaced **three
  separate untransacted writes**. The three rechecks are the writes' own
  `UNIQUE` constraints rather than a second `SELECT` that could lose the same
  race, and the rollback test carries a control. The LDAP shadow upsert was
  deliberately not reused — its key is a different row shape and on conflict
  it *updates*, so a concurrent provisioning would have been absorbed rather
  than refused. An absent email claim now denies (`:710-711`), which makes
  the operator guide's sentence true for the first time and is a behaviour
  change owed a release note. `L04`'s upstream-`sub` attribute is gone,
  closing an item carried since stage 6b.

### Released

- **`0.80.0`, 2026-10-13, from `7aecd98`** — CI run `38313796878`, 27 jobs, 0
  skipped. Federation stops trusting the transport: the callback verifies the
  ID token's signature and claims, and an attempt is a durable row claimed
  once before the code is exchanged. One security disclosure (the callback
  revealed whether an email was already registered, by two routes, both
  closed), at Low–Medium with no advisory. `target/` cleared at the boundary —
  422 GB apparent, **161 GB actually reclaimed**, `/home` 83% → 75%.


### Landed, 2026-10-12

- [`stage6c-attempt-state-machine-and-f05-2026-10-11.md`](stage6c-attempt-state-machine-and-f05-2026-10-11.md)
  — **landed `87a52aa`, Level B 24/24** plus A3.2 and A3.4, after one fix
  round. The three `:597-599`/`:603-604` divergences are closed: the claim is
  **split** from nonce consumption and runs before discovery and the token
  exchange (moving it back no longer compiles — every terminalization in
  between references a `claimed` that would not yet exist); `:598`'s
  **constant-time browser binding** is compared for the first time, closing a
  limitation carried since stage 4, with an absent cookie reaching the same
  comparison a wrong one does; and `:603-604`'s error response consumes the
  matching attempt without a claim or an exchange, identically whether the
  state is absent, unmatched or found. **F05** terminalizes on all 28
  post-claim failure paths, with `:715`'s *"bounded reason enums"* closed on
  both halves by construction — a new reason cannot be invented without
  extending an exhaustive match, and a `Denied` site cannot carry an
  `UpstreamFailure` detail. **Two standing closure limitations close here**;
  two remain (the tautological superseded comparison, and the legacy mapping
  path until stage 12).

### Previously dispatched, now closed

- [`stage6c-attempt-state-machine-and-f05-2026-10-11.md`](stage6c-attempt-state-machine-and-f05-2026-10-11.md)
  — **DISPATCHED, this is the open work.** Three divergences from
  `:597-599`/`:603-604`, found by checking the callback's actual order of
  operations before dispatching, **all from the architect's own stage 3 and 4
  dispatches**: the claim is bundled with nonce consumption so it waits for the
  ID token and the code is exchanged *first*, where `:597-599` puts the
  `pending -> exchanging` transition **before** the exchange; `:598`'s
  constant-time **browser binding** comparison is absent, so
  `browser_binding_sha256` has been stored since stage 1 and compared against
  nothing; and `:603-604`'s error response does not consume the matching
  attempt. F05 lands with them rather than after, because moving the claim
  earlier widens the window in which a failure leaves a row `exchanging`.
  Carries stage 6b's review item: a descriptor whose declaration drifts from
  the parser's expected shape is dropped silently and its matrix row goes back
  to being checked for nothing — this stage adds four more of them, so it adds
  the guard.

### Settled, and one half reopened

- [`emit-must-attempt-prerequisite-decision-2026-10-11.md`](emit-must-attempt-prerequisite-decision-2026-10-11.md)
  — **the decision taken, and the cost I failed to name.** `emit_must_attempt`
  is built as stage 6b; F05 then uses it, because A6 (`RFC 094 :120`) is the
  reason the Class-B shape is right there — failure to log a denial must never
  prevent the denial, and an atomic F05 would roll the terminalization back on
  exactly the path that most needs the record. F01 goes the other way, Class A
  folded into L04, because `auth.federation.signin.success` is **already**
  atomic in production. **§4 reopens the third item:** amending RFC 094 now may
  be a material change under RFC 000 `:43-47`, which returns an Accepted RFC to
  `proposed/` — build-prohibited, and RFC 094 gates every registry-dependent
  lane. Measured facts that narrow it: no gate ties the manifest's class to RFC
  094's prose, and F01 is two dispatches out. Revised recommendation inside;
  the other two items stand as accepted.
- [`class-b-emission-and-f01-class-2026-10-11.md`](class-b-emission-and-f01-class-2026-10-11.md)
  — the finding and the argument that produced that decision. Kept as the
  record.

### Landed, this round

- [`stage6b-class-b-emission-2026-10-11.md`](stage6b-class-b-emission-2026-10-11.md)
  — **landed `2c92e46`, Level B 24/24** plus A3.2 and A3.4. RFC 094
  `:292-298`'s `emit_must_attempt`, with A6 (`:120`) proven end to end: with
  the audit log broken by a real `BEFORE INSERT … RAISE(ABORT)` trigger, the
  takeover denial still happens and the failure is logged. Class-B events got
  their own descriptor type rather than being forced through
  `declare_write_command!`, whose seal exists to prove membership in a
  transaction a Class-B emission does not have; `AuditClass::MustAttempt` was
  removed rather than left unimplemented, which makes *"every
  `EventDescriptor` is Atomic"* a fact with no other value to write, proven by
  a compile-fail fixture. **It took a fix round**: the new descriptor type was
  invisible to `scripts/rust_registry.py`, so the matrix row it was meant to
  bind was still checked for nothing. Rows actor-checked: 25 → 26. The fix
  round also caught a G16 failure the architect introduced and reported as
  clean — a `contracts/` edit checked against G13 only.

- [`stage6a-attempt-retention-and-terminal-cookies-2026-10-11.md`](stage6a-attempt-retention-and-terminal-cookies-2026-10-11.md)
  — **landed `3c76c6c`, Level B 24/24** plus A3.2 (347 fixture tests) and A3.4.
  `:604-605`'s 24-hour purge, registered in `runtime/gc.rs` beside the existing
  six — the predicate derived against the schema's actual columns, since the
  table has no terminal-transition timestamp, and the serialization checked on
  both sides of what is a `TEXT` comparison. `:600-601`'s cookie expiry made
  **structural**: `federated_callback` and `complete_federated_signin` return a
  `Terminal` newtype whose only constructor removes both federation cookies, so
  a terminal path that forgets them does not compile. Before it, the state
  cookie was expired on 2 of 23 terminal paths and the browser-binding cookie
  on none. One hardening item carried to the next dispatch: the removal cookies
  carry no `Path`, and work today only because every terminal response is
  emitted from `/auth/federated/callback`.

### Landed, and released in `0.80.0`

- [`stage6e-federation-failure-surfacing-2026-10-12.md`](stage6e-federation-failure-surfacing-2026-10-12.md)
  — **landed `92b903e`, Level B 24/24, and released in `0.80.0`.** Nothing reads the `fed_error` query parameter
  (`handlers/admin/auth.rs:37-40` has one field, `next`), so every federation
  failure — including the default `link_only`'s own `link_required` — lands a
  user on a login page that says nothing. A **bounded map** onto a small set of
  user-visible messages, never a rendering of the internal value: collapsing
  the collision-adjacent values onto one message is the security-relevant part,
  since `email_collision` versus `link_required` is the pair whose observable
  difference was the enumeration oracle stage 5 closed. Plus the operator guide
  saying which provision mode to choose and what `link_only` costs in this
  release. Was stage 13; moved to the front on 2026-10-12.

### Authorized 2026-10-12

- [`five-open-decisions-reviewed-2026-10-12.md`](five-open-decisions-reviewed-2026-10-12.md)
  — **APPROVED.** `@nabbisen`, 2026-10-12: "Approved." Three effects, all now
  in the stage plan: the surfacing work became **stage 6e** and is dispatched
  ahead of stage 7, because it is a release precondition rather than a later
  tidy; **stage 6d (F01) moves to after stage 11**, with the RFC 094 amendment
  taken in the gap before stage 12 so no stage is implemented against a
  prerequisite the repository contradicts; and **`0.80.0` is cut after stage
  6e**. The `(A required)` marking on `auth.federation.takeover_blocked` is
  carved out of the 2026-09-24 ruling's twelve — eleven remain for M2b.
  **096-A's acceptance is not part of this** and remains outstanding.

### Open with the owner

- [`five-open-decisions-reviewed-2026-10-12.md`](five-open-decisions-reviewed-2026-10-12.md)
  — **the five open decisions, reviewed against the fundamental philosophy, and
  two of the architect's own recommendations withdrawn.** Read this before the
  four documents below it: it supersedes the release-timing recommendation in
  the stage-plan revision proposal (§5) and the sequencing recommendation in
  the `emit_must_attempt` prerequisite decision (§4). **The release
  precondition is not "stage 6c landed"** — it is that a user reaching the
  federation path either signs in or is told, in the product, what to do, which
  makes the `fed_error` surfacing stage a precondition rather than a later
  tidy. **And F01 is not to be built against a record that says something
  else** — the RFC 094 amendment is sequenced into a gap where nothing is in
  flight, with stage 6d moved after stage 11, rather than parked behind a
  gate's silence. Measured costs for both, including what a `proposed/`
  round-trip does and does not block.

- [`class-b-emission-and-f01-class-2026-10-11.md`](class-b-emission-and-f01-class-2026-10-11.md)
  — **DECISION REQUEST; F05 and F01 wait on it.** Both specify a post-commit
  Class-B `emit_must_attempt` event and **that API does not exist**:
  `registry.rs:118-124` records `AuditClass::MustAttempt` as *"not implemented
  in this Stage-1 slice"*, nothing uses it, `commands/tests.rs:162` asserts no
  descriptor does, and it is in neither RFC 094's M2a foundation list (`:575-579`)
  nor M2a's seven closure criteria — **the third instance** of RFC 094's prose
  declaring a control its criteria do not require, after the M2a exit-criteria
  divergence and `ReadConn`. And a second fact cuts the other way for F01:
  `auth.federation.signin.success` is **already atomic** in production
  (`commands.rs:2144-2148`, L04 on `db.class_a`), so building F01 as recorded
  would downgrade RFC 102's achieved property. Recommendation: F01 Class A
  folded into L04; `emit_must_attempt` built first as stage 6b, then F05 on it;
  both carried by one RFC 094 amendment.

### Authorized

- [`096-b1-stage-plan-revision-proposal-2026-10-11.md`](096-b1-stage-plan-revision-proposal-2026-10-11.md)
  — **AUTHORIZED 2026-10-11** and **folded into the stage plan**, which remains
  the one authoritative order. Kept as the record of the argument: the plan
  under-scoped 096-B1, because `:89-93` assigns **F01–F06** to this phase and
  its stage 6 named F01, F02 and F04 — so F03, F05 and F06 were in no stage,
  along with the `federation_mfa_pending` / `federation_webauthn_ceremony`
  migrations (`:728-738`), `FederatedMfaVerifier` (`:752-765`), `:604-605`'s
  purge and `:600-601`'s terminal-path cookie expiry. 096-B1 is **nine stages
  from closing, not two** — 6 through 14, and **twelve dispatches**, since
  stage 6 runs as 6a–6d. **The architect's error**, found
  before dispatching rather than after stage 8; the plan's own `:23` check was
  real, and `:23` is not the whole scope. The release checkpoint authorized
  with it: cut `0.80.0` after stage 6, then continue toward `0.81.0`.

### Landed

- [`stage5-verified-identity-mapping-2026-10-10.md`](stage5-verified-identity-mapping-2026-10-10.md)
  — **landed `8a8dec4`, Level B, 24/24 gates green**, plus A3.2 (347 fixture
  tests) and A3.4. `identity_resolution::resolve_verified_identity` resolves a
  stage-4 capability to one of five named states; `(provider_id, sub)` is the
  sole lookup key and an existing link authenticates without email. A decision,
  not a mutation — stage 6 acts on it.

  **It took a fix round, and the finding was not in the module.** The module
  returned one generic state for `link_only` as `:699` requires; the live
  callback did not, because the legacy collision block sat before the
  `provision_mode` match. It now sits inside the
  `provision_on_first_login` arm, where `:707-708` puts collision denial, and
  `tests/e2e/r096_b1_stage5.rs` pins the generic result against the live router
  rather than the module. `:714-715` is satisfied on this path in the same
  round — no raw email, `sub` or upstream error text in a log field or an audit
  note — and the fourth such site was found by the implementation role doing
  the full grep rather than the three the review named.

  `/auth/federated/link` and its unsigned pending-link cookie are removed, not
  reused (`:700-701`), and `configuration.md` and `operators.md` now say what
  `link_only` does: no self-service linking in this release, an administrator
  creates the link record. **Two items go to whichever stage carries F04** —
  named that way because the numbering is what the proposal above puts in
  question: `L04`'s audit event carries the upstream `sub`, and
  `operators.md`'s `provision_on_first_login` sentence becomes true only when
  the new resolution is authoritative.

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

### Landed

- [`stage4-code-exchange-and-validation-2026-10-10.md`](stage4-code-exchange-and-validation-2026-10-10.md)
  — **landed `d16b4c3`, Level B, 24/24 gates green. 096-A is no longer dormant:**
  each of its five validators now has exactly one non-test caller, all on the
  live callback, verified by grep. An ID token is mandatory; `MAX_STRING_LEN` at
  8 KiB is the cap that actually binds, tighter than the RFC's own 16 KiB.

  **Two limitations on the standing list.** The superseded-attempt comparison is
  wired and unit-tested but **tautological**: `/start` and the callback both read
  the version from one placeholder returning `(0,0)`, because
  `federation_provider` has no such columns and `C23` is `target-absent` M2b
  work. So an operator changing a provider's `issuer` or `client_id` mid-flight
  is **not** detected today, which is what `:580-581` exists to prevent —
  096-B1's closure must not claim otherwise. And `browser_binding` is generated
  and cookied per the RFC's design but nothing compares it yet.

  Two fixes found while in the area, neither asked for: nonce and state were
  `random_token(16)` where `:564` requires independent 32-byte CSPRNG values —
  two of four under-sized — and `access_token` is now `SecretString` per P6.

### Landed

- [`stage9-the-remaining-corpus-rows-2026-10-09.md`](stage9-the-remaining-corpus-rows-2026-10-09.md)
  — **landed `56c91de`, Level B.** The Claims and Discovery sections of
  `validation-matrix.md`, each row carrying the layer that proves it, plus the
  three rows the matrix demanded and nothing tested: the exponential NumericDate
  spelling, a multi-byte Unicode `sub` proven to return byte-identical, and the
  well-known derivation vectors driven end to end through real TLS — including
  the RFC 8414 insert-before-path form, which must never be requested.

### Dispatched and open

- [`stage4-code-exchange-and-validation-2026-10-10.md`](stage4-code-exchange-and-validation-2026-10-10.md)
  — **DISPATCHED, this is the open work.** **The stage where 096-A stops being
  dormant**: `identity_claims`, `time_claims`, `optional_claims` and
  `identity_capability` have zero non-test callers today and all gain their first
  here. Expected values come from the claimed attempt row, not live config —
  `:580-581` is why. Also the token-response envelope row the 096-A corpus left
  to this stage. *(It also corrects a wrong reason in stages 1–3's dispatches:
  `handlers/federation.rs` was withheld from **096-A**, not until stage 7, so
  this stage edits it.)*

### Landed

- [`stage3-claim-and-nonce-consumption-2026-10-10.md`](stage3-claim-and-nonce-consumption-2026-10-10.md)
  — **landed `d8e10d4`, Level B, 24/24 gates green.** `F08`: `pending ->
  exchanging` in one conditional `UPDATE` whose affected-row count is the sole
  authority — dropping the condition fails the concurrent-claim test 29 of 30
  runs with both callers winning, reproduced independently. Clock regression
  checked before expiry, because `expires_at <= now` alone fails *open* under a
  backward clock. **096-A's `validate_nonce` gained its first production
  caller**, so the nonce is genuinely one-time (`:67-68`), with the digest
  converted to lowercase hex at the boundary — which closes by construction the
  uppercase gap 096-A's review deferred.

  One mutation survived the first pass and was handled properly: disabling the
  early `status` check killed no test, because the conditional `UPDATE`
  independently refuses the same rows. It is not redundant for a row that is
  both non-`pending` *and* clock-regressed, where it decides which error wins;
  the test added for that priority now catches it, and nothing else does.

### Landed

- [`stage9-the-remaining-corpus-rows-2026-10-09.md`](stage9-the-remaining-corpus-rows-2026-10-09.md)
  — **landed `56c91de`, Level B.** The Claims and Discovery sections of
  `validation-matrix.md`, each row carrying the layer that proves it, plus the
  three rows the matrix demanded and nothing tested: the exponential NumericDate
  spelling, a multi-byte Unicode `sub` proven to return byte-identical, and the
  well-known derivation vectors driven end to end through real TLS — including
  the RFC 8414 insert-before-path form, which must never be requested.

### Dispatched and open

- [`stage3-claim-and-nonce-consumption-2026-10-10.md`](stage3-claim-and-nonce-consumption-2026-10-10.md)
  — **DISPATCHED, this is the open work.** `pending -> exchanging` as a single
  conditional `UPDATE` whose affected-row count is the authority, the fail-closed
  `now_at_claim < created_at` check, and 096-A's `validate_nonce` given its first
  production caller. **This is where "the nonce rule" becomes "genuinely
  one-time"** (`:67-68`).

### Landed

- [`stage2-durable-attempt-start-2026-10-10.md`](stage2-durable-attempt-start-2026-10-10.md)
  — **landed `226d1a6`, Level B, 24/24 gates green.** `F07`, Class P: the
  `pending` row with its PKCE verifier sealed under an AAD binding attempt id,
  provider id, config version and activation generation. The AAD separators are
  load-bearing — unseparated, version 1/generation 23 and version 12/generation 3
  both concatenate to `"123"`, making two attempts' verifiers interchangeable.
  That binding also buys tamper-evidence that fails closed: altering any of the
  four bound columns makes the verifier unopenable, so a superseded attempt stops
  being claimable by construction.

  Extending RFC 094's numbering is established practice — `U37` by RFC 103 stage
  3, `O05`/`O06` by RFC 124, the whole `L` family by RFC 102 stage 2. *(Stage 2's
  own citation for this was wrong: `U30`–`U33` are RFC 094's own, added in
  `d70ead3`; RFC 102 only corrected their rationales.)*

### Landed

- [`stage9-the-remaining-corpus-rows-2026-10-09.md`](stage9-the-remaining-corpus-rows-2026-10-09.md)
  — **landed `56c91de`, Level B.** The Claims and Discovery sections of
  `validation-matrix.md`, each row carrying the layer that proves it, plus the
  three rows the matrix demanded and nothing tested: the exponential NumericDate
  spelling, a multi-byte Unicode `sub` proven to return byte-identical, and the
  well-known derivation vectors driven end to end through real TLS — including
  the RFC 8414 insert-before-path form, which must never be requested.

### Dispatched and open

- [`stage2-durable-attempt-start-2026-10-10.md`](stage2-durable-attempt-start-2026-10-10.md)
  — **DISPATCHED, this is the open work.** The `pending` attempt row, the PKCE
  verifier sealed with AAD binding the four values that are already columns on
  it, the 600-second lifetime and an injected clock that fails closed on
  regression. **It carries two corrections to the authorized plan**, both the
  architect's: four of five F-command assignments were wrong, and **none of the
  101 registered write commands creates an attempt row or performs the claim**,
  so this stage needs a manifest row that does not exist or G17 fails. Whether
  extending RFC 094's F-numbering is the implementer's to do is left open rather
  than assumed.

### Landed

- [`stage1-attempt-state-migration-2026-10-10.md`](stage1-attempt-state-migration-2026-10-10.md)
  — **landed `0ed0db5`, Level B, 24/24 gates green.** Migration `0046`,
  `federation_login_attempt`. Hashes are `BLOB` in a `STRICT` table, which does
  more than the length `CHECK` alone: a 32-character hex string cannot
  masquerade as 32 raw bytes, because `STRICT` refuses `TEXT` in a `BLOB`
  column. `provider_id` carries no `FOREIGN KEY`, argued from `:580-581` — the
  version and generation columns exist so a superseded attempt stays
  *identifiable*, and a `CASCADE` would make it vanish.

  **One property is not schema-enforced and stage 3 must carry it:** a claimed
  row can be walked back to `pending` by `UPDATE`. The table `CHECK` makes
  `status` and `claimed_at` *agree*; it cannot make the transition one-way,
  because a row `CHECK` never sees where a row came from. Single-use is a
  security property, so stage 3 enforces it with a conditional
  `UPDATE ... WHERE status='pending'`.

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
  — **AUTHORIZED 2026-10-09**, and **its stage 6 onward is superseded by a
  proposal** —
  [`096-b1-stage-plan-revision-proposal-2026-10-11.md`](096-b1-stage-plan-revision-proposal-2026-10-11.md),
  which is not in effect. Read both: stages 0–5 ran from this document and are
  complete; what remains was under-scoped here by three commands, two
  migrations and four requirements. Nine stages, checked against
  `:23` and `:83-85` **before** stage 1 rather than after stage 9, which is how
  096-A's plan came to be nine stages for fifteen stages of work. **That check
  was not sufficient**: `:89-93`'s F01–F06 assignment is not in `:23`.
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

### Settled

*(This entry was still filed under "Open with the owner" until 2026-10-11,
eight days after it was answered. Index staleness of exactly the kind the
2026-10-07 note above apologises for.)*

- [`exp-boundary-strictness-2026-10-08.md`](exp-boundary-strictness-2026-10-08.md)
  — **SETTLED 2026-10-09; the amendment landed.** RFC 096 `:662` stated
  `exp`'s rule as the strict `now < exp + 60s`, while `:663` and `:664` state
  `iat`'s and `nbf`'s non-strictly, and `jsonwebtoken` accepts at
  `now <= exp + 60` — so the RFC refused one instant the implementation
  accepts. `:662` now reads non-strictly, matching its own two siblings and
  what has always been built; see the RFC's 2026-10-09 amendment summary.
  Enforcing the strict bound in our own code was the rejected alternative: one
  rule in two places, which is the reasoning this RFC's `aud` design had
  already rejected.

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
