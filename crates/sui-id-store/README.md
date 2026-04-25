# sui-id-store

[![crates.io](https://img.shields.io/crates/v/sui-id-store?label=rust)](https://crates.io/crates/sui-id-store)
[![Rust Documentation](https://docs.rs/sui-id-store/badge.svg?version=latest)](https://docs.rs/sui-id-store)
[![Dependency Status](https://deps.rs/crate/sui-id-store/latest/status.svg)](https://deps.rs/crate/sui-id-store)
[![License](https://img.shields.io/github/license/nabbisen/sui-id-store)](https://github.com/nabbisen/sui-id-store/blob/main/LICENSE)

Persistence layer (SQLite + field-level encryption) for sui-id. Owns the
SQLite connection, runs schema migrations on startup, and exposes thin
repository functions for the domain layer in `sui-id-core`.

## Encryption model

Sensitive columns are sealed with XChaCha20-Poly1305 using a master key
kept *outside* the database. This avoids the heavy dependency tree of
SQLCipher while still preventing a stolen `.sqlite` file from yielding
plaintext refresh tokens or signing keys.

The master key never enters the database file; it is supplied externally
via the `SUI_ID_MASTER_KEY` environment variable or a separate key file.
