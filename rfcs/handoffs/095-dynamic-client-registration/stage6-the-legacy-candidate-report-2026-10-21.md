# M3 stage 6 — the legacy-candidate report, read-only

> ## Whose turn it is: **the implementation role's.**
> Nothing in this stage waits on `@nabbisen` — though what it *produces*
> does: the candidate dispositions are a closure prerequisite and his alone.
> Stage 4 is still blocked on his self-review ruling; stage 6 does not touch
> it.

**Dispatched.** 2026-10-21 JST, by the architect.
**Plan.** [`m3-stage-plan-2026-10-20.md`](m3-stage-plan-2026-10-20.md) §C,
stage 6. Order is 2 → 3 → **5** → 4 → 6; stage 5 landed `a8583e4`, so this
is the last unblocked stage of `0.83.0`.
**Normative source.** **RFC 095 `:350-360`** — not the matrix, which does
not cover this. Quoted in §1.
**Stage 5's review.** `.git-exclude/reviewed/rfc-095-stage5-browser-presentation-2026-10-21.md`
**Baseline.** Read the tip with `git log -1`.

## 1 — the rule, quoted, because every clause of it constrains you

RFC 095 `:350-360`:

> *"Pre-RFC-094 dynamic registrations are a separate historical ambiguity:
> `clients::create` discarded `registered_via`, so their rows look like
> administrator-created clients. The migration **must not infer provenance
> from shape, timestamps, names, or missing application URIs**. A read-only
> upgrade report lists only **positive `client.dynamic_register` audit
> targets whose client row still says `admin`**, labels them
> `legacy_unstamped_dynamic_candidate`, and **records when audit history is
> absent or unverifiable**. **It does not mutate them.** Before M3 closure,
> the owner reviews every reported candidate and durably records
> disablement, explicit legacy acceptance, or a separately audited metadata
> correction. **No "zero candidates" claim is allowed when the historical
> best-effort audit cannot prove completeness."*

Four prohibitions and one positive rule. The prohibitions are the easy ones
to violate by accident, and §5 is about the last sentence, which is the
hardest.

## 2 — the history, measured, so you are not guessing at what the window is

I verified all of this from the repository rather than from the RFC's
summary of it.

**The ambiguity is real and its bounds are exact.** `clients::create` as
RFC 008 shipped it (`48946f6`, 2026-07-02) inserts eleven columns:

```sql
INSERT INTO clients(id, name, confidential, secret_hash, redirect_uris,
                    is_disabled, is_deleted, allowed_scopes,
                    post_logout_redirect_uris, created_at, updated_at)
```

`registered_via` is **not among them**, so the column took its schema
DEFAULT `'admin'` — even though the handler built its `ClientRow` with
`RegistrationSource::Dynamic`. The RFC's description is accurate.

**It was closed by `930513b`** (2026-10-02, *"C15 is atomic — validate
first, then transact"*). So the window is **2026-07-02 → 2026-10-02**.
Put both commit hashes in the report's own output; an operator reading it
should not have to take the window on faith.

**The audit row they left behind.** RFC 008's handler wrote:

```rust
let _ = sui_id_store::repos::audit::append(
    &app.db,
    &AuditLogRow {
        at: now, actor: None,
        action: "client.dynamic_register".into(),
        target: Some(client_id.to_string()),
        result: "ok".into(),
        note: Some(format!("name={client_name}")),
    },
)
```

So a candidate is an `audit_log` row with `action = 'client.dynamic_register'`
whose `target` is a client id, against a `clients` row with
`registered_via = 'admin'`. That is the whole join, and it is the only
evidence the RFC permits you to use.

**`let _ =` is why completeness cannot be proven, and it is the reason the
RFC's last sentence exists.** The append was best-effort: if it failed, the
client was still created and *nothing recorded it*. Today's C15 is Class-A
atomic — the audit row and the client row commit together or neither does
— but that guarantee does not reach backwards. Say this in the report's
output, not only in a doc comment.

**Two things you might otherwise worry about, already checked:**

- **No audit pruning exists.** `audit_log`'s own DDL comment
  (`0001_initial.sql:119`) says *"Append-only; no UPDATE / DELETE statements
  are issued by the codebase"*, and I found none. Missing rows are the
  `let _ =` window, not retention.
- **Every candidate row is inside the hash-chained era.** The chain arrived
  in migration `0009`; `registered_via` arrived in `0035`. No
  `client.dynamic_register` row can be `legacy_unhashed`, so that bucket of
  `ChainVerifyReport` should be zero for this window — and if it is not,
  that is a finding, not a formality.
- **Clients are never hard-deleted** — `repos::clients` has `soft_delete`
  and no `DELETE FROM clients` exists anywhere. §4.3 still requires you to
  handle a target with no client row; see why there.

## 3 — where it lives, and why not in the migration

**A CLI subaction: `sui-id admin legacy-dynamic-candidates [--config PATH]`.**

A migration runs once, unrepeatably, with its output buried in upgrade
logs. The owner must be able to **re-run this** — to see the list, act on
part of it, and look again. That is the deciding reason. It also keeps the
migration doing what the RFC says migrations must do here, which is
nothing.

The CLI is hand-rolled and RFC 107 gave it one list per level. Adding a
subaction means **three places**, and RFC 107's own tests will fail you if
you miss one:

- `ADMIN_SUBACTIONS` (`cli.rs:29-36`);
- the `match` in `run_admin_subcommand` (`cli.rs:155-174`) — note the
  `unreachable!` arm that catches a listed-but-undispatched name;
- `help_text()` (`cli.rs:678`), both the `USAGE:` block and the lower
  descriptive block at `:757`.

### 3.1 — a trap in the existing test, which you must not paper over

`every_admin_subaction_dispatches` (`cli/tests.rs:98-110`) runs **every**
subaction with no flags and calls `.expect_err(...)`, captioned *"every
admin subaction is missing a required flag here"*. Its actual assertion is
only that the message does not say *"unknown admin subaction"*.

Your command has **no required flag**. So one of two things happens:

- it fails because `Config::load("./sui-id.toml")` finds nothing (tests run
  with the crate directory as CWD, and `sui-id.toml` exists only at the
  repository root) — the test passes **for a reason that has nothing to do
  with what it claims to check**; or
- it succeeds, and `expect_err` panics.

**Measure which, and report it.** If it is the first, fix the harness
honestly — the caption is wrong for your command and the next person to add
a flagless subaction will hit the same thing. Do not add a fake required
flag to make the existing test shape work; that would be changing the
command's interface to suit a test, which is backwards.

## 4 — what the report must contain

One `--config`-resolved database, opened exactly as
`run_admin_issue_registration_token` does (`cli.rs:901-912`). **Read-only
throughout**: no `set_registered_via` (it exists, at `clients.rs:334` — do
not call it), no `set_disabled`, no writes of any kind, and no audit event
for having run the report.

### 4.1 — the candidates

For each `audit_log` row with `action = 'client.dynamic_register'` whose
target resolves to a `clients` row with `registered_via = 'admin'`, print,
labelled `legacy_unstamped_dynamic_candidate`:

- the client id, and the name **as the client row has it now** (not the
  audit `note`'s `name=` — they can differ, and the row is the current
  truth);
- `created_at`;
- **`is_disabled` and `is_deleted`**, both of them. RFC 095 gives three
  dispositions: disablement, legacy acceptance, or a corrected metadata
  write. Choosing among them is impossible without the row's current
  state. **Do not filter deleted or disabled rows out** — a silently
  shortened list is the failure this whole stage exists to prevent.
- the audit row's `seq` and `at`.

`result` is `'ok'` for every row this descriptor ever emits — C15 sets
`AuditResult::Ok` unconditionally (`commands.rs:4051-4052`) and, being Class-A,
only commits alongside the client row. Filtering `result = 'ok'` is
harmless and documents the RFC's word *"positive"*; **do not write a
comment claiming it discriminates between outcomes**, because it does not.

### 4.2 — the audit-chain state, reported, not assumed

*"records when audit history is absent or unverifiable."*

`audit::verify_chain_tail(db, limit)` (`repos/audit.rs:376`) returns
`ChainVerifyReport { checked, broken_at_seq, legacy_unhashed }`. **It is a
tail verifier** — it covers the most recent `limit` rows plus one boundary
row, and its own doc is explicit that it tells you nothing about rows older
than that.

The candidates are **old**, so a default-sized tail tells you nothing about
them. Either verify far enough back to cover the oldest candidate's `seq`,
or state plainly in the output which range was verified and that the
candidates fall outside it. **What you must not do is run a tail check and
let the output imply the candidates' history was verified.** That is the
same shape of defect as a passing test that tests nothing, and it would be
worse here because a human acts on it.

Report, explicitly: the verified `seq` range, `broken_at_seq` if any, and
`legacy_unhashed` (which should be `0` for this window — say so if it is
not).

### 4.3 — the third category, which a naive join loses

An `audit_log` target that **does not resolve to a client row at all**, or
that does not parse as a client id.

An inner join drops these silently. They are not "no candidate" — they are
*evidence of a registration whose client cannot be found*, which is
strictly more alarming than a candidate. No hard delete exists in the
codebase, so this should be empty; a restored, hand-edited or
partially-migrated database can still produce it, and "it cannot happen"
is not a reason to make it invisible when it does.

Print it as its own labelled section with the target string and the audit
`seq`.

## 5 — the output when there is nothing to report

**This is the part the RFC legislates and the part most likely to be got
wrong.**

> *"No 'zero candidates' claim is allowed when the historical best-effort
> audit cannot prove completeness."*

An empty result must **not** print "0 candidates", "none found", "clean",
or anything a reader can take as *there were none*. The report must say
what is true: no candidate is visible **in the evidence the RFC permits**,
and that evidence is known-incomplete, because between `48946f6` and
`930513b` a failed best-effort audit append left a dynamically registered
client with no trace at all.

Write that sentence into the output for the empty case, with the window and
both commit hashes. An operator who runs this and sees a bare zero will
conclude the question is settled. It is not, and the RFC says so in as many
words.

**The same honesty applies to the non-empty case**: a list of five is five
*found*, not five *existing*. One line to that effect, every run, not only
when the list is empty.

## 6 — what you must not do, restated because the RFC names each one

- **No provenance inference** from shape, timestamps, names, or missing
  application URIs. If you find yourself writing a heuristic — "created in
  the window and has no `logo_uri`, so probably dynamic" — stop. The audit
  target is the only admissible evidence, and a client with no audit row is
  not a candidate, it is a gap you cannot see.
- **No mutation.** Not the client rows, not `registered_via`, not the
  disabled flag, not an audit event recording that the report ran.
- **No disposition.** The report lists; `@nabbisen` disposes. Do not add a
  `--fix`, a `--stamp`, or a suggested action per row.

## 7 — tests

Sibling-file convention, as stage 5 used.

**Store-level**, for whatever read-only query function you add
(`repos::clients` or `repos::audit`, your call — say which and why):

1. **A candidate is found.** Seed a client with `registered_via = 'admin'`
   and an audit row targeting it; assert it is returned.
2. **A stamped client is not.** Same shape, `registered_via = 'dynamic'`;
   assert it is absent. Without this, test 1 passes if the query returns
   every audited client.
3. **An admin client with no audit row is not.** Assert it is absent —
   this is the no-inference rule, as an executable statement.
4. **An audit target with no client row** lands in §4.3's category, not in
   the candidate list and not nowhere.

**CLI-level**: the three RFC 107 bindings, plus §3.1's measurement.

**The empty-case output is a test, not a formality.** Assert that the
rendered output for zero candidates contains the incompleteness statement
and does **not** contain a bare zero-count claim. That is §5's requirement
made executable, and it is the one a future tidy-up is most likely to
delete as noise.

**Mutation evidence**, two, no more: (a) make the query ignore
`registered_via` and show test 2 failing; (b) remove the incompleteness
sentence and show the empty-case test failing. Stage 5's round proved its
e2e by mutation and it was worth it.

## 8 — documentation

`docs/src/guides/upgrade.md` is where an operator will look — it already
carries the `0035` migration row that added `registered_via`. Add the
command, what it is for, and **why an empty result is not an all-clear**.
The in-tree mention of `legacy_unstamped_dynamic_candidate` is currently
the RFC and the ROADMAP only; an operator does not read those.

## 9 — what to return

The usual package, plus:

- **§3.1's measurement** — which of the two things
  `every_admin_subaction_dispatches` does with your command, and what you
  did about it.
- **Where you put the query and why** (store repo vs. a CLI-local join).
- **The exact empty-case output**, pasted. I want to read the words an
  operator will read.
- **The chain-verification range you chose** and what it cost on a database
  of realistic size, since §4.2 leaves that open.
- Both mutations from §7.
- The full matrix; Level B scope. Report it local.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the last
  commit of the round, baselined on the tip at that moment.

**A `CHANGELOG` entry: yes.** `### Added` — a new admin subaction. Say in
one clause that it is read-only. **Not `### Security`** and not `### Fixed`:
nothing was broken, and this reports on a historical ambiguity rather than
closing one.

## 10 — what this stage does not do

It does not close M3, and it is not the last word on these clients. Two
prerequisites remain after it, and **both belong to `@nabbisen`**. One is a
durable disposition of every reported candidate; the other is his sign-off
on the evidence. Stage 4 remains undispatched pending his separate ruling.

Leave the working tree uncommitted. One open dispatch at a time; this is it.
