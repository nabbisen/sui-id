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

**Check what the registry actually has before starting.** The published version
is not necessarily the newest tag: on 2026-08-26 the registry held **0.76.9**
while the repository carried signed tags through **0.76.12**, so three tagged
versions had never been published. Verify with an explicit User-Agent, which
crates.io requires — without one the API returns a policy error for *every*
crate, which reads as "not published" and is not:

```bash
curl -s -H "User-Agent: sui-id-release (you@example.com)" \
  https://crates.io/api/v1/crates/sui-id | jq -r .crate.max_version
```

After step 5, `cargo install sui-id` works for end users.

## Pre-publish checklist

Before tagging a release and running the steps above:

1. `cargo fmt --all -- --check` is clean.
2. `cargo clippy --workspace --all-targets -- -D warnings` is clean.
3. `cargo test --workspace` is green.
4. `cargo package -p sui-id-shared --allow-dirty` produces a package and the
   verify build succeeds (the others can only be verified end-to-end after
   `sui-id-shared` is on the index).
5. The version field in the workspace `[workspace.package]` has been bumped
   and `Cargo.lock` has been refreshed. Internal workspace crate dependencies
   are centralized in root `[workspace.dependencies]`.
6. `CHANGELOG.md` has an entry for the new version.
7. The git working tree is clean (no `--allow-dirty` for the actual publish).

## Yanking

If a published version turns out to be broken:

```bash
cargo yank --version 0.1.0 -p sui-id
```

Run this for every crate in the affected version, in the *reverse* of the
publish order.

## Why path + version dual-spec

crates.io rejects packages whose dependencies use `path` only — the
registry has no way to resolve a local path. We carry both:

```toml
sui-id-shared = { version = "0.1.0", path = "../sui-id-shared" }
```

Inside the workspace, cargo prefers `path`; in a published package, cargo
strips the `path` and falls back to the `version` from the registry. This
is the canonical way to publish a multi-crate workspace.
