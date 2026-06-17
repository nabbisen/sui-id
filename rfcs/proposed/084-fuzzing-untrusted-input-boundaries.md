# RFC 084 — Fuzzing Strategy for Untrusted Input Boundaries

**Status.** Proposed
**Tracks.** Strategy theme 7 (audit gap G7). Category B
(infrastructure) / C (target growth).
**Touches.** New top-level `fuzz/` cargo-fuzz workspace
(excluded from the main workspace members), small `pub` exposure
adjustments in `sui-id-core`/`sui-id-shared`, `.github/workflows`
(scheduled job), release-archive exclusion list.

## Summary

Introduce `cargo-fuzz` (libFuzzer) with a small, high-value set
of fuzz targets over the parsers and validators that consume
hostile input *before* authentication: authorize-endpoint
parameter validation, PKCE verification, JWT compact-form
parsing/verification, typed-ID and locale parsing, and
callback/logout parameter handling. Fuzzing runs as a scheduled
CI job (and locally on demand), never on the PR critical path.

## Motivation

Strategy §5.8/§7: the Web API boundary faces malformed and
adversarial input; fuzzing is the designated technique for
parser robustness. v0.63.1 has zero fuzz targets. The codebase's
guarantees (`unsafe_code = forbid`) already exclude memory
unsafety, so the payoff here is *panic-freedom and logic
robustness*: a reachable panic in a pre-auth path is a remote
DoS primitive (and with `panic = "abort"` in release, a
process-killing one); surprising accepts in validators are
worse. Pure functions taking attacker bytes are cheap to fuzz
and exactly where the strategy says to spend.

## Background

Inventory of untrusted surfaces (pre-auth reachable):

| Surface | Function(s) | Notes |
|---|---|---|
| `/oauth2/authorize` query | `begin_authorization` param validation, `is_redirect_uri_registered`, scope policy | string-heavy, already proptest'd partially |
| `/oauth2/token` body | PKCE `verify_pkce`, grant param parsing, `RawRefreshToken::from_untrusted` | |
| Bearer/JWT | `verify_access_token` parsing stage (header/payload split, base64, JSON, claim shapes) | signature check needs a key: fuzz with a fixed test key |
| IDs in paths/forms | `FromStr` for all `ids.rs` newtypes | |
| Locale negotiation | Accept-Language parsing in `sui-id-i18n` | |
| Logout/callback params | post-logout redirect validation | |

JWKS *parsing* is sui-id-internal (we publish, not consume,
JWKS), so it is out — an example of trimming the strategy's
candidate list to the actual codebase.

## Target code areas

```
fuzz/
  Cargo.toml            # separate workspace; not a member of root
  fuzz_targets/
    authorize_params.rs # arbitrary query map -> begin_authorization validation path (in-memory db, fixed client fixture)
    pkce_verify.rs      # (method, verifier, challenge) arbitrary strings
    jwt_parse.rs        # arbitrary bytes -> verify_access_token w/ fixed key
    ids_fromstr.rs      # arbitrary strings -> every typed-ID FromStr
    accept_language.rs  # arbitrary header -> locale resolution
    logout_params.rs    # arbitrary params -> post-logout validation
```

Each target's invariant: **no panic**, plus target-specific
properties (e.g. `pkce_verify`: returns `Ok` only when the
S256(verifier) equals challenge — cross-checked in the harness;
`ids_fromstr`: `Ok(v)` ⇒ `v.to_string().parse() == Ok(v)`).
Corpora seeded from real request shapes; corpus committed under
`fuzz/corpus/` (small, curated).

## Security properties / invariants

- **P1.** No input reachable before authentication panics any
  fuzzed function.
- **P2.** Validators accept only inputs satisfying their
  documented predicate (differential check inside the harness
  where a reference predicate is expressible).
- **P3.** Round-trip coherence for parse/display pairs.

## Non-goals

- No fuzzing of the HTTP stack itself (Axum/hyper upstream), the
  SQL layer, or rendering.
- No OSS-Fuzz onboarding in this RFC (revisit if the project's
  visibility grows).
- Not a PR gate: fuzzing is time-unbounded by nature.

## Proposed design

(Above.) Toolchain: `cargo fuzz` requires nightly for
`-Zsanitizer`; pin a known-good nightly in `fuzz/rust-toolchain.toml`
so the stable MSRV of the main workspace is untouched. The
`fuzz/` directory is excluded from release archives (it is a
development artifact) — add to the §22.3 tar exclusion list.

## Data model impact / API impact

None / a handful of `pub` (or `#[doc(hidden)] pub`) re-exports so
targets can reach validators without enabling test cfgs.

## Testing strategy

- Each target ships with a smoke invocation in CI
  (`cargo fuzz run <t> -- -runs=1000`, scheduled weekly job) plus
  `cargo fuzz build` on PRs touching `fuzz/` only.
- Regression inputs from any future finding land as
  `fuzz/regressions/` and are replayed by a normal `#[test]` in
  the owning crate, so fixes are guarded on stable toolchain too.

## Migration strategy

None.

## Rollout plan

Suggested v0.68.0: land harness + the six targets with a 1-hour
initial local run each, file/fix anything found before tagging.
Thereafter: weekly scheduled runs, findings triaged as hotfixes
per project policy.

## Risks and mitigations

- *Risk:* nightly toolchain breakage. Mitigation: pinned nightly,
  updated deliberately; failures are scheduled-job-only.
- *Risk:* fuzz harness bit-rot as signatures evolve. Mitigation:
  `cargo fuzz build` in PR CI keeps targets compiling.
- *Risk:* corpus bloat in repo. Mitigation: curated seeds only;
  large corpora stay local/CI-cache.

## Acceptance criteria

- `fuzz/` builds; six targets run; scheduled workflow exists.
- Initial runs completed with all findings resolved or filed.
- Archive build excludes `fuzz/`; main workspace MSRV unchanged.

## Open questions

- `arbitrary`-derived structured inputs vs raw bytes for
  `authorize_params` (structured finds logic bugs faster; raw
  finds parser bugs) — likely both, decided by initial-run yield.
