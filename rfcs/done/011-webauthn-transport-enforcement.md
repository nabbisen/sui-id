# RFC 011 — Enforce WebAuthn transport (HTTPS or localhost-http) at the server

**Status.** Proposed
**Priority.** High. Spec compliance gap with security implication.
**Tracks.** v0.29.3 codebase review — high-priority finding #2.
**Touches.** `sui-id-core::webauthn` (the `build()` constructor),
plus a unit test alongside it.

## Summary

The development specification (sec. 18) requires that "WebAuthn is
HTTPS or `localhost` only." Today this requirement is held
implicitly by browsers (which refuse WebAuthn ceremonies on
non-secure contexts) and by deployment convention. The server
itself does not enforce it: `webauthn::build()` parses the issuer
URL and constructs an origin without rejecting `http://example.com`.

This RFC adds a tight scheme/host check at the one constructor
site so the invariant is server-enforced, not browser-enforced.
A misconfigured deployment fails fast and visibly at startup,
rather than producing a mysteriously broken passkey ceremony at
runtime.

## Why now

This is a small change with a large consistency win. sui-id's
posture across the rest of the codebase is to encode invariants
in the server, not to assume the client will catch them
(`redirect_uri` exact match, PKCE S256-only, `unsafe_code = forbid`,
constant-time comparisons, etc). WebAuthn transport is the one
spot where we currently delegate the check to the browser. The
fix takes a few lines and brings WebAuthn in line with the rest
of the codebase's stance.

## Design

### The check

```rust
// crates/sui-id-core/src/webauthn.rs
pub fn build(issuer: &str) -> CoreResult<Webauthn> {
    let parsed = Url::parse(issuer)
        .map_err(|_| CoreError::Internal("invalid issuer URL".into()))?;

    let scheme = parsed.scheme();
    let host = parsed
        .host_str()
        .ok_or(CoreError::Internal("issuer URL has no host".into()))?;

    // Server-enforced WebAuthn transport invariant.
    // Spec sec. 18: "WebAuthn は HTTPS または `localhost` 上でのみ動作".
    let localhost_ok = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if scheme != "https" && !(scheme == "http" && localhost_ok) {
        return Err(CoreError::Internal(format!(
            "WebAuthn requires https, or http on localhost; got {scheme}://{host}"
        )));
    }

    // (existing) build the Webauthn instance with the validated origin.
    WebauthnBuilder::new(rp_id_from(&parsed)?, &parsed)?
        .rp_name(/* … */)
        .build()
        .map_err(CoreError::from)
}
```

### Failure mode

`build()` is called from `AppState::new`. A failed check returns
`Err`, which propagates out of state initialisation and aborts
process start. The operator sees the error message at process
start, not at first WebAuthn ceremony. This matches sui-id's
fail-loud-at-startup posture for other config invariants
(invalid `redirect_uri`, missing master key, unparseable TOML).

### Loopback host coverage

The accepted localhost variants are `localhost`, `127.0.0.1`,
`::1`. These are the three forms WebAuthn implementations
universally treat as secure context. We deliberately do **not**
expand this to "any private RFC 1918 range" or "any host
matching `*.localhost`" — neither is treated as a secure context
by browsers, so accepting them at the server level would create
a configuration that passes startup but fails at the browser.

### Interaction with `dev mode`

`--dev` defaults to binding `127.0.0.1:8801` and uses
`http://localhost:8801` as the issuer. This passes the new check
unchanged. Dev-mode operators who override the bind address with
something non-loopback already have to confirm that interactively
(see spec sec. 11.13); this RFC adds a second, hard gate: even
with the interactive confirmation, `--dev` on `0.0.0.0:8801` over
plain HTTP still fails the WebAuthn transport check at startup.
That's correct — passkey ceremonies wouldn't have worked there
anyway.

## Tests

A new unit test in `crates/sui-id-core/src/webauthn.rs`'s `tests`
module:

```rust
#[test]
fn build_rejects_plain_http_on_non_loopback() {
    let err = build("http://example.com").expect_err("must reject");
    let msg = format!("{err}");
    assert!(msg.contains("https"), "error message mentions https: {msg}");
}

#[test]
fn build_accepts_https() {
    build("https://idp.example.com").expect("https accepted");
}

#[test]
fn build_accepts_http_on_localhost_variants() {
    for host in ["localhost", "127.0.0.1", "[::1]"] {
        let url = format!("http://{host}:8801");
        build(&url).unwrap_or_else(|e| panic!("must accept {url}: {e}"));
    }
}

#[test]
fn build_rejects_http_on_non_loopback_with_port() {
    build("http://example.com:8801").expect_err("must reject");
    build("http://192.168.1.10:8801").expect_err("must reject");
}

#[test]
fn build_rejects_unparseable_url() {
    build("not a url").expect_err("must reject");
    build("https://").expect_err("must reject"); // no host
}
```

These tests run as part of `cargo test -p sui-id-core` and need
no extra fixtures.

## Security considerations

- **Defence in depth, not the only defence.** Browsers already
  refuse WebAuthn on non-secure contexts. The server check is a
  belt-and-braces invariant — it catches misconfigurations
  before they can surface as user-visible breakage, and it makes
  the assertion legible in code.
- **Loopback list correctness.** `127.0.0.1` and `::1` match the
  IPv4 and IPv6 loopback address respectively; `localhost`
  matches the conventional name. None of these resolve outside
  the local host. The check does not rely on DNS resolution
  (which would be a TOCTOU concern); it pattern-matches the host
  string directly.
- **No bypass via redirect.** WebAuthn origin validation happens
  during ceremony verification, with the origin baked into the
  `Webauthn` instance built at startup. There is no per-request
  override.
- **Consistency with existing redirect-URI policy.** sui-id
  already validates `redirect_uri` schemes (HTTPS or loopback HTTP)
  for OAuth clients in `admin::clients::validate_redirect_uri`.
  This RFC applies the same shape of check to the WebAuthn issuer
  URL. The validation logic could be factored into a shared
  helper (`net::is_secure_context_url`) — recommended but not
  required for this RFC; a follow-up DRY pass.

## Open questions

- **Should the validation helper be shared with the redirect-URI
  validator?** Recommend yes, as a small follow-up. Not in the
  scope of this RFC.
- **Should we honour an explicit override
  (`security.allow_insecure_webauthn = true`) for sandbox /
  testing scenarios?** Recommend no. The browser will refuse
  anyway, so the override would only help testing, and tests
  already use loopback. Don't ship the foot-gun.
