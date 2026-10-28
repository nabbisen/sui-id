# RFC 142 — Upgrading is the operation we test least

**Status.** Proposed
**Security review.** Required — a failed or half-understood upgrade of an
identity provider's database is an availability and integrity event, and
one of the measures proposed here touches what is written to disk before
a migration runs.

**Design prerequisites.** None.
**Implementation prerequisites.** RFC 000 forbids implementation while
this is Proposed.
**Closure prerequisites.** A populated database at each covered
historical schema version migrates to head in CI with its rows and key
invariants intact; the supported upgrade range is stated in
`docs/src/guides/upgrade.md` and, where a floor exists, enforced in code
with a negative fixture; an operator can see what an upgrade would do
before it does it; and whatever D4 settles on is implemented with a test
that proves the refusal or the backup actually happens.
**Tracks.** Long-lived maintenance. Arises from a question about upgrade
safety, 2026-10-28.
**Touches.** `crates/sui-id-store/src/migrations.rs` and its tests,
`crates/sui-id/src/cli.rs`, `crates/sui-id/src/runtime/database.rs`,
`docs/src/guides/upgrade.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** The architect (high-capability model).

## Summary

Upgrading is the operation an operator performs most often and the one
this project exercises least. The **mechanism** is sound and this RFC
does not change it. What is missing is **evidence that it works across
the whole chain**, **a statement of what is supported**, and **a way for
an operator to find out what will happen before it happens**.

## Background — what already works, so this RFC does not re-propose it

Measured 2026-10-28 on `ff595aa`. Credit where the design is already
right:

- **Each migration runs in its own `Immediate` transaction, and the
  schema version is written in that same transaction** (`migrations.rs`
  `apply_migration`, RFC 112 D7). A failure part-way through 49
  migrations therefore leaves the database at a **well-defined earlier
  version**, not a half-migrated one, and the next start resumes.
- **A database the build does not understand is refused before anything
  is written** — `SchemaTooNew` and `SchemaVersionInvalid`, both with
  operator-facing lines, and the refusal names **which release last
  migrated the file** (`last_migrated_by`, `runtime/database.rs:83-92`).
- **The schema version is visible** — the admin Settings page
  (`handlers/settings.rs:328-339`) and `verify-backup`'s report
  (`cli.rs:130`).
- **`backup`, `restore` and `verify-backup` exist**, and the upgrade
  guide's procedure leads with a backup and warns that `cp` of a
  WAL-mode `.sqlite` can miss committed data.
- **A harness for testing a migration against an older schema exists**:
  `migrations::run_up_to(conn, N)`.

**None of that is the gap.** The gap is below.

## The gap, measured

**`run_up_to` is called for exactly two migrations.** Every call site is
in `tests_rfc021.rs`, for 0021 and 0022 — that RFC's own round. There
are **49 migrations**. **No test takes a populated database at an early
schema version to the current head.**

So the project's position today is: the chain is believed to work
because each link was reviewed when it was written, and because nobody
has reported otherwise. For a self-hosted identity provider whose
advertised install path is `cargo install sui-id`, where the operator
upgrades on their own schedule and may skip several releases, that
belief is carrying more weight than the evidence behind it.

Two smaller gaps follow from the same place:

- **Nothing states which upgrades are supported.** `check_supported`
  refuses a schema **newer** than the build; there is no floor and no
  claim. Whether `0.76 → 0.86` in one step is supported is currently a
  guess — a reasonable one, since migrations are forward-only from 0001,
  but a guess.
- **Nothing lets an operator look before leaping.** There is no way to
  ask a binary what it would do to a database. The first thing that
  happens is the thing itself.

## Design

### D1 — a migration corpus, and it is the one that matters

**For each covered historical schema version: build a populated database
at that version, migrate it to head, and assert the data survived.** In
CI, as a gate.

**Populate it by construction, not by checking in binaries.**
`run_up_to(conn, N)` brings a fresh database to version `N`; per-version
seed SQL then inserts rows using the schema *as it was at N*. No binary
fixtures in git, and the corpus stays honest as migrations change,
because a renumbering breaks the helper loudly — `migration_sql` already
panics on a missing version for exactly that reason.

**Assert more than "it did not error."** Per covered version: row counts
per table before and after, the specific invariants each intervening
migration claims to preserve, and that the final schema version equals
`MAX_SCHEMA_VERSION`. A migration that silently drops rows passes a
"no error" test and fails this one.

**Cost, stated plainly:** per-version seed SQL is real work, roughly
proportional to the number of versions covered. That is why D1 asks
which versions, not all of them — see the open questions.

### D2 — state the supported range, and enforce a floor if one exists

Once D1 runs, we will know whether every covered version reaches head.

- **If all do**: say so in `docs/src/guides/upgrade.md` — any supported
  release upgrades directly to any later one, no stepping-stone
  required. That sentence is worth more to an operator than any amount
  of mechanism.
- **If some do not**: that is a defect to fix, or a floor to declare.
  A declared floor gets a `SchemaTooOld` refusal **symmetric with
  `SchemaTooNew`** — refused before anything is written, naming the
  version found, the floor, and the release to upgrade through first.
  With a negative fixture, like every refusal this project has.

**Do not declare a floor to avoid fixing a migration.** A floor is a
statement that an upgrade path is unsupported, and it strands whoever is
below it.

### D3 — let an operator see what will happen

A read-only subcommand that opens nothing for writing and reports: the
stored schema version, the release that last migrated the file, the
list of migrations that **would** apply, and the version it would end
at. Then exits.

This is the second half of the project's philosophy applied to the
riskiest routine operation an operator performs. Today the way to find
out what an upgrade does is to do it.

**It must not write.** Not the version, not `sui_meta`, not a WAL frame
it did not need. The existing `database::check` path already refuses
without opening for writing and is the model.

### D4 — the backup the guide already asks for

The upgrade guide's first step is a backup. **Nothing enforces it**, and
the failure a backup protects against — a migration that corrupts or
loses data — is precisely the one that cannot be undone afterwards.

Two shapes, and I do not think this is mine to pick:

- **(a) Take one automatically** before the first migration of a run,
  to a path derived from the database path, with `--no-backup` to
  decline. Costs disk and time on every upgrade, including the many
  that are trivial, and an operator who already backs up gets a
  duplicate.
- **(b) Refuse to migrate without one**, unless the operator passes a
  flag asserting they have one. Costs nothing and helps nobody who
  ignores the prompt; it converts a documented step into an enforced
  one, which is honest but adds friction to every upgrade.

**My recommendation is (a)**, because the operator who most needs the
backup is the one who did not read the guide, and (b) does not help that
person. But (a) writes to an operator's disk without being asked, which
is the kind of thing that should be a deliberate decision rather than an
implementation detail.

## Alternatives rejected

- **Down migrations.** Forward-only is deliberate and should stay. A
  down migration that cannot restore dropped data is worse than no down
  migration, because it looks like a way back. Restoring a backup is the
  way back, which is why D4 matters more than this would.
- **Checking in binary fixture databases.** Faster to write and
  immediately stale: a fixture built once stops reflecting what the
  schema at that version actually was the moment anything is corrected.
  D1's construction approach fails loudly instead.
- **Testing only the newest few versions.** The operator most at risk is
  the one who upgrades least often, which is exactly the operator this
  would not cover.

## Open questions for the approver

1. **Which historical versions does the corpus cover?** Every published
   release is the thorough answer and the expensive one. My
   recommendation is **the `0.76` line's first published version, one
   mid-point, and the two most recent minors** — four, chosen so the
   oldest realistic installation and the common cases are both covered,
   and extended later if D1 finds anything.
2. **D4: (a) or (b)?** Recommended (a).
