# `sui-id --help` names every subcommand the binary accepts

**Authorized by.** [`ROADMAP.md`](../../ROADMAP.md) §Non-RFC work packages. Owner
authorization, 2026-09-16.
**Implementer.** Mid-capability model. **Baseline.** `9f6acdb` or later.

## The defect

Measured by reading at `9f6acdb`. `print_help` (`crates/sui-id/src/cli.rs:508`)
omits three subcommands:
- `setup`, dispatched in `crates/sui-id/src/main.rs`;
- `admin rotate-metrics-token`;
- `admin issue-registration-token`.

The last two are dispatched in `run_admin_subcommand`. That dispatcher's own
error message lists the admin subactions a third time. Three hand-written lists
drifted apart.

## Required

- **One list per level.** A single `const` names the top-level subcommands, and
  another names the admin subactions. The dispatchers' "unknown …" messages build
  from these consts. So does a test.
- **Help is complete.** `print_help` documents every entry, with usage and a
  one-line description, in the existing style. Add the three missing ones. Take
  their flags from each subcommand's own argument parsing, not from memory.
- **Binding test.** Every name in each const appears in the help text. Every
  dispatched arm is in its const, so an arm with no entry fails to compile or
  fails the test; choose the mechanism and say why.
- **Coordinate with RFC 103.** It will add `admin issue-recovery-link`. Do not add
  that name here; this package gives RFC 103 the place to add it.

## Evidence

- `sui-id --help` output before and after.
- Mutation check: remove one help line, and show the test failing.
- fmt, both clippy scopes, `cargo test --workspace` count before and after, MSRV
  1.95.
- `docs/src/guides/upgrade.md` and `deployment.md`: confirm every subcommand they
  cite now appears in help.
