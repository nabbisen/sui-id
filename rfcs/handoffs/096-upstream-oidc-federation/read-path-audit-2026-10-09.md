# Read-path audit — does anything reached from a read call site write?

**Dispatched.** 2026-10-09 JST, by the architect.
**Authorized.** `@nabbisen`, 2026-10-09, on the recommendation that it run as
its own task after the `ReadConn` ruling: "Your recommendation is accepted."
**Why now.** Sequenced **before** 096-B1 stage 0, because its output is exactly
what stage 0 converts. Background:
[`readconn-prerequisite-decision-2026-10-09.md`](readconn-prerequisite-decision-2026-10-09.md).
**This is an audit, not a fix.** Change no behaviour. If you find something that
wants fixing, report it; do not fix it in this task.

## The question

**Does anything reached from a read-shaped call site perform a write?**

Not "could it in principle" — the answer to that is yes, and measured below.
The question is whether any call site in this tree actually does.

## What is already measured, so you do not re-derive it

- **`Database::with_conn` hands out `&Connection`** (`db.rs:65-68`), a shared
  reference — and `rusqlite::Connection::execute` takes `&self`. **So a write is
  reachable from every read call site**, with nothing in the type to stop it.
  That is the gap `ReadConn` was designed to close and does not exist to close.
- **No per-statement `sqlite3_stmt_readonly` interrogation** and **no assertion
  that `rusqlite`'s `functions`, `vtab` and `load_extension` features stay
  disabled** exist anywhere in `sui-id-store`. RFC 094's amendments call both
  required M2a controls.
- **Scope: 332 `with_conn` / `with_conn_sync` call sites**, of which **184 are
  in non-test `src`**. Re-count these yourself; my numbers are from `grep` and
  the test/non-test split is a path heuristic, not a measurement of intent.

## Method

Classify **every** non-test call site as read-only or writing, with evidence per
site rather than per file. A site is **writing** if the closure, or anything it
calls transitively, issues `INSERT`, `UPDATE`, `DELETE`, `REPLACE`, `CREATE`,
`DROP`, `ALTER`, a side-effecting `PRAGMA`, or `execute`/`execute_batch` at all.

Two traps worth naming:

- **Transitivity.** A closure that only calls a helper is not read-only because
  the closure body has no SQL in it. Follow the calls.
- **`execute` with a `SELECT`** is still `execute`, and
  `sqlite3_stmt_readonly` would pass it. Judge by what the statement does, and
  say when the two readings differ — that distinction is the whole reason RFC 094
  wanted per-statement interrogation *and* the feature assertion.

Test call sites are out of scope for the verdict, but **say how many you
excluded and on what rule**, so the exclusion is checkable rather than assumed.

## What to return

A package under `.git-exclude/review-requests/`:

1. **The verdict**, first line: does any non-test read call site write, yes or
   no.
2. **The table**: every non-test call site — file, line, read-only or writing,
   and the evidence. Group by file where the answer is uniform, but do not
   collapse a file that is mixed.
3. **Any site you could not classify**, and why. "Unclear" is a result; a guess
   is not.
4. **The count you measured**, against my 332 / 184, with the difference
   explained if there is one.
5. **The conversion list for stage 0**: which sites become `ReadConn` and which
   must stay on a full connection because they genuinely write. That list is
   this audit's deliverable to stage 0.
6. **Anything you think is wrong with this dispatch.** Six of 096-A's nine
   stages corrected something of mine.

## If you find a real write on a read path

**Put it in `.git-exclude/` and say so in the package's first line. Do not
commit it, and do not describe it in a commit message.** The repo is public and
`.github/SECURITY.md` forbids publishing an unfixed finding. A write reachable
on a read path would be a defect worth assessing before it is public, even if
nothing exploits it — that call is `@nabbisen`'s, not ours.

If the answer is a clean "no", that is not a security finding and the audit can
be recorded in git normally.
