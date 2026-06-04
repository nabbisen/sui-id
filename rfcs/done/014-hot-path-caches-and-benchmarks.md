# RFC 014 — Hot-path caches and benchmark harness

**Status.** Exploratory
**Priority.** Medium. Performance work, predicated on benchmarks.
**Tracks.** v0.29.3 codebase review — medium-priority finding #6.
**Touches.** `sui-id-core` (cache layer alongside `cors`, `tokens`,
`oauth_token`), `sui-id-store` (cache invalidation hooks on
client and signing-key writes), new `benches/` directory at the
workspace root, `Cargo.toml` (criterion as a dev-dependency).

## Summary

Two hot paths in the request critical path read DB rows on every
call:

- `cors::origin_matches_any_redirect_uri` — at every
  `/oauth2/token` request, walks every registered client and
  parses its `redirect_uris` to compute a set of allowed
  origins.
- `tokens::verify_access_token` and `oauth_token::introspect` —
  load the published signing-keys list from the DB on every
  call to validate a JWT signature.

Both lookups are read-mostly (clients change rarely, signing-key
rotations are days-to-weeks apart), but currently neither is
cached. This RFC introduces an in-process cache for both, with
write-side invalidation when the underlying data changes, and
sets up a `criterion` benchmark harness so the impact can be
measured rather than asserted.

The cache and the benchmark harness ship together because each
is more useful with the other: the harness gives us numbers to
demonstrate the cache helped (or didn't), and the cache work is
the natural moment to introduce the harness so it has its first
real customer.

## Why now, and why not bigger

- "Now" because RFC 013 lands the foundation for measuring DB
  load and would benefit from caches reducing the load it has
  to mitigate.
- "Not bigger" because we don't need a full cache framework —
  two cached read paths covers the measured-hot cases, and we
  don't have evidence yet that more is necessary. Resist the
  urge to introduce a generic caching layer ahead of the data.

## Requirements

After this RFC ships:

1. A `criterion`-based benchmark harness exists, runs from
   `cargo bench`, and exercises at least the seven scenarios
   the codebase review names.
2. The redirect-URI origin lookup at the token endpoint is
   served from a cache that's rebuilt on client write.
3. The signing-key set used for JWT verification is served
   from a cache that's rebuilt on signing-key rotation.
4. Cache correctness — concretely, a write that should
   invalidate the cache always does — is covered by an
   integration test, not just a benchmark.
5. The default `cargo test` does not invoke the benchmark
   harness. Benchmarks live behind `cargo bench` so contributor
   workflow is unchanged.

## Design

### Cache shape

`arc-swap` is the Rust standard for read-mostly snapshot caches:
readers get a cheap `Arc<T>` clone, writers atomically replace
the snapshot. Recommend it for both caches.

```rust
// crates/sui-id-core/src/cache/redirect_origins.rs
pub struct RedirectOriginsCache {
    snapshot: ArcSwap<HashSet<Origin>>,
}

impl RedirectOriginsCache {
    pub fn rebuild_from(&self, db: &Database) -> StoreResult<()> {
        let origins = repos::clients::list_all(db)?
            .into_iter()
            .flat_map(|c| c.redirect_uris)
            .filter_map(|u| Origin::try_from_url(&u).ok())
            .collect();
        self.snapshot.store(Arc::new(origins));
        Ok(())
    }

    pub fn contains(&self, origin: &Origin) -> bool {
        self.snapshot.load().contains(origin)
    }
}
```

Same shape for the signing-key cache.

### Where the caches live

`AppState` gains two fields:

```rust
pub struct AppState {
    // … existing fields
    pub redirect_origins: Arc<RedirectOriginsCache>,
    pub jwks_cache:       Arc<JwksCache>,
}
```

Both are built at startup (after the DB is open) by calling
`rebuild_from(&db)`. Both are accessed via `Arc` so they cross
the request boundary cheaply.

### Invalidation (the hard part)

A read-mostly cache is correct only if every write that should
invalidate it actually does. The repo functions that mutate the
underlying tables become the invalidation hooks:

- `repos::clients::create / update / delete` →
  `state.redirect_origins.rebuild_from(&state.db)?`
- `repos::signing_keys::insert / mark_retired / delete` →
  `state.jwks_cache.rebuild_from(&state.db)?`

The repo layer doesn't know about `AppState`, so the
invalidation lives one level up: in `sui-id-core::admin::clients::*`
and `sui-id-core::signing_keys::*`. Every call site that mutates
already takes an `&AppState`-equivalent or can be threaded one;
the rebuild call is one line at the end of the mutation
function.

A rebuild on every mutation is overkill for a deployment with
a thousand clients but right-sized for sui-id's expected scale
(small deployments, small client lists). Optimisation if and
when it matters: a `clients::patch` style for incremental
update. Out of scope for this RFC.

### Failure mode of rebuild

If the rebuild's underlying DB read fails, the cache stays at
its previous snapshot. The mutation that triggered the rebuild
returns its `Err` to the caller. The cache continues to serve
the prior (now slightly stale) snapshot. This is the conservative
direction: a stale cache is safer than an empty one (an empty
redirect-origins cache would reject every CORS preflight).

The audit log records the mutation; if it succeeded but the
cache rebuild failed, the audit row's `result` is `ok` (the
write happened) and a separate operational log entry at warn
level captures the rebuild failure. Operators investigate via
log; the next successful mutation re-syncs the cache.

### Cache freshness across restart

Trivial: the cache is built at startup from the DB, so a
restart always starts with a fresh snapshot. There's no
persistent cache state to corrupt or to migrate.

## Benchmark harness

### Layout

```
benches/
  auth_flow.rs              # criterion entry; dispatches to scenarios
  scenarios/
    login_password_only.rs
    login_with_totp.rs
    exchange_code.rs
    exchange_refresh_rotation.rs
    verify_access_token.rs
    session_resolve_and_touch.rs
    token_cors_origin_lookup_many_clients.rs
```

`Cargo.toml` (workspace root):

```toml
[workspace.package]
# … existing

[workspace.dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
```

Per-crate `Cargo.toml` for `sui-id`:

```toml
[[bench]]
name = "auth_flow"
harness = false
```

### Scenarios

Each scenario is a `criterion_group!` with a small fixture
setup and one or more `bench_function` calls. The seven
scenarios from the codebase review map to the seven files
listed above.

The scenarios use `test_app()` from `tests/e2e/common.rs`
where possible (in-memory SQLite, no I/O). Where a scenario
needs a populated DB, the fixture creates the rows once
outside the timed region.

### Acceptance criteria for the cache work

The benchmark harness must show, for the two cached paths,
a measurable improvement at non-trivial cardinality:

- `token_cors_origin_lookup_many_clients` with 100 clients:
  cached path is at least 10× faster than uncached.
- `verify_access_token`: cached JWKS lookup is at least 2×
  faster than uncached at any cardinality.

These are not contracts in the test suite; they're acceptance
notes for the human reviewing the benchmark output. If they
don't materialise, the cache change isn't pulling its weight
and we should reconsider.

## Multiple implementation steps

1. **Benchmark harness only.** Land `benches/` with the seven
   scenarios but no caches yet. Establish a baseline. Useful
   on its own.
2. **JWKS cache.** Smaller surface (one cache, three
   invalidation sites). Validates the `ArcSwap` pattern.
3. **Redirect-origins cache.** Larger blast radius for
   invalidation hooks (every client mutation site). Land last.

Each step is independently shippable.

## Tests

- **Cache invalidation.** A new integration test in
  `tests/e2e/cache_invalidation.rs` drives the sequence:
  observe a client list, mutate a client, observe the new
  list — assert the cache reflects the mutation. Same shape
  for signing keys.
- **Cache cold start.** Restart the test fixture's
  `AppState`; assert the cache rebuilds and matches the DB.
- **Cache stale-on-rebuild-failure.** Inject a mock DB
  failure during rebuild; assert the cache continues to
  serve the prior snapshot and the mutation returns `Err`.
- **Existing test suite.** All existing tests must pass
  unchanged — caches are an internal optimisation, not an
  API change.

## Security considerations

- **Cache as a stale-data risk.** The two caches are
  authoritative for security decisions (CORS allowance,
  JWT signature validity). A stale cache could *over-allow*
  a CORS origin or *over-allow* a JWT signed by a now-retired
  key. Both are bounded:
  - Origins cache: stale only if a client was deleted or
    its `redirect_uris` shrunk. The window is "from
    deletion until the next mutation rebuilds." For the
    delete case, the audit log records the deletion; the
    cache eventually catches up.
  - JWKS cache: stale only if a key was just retired. JWT
    verification against a retired-but-not-deleted key
    *should* succeed during the grace window anyway (that's
    the whole point of the grace window). Deletion of a
    signing key also triggers a rebuild.
- **Cache as a DoS vector.** A malicious admin who triggers
  thousands of client mutations forces thousands of
  rebuilds. Rate limiting on the admin clients endpoint is
  already in place; this RFC doesn't introduce a new vector,
  it just makes mutations slightly more expensive.
- **Memory.** The redirect-origins cache size is bounded by
  client count. Even at 10,000 clients with multiple URIs
  each, the memory footprint is tens of KB. Negligible.

## Open questions

- **Time-based cache expiry?** Recommend no. The caches are
  invalidated on write, which is the only event that should
  invalidate them. Time-based expiry would just reintroduce
  the staleness window we're trying to close.
- **Should the cache live in `sui-id-store` instead of
  `sui-id-core`?** The cache reads from the store, but its
  consumers are core/handler-level. Recommend `sui-id-core`
  (or a small new `sui-id-cache` crate if it grows). Keeps
  `sui-id-store` focused on persistence.
- **Should benchmarks be wired into CI?** Recommend no for
  the v1 of this work. Benchmarks are noisy in CI runners;
  better to run them locally and capture the output as part
  of the RFC follow-up. CI gating on benchmark numbers
  would be a separate, larger conversation.
