# RFC 096-B1 stage 0 — `ReadConn`

**Dispatched.** 2026-10-09 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
authorized 2026-10-09. This is its stage 0.
**Why this stage exists.** [`readconn-prerequisite-decision-2026-10-09.md`](readconn-prerequisite-decision-2026-10-09.md)
— `ReadConn` is one of four M2a foundation pieces RFC 096 `:89-93` names as
required by 096-B1, and it did not exist.
**Your input.** [`read-path-audit-result-2026-10-09.md`](read-path-audit-result-2026-10-09.md),
your own audit, accepted. Its conversion list is this stage's scope. **Work from
its per-file tables, not its conversion-list paragraph** — that paragraph
self-corrects mid-sentence on `users.rs` and `scope_definition.rs`; the tables
are unambiguous and the totals reconcile.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## What to build

**1. `ReadConn` — the typed read-only wrapper.** A handle that can read and
cannot write, enforced by its type rather than by convention. The gap it closes,
measured: `Database::with_conn` hands out `&Connection` (`db.rs:65-68`) and
`rusqlite::Connection::execute` takes `&self`, so today a write is reachable
from every one of the 163 non-test call sites with nothing in the type to stop
it.

How you expose it is yours to design, and say why in the package. The obvious
shape is a `Database::with_read` taking a closure over `&ReadConn`, with
`ReadConn` offering `prepare`/`query`-shaped methods and no `execute`. **State
what stops a caller reaching the inner `Connection`** — if `ReadConn` can hand
it back, the type guarantees nothing.

**2. The per-statement `sqlite3_stmt_readonly` interrogation.** RFC 094's
2026-08-12 amendment made this a *required* M2a control. Every statement
prepared through `ReadConn` is interrogated and refused if it is not read-only.

**This must be a running check, not a one-time audit result** — your own audit
made exactly this point and it is the reason it matters: you confirmed no site
today hides a `SELECT` behind `execute` such that `sqlite3_stmt_readonly` and
the intent disagree, but nothing stops a future edit introducing that mismatch
with nobody re-running the audit. The interrogation is what makes it
continuous. Say in the package what happens when it fires: an error is right, a
panic needs arguing for.

**3. The feature assertion.** RFC 094's 2026-08-26 amendment requires an
assertion that `rusqlite`'s `functions`, `vtab` and `load_extension` features
stay disabled, with its own reasoning: statement-level read-only status does not
constrain side-effecting application functions or virtual tables, and *"that
surface is currently not compiled in, and nothing checked it."* Nothing checks
it today either.

Decide where this lives — a compile-time assertion, a startup check, or a gate —
and justify it. A compile-time failure is strongest if expressible; a test that
only passes today is the weakest and should be argued for rather than defaulted
to.

**4. Convert the 77 READ sites.** Across 23 files, per your tables. **Do not
touch a WRITE site's connection type** — your audit established that all 86 are
single-statement writes through `with_conn`, not `with_tx`, and none is a
conversion candidate.

## Constraints

**No behaviour change on any read path.** This is a type-level and
instrumentation change. If converting a site would change what it returns or
when it errors, stop and report it rather than adapting the call site to fit.

**`with_conn` stays.** The 86 WRITE sites use it and are correct. This stage
adds a narrower door; it does not remove the wide one.

**This is RFC 094's control, built here.** Nothing in this stage touches
federation, attempt state or 096-A's validators — those are stages 1 onward.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The design**, with the two decisions above stated and justified: what stops
   a caller reaching the inner `Connection`, and where the feature assertion
   lives.
2. **A test that the type guarantee holds** — a `compile_fail` fixture proving a
   write cannot be expressed through `ReadConn` is the strongest form, and this
   project has five of them already.
3. **A test that the interrogation fires**, by constructing a statement that
   passes the type check and fails `sqlite3_stmt_readonly`. If you cannot
   construct one, that is a finding about the type guarantee being stronger than
   expected — say so rather than omitting the test silently.
4. **The conversion**, with a count reconciling against the audit's 77, and any
   site you could not convert, with the reason.
5. **Mutation evidence** for the interrogation and the feature assertion.
   A surviving mutant reported honestly beats a contrived test — stage 7's
   `ct_eq` report is the precedent.
6. **Per-hunk SHA-256** against the tip you named.
7. **Gate evidence** from a throwaway clone with its own `target/`, plus
   `cargo test --workspace` — which found a real problem in two of 096-A's last
   three stages.
8. **Anything you think is wrong with this dispatch.** Six of 096-A's nine
   stage dispatches corrected something of mine, and your audit corrected a
   third one — the inflated 332/184.

**Not 096-B1's closure, and not 096-A's acceptance.** Stage 0 is a prerequisite,
not one of `:23`'s six closure items; stages 1–8 carry those.
