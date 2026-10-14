# RFC 107 — independent design review

**Reviewed.** 2026-10-14 JST, by the implementation role, under RFC 139's
dispatch §6. The architect authored RFC 107 and cannot supply this review;
this role did not author it and can.
**Scope.** Design only — is the problem real, is the fix the right shape,
does the closure prerequisite follow from the proposal. No code changed; RFC
107 is not implemented here.
**Verdict.** **The problem is real and the fix is the right shape. One
factual error in RFC 107's own Summary, found by measuring at its own cited
baseline rather than trusting the prose.**

## 1 — is the problem real

**Confirmed by reading source, not by repeating the handoff's own claims.**

`crates/sui-id/src/main.rs:90-108`, `find_subcommand`:

```rust
const FLAGS_WITH_VALUE: &[&str] = &["--config", "--to", "--from"];
```

Walking this by hand against `sui-id --dev-seed path.toml`: `--dev-seed`
starts with `-` and is not in `FLAGS_WITH_VALUE`, so the loop takes the
`else` branch (`i += 1`) instead of skipping its value. The next token,
`path.toml`, is the first non-flag token the loop sees, and `find_subcommand`
returns it as *the subcommand*. `main.rs:77` then bails with `unknown
subcommand "path.toml"`. Every `--dev-*` flag that takes a value
(`--dev-seed`, `--dev-bind`, `--dev-db`, `--dev-admin-password`,
`--dev-client-secret`, confirmed at `main.rs:208-229`) hits the same path.
**Independent corroboration, not just my own trace**: `docs/src/getting-started/quick-start.md`
already documents this exact failure as today's behavior — "every `--dev-*`
flag that takes a value … is rejected at startup with 'unknown subcommand'"
— in a document RFC 107 did not write and does not touch.

`crates/sui-id/src/cli.rs:100-111`, `run_admin_subcommand`: the dispatcher's
match arms include `rotate-metrics-token` and `issue-registration-token`
(`:107-108`), and its own `"unknown admin subaction"` error message
(`:109-111`) names both. `print_help` (`:614-733`) does not — its `USAGE`
block stops at `admin rotate-key`, `setup` and `--dev`, and its
`SUBCOMMANDS` section has no entry for either. Three lists (dispatcher match
arms, the error message, the help text), and the third has drifted from the
other two. Real.

`backup/ops.rs:234-268`, `run_verify`: stages a real temporary file
(`std::fs::write(&temp_db, &db_bytes)`, `:250`) to run `PRAGMA
integrity_check`, and removes it afterward. `cli.rs`'s own help text for
`verify-backup` says it runs "without writing anything" — contradicted by
the function it describes. `docs/src/guides/deployment.md:427-430` already
states the accurate version ("the only file it creates is a temporary copy
of the snapshot … It removes that copy once the check has run"), so the fix
is bringing the CLI's own help text into line with what the project already
says correctly elsewhere, not inventing new wording.

**The one claim that does not hold.** RFC 107's Summary: *"`print_help`
omits three subcommands the binary dispatches — `setup`, `admin
rotate-metrics-token` and `admin issue-registration-token`."* `setup` is not
omitted. `crates/sui-id/src/cli.rs:680-693` (current tree) lists it in full,
with its flags. Checked at the RFC's own stated baseline, not just the tip:
`git show 9f6acdb:crates/sui-id/src/cli.rs` — the commit the handoff names
as its measurement point — already has the `setup` help entry at that
revision's line 558. The claim was wrong when it was written, not only now.

This does not weaken the RFC — the other two missing admin subactions and
the error-message drift are independently sufficient to justify it — but it
should not ship into an Accepted RFC. It is also a small, sharp
demonstration of the RFC's own thesis: *a hand-kept enumeration drifts from
the thing it describes*, and here that happened to the RFC's own prose
before it happened to the code it was proposing to fix.

## 2 — is the fix the right shape

**A single `const` per level, feeding both the help text and the dispatch
error message, with a test binding the const to actual dispatch arms.** This
is not a new mechanism for this codebase — it is the same shape the project
already uses wherever a hand-kept enumeration has drifted before:
`contracts/write-commands.toml` plus `check-write-commands.py` (G17),
`contracts/gate-inputs.toml` plus its own checker, and — reviewed the same
day as this one — the F03 seam's proposed scope test in the shape of
`r094_m2a_db_access_scope.rs`. One source, a test or gate that reads it back
against reality, rather than a convention a future editor has to remember.
The closure prerequisite does not ask for anything this shape cannot give
it.

**"Value-taking flags, one list" is correctly scoped, and correctly does
not reach further than it needs to.** Measured which flags exist at all:
beyond `--config`/`--to`/`--from`, the binary also parses `--username`,
`--reason`, `--new-key`, `--admin-username`, `--admin-email`,
`--admin-display-name`, `--max-uses`, `--note` (admin subaction flags) and
the five `--dev-*` flags (top-level). The handoff's
own required list — "the top-level flags, the backup, restore and verify
flags, and every `--dev-*` flag" — does not mention the admin-subaction
flags, and it should not: `find_subcommand` returns on the *first* non-flag
token it sees, and every admin-subaction invocation (`sui-id admin
reset-mfa --username NAME`) has `admin` sitting there first. Those flags'
own values are consumed later, by the subaction's own argument parsing,
never by `find_subcommand`'s scan. Scoping the unified list to exactly the
flags that can appear *before* a subcommand token is found is the right cut,
not an oversight.

**"Binding test... choose the mechanism and say why" is appropriately
unprescriptive.** The handoff does not mandate a compile-time exhaustiveness
trick over a runtime test; either can satisfy "an arm with no entry fails to
compile or fails the test." Leaving that choice to the implementer, while
requiring the choice be justified, matches how this project treats
local-implementation decisions elsewhere
(`.git-exclude/roles/mid-capability-model-operating-instructions.md:274`'s
own allowance for "Small private helper structure").

## 3 — does the closure prerequisite follow from the proposal

RFC 107's closure prerequisite has three clauses; each maps onto a specific
"Required" item, and none of the "Required" items reaches past what the
prerequisite needs:

| Closure clause | Satisfied by |
|---|---|
| "Every subcommand the binary dispatches appears in `--help`" | "Help is complete" — add the three (now two, after §1's correction) missing entries, sourced from each subcommand's own argument parsing |
| "the lists cannot drift again without a test failing" | "One list per level" + "Binding test" — the dispatcher and the help text read from the same `const`, and a test asserts every dispatched arm has an entry |
| "a flag's value is never mistaken for a subcommand" | "Value-taking flags, one list" + the `find_subcommand`-over-every-flag test |

No gap, no unaddressed clause, no requirement that exceeds the prerequisite
it is there to satisfy.

## 4 — what to fix before acceptance

1. **Correct the Summary.** Replace "`print_help` omits three subcommands …
   `setup`, `admin rotate-metrics-token` and `admin issue-registration-token`"
   with the two that are actually missing, and either drop the `setup`
   example or replace it with a true one (none is needed — two is still
   worth an RFC, and the `find_subcommand`/dev-flag defect alone would
   justify this RFC on its own).
2. Nothing else found wrong. The handoff's remaining claims (`verify-backup`'s
   help text, RFC 103's CLI having already landed, the `--dev-*` flag list)
   were each checked against source and hold.

## 5 — one adjacent observation, recorded outside this repository

While verifying the flag lists I found one item adjacent to RFC 107's scope
but outside it. It is **not** a blocker for accepting RFC 107 and is not
needed to review this design. The architect has recorded it where findings of
its kind are recorded, pending `@nabbisen`'s own reading of it.

## 6 — not reviewed here

Implementation itself — this is a design review, and RFC 139 §6 is explicit
that this role does not implement RFC 107 or accept it. RFC 107's own
header already names who that belongs to.

---

**Result path:** `rfcs/handoffs/107-cli-help-completeness/design-review-2026-10-14.md`
