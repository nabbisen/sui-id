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
**Baseline.** **After RFC 098 dispatch 2 lands** (it moves the matrix to
`ci/audit-coverage-matrix.md` and repoints the script's `MATRIX=`). Do not start
this on a tree where step 4 is unmerged: the fixture's matrix path would be
wrong the moment it lands. If dispatch 2 is not yet on `main`, wait.

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
