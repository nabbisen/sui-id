# Security-Critical Assurance Audit — sui-id v0.63.1

*Deliverable 10.1 of the Security-Critical Assurance Strategy
(architect instruction, v0.63.1). Companion deliverables: RFCs
078–086 (`rfcs/proposed/` → progressively to `rfcs/done/`) and
the adoption roadmap (§9 below).*

*This is a **historical audit record**: it describes the state of
the codebase at v0.63.1. Resolution notes (marked **✅ Fixed**) are
added in-place as each gap is closed. Readers who only need the
current security posture should start at §9 (roadmap) and follow
the resolved-RFC links.*

---

## 1. Scope and method

The audit inspected the v0.63.1 workspace from the viewpoint of
security-critical lifecycle correctness, per strategy §5. Every
target area named in the strategy (§5.1–§5.8) was mapped to the
actual modules, the lifecycle and storage behaviour was read in
full, and the existing test surface was measured. The audit also
validated the strategy's default architectural direction (§12)
against the codebase; the validation result is in §8.

Baseline measured during the audit (this environment; the main
`sui-id` crate test binary is excluded because its ~2 GB link
step exhausts memory here — it remains covered by upstream CI):

| Crate | Tests at v0.63.1 | Tests at latest |
|---|---|---|
| `sui-id-shared` | 12 | 20 (+8 secrets tests, v0.64.0) |
| `sui-id-i18n` | 13 | 13 |
| `sui-id-store` | 36 | 36 |
| `sui-id-core` | 125 | 125 |
| `sui-id-web` | 0 (render-only crate) | 0 |
| **Total (5 crates)** | **186 passing** | **194 passing** |

Two baseline regressions were found and fixed before RFC work
(shipped as v0.63.2):

1. `sui-id-core/src/security.rs` doctest failed to compile
   (`SecurityLevel` not in scope) — `cargo test --doc` was red.
2. `mod.rs` files had crept back in (`sui-id-core/src/admin/mod.rs`,
   `sui-id/src/backup/mod.rs`) during the RFC 075 split,
   violating the spec §8.3 hard policy. Moved to umbrella style.
3. A pre-existing `unused_imports` warning in
   `handlers/admin/clients.rs` violated the 0-warnings gate. Removed.

One environmental finding is flagged but deliberately **not**
fixed in this release: `cargo fmt --check` under current stable
rustfmt (style edition 2024) reports mechanical diffs across most
of the workspace (import-group ordering, line-wrap changes). A
whole-workspace `cargo fmt` sweep should ship as its own
dedicated, review-trivial release — or the project should pin a
toolchain via `rust-toolchain.toml` — before the CI fmt gate is
trusted again. Mixing thousands of style-only lines into a
substantive release would defeat review.

---

## 2. Current security-critical modules

| Concern | Module(s) |
|---|---|
| Authorization code lifecycle | `sui-id-core/src/authorize.rs` (issue/exchange), `sui-id-store/src/repos/auth_codes.rs` (consume) |
| PKCE | `sui-id-core/src/tokens.rs::verify_pkce` (S256 only, `subtle` constant-time) |
| Refresh token lifecycle | `authorize.rs::exchange_refresh`, `repos/refresh_tokens.rs` |
| Token issuance / JWT | `sui-id-core/src/tokens.rs`, `jwt.rs`, `jwt/` |
| Introspection / revocation | `sui-id-core/src/oauth_token.rs` |
| Session lifecycle | `sui-id-core/src/session.rs`, `repos/sessions.rs` |
| Role model & extractors | `sui-id-store/src/models.rs::Role` (Admin/Auditor/User), `CurrentAdmin` / `CurrentAdminOrAuditor` extractors in `sui-id/src/handlers/` |
| Secrets at rest | `sui-id-store/src/crypto.rs` (XChaCha20-Poly1305, per-column AAD, `secrecy`) |
| Audit log | `repos/audit.rs` (SHA-256 hash chain) |
| Typed identifiers | `sui-id-shared/src/ids.rs` (UUID newtypes via macro) |

## 3. Current lifecycle models

**Authorization code.** Stored by SHA-256 hash (plaintext never
persisted). `auth_codes::consume` runs a transaction:
SELECT → Rust-side `consumed || expired` check → `UPDATE … SET
consumed = 1`. Single-use intent is explicit. Exchange order in
`exchange_code`: client lookup → disabled check → client auth →
**consume** → client binding → redirect_uri binding → PKCE →
user-state re-check → issue. Consume-first means a failed
binding check still burns the code (correct, fail-safe).

**Refresh token.** Rotation with family-wide theft response.
`exchange_refresh`: client auth → `find_any` (returns revoked
rows too) → client binding → **revoked ⇒ revoke_family + audit
`auth.refresh.theft_detected`** → expiry → `revoke(old)` →
issue new row carrying `family_id`. `purge_expired` retains
revoked-but-unexpired rows so replay still hits theft detection
(a previously-fixed bug, documented in the repo).

**Session.** `revoked_at` / `expires_at` columns; revoke is
predicate-gated (`AND revoked_at IS NULL`); `touch` deliberately
un-gated (documented benign race). Idle timeout and concurrent
cap enforced in `sui-id-core/src/session.rs`.

## 4. Storage and concurrency model

`Database` wraps **one** `Mutex<Connection>`; all repo calls are
serialized through `with_conn` / `with_tx` on the blocking pool.
Consequences:

- *Within one closure*, sequences are trivially atomic.
- *Across closures*, interleaving is possible: any flow that
  reads in one `with_conn` and writes in another has a window.
- DB constraints present: `auth_codes.code_hash` PRIMARY KEY,
  partial UNIQUE index on `refresh_tokens.token_hash`,
  CHECK constraints on booleans (migration 0021/0022), FKs ON.

## 5. Identified gaps (risk-ranked)

### G1 — Refresh rotation is not atomic and not guarded (High)

`exchange_refresh` performs `find_any` (closure 1), the revoked /
expiry checks in Rust, `revoke(old)` (closure 2), then inserts
the new token (closure 3). Two concurrent requests presenting the
same active token can both pass the checks before either revoke
lands; `revoke` does not report rows-affected, so neither request
notices losing the race. **Result: one parent token can yield two
active children in the same family**, weakening the rotation
invariant "exactly one active token per family chain" and muddying
later theft detection. The fix is a single transaction with a
rows-affected guard (`UPDATE … WHERE id=? AND revoked_at IS NULL`
must affect exactly 1 row; 0 rows ⇒ treat as replay). → RFC 080.

### G2 — Plaintext refresh token is `Debug`-reachable (High)

**✅ Fixed in v0.64.0 (RFC 078).** `RefreshTokenRow.token_plain`
was removed. The plaintext is now held exclusively in
`RawRefreshToken` whose `Debug`/`Display` print `[REDACTED]`.
The single intentional egress is `.expose()`, called at HTTP
response serialization only. 8 unit tests pin the redaction
behaviour.

*At v0.63.1:* `RefreshTokenRow` derived `Debug` and carried
`token_plain: Option<String>`. Any `{:?}` of the row (error
context, future tracing) printed the live token. Spec §6.2
prohibited secrets in logs; the type system did not help
enforce it. → RFC 078.

### G3 — Typed IDs have public constructors; token identifiers are bare strings (Medium)

**✅ Fixed in v0.64.0 (RFC 078).** The `define_id!` macro
inner field is now private (`Uuid` not `pub Uuid`) — struct
literal construction outside the module is a compile error.
`RefreshTokenRow.id` is now `RefreshTokenId`, `.family_id` is
`FamilyId`, `AuthorizationCodeRow.code_hash` is `CodeHash`.
Cross-type assignment (passing a family id where a token id is
expected) is a compile error.

*At v0.63.1:* `ids.rs` newtypes exposed `pub struct X(pub Uuid)`
— any code could fabricate or transmute IDs. Refresh-token `id`
/ `family_id` and `code_hash` were plain `String`, so the
classic mix-up bug class was open. → RFC 078.

### G4 — Auth-code consume correctness leans on the connection model (Medium)

`consume`'s SELECT-then-UPDATE is safe today only because a
single mutex serializes everything. The UPDATE carries no
`AND consumed = 0` predicate and rows-affected is not checked.
If the storage layer ever moves to a pool or WAL-concurrent
readers (proposed RFC 009 explores SQL backends), single-use
silently stops being enforced. Belt-and-braces predicate + guard
costs nothing now. → RFC 079.

### G5 — No pure, centralized authorization decision (Medium)

Authorization is enforced in Axum extractors (`CurrentAdmin`,
`CurrentAdminOrAuditor`) plus scattered `can_write` checks.
There is no single `authorize(actor, action) -> Decision`
function that property tests can pin down (deny-by-default,
monotonicity, last-admin safeguard). → RFC 082.

### G6 — Lifecycle invariants tested by example only (Medium)

proptest exists (redirect-URI comparator, PKCE, crypto, session
bits) but no *state-machine* tests generate operation sequences
(issue → rotate → replay → revoke …) against an oracle model.
The G1 class of bug is exactly what such tests find. → RFC 083.

### G7 — No fuzzing of untrusted-input parsers (Medium-Low)

No `fuzz/` targets. Highest-value surfaces: authorize-endpoint
query parameters, PKCE verifier, JWT compact parsing, JWKS/
config parsing, callback parameters. → RFC 084.

### G8 — Audit coverage is convention, not construction (Medium-Low)

Audit events are emitted by hand at call sites; several are
`let _ =` (fire-and-forget) even where the operation result is
security-relevant. There is no completeness matrix and no
mechanism that couples a privileged mutation to its audit row in
one transaction. → RFC 085.

### G9 — Strategy's "tenant boundary" does not map 1:1 (informational)

sui-id is **single-tenant by design** (multi-tenant expansion is
parked as proposed RFC 025). The strategy's RFC-4 theme is
therefore translated to the boundaries the codebase actually has:
*role scope* (Admin / Auditor / User), *client binding*
(introspection `aud` rule, token↔client checks), and *system vs
self-service scope*. The translation keeps a forward-compatible
seam for RFC 025. → RFC 081.

## 6. Test coverage of the gaps

*Coverage at v0.63.1 (baseline) and current state.*

| Gap | Coverage at v0.63.1 | Coverage now |
|---|---|---|
| G1 | None for concurrency; single-flow rotation covered by example tests | Unchanged — RFC 080 pending |
| G2 | None (formatting behaviour untested) | **✅ 8 unit tests** in `sui-id-shared::secrets` pin `Debug`/`Display` redaction (v0.64.0) |
| G3 | ID parse/display tests only | **✅** compile-fail intent documented; existing ID tests still pass with private inner (v0.64.0) |
| G4 | Single-use covered by example test (sequential) | Unchanged — RFC 079 pending |
| G5 | Extractor-level integration tests in the binary crate | Unchanged — RFC 082 pending |
| G6 | proptest on pure comparators only | Unchanged — RFC 083 pending |
| G7 | None | Unchanged — RFC 084 pending |
| G8 | Presence asserted for a few events, no matrix | Unchanged — RFC 085 pending |

## 7. Category classification (strategy §6)

| Item | Category | Status |
|---|---|---|
| Newtypes / private constructors / secret-redacting types (RFC 078) | **A** | ✅ v0.64.0 |
| Auth-code consume hardening: predicate + rows-affected + property tests (RFC 079) | **A** | ✅ Shipped (v0.65.x) |
| Transactional refresh rotation + reuse guard (RFC 080) | **A** | ✅ Shipped (v0.65.x) |
| Actor-scope boundary & scoped repository signatures (RFC 081) | **B** | ✅ Shipped (v0.66.x) |
| Pure authorization core + property tests (RFC 082) | **B** | ✅ Shipped (v0.66.x) |
| State-machine proptest harness (RFC 083) | **B** | ✅ Shipped (v0.67.x) |
| Fuzzing harness for input boundaries (RFC 084) | **B/C** | ✅ Shipped (v0.67.x) |
| Audit completeness matrix + transactional audit (RFC 085) | **B** | ✅ Shipped (v0.67.x) |
| Kani / TLA+ / Flux pilots (RFC 086) | **C** | ✅ Shipped (v0.68.x) |
| Verus adoption; typestate for UI state; formal verification of repos; policy engine | **D** | Deferred |

## 8. Validation of the strategy's default direction

The default direction (strategy §12) **holds** against the
codebase, with two adjustments:

1. **Tenant boundary → actor/client scope boundary** (G9 above).
2. **Typestate is recommended narrowly**: the auth-code and
   refresh lifecycles are short and DB-mediated; most of their
   safety must come from *storage* atomicity, not in-memory
   states. Typestate is adopted where it genuinely orders
   operations (the validated-exchange pipeline in RFC 079/080),
   and rejected for sessions, whose transitions are
   runtime-data-driven (consistent with strategy §5.3's own
   "runtime validation at repository boundaries" lean).

Everything else — proptest as primary, fuzzing at boundaries, DB
constraints as part of the security model, small pilots only —
is adopted as written.

## 9. Adoption roadmap (deliverable 10.3)

> **Resolution update (v0.76.4):** All nine RFCs (078–086) are shipped and in
> `rfcs/done/`. The table below is the historical planned order; Status column
> updated to reflect actual delivery.

Order respects dependencies: types first (everything else names
them), then the two storage-atomicity RFCs (highest risk), then
the boundary/decision layer, then the test-infrastructure RFCs
that exercise all of the above, then pilots.

| Step | RFC | Target release | Depends on | Status |
|---|---|---|---|---|
| 1 | [078 type modeling baseline](../rfcs/done/078-security-type-modeling-baseline.md) | v0.64.0 | — | ✅ Shipped |
| 2 | [080 refresh rotation atomicity](../rfcs/done/080-refresh-rotation-atomicity.md) | v0.65.0 | 078 | ✅ Shipped |
| 3 | [079 auth-code lifecycle](../rfcs/done/079-authorization-code-lifecycle-assurance.md) | v0.65.0 | 078 | ✅ Shipped |
| 4 | [081 actor scope boundary](../rfcs/done/081-actor-scope-boundary.md) | v0.66.0 | 078 | ✅ Shipped |
| 5 | [082 authorization decision core](../rfcs/done/082-authorization-decision-core.md) | v0.66.0 | 081 | ✅ Shipped |
| 6 | [083 state-machine proptest](../rfcs/done/083-security-state-machine-testing.md) | v0.67.0 | 079, 080 | ✅ Shipped |
| 7 | [085 audit completeness](../rfcs/done/085-audit-event-completeness.md) | v0.67.0 | 081 | ✅ Shipped |
| 8 | [084 fuzzing harness](../rfcs/done/084-fuzzing-untrusted-input-boundaries.md) | v0.68.0 | 078 | ✅ Shipped |
| 9 | [086 formal pilot (time-boxed)](../rfcs/done/086-formal-model-checking-pilot.md) | evaluation only | 080, 082 | ✅ Shipped |

Releases above are *suggested* groupings; each RFC remains
independently shippable. No step gates the verification-phase
soak, and none implies any v1 designation.
