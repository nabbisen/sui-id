# RFC 096-B1 stage 0 (fix) — the side-effecting "read-only" statement class

**Dispatched.** 2026-10-10 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** **Start from your returned stage 0 tree**, not a clean one.
Everything in it is accepted except the one item below. Read the tip with
`git log -1`, hash against it, name the full SHA.
**Review result.** `.git-exclude/reviewed/rfc-096-b1-stage0-readconn-2026-10-10.md`

## The item

`ReadConn` promises a handle that cannot write. It accepts a class of statements
that `sqlite3_stmt_readonly` reports read-only and which nonetheless have
effects. Proven by executing one through a legitimately obtained `&ReadConn`:

```
ATTACH DATABASE '<path>' AS evil
  prepare: OK, query_row: Err(QueryReturnedNoRows), with_read outcome: Ok(())
  -> the file at <path> was created
```

Accepted at `prepare`, all of them:

```
ATTACH · DETACH · BEGIN · COMMIT · ROLLBACK · SAVEPOINT · RELEASE
```

**This is your own reasoning carried one step further, not a correction of it.**
You refused `PRAGMA` unconditionally by first-token text because
`sqlite3_stmt_readonly` is unreliable for it — correct, found by reading
`rusqlite`'s own source, and verified here at `raw_statement.rs:239`. The class
you identified has more members: statements SQLite reports read-only that act on
connection, transaction or filesystem state rather than on database content.

**Transaction control is the one I would fix first**, ahead of `ATTACH`. A
`BEGIN` executed inside a `with_read` closure leaves the connection in a
transaction *after* the closure returns, on a connection the pool then hands to
the next caller. That has a wider blast radius than creating an empty file.

**Not reachable today**, and I am not implying otherwise: all 67 converted sites
pass static `SELECT` text, as your own audit established. The gap is in the
guarantee a future caller will rely on.

## What to do

**Extend the first-token refusal** — the `is_pragma` mechanism you already built
— to the class. Same shape: refuse by the statement's own leading keyword,
before `sqlite3_stmt_readonly` is consulted, because that function cannot be
trusted to classify these either.

Three things to decide and state rather than assume:

1. **The membership list.** I measured seven. **Derive it yourself rather than
   taking my seven as complete** — I probed a list I thought of, which is not the
   same as enumerating the class. SQLite's own documentation for
   `sqlite3_stmt_readonly` names the cases it does and does not cover, and its
   keyword list is finite; work from that, and say what you found that I did not.
2. **One error or several.** `PragmaRefused` is specific and reads well. Whether
   the class shares one variant naming the keyword, or each gets its own, is
   yours — but the error an operator reads must say which keyword and why, not
   merely "refused".
3. **Whether the keyword check is enough.** `is_pragma` keys on the first six
   non-whitespace bytes. A leading comment (`/* x */ ATTACH ...`) or a leading
   `EXPLAIN` would defeat a naive first-token check. **Test both**, and if either
   gets through, say so — that is a finding about the mechanism, not just this
   class.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The derived membership list**, with its source, and the diff against my
   seven in both directions.
2. **A refusal test per member**, asserting the specific error.
3. **The leading-comment and leading-`EXPLAIN` cases**, whichever way they fall.
4. **An execution test for at least `ATTACH`** — prepare *and* run it, and assert
   no file appears. The prepare-only test I first wrote passed while the
   statement was still exploitable on execution; that is the trap worth pinning.
5. **Mutation evidence** for the extended refusal.
6. **Per-hunk SHA-256** against the tip you named.
7. **Gate evidence** from a throwaway clone with its own `target/`.
8. **Anything you think is wrong with this.** My membership list is a probe, not
   an enumeration, and item 3 may well find the mechanism needs more than a
   keyword check.

**Accepted as-is and not to be touched:** the `ReadConn`/`ReadStatement` types
and their private fields, the absent `execute`, the `sqlite3_stmt_readonly`
interrogation, the unconditional `PRAGMA` refusal, all five `compile_fail`
fixtures, the three feature assertions, the 67 conversions, the one reported
non-conversion, and both shared-helper decisions.

**One correction to the dispatch you worked from:** it said "77 READ sites across
23 files". Your 68 across 26 is right, and I confirmed the audit's WRITE figure
is wrong too — 95, not 86 — with 68 + 95 = 163 reconciling exactly to its own
non-test total. Nothing is unaccounted for.
