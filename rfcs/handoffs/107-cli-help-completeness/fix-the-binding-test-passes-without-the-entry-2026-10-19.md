# RFC 107 — fix round: the binding test passes with the entry gone

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. RFC 107 is **Accepted** (2026-10-18).

**Dispatched.** 2026-10-19 JST, by the architect.
**Round.** Fix round on the package returned 2026-10-19.
**Baseline.** `658ca38` — this dispatch's own commit, docs only, touching no
`crates/` file. **Your working tree is untouched and stays uncommitted**
(`cli.rs` md5 `044cf8a5…`, as you left it); build on it, do not re-create it.
Your returned hunks were measured against `21a7128`; they are unchanged by my
commit, but the round's final hashes go against `658ca38`.
**Previous dispatch.** [`dispatch-one-list-per-level-2026-10-18.md`](dispatch-one-list-per-level-2026-10-18.md)
— still the specification. Everything in it that I did not reopen below is
accepted.

**Most of the package is accepted and I re-measured rather than trusted it:**
all 16 `VALUE_TAKING_FLAGS` verified value-taking at their own parsing sites
and all 9 exclusions verified bare, no value-taking short flag in existence,
the `unreachable!()` guard correctly placed after the six real arms so no user
input can reach it, hunk hashes regenerated against `21a7128` and identical
(19 hunks), G21 clean, no `### Security` text touched. Your §5 handling of the
G21 miss — amend the one unpushed commit, then re-run the **whole** matrix
against the new hash — is exactly right.

Three items. The first is the round.

## 1 — the binding test passes with the entry removed

`crates/sui-id/src/cli/tests.rs:7-21` asserts `text.contains(name)`. Substring
containment, over a help text in which names appear inside other names and
inside ordinary prose.

**Proved by running it, not argued.** I removed `backup`'s USAGE line *and* its
entire SUBCOMMANDS description block — the subcommand wholly undocumented in
`--help` — and:

```
test cli::tests::cli_bindings_cover_help_and_dispatch ... ok
test result: ok. 7 passed; 0 failed
```

It passes because `verify-backup` contains `backup`. (`cli.rs` was copied aside
first and restored byte-identical, md5 `044cf8a5…` confirmed both sides; your
tree is as you left it.)

Measured across all five:

| name | satisfied by unrelated text? | by what |
|---|---|---|
| `backup` | **yes** | `verify-backup` |
| `restore` | **yes** | *"a real restore to catch…"*, and the `--from`/`--force` option lines |
| `admin` | **yes** | `--admin-username`, `--admin-email`, `--admin-display-name` |
| `verify-backup` | no | |
| `setup` | no | |

**Three of five.** So RFC 107's second closure prerequisite — *"the lists
cannot drift again without a test failing"* — is materially false for the
top-level list today: the exact defect this RFC exists to fix can recur for
three names with every test green.

**Why your mutation check did not catch it.** §4's mutation 3 used
`rotate-metrics-token`, which has no substring collision, and it correctly
failed. One representative was not enough, and the previous dispatch asked for
more than one in those words: *"Mutate each away and confirm it fails — a test
over a list is exactly the shape that can pass while testing nothing."*
**Each.** That is the transferable part of this round, and it is the only
reason it exists.

### What to do

- **Anchor the assertion** so a name matches only where it is the entry being
  documented — a line-anchored match within the SUBCOMMANDS block, a
  word-boundary match, or whatever you judge cleanest. I am not specifying the
  mechanism.
- **Make it structural, not name-specific.** `ADMIN_SUBACTIONS`' six names have
  no collisions *today*; that is luck, not a property, and the fix should cover
  them by construction rather than by their current spelling.
- **Mutate every name away individually** — all five top-level and all six
  admin — and show each one failing. Eleven mutations, eleven failures. That
  is the evidence this round returns.

## 2 — §2's disclosed limit is real, but stated too strongly

Your reasoning for choosing a test over a compile-time binding is sound and I
agree with it: restructuring a plain string-literal `match` into a table of
boxed, pinned futures would buy a guarantee at the price of real indirection in
a dispatcher that runs once per process. **That call stands; do not revisit
it.**

What is too strong is the conclusion. §2 says the arm-without-const direction
*"has no test that can force a compile error without the same restructuring."*
True of **compile** errors — and then treated as though the direction were
uncoverable. **This repository already gates a list against Rust source
literals, bidirectionally.** `scripts/check-audit-matrix.sh` (G13) says so in
its own header: *"Backward: every source literal must have a matrix row."*

An arm added without a const entry is drift of precisely the kind RFC 107
exists to stop: the subaction works, and is absent from both `--help` and the
unknown-subaction message.

**Close it or restate the limit accurately — your call, and I will take either
with its reasoning.** One cheap option if you want it: `include_str!` on the
module's own source and a scan of the `match` for `Some("…")` arms — compile
time, no I/O, a few lines. If you judge that not worth its weight, say so
plainly and the disclosure is corrected rather than overstated. What should not
survive this round is a sentence implying the direction cannot be covered.

## 3 — `--expires-in`: delete the mention, the decision is made

Your §3 was right to flag `cli.rs:893` rather than fix it silently, and right
that it is a product decision. **I am making it: delete the false mention, do
not implement the flag.** `cli.rs:893`'s doc comment advertises
`[--expires-in Nd]`; `:962` hardcodes `expires_at: None`; nothing parses it.
The user-facing `--help` is already correct, so this is a doc comment that lies
about its own function — the same defect class as `cli.rs:95`, which the
previous dispatch had you fix for the same reason.

It goes in this round because the round is open and you are already in the
file, not because it was wrong to scope it out.

## 4 — what to return

The usual package, plus:

- **The eleven mutations from item 1**, each shown failing.
- **Your decision on item 2**, with the reasoning, whichever way it goes.
- The full matrix, Level B scope — this touches `crates/`.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the **last**
  commit of the round, **baselined against `658ca38`** — I have already landed
  this dispatch, so that is the commit, not `21a7128`.

**No new `CHANGELOG` entry.** Your three `### Fixed` entries already describe
the user-visible behaviour and none of this round changes it; item 2 may make
one sentence in a doc comment more accurate, which is not a changelog event.

Leave the working tree uncommitted. One open dispatch at a time; this is it.

## 5 — not required, recorded for when the file is next touched

`every_admin_subaction_dispatches` depends on `./sui-id.toml` being absent from
`crates/sui-id/`. True today — `cargo test` runs with CWD at the package root
and the repository's `sui-id.toml` is one level up — so every subaction fails
at config load, which is what makes the assertion hold. If a config ever sits
beside the crate, `rotate-metrics-token` and `issue-registration-token` would
load it, open the database and **do real work** before the test failed. It
fails loudly rather than passing wrongly, so it is not a correctness hole and
**I am not asking you to change it in this round.**

## 6 — what this closes

The two closure prerequisites item 1 reopens: every dispatched subcommand
appears in `--help`, and the lists cannot drift again without a test failing.
**It does not close the RFC** — that needs its own `Closure reviewed on` and
`Closure approved by` record.

RFC 107 still **does not gate the `0.81.0` cut**. If this round is not back in
time, the cut goes without it and 107 moves to the next cycle, exactly as the
`ROADMAP.md` cycle D row says.
