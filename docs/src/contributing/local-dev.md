# Local development

## Prerequisites

- Rust 1.91 (the workspace sets `rust-version = "1.91"`)
- `libssl-dev` (for TLS in the HTTP client used by HIBP checks)

Install Rust via [rustup](https://rustup.rs/):

```bash
rustup install 1.91
rustup default 1.91
```

## Build

```bash
cargo build
```

The workspace builds all five crates:
`sui-id-shared`, `sui-id-store`, `sui-id-i18n`, `sui-id-core`, `sui-id-web`, `sui-id`.

## Run in dev mode

```bash
cargo run -- --dev
```

Opens at `http://127.0.0.1:8801`. The admin panel is accessible immediately
with the dev seed credentials printed at startup.

## Tests

Run all unit tests:

```bash
cargo test --workspace --lib
```

Run end-to-end tests (slower — spawns a real Axum server):

```bash
CARGO_BUILD_JOBS=1 cargo test --test e2e
```

> **Note:** e2e tests require `CARGO_BUILD_JOBS=1` in memory-constrained
> environments (the default linker uses more RAM than the test runner).

Run a single test module:

```bash
cargo test -p sui-id-core --lib password
cargo test -p sui-id-store --lib audit
```

## Static analysis

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

## Documentation

Build the mdbook documentation:

```bash
cargo install mdbook
cd docs
mdbook build
mdbook serve   # serves at http://localhost:3000
```

## Project layout

```
crates/
├── sui-id-shared   # DTOs, typed IDs, public error types
├── sui-id-i18n     # Locale enum, Strings struct, per-locale constants
├── sui-id-store    # SQLite, migrations, column encryption, repositories
├── sui-id-core     # Domain logic (no HTTP, no UI)
├── sui-id-web      # Leptos SSR pages and components
└── sui-id          # Binary: Axum router, config, startup, asset serving
docs/               # mdbook source
rfcs/               # RFC documents (proposed/, done/)
static/             # Embedded static assets (favicon, JS for WebAuthn)
```

## Adding a new admin page

1. Define a `FooData` struct in `crates/sui-id-web/src/pages.rs`.
2. Write `pub fn render_foo(data: FooData, dev_mode: bool, lang: Locale) -> String`.
3. Export in `crates/sui-id-web/src/lib.rs`.
4. Add a route in `crates/sui-id/src/router.rs`.
5. Write the handler in `crates/sui-id/src/handlers/admin.rs`.
6. Use `resolve_admin_locale(&app, admin_id).await` for locale resolution.

## RFC process

Significant changes follow the RFC workflow:

1. Create `rfcs/proposed/<number>-<slug>.md` describing the problem, approach,
   and test plan.
2. Implement.
3. Move the file to `rfcs/done/` when the code lands.

RFC numbers are sequential. Browse `rfcs/done/` for examples of the format.
