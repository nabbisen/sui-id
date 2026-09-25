# RFC 112 — independent design review request

**RFC.** [RFC 112 — Refuse to run against a database this build does not understand](../../proposed/112-schema-version-fail-closed.md). **Proposed.**
**Reviewer.** Mid-capability model, implementation role. It authored neither the
RFC nor its handoff.
**Route.** The routing recorded in `ROADMAP.md` §S1 (2026-08-26); §S1b puts that
rule itself to `@nabbisen` for a prospective decision, which does not change
where this review goes today.
**Why before anything is built.** RFC 112 is **Proposed** and its security
review is marked **Required** in its own header. RFC 105 shipped with no
independent design review because it was dispatched for implementation while
Proposed; that is not repeated. **This review comes first; acceptance follows
it; implementation follows acceptance.**
**Baseline.** `d59bc44` or later.
**Scope.** Read-only. Change no code and no RFC text. Report findings.

## What RFC 112 claims, and what to attack

`migrations::run` skips every migration at or below the stored
`schema_version` and returns `Ok`, so an old binary runs against a new schema
and reads and writes tables whose shape it does not know. Separately, a failed
or unparsable version read becomes `0`, and every migration is then re-applied
from 0001 against a populated database. The proposed answer is to **fail
closed** in both cases.

The handoff measured this at `9f6acdb`. **Re-measure at the baseline**; do not
inherit the numbers.

## 1. The measurements — confirm or refute each

One row per claim: the claim, the `file:line` at the baseline, whether it holds.

1. `run` skips migrations at or below the stored version and returns `Ok`, with
   no comparison against `MAX_SCHEMA_VERSION`.
2. The stored version is read such that **any** read error and **any**
   unparsable value collapse to `0`.
3. `restore` **does** refuse a backup newer than `MAX_SCHEMA_VERSION` — cite it,
   because the RFC's argument is that the server is inconsistent with it.
4. **The re-run is real, and what it costs.** Stamp a populated database at a
   version the runner will not believe, run the migrations, and say **exactly
   what happens**: does 0001 fail on an existing table, silently succeed, drop
   anything, or corrupt anything? The severity of defect 2 is whatever this
   shows, not what the RFC assumes. If it is harmless today, say so.
5. **Every path that opens a database.** Enumerate the callers of
   `Database::open` — server start, each CLI subcommand, backup, restore, any
   test-only path — and say which would be refused under the proposed rule.

## 2. Is fail-closed the right rule, and is refusing to start safe?

6. **The trade this makes.** Fail-closed converts a silent data-integrity risk
   into an outage: a service that will not start until an operator acts. State
   the case *against* it honestly — a rolled-back deployment, a canary or blue
   /green pair sharing one database, an operator who cannot immediately reach
   the newer binary — and then say whether refusal is still right. If it is,
   say why the integrity risk dominates. This project's standard is a design
   that is finally safe **and** does not confuse its operator; a refusal that
   strands someone at 3 a.m. with no route forward fails the second half.
7. **Which commands must still work when the database is too new.** An operator
   whose service refuses to start may need to take a backup *before* restoring
   one, or read a version, or get help output. Say which subcommands should be
   exempt, and what makes an exemption safe (read-only? no schema assumption?
   file-level copy?). A blanket refusal at `Database::open` may take these with
   it — check whether it does.
8. **The check is at open time only.** Two binaries can share one database: the
   newer one migrates while the older one is already running. Does a
   startup-only check leave that hole open, and is it in scope? If it is not
   closable here, say what would close it and recommend where that belongs.
9. **The error taxonomy.** `SchemaTooNew { found, supported }` and
   `SchemaVersionInvalid` are two variants. Is that the right cut? Consider a
   stored version that is *lower* than expected but whose migrations cannot
   apply, a database from a fork, and a version row present but empty. Say
   whether any case falls between the variants.
10. **Nothing is written on refusal.** The RFC requires it. Confirm it is
    expressible — including that no transaction, no `PRAGMA user_version`
    write, and no WAL side effect occurs on the refusal path — and say what the
    test should assert to pin it.

## 3. The operator's line — the part most likely to be got wrong

11. **What the message must say.** It names both versions. Say what else the
    operator needs in that one line to act without guessing: which binary,
    which backup, and what *not* to do (do not delete, do not re-run). Draft
    the exact line.
12. **Does it reach the operator?** Trace how `Database::open`'s error becomes
    stderr and an exit code at server start **and** in the CLI. Say where it
    would currently be swallowed, wrapped or logged at a level nobody sees.
13. **Is a refusal recorded anywhere?** It cannot be an audit row — the
    database is the thing being refused. Say whether a log line is enough,
    what level, and whether RFC 094's audit contract has anything to say about
    a security-relevant event that cannot be written.

## 4. Anything else

14. **Interaction with RFC 106** (backup/restore hardening, also Proposed) and
    with `MAX_SCHEMA_VERSION`'s other readers. The two must not end up with two
    definitions of "too new".
15. **Threats this introduces or misses** — in particular whether refusing to
    start gives anyone who can write the version row a denial-of-service they
    did not already have, and who can write it.
16. **Anything that cannot be built as described**, or is better built another
    way, including what the RFC does not mention and should. Name the docs that
    must change with it (`docs/src/guides/upgrade.md`,
    `docs/src/guides/deployment.md`) and whether either already says something
    that this would make false.

## What to return

A review-request package under `.git-exclude/review-requests/`, containing:
- the claim table for §1, one row per claim, with `file:line` at the baseline;
- the measured result of item 4, which sets defect 2's severity;
- findings ranked blocker / high / medium / low;
- your answers to items 5–16, each citing what you read;
- a recommendation: accept as written, accept with the changes you name, or do
  not accept.

**"Do not accept" is a legitimate outcome** and will not be treated as a failure
to deliver. Do not implement anything: RFC 112 is Proposed, and this review is
what lets it be accepted.
