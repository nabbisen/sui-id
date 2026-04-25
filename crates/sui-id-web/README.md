# sui-id-web

[![crates.io](https://img.shields.io/crates/v/sui-id-web?label=rust)](https://crates.io/crates/sui-id-web)
[![Rust Documentation](https://docs.rs/sui-id-web/badge.svg?version=latest)](https://docs.rs/sui-id-web)
[![Dependency Status](https://deps.rs/crate/sui-id-web/latest/status.svg)](https://deps.rs/crate/sui-id-web)
[![License](https://img.shields.io/github/license/nabbisen/sui-id-web)](https://github.com/nabbisen/sui-id-web/blob/main/LICENSE)

Server-rendered admin and setup UI for sui-id built on Leptos 0.8 in SSR-only
mode. No WASM bundle is shipped — pages are rendered server-side and
ordinary HTML POSTs handle state changes.

This crate exposes one render function per page (setup, login, dashboard,
users, clients, audit, error). The HTTP wiring lives in the `sui-id` binary
crate.
