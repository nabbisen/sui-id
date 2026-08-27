#!/usr/bin/env bash
# Negative self-tests for RFC 093 A3.4 (scripts/check-gate-inputs.sh).
#
# Each fixture is a copy of the real ci/gate-inputs.toml, the real RFC 093
# Gate Matrix v1 table, and the real .github/workflows/ tree, with exactly
# one deliberate violation applied. Using the real files as the baseline
# (rather than a synthetic minimal schema) matches condition 4's own design:
# it is hardwired to the eleven real lane names RFC 093 defines, not an
# abstract schema, so a fixture needs the real shape to exercise it.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
checker="$repo_root/scripts/check-gate-inputs.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

make_valid_fixture() {
  local target=$1
  mkdir -p "$target/ci" "$target/rfcs/done" "$target/.github/workflows"
  cp "$repo_root/ci/gate-inputs.toml" "$target/ci/gate-inputs.toml"
  cp "$repo_root/rfcs/done/093-build-toolchain-release-gates.md" \
    "$target/rfcs/done/093-build-toolchain-release-gates.md"
  cp "$repo_root/.github/workflows/ci.yml" "$target/.github/workflows/ci.yml"
  cp "$repo_root/.github/workflows/audit.yml" "$target/.github/workflows/audit.yml"
  cp "$repo_root/.github/workflows/fuzz.yml" "$target/.github/workflows/fuzz.yml"
}

run_checker() {
  local root=$1
  bash "$checker" --all --policy ci/gate-inputs.toml \
    --rfc rfcs/done/093-build-toolchain-release-gates.md \
    --workflows-dir .github/workflows \
    --root "$root"
}

expect_failure() {
  local name=$1
  local expected=$2
  local fixture="$tmp/$name"
  local output="$tmp/$name.output"

  if run_checker "$fixture" >"$output" 2>&1; then
    echo "fixture $name unexpectedly passed" >&2
    cat "$output" >&2
    exit 1
  fi
  if ! grep -Fq "$expected" "$output"; then
    echo "fixture $name failed for the wrong reason; expected: $expected" >&2
    cat "$output" >&2
    exit 1
  fi
  echo "fixture $name: expected failure observed"
}

expect_success() {
  local name=$1
  local fixture="$tmp/$name"
  local output="$tmp/$name.output"

  if ! run_checker "$fixture" >"$output" 2>&1; then
    echo "fixture $name unexpectedly failed" >&2
    cat "$output" >&2
    exit 1
  fi
  echo "fixture $name: pass"
}

valid="$tmp/valid"
make_valid_fixture "$valid"
expect_success valid

# --- Condition 1: unpinned action reference ------------------------------
unpinned="$tmp/unpinned-action"
make_valid_fixture "$unpinned"
sed -i 's|uses: actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803 # v6|uses: actions/checkout@v6|' \
  "$unpinned/.github/workflows/ci.yml"
expect_failure unpinned-action "condition 1: unpinned action reference"

# --- Condition 2: workflow SHA missing from [actions] ---------------------
unrecorded="$tmp/unrecorded-sha"
make_valid_fixture "$unrecorded"
sed -i "s|d23441a48e516b6c34aea4fa41551a30e30af803|111111111111111111111111111111111111111e|g" \
  "$unrecorded/.github/workflows/ci.yml"
expect_failure unrecorded-sha "condition 2: workflow action SHA(s) not recorded in [actions]"

# --- Condition 3: stale [actions] row (SHA no longer used anywhere) ------
stale="$tmp/stale-action"
make_valid_fixture "$stale"
{
  echo ""
  echo "[actions]"
  echo 'stale_entry = "222222222222222222222222222222222222222f"'
} >>"$stale/ci/gate-inputs.toml"
expect_failure stale-action "condition 3: [actions] SHA(s) not used by any workflow"

# --- Condition 4a: [rust_components] missing a required lane -------------
missing_lane="$tmp/rust-components-missing-lane"
make_valid_fixture "$missing_lane"
sed -i '/^G09b = \[\]$/d' "$missing_lane/ci/gate-inputs.toml"
expect_failure rust-components-missing-lane "condition 4: [rust_components] is missing G09b"

# --- Condition 4b: [rust_components] wrong component array ---------------
wrong_components="$tmp/rust-components-wrong-value"
make_valid_fixture "$wrong_components"
sed -i 's/^G08 = \["rustfmt"\]$/G08 = ["clippy"]/' "$wrong_components/ci/gate-inputs.toml"
expect_failure rust-components-wrong-value "condition 4: [rust_components] G08 ="

# --- Condition 4c: [rust_components] duplicate lane -----------------------
dup_lane="$tmp/rust-components-duplicate"
make_valid_fixture "$dup_lane"
sed -i '/^G01 = \[\]$/a G01 = []' "$dup_lane/ci/gate-inputs.toml"
expect_failure rust-components-duplicate "condition 4: [rust_components] declares G01 more than once"

# --- Condition 4d: [rust_components] unexpected extra key ----------------
extra_lane="$tmp/rust-components-extra-key"
make_valid_fixture "$extra_lane"
sed -i '/^G09b = \[\]$/a G10 = []' "$extra_lane/ci/gate-inputs.toml"
expect_failure rust-components-extra-key "condition 4: [rust_components] has unexpected key"

# --- Condition 5: version is not 1 ----------------------------------------
bad_version="$tmp/bad-version"
make_valid_fixture "$bad_version"
sed -i 's/^version = 1$/version = 2/' "$bad_version/ci/gate-inputs.toml"
expect_failure bad-version "condition 5: version = 2, expected 1"

# --- Condition 5b: gate_matrix_version missing (not exactly one) ---------
missing_gmv="$tmp/missing-gate-matrix-version"
make_valid_fixture "$missing_gmv"
sed -i '/^gate_matrix_version = 1$/d' "$missing_gmv/ci/gate-inputs.toml"
expect_failure missing-gate-matrix-version "condition 5: manifest requires exactly one top-level gate_matrix_version"

# --- Condition 6: a gate-lane job uses the wrong runner -------------------
wrong_runner="$tmp/wrong-runner"
make_valid_fixture "$wrong_runner"
awk '
  /^  G01:$/ { print; in_g01 = 1; next }
  in_g01 && /^    runs-on: ubuntu-24.04$/ { print "    runs-on: ubuntu-latest"; in_g01 = 0; next }
  { print }
' "$wrong_runner/.github/workflows/ci.yml" >"$wrong_runner/.github/workflows/ci.yml.new"
mv "$wrong_runner/.github/workflows/ci.yml.new" "$wrong_runner/.github/workflows/ci.yml"
expect_failure wrong-runner "condition 6: gate-lane job(s) not using [runner] label"

# --- Condition 6b: a gate-lane job has no runs-on line at all (C1) -------
# GitHub Actions itself rejects a job with no runs-on, so this cannot slip
# a lane past the gate in practice — but the check must still assert
# *presence*, not only compare a value that might not exist. Regression
# fixture for review finding C1 (dead seen_runs_on scaffolding).
missing_runs_on="$tmp/missing-runs-on"
make_valid_fixture "$missing_runs_on"
awk '
  /^  G03:$/ { print; in_g03 = 1; next }
  in_g03 && /^    runs-on: ubuntu-24.04$/ { in_g03 = 0; next }
  { print }
' "$missing_runs_on/.github/workflows/ci.yml" >"$missing_runs_on/.github/workflows/ci.yml.new"
mv "$missing_runs_on/.github/workflows/ci.yml.new" "$missing_runs_on/.github/workflows/ci.yml"
expect_failure missing-runs-on "gate-lane job G03 has no runs-on line"

# --- Condition 7a: [gates] command diverges from the RFC table -----------
diverged_command="$tmp/gates-diverged-command"
make_valid_fixture "$diverged_command"
sed -i 's|^G02 = "cargo +1.95 test --workspace --locked"$|G02 = "cargo +1.95 test --workspace"|' \
  "$diverged_command/ci/gate-inputs.toml"
expect_failure gates-diverged-command "condition 7: [gates] does not match RFC 093's Gate Matrix v1 table"

# --- Condition 7b: [gates] missing a lane the RFC table has ---------------
missing_gate="$tmp/gates-missing-lane"
make_valid_fixture "$missing_gate"
sed -i '/^G09b = /d' "$missing_gate/ci/gate-inputs.toml"
expect_failure gates-missing-lane "condition 7: Gate Matrix v1 lane(s) absent from both [gates] and [gate_matrix_exceptions]"

# --- Condition 7c: [gates] has an extra lane not in the RFC table --------
extra_gate="$tmp/gates-extra-lane"
make_valid_fixture "$extra_gate"
sed -i '/^G09b = /a G10 = "echo not-a-real-lane"' "$extra_gate/ci/gate-inputs.toml"
expect_failure gates-extra-lane "condition 7: [gates] does not match RFC 093's Gate Matrix v1 table"

# --- Condition 7d: [gates] duplicate key -----------------------------------
dup_gate="$tmp/gates-duplicate-key"
make_valid_fixture "$dup_gate"
sed -i '/^G01 = /a G01 = "cargo +1.95 build --workspace --all-targets --locked"' \
  "$dup_gate/ci/gate-inputs.toml"
expect_failure gates-duplicate-key "condition 7: [gates] has duplicate key"

# --- Condition 7e: the one permitted normalisation is honoured -----------
# G05/G06's RFC form uses "`cmd1` and `cmd2`"; the manifest's "&&" form
# must be accepted as equivalent, not flagged as drift. The valid fixture
# already proves this (it passes with the real G05/G06 rows unmodified),
# but assert it explicitly so a future change to the normalisation logic
# that stops matching G05/G06 specifically is caught even if some other
# row masks the general diff.
and_form="$tmp/gates-and-form-accepted"
make_valid_fixture "$and_form"
expect_success gates-and-form-accepted

# --- Condition 7f: a lane on [gate_matrix_exceptions] passes (RFC 093 --
# --- M1b C2.1) -------------------------------------------------------------
# Move a real lane (G09b) from [gates] to the exception list with a
# reason. It remains fully accounted for -- the completeness rule does
# not care *which* of the two lists a lane is in, only that it is in
# exactly one. This is deliberately not just "the real fixture already
# has G12 excepted": it isolates the property by moving a lane that
# normally is *not* excepted, so the pass is not coincidental to G12
# specifically.
exception_pass="$tmp/gate-matrix-exception-accepted"
make_valid_fixture "$exception_pass"
sed -i '/^G09b = "cargo/d' "$exception_pass/ci/gate-inputs.toml"
sed -i '/^\[gate_matrix_exceptions\]/a G09b = "fixture: moved to exceptions to test the accept path"' \
  "$exception_pass/ci/gate-inputs.toml"
expect_success gate-matrix-exception-accepted

# --- Condition 7g: a lane in both [gates] and [gate_matrix_exceptions] --
# --- fails (RFC 093 M1b C2.1) -----------------------------------------------
# G12 stays in [gate_matrix_exceptions] (as it must) *and* also gains a
# [gates] entry with its real RFC command -- the two lists are supposed
# to be disjoint by construction, so this must fail even though the
# added command is not itself wrong.
both_lists="$tmp/gate-matrix-exception-and-gates-fails"
make_valid_fixture "$both_lists"
sed -i '/^\[gate_matrix_exceptions\]/i G12 = "bash scripts/check-ui-invariants.sh --all --policy ci/ui-invariants.toml"\n' \
  "$both_lists/ci/gate-inputs.toml"
expect_failure gate-matrix-exception-and-gates-fails \
  "condition 7: lane(s) present in both [gates] and [gate_matrix_exceptions]"

# --- Condition 8a: [tools] version drifted from what ci.yml installs -----
# (RFC 093 M1b C2.1 -- mdBook specifically had no enforcement at all
# before this: ci.yml could install any version and every prior check
# would still pass.)
tool_drift="$tmp/tools-mdbook-drift"
make_valid_fixture "$tool_drift"
sed -i 's/^mdbook = "0.5.4"$/mdbook = "0.6.0"/' "$tool_drift/ci/gate-inputs.toml"
expect_failure tools-mdbook-drift \
  'condition 8: [tools] mdbook = "0.6.0", but .github/workflows/ci.yml installs/invokes: 0.5.4'

# --- Condition 8b: [tools] entry with nothing in ci.yml to check it -----
# --- against (declared but unused) --------------------------------------
# Delete every line the checker's own grep pattern would match, so
# python-version is not merely wrong but entirely absent -- proving the
# "not found anywhere" branch, distinct from 8a's "found, but drifted".
tool_unused="$tmp/tools-python-unused"
make_valid_fixture "$tool_unused"
sed -i '/python-version: "3\.14"/d' "$tool_unused/.github/workflows/ci.yml"
expect_failure tools-python-unused \
  'condition 8: [tools] python = "3.14" not found anywhere'

# --- Condition 8c: [tools] drifted in one job but not the others --------
# Neither 8a (all wrong) nor 8b (none found) exercises "some right, some
# wrong" -- the actual failure mode condition 8 exists to prevent: a
# version moving under a pin that nothing reads, in one job, while every
# other job (and the manifest) still says the old value. Change only the
# first of ci.yml's three python-version occurrences.
tool_partial_drift="$tmp/tools-python-partial-drift"
make_valid_fixture "$tool_partial_drift"
awk '
  !done && /python-version: "3\.14"/ { sub(/3\.14/, "3.13"); done = 1 }
  { print }
' "$tool_partial_drift/.github/workflows/ci.yml" >"$tool_partial_drift/.github/workflows/ci.yml.new"
mv "$tool_partial_drift/.github/workflows/ci.yml.new" "$tool_partial_drift/.github/workflows/ci.yml"
expect_failure tools-python-partial-drift \
  'condition 8: [tools] python = "3.14", but .github/workflows/ci.yml installs/invokes: 3.13 3.14'

echo "gate-inputs negative fixtures passed"
