# RFC 134 step 2 — D3: discovery is untrusted input

**RFC status: Accepted** (`rfcs/done/134-federation-egress-is-a-policy.md`),
so implementation is permitted under RFC 000.

**Dispatched: step 2 only (D3).** Steps 3, 4 and 6 are **not** dispatched. Step 1
landed in `981b228`; step 5 was answered by the architect and needs no work.

**This is the highest-value control in RFC 134.** The provider's `client_secret`
is POSTed to `discovery.token_endpoint`, and nothing currently checks that the
endpoint belongs to the provider.

## A correction to the RFC, before anything else

**RFC 134 D3 says this step carries "admin UI to maintain it". That is wrong, and
you should not build an admin UI.** I wrote it without checking how federation
providers are configured.

Measured: `crates/sui-id/src/runtime/startup.rs:303-333` seeds providers from
`cfg.federation_providers` on boot, creating a row only when the slug is absent.
There is **no** admin UI for federation providers — `FederationProvider` appears
nowhere in `crates/sui-id-web/src`, and `crates/sui-id/src/http/router.rs` has no
provider routes. The configuration surface is
`FederationProviderConfig` in `crates/sui-id/src/runtime/config.rs`.

So the origin set is **a config field, seeded at startup like every other
provider attribute**. I am treating this as a correction of a factual error about
the existing system rather than a change of intent — the requirement was
"administrator-maintainable", the config file is how an administrator maintains a
provider, and nothing is added to or removed from what must hold. **If it is read as a scope
change instead, RFC 134 returns to `proposed/` and this dispatch waits.** Flagged
here rather than resolved quietly.

## What to build

### 2a — The origin set

**Config:** a new field on `FederationProviderConfig`:

```rust
/// Canonical HTTPS origins this provider's discovery document may name,
/// e.g. ["https://accounts.google.com", "https://oauth2.googleapis.com"].
/// Omitted or empty means the issuer's origin alone.
#[serde(default)]
pub allowed_origins: Vec<String>,
```

`#[serde(default)]` keeps existing configs loading unchanged.

**Schema:** migration `0045_federation_provider_allowed_origins.sql`, adding
`allowed_origins TEXT NOT NULL DEFAULT ''` to `federation_provider` (the next
free number; `0044_forgot_password_requests.sql` is current).

**The default is the issuer's origin alone, and it is computed at validation
time, not stored.** An empty column means "issuer origin only". This is
deliberate:

- it needs no URL parsing inside a SQL migration;
- every existing row gets the **strictest** correct default rather than a
  permissive one;
- a provider that genuinely serves endpoints from a second origin must say so,
  which is the administrator stating a fact they know and we cannot infer.

**Cap the set at 8** (RFC 096's matrix, Origins row: *1–8 explicit canonical
origins including issuer*). Reject a longer list at config load with a clear
error, not at first sign-in.

### 2b — Validation, in one place, enforced by the type system

**Do not add a gate condition for this, and do not validate at each call site.**
Make an unvalidated endpoint impossible to use:

`fetch_discovery` (`crates/sui-id/src/http/handlers/federation.rs:105`) currently
returns `OidcDiscovery` with public `String` fields. Change it to return a
**`ValidatedDiscovery`** whose fields are private and reachable only through
accessors, and which **cannot be constructed except by the validating
constructor**. Then a future call site that forgets the check does not compile,
rather than passing a gate that greps for a pattern.

This is the same reasoning as D2's resolver placement: put the control where
bypassing it is impossible, not where a checker notices.

Validate, for each of the three endpoints the struct carries —
`authorization_endpoint`, `token_endpoint`, `userinfo_endpoint`:

1. it parses as an absolute URL;
2. its scheme is **`https`**;
3. its origin — scheme, host, **and port**, with default ports normalized — is in
   the provider's allowed set (or equals the issuer's origin when the set is
   empty).

Also validate the **issuer itself** is canonical HTTPS before the discovery fetch
is attempted; today `fetch_discovery` string-concatenates it
(`:106-109`) with no check.

**`jwks_uri` is deliberately not in scope** — `OidcDiscovery` does not carry it,
nothing fetches it, and adding it now would be speculative. When JWKS
verification arrives it inherits the same validation by construction, which is
the point of the newtype.

### 2c — Failure behaviour

A rejected endpoint is a **provider configuration failure, not a user error**.
Reject before any request is made, log with the provider slug and the offending
origin, emit the existing `AUDIT_SIGNIN_UPSTREAM_FAILURE` event, and redirect to
the same `fed_error` shape the other failures use. **Do not echo the offending
URL to the browser** — it is attacker-influenced content.

## Tests

- **The central one:** a discovery document whose `token_endpoint` names an
  origin outside the set must result in **no request being made to it**. Assert
  on the absence of the request, not merely that sign-in failed — a test that
  only checks the error can pass while the secret has already been sent.
- Each of the three endpoints rejected independently.
- An `http://` endpoint rejected even when its host is in the set.
- Port handled: `https://host:443` and `https://host` are the same origin;
  `https://host:8443` is not.
- The empty-set default accepts an endpoint on the issuer's origin and rejects
  one elsewhere.
- A 9-origin config fails at load with a clear message.
- **A compile-fail fixture** proving `ValidatedDiscovery` cannot be constructed
  outside its module — the `trybuild` pattern you used for the cookie feature in
  step 1, which worked well.
- The removal check per control, as before.

## Return

Per-hunk SHA-256 against a stated baseline, removal evidence per control, and the
full local gate set — **including A3.2**, and including G19 and G20.
