# RFC 093 M1a — trustworthy build baseline

**Governing RFC:** [RFC 093](../../accepted/093-build-toolchain-release-gates.md)
**Lane:** A (`codex-developer`)
**Exit gate:** G01–G09 pass on one clean commit, hosted, with recorded tool
versions; the gate-input manifest is enforced by a tracked check.

Themes are ordered. A0 and A2 may start immediately and in either order; A1
needs the governance commit `e10a40f` (already landed); A3 needs A0–A2; Theme B
needs A2.

---

## Theme A0 — two corrective fixes

**Gate:** none. Owner-authorized 2026-07-28 under the roadmap's corrective-fix
exception. Both have been red since at least 2026-07-16 and unblock five gate
lanes between them.
**Owns:** root `Cargo.toml` dependency line; `docs/book.toml`.

### A0.1 — `ldap3` rustls crypto provider

`Cargo.toml:74` declares `features = ["tls-rustls"]`, for which `ldap3 0.12.1`
emits `compile_error!`: *"No crypto provider selected for Rustls, use
`tls-rustls-aws-lc-rs` or `tls-rustls-ring`."*

Select one provider and justify it in the commit message. **Recommended:
`tls-rustls-ring`** — the workspace already reaches `ring` through its rustls
stack, so this avoids introducing `aws-lc-rs` and its C toolchain into the
build. Verify against `Cargo.lock` first; if `aws-lc-rs` is already present and
`ring` is not, invert the choice and record why.

Verify: `cargo check --workspace --all-targets --all-features --locked` compiles
`ldap3`. Expected to unblock G03, G04, G06 and make G09 buildable.

> **Outcome (landed `73b5baf`, corrected `aebfd36`).** `tls-rustls-ring` was
> chosen, and the stated rationale — "avoids introducing `aws-lc-rs`" — turned
> out to be moot: reqwest's rustls integration already compiles `aws_lc_rs` in.
> Having **two** backends present is what makes `rustls::ClientConfig::builder()`
> ambiguous, which is the provider panic G09 exists to catch. Note the two are
> separate concerns: `ldap3`'s *feature* is `tls-rustls-ring` (what compiles in),
> while startup *installs* `aws_lc_rs` as the process default (what actually
> gets used, including by reqwest). Do not "align" them — see A3.3.

### A0.2 — mdBook GitHub icon

`docs/book.toml:13` sets `git-repository-icon = "fa-github"`, the Font Awesome
*regular* form. GitHub is a *brands* icon. Change it to the brands form the
configured mdBook 0.5.4 theme expects.

Verify: `mdbook build docs` exits 0. Unblocks G10a.

**Evidence:** both diffs, both verification commands with output, and
`cargo test --workspace` still passing.

---

## Theme A1 — raise MSRV to 1.95

**Gate:** governance commit `e10a40f` (landed).
**Owns:** `Cargo.toml` `rust-version`; `README.md`; `ci/gate-inputs.toml`
`[tools] rust_msrv`; `CHANGELOG.md`.

The floor is **measured, not estimated**. `libsqlite3-sys 0.38.1` calls
`cfg_select!` in its build script — an unstable library feature until 1.95 —
reached through `rusqlite 0.40` on the `bundled` path. On this workspace 1.91,
1.92, 1.93 and 1.94 all fail E0658; 1.95 builds. Reproduce once if you want
independent confidence, but do not re-derive the value.

Steps:

1. `rust-version = "1.95"` in the workspace manifest.
2. Update the README MSRV claim **and state the `rustup` expectation** — 1.95 is
   newer than most distribution-packaged toolchains, so operators building from
   source will generally need `rustup` rather than their distro's Rust.
3. Update `rust_msrv` in `ci/gate-inputs.toml`.
4. CHANGELOG entry stating plainly that the previously declared 1.91 was not
   buildable, not merely untested.

Verify: `cargo +1.95 build --workspace --all-targets --locked` and
`cargo +1.95 test --workspace --locked` both pass.

---

## Theme A2 — clippy cleanup

**Gate:** none. Land A0.1 first to clear the all-features lane in one pass.
**Land this before Theme B** — it has the widest file footprint in M1a and will
collide with the `federation.rs` move otherwise.

Current: `cargo clippy --workspace --all-targets --locked -- -D warnings` fails
with 66 library and 179 library-test errors on stable 1.97.1.

Rules:

1. **Fix, do not silence.** RFC 093 Requirements item 3 permits fixing
   stable-only lint drift but forbids silencing it globally or converting it to
   an allowlist to preserve a date. A targeted `#[allow]` with a one-line
   justification is acceptable where the lint is genuinely wrong; crate-level or
   workspace-level allows are not.
2. **Production `unwrap()` needs judgement, not mechanical replacement.**
   `unwrap_used` findings include production paths such as
   `crates/sui-id/src/http/security_headers.rs:252`. For each, decide whether
   the invariant genuinely holds — then `expect()` with a message naming the
   invariant — or whether it can fail, and propagate the error. Never convert a
   real failure mode into a silent default.
3. Test-only `unwrap()` is acceptable; prefer `expect()` where the message aids
   debugging.
4. **No behaviour changes.** If a lint fix would alter runtime behaviour, stop
   and raise it — that is a defect, not lint drift.

Verify: `cargo clippy --workspace --all-targets --all-features --locked -- -D
warnings` exits 0 and `cargo test --workspace` passes with an unchanged count.

**Evidence:** before/after counts, every `#[allow]` added with its
justification, and a one-line disposition for each production `unwrap`. Submit
in per-crate batches; a single 245-error package is not reviewable.

---

## Theme A3 — gate lanes G01–G09

**Gate:** A0, A1, A2 complete.
**Owns:** `.github/workflows/ci.yml`; `scripts/ci-gate.sh`;
`scripts/tests/check-gate-matrix-fixtures.sh`; `scripts/tests/fixtures/`;
`crates/sui-id-store/tests/ldap_smoke.rs`; `ci/gate-inputs.toml`;
`scripts/check-gate-inputs.sh`.

### A3.1 — lane wiring

Wire **G01–G09 plus G07b** as blocking jobs executing the literal Gate Matrix
commands. G07b was added to RFC 093 on 2026-07-28; see D2 below for why.

Each job is `checkout` + `bash scripts/ci-gate.sh <GATE_ID>`. The dispatcher —
not the YAML — owns the evidence block: record the environment, resolve
`git rev-parse HEAD` and assert it equals `$GITHUB_SHA`, assert a clean tree,
echo the literal command, and capture `exit_status` with start/end timestamps.
The literal commands live in `ci/gate-inputs.toml` under `[gates]`; the
dispatcher executes them verbatim. See D1 for the full shape and rationale.

### A3.1 — structural decisions (settled 2026-07-28)

Three structural choices were raised before implementation. All three are
decided; do not re-open them while drafting.

#### D1 — Dispatcher, with the command table as data

**Decision: one `scripts/ci-gate.sh <GATE_ID>` dispatcher.** `ci.yml` jobs
become `checkout` + `bash scripts/ci-gate.sh G01`.

The literal command for each lane lives in `ci/gate-inputs.toml` under a new
`[gates]` table — RFC 093 already requires the matrix to be stored "in a
machine-readable or mechanically parsed repository file under `ci/`". The
dispatcher reads and executes the recorded command verbatim.

```toml
[gates]
G01 = "cargo +1.95 build --workspace --all-targets --locked"
G02 = "cargo +1.95 test --workspace --locked"
# … one entry per lane, exactly as RFC 093's table records it
```

The dispatcher owns the environment/evidence block once: resolve
`git rev-parse HEAD`, assert it equals `$GITHUB_SHA`, assert a clean tree,
print the runner image and tool versions, echo the literal command, then run it
capturing `exit_status` and start/end timestamps.

Rationale, and why the stated indirection risk is answered rather than accepted:
nine inline jobs would duplicate that evidence block nine times, giving nine
chances to omit the HEAD/`GITHUB_SHA` assertion — which is exactly Wave 2
finding B2, already made once. Centralising it makes the evidence contract
uniform and testable. The dispatcher's own risk — that `ci-gate.sh G07` might
not run G07's matrix command — is closed mechanically in A3.4:
`scripts/check-gate-inputs.sh` compares `[gates]` against RFC 093's table and
fails on any divergence. That converts a convention into a checked invariant,
which is the standard this programme holds everything else to.

Local reproduction is then exact: `bash scripts/ci-gate.sh G07` runs precisely
what CI runs.

#### D2 — Remove `build-test`; remove `fmt-clippy` only together with new lane G07b

**Decision: remove both legacy jobs in the A3.1 change — but add G07b first,
in the same change.**

The request's reasoning for removing them was right in substance and wrong in
one specific: `build-test` is a strict subset of G05, but **`fmt-clippy` is not
a strict subset of G07**. G07 runs `--all-features` only, and an all-features
clippy run never sees code behind `#[cfg(not(feature = "…"))]`. The workspace
has exactly one such site — `crates/sui-id/src/runtime/startup.rs`, the
LDAP-absent branch, which contains a `let _ = password;` binding of precisely
the shape clippy has lints about.

So Gate Matrix v1 as written had a hole: no default-features clippy lane.
RFC 093 has been amended to add **G07b — stable, default features, clippy**,
and its Feature-and-target policy now records that G07 and G07b are both
mandatory and neither subsumes the other.

With G07b present, removing `build-test` and `fmt-clippy` loses nothing, and the
distinction the request drew from the G12 case holds: G12's legacy UI jobs use
independent detection logic (grep) from `ui-invariants-v1` (awk lexer), so
keeping both hedged against a mechanism bug. These legacy jobs use the same
`cargo` mechanism as G01–G09, just a narrower matrix of it — there is no
independent detection to hedge with, only duplicated minutes. **G08 is
feature-independent**, so `cargo fmt` needs no second lane.

Order within the change: add G01–G09 **and G07b**, prove them green, then delete
the two legacy jobs in the same commit. Do not delete first.

#### D3 — Cache the registry only; no `target/` cache for matrix lanes

**Decision: Option B.** Cache `~/.cargo/registry` and `~/.cargo/git`; do **not**
cache `target/` for G01–G09.

The registry cache is content-addressed source and is safe to share across every
toolchain and feature combination. `target/` is where the isolation risk lives,
and its payoff is CI minutes rather than correctness.

There is a little more correctness content here than the request allowed: a gate
matrix exists to distinguish build configurations, so its evidence should not
depend on artifacts produced under a different one. Cargo fingerprinting usually
catches a mismatch, but "usually" is not the standard for a lane whose output is
release evidence.

The decisive argument, though, is sequencing: **no lane has ever run hosted, so
nobody knows what any of this costs.** Optimising cache topology before the first
green matrix run is guessing. Establish correctness, measure real lane runtimes,
then revisit with data — and if `target/` caching is added later, it must be
keyed per `(toolchain, feature-set)` pair, never shared.

Keep the existing key discipline: RFC 093 requires cache keys to include the
input-manifest revision plus `Cargo.lock`, which the current
`…-gate-input-v1-cargo-${{ hashFiles('**/Cargo.lock') }}` shape already does.
Preserve it for the registry cache.

### A3.2 — negative self-tests (blocking; do not defer)

Each of G01–G08 **and G07b** needs a deliberately invalid fixture under
`scripts/tests/fixtures/gate-matrix/` that makes its lane fail: compile-error,
failing-test, lint-warning, and format-drift.
`scripts/tests/check-gate-matrix-fixtures.sh G01 … G08 G07b` must observe every
intended failure and exit zero only after each was detected.

**G07b's fixture must be feature-gated to prove the gap it exists to close.**
A plain lint warning would fail G07 too and would not show that G07b covers
anything G07 misses. Put the deliberate lint violation behind
`#[cfg(not(feature = "ldap"))]`, then assert that the fixture **fails G07b and
passes G07** — that pair is the actual proof.

G10a is self-tested by `mdbook build scripts/tests/fixtures/mdbook-missing-chapter`,
which must exit non-zero.

This is roughly half the theme. G12 is the model: most of its review rounds were
about adversarial fixture completeness, so build fixtures alongside each lane
rather than afterwards. Include count-preserving mutations wherever a check
counts anything.

#### A3.2 harness — hosted-run defect and required fix (2026-07-29)

**The first hosted run failed here, and the design fault is the architect's.**
Two things I specified are incompatible in CI:

1. `ci-gate.sh` asserts `git rev-parse HEAD == $GITHUB_SHA` — added deliberately
   to bind gate evidence to a commit (review findings B2, then hardened by C1).
2. The A3.2 harness stages each fixture into a throwaway git repo with its own
   one-commit history, so fixtures run through the **real** dispatcher.

In CI, `GITHUB_SHA` is the real repository's commit while the fixture's `HEAD` is
the throwaway commit. The assertion fires and the dispatcher exits 1 **before
running the gate command at all**. Observed:

```
G01..G07b against compile-error: expected failure observed   <- vacuous
G08 against compile-error unexpectedly failed                <- caught here
  event_commit=1e59e3d…      checked_out_commit=a5ada8ce…
  ::error::ci-gate G08: checked-out HEAD does not match GITHUB_SHA
```

Locally this never appeared because `GITHUB_SHA` is unset, so the assertion was
skipped — the blind spot flagged twice in review as "this branch has never
executed."

**The serious part is not the failure, it is that eight assertions passed for the
wrong reason.** Every `expect_gate_fails` case was satisfied by the precondition
error rather than by the fixture's deliberate defect. The harness only noticed at
the first `expect_gate_passes`. A harness whose purpose is proving other checks
do not pass for the wrong reason did exactly that.

**Two fixes are required, not one.**

**Fix 1 — make the assertion true rather than skipped.** Before invoking the
dispatcher for a staged fixture, set `GITHUB_SHA` to that fixture repo's own
`HEAD`:

```bash
GITHUB_SHA=$(git -C "$fixture_dir" rev-parse HEAD)   bash "$ci_gate" "$gate" --root "$fixture_dir"
```

Do **not** unset `GITHUB_SHA` and do **not** add a `--skip-sha-check` flag to the
dispatcher. Unsetting reproduces the local blind spot that hid this; a bypass flag
puts a hole in the evidence contract that a real lane could one day use. Setting
it to the fixture's actual HEAD keeps the assertion executing and asserting
something true — the tree being gated is the commit claimed.

**Fix 2 — `expect_gate_fails` must verify *why* it failed.** A non-zero exit is
not sufficient; the failure must come from the gate command, not from the
dispatcher's preconditions. Assert that the output contains `exit_status=` (the
dispatcher reached and ran the command) and does **not** contain
`::error::ci-gate`. Without this, the harness stays blind to any future
precondition failure in exactly the same way.

Fix 2 is what would have caught this in CI even with Fix 1 absent, and it is the
same discipline as the count-preserving mutation requirement above: prove the
assertion fails for the intended reason, not merely that it fails.

**Verification required:** re-run hosted. All 38 assertions must pass *and* the
job must be green with Fix 2 in place — because with Fix 2 and without Fix 1,
the eight previously vacuous assertions will now fail loudly, which is the
correct behaviour and the proof that Fix 2 works.

#### Fixture manifest shape — settled 2026-07-29

Owner-confirmed: one declaration in the root manifest rather than five repeated
per-fixture ones, chosen for overall project-structure simplicity. The superseded
alternative and the reasoning behind both are preserved in
`.git-exclude/reviewed/m1a-a3.2-fixture-manifest-boilerplate-decision-2026-07-29.md`.

**Fixture manifests carry no `[workspace]` table.** Workspace scoping is handled
once, in the root `Cargo.toml`:

```toml
[workspace]
# Gate Matrix negative fixtures are deliberately broken packages (compile
# errors, failing tests, lint violations). Excluding the directory keeps them
# out of the workspace so `cargo … --workspace`, which the dispatcher runs
# verbatim, resolves to the single fixture it is invoked in rather than
# building them all together.
exclude  = ["scripts/tests/fixtures"]
```

Each fixture is then minimal:

```toml
[package]
name    = "gate-matrix-fixture-xxx"
version = "0.0.0"
edition = "2024"
```

(plus `[features]` where the fixture needs one, as `lint-warning-feature-gated`
does). No `[workspace]`, no `[lib] path` — that is Cargo's default — and no
`publish = false`, since nothing here publishes.

**Why scoping is needed at all.** The dispatcher runs each Gate Matrix command
verbatim (decision D1), and several include `--workspace`. Cargo resolves that by
walking up to the nearest ancestor `[workspace]`. Verified behaviour:

| Configuration | Result |
|---|---|
| Fixtures share one workspace | **Contaminated** — `cargo build --workspace` from any member also builds every other; `compile-error` fails the build for all of them, destroying per-fixture isolation |
| No scoping at all, fixture in-place | **Hard error** — *"current package believes it's in a workspace when it's not"*; Cargo finds the sui-id root workspace |
| **Root `exclude`, no per-fixture table** | **Correct** — each fixture resolves standalone, isolation preserved |
| Empty `[workspace]` in each fixture | Also correct, but five unexplained empty tables that read as boilerplate |

The last two are functionally equivalent. The root `exclude` is preferred: one
declaration, in the place that already defines workspace membership, where it can
be commented — instead of five bare empty tables that invite exactly the two
"cleanups" that break things (deleting them, or gathering the fixtures into a
shared workspace, the second of which fails **silently** while the harness still
appears to run all 38 assertions).

Verified end-to-end on a worktree of the real repository with the empty tables
removed and the root `exclude` added: all 38 assertions pass, the G07/G07b
asymmetry holds (0 / 101), `compile-error` does not contaminate siblings, and
`cargo metadata` still resolves the six real workspace members.

**If fixtures ever move, the `exclude` path moves with them.** That coupling is
the one cost of this approach and is why the comment above it names the reason.

### A3.3 — LDAP smoke (G09a/G09b)

Create `crates/sui-id-store/tests/ldap_smoke.rs` with exactly two tests:

- `rustls_provider_and_tls_connector_reach_fixture` — install or select the
  process rustls crypto provider exactly as production startup does, construct
  the configured connector with the chosen `tls-rustls-*` feature, connect to a
  loopback fixture or controlled listener, and assert the TLS/provider path
  executed. An expected authentication or connection rejection is acceptable
  **only after** the provider path was reached and asserted. Fail on unexpected
  plaintext downgrade or an unclassified error.
- `rejects_missing_crypto_provider` — the negative fixture; must catch provider
  absence before any network assertion.

A compile-only test does not satisfy G09. A public LDAP service is forbidden for
the blocking lane. The fixture contains no real directory credentials.

### A3.4 — `ci/gate-inputs.toml` enforcement

Closes the finding that the manifest is currently read by nothing and therefore
records a contract it cannot defend.

Create a tracked `scripts/check-gate-inputs.sh` that fails closed on all of:

1. every `uses:` in `.github/workflows/**` matches `@[0-9a-f]{40}`;
2. every workflow action SHA appears in `[actions]`;
3. every `[actions]` SHA is used by at least one workflow (catches stale rows);
4. `[rust_components]` declares `G01 G02 G03 G04 G05 G06 G07 G07b G08 G09a
   G09b` exactly once each, with `G07`/`G07b` = `["clippy"]`,
   `G08` = `["rustfmt"]`, the rest `[]`;
5. `version` and `gate_matrix_version` are both `1`;
6. every gate-lane job uses the `[runner] label`;
7. **`[gates]` matches RFC 093's Gate Matrix table exactly** — same lane set,
   same command string per lane, no extras, no omissions, and no duplicate key
   within `[gates]`. This is what makes the D1 dispatcher safe: it turns "the
   dispatcher runs the matrix command" from a convention into a checked
   invariant. Parse the RFC's table rather than hardcoding a second copy of the
   commands, or the check just moves the drift somewhere else.

   **One normalisation is required, and only one.** RFC 093 renders the G05 and
   G06 rows as two backticked commands joined by the English word *and*, while
   the manifest joins them with `&&`. Treat that single form as equivalent and
   say so in the checker; do not add a general fuzzy-match, or the check stops
   detecting real drift. Every other lane must compare byte-for-byte.

Add one negative fixture per condition, mirroring the G12 fixture style, and
wire it as a CI step echoing its literal command.

### A3.5 — resolve the `RUSTFLAGS` collision

`.github/workflows/ci.yml:21` sets `RUSTFLAGS: "-D warnings"` at workflow level,
so it applies to every job. Under the Gate Matrix that makes G01–G06 build and
test lanes fail on lint drift — work the matrix assigns to G07 — and conflicts
with RFC 093 Requirements item 3, which allows stable-only lint drift to be
fixed without touching MSRV.

Remove it from the workflow-level `env:`. G07 already carries `-D warnings` in
its clippy invocation. If you conclude the global setting should stay, that is a
Gate Matrix semantics change requiring an RFC 093 amendment: raise it.

---

## Theme B — preparatory `federation.rs` split

**Gate:** A2 complete. **Owner:** neither lane; reviewed on its own.

`crates/sui-id/src/http/handlers/federation.rs` is ~886 lines containing both
callback/validation logic (RFC 096) and provider/link mutation logic (RFC 094
C17/C18/C23). Concurrent editing by two lanes is the likeliest way to produce a
merge that silently drops a security check. It is a named prerequisite in
RFC 096's implementation prerequisites.

**Requirement: zero behaviour change.** This is a pure move.

Suggested shape — confirm against the code before committing:

```
handlers/federation.rs            thin router / entry points
handlers/federation/discovery.rs  provider metadata retrieval
handlers/federation/callback.rs   state, nonce, code exchange, ID-token handling
handlers/federation/admin.rs      provider and link mutation (RFC 094 territory)
```

Per the project rule, tests move to `handlers/federation/tests/` mirroring the
split rather than living inside implementation files.

Verify: `cargo test --workspace` passes with an unchanged test count; `cargo
fmt` and clippy clean; `git diff` shows moves, not rewrites. Reviewers will
check that no conditional, comparison, or early return changed during the move.
