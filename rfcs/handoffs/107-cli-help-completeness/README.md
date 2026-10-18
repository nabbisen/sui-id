# `sui-id --help` names every subcommand the binary accepts

**Authorized by.** [RFC 107](../../accepted/107-cli-help-completeness.md),
**Accepted 2026-10-18**. That supersedes the 2026-09-16 authorization under
`ROADMAP.md` §Non-RFC work packages, which is the line this handoff was
written with and which is now historical record.
**Independent design review.** [`design-review-2026-10-14.md`](design-review-2026-10-14.md).
**Dispatched.** 2026-10-18 —
[`dispatch-one-list-per-level-2026-10-18.md`](dispatch-one-list-per-level-2026-10-18.md),
which carries the measurement at `1a49e95` and the order. Where it and this
file differ, the dispatch says so explicitly.
**Implementer.** Mid-capability model. **Baseline.** Read the tip with
`git log -1`; `9f6acdb` is the measurement point below, not the baseline to
build on.

## The defect

Measured by reading at `9f6acdb`. `print_help` (`crates/sui-id/src/cli.rs:508`)
omits **two** subactions:
- `admin rotate-metrics-token`;
- `admin issue-registration-token`.

> **Corrected 2026-10-18.** This list said *three* and began with `setup`.
> **`setup` is not omitted** — `cli.rs:680` documents it, and so did `9f6acdb`,
> the commit this handoff names as its own measurement point. The error was
> found by the independent design review
> ([`design-review-2026-10-14.md`](design-review-2026-10-14.md) §1), which
> checked the claim against both trees instead of taking it, and required the
> correction before acceptance. RFC 107's own Summary was corrected on
> 2026-10-15; **this file was missed and is corrected now.** Do not add a
> `setup` entry: it is already there.

Both are dispatched in `run_admin_subcommand`. That dispatcher's own
error message lists the admin subactions a third time. Three hand-written lists
drifted apart.

**The same drift breaks dev mode** (added 2026-09-16, RFC 098 dispatch 15,
measured live). `find_subcommand` (`crates/sui-id/src/main.rs:70-88`) skips a
flag's value only for `--config`, `--to` and `--from`. So the value after
`--dev-seed`, `--dev-bind`, `--dev-db`, `--dev-admin-password` or
`--dev-client-secret` is read as a subcommand, and startup fails with
`unknown subcommand "<value>"`. No value-taking dev flag works today. This is a
fourth hand-kept list of the same flags.

## Required

- **One list per level.** A single `const` names the top-level subcommands, and
  another names the admin subactions. The dispatchers' "unknown …" messages build
  from these consts. So does a test.
- **Help is complete.** `print_help` documents every entry, with usage and a
  one-line description, in the existing style. Add the **two** missing ones —
  see the correction above. Take their flags from each subcommand's own
  argument parsing, not from memory.
- **Binding test.** Every name in each const appears in the help text. Every
  dispatched arm is in its const, so an arm with no entry fails to compile or
  fails the test; choose the mechanism and say why.
- **Value-taking flags, one list.** Replace `FLAGS_WITH_VALUE` with a single
  source for every flag that takes a value: the top-level flags, the backup,
  restore and verify flags, and every `--dev-*` flag. The same source feeds the
  help text. A test runs `find_subcommand` over each value-taking flag followed
  by a value, and asserts that no subcommand is found. A second test starts dev
  mode with each `--dev-*` flag set and asserts it is honoured: `--dev-bind`
  binds the given address, `--dev-seed` reads the given file.
- **Docs.** `docs/src/getting-started/quick-start.md` (dispatch 15 states the
  flags are rejected today) and `docs/src/guides/operators.md:225-268` must match
  the fixed behaviour.
- **`verify-backup`'s help line** says it runs "without writing anything". It
  stages a temporary copy of the snapshot for the integrity check; say so, as
  `docs/src/guides/deployment.md` now does (backup move, 2026-09-17).
- **RFC 103's CLI has landed** (`b3ee7de`): `admin issue-recovery-link` is in the
  usage block, the descriptions and the dispatcher's unknown-subaction message,
  each hand-kept. `admin reset-mfa` landed earlier. The single const this package
  introduces must list **both**, and the binding test must cover them.
- **Coordinate with RFC 103.** It will add `admin issue-recovery-link`. Do not add
  that name here; this package gives RFC 103 the place to add it.

## Evidence

- `sui-id --help` output before and after.
- Mutation check: remove one help line, and show the test failing.
- fmt, both clippy scopes, `cargo test --workspace` count before and after, MSRV
  1.95.
- `docs/src/guides/upgrade.md` and `deployment.md`: confirm every subcommand they
  cite now appears in help.
