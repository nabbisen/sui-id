# G13 — register the audit-coverage check as an interim lane owned by RFC 094

**Governing RFC.** [RFC 094](../../accepted/094-transactional-audit-registry.md)
§Gate Matrix lanes owned by RFC 094 (added 2026-09-12) and §Structural coverage
gate §Migration: *"Adding the structural-gate lane afterwards is a manifest entry
plus a single-row table in this RFC — the smallest change that can add a gate."*
The table is already in the RFC. This dispatch is the manifest entry, the job,
and the fixture.
**Why.** `scripts/check-audit-matrix.sh` has never run in CI. RFC 085 closed at
v0.68.0 on "CI gate live and bidirectional"; no workflow in the repository's
history ever invoked it (finding: `.git-exclude/reviewed/audit-matrix-gate-never-wired-2026-09-12.md`).
**Authorized.** `@nabbisen`, 2026-09-12, as step 2 of the R10 resolution.
**Implementer.** Mid-capability model.
**Baseline.** `c451c52` or later — RFC 098 dispatch 2 has landed: the matrix is
at `ci/audit-coverage-matrix.md` and the script's `MATRIX=` points there.
**Unblocked 2026-09-12.** Proceed.
**Landed.** `1ec5dad`, 2026-09-12 — reviewed at
`.git-exclude/reviewed/rfc-098-dispatch-4-and-g13-2026-09-12.md`. Stop condition 1
did not trigger; stop condition 2 did (a fifth file, `68ab6ce`), and was right.

## The mechanism doing the work

This is the first lane registered by an RFC other than 093, so it is also the
first real exercise of R10. A3.4 will read RFC 094's table under the recorded
heading and require the manifest to match it — check 3 fails if the RFC declares
G13 and the manifest does not; check 4 fails if the command differs by a byte.
That is the registry proving itself on real input, not a fixture. Expect A3.4 to
be the gate that tells you whether you got the manifest right.

## Scope — four files, one commit

| File | Change |
|---|---|
| `ci/gate-inputs.toml` | `[gate_lane_sources]`: add `"094" = "Gate Matrix lanes owned by RFC 094"` — the heading verbatim. `[gate_owners]`: `G13 = "094"`. `[gates]`: `G13 = "bash scripts/check-audit-matrix.sh"` — byte-for-byte the RFC's cell with backticks removed. `[rust_components]`: `G13 = []` if condition 4 requires every lane to have an entry (read condition 4; G10b/G11 will tell you the convention for script lanes). |
| `.github/workflows/ci.yml` | A `G13` job shaped like G11's, **without** the Python step — the script is bash and grep only: checkout, then `bash scripts/ci-gate.sh G13`. Name it `"G13 — audit coverage matrix (interim, RFC 094)"`. |
| `scripts/tests/fixtures/gate-matrix/audit-desync/` | A self-contained fixture repo in the A3.2 shape: a minimal `crates/<one crate>/src/lib.rs` carrying one audit event literal in the form the script greps for, and a matrix file at the path the script's `MATRIX=` names, with **two** desyncs — a literal in source with no row, and a row with no literal — so both directions of the check are proven to fire, not one. Copy the real script's grep pattern into the fixture by reading `SRC_DIRS`/the literal regex, not by guessing what "the form" is. |
| `scripts/tests/check-gate-matrix-fixtures.sh` | Register G13: fails on `audit-desync`, passes on a clean copy of the real tree (or a minimal in-sync fixture, if the real tree is too large to stage — say which). Driven through `ci-gate.sh` like the others. |

**Not in scope:** any edit to RFC 094 (its table is already there; if you find
the table wrong, stop — it is the architect's), RFC 085 (its correction note is
the architect's, written at merge when the lane is green on the hosted runner),
`scripts/check-audit-matrix.sh` itself (dispatch 2 owns its path; nothing else
about it changes), and the three unreachable detectors from R10-b (R10-c, later).

## Two stop conditions

1. **`ci-gate.sh` and the script's relative paths.** The script uses
   `SRC_DIRS="crates"` and `MATRIX=…` relative to the working directory. A3.2's
   harness drives lanes through `ci-gate.sh` inside a staged fixture repo. If the
   dispatcher's `--root` handling means the script resolves those paths against
   the wrong directory, **stop and report what you measured** — do not make the
   script accept a root argument, and do not make the fixture `cd`. That is a
   design question about how script lanes and the dispatcher compose, and it
   affects G10b/G11 too.
2. **Condition 8 or the A3.2 harness needing a change beyond registering G13.**
   A fifth file is a stop condition.

## Evidence required

1. `bash scripts/check-gate-inputs.sh --all --policy ci/gate-inputs.toml` — exit
   0, **and** the run you did *before* adding `G13` to `[gates]` but after adding
   `"094"` to sources: it must have failed check 3 naming `G13`. Report both.
   That is the registry proving it reads RFC 094's table.
2. `bash scripts/ci-gate.sh G13` on the real tree: **54 matrix entries, 54 source
   literals**, exit 0 — the same numbers the script reports by hand.
3. `bash scripts/tests/check-gate-matrix-fixtures.sh`: G13 fails on
   `audit-desync` with *both* desyncs named in its output, passes on the clean
   case. Report the two lines.
4. All three harnesses green; G11; G10b both scopes.
5. Mutation: comment out one of the two grep directions in a scratch copy of
   the script and show the fixture stops catching that direction. A fixture that
   only proves one direction proves the bidirectional claim vacuously.

## After this lands

The architect adds RFC 085's dated correction note naming this commit, and RFC
098's documentation lane follows on the same rail. R10-c retires the three dead
detectors afterwards.

## G13-b — the allowlist that could not see two real events

**Dispatched 2026-09-13**, from RFC 098 dispatch 10's review. One commit.

**Steps 1 and 2 landed** `a4a178d`, 2026-09-13. **Step 3 as first written was wrong** — the
implementer measured it rather than building it: deriving from `events.rs` and
the command descriptors alone yields `admin|auth|mfa|signing_key|user` and a
gate that passes at **40/40**, fourteen events fewer than today, still blind to
`webauthn.`. Ruled in `.git-exclude/reviewed/g13-b-derived-allowlist-2026-09-13.md`;
steps 3 and 4 are restated below and supersede the originals.

`scripts/check-audit-matrix.sh` recognises an audit event literal by a
namespace allowlist — `(user|client|signing_key|settings|me|auth|admin|oauth2|mfa)\.`
at lines 30 and 43. Nothing keeps that list in step with the code. `mfa.` was
added by hand on 2026-09-09 after the gate missed it; today it cannot see
`webauthn.credential.register` and `webauthn.credential.delete`
(`handlers/me_security/passkey.rs:167,209`), two real audit actions absent
from the matrix, and would not have seen the six `oauth.*` names in
`events.rs:184–189` had they ever been emitted — which they never were.

**Do:**

1. **Register the two `webauthn.*` events** in `ci/audit-coverage-matrix.md`
   with correct rows (actor, subject, note shape) taken from the emitting code.
2. **Delete the six dead `oauth.*` variants** from `crates/sui-id-core/src/events.rs`
   — `AuthorizeIssued`, `AuthorizeRejected`, `TokenIssued`, `TokenRefreshed`,
   `TokenIntrospected`, `TokenRevoked` — and their `name()`/`outcome()`/`note()`
   arms. Zero constructions in the workspace; `cargo check` proves it.
3. **Derive the allowlist from the code.** Replace the literal group with one
   computed at run time from the declared vocabulary: the first segment of
   every string in `events.rs`'s `name()` arms, and of every `event:` literal
   in a `declare_write_command!` invocation. A namespace declared in neither
   is, by definition, not an audit event — that is the honest limit of a
   string gate. Print the derived set in the run's output so a reviewer sees
   what the gate saw.
4. **Negative fixture** in the A3.2 `audit-desync` set: a literal in a
   namespace the code declares but the matrix lacks must be reported by the
   backward check — this is exactly today's `webauthn.` case, and the fixture
   must fail on the pre-change script and pass on the new one.

### Steps 3 and 4, restated 2026-09-13 (ruling: option 1)

**3. Derive the allowlist from the three places the code writes an audit
action.** Computed at run time inside `scripts/check-audit-matrix.sh`, replacing
the literal group at both grep sites:

| Source | Extract | Exclude |
|---|---|---|
| `crates/sui-id-core/src/events.rs` | the string in every `name()` arm (`=> "ns.…"`) | — |
| `crates/sui-id-store/src/commands.rs` | every `name: "ns.…"` in a `declare_write_command!` | `registry.rs` — its `proof_only.system_principal_forbidden` is a test descriptor, not an event |
| every non-test `.rs` under `crates/` | every `action: "ns.…"` literal in an `AuditLogRow` construction | files under `tests/`, `tests.rs`, `#[cfg(test)]` modules |

The derived set on the tree at step 2 is
`admin|auth|client|mfa|oauth2|settings|setup|signing_key|token|user|webauthn`
— print it in the run's output so a reviewer sees what the gate saw. The
literal group is gone; nothing hand-maintained remains.

**Then register the three events the derivation surfaces**, rows from the
emitting code as step 1 did: `setup.create_initial_admin`
(`handlers/setup.rs`), `token.introspect` and `token.revoke`
(`handlers/oauth_token.rs`). Do not add an exclusion list; do not leave them
unregistered.

**Evidence for step 3:** `audit-matrix gate PASS: 59 matrix entries, 59 source
literals`, with the derived namespace set printed above it. And the run of the
*new* script against the tree at step 2 (before the three rows), which must
fail naming exactly those three — the gate proving it sees what the old one
could not.

**4. The fixture.** In the A3.2 `audit-desync` set: the fixture crate emits a
raw `action: "webauthn.fixture_only"` with no matrix row. It must be reported
by the backward check under the new script and **not** reported under
`1ec5dad`'s — run both, show both. Add the assertion to `expect_gate_output`
alongside the two existing directions.

**Not in scope:** any change to the lane's command (unchanged, so RFC 094's
table and the manifest are untouched), or to G13's status as interim — the
namespace blindness is one more reason `audit-structure` is the authoritative
control at M2b, and the architect adds that note to RFC 094 at merge.

**Evidence.** 54 + 2 = **56 matrix entries, 56 source literals** (the six dead
names were never literals the gate counted, so deleting them changes nothing
there). The derived namespace set printed. A3.2 green with the new fixture,
and the fixture shown failing against `1ec5dad`'s script. `cargo check`,
clippy both scopes, `cargo test` (the events enum is exercised by tests;
report the count).
