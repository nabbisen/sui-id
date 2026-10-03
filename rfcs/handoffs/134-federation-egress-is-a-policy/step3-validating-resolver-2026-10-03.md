# RFC 134 step 3 — D2: a resolver that validates, and returns exactly one address

**RFC status: Accepted** (`rfcs/accepted/134-federation-egress-is-a-policy.md`).

**Dispatched: step 3 only (D2).** Steps 4 and 6 are **not** dispatched. Steps 1
and 2 landed (`981b228`, `842b75b`); step 5 was answered by the architect.

**This is the largest piece of RFC 134** and the one whose tests are RFC 096's
resolver corpus.

## Facts I verified so you do not have to

Each of these cost me a measurement and would cost you one. **If any is wrong,
stop and report it** — that means my verification was wrong and I want to know.

**1. The trait, exactly** (`reqwest-0.13.4/src/dns/resolve.rs`):

```rust
pub type Addrs = Box<dyn Iterator<Item = SocketAddr> + Send>;
pub type Resolving = Pin<Box<dyn Future<Output = Result<Addrs, BoxError>> + Send>>;
pub trait Resolve: Send + Sync { fn resolve(&self, name: Name) -> Resolving; }
```

`Name` exposes only `as_str()`. **It carries no port** — see (2).

**2. Return `SocketAddr`s with port `0`.** This is not obvious and getting it
wrong produces connections to port 0. reqwest's own hickory resolver does
`SocketAddr::new(ip_addr, 0)` (`src/dns/hickory.rs:47`), and hyper-util then
fills the real port in: `set_port(&mut addr, port, dst.port().is_some())`, whose
body is `if explicit || addr.port() == 0 { addr.set_port(host_port) }`
(`hyper-util-0.1.20/src/client/legacy/connect/http.rs:548,993-997`). **A non-zero
port you invent would be kept.**

**3. `is_global()` is unstable, on 1.99.0, today.** `rustc +stable` rejects it
with `error[E0658]: use of unstable library feature 'ip'`. So **the vendored
table is not a stylistic preference — it is the only option on stable**, and it
happens to be what RFC 096's matrix demands anyway ("every IANA special row
**regardless of global flag**"). Do not reach for `is_global` and do not add a
crate to get it.

`is_loopback()`, `is_private()`, `is_link_local()`, `is_unspecified()`,
`is_multicast()`, `is_broadcast()`, `is_documentation()` **are** stable and may
be used — but they are a convenience, not the policy. The table is the policy.

**4. `tokio`'s `net` feature is on, but not because `sui-id` asks for it.** The
workspace `tokio` declares `macros, rt-multi-thread, signal, fs, sync, time` —
no `net`. It is reachable in `sui-id` only through unification, because
`sui-id-core` and `sui-id-store` each enable it. **If you use
`tokio::net::lookup_host`, declare `features = ["net"]` on `sui-id`'s own `tokio`
dependency**, so the build does not depend on a sibling crate's feature list
staying as it is.

## What to build

### 3a — The vendored prefix table

A table of denied prefixes, **vendored, not computed**, each row carrying the
registry and row it comes from. Source: IANA's *IPv4 Special-Purpose Address
Registry* and *IPv6 Special-Purpose Address Registry*, **every row**, plus the
matrix's explicit deny list: **NAT64 (both prefixes), 6to4, Teredo,
IPv4-mapped, IPv4-compatible, and anycast**.

**Do not transcribe this from memory, mine or yours.** Take it from the
registries, cite each row, and record the registry revision date in the file's
header so a later reader knows what it was current against. A prefix table that
nobody can trace to a source is the kind of artifact that quietly rots.

**Normalize before comparing.** An IPv4-mapped IPv6 address must be tested as
both — the matrix lists IPv4-mapped as a denial in its own right, so this is
belt and braces, but a future narrowing of that row must not silently open a
path.

### 3b — The resolver

Implement `reqwest::dns::Resolve` on an egress resolver that:

1. resolves the name through a base resolver;
2. **rejects an empty answer, or more than 8**;
3. normalizes every returned address and **rejects the whole answer if any one**
   fails the table — not "filters out the bad ones". A mixed public/private
   answer is a rebinding signal, not a list to pick from;
4. returns **exactly one** surviving address, with port `0`.

**Base resolver: yours to choose, with one constraint — do not add a crate.**
`tokio::net::lookup_host` (see fact 4) and the `GaiResolver` reqwest itself uses
are both already in the graph. State which you chose and why.

**Why validating here is the design, restated because it is the whole point:**
the connector dials what the resolver returned, so there is no window between
the check and the connection and no second answer to fall back to. **A pre-flight
lookup followed by a connect would be the defect this prevents.**

Wire it with `.dns_resolver(...)` in step 1's single constructor. **Nothing else
may construct a client** — G19 already enforces that, and already fails on
`resolve(`/`resolve_to_addrs(`, which would bypass this.

### 3c — Failure behaviour

A rejected answer fails the request. It is **not** a user error and its detail
does not reach the browser: log the name and the reason, and let the existing
`FetchDiscoveryError::Network` path handle the surface, since from the caller's
position the fetch failed. Do not add a new user-visible error class for it.

## Tests — the matrix's corpus is the specification

`rfcs/handoffs/096-upstream-oidc-federation/validation-matrix.md`, §"DNS, TLS,
HTTP, and JSON", names the corpus and it is not negotiable down:

> both edges of every vendored IANA/explicit prefix, ordinary public IPv4/IPv6
> peers, mixed public/private answers, rebinding, address-order changes,
> IPv4-mapped/compatible, IPv6 zone IDs, metadata-service addresses, both
> boundaries of both NAT64 prefixes with public/private/link-local/metadata
> embedded IPv4, 6to4, and Teredo.

**Both edges of every prefix** means first and last address in range, for every
row in the table. That is the bulk of the work and it is the point — an
off-by-one in a prefix boundary is exactly the defect a spot-check misses.

These are unit tests over the validation function; they need no network. A
live-resolution test is needed only to prove the resolver is actually wired into
the client — one is enough.

**The removal check per control**, as before: delete a table row, confirm the
test for that row fails.

## Return

Per-hunk SHA-256 against a stated baseline, the removal evidence, the registry
revision date you vendored against, and the full local gate set — **including
A3.2, G19 and G20**.
