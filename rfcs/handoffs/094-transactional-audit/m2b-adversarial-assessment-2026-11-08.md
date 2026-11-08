# RFC 094 M2b — the architect's adversarial assessment, 2026-11-08

> ## Whose turn it is: **yours** — three decisions, §6.

**Prepared by.** The architect, 2026-11-08, after cycle J's nine stages
landed (`1573a72`).
**What this is.** RFC 094's M2b closure field ends with an acceptance that
rests *"**on the architect's adversarial assessment of it**"*. This is that
assessment. **Adversarial** means each criterion was
tried against the code, not read from the records. Where a record and the
code disagreed, the code wins and the record is named.
**What it is not.** It is not acceptance. That is yours, and **the
architect dispatched every M2b stage**, so it could not be the architect's
in any case.

## Verdict

**M2b is not ready to close.** Two of the four criteria fail as RFC 094
specifies them:

| # | Criterion | Verdict |
|---|---|---|
| 1 | Every remaining Class-A row converted | **Not met.** Three Class-A rows are neither converted nor dispositioned (§1) |
| 2 | No production Class-A best-effort append anywhere | **Met, on the architect's proposed reading**, which is §F1, still yours (§2). Two residuals |
| 3 | AST boundary gate landed, with a negative fixture that rejects an unregistered write | **Met in the letter, not in substance.** A write omitted from both manifest and registry **passes** G23 when it sits in an allow-listed file (§3) |
| 4 | Acceptance on this assessment | Yours, after 1 and 3 |

**Both failures are fixable in one stage**, proposed in §5. Neither one is a
regression: each is a gap that was always there and is now visible.

## 1 — criterion 1: three Class-A rows nobody has decided about

Measured over `contracts/write-commands.toml` at `1573a72`: **68 Class-A
rows**. Of these:
- **58 are sealed**;
- **3 are covered by a sealed successor**: U26 → L08, and C04 and C09 → C03;
- **4 are dispositioned or removed**: U11, C19 and C20 reserved, K02
  removed;
- **and these three carry no disposition at all**:

| Row | Status | What exists |
|---|---|---|
| **C17** provider enable/disable | `target-legacy` | `repos::federation_provider::set_enabled` (`:222`) |
| **C18** provider delete | `target-legacy` | `repos::federation_provider::delete` (`:303`) |
| **C23** provider trust-policy replace | `target-absent` | nothing |

**Measured: neither legacy writer has a production caller.** All four calls of
`set_enabled` are in tests, and `delete` has none, so no unaudited provider
change runs today. But both are `pub`, take `&Database`, and are compiled into
the production store crate, **one new call away from an unaudited Class-A
mutation**. That is the shape K02 was **deleted** for in wave 4 stage 4,
when it too had no caller.

**This is the U26 failure again**, recorded in cycle J §B: *"a measurement
confirming that a row has no command is not a decision that it needs none."*
C19 and C20, the federation **link** rows, were dispositioned on
2026-10-31: *"reserve, against RFC 096-B2"*. The **provider** rows were
not, and they belong to the same RFC.

**The code agrees that they are deferred, and points at the wrong place.**
Two comments defer provider version/generation work *"until M2b lands"* /
*"until M2b adds the real columns"* (`authn/federated_mfa.rs:339`,
`http/handlers/federation.rs:400`). It is RFC 096-B2's work. If M2b closes
with those comments unchanged, they will point at a milestone that has
already closed.

## 2 — criterion 2: met on the proposed reading, with two residuals

Every `audit::append` outside a transaction in production code, re-measured
at `1573a72`. There are nine sites; stage 1's list had a different set, since
`admin/auth.rs:354` is gone (L08) and `events.rs:216` was missing from it.

| Site | Event | Accompanies a Class-A mutation? |
|---|---|---|
| `audit_chain.rs:150` | `audit.chain.verification_failed` | No: a read |
| `authn/session.rs:61` | `auth.login.failure` (refused before any credential check) | No: mutates nothing |
| `authn/session.rs:235` | `auth.login.password_ok_mfa_required` | No: U32 is Class **P**; best effort by design (RFC 102 A6, matrix `:534`) |
| `events.rs:216` | `PasswordResetRequested`/`Throttled`/`EmailSent`/`EmailFailed` (six callers, all `forgot_password.rs`) | No: U34 is Class **P**, whose row says *"request/email events are Class B"* |
| `oidc/authorize.rs:587` | `oauth2.exchange_code.user_revoked` | No: a denial |
| `cli.rs:666` | `admin.master_key.rotated` | No: a file rename. The DB half is K07, sealed |
| `handlers/federation.rs:1916` (`emit_audit_soon`, caller `:498`) | sign-in upstream failure | No: a configuration failure |
| `handlers/oauth_token.rs:91` | `token.introspect` | No: a read |
| `handlers/oauth_token.rs:144` | `token.revoke` | **A revocation.** Class B **by design**: RFC 094 invariant 5 and the matrix's own Class B definition name *"revocation"* as what logging must never suppress |

**On §F1's reading** (*"no best-effort append accompanying a Class-A
mutation"*), **criterion 2 is met.** On a literal reading of every append
as a violation, it fails on all nine, including those that write nothing.
That reading is why §F1 is decisive, and why it is still yours.

**Residual R1: seven of the nine sites fail Class B's own definition.** The
matrix (`:32`) defines B as *"a failure is **logged loudly**"*. Every site
above except `events.rs:216` (which logs) and `cli.rs:666` (which reports on
stderr since `0.87.1`) is `let _ = audit::append(...)`, which **discards** the
failure. RFC 094 already has the right tool, `emit_must_attempt`
(`:292-298`). Cycle J carried this as a by-product and it is still open.

**Residual R2: T07's rationale hides a caller.** T07 (access-token deny-list
insert) and T06 (refresh revoke) are Class I, *"subordinate to
U20/U19/C06 or another inventoried revoke command"*. RFC 7009's
`/oauth2/revoke` calls both directly through `oauth_token::revoke`, and **no
row owns that path.** This is K06's lesson exactly: *"a wrong rationale does
not merely mislead, it suppresses the search."* It needs a row, Class B by
design.

## 3 — criterion 3: the fixture is rejected, and so would be little else

**The negative fixture passes**: G23 rejects
`scripts/tests/fixtures/write-authority/unregistered-write`. **And the
module boundary works.** Probe B below fails by name. Both were re-run
today.

**But RFC 094 asks for more than the boundary**, three times over:
- the structural-gate section: *"It fails if **a source write site lacks a
  generated command declaration**"*;
- M2b's own list: *"The AST negative fixture proves **a write omitted from
  both manifest and registry** is rejected"*;
- invariant 6: *"A new unclassified mutation cannot obtain a write
  transaction **or pass CI**."*

**Probes**, run on the real tree with a green baseline first, each reverted
and hash-checked:

| Probe | Code added | G23 |
|---|---|---|
| A | `pub fn probe_unregistered_write(conn) { conn.execute("UPDATE users SET is_admin = 1", []) }` in `repos/users.rs`, an **allow-listed** file, with **no row and no descriptor** | **exit 0: passes** |
| B | the same shape, `DELETE FROM audit_log`, in `metrics.rs`, a **non**-allow-listed store file | exit 1: *"raw type `rusqlite::Connection` outside the allow-listed module set"* |

**So inside the 29 allow-listed repo files, a write omitted from both manifest
and registry is not rejected**, and an `async fn(db: &Database)` repo writer
is callable from `sui-id-core` without either. A rough count (a regex, not
authoritative): **148** writing functions in those files, **35** named
nowhere in the manifest by `module::function`. Most are by shape benign
(`*_within_tx` primitives that only sealed commands call, and `purge_expired`,
which O03 covers in prose). But one confirmed case is already there:
**`user_consent::revoke` (`repos/user_consent.rs:215`), a `pub async`
`&Database` writer that deletes consent rows, with zero callers anywhere,
named in no command.** It joins `federation_provider::set_enabled` and
`delete` (§1) as dead-but-public bare writers the gate cannot see.

**How this happened: the architect's dispatch, not the build.** Cycle J
stage 6's dispatch §3 defined the checker as RFC 094's *condition 7*
(reject forbidden calls *"outside the allowed modules"*) on the stated
premise that the allowed set would be *"the private executor, the migration
runner, and test fixtures"*. Measurement correctly widened it to 36 files,
every repo included, with reasons recorded. **At three modules the boundary
alone would have been close to enough; at 36 files the other half, a write
site with no declaration, became the load-bearing part, and no stage was
ever given it.** The dev team built exactly what was dispatched.

## 4 — the other M2b clauses in RFC 094's own list

- **"The structural gate is blocking, and it, the generated/reference
  documentation, and the registry agree in full."** G23 is a blocking lane
  and agrees with the registry for every **declared** command. §3 is why
  "in full" cannot be claimed yet.
- **"Audit-chain and atomicity claims describe the implemented boundary
  exactly, and the partial-conversion statement is removed."**
  `docs/src/reference/audit-events.md` describes what a verified chain
  proves (cycle J stage 1). No matrix **cell** still reads `B (A
  required)`. Narrative paragraphs at `contracts/audit-coverage-matrix.md:84`
  and `:93` say *"the remaining eight are still…"* in the present tense about
  rows since converted. They are history, and should say so. Removing the
  partial-conversion statement honestly also waits on §1.
- **"…accepts adversarial, rollback, concurrency, and clean-matrix
  evidence."** Rollback: G20 declares every sealed row. Clean matrix: Level
  B on `1573a72`, `workflow_dispatch` run `42435137349`, **28 jobs, 28 success, 0 failed, 0 skipped**.
  **Concurrency was not re-assessed here.** The evidence for it is whatever
  race tests the individual stages carry, and this assessment did not
  inventory them. That inventory belongs in the evidence you accept, and
  stage 10's return should list it (§5). This assessment does not claim
  more.

## 5 — the proposal: one more stage before M2b closes

**Cycle J stage 10 — the write site, not only the module.** One round:

1. **A function-level check in `xtask audit-structure`**: inside an
   allow-listed file, every function that contains a write call must be
   named, as `module::function`, in some row's `mutation_surface`. Rows
   already name their functions in backticks, so it reads what exists. **A
   second negative fixture** sits **inside** an allow-listed path: probe A,
   kept.
2. **Run it, and let it find what it finds.** Every function it flags gets a
   decision in the same round, never a silent allow-list entry: name it in
   the row that owns it, give it a row, or **delete it** if it has no caller.
   That covers `user_consent::revoke`, and C17/C18's `set_enabled` and
   `delete`, which become reserve rows like C19/C20, with the code gone or
   behind `#[cfg(test)]`.
3. **Disposition C17, C18 and C23** against RFC 096-B2, in C19's words, and
   **re-point the two "until M2b" comments** to RFC 096-B2.
4. **R2:** a row for RFC 7009 revocation, Class B by design, and T06/T07's
   rationales corrected.
5. **The concurrency evidence, inventoried**: every race and replay test
   behind a sealed Class-A command, by `file:line`, and the sealed commands
   that have none. Measurement only, so that §4's gap is closed by a list
   rather than by an assertion.
6. **R1** (`let _` → `emit_must_attempt` at the seven sites) is the same
   size again and touches nothing M2b requires. **Recommended as its own
   round after `0.88.0`, not inside this one.**

**The alternative**, amending RFC 094 to describe the module boundary that
was delivered, would weaken invariant 6. That is a material change to an
Accepted RFC's security invariant, which sends it back to `proposed/` (RFC
000). **Not recommended.**

**Effect on the schedule:** `0.88.0` (*"M2b closes, and RFC 142 closes"*)
waits for stage 10. RFC 142's closure does not depend on it.

## 6 — what is yours

1. **Stage 10 before M2b closes** (§5): approve it, or choose the
   amendment route instead. **Recommended: approve.**
2. **§F1, criterion 2's reading.** **Recommended:** *"no best-effort append
   accompanying a Class-A mutation"*. It is the reading under which
   criterion 2 means something and is met (§2).
3. **§F3, `rusqlite` under `[dev-dependencies]`.** RFC 094's text forbids it
   outside `sui-id-store`, and M2a closed on the opposite reading
   (*production reach only*). **Recommended:** settle on M2a's reading and
   amend that one sentence to say `[dependencies]`. The strict reading costs
   nine e2e rewrites and protects no production path. **This amendment is
   yours to judge material or not**; the architect does not decide that for
   an Accepted RFC.

## 7 — after stage 10: the criteria re-assessed, 2026-11-08

> ## M2b's evidence accepted. `@nabbisen`, 2026-11-08: *"Accepted."*
> With M2a closed on 2026-10-03, this completes RFC 094's per-stage closure.
> RFC 094 moves to `done/` as `Implemented (v0.88.0)` in the `0.88.0`
> preparation, with this document and the M2a closure review as its
> `Closure evidence`.

Stage 10 and its fix round landed in `cbfb8f9`. Level B on that exact
commit: `workflow_dispatch` run `42462622748`, **28 jobs, 28 success, 0 failed,
0 skipped**. `Security audit` on the same commit: run `42462625022`, **success**. Each criterion was **re-tried against the code**, as
§1–§3 were. None was read off the stage's own report.

| # | Criterion | Now |
|---|---|---|
| 1 | Every remaining Class-A row converted | **Met.** 68 Class-A rows: 58 sealed; 3 covered by a sealed successor (U26 → L08, C04 and C09 → C03); 7 dispositioned or removed (U11, C19, C20, **C17, C18, C23**, all reserved against RFC 101 or RFC 096-B2; K02 removed). The legacy writers `federation_provider::set_enabled` and `delete` are test-gated out of production |
| 2 | No production Class-A best-effort append (reading **(b′)**, settled) | **Met.** No best-effort append accompanies a Class-A mutation. **R1**: all seven discarding sites now report their own failure, five through `emit_must_attempt` and two through `tracing::error!` beside a name L02's sealed descriptor owns, with fault-injected tests proving the revocation or denial still holds. **R2**: RFC 7009 revocation is row **T05**, and T06/T07 name it. **Operator reference**: `docs/src/reference/audit-events.md`, *"Events that can be missing from the log"* |
| 3 | AST boundary gate with a negative fixture | **Met, in substance.** G23 now checks the **write site**: every writing function in an allow-listed file must be named in a row. Re-probed on the real tree, all five shapes now **fail by name**: §3's probe A; a prefix of a registered name; a write through `query_row … RETURNING`; a `#[cfg(not(test))]` writer; and a new `DELETE FROM audit_log` beside the exempted `audit::append`. Each is kept as a fixture. **Stated limitation:** SQL bound to a variable or built at runtime before reaching a query method is not inspected |
| §F3 | `rusqlite` boundary (production reach, settled) | **Enforced by G23**, which now reads every `Cargo.toml` with fixtures; the e2e witness is retired. RFC 094's two sentences are amended in the same commit |

**What the re-assessment found, and the review fixed in the same round:**
- **The new check could first be passed four ways**: a substring name
  match, `query_row` writes, a `cfg` test that skipped `not(test)`, and
  whole-file exemptions.
- **Fixed, it flagged 20 functions**, 18 of them bare writers with no
  production caller (`users::set_role` and `users::set_disabled` among them).
  All were resolved: 3 deleted, 15 test-gated, 2 named.
- **A production defect, older than M2b.** `crates/sui-id/Cargo.toml` had
  enabled `sui-id-core/test-support` under `[dependencies]` since
  2026-07-02, so every release build compiled an in-memory HIBP mock that no
  production path constructed. The dev-dependency meant to carry the feature
  carried none. Moved, and G23 now refuses a `test-support` feature on any
  production dependency table.
- **The dev team's own finding:** expired password-reset tokens were never
  cleaned up. They now are, with a test.

### The concurrency evidence (§4's gap), inventoried and verified

Every cited test was confirmed to exist at the stated location.

| Command | Evidence |
|---|---|
| U05 admin role change | `crates/sui-id-store/src/commands/tests/runner/user_admin.rs:438` `u05_concurrent_demotions_of_the_last_two_admins_leave_exactly_one` |
| U10 forgot-password completion | `crates/sui-id/tests/e2e/r103_stage1.rs:388` `r103_two_concurrent_completions_change_the_password_once`; `crates/sui-id/tests/e2e/r118.rs:300` (sequential replay) |
| L02 second-factor sign-in | `crates/sui-id/tests/e2e/r102_stage3.rs:954`, `:977`, `:1000`, `:1029` (four races) |
| L05 step-up success | `crates/sui-id/tests/e2e/r102_stage6.rs:408` |
| S10 first setup | `crates/sui-id-store/src/commands/tests/runner/setup.rs:206` |
| T04 refresh rotation / reuse | `crates/sui-id-store/src/repos/refresh_tokens/tests.rs:149` (8 concurrent rotations); `crates/sui-id/tests/e2e/refresh_theft.rs:19` (replay) |
| F04, structurally | its gate, `federation_login_attempt::claim`: `crates/sui-id-store/src/repos/federation_login_attempt/tests.rs:481` |
| All Class-A writes | `crates/sui-id-store/src/commands/tests/runner/chain_integrity.rs:4`: the chain stays unbroken under concurrent Class-A commands |

**51 sealed commands have no race test of their own**, and **that is not a
gap**. The store holds **one connection behind a `Mutex`**, locked for the
whole transaction (`crates/sui-id-store/src/backend.rs:81-91`, `:131-132`,
`:161-162`), so every transaction within one process is serialized and
cannot interleave. Across processes, a deferred WAL transaction that writes
after another process committed fails with `SQLITE_BUSY`: an error, never a
silent lost update. The stage's first report had flagged U22 as a
lost-update risk; **measured, it is not one**, and the report was corrected.
The tests above exist where the property is more than serialization:
exactly one winner, a consumed token, a code used once.

**What remains outside M2b, deliberately:** commands reserved against RFC 096-B2
and RFC 101 wait for those RFCs. SQL built at runtime is outside G23's reach
and stays a reviewer's to catch.

**Recommendation: accept M2b's evidence.** On your word, RFC 094 records M2b
closed in the `0.88.0` preparation, alongside RFC 142's move to `done/`.
