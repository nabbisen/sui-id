# RFC 148 — handoff

**RFC.** [`../../accepted/148-one-command-system.md`](../../accepted/148-one-command-system.md)
**Status.** **Accepted 2026-11-15** ([design review record](design-review-2026-11-15.md)). Not yet dispatched: its build is scheduled in the ROADMAP's cycles N–P, authorized 2026-11-15.

## Before a dispatch

The RFC's §6 has three questions for the owner: the parser (`clap`, or one
written here); the aliases' lifetime; and whether an explicit `--config`
that is missing should prompt. The dispatch is written after those answers,
because the first changes the dependency set.

## Measured, 2026-11-15

On the `0.91.0` binary (`a0a6c1a`), from an empty directory: the outputs of
`sui-id`, `--help` (190 lines), `help`, `setup`, `admin` and `-h`. On the
source: the dispatch in `crates/sui-id/src/main.rs:80-117`, the `admin`
dispatch at `crates/sui-id/src/cli.rs:271-279`, and the documented service
unit's `ExecStart`.
