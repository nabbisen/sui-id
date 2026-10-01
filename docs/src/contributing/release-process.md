# Publishing to crates.io

This document captures the order and the commands used to publish sui-id's
crates. It exists for the maintainers' benefit; users do not need it.

## Crate dependency graph

*Corrected 2026-08-26. The previous graph omitted `sui-id-i18n` entirely and
showed `sui-id-web` depending on `sui-id`, which is backwards. Derived below
from the `[dependencies]` sections directly.*

| Crate | Internal dependencies |
|---|---|
| `sui-id-shared` | *none* |
| `sui-id-i18n` | *none* |
| `sui-id-store` | `sui-id-shared` |
| `sui-id-core` | `sui-id-shared`, `sui-id-store`, `sui-id-i18n` |
| `sui-id-web` | `sui-id-shared`, `sui-id-store`, `sui-id-i18n` |
| `sui-id` | all five |

```
sui-id-shared ──┬── sui-id-store ──┬── sui-id-core ──┐
                │                  │                 │
sui-id-i18n ────┴──────────────────┴── sui-id-web ───┴── sui-id
```

`sui-id` (the binary crate) is what users install with `cargo install sui-id`.
The **five** `sui-id-*` library crates are implementation detail; they are
published because the binary depends on them, not because they are intended
as a public library API.

## Publication order

Publish strictly bottom-up. Each `cargo publish` step both uploads the
crate *and* updates the local crates.io index, so the next step can find
its dependency.

> **Corrected 2026-08-26.** This list previously had five steps and omitted
> `sui-id-i18n`, which `sui-id-core` and `sui-id-web` both depend on and which
> is published on crates.io. Following it literally **would have** published
> `sui-id-shared`, then failed at `sui-id-core`, leaving a partial release on
> the registry that can only be yanked, not withdrawn. Caught during the 0.77.0
> release, before anything was published. It also described `sui-id-web` as
> depending only on `sui-id-shared`.

```bash
# 1. Foundation: shared types (no internal deps)
cargo publish -p sui-id-shared

# 2. Foundation: i18n (no internal deps)
cargo publish -p sui-id-i18n

# 3. Storage: depends on sui-id-shared
cargo publish -p sui-id-store

# 4. Domain logic: depends on sui-id-shared, sui-id-store, sui-id-i18n
cargo publish -p sui-id-core

# 5. UI: depends on sui-id-shared, sui-id-store, sui-id-i18n
cargo publish -p sui-id-web

# 6. Binary crate: depends on all five
cargo publish -p sui-id
```

**Dry-run each step before the real one.** `cargo publish --dry-run -p <crate>`
catches a manifest or dependency-resolution problem while it is still free. A
publish cannot be undone — `cargo yank` marks a version unusable for new
dependents but does not remove it.

**Check what the registry actually has before starting, and again after
publishing (RFC 131 D7).** The published version is not necessarily the
newest tag: on 2026-08-26 the registry held **0.76.9** while the repository
carried signed tags through **0.76.12**, and that gap sat undetected for
three months — the checklist described it as a caution about *how to verify
the registry*, not as the open defect it was. **0.78.0 repeated it.**

```bash
python3.14 scripts/check-published-versions.py --root .
```

This is the detector, run as a required step — not on a schedule. A cron in
this repo already failed eight consecutive weeks unnoticed
(`.github/workflows/fuzz.yml:3-5`), and the publish gap exists *because
nobody was looking*; a mechanism that depends on someone looking cannot be
its own fix. It is not a `[gates]` lane either: it needs a live network call
to crates.io (with an explicit `User-Agent` — without one the API returns a
policy error for *every* crate, which reads as "not published" and is not),
and `[gates]` stays offline and deterministic.

Run it **before** starting (confirms the previous release is fully out, or
is recorded in `CHANGELOG.md` as abandoned) and **after** the publish loop
below (confirms this one reached the registry too). A version it finds
tagged-but-unpublished is either published late or recorded as abandoned in
`CHANGELOG.md` with a reason — never left looking unfinished.

After step 6, `cargo install sui-id` works for end users.

## Pre-publish checklist

RFC 131 D2/D4: a release cut is one of the three claims that requires
**Level B** — every gate in `contracts/gate-inputs.toml`'s `[gates]` table,
green on the exact commit being tagged. Obtain it by dispatching the CI
workflow manually (`workflow_dispatch`) on that commit and confirming the
`CI` run — not some other run on the same commit; check the job count
against `[gates]`, not just that *a* run reports success. RFC 130 D7 makes
`workflow_dispatch` reliable: it always runs the complete matrix, regardless
of what changed.

This document does not restate those commands — a restatement drifts from
the gate it imitates, which is exactly how a release checklist's bare
`cargo fmt` passed locally while `cargo +stable fmt --all -- --check` failed
in CI. `scripts/check-gate-inputs.sh` (A3.4) asserts that this file contains
no such restatement.

Level B covers compilation, tests and lints; it does not cover packaging.
Before tagging, in addition to Level B on the commit being tagged:

1. `cargo package -p sui-id-shared --allow-dirty` produces a package and the
   verify build succeeds (the others can only be verified end-to-end after
   `sui-id-shared` is on the index).
2. The version field in the workspace `[workspace.package]` has been bumped
   and `Cargo.lock` has been refreshed. Internal workspace crate dependencies
   are centralized in root `[workspace.dependencies]`.
3. `CHANGELOG.md` has an entry for the new version.
4. The git working tree is clean (no `--allow-dirty` for the actual publish).

## Yanking

If a published version turns out to be broken:

```bash
cargo yank sui-id --version 0.1.0
```

Run this for every crate in the affected version, in the *reverse* of the
publish order.

## Why path + version dual-spec

crates.io rejects packages whose dependencies use `path` only — the
registry has no way to resolve a local path. We carry both:

```toml
sui-id-shared = { version = "0.77.0", path = "crates/sui-id-shared" }
```

Inside the workspace, cargo prefers `path`; in a published package, cargo
strips the `path` and falls back to the `version` from the registry. This
is the canonical way to publish a multi-crate workspace.

**Pin the version to the current release. Do not write `version = "0"`.**
`^0` means `>=0.0.0, <1.0.0` — every 0.x ever published is a candidate, and
cargo will backtrack into ancient ones when a constraint conflicts.

Found during the 0.77.0 release: all five internal dependencies carried
`version = "0"`, and packaging `sui-id-core` resolved `sui-id-store` back to
**v0.2.0**, which wants `rusqlite ^0.32` → `libsqlite3-sys ^0.30`, while
`sui-id-core` needs `rusqlite 0.40.1` → `libsqlite3-sys 0.38.1`. Both link the
native `sqlite3` library and cargo permits only one:

```
package `libsqlite3-sys` links to the native library `sqlite3`,
but it conflicts with a previous package which links to `sqlite3`
```

A `--dry-run` caught it before anything was published. The looseness also lets
a *consumer* resolve mismatched internal crate versions, so this is not only a
packaging concern.

Bump these five specs with the workspace version at each release.
