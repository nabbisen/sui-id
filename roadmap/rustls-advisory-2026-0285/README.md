# RUSTSEC-2026-0285: move rustls off a vulnerable version

**Authorized by.** [`ROADMAP.md`](../../ROADMAP.md) §Non-RFC work packages.
Routine security-advisory maintenance, dispatched by the architect on 2026-09-16
and reported to the owner the same day.
**Implementer.** Mid-capability model. **Baseline.** `af26ac5` or later.
**State.** Complete, `536ffd5`, 2026-09-16.
**Priority.** Ahead of the other open packages. Main fails the scheduled
`Security audit` workflow until this lands.

## The finding

The scheduled `Security audit` run at `9f6acdb` failed:

| | |
|---|---|
| Crate | `rustls` 0.23.41 |
| Advisory | [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285), 2026-09-14, severity 5.3 (medium) |
| Title | TLS 1.3 handshake messages incorrectly accepted across encryption level boundaries |
| Solution | `>=0.23.45` |
| Reached through | `reqwest` 0.13.4 (via `hyper-rustls`, `tokio-rustls`, `rustls-platform-verifier`) in `sui-id` and `sui-id-core`, plus a direct dependency in `sui-id` |

The push-triggered CI did not catch it because `.github/workflows/audit.yml` runs
on push only when a `Cargo.toml` or `Cargo.lock` changes.

`cargo update -p rustls --dry-run` at `af26ac5` resolves within the existing
requirements:

```
rustls         0.23.41  -> 0.23.45
rustls-webpki  0.103.13 -> 0.103.15
aws-lc-sys     0.42.0   -> 0.45.0
```

## Required

1. `cargo update -p rustls`, lockfile only. **No `Cargo.toml` change.** If the
   update needs one, stop and report.
2. **`aws-lc-sys` moves three minor versions.** It is a native build, so confirm
   all three:
   - the MSRV 1.95 build;
   - the latest-stable build;
   - the release build, with the toolchain CI uses.
3. **Confirm that `ldap3`'s rustls provider selection is unaffected.** RFC 093
   M1a fixed a provider-selection defect there, so check that the provider
   choice is unchanged.
4. **Run `cargo audit` locally.** It must report no vulnerabilities. The two
   allowed `unmaintained` warnings (`paste`, `proc-macro-error2`) are unchanged
   and out of scope.

## Evidence

- The `Cargo.lock` diff, limited to the three crates above plus anything they
  pull in; list each added or changed package.
- `cargo audit` output before and after.
- fmt, both clippy scopes, `cargo test --workspace` count before and after, MSRV
  1.95 build, and the e2e suite (LDAP and federation tests included).
- After push, the architect runs the `Security audit` workflow on the new
  commit.
