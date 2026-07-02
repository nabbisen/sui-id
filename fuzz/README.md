# Fuzz Targets

This is a separate `cargo-fuzz` workspace, intentionally kept outside the
main Cargo workspace so the main project stays on the stable MSRV.

Run targets from this directory, for example:

```bash
cargo fuzz run accept_language
```

Targets that depend on `sui-id-core` require the optional feature:

```bash
cargo fuzz run pkce_verify --features core-targets
```
