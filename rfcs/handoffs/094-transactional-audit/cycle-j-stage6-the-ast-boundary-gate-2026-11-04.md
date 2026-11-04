# Cycle J stage 6 — the `syn` AST boundary gate, and G17 retires

> ## Whose turn it is: **the implementation role's.**
> **This is the largest round of the cycle and the gates force it to be
> one round.** It delivers `xtask audit-structure`, the contract that
> defines its allow-list, its mandatory negative fixture, its
> registration as a Gate Matrix lane, and the retirement of G17 — all
> together, for the reasons in §2. Read §2 before planning.

**Dispatched.** 2026-11-04, by the architect.
**Stage.** 6 of nine. §D of
`rfcs/handoffs/094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md`.
**Closure prerequisite.** RFC 094 M2b: *"the AST boundary gate landed,
**with a negative fixture that rejects an unregistered write**"*. This
is that criterion.
**Scope.** The gate. **Not** the dependency condition (§6 — held out,
with a question for the owner). **Not** stages 7–9.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.
**Status check.** RFC 094 is **Accepted**. Building is permitted.

---

## 1 — one thing RFC 094 says you must build already exists

RFC 094 (`rfcs/accepted/094-…md`, the structural-gate section) says:

> *"Registering it requires a mechanism that does not exist yet.
> `scripts/check-gate-inputs.sh` validates every lane against RFC 093's
> table alone, so no other RFC can own a lane. **This RFC delivers
> multi-source lane ownership** as part of its structural-gate work."*

**That is stale. It was delivered.** `scripts/check-gate-inputs.sh`
carries *"condition 7 (RFC 094 R10): the multi-source lane registry"*
with six checks, and `contracts/gate-inputs.toml:53` has the
`[gate_owners]` table. Registering this lane is a `[gate_owners]` entry
of `"094"`, not a mechanism to build.

**Do not build it, and do not amend the RFC.** Report the stale
sentence in your return; correcting an Accepted RFC's text is a
separate decision and not yours or mine to take in passing.

## 2 — why this cannot be split the obvious way

The obvious split is "build the tool, register it later". **G18 forbids
it.** `contracts/contract-paths.toml` states the rule the gate enforces:

> *"every file in the directory has a row in its README, and every row
> names a file that exists, **a gate that is in `[gates]` and in the
> Gate Matrix**, a script that exists and an RFC that exists."*

So `contracts/write-authority.toml` cannot exist until the lane that
reads it is a registered `[gates]` lane with a Gate Matrix row. And RFC
094 requires that *"when it lands, G17 is retired in the same change so
that one file does not have two definitions of true."*

**Therefore one change**: the `xtask` crate, the contract file, its
`contracts/README.md` row, the checker, the negative fixture, the lane
registration, and G17's retirement.

**If measurement says it must still be split, say so before building,
and say where.** Any split must keep the contract file, the gate, the
README row and the registration together. The separable candidate is
the hardest condition — alias and helper indirection (§3's last
bullet). If you propose deferring that, propose it as its own stage with
the gap named, not as a quiet omission; a structural gate that misses
indirection still reports success.

## 3 — what the checker must reject

RFC 094's condition 7 of its control list. Parse **production** Rust
modules under `crates/` and reject, outside the allowed modules:

- raw `rusqlite::Connection` / `Transaction` / `Statement`;
- statement execution, and `execute` / `batch` / `transaction` calls;
- `with_conn` / `with_tx`;
- writable `PRAGMA`;
- `ATTACH` / `DETACH`;
- backup / restore;
- extension loading;
- raw-handle / FFI access;
- **aliases and helper indirection to any forbidden type.**

**The allowed set is the private executor, the migration runner, and
test fixtures**, and it must be **exact** in
`contracts/write-authority.toml` — RFC 094: *"The small allowed-module
list is exact in `contracts/write-authority.toml`; changing it requires
independent review."* Put that sentence's force into the file's own
header, so the next person widening the list reads the constraint
where they are working.

Derive the list from the tree, not from this dispatch. If the honest
list is larger than "executor, migrations, fixtures", **say so with the
`file:line` of each addition and why** — a widened allow-list recorded
with reasons is a finding; one recorded silently is the hiding place
`contract-paths.toml`'s own header warns about.

## 4 — G17 has seven checks, not the four the RFC names

RFC 094 summarises G17 as four things. **The script has seven** —
`scripts/check-write-commands.py` `(A)` through `(G)`: code→inventory
completeness, inventory→code for sealed rows, `event`/`descriptor`
derived rather than written, `files` existence, row well-formedness,
**no hand-written count**, and `sealed_by`. The last two postdate the
RFC's summary entirely. Enumerate them from the source and state in
your return which of the seven `audit-structure` now performs and how
each is tested.

**Retire G17 in the same change**: remove the `[gates]` key, its
`[gate_owners]` entry, its Gate Matrix row, its `ci.yml` job, and the
script, and check A3.4's conditions still pass — condition 7's
lane-completeness rule in particular, which requires every lane
declared by a source RFC to be in `[gates]` or
`[gate_matrix_exceptions]`, never both and never neither.

**Nothing the inventory relies on may silently stop being checked.**
`contracts/write-commands.toml` is 103 rows and the thing the whole
cycle has been measured against; if any of the seven checks is dropped
rather than carried, name it and say why.

## 5 — the negative fixture, which the criterion names explicitly

RFC 094: *"Its mandatory negative fixture adds a new repository function
that calls `Connection::execute("UPDATE …")` without any declaration;
both `cargo check` and `cargo xtask audit-structure
--negative-fixtures` must reject it."*

**Both.** The compiler capability is primary and the AST check protects
the module boundary; a fixture that only one of them rejects does not
demonstrate the criterion.

Follow the precedent already in the tree: RFC 093 A3.2's gate-matrix
fixtures live under `scripts/tests/fixtures/`, which the root
`Cargo.toml` **excludes from the workspace** (`exclude =
["scripts/tests/fixtures"]`) precisely so a deliberately-broken package
does not break `cargo … --workspace`. Read how those are invoked before
inventing a mechanism.

**The fixture must fail for the right reason.** Assert the rejection
message or exit distinguishes "unregistered write" from "did not
compile for some other reason" — RFC 094's own words: *"a fixture
asserted against the wrong [control] passes for the wrong reason."*

## 6 — the dependency condition is held out, and here is the conflict

RFC 094 specifies, in the same section, that `rusqlite` must not appear
in **"the `[dependencies]` or `[dev-dependencies]` table of any crate
other than `sui-id-store`"**.

**Implementing that literally fails the current tree**, and I measured
it rather than assuming:

- `crates/sui-id/Cargo.toml:115` declares `rusqlite` under
  `[dev-dependencies]` (table opens at `:100`), with a comment at
  `:113` recording why.
- **At least nine e2e test files use it**, including
  `tests/e2e/key_rotation.rs`'s `Home::raw()` and the `audit_note`
  helper stage 5 added last round.
- And the M2a criterion-7 test's own doc comment settles the opposite
  reading: `tests/e2e/r094_m2a_db_access_scope.rs` says *"The clause is
  about production reach, not every Cargo edge: a crate may use
  `rusqlite` under `[dev-dependencies]` for its own tests (as both
  `sui-id-store` and `sui-id` do) without compromising it; only a
  `[dependencies]` entry outside `sui-id-store` does."* **M2a closed on
  that reading.**

So an Accepted RFC's normative text and the test that discharged its
own closure criterion disagree, and the stricter reading would mean
rewriting nine test files to inspect the database through
`sui-id-store`'s API instead of raw SQL.

**That is not yours to resolve and I have not resolved it either.** It
is with `@nabbisen`. **Do not implement the dependency condition in
this round, in either form.** Nothing regresses by waiting:
`r094_m2a_db_access_scope.rs` already enforces the `[dependencies]`
reading on every test run.

## 7 — the `xtask` crate's constraints, which are release constraints

A seventh workspace member touches the release machinery. All of these
are measurable, so measure them:

- **`publish = false`.** It is a dev tool and must never reach the
  registry.
- **Do not add it to `[workspace.dependencies]`.** The release checklist
  counts **exactly six** version sites — `scripts/check-publish-preconditions.py`'s
  `DEP_CRATES` (five) plus `[workspace.package].version`. A seventh site
  would make the preconditions describe something other than the six
  published crates. Confirm `grep -n '0\.87\.1' Cargo.toml` still
  returns six lines after your change.
- **It must pass every lane**: `+1.95` and `+stable`, with and without
  `--all-features`, `clippy -D warnings`, and `fmt`. `cargo … --workspace`
  will now build it.
- **`syn` is already in `Cargo.lock`** — `2.0.119` and `3.0.9`, both
  transitive. State which you depend on and why, and confirm it builds
  on MSRV **1.95**. `Cargo.lock` will gain edges, which also puts this
  commit in `audit.yml`'s path filter; that is expected, not a problem.

## 8 — what not to do

- No multi-source lane mechanism (§1 — it exists).
- No dependency condition (§6).
- Do not amend `rfcs/accepted/094-…md`. Report its stale sentence.
- Do not widen the allow-list without recording each addition.
- Do not restate a gate command in prose; A3.4 forbids it.
- No `audit.yml` run, no Level B claim. Both readings stay with me.

## 9 — `CHANGELOG`

**Probably none, and say which you chose.** This is developer-facing
tooling: a gate replaces a script. Nothing an operator or a library
consumer can observe changes. If you think otherwise, write the entry
and say what a reader outside this repository does with it.

## 10 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage6-the-ast-boundary-gate-2026-11-04.md`,
with: the baseline commit; the exact allow-list and the justification
for anything beyond the three allowed module kinds; which of G17's seven
checks moved and how each is tested; the fixture's rejection evidence
from **both** controls; the six-version-site confirmation; the `syn`
choice and its MSRV check; per-hunk hashes; and the gate summary **run
on the real tree after copy-back**, or a `Gates.` line saying which tree
it ran on.

**If this is more than one round, say so before building.** You have
been right about scope before and I have been wrong about it twice this
cycle. A measured "this is three rounds, here is the split" is a better
return than a half-finished one.
