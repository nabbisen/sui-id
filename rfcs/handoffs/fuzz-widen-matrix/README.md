# Widen the fuzz matrix to all six targets

> **Complete — landed as `d5e5402`** (fix(fuzz): repair pkce_verify and widen the matrix to all six targets).
> All six targets build and run; `pkce_verify` compiles for the first time.
> Marked closed 2026-08-27. Retained as the record of what was asked and why;
> **no action remains.**

**Tracks.** RFC 084's stated target set. Not an RFC — repair plus coverage.
**Owner.** Implementation.
**Blocks.** Nothing. This is the only unblocked path to substantive product
findings while RFCs 094–100 wait for a reviewer.
**Baseline.** `acd36f0`.

## Where things stand

The harness ran hosted for the first time on 2026-08-26, run `32948354732`:

| Target | Result |
|---|---|
| `accept_language` | 100,000 runs, no crash. cov 558, corpus 70 — it genuinely explored |
| `ids_fromstr` | 100,000 runs, no crash |

The workflow matrix contains only those two. **The other four have never been
fuzzed by anyone** — not locally, not in CI, not once since RFC 084 created them
on 2026-07-01:

`authorize_params`, `pkce_verify`, `jwt_parse`, `logout_params`.

They are the more interesting four. All take attacker bytes before
authentication, and with `panic = "abort"` in release a panic in any of them is a
process kill an unauthenticated stranger can trigger. `jwt_parse` exercises the
same JWT surface as the unfixed federation defect, from the verifying side.

## Finding 1 — `pkce_verify` has never compiled

It is not merely unfuzzed. It does not build, and never has.

**1a. Two undeclared dependencies.**

```
error[E0432]: unresolved import `base64ct`   --> fuzz_targets/pkce_verify.rs:41
error[E0432]: unresolved import `sha2`       --> fuzz_targets/pkce_verify.rs:42
```

`fuzz/Cargo.toml` declares neither. The root workspace already pins both, so use
the same versions — `fuzz/` is a separate workspace and cannot use
`workspace = true`:

```toml
sha2                        = "0.11"
base64ct                    = { version = "1.6", features = ["alloc"] }
```

**Verified:** adding those two clears both E0432 errors.

**1b. A lifetime error behind them**, which only appears once 1a is fixed:

```
error: lifetime may not live long enough
  --> fuzz_targets/pkce_verify.rs:23:29
23 |     let to_str = |p: &[u8]| std::str::from_utf8(p).unwrap_or("");
```

The closure returns a `&str` borrowed from its argument, and closure inference
cannot express that higher-ranked relationship. A named function can:

```rust
fn to_str(p: &[u8]) -> &str {
    std::str::from_utf8(p).unwrap_or("")
}
```

I have **not** verified this second fix compiles — diagnosed only. Confirm before
relying on it, and if the real fix differs, say so.

**Why nobody noticed:** `pkce_verify` was never in the workflow matrix, and RFC
084's stated bit-rot guard — `cargo fuzz build` in PR CI — could never run,
because the workflow has never subscribed to `pull_request` and the repository
has no pull requests. Recorded in RFC 084's *Delivery mechanism changed on* note.

## Finding 2 — the other three build, and need one workflow change

`authorize_params`, `jwt_parse` and `logout_params` all compile today with
`--features core-targets`. Verified: `cargo +nightly check --bin <t> --features
core-targets` exits 0 for each.

They pull `sui-id-core`, which pulls openssl, so the runner needs `libssl-dev`
and the feature flag.

**In `.github/workflows/fuzz.yml`'s `fuzz-run` job:**

1. Add the four targets to the matrix.
2. Add a step before `Install cargo-fuzz`:

```yaml
      - name: Install libssl-dev (core-dependent fuzz targets)
        run: sudo apt-get update && sudo apt-get install -y libssl-dev
```

3. Pass the feature for the four that need it. The cleanest shape is a matrix
   `include:` carrying a per-target `features` value, so the two openssl-free
   targets keep building without it — do not add `--features core-targets`
   unconditionally, since that would make `accept_language` and `ids_fromstr`
   newly depend on openssl for no reason.

## Verify before opening a PR-shaped change

There is no PR gate here, so local verification is the only pre-dispatch check:

```bash
cd fuzz
cargo +nightly check --bins --features core-targets   # all six must compile
```

Then dispatch the workflow and confirm each target reports
`Done 100000 runs` rather than merely exiting 0. A fuzz job that exits 0 without
running is the vacuous pass this project keeps finding; the run above was checked
for `#100000 DONE` and coverage growth, not just a green tick.

## If a target crashes

That is the useful outcome, not a failure. The workflow's `failure()` step
uploads `fuzz/artifacts/<target>/`, which contains a reproducing input.

A crash is a **product defect** — a panic reachable from unauthenticated input.
Fixing it needs no RFC and ships as a corrective release under the ROADMAP's
revised rule (no new public surface). Report it with the artifact rather than
fixing it inside this change: the widening and any fix should be separate, so a
regression has an unambiguous cause.
