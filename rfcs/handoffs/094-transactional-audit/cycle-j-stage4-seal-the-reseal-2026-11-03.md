# Cycle J stage 4 — seal the re-seal, and close M2b criterion 2

> ## Whose turn it is: **the implementation role's.**
> **This closes M2b's criterion 2.** It is the last Class-A best-effort
> append in production that has a top-level operation behind it, and the
> only operation in the system with **no command row anywhere**.
> Stage 5 — making a half-finished rotation legible to an operator — is
> separate and depends on this.

**Dispatched.** 2026-11-03, by the architect.
**Renumbered.** This is **stage 4** of nine. It was called "stage 3" in
earlier prose while §D's stage 3 was the gate work; §D is now numbered
against what happened. See
`rfcs/handoffs/094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md`
§D.
**Scope.** The master-key re-seal becomes a sealed command. **No gate
work** (stages 6–7), **no second record and no doc rewrite** (stage 5),
**no AST gate.**
**Baseline.** The tip of `main` when you start — `git log -1`, record it.
**Status check.** RFC 094 is **Accepted**; M2b criterion 2 is its
closure prerequisite. Building is permitted.

---

## 1 — what is true today, measured not remembered

`sui-id admin rotate-key` re-seals six tables inside **one**
transaction and appends its audit record **outside** it, best-effort:

- `crates/sui-id-core/src/oidc/key_rotation.rs:99` —
  `pub async fn rotate_master_key(db: &Database, new_key: &MasterKey)`.
  Re-seals `signing_keys`, `refresh_tokens`, `user_totp` (secrets and
  recovery codes), `user_webauthn_credentials`, `smtp_config` and
  `email_outbox` in one `db.with_tx_sync`.
- `crates/sui-id/src/cli.rs:534` —
  `let _ = sui_id_store::repos::audit::append(…)` emitting
  `admin.master_key.rotated`. **The error is discarded.** The note
  carries the row count and the archived old-key path.

**There is no row for this operation in
`contracts/write-commands.toml`** — all 102 rows checked, not sampled.
`contracts/audit-coverage-matrix.md:206` classes the event
`B *(A required)*`, and the matrix's own note (corrected by stage 1)
already says why the row is missing: **K06's rationale used to claim the
re-seal primitives were "subordinate to S10 or K01 only"**, which told
every later reader there was no top-level command to look for. A wrong
rationale did not merely mislead, it suppressed the search.

**Do not treat the missing row as a discovery.** It is recorded in
`audit-coverage-matrix.md` and in K06's corrected rationale. Your job is
to close it, not to find it.

## 2 — why this is not "add a sealed command"

**`declare_write_command!` can only be used where it is defined**:
`crates/sui-id-store/src/registry.rs:1367`. The orchestration is in
`sui-id-core`, which is **downstream** of `sui-id-store`. So the
re-seal must move, and `rotate_master_key` is **public API** with three
live callers, all measured:

| caller | what it is |
|---|---|
| `crates/sui-id/src/cli.rs:498` | the production call |
| `crates/sui-id/tests/e2e/key_rotation.rs:335` | in-process test of the re-seal itself |
| `crates/sui-id/tests/e2e/key_rotation.rs:421` | second in-process test, same shape |

(`crates/sui-id-core/src/lib.rs:47-48` re-exports it as
`sui_id_core::key_rotation` via `#[path = "oidc/key_rotation.rs"]`, so
the public path and the file path differ — do not be misled by either.)

**Establish and report, before building**, which of these the move is:

- **(a) Move it.** `rotate_master_key` is deleted from `sui-id-core`,
  the orchestration lands in `sui-id-store` behind the sealed command,
  and the two tests are rewritten against the command.
- **(b) Keep a delegating wrapper** in `sui-id-core` so the public path
  still resolves.

**If you choose (b), the wrapper needs a caller-side test, and you must
prove it fails.** This cycle has been bitten by a kept wrapper
**twice** — cycle I stage 3's CLI helper and stage 2's
`upsert_ldap_shadow` — and both times reverting the handler to the
same-shaped wrapper left the **whole workspace suite green**. Mutate the
wrapper to bypass the sealed command and show a test fails. A wrapper
nothing pins is a second, unrecorded path to the same mutation.

I am not choosing between (a) and (b) for you: the decision turns on how
much of the re-seal is `sui-id-store` internals already, which you will
see before I will. Report which you chose and why.

## 3 — the event name, and the one trap in it

**Add a new name. Do not reuse `admin.master_key.rotated` for the
sealed half.**

Today that event fires **after both renames** — its note includes
`bak_path`, which does not exist until the old key file has been
archived — so it already means *the rotation completed*. An operator
reading `admin.master_key.rotated` concludes the key file is in place.
If the sealed command emitted it at transaction-commit time, that
conclusion would become false exactly in the case that matters: the
database re-sealed, the key file not yet replaced.

So:

- **New, this stage:** `admin.master_key.resealed` — Class A, atomic
  with the six-table transaction. Note: the per-table counts the report
  already carries.
- **Unchanged, this stage:** `admin.master_key.rotated` keeps its
  current meaning and its current position after the renames. **Its
  swallowed error and the documentation of the pair are stage 5**, not
  yours.

**No retirement, and therefore no `CHANGELOG` retirement treatment.**
Nothing promised about `admin.master_key.rotated` changes. Say so
explicitly in your return, because a reader who knows this project
retires names on conversion will expect one.

## 4 — the by-product that comes with it, and the one that does not

**Comes with it.** `rotate_master_key` is `pub async fn` and uses
`db.with_tx_sync`, whose documented contract
(`crates/sui-id-store/src/db.rs:141`, inheriting `with_conn_sync`'s at
`:123` — *"use only in the migration runner and in blocking test
helpers **outside an async runtime**"*). That is a live contradiction of the function's own
contract, and routing through `db.class_a(...)`'s async path fixes it
**incidentally** — it is a consequence of this conversion, not separate
work. Note in your return that it is fixed and that the contradiction
is gone, with the line reference.

**Does not.** The swallowed `audit::append` error at `cli.rs:534` is
stage 5. Converting the re-seal shrinks it — the sealed half's failure
propagates — but what remains is reporting the *second* record's
failure, which belongs with the second record.

## 5 — the refusal path: leave it alone, and here is the measurement

The ordering fix added a refusal: `cli.rs:455-464` bails when
`<key_file>.new` already exists, **before the database is opened**,
because that file may hold the key the database is already sealed
under.

I considered requiring an audit record for that refusal. **Do not add
one.** My own first reasoning for leaving it out was wrong and the
correct reason is different, so both are recorded:

- **Wrong reason** (mine, 2026-11-03): "in the half-rotated case the
  database cannot be opened with the configured key, so no record is
  achievable." **Measured false.** `Database::open`
  (`crates/sui-id-store/src/db.rs:40-50`) never uses the key — it
  checks the schema file, opens the connection, sets three pragmas,
  runs migrations and stores the key. And `audit::append`
  (`repos/audit.rs:116`) takes no key and writes plaintext columns. A
  record **is** achievable.
- **Actual reason.** `Database::open` runs `migrations::run`. Recording
  the refusal durably would mean **migrating a database whose state is
  exactly what is in doubt**, in a path whose entire purpose is to touch
  nothing. The project already draws this line: `runtime/database.rs`
  carries a separate `check(path)` precisely to refuse *"without
  opening the database for writing"*. The durable evidence of a refused
  rotation is the staged file's own existence, plus the refusal on
  stderr, and that is enough — adding a record would also add a new
  best-effort append, moving criterion 2 backwards.

**If you find either measurement wrong, stop and say so** — the
decision flips on them.

## 6 — the records to update, all of them

- **`contracts/write-commands.toml`** — a new row. `K07` is free (`K01`,
  `K02`, `K03`, `K06` are in use; `K04`/`K05` do not appear in the
  contract — confirm that before reusing either, and prefer `K07`).
  Class `K`, `sealed = true`, with `mutation_surface`, `files`,
  `test_id`, `event` and `descriptor` filled like the rows around it.
  Actor: **none**, `system_principal: permitted` — the operator's
  authority is possession of the master key and filesystem access,
  the shape U08 (`:125`), K01 (`:1170`) and C22 (`:814`) already use.
  Check it against those three rows rather than taking it from me — the
  inventory's C ids stop at C24, so do not look for a C25.
- **`contracts/write-commands.toml`, K06** — its rationale ends *"The
  missing row is scoped in cycle J, not here."* Point it at the new row
  now that the row exists.
- **`contracts/audit-coverage-matrix.md:206`** — `B *(A required)*` →
  `A`, and the narrative at `:102` that says the row is outstanding.
- **`docs/src/reference/audit-events.md`** — the new event. Its table at
  `:87` describes `admin.master_key.rotated` as *"All column-encrypted
  values were re-sealed under the new key"*, which is now the **other**
  event's meaning. Add `admin.master_key.resealed` and leave the
  existing row's wording to stage 5, which owns the pair's explanation.
- **G17 and G20 counts** move. Report both numbers before and after.

## 7 — the tests this needs

1. **The sealed command appends exactly one row, in the transaction.**
   The pattern is `s05_rotate_metrics_token_appends_exactly_one_audit_row`
   (`crates/sui-id-store/src/commands/tests/runner/server_settings.rs:233`)
   — read it before writing yours.
2. **Injected-failure rollback.** The re-seal fails mid-transaction and
   **no** audit row survives. M2a's own criterion is that every sealed
   command has one; the inventory stands at **57** sealed, so do not add the 58th without
   one.
3. **The caller test**, if and only if you keep a wrapper (§2). Mutate
   the wrapper to bypass the command and show a named test fails.
4. **The two existing in-process tests** at `key_rotation.rs:335` and
   `:421` keep covering what they cover today. If you rewrite them, say
   what each asserted before and after.

**Do not assert the audit row's `at` against a caller-supplied clock.**
`build_audit_row` (`crates/sui-id-store/src/registry.rs:1143`) sets
`at: chrono::Utc::now()` directly at `:1152`, not from `SharedClock`.

## 8 — what not to do

- No gate work: the AST gate, the structural gate and the string gate
  are stages 6 and 7.
- No second record, no `operators.md` rewrite, no fix to the swallowed
  append: stage 5.
- No `CHANGELOG` retirement section — §3.
- Do not restate a gate command in prose anywhere; A3.4 forbids it.
- Do not run `audit.yml` or claim Level B. Both readings stay with the
  architect.

## 9 — `CHANGELOG`

One `Changed` entry under `[Unreleased]`, leading with what an operator
gets: the master-key rotation's record is now written in the same
transaction as the re-seal, so a re-seal that commits cannot leave no
trace of itself. Name the new event. Say plainly that
`admin.master_key.rotated` is unchanged, since a reader who has watched
this cycle retire names will assume otherwise.

## 10 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage4-seal-the-reseal-2026-11-03.md`,
with: the baseline commit; (a) or (b) from §2 and why; the §5
measurements if you re-ran them; G17/G20 before and after; per-hunk
hashes via `scripts/hunk-hashes.py`; and the gate summary.

**On the gate summary — new, and it applies from this round.** If you
build in a throwaway clone, the run you cite must be **the run on the
real tree, after copying back**, or the `Gates.` line must say which
tree it ran on. Last round's `48/48` was taken in the clone, where the
dispatch file was absent; on the real tree the same matrix is **47/1**,
because G16 reads untracked files. The deliverable was right and the
evidence described a tree nobody shipped. Cheapest fix: copy back
first, run the matrix last.
