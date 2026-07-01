# RFC 087 — Clippy and Rustfmt Baseline Cleanup (Rust 1.96)

**Status.** Proposed
**Tracks.** Toolchain hygiene / technical debt. Category D.
**Touches.** `crates/sui-id-web/src/` (pages and components),
`crates/sui-id-shared/src/secrets.rs`. No behaviour change.

## Summary

Resolve the gap between the Rust 1.96 stable toolchain (the
current CI-signal toolchain) and the committed codebase. Two
defect classes were surfaced during the v0.65.0 session:

1. **Clippy (16 pre-existing errors)** — `clippy --workspace
   -- -D warnings` fails on 16 sites across `sui-id-web` (pages
   and components) and one site in `sui-id-shared`, all from
   lints newly promoted or tightened in recent stable releases.
2. **Rustfmt (1 pre-existing diff)** — `cargo fmt --check` flags
   `crates/sui-id-web/src/components/badges.rs` for aligned match
   arms; no `rustfmt.toml` exists, so the committed code is not
   default-fmt-clean.

Neither class was introduced by v0.65.0. Both classes represent
accumulated toolchain drift that will make the CI-signal build
unreliable and should be resolved before the next RFC-gated
development arc begins.

## Motivation

CI uses `dtolnay/rust-toolchain@stable`, which tracks latest
stable and was pinned to 1.96 at v0.65.0 release time. The
`[workspace.lints]` table promotes several clippy lints to
`"warn"`, which `-D warnings` escalates to errors.  The current
tree fails 17 of those escalated sites in two crates, and has
one rustfmt diff. The "0 warnings" invariant the project targets
does not hold under the current toolchain without a baseline fix.

Leaving this unfixed means:
- CI warnings accumulate silently and hide real regressions.
- Contributors running `cargo clippy -- -D warnings` locally see
  failure on untouched code, which erodes trust in the gate.
- The next RFC implementation session starts on a red baseline.

## Background

The 17 sites and their lint categories, as surfaced during v0.65.0:

**`sui-id-web` (16 errors, clippy `--no-deps`):**

| File | Lint |
|---|---|
| `pages/me_security/sessions.rs:65` | `no_effect_replace` |
| `pages/me_security/sessions.rs:39` | (same) |
| `pages/auth.rs:86`, `:195`, `:315` | `unnecessary_map_or` / `unit_arg` |
| `pages/confirm.rs:25`, `:34`, `:83` | `unit_arg` / `unnecessary_map_or` |
| `pages/dashboard.rs:76`, `:94` | `empty_line_after_doc_comments` |
| `pages/setup.rs:162`, `:202`, `:256` | `empty_line_after_doc_comments` |
| `pages/settings.rs:65` | `no_effect_replace` |
| `components/chrome.rs:6`, `:7` | `doc_lazy_continuation` |

**`sui-id-shared` (1 error):**

| File | Lint |
|---|---|
| `src/secrets.rs:220` | `expect_used` (`expect("base64url is ascii")`) |

**Rustfmt (1 diff):**

| File | Issue |
|---|---|
| `components/badges.rs` | Aligned match arms; not default-fmt-clean |

## Target code areas

- `crates/sui-id-web/src/pages/me_security/sessions.rs`
- `crates/sui-id-web/src/pages/auth.rs`
- `crates/sui-id-web/src/pages/confirm.rs`
- `crates/sui-id-web/src/pages/dashboard.rs`
- `crates/sui-id-web/src/pages/setup.rs`
- `crates/sui-id-web/src/pages/settings.rs`
- `crates/sui-id-web/src/components/chrome.rs`
- `crates/sui-id-web/src/components/badges.rs`
- `crates/sui-id-shared/src/secrets.rs`

## Security properties / invariants

None. This RFC contains no logic changes.

However, `secrets.rs:220` requires care: it is inside
`random_base64url`, which generates token entropy via
`getrandom`. The `expect("base64url is ascii")` call is on
`String::from_utf8(out)` where `out` is already base64url-encoded
bytes — genuinely infallible. The fix options are:

- **Option A (preferred):** Replace with a safety comment +
  `from_utf8_unchecked` wrapped in a `#[allow(unsafe_code)]`
  block with explicit justification — base64url output is
  guaranteed ASCII; the safety argument is trivially checkable.
  The workspace `[workspace.lints.rust]` has `unsafe_code =
  "forbid"`, so this requires a targeted `#[allow]` *and* a
  note that the workspace forbid covers production paths, not
  this exception.
- **Option B:** Replace with `.unwrap_or_else(|_| unreachable!()`
  and add `#[allow(clippy::expect_used)]` locally — cleaner if
  the workspace lints evolve, but relies on the `unreachable!`
  assumption being right.
- **Option C:** Use `String::from_utf8(out).map_err(|_|
  unreachable!())?` and bubble an error — forces a signature
  change up the call stack, disproportionate to the situation.

Option B is safest given the workspace lint config (avoids the
`unsafe_code` forbid entirely). The implementer should verify
which option is appropriate at the call site.

## Non-goals

- No logic changes anywhere.
- No API shape changes.
- No new features or token additions.
- No changes to the CI gate scripts themselves (the gates
  already correctly target the lints; this RFC just clears the
  findings).
- No introduction of a `rustfmt.toml` to alter project-wide
  formatting conventions — the fix for `badges.rs` is to reformat
  to default style, not to legitimise the deviation.

## Proposed design

Address each site minimally:

- **`no_effect_replace`** — remove the no-op `.replace('\'', "\'")`
  calls (replacing a character with itself has no effect; the lint
  name describes it exactly).
- **`unnecessary_map_or`** — rewrite `foo.map_or(_, _)` patterns
  to the simpler form clippy suggests (`foo.is_some_and(…)` or
  similar). The exact suggestion appears in the clippy output.
- **`unit_arg`** — rewrite chains that pass `()` as an argument
  to match the clippy suggestion.
- **`empty_line_after_doc_comments`** — remove the blank line
  between `///` doc comment blocks and the items they document.
- **`doc_lazy_continuation`** — fix the doc comment continuation
  lines flagged in `chrome.rs` (usually a missing leading `///`
  or a stray blank line inside a doc block).
- **`expect_used` in `secrets.rs`** — use Option B above (local
  `#[allow]` + `unreachable!`) unless the security review at the
  call site motivates otherwise.
- **`badges.rs` rustfmt** — run `rustfmt --edition 2024` on the
  file and commit the result.

## Data model impact

None.

## API impact

None.

## Testing strategy

The existing test suite is the acceptance signal. No new tests
are required — the lints are style and trivial-logic issues, not
logic changes. Passing gates:

- `cargo clippy --workspace -- -D warnings` → 0 errors on all
  crates (including `sui-id-shared`; all clippy-buildable crates
  must be clean, even though the main binary OOMs the linker).
- `cargo fmt --check --all` → clean diff on all files.
- `cargo test -p sui-id-store -p sui-id-shared -p sui-id-i18n -p
  sui-id-web` → all tests green (unchanged from v0.65.0 baseline).
- All four CI gates unchanged: `text-leaks` = 0, `css-tokens`
  resolves, `semantic-palette-parity` = 36, `inline-style-bound`
  ≤ 20.

## Migration strategy

Not applicable (no data or API changes).

## Rollout plan

Ships as a standalone patch or minor release immediately before
the next RFC implementation arc (suggested v0.65.1 or v0.66.0
depending on session timing). Because the fix is pure cleanup,
it can also be absorbed into the first commit of the next
RFC session if sequencing is tighter to keep releases at logical
boundaries.

## Risks and mitigations

- *Risk:* a clippy fix in `secrets.rs` accidentally changes
  security-relevant logic.  
  Mitigated: the only change is the `expect` → `unwrap_or_else`
  replacement; the cryptographic path (`getrandom::fill`,
  `Base64UrlUnpadded::encode`) is untouched. Diff will be trivial
  to audit.
- *Risk:* rustfmt reformatting `badges.rs` breaks something.  
  Mitigated: `badges.rs` is a pure view-rendering file
  (`status_badge` and `kind_badge` render-only functions); there
  is no logic path to break. The test suite covers the crate.
- *Risk:* a lint suppression in `sui-id-web` pages masks a real
  future bug in the same file.  
  Mitigated: suppressions are not the approach here — each lint
  is fixed at the source (no-op replaced, doc comment fixed,
  etc.), not suppressed.

## Acceptance criteria

- `cargo clippy --workspace -- -D warnings` exits 0 on all
  crates the build environment can compile (all except the main
  binary, which OOMs the linker).
- `cargo fmt --check --all` exits 0 (no diff).
- Test suite green on all buildable crates.
- All four CI gate scripts pass unchanged.
- `secrets.rs:220` fix is reviewed and confirmed to not alter
  the entropy or encoding path.

## Open questions

- Whether to add a `rustfmt.toml` stabilising the project's
  intended style (e.g. `max_width`, `tab_spaces`). Decision
  deferred: the immediate task is to reach default-fmt-clean;
  a `rustfmt.toml` can come later if the team has specific style
  preferences diverging from defaults.
- Whether to suppress or fix the `sui-id-core` clippy findings
  (the core crate is not clippy-buildable in the CI environment
  due to openssl; findings there are invisible locally without
  libssl-dev). Recommendation: fix them at the same time, but
  verify locally with libssl-dev available. If they can't be
  verified in this environment, file a separate note.
