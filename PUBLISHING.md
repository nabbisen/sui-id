# Publishing to crates.io

This document captures the order and the commands used to publish sui-id's
crates. It exists for the maintainers' benefit; users do not need it.

## Crate dependency graph

```
sui-id-shared   (no internal deps)
   │
   ├── sui-id-store
   │      │
   │      └── sui-id-core
   │             │
   │             └── sui-id ──── sui-id-web
   │
   └── sui-id-web
```

`sui-id` (the binary crate) is what users install with `cargo install sui-id`.
The four `sui-id-*` library crates are implementation detail; they are
published because the binary depends on them, not because they are intended
as a public library API.

## Publication order

Publish strictly bottom-up. Each `cargo publish` step both uploads the
crate *and* updates the local crates.io index, so the next step can find
its dependency.

```bash
# 1. Foundation: shared types (no internal deps)
cargo publish -p sui-id-shared

# 2. Storage: depends on sui-id-shared
cargo publish -p sui-id-store

# 3. Domain logic: depends on sui-id-shared, sui-id-store
cargo publish -p sui-id-core

# 4. UI: depends on sui-id-shared
cargo publish -p sui-id-web

# 5. Binary crate: depends on all four
cargo publish -p sui-id
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
5. The version field in the workspace `[workspace.package]` has been
   bumped, and the `path = "..."` dependencies in each crate have a matching
   `version = "..."` so the published crates pin a registry version, not
   just a path.
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
