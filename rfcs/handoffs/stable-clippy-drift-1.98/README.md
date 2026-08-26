# Stable clippy drift — rustc 1.98.0

**Tracks.** Unblocking `main`. Not an RFC — a toolchain-drift repair.
**Owner.** Implementation.
**Blocks.** Everything. G07 and G07b are blocking lanes; `main` is red.
**Baseline.** `10aa353`.

## What happened

Nothing in this repository changed. **Stable rustc moved 1.97.1 → 1.98.0**
(released 2026-08-18) and two lints now fire that did not before.

| | |
|---|---|
| Last green CI | run `32081043981` on `0183fc5`, G07 stable = **1.97.1** |
| First red CI | run `32934932617` on `98b02ce`, G07 stable = **1.98.0** |

G05/G06/G07 resolve `stable` at run time precisely so this surfaces. RFC 093
Requirements item 3: *"Stable-only lint drift may be fixed without raising MSRV;
it may not be silenced globally or converted to an allowlist merely to preserve a
date."* This is the gate working.

**There are two distinct findings, not one.** An early read of the log shows only
the first.

## Finding 1 — 13 redundant glob imports in `sui-id-web`

```
error: unused import: `super::super::common::*`
```

Thirteen files under `pages/settings/` and `pages/me_security/`.

**They are genuinely redundant, and I checked rather than assumed** — the last
"empty-looking" thing in this repo turned out to be load-bearing. Twelve of the
thirteen *do* use a `common` export (`Flash`, `empty_state`, `table_empty_row`
and friends). They reach it through `use super::*`: `pages/settings.rs:3` already
carries `use super::common::*`, and a child module's `use super::*` picks that up.
So both imports supply the same names and 1.98.0 now notices.

**Fix:** delete the line `use super::super::common::*;` from each of the 13 files.
Nothing else. Do not remove `use super::*`, which is what actually provides the
names.

## Finding 2 — `result_large_err` in `sui-id/src/http/handlers.rs:623`

```
error: the `Err`-variant returned from this function is very large
       the `Err`-variant is at least 128 bytes
```

`require_fresh_step_up` returns `Result<(), axum::response::Response>`. **This is
deliberate and already documented** in the function's own doc comment: the `Err`
is not an error, it is an alternative response — a redirect to step-up — and `?`
is deliberately not used. There are **18 call sites**.

Two options:

**(a) Targeted allow — recommended.**

```rust
// clippy::result_large_err: the Err variant is an axum Response by design --
// see the doc comment above. Boxing it would add an allocation on the
// redirect path and churn 18 call sites to satisfy a stack-size lint about
// a Result that is consumed immediately.
#[allow(clippy::result_large_err)]
pub async fn require_fresh_step_up(
```

**(b) Box the error:** `Result<(), Box<axum::response::Response>>`, updating 18
call sites.

I recommend (a). RFC 093 forbids silencing drift *globally* or via an allowlist
*to preserve a date*; a single targeted allow with a written reason, on a pattern
the code already documents as intentional, is neither. **If you disagree, say so
rather than following the recommendation** — the distinction between "considered
judgment" and "convenient silence" is exactly the sort of thing worth arguing
about, and (b) is defensible.

## Verified, not predicted

I applied both fixes in a throwaway worktree at `10aa353` and ran the gate
commands verbatim:

| Check | Result |
|---|---|
| `clippy --workspace --all-targets --locked -- -D warnings` (G07b) | **clean** |
| `clippy --workspace --all-targets --all-features --locked -- -D warnings` (G07) | **clean** |
| `fmt --all -- --check` (G08) | clean |
| `build --workspace --all-targets --locked` | succeeds |
| Footprint | **14 files, +5 / −13** |

The remaining `proc-macro-error2` future-incompat warning is pre-existing, is not
an error, and is not part of this.

## One trap, because it cost me twenty minutes

**`cargo clippy` does not re-emit diagnostics for crates it did not rebuild.** My
second local run reported zero errors and I briefly believed the drift had
resolved itself. It had not — the crate was cached.

To reproduce honestly:

```bash
cargo clean -p sui-id-web
cargo +stable clippy -p sui-id-web --all-targets --locked
```

Do not report a green local run without a clean rebuild of the affected crate.

## After this lands

`main` goes green, and the next item is a **patch release**. The rustls crypto
provider fix (`e6f9aea`, `aebfd36`) has been unreleased since 2026-07-28 — it
fixes a real startup panic on any LDAPS or implicit-TLS SMTP connection, and it
does not depend on anything the RFC 094/096 review blockage affects.
