# RFC 096-B1 stage 8, fix round — bind the link and the parent row

**Dispatched.** 2026-10-13 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
its stage 8.
**Stage dispatch.** [`stage8-migration-0047-mfa-pending-and-ceremony-2026-10-13.md`](stage8-migration-0047-mfa-pending-and-ceremony-2026-10-13.md).
**Review result.** `.git-exclude/reviewed/rfc-096-b1-stage8-migration-0047-mfa-pending-and-ceremony-2026-10-13.md`
— read it first; it carries the measurements behind every item here.
**Baseline.** Still `230e36bb58195aa609fb50b3a38fde602bb93c04`. `0047` has
never been committed, so this round **edits the migration in place**. Do not
add an `0048`.

**Nothing you returned was wrong.** This round exists because three
obligations the clause table assigned to later stages' *code* are enforceable
in the *schema*, which I established by building them and running them, not by
reasoning about them. The clause table changes with the migration.

Your two findings are both accepted. One of them corrects my dispatch (G19 is
federation egress, RFC 134; no gate checks migrations at all) — you were right
to check it instead of obeying it.

## R1 — an unparseable timestamp silently disables the expiry `CHECK`

`strftime('%s', <unparseable>)` is `NULL`, `NULL = NULL + 300` is `NULL`, and
a SQLite `CHECK` passes on `NULL`. Both of these are accepted today:

```sql
INSERT INTO p VALUES ('b','not-a-timestamp','also-garbage');
INSERT INTO p VALUES ('c','','');
```

So the `+300` rule binds only a writer that already stores a parseable
timestamp. Make it two-valued — verified to keep every one of your positive
fixtures passing, including the no-fractional-part and midnight cases, and a
`T`/`Z` form:

```sql
CHECK (
    strftime('%s', created_at) IS NOT NULL
    AND strftime('%s', expires_at) IS NOT NULL
    AND CAST(strftime('%s', expires_at) AS INTEGER)
      = CAST(strftime('%s', created_at) AS INTEGER) + 300
)
```

Negative fixtures owed: an unparseable `created_at`, an unparseable
`expires_at`, and the empty string.

## R2 — the ceremony's cross-table obligations are enforceable

Your statement that a SQLite `CHECK` sees one row and never another table's is
correct. The conclusion drawn from it — that `:736-737`'s exact binding
agreement and `:737-738`'s no-later expiry *"are not schema properties"* — is
not: a **composite `FOREIGN KEY`** compares two tables by construction.

Add a `UNIQUE` index over the binding on the parent, carry the parent's
`expires_at` on the ceremony row, and point one composite FK at it:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS ux_federation_mfa_pending_binding
  ON federation_mfa_pending (id, provider_id, provider_config_version,
                             provider_activation_generation, user_id, expires_at);
```

```sql
-- federation_webauthn_ceremony
  pending_expires_at  TEXT NOT NULL,
  expires_at          TEXT NOT NULL CHECK (
      strftime('%s', expires_at) IS NOT NULL
      AND strftime('%s', pending_expires_at) IS NOT NULL
      AND CAST(strftime('%s', expires_at) AS INTEGER)
        <= CAST(strftime('%s', pending_expires_at) AS INTEGER)
  ),
  FOREIGN KEY (pending_id, provider_id, provider_config_version,
               provider_activation_generation, user_id, pending_expires_at)
    REFERENCES federation_mfa_pending (id, provider_id, provider_config_version,
               provider_activation_generation, user_id, expires_at)
    ON DELETE CASCADE
```

The two halves are one mechanism: the FK is what makes the copied
`pending_expires_at` truthful, and the truthful copy is what makes no-later
expiry expressible as a single-row `CHECK`. Measured, on the real column
shapes with `PRAGMA foreign_keys = ON`: a correct copy with an earlier expiry
stores; a wrong `user_id`, a wrong `provider_config_version` and an untruthful
`pending_expires_at` each fail the FK; an expiry later than the parent's fails
the `CHECK`; deleting the parent cascades.

Two things to write down rather than leave implicit:

- `pending_expires_at` is a **deliberate denormalization**, and the comment
  should say what it buys: a cross-table invariant becomes a constraint
  instead of a convention.
- The FK makes `UPDATE federation_mfa_pending SET expires_at = …` fail while a
  ceremony exists — a pending row cannot be extended out from under a live
  ceremony. I ran it. Terminalizing the parent (`status`, `failure_count`,
  `continuation_sealed`) is unaffected, since those are not FK columns; I ran
  that too. Record the first as a property, with a test.

Keep the ceremony's own `provider_config_version >= 0` /
`provider_activation_generation >= 0` checks.

## R3 — `:722`'s *link* is stored but not bound

The clause table says the `(user_id, provider_id)` pair *"already names the
exact link row."* It names one; nothing makes it exist. Today a
`federation_mfa_pending` row is valid for a pair that was never linked, and it
survives an unlink.

`federation_link`'s `PRIMARY KEY (user_id, provider_id)` (migration 0038) is a
usable parent key:

```sql
FOREIGN KEY (user_id, provider_id)
    REFERENCES federation_link (user_id, provider_id) ON DELETE CASCADE
```

**Replace the two single-column FKs with this one** — do not add a third. It
is strictly stronger: measured, deleting the user and deleting the provider
still cascade the pending row and its ceremony, transitively through the link;
a pending row for an unlinked pair is refused; and an unlink now takes the
live challenge with it, which is the behaviour we want when a user revokes a
provider connection mid-challenge. Keep both columns; they are read, and the
ceremony's composite FK needs them.

Your own `FOREIGN KEY` reasoning is not overturned by this — it is extended.
Rewrite the comment so it argues the same way about the link, not so it merely
lists the new constraint.

**Check this before you write it, and say so in the package.** Is there any
path in RFC 096 that creates a federated pending-MFA row *before* the link
exists? My reading of `:719-722` is no: a user provisioned by F04 has no local
MFA and never enters the pending challenge, and every other route resolves an
existing link first. If you find a route where this FK would refuse a
legitimate row, **that is a finding and it comes back to me** — do not work
around it, and do not weaken the FK to accommodate a route without telling me
which one.

Note in your package that `federation_link::delete`
(`repos/federation_link.rs:162`) has **no caller today** — the self-service
unlink is not wired — so the unlink cascade is a property for when it is, not
one exercised now. Say it that way rather than claiming a live path.

## R4 — assert `MAX_SCHEMA_VERSION >= 47`

Your replacement canary is the same hardcoded literal under the same
self-dating name as the one you removed, and your package already says it is
due for removal next stage. A canary every future stage must edit teaches
people to edit canaries. `>= 47` still fails if `0047` is never registered in
`MIGRATIONS`, which is the thing worth catching, and it stays true afterwards.
Rename it accordingly.

## R5 — one stated rule for timestamps

Write the rule in the migration and hold both tables to it: **no timestamp
column may hold a value SQLite cannot parse.** R1 covers
`federation_mfa_pending` (its `CHECK` references `created_at`). The ceremony's
`created_at` is referenced by nothing — give it
`CHECK (strftime('%s', created_at) IS NOT NULL)`.

## R6 — pin the two generic tables' column sets

Your `:738` finding is accepted: the prohibition is an absence, and nothing in
SQLite forbids a future column. But *"review of any future migration"* is a
duty with no owner and no failure signal. Give it one, in
`tests_rfc096_b1_stage8.rs`, via `PRAGMA table_info`, asserting the **exact**
column-name list:

- `login_pending_mfa` — `id`, `user_id`, `expires_at`, `created_at`
- `webauthn_pending` — `id`, `kind`, `user_id`, `state_json`, `expires_at`,
  `created_at` (migration 0013's rebuild, not 0004's original)

An exact set, not a pattern match: a future reference may not be named
`federation_*`, and anything added to either table deserves a deliberate
decision at that moment. Put `:738` in the test's doc comment so whoever
fails it knows why it exists. Keep the migration's closing comment — the test
enforces what it says; it does not replace saying it.

## Not in scope, unchanged

No writer, no purge (stage 9's, with the first writer), no
`FederatedMfaVerifier`, no F02/F03/F06.

**One question I am not asking you to answer here**, recorded so it is not
lost: what stops a user at `failure_count = 5` from starting a fresh federated
login and getting a new pending row at 0? `:740`'s *"five is the fixed maximum
across all local methods combined"* governs one challenge. The schema cannot
answer it. It is stage 10's, and I will ask for it there.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The revised migration**, with each of R1–R3 and R5 argued at the
   constraint it governs, not in a header block.
2. **The revised clause table.** Four rows move from a later stage's code to
   this stage's schema (`:730` link, `:732-733` expiry well-formedness,
   `:736-737` binding agreement, `:737-738` no-later expiry). The rows that
   stay on the code's side — `:733`'s read-time `guard_at` comparison,
   `:733-734`'s no-revival-past-terminal, `:734`'s purge, `:740-751`'s
   semantics — should each still name their stage.
3. **Negative fixtures for every new constraint**, the same standard as the
   first round: refused, demonstrated, not asserted. At minimum the three
   unparseable-timestamp cases, the three FK-mismatch cases, the later-expiry
   case, the unlinked-pair case, and the parent-expiry-`UPDATE` case.
4. **The cascade chains, measured**: unlink, user delete, provider delete —
   each showing what remains in both new tables.
5. **The R6 test**, and confirmation it fails when a column is added (mutate
   it to prove the test can fail; revert, `md5sum`).
6. **Your answer on the R3 flow question**, with the RFC lines you read.
7. **Migration evidence again** — fresh and 46→47 identical, by the same
   definition you gave, on the revised file.
8. **Per-hunk SHA-256** against `230e36bb58195aa609fb50b3a38fde602bb93c04`,
   full-content for new files.
9. **Gate evidence**: the full matrix from a throwaway clone with its own
   `target/`. Not G19 this time, and not because migrations are its subject —
   because the manifest declares it on `paths = ["**"]` like the rest.
10. **The count reconciliation** from `sui-id-store --lib` **432** (measured by
    me on your returned tree, not carried from a dispatch), `sui-id --lib`
    **560**, `sui-id --test e2e` **532**, `sui-id-i18n` **19**, `scripts/tests`
    **349**.
11. **Anything you think is wrong with this round.** R2 and R3 are mine, built
    and measured but not yet run inside this codebase's test suite against the
    bundled SQLite (3.50.x via `libsqlite3-sys 0.38.1`; I used the 3.53.4
    CLI). If either behaves differently there, that is the finding and it
    outranks the instruction.
