# sui-id-shared

[![crates.io](https://img.shields.io/crates/v/sui-id-shared?label=rust)](https://crates.io/crates/sui-id-shared)
[![Rust Documentation](https://docs.rs/sui-id-shared/badge.svg?version=latest)](https://docs.rs/sui-id-shared)
[![Dependency Status](https://deps.rs/crate/sui-id-shared/latest/status.svg)](https://deps.rs/crate/sui-id-shared)
[![License](https://img.shields.io/github/license/nabbisen/sui-id-shared)](https://github.com/nabbisen/sui-id-shared/blob/main/LICENSE)

Shared types and DTOs for sui-id workspace.

This crate is an implementation detail of sui-id and intentionally has a
narrow surface area: typed identifiers (`UserId`, `ClientId`, …), the
public-facing JSON API DTOs, and the `ApiError` envelope. It does not
contain domain logic, storage, or HTTP code.

You generally do not depend on this crate directly. Install the binary
instead:

```bash
cargo install sui-id
```
