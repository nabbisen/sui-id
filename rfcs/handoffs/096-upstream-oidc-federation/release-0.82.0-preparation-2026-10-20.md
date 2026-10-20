# `0.82.0` — the release preparation

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. The cut is authorized; the tag and the
> publish are not dispatchable and stop with the architect.

**Dispatched.** 2026-10-20 JST, by the architect.
**Authorization.** 2026-10-20, on
[`release-0.82.0-scope-2026-10-20.md`](release-0.82.0-scope-2026-10-20.md) §B's
measured severity.
**Baseline.** Read the tip with `git log -1`.

## 0 — this dispatch is deliberately light, and that is the point

A standing constraint was recorded 2026-10-20: *"it will harm the project if
some check for the release gate only is processed at each RFC, each
implementation or each review."* This round is a **lockfile update**. It
changes no behaviour, so most of what a feature round is asked for would buy
nothing here. **No mutation evidence, no doc sweep, no per-element
verification is asked for below.** If you find yourself producing a long
package, something has gone wrong with the scope, not with your thoroughness.

## 1 — the update

```
cargo update
```

**Lockfile only. No manifest edit.** Confirm `Cargo.toml` is untouched by it.
Measured in the dry run already, so these are a cross-check rather than a
discovery:

| | |
|---|---|
| `rustls` | 0.23.45 → 0.23.46 |
| `quinn-proto` | 0.11.15 → 0.11.19 |
| `proc-macro-error2` | **removed** |

Cargo resolves to *"highest Rust 1.95 compatible versions"*, so the MSRV floor
should hold by construction — **confirm it rather than assume it**, since that
is the one thing an update can quietly break.

## 2 — the bump

`Cargo.toml:20` and the five `[workspace.dependencies]` specs, `0.81.0` →
`0.82.0`. Refresh `Cargo.lock` by running cargo, not by editing it. Same three
hand-edited places as last time; no test pins the version.

## 3 — `CHANGELOG.md`

Promote to `## [0.82.0] — <the date you do it>`, following whichever
`[Unreleased]` convention you confirmed at `0.81.0`.

**It has one entry, under `### Security`, and I am specifying its substance
because getting the severity right matters more than the prose:**

- **Reachable, and the reason to cut:** `rustls` 0.23.45 → 0.23.46 closes four
  advisories — **RUSTSEC-2026-0346** (Medium 4.0, `Debug` impls for client
  session values expose TLS secrets), **RUSTSEC-2026-0349** (Low, an
  unauthenticated ServerHello can downgrade later connections to classical key
  exchange), **RUSTSEC-2026-0348** (Low, brotli certificate decompression can
  allocate up to 1 GiB *before* authentication) and **RUSTSEC-2026-0347**
  (Low, buffered application data sent to the outer server after ECH
  rejection). `rustls` reaches this project through `reqwest`, a production
  dependency of `sui-id-core` — federation egress, the TLS client path.
- **Not reachable, and say so in the entry:** `quinn-proto` 0.11.15 → 0.11.19
  closes five more, two rated High 7.5. **They do not apply to this project.**
  `cargo tree -i quinn-proto` finds no path into the build on any target;
  `reqwest` declares `quinn` as an optional HTTP/3 dependency this workspace
  does not enable, and `cargo audit` reads `Cargo.lock`, which carries the
  union of optional dependencies, rather than the feature-resolved graph.
  **Write that distinction into the entry.** An operator reading "two High
  advisories fixed" would reasonably conclude they had been exposed, and they
  had not. That is the half of this project's design philosophy about people
  not being misled, applied to a changelog.

**Nothing else goes in `[0.82.0]`.** The three tidies (`2f4511c`) are on
`main` and are internal; we already settled they get no entry, and that does
not change because a version number is now available to them.

**This entry is not drafted in `.git-exclude/` first.** The `### Security`
disclosure gate is for *our own* findings: `.github/SECURITY.md` reserves a
severity-and-timeline discussion to what a reporter can expect before **public
disclosure**. A RUSTSEC advisory is already public, with its own ID and
publication date — this entry discloses nothing. Architect's call, stated here
so it is reviewable rather than silent.

## 4 — the release rule, discharged briefly and honestly

`.git-exclude/rules/…-general-common.md` requires a control audit and a doc
verification at every release. **Both are short here and I am telling you so
rather than having you discover it:**

- **Controls:** no first-party code changes, so no control changes. The
  relevant statement is that the TLS client path's dependency is now at a
  version without the four reachable advisories. One or two sentences.
- **Docs:** no behaviour changes, so no documented claim changes. Confirm the
  two staleness banners' workspace figure — `docs/development-specification.md`
  and `docs/threat-model.md` — which the bump makes stale again, and nothing
  else. Do **not** repeat `0.81.0`'s full documentation sweep.

## 5 — evidence

- **`cargo audit`**, captured **without a pipe** so the exit code is the
  command's own. Expect the nine to be gone; `paste` remains `unmaintained`
  and is an allowed warning, so a clean exit is the expected result.
- The full matrix — `Cargo.toml` and `Cargo.lock` change, so Level B scope.
  **This is the round's real risk**: a dependency update is exactly the thing
  that breaks a build or a lint on a lane you did not expect, so report every
  lane, and if `+stable` and `+1.95` disagree, that disagreement *is* the
  finding.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the last commit
  of the round, baselined on the tip at that moment.

**Level B is not yours to obtain**; the architect dispatches it on the pushed
release commit. Report your local result and say it is local.

## 6 — what this does not do

It does not tag, publish, or touch anything outside the four files this round
needs. After review the architect commits `release: 0.82.0`, pushes, obtains
Level B, **runs the `Security audit` workflow on that exact commit** — a step
added to the pre-publish checklist on 2026-10-20 because `0.81.0` was
published on a commit whose audit lane was red — then tags `0.82.0`, no `v`
prefix, and publishes.

Leave the working tree uncommitted. One open dispatch at a time; this is it.
