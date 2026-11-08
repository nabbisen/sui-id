# Cycle J stage 10 — the write site, not only the module

> ## Whose turn it is: **the implementation role's.**
> The last stage before M2b can close. §D of
> [`cycle-j-and-0.88.0-2026-11-01.md`](cycle-j-and-0.88.0-2026-11-01.md).

**Dispatched.** 2026-11-08, by the architect.
**Authorized.** `@nabbisen`, 2026-11-08: *"Yes. Stage 10."*, and, for the
two rulings that shape it, *"Both your directions are accepted."*
**Why it exists.** [`m2b-adversarial-assessment-2026-11-08.md`](m2b-adversarial-assessment-2026-11-08.md)
§1 and §3, and [`m2b-criterion-2-and-f3-verified-2026-11-08.md`](m2b-criterion-2-and-f3-verified-2026-11-08.md).
**Read those first.** This dispatch does not restate their measurements.
**Governing RFC.** RFC 094, **Accepted**. Building is permitted.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 0 — the shape, and when to stop and report

Six parts. §1 is the gate, and it decides how large §2 is. **Build §1 first,
run it on the real tree, and report its flagged list before resolving it**
if the list holds **more than about twenty functions** that need a decision
(not just a name added to a row that already owns them). In that case the
stage splits there, as 10a and 10b. Below that, carry on in one round.

## 1 — the gate checks the write site, not only the module

**What RFC 094 requires and G23 does not do yet** (the assessment's probe
A): *"It fails if **a source write site lacks a generated command
declaration**"*; *"The AST negative fixture proves **a write omitted from
both manifest and registry** is rejected"*; invariant 6: *"A new
unclassified mutation cannot … **pass CI**."*

**Add to `xtask audit-structure`:** inside every allow-listed file
(`contracts/write-authority.toml`), every function, **including `impl`
methods and including the closures in its body**, that contains a **write
call** must be named as `module::function`, in backticks, in some row's
`mutation_surface` in `contracts/write-commands.toml`.
- **A call-level definition of "write call" is new.** `write_authority.rs`
  forbids three *types* (`FORBIDDEN_TYPES`, `:25`) and two *identifiers*
  (`with_conn`, `with_tx`, `FORBIDDEN_IDENTS`, `:26`), which is exactly why it
  works at module granularity and not below. Start from the call kinds the
  stage 6 dispatch's §3 listed (statement execution, `execute` /
  `execute_batch`, transactions, writable `PRAGMA`, `ATTACH`/`DETACH`,
  backup/restore), and exclude the read-only ones (`query_row`,
  `query_map`, a `prepare` used only for reads). **Write the definition
  into the module's doc, with the reasoning**, and report it in the return. A definition too narrow
  reports success on a tree it never inspected; one too broad buries the
  real findings in reads.
- **`module`** is the file stem, as rows already write it
  (`users::set_disabled_within_tx`). If a row writes `repos::users::…`,
  accept both forms, and say which the tree uses.
- `#[cfg(test)]` items and test modules are out of scope, as for the
  boundary check.
- **The migration runner and the backup module** are allow-listed and write
  by design. Each gets its own named exemption **in the contract file**, with
  a reason, never inline in the checker.

**A second negative fixture, inside an allow-listed path**: the
assessment's probe A, kept. It is a fixture tree whose `write-authority.toml`
allow-lists `src/repos/users.rs`, which contains a writing function named in
no row. **The gate must fail by name.** The same tree also holds a *named*
writing function, which must pass. Use the fixture style
`xtask/tests/*_fixtures.rs` already uses.

## 2 — resolve every function §1 flags, one decision each

For each flagged function, **measure its production callers first**
(`git grep`, excluding tests), then:
- **It belongs to an existing command**: name it in that row's
  `mutation_surface`, and cite the caller chain to the command in the return;
- **it is a write nothing owns**: it gets a row, which is a finding, so say
  so;
- **it has no production caller**: **delete it** (K02's precedent). If tests
  in other crates still need it, put it behind
  `#[cfg(any(test, feature = "test-support"))]`, the feature `sui-id-store`
  gained in stage 9, and say which tests;
- **never** a silent allow-list or manifest entry to make the gate pass.

**Already known, so expect them:**
- `user_consent::revoke` (`repos/user_consent.rs:215`): no caller anywhere,
  so delete;
- `federation_provider::set_enabled` (`:222`) and `::delete` (`:303`): test
  callers only, so test-gate them (§3);
- **`purge_expired` functions**, which O03 covers in prose (*"purge functions
  in auth codes, sessions, …"*): name each one in O03;
- **`*_within_tx` primitives** called only from sealed commands: name each
  in its command's row.

**Report the full table**: function, callers, decision, row.

## 3 — the three undispositioned Class-A rows, and two comments

- **C17, C18 and C23** get C19's disposition, in its words: *"Dispositioned
  2026-11-08 (cycle J stage 10): reserve, against RFC 096-B2"*, naming what
  RFC 096-B2 will build.
- With `set_enabled` and `delete` test-gated, C17 and C18 no longer describe
  production code. **Set their `status` and `files` to whatever the
  contract's own header then makes true** (`target-absent` with no files is
  the likely answer), and say why.
- **Re-point the two "until M2b" comments** to RFC 096-B2:
  `authn/federated_mfa.rs:339` and `http/handlers/federation.rs:400`. After
  this stage M2b has closed, and those comments must not point at it.

## 4 — criterion 2 as settled: R1, R2, and the operator reference

**R1: every surviving best-effort append reports its own failure.** The
seven `let _ = audit::append(...)` sites, measured at `044b12c`:

| Site | Event |
|---|---|
| `crates/sui-id-core/src/audit_chain.rs:150` | `audit.chain.verification_failed` |
| `crates/sui-id-core/src/authn/session.rs:61` | `auth.login.failure` (refused before any credential check) |
| `crates/sui-id-core/src/authn/session.rs:235` | `auth.login.password_ok_mfa_required` |
| `crates/sui-id-core/src/oidc/authorize.rs:587` | `oauth2.exchange_code.user_revoked` |
| `crates/sui-id/src/http/handlers/federation.rs:1916` | `emit_audit_soon` (sign-in upstream failure) |
| `crates/sui-id/src/http/handlers/oauth_token.rs:91` | `token.introspect` |
| `crates/sui-id/src/http/handlers/oauth_token.rs:144` | `token.revoke` |

Move each onto **`emit_must_attempt`** (`crates/sui-id-store/src/repos/audit.rs:186`),
whose own doc names this exact defect, with a `MustAttemptDescriptor` each,
as its six existing callers have. **Re-measure the list first**, and say if it
has changed. **Tests:** with the store's fault injector (`backend.rs:185`)
failing the append, for at least `token.revoke` and one denial
(`auth.login.failure` or `oauth2.exchange_code.user_revoked`), assert both
that **the failure is logged** (the `tracing::error!` line) and that **the
primary response is unchanged**: the token is still revoked, the denial
still denies. **Mutation:** revert one site to `let _`, and its test goes red.

**R2: the RFC 7009 path gets a row, and T06/T07 name it.** `/oauth2/revoke`
calls `deny_list::insert` and the refresh revoke through
`sui_id_core::oauth_token::revoke`, and no row owns that path. Give it one,
with the next free `T` id (`T05` and `T10` are free, measured). **Choose its
class from the five the contract uses (A, P, O, I, X) and justify the choice.
If none fits a mutation whose record must not gate it, stop and say so**:
that would be a gap in the class vocabulary, not something to paper over.
Correct T06's and T07's *"subordinate to"* rationales to include it.

**The operator reference.** `docs/src/reference/audit-events.md` gains a
short section, **"Events that can be missing from the log"**. It says that
the events the coverage matrix marks Class B are recorded best effort, so a
security response never waits on the audit write; it lists them; and it says
that **each one that fails leaves an error in the service log**, quoting the
exact field (`event = …`) to search for, as `emit_must_attempt` prints it.
**An operator who finds no `token.revoke` row must be able to learn from this
page how to tell "never happened" from "happened, not recorded".**

## 5 — §F3 as settled: G23 reads the manifests

- **G23 checks every crate's `Cargo.toml`.** `rusqlite` in `[dependencies]`,
  or in any `[target.*.dependencies]`, of any crate other than
  `sui-id-store` fails by name. `[dev-dependencies]` is allowed, which is the
  ruling. `[workspace.dependencies]` is a declaration, not a dependency, and
  is allowed. **State how you treat `[build-dependencies]`**, with the
  reasoning: build scripts are not shipped code, but they do run.
- **A negative fixture**: a fixture crate with `rusqlite` under
  `[dependencies]` fails; the same crate with it under `[dev-dependencies]`
  passes.
- **`tests/e2e/r094_m2a_db_access_scope.rs` retires in the same change**, as
  G17 did when G23 took its checks: one boundary, one gate, one definition
  of true. If you find a reason to keep it, say what it witnesses that G23
  does not.
- **Do not edit RFC 094.** The architect amends its sentence in the commit
  that lands this, so the text never claims enforcement that does not exist
  yet.

## 6 — the concurrency evidence, inventoried

**Measurement only.** For every **sealed** Class-A command, list its race,
replay or concurrent-winner tests by `file:line`, and list the sealed
commands that have none. The assessment (§4) does not claim concurrency
evidence it never inventoried, and this list is what it will cite.

## 7 — constraints

- **No migration**, no schema change.
- **No allow-list widening** in `write-authority.toml` without a recorded
  reason per entry (stage 6's rule, unchanged).
- **`CHANGELOG.md`**: R1 is operator-visible (a failed audit write now leaves
  an error line), and so is the new reference section. One `### Changed`
  entry, in an operator's words. The gate and contract work is developer
  tooling and needs none.
- **Gate evidence**: `bash scripts/local-matrix.sh --changed` on the real
  tree, quoting its scope line. A pre-check clone, if you use one, goes
  beside the tree on the main disk.
- No tag, no `workflow_dispatch`, no Level B claim.

## 8 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage10-the-write-site-2026-11-08.md`:
- the baseline;
- §1's definition of a write call;
- the flagged list and its decision table (§2);
- the second fixture, and **probe A re-run on the real tree, now failing**;
- the C17/C18/C23 rows as changed;
- R1's seven sites, re-measured, and their tests and mutation;
- R2's row and class reasoning;
- the operator section's text;
- the manifest check and its fixture, and how `[build-dependencies]` is
  treated;
- the concurrency inventory;
- per-hunk hashes;
- the matrix scope line and summary.

**On sizing:** the largest round since stage 6, and the split point is in §0.
**If §1's definition turns out to need judgment calls on more than a handful
of functions, report the calls before making them.** That decision is what
this stage exists to make visible, so it must not be made silently.
