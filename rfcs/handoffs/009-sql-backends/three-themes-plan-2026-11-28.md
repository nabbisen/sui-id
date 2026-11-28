# Tenancy, embedding and storage: one plan

> ## Whose turn it is: **the architect's.** Phase A (§4) starts once Q5 has landed: A1 first, with A4 beside it.
> Phase A needs no freeze exception. Nothing here is built until each stage's RFC is Accepted and its exception is granted.
> §7 checks each item and the series against the fundamental philosophy, and lists the changes that check made.

**Proposed.** 2026-11-28, by the architect, at `@nabbisen`'s request (*"Helpful. Proceed carefully."*).
**Authorized.** 2026-11-28, by `@nabbisen`: *"The plan is authorized."* With the instruction to verify that each item and the
series meet his fundamental philosophy. The authorization was given to §§0–6 as they stood. §7's changes came after it, and
are listed there so that each can be objected to.
**Measured at** `ba7c4a6`, and against crates.io on 2026-11-28.

## §0: what was decided on 2026-11-28

`@nabbisen`, on the architect's three questions:
1. **Multi-tenancy**: *"Yes. Multi-tenancy is required."*
2. **RFC 009** (other databases), the review and stage 1's freeze exception for the cycle after Q: *"Accepted."*
3. **The library**: *"Not necessarily required. SeaORM was just an example of mine. Besides, SeaORM was preferred to Diesel
   by me - although I do not know the latest status on Diesel."*

## §1: where each theme stands

| Theme | RFC | Status | What exists |
|---|---|---|---|
| **Other databases** | [RFC 009](../../proposed/009-sql-backends.md) | Accepted on 2026-11-28 (decision 2). Its move to `accepted/` follows with this plan's records | Refreshed 2026-11-25: eleven guarantees each backend must re-prove; a breadth model (Supported or refused by name); five stages |
| **Embedding** | [RFC 119](../../accepted/119-embedding-contract.md) | Accepted 2026-11-01 | The host's obligations (H1–H8). [How a host sets up an embedded sui-id](../119-embedding-contract/host-setup-design-2026-11-26.md), designed 2026-11-26. The store-boundary RFC and the embedding RFC are unwritten. Contract V2 is gated |
| **Multi-tenancy** | [RFC 025](../../proposed/025-multi-tenant-expansion.md) | Proposed, *"longer-term, no scheduled delivery"*, now **required** (decision 1) | A detailed design written at v0.29.5. Two open questions say what it lacks: the operator-facing half (6), and per-tenant master keys (7) |

**All three were frozen** by the ROADMAP's remediation programme (*"New feature expansion, multi-tenancy (RFC 025), and
alternative SQL backends (RFC 009) remain frozen until every milestone in this programme has passed its exit gate"*).
Decision 2 grants RFC 009 stage 1 an exception. Nothing else is excepted yet.

## §2: why the three must be designed together

Each theme changes the same three things. Designed apart, the later ones would rework what the earlier ones proved.

1. **Every table.**
   - RFC 009 stage 1 renames the tables to a fixed `sui_id_` prefix (the host setup design, §2).
   - Tenancy gives the tenant-bearing tables a `tenant_id` and makes their unique keys per tenant (RFC 025 §2, §3).
   - ~~Two passes over every table means two migrations of every deployment, and two chances to drop a constraint.~~
     **Corrected in §7 (C1), by measurement**: a rename is not a reshape. SQLite keeps every constraint through it. The one
     pass that can drop a constraint is tenancy's reshape, and §7 gives it a proof.
2. **Every store call.**
   - RFC 009 stage 1 rewrites every command and repository behind an interface no driver type crosses. That is the
     largest stage, and it touches every store call site.
   - Tenancy then needs every one of those calls to be **scoped to a tenant**.
   - **If stage 1's interface has no tenant scope, tenancy rewrites every call site a second time.** If it has one from the
     start, carrying a single tenant until tenancy is built, tenancy changes what the scope holds, not where it goes.
3. **Every request.**
   - The host setup design carries the host's obligations (H1–H4, H6) as a typed per-request context (§7).
   - Tenancy adds a tenant to every request.
   - **One request context should carry both**, designed once.

**And the database itself.** Isolation between tenants can be enforced by the application (a `tenant_id` checked on every
query), or partly by the database (composite foreign keys that make a cross-tenant reference impossible to store; PostgreSQL
row-level security). RFC 009's rule is that a guarantee holds **by construction**, not by care. That favours the database
doing it wherever every Supported engine can. RFC 025 predates the rule, and its choice (tenancy inherited through foreign
keys, scoped by joins) has to be re-measured against it.

## §3: the library (decision 3)

**Measured on crates.io, 2026-11-28:**

| Crate | Latest | Updated | What it is |
|---|---|---|---|
| `rusqlite` | 0.43.0 | 2026-11-15 | **What sui-id uses today**: 0.41.1, with bundled `libsqlite3-sys` 0.39.1 |
| `sqlx` | 0.9.1 | 2026-11-18 | Async driver for SQLite, PostgreSQL and MySQL. Its SQLite driver accepts `libsqlite3-sys` `>=0.30.1, <0.41` |
| `sea-orm` | 2.0.4 | 2026-09-27 | An ORM **built on sqlx 0.9 and SeaQuery 1.0**, so it inherits sqlx's driver properties and adds a model layer |
| `diesel` | 2.4.0 | 2026-11-13 | **Actively maintained**: a synchronous ORM and query builder. Its SQLite backend accepts `libsqlite3-sys` `>=0.17.2, <0.41` |
| `diesel-async` | 0.10.0 | 2026-11-13 | Diesel's async companion, for PostgreSQL and MySQL |
| `tokio-postgres` | 0.7.18 | 2026-06-12 | A native PostgreSQL driver |

**What this means:**
- **RFC 009 §4 decides the driver by a prototype, against stated criteria.** It does not decide by preference. The
  criteria, in order:
  1. the eleven guarantees hold by construction;
  2. a SQLite security fix reaches a release without waiting for an intermediate crate;
  3. no live database at build time;
  4. dependency weight and supply chain;
  5. an embedder's version coupling.
- **SeaORM and Diesel join the prototype's candidates**, beside sqlx, a hybrid (rusqlite for SQLite), and native drivers.
  **Your preference for SeaORM over Diesel is recorded as the tie-breaker** between two ORMs that measure alike. It does not
  outrank the criteria. You said an ORM is not required, so "no ORM" stays a candidate on equal terms.
- **Criterion 2 is not settled by choosing a newer crate.** Every candidate pins the bundled SQLite within a range. A new
  major version of `libsqlite3-sys` waits for that crate's own release, whichever we choose. sqlx and Diesel both stop below
  0.41. The prototype measures each candidate's record of how fast it followed SQLite security fixes.
- **The architect's expectation, to be measured, not assumed**: an ORM helps less here than usual. Our guarantees rest on
  exact SQL: row and advisory locks, read-only transactions, enforced `CHECK`s, and commit-failure injection. An ORM reaches
  those through raw SQL. It would add weight, and still need hand-written SQL in exactly the places that matter.

## §4: the sequence proposed

### Phase A: design, alongside cycles Q and R (no freeze exception needed)

| # | Work | Why first |
|---|---|---|
| **A1** | **RFC 025's refresh**, against the code as built. It covers: the tenant scope at the store interface; isolation by construction (§2); open question 6 (what an operator reads: which tenant am I acting in, global versus tenant authority, a single-tenant deployment that shows no tenancy at all); open question 7 (per-tenant master keys, against RFC 100's single-key custody); the issuer-URL transition for existing deployments; rate limits and lockout per tenant; and its relation to embedding | Stage 1's interface depends on its answer to "where does the tenant scope live" |
| **A2** | **RFC 009 stage 1, re-specified with A1's scope**: the typed operations carry a tenant scope from day one, holding a single tenant until tenancy is built | So tenancy does not revisit every call site |
| **A3** | **The store-boundary RFC** (embedding), written beside A2. Its request context carries the host's obligations and the tenant together | One request context, designed once |
| **A4** | **The driver prototype** (RFC 009 stage 2), as measurement: §3's candidates against §3's criteria, recorded as RFC 009's amendment. **Moved here from beside S** (§7, C4) | The driver decides whether a transaction body is synchronous or asynchronous, and so the shape of the interface S freezes |

Each refresh is put up for acceptance, as RFC 152 was. A1 meets §7's requirements R1–R5, and A2 meets C1–C3.

### Phase B: building, one cycle each, after cycle R

| Cycle | Build | Needs |
|---|---|---|
| **S** | **RFC 009 stage 1**: the database-neutral store, the `sui_id_` prefix, the liveness guard in the store's own open, and the tenant scope, holding one tenant. Nothing changes for users or relying parties; the rename is visible only to an operator's own SQL, and the upgrade note says so (C3) | the exception granted by decision 2; A1, A2 and A4 accepted |
| **T** | **Tenancy's core** (RFC 025): the tenants, per-tenant keys and settings, routing, the two administrator authorities, per-tenant audit chains, and the upgrade of an existing deployment into one default tenant. **Its reshape migration carries a generated before-and-after constraint proof, and isolation is proved per operation with two tenants** (C1, C2) | RFC 025 accepted; **its own freeze exception** |
| **U** | **PostgreSQL** (RFC 009 stage 3). Its schema is designed tenant-aware from its first migration, so no deployment is ever migrated from untenanted PostgreSQL. **Not selectable by any operator until V is in the same release** (C5) | stages 1–2; T |
| **V** | **The operator's side** (RFC 009 stage 4): `[storage]`, backup and restore, the guide. **PostgreSQL becomes Supported in the release that contains V** | U |
| **later** | the embeddable module; MariaDB/MySQL (RFC 009 stage 5); embedding contract V2 | each its own RFC and exception |

**Why tenancy before PostgreSQL.** A SaaS deployment with many tenants is the case most likely to want PostgreSQL. If
PostgreSQL ships first, every PostgreSQL deployment must later be migrated into tenancy, on a second engine. If tenancy comes
first, PostgreSQL's first schema already has it.

**Why stage 1 moves from "the cycle after Q" to after R.** Decision 2 granted the exception for the cycle after Q. Two things
measured since then make that cycle the wrong one:
- **Cycle R is already the cycle after Q** (authorized 2026-11-26: RFC 150 and RFC 101), and both are store-heavy, with new
  tables, migrations and commands. RFC 009 §6 itself says stage 1 *"must not run beside other store-heavy work"*.
- **Decision 1 makes A1 a prerequisite.** Stage 1 cannot freeze its interface before tenancy's scope is designed.

So stage 1 becomes cycle S, directly after R. The exception is unchanged; only its cycle moves. This is §6's first decision.

## §5: what this does not change

- **Cycles Q and R** (`0.95.0` and `0.96.0` for Q, `0.97.0` for R, since 2026-11-28), as authorized.
- **The remediation programme** (M6, M7). **Every Phase B cycle delays M6 and M7 by its own length.** That is a trade on the
  path to production readiness, and the freeze exists to make it visible. It is named here so that each exception is
  granted knowingly.
- **Contract V2's entry conditions** (the host setup design, §14).

## §6: decisions (authorized 2026-11-28)

Authorized with the plan. Decision 3 was authorized as it is written: tenancy's exception is named, and **not yet granted**.

1. **Stage 1's cycle**: S, directly after R, instead of "the cycle after Q" (§4's last paragraph). The exception of decision
   2 is unchanged.
2. **This plan's order**: Phase A now; then stage 1; then tenancy before PostgreSQL.
3. **Tenancy's own freeze exception**, for cycle T. It can wait until RFC 025's refresh is accepted. Naming it now only
   says the plan depends on it.
4. **The library**: SeaORM and Diesel join the prototype's candidates; "no ORM" stays one; your SeaORM preference is the
   tie-breaker between ORMs (§3).

**Questions the refreshes will put to you, not needed now:**
- **Multi-instance** (RFC 009 §7.1): a SaaS deployment with many tenants makes more than one sui-id process on one
  PostgreSQL more likely wanted.
- **Per-tenant master keys** (RFC 025, question 7).
- ~~What an operator of a single-tenant deployment sees of tenancy: ideally nothing.~~ Now a requirement on A1, not a
  question: §7, R2.

## §7: each item checked against the fundamental philosophy (2026-11-28)

The two halves: (1) *"finally clean, safe and secure, and robust and sophisticated design"*; (2) *"APIs and UI/UX for users
not to be confused or misunderstand"*. Each item was checked against both, and the plan's own premises were measured rather
than trusted. **Three premises failed measurement or were missing (C1, C4, R3), and two items would have let a user
misunderstand (R1, C5).** Each is corrected above, and listed here.

R1–R5 and C1–C5 are **the architect's requirements**, proposed for the refreshes. They are proposals, not rulings. Each is
put to him with its RFC's acceptance.

### A1, RFC 025's refresh: meets both halves only with five requirements

- **R1. Upgrading into tenancy changes nothing a relying party sees** (half 2). The default tenant keeps its issuer byte for
  byte, and its keys, `sub` values and endpoints. **RFC 025 §8 recommends the opposite**: a both-issuer transition, *"accept
  old issuers on verify endpoints for one release"*. That makes every relying party of every existing deployment change its
  configuration within one release, and the ones that miss it fail. Upgrade should never ask that.
- **R2. A single-tenant deployment shows nothing of tenancy** (half 2). No slug in its URLs, and no tenant in its console,
  CLI, configuration or guide, until a second tenant is created. Creating one changes nothing for the first. RFC 025's open
  question 4 says *"should feel as light"*; this makes it a requirement the refresh's tests hold.
- **R3. A conflict measured in the code, which A1 must resolve.** R1 keeps the default tenant at `/`.
  - The session cookie `sui_id_session` is set with `Path=/` (`crates/sui-id/src/http/handlers.rs:88`).
  - RFC 025 P3 scopes each tenant's cookie by path (`Path=/{slug}/`), with a name that does not encode the tenant.
  - A `Path=/` cookie is sent with every `/{slug}/` request too. So a browser signed in to both sends two cookies with the
    same name. The readers pick one by its position in the header: the session extractor through a cookie jar
    (`crates/sui-id/src/http/handlers.rs:164`), and sign-out by a loop over the header
    (`crates/sui-id/src/http/handlers/oidc.rs:733`).
  - That leaves a request's session, and so its tenant, to the order the browser sends cookies in. It fails half 1, and the resulting sign-outs fail half 2.
  - The candidates are the tenant in the cookie's name, or tenants by host instead of by path. **Not decided here.**
- **R4. Each kind of isolation is stated as it is delivered** (both halves): data, signing keys, audit chain, rate limits, and
  data at rest. While the master key is shared (open question 7), no page, guide or message may call the tenants isolated at
  rest.
- **R5. Isolation by construction where every Supported engine can** (half 1). Composite foreign keys need a `tenant_id` on
  the child tables, against RFC 025 §2's rule that child tables inherit the tenant through a foreign key. That trade-off is
  measured in the refresh, not assumed.

### A2 and S, stage 1: meet both halves with three corrections

- **C1. §2's premise was wrong, measured.** I applied all 55 migrations to SQLite 3.54.1, the version `libsqlite3-sys`
  0.39.1 bundles. Then I renamed every table to `sui_id_` in one transaction, and compared each table's
  `table_info`, `foreign_key_list`, `index_list` and `CHECK` clauses before and after.
  - The columns, foreign keys (each retargeted to the new name), index counts and `CHECK`s are identical, table by table.
  - `foreign_key_check` is empty and `integrity_check` is `ok`. No permanent trigger exists: 0053's is temporary.
  - **So the rename cannot drop a constraint. Tenancy's reshape can.**
    - `users.username` and `federation_provider.slug` are inline `UNIQUE`s. Making them per-tenant rebuilds the table, and
      `users` is the table most often referenced.
    - So **T's migration carries a proof**: the constraint set before and after, generated from the schema itself and
      compared, with every difference named as intended.
    - Its mutation: a rebuild that drops one `CHECK` must fail the proof.
  - Also measured: the 31 tables include `sui_meta`, which is already namespaced. **A2 states whether it becomes
    `sui_id_meta`.**
- **C2. A scope that always holds one tenant cannot show it is honoured** (half 1: a claim nothing can test).
  - **S claims only that the scope is present**: no store operation can be called without one, by type.
  - **T claims isolation**, and proves it per operation with two tenants. The list of operations it proves is generated
    from the same definition as the operations, so a new operation cannot miss the test.
  - Its mutation: one operation drops its tenant predicate, and the test must fail.
- **C3. The rename is visible to an operator** (half 2). Nothing changes for users or relying parties. But an operator's
  own SQL, monitoring queries and scripts name the old tables, so the upgrade note names every table's new name.

### A3, the store-boundary RFC: meets both halves

One addition: the request context carries the tenant in a single-tenant deployment too, by type. Then the embedded path and
the standalone path are one path, and R2 holds for an embedding host as well.

### A4, the driver prototype: moved before S (C4)

- **The plan put it beside S, and that was wrong.** S freezes the store's interface. The driver decides whether a
  transaction body is synchronous or asynchronous.
  - RFC 009 §2 row 1, the audit row committing with its effect, holds today *"by construction"*. It holds because the
    closure is synchronous and cannot `await`.
  - An asynchronous driver changes that shape.
  - Built before the measurement, S could freeze a shape the chosen driver breaks, and every call site would be rewritten
    again. Avoiding that second rewrite is this plan's reason to exist.
- **Measurement needs no exception**, so A4 runs in Phase A, beside A1.
- This reorders RFC 009 §5's stages 1 and 2. The reorder is put up with A2, as RFC 009's amendment.

### T, tenancy's core: meets both halves with R1–R5, C1 and C2

Its freeze exception is named and not granted (§6, decision 3).

### U and V, PostgreSQL and the operator's side: one release (C5)

- **As planned, a release could have contained U without V.**
  - That release would have PostgreSQL, but no `[storage]` guide and no backup that keeps the key file. RFC 009 row 10 says
    *"the database dump is unreadable without it"*.
  - An operator could run it and believe they were backed up. That fails both halves.
- **So PostgreSQL is refused by name until the release that contains V**, and becomes Supported in that release. That is
  RFC 009 §4a's binary support model, applied to the schedule.
- **U's instance lease** refuses a second instance with a message that names the reason and what to do instead.

### The series

- **The order holds against both halves**, after C4: Phase A (A1 with A4, then A2 and A3), then S, then T, then U and V in
  one release.
  - Tenancy before PostgreSQL still means no deployment migrates into tenancy on a second engine.
  - Only two moments in the series reach a user: T's upgrade and the PostgreSQL release. They are covered by R1 and R2, and
    by C5.
- **Each guarantee is claimed by the stage that can prove it**: presence in S, isolation in T, support in V. No stage claims
  more than its tests show.
- **Stated plainly, so it is not misread: this plan builds no embeddable release.**
  - It builds what embedding needs: S's boundary and A3's design.
  - The embeddable module stays under *later*, after V.
  - Bringing it forward is a scheduling decision still to be made, and nothing in the plan forecloses it.
- **The freeze trade is unchanged** (§5): every Phase B cycle delays M6 and M7 by its own length.

### What the changes ask of `@nabbisen`

Nothing now.
- R1–R5 come to him with RFC 025's refresh.
- C1–C4 come to him with A2, and with RFC 009's amendment for A4.
- C5 comes to him with U.
- If any of them should not stand, it can be refused then, or now.
