# RFC 134 step 1 — the egress constructor, G19, and D5 Tier 2

**RFC status: Accepted** (`rfcs/done/134-federation-egress-is-a-policy.md`,
accepted 2026-10-03), so implementation is permitted under RFC 000.

**Dispatched: step 1 only** — D1, D4 and D5 Tier 2. **Steps 2, 3, 4 and 6 are not
dispatched**; do not start them. Step 5 is **answered below by the architect** and
needs no work from you. Every section of this file states its own status; nothing
here is "while you're in there".

**Why step 1 alone.** D4's gate is what makes every later control durable. It
should exist and be green before the controls it guards are written, so that
steps 2–4 land against a gate that already refuses the bypasses — not after.

## What to build

### 1a — One egress module, one constructor

Move federation's HTTP client out of `crates/sui-id/src/runtime/state.rs:94-99`
into a dedicated module. Today it is:

```rust
reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(10))
    .build()
```

Every setting below goes on that one builder. All are verified present in
`reqwest` 0.13.4 under this workspace's feature set — `rustls` → `__rustls` →
`__tls`, so the `__tls`-gated ones are available:

| Setting | Why |
|---|---|
| `.redirect(reqwest::redirect::Policy::none())` | a 3xx must fail, same origin or not |
| `.no_proxy()` | no ambient proxy |
| `.referer(false)` | no ambient referer |
| `.http1_only()` | HTTP/1.1 ALPN only |
| `.min_tls_version(tls::Version::TLS_1_2)` | TLS floor |
| `.max_tls_version(tls::Version::TLS_1_3)` | TLS ceiling |
| `.connect_timeout(3s)` / `.read_timeout(5s)` / `.timeout(10s)` | the three bounds |
| `.http1_allow_obsolete_multiline_headers_in_responses(false)` | **D5 Tier 2** |
| `.http1_ignore_invalid_headers_in_responses(false)` | **D5 Tier 2** |
| `.http1_allow_spaces_after_header_name_in_responses(false)` | **D5 Tier 2** |

**The last three are already the defaults. Set them anyway** — that is the whole
point. Calling them states the requirement in our source, so a future release
relaxing a default cannot reach us silently and nobody has to go verify what the
default was. **A comment saying "already the default" invites a later cleanup to
delete the call; write the comment so it says why the call must stay.**

**No cookie store:** the `cookies` feature is off, so there is nothing to
disable. Add a compile-time assertion that it is off
(`#[cfg(feature = "cookies")] compile_error!(...)` or equivalent) so enabling it
later for an unrelated reason cannot silently give federation a cookie jar.

### 1b — Remove the per-request timeout

`crates/sui-id/src/http/handlers/federation.rs:112` sets
`.timeout(Duration::from_secs(10))` on the discovery request.
`RequestBuilder::timeout` **overrides** the client's, so leaving it there makes
D1's bound dead code on that path — invisibly, because the two values agree
today. Remove it. The client policy is the only source of timeouts on this path.

### 1c — G19, the gate

A new gate, **G19**, failing on any of three conditions anywhere under the
federation path:

1. `reqwest::Client::builder()` outside the egress module;
2. **`resolve(` or `resolve_to_addrs(` anywhere, including inside the egress
   module.** reqwest documents that per-name overrides are applied *on top of* a
   custom `dns_resolver`, so one such call pins a hostname to a chosen address
   and step 3's resolver will never run for that name;
3. any per-request `.timeout(` on the federation path.

**Register it in all three places** in `contracts/gate-inputs.toml`, following
G18's shape exactly:

- the RFC-origin map (near line 69): `G19 = "134"`;
- the `[gates]` command (near line 97);
- the lane entry (near line 171), with **`paths = ["**"]`**.

**`paths = ["**"]` is deliberate.** The existing comment justifies always-on for
G10a–G18 as governance gates that are collectively cheap. G19 is a security gate,
and a path-scoped one could be skipped by a change that moves a violation into a
file outside its declared scope — which is precisely the evasion it exists to
catch. It is a text scan over a handful of files, so the cost argument holds too.

**On the check's honesty.** A textual scan is a **proxy**: it catches the call
spelled the obvious way and not one reached through an alias or a macro. **Say so
in the script's docstring**, the way `scripts/check-rfc-integrity.py` says filing
location is a proxy for subject. Do not describe G19 as proving the bypasses are
impossible; it proves they are not present as written. A gate that oversells
itself is worse than one that states its limit.

## Step 5 — answered, not dispatched

The RFC asked whether `reqwest`'s `tls_info` could evidence the certificate-chain
bound. **It cannot, and you do not need to investigate it.**

`TlsInfo::peer_certificate()` returns `Option<&[u8]>` — **the leaf certificate
only, not the chain** — so it cannot measure a 16-certificate / 128 KiB chain.

A `rustls` `ServerCertVerifier` does see the intermediates and could measure them
before delegating to the real verifier. **I am not asking for that**: it puts our
code in the certificate-verification path to enforce a byte bound, and the
downside of a mistake there is accepting a bad certificate, which is far worse
than the bound being absent. The row therefore belongs in **Tier 3** with the
other unreachable ones. I will record that in the RFC separately; **no work for
you here.**

## Tests

- Each setting gets a test that **fails when the setting is removed.** A test
  that still passes with the line deleted is not evidence of the line.
- Redirects: a server returning 302 to a second origin must produce an error, and
  the test must assert **no second request was made**, not merely that the call
  failed.
- G19: a negative test per condition — a fixture containing each of the three
  forbidden constructs, asserting the gate fails and names the file. Follow
  `scripts/tests/test_rfc_integrity.py`'s shape.
- The three Tier 2 toggles: assert the constructor sets them. Testing hyper's
  parsing behaviour is not required and is not what the control is.

## Return

The usual: per-hunk SHA-256 over unified-diff text against a stated baseline,
plus G19's own output and the full local gate set. **If any builder method does
not exist or is gated differently than stated above, stop and report it** rather
than substituting a near-equivalent — I verified them against the vendored
`reqwest` 0.13.4 source, and a discrepancy means my verification was wrong and I
want to know.
