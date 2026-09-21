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
  one-line description, in the existing style. Add the three missing ones. Take
  their flags from each subcommand's own argument parsing, not from memory.
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
