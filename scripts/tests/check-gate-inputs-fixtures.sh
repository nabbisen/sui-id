#!/usr/bin/env bash
# Negative self-tests for RFC 093 A3.4 (scripts/check-gate-inputs.sh).
#
# Each fixture is a copy of the real ci/gate-inputs.toml, the real RFC 093
# Gate Matrix v1 table, and the real .github/workflows/ tree, with exactly
# one deliberate violation applied. Using the real files as the baseline
# (rather than a synthetic minimal schema) keeps each fixture attributable to
# the one violation applied to it: condition 7 resolves real RFCs by number.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
checker="$repo_root/scripts/check-gate-inputs.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

make_valid_fixture() {
  local target=$1 dir
  mkdir -p "$target/ci" "$target/.github/workflows"
  cp "$repo_root/ci/gate-inputs.toml" "$target/ci/gate-inputs.toml"
  # All four lifecycle folders, because R10 resolves an RFC number to a file
  # by searching them — a fixture holding only RFC 093 could not tell
  # "resolves to exactly one" from "happens to be the only file present".
  for dir in proposed accepted "done" archive; do
    mkdir -p "$target/rfcs/$dir"
    if [[ -d "$repo_root/rfcs/$dir" ]]; then
      cp "$repo_root/rfcs/$dir"/*.md "$target/rfcs/$dir/" 2>/dev/null || true
    fi
  done
  cp "$repo_root/.github/workflows/ci.yml" "$target/.github/workflows/ci.yml"
  cp "$repo_root/.github/workflows/audit.yml" "$target/.github/workflows/audit.yml"
  cp "$repo_root/.github/workflows/fuzz.yml" "$target/.github/workflows/fuzz.yml"
}

# A fixture with an additional lane source. RFC 900 does not exist and never
# will: the number is outside the allocated range precisely so this fixture
# can add a source without colliding with a real one. It used to stand in for
# RFC 094 by deleting the real file -- which stopped working the moment RFC
# 094 became a real source with a real lane (G13), because the fixture copies
# the real manifest and the two "094" keys then made it invalid TOML. A
# fixture that impersonates a real RFC is a fixture waiting for that RFC to
# become real. The heading is level `###` so every case built on this also
# exercises heading-level-agnostic matching.
make_multi_source_fixture() {
  local target=$1
  make_valid_fixture "$target"
  cat >"$target/rfcs/accepted/900-fixture.md" <<'FIXTURE_RFC'
# RFC 900 — fixture stand-in

Fixture-only stand-in used by scripts/tests/check-gate-inputs-fixtures.sh.
It exists to give the lane registry a second source to resolve.

### Gate Matrix lanes owned by RFC 900

| Lane | Toolchain | Features | Command |
|---|---|---|---|
| G90 | stable | n/a | `bash scripts/fixture-lane.sh` |
FIXTURE_RFC
  sed -i 's|^"093" = "Gate Matrix v1"$|&\n"900" = "Gate Matrix lanes owned by RFC 900"|' \
    "$target/ci/gate-inputs.toml"
  sed -i 's|^G11 = "093"$|&\nG90 = "900"|' "$target/ci/gate-inputs.toml"
  sed -i 's|^G11 = "python3.14 scripts/check-rfc-integrity.py --root . --policy ci/rfc-policy.toml"$|&\nG90 = "bash scripts/fixture-lane.sh"|' \
    "$target/ci/gate-inputs.toml"
}

run_checker() {
  local root=$1
  bash "$checker" --all --policy ci/gate-inputs.toml \
    --workflows-dir .github/workflows \
    --root "$root"
}

expect_failure() {
  local name=$1
  shift
  local fixture="$tmp/$name"
  local output="$tmp/$name.output"
  local expected

  if run_checker "$fixture" >"$output" 2>&1; then
    echo "fixture $name unexpectedly passed" >&2
    cat "$output" >&2
    exit 1
  fi
  # Every expected substring must appear: a registry failure is pinned by
  # both its check number and the lane or count that identifies it, so a
  # case cannot pass by failing the right check about the wrong thing.
  for expected in "$@"; do
    if ! grep -Fq "$expected" "$output"; then
      echo "fixture $name failed for the wrong reason; expected: $expected" >&2
      cat "$output" >&2
      exit 1
    fi
  done
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
# In a hand-written workflow: ci.yml is generated and not read by condition 1
# (RFC 116 stage 3); an unpinned action there is caught by the generator's own
# check, and by condition 2 here.
sed -i 's|uses: actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803 # v6|uses: actions/checkout@v6|' \
  "$unpinned/.github/workflows/audit.yml"
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
# Appended inside the existing [actions] table, anchored on a key unique to
# it. Re-declaring [actions] instead would be a second violation now that the
# manifest is parsed as TOML first, and this fixture would stop reaching
# condition 3 at all.
sed -i '/^setup_python_v7 = /a stale_entry = "222222222222222222222222222222222222222f"' \
  "$stale/ci/gate-inputs.toml"
expect_failure stale-action "condition 3: [actions] SHA(s) not used by any workflow"

# --- Condition 0 (the TOML precheck): a duplicate key in [rust_components] --
# Since R10-b a duplicate key is caught by the TOML precheck, before any
# condition runs. (It was filed under condition 4, which RFC 116 stage 3 moved
# to the workflow generator; the precheck is what still catches this.)
dup_lane="$tmp/rust-components-duplicate"
make_valid_fixture "$dup_lane"
sed -i '/^G01 = \[\]$/a G01 = []' "$dup_lane/ci/gate-inputs.toml"
expect_failure rust-components-duplicate "not valid TOML" "line"

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

# --- Condition 7a: [gates] command diverges from the RFC table -----------
diverged_command="$tmp/gates-diverged-command"
make_valid_fixture "$diverged_command"
sed -i 's|^G02 = "cargo +1.95 test --workspace --locked"$|G02 = "cargo +1.95 test --workspace"|' \
  "$diverged_command/ci/gate-inputs.toml"
expect_failure gates-diverged-command \
  "condition 7 (check 4):" "G02 (owner 093)"

# --- Condition 7b: [gates] missing a lane the RFC table has ---------------
missing_gate="$tmp/gates-missing-lane"
make_valid_fixture "$missing_gate"
sed -i '/^G09b = "cargo +stable test -p sui-id-store/d' "$missing_gate/ci/gate-inputs.toml"
expect_failure gates-missing-lane \
  "condition 7 (check 3):" "G09b"

# --- Condition 7c: [gates] has an extra lane not in the RFC table --------
extra_gate="$tmp/gates-extra-lane"
make_valid_fixture "$extra_gate"
sed -i '/^G09b = "cargo +stable test -p sui-id-store/a G10 = "echo not-a-real-lane"' \
  "$extra_gate/ci/gate-inputs.toml"
expect_failure gates-extra-lane \
  "condition 7 (check 1):" "G10 (0 [gate_owners] entries"

# --- Condition 7d: [gates] duplicate key -----------------------------------
dup_gate="$tmp/gates-duplicate-key"
make_valid_fixture "$dup_gate"
sed -i '/^G01 = "cargo +1.95 build --workspace --all-targets --locked"$/a G01 = "cargo +1.95 build --workspace --all-targets --locked"' \
  "$dup_gate/ci/gate-inputs.toml"
# Caught by the TOML precheck since R10-b, for the same reason as
# rust-components-duplicate above: condition 7's duplicate detector is intact
# but no longer reachable through a manifest.
expect_failure gates-duplicate-key "not valid TOML" "line"

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
# exactly one. The real manifest's exception list is empty (RFC 116 stage 4
# moved G12 out of it), so this case is the only thing that exercises the accept
# path: it moves a lane that normally is *not* excepted.
exception_pass="$tmp/gate-matrix-exception-accepted"
make_valid_fixture "$exception_pass"
sed -i '/^G09b = "cargo/d' "$exception_pass/ci/gate-inputs.toml"
sed -i '/^\[gate_matrix_exceptions\]/a G09b = "fixture: moved to exceptions to test the accept path"' \
  "$exception_pass/ci/gate-inputs.toml"
expect_success gate-matrix-exception-accepted

# --- Condition 7g: a lane in both [gates] and [gate_matrix_exceptions] --
# --- fails (RFC 093 M1b C2.1) -----------------------------------------------
# G13 stays in [gates] and *also* gains an exception entry with a reason -- the
# two lists are supposed to be disjoint by construction, so this must fail even
# though the added reason is not itself wrong. (It used G12, which stopped being
# an exception in RFC 116 stage 4.)
both_lists="$tmp/gate-matrix-exception-and-gates-fails"
make_valid_fixture "$both_lists"
sed -i '/^\[gate_matrix_exceptions\]/a G13 = "fixture: listed as an exception while still in [gates]"' \
  "$both_lists/ci/gate-inputs.toml"
expect_failure gate-matrix-exception-and-gates-fails \
  "condition 7 (check 5):" "G13"

# --- Conditions 4, 6 and 8 -------------------------------------------------
# No fixture here: RFC 116 stage 3 moved them to scripts/generate-ci-workflow.py
# (component sets, runner label, [tools] versions), whose own negative tests are
# scripts/tests/test_generate_ci_workflow.py. What each old fixture proved is
# proved there: a missing or extra [rust_components] lane, a wrong component
# array, a wrong runner label, a [tools] version that drifted or that nothing
# uses, and a version that drifted in one job only.

# ==========================================================================
# RFC 094 R10: the multi-source lane registry.
#
# Every case below mutates the *multi-source* fixture, not the single-source
# one, so a failure is attributable to the check under test rather than to
# the migration. The positive case comes first: without it, a green
# single-source run would prove nothing about the mechanism the registry
# exists to add.
# ==========================================================================

# --- Positive: a second owning RFC is accepted ----------------------------
multi_source="$tmp/registry-multi-source-valid"
make_multi_source_fixture "$multi_source"
expect_success registry-multi-source-valid

# --- Check 1: a [gates] lane with no [gate_owners] entry ------------------
check1="$tmp/registry-check1-unowned-lane"
make_multi_source_fixture "$check1"
sed -i '/^G90 = "900"$/d' "$check1/ci/gate-inputs.toml"
expect_failure registry-check1-unowned-lane \
  "condition 7 (check 1):" "G90 (0 [gate_owners] entries"

# --- Check 2: an owner that is not a declared source ----------------------
check2_unsourced="$tmp/registry-check2-owner-not-a-source"
make_multi_source_fixture "$check2_unsourced"
sed -i '/^"900" = "Gate Matrix lanes owned by RFC 900"$/d' \
  "$check2_unsourced/ci/gate-inputs.toml"
expect_failure registry-check2-owner-not-a-source \
  "condition 7 (check 2):" 'not declared in [gate_lane_sources]'

# --- Check 2: a source whose number resolves to no file ------------------
check2_zero="$tmp/registry-check2-resolves-to-zero"
make_multi_source_fixture "$check2_zero"
find "$check2_zero/rfcs/accepted" -maxdepth 1 -name '900-*.md' -delete
expect_failure registry-check2-resolves-to-zero \
  "condition 7 (check 2):" "resolves to 0 files"

# --- Check 2: a source whose number resolves to two files ----------------
# Zero and several are both failures, never a first-match guess: a number
# resolving to two files is the case a "first match wins" reading would
# silently accept.
check2_two="$tmp/registry-check2-resolves-to-two"
make_multi_source_fixture "$check2_two"
cp "$check2_two/rfcs/accepted/900-fixture.md" \
  "$check2_two/rfcs/archive/900-duplicate.md"
expect_failure registry-check2-resolves-to-two \
  "condition 7 (check 2):" "resolves to 2 files"

# --- Check 3: a source declares a lane the manifest accounts for nowhere --
check3="$tmp/registry-check3-source-lane-unaccounted"
make_multi_source_fixture "$check3"
sed -i '/^G90 = "bash scripts\/fixture-lane.sh"$/d' "$check3/ci/gate-inputs.toml"
expect_failure registry-check3-source-lane-unaccounted \
  "condition 7 (check 3):" "G90"

# --- Check 4: the command drifts in the owning RFC's own table ------------
# Drift is introduced in RFC 900's table, so a checker still comparing every
# lane against RFC 093 would not see it.
check4_drift="$tmp/registry-check4-command-drift-in-owner-table"
make_multi_source_fixture "$check4_drift"
sed -i 's|`bash scripts/fixture-lane.sh`|`bash scripts/fixture-lane.sh --extra`|' \
  "$check4_drift/rfcs/accepted/900-fixture.md"
expect_failure registry-check4-command-drift-in-owner-table \
  "condition 7 (check 4):" "G90 (owner 900)"

# --- Check 4: the one permitted normalisation is not widened -------------
# The permitted normalisation is one-directional: a source RFC may join two
# backticked commands with the word "and" where the manifest joins them with
# "&&". The reverse — a manifest joining with "and" — is not normalised and
# must fail. A bidirectional (widened) implementation would accept it.
#
# The dispatched fixture specified mutating RFC 093's G05 row instead,
# replacing "` and `" with "` && `". Measured: that passes, and cannot fail,
# because backticks are stripped after the substitution, so both spellings
# converge on the same string. Mutating the manifest side is the construction
# that actually discriminates. See the review request.
check4_norm="$tmp/registry-check4-normalisation-not-widened"
make_multi_source_fixture "$check4_norm"
sed -i 's|^G05 = "cargo +stable build --workspace --all-targets --locked && cargo +stable test --workspace --locked"$|G05 = "cargo +stable build --workspace --all-targets --locked and cargo +stable test --workspace --locked"|' \
  "$check4_norm/ci/gate-inputs.toml"
expect_failure registry-check4-normalisation-not-widened \
  "condition 7 (check 4):" "G05 (owner 093)"

# --- Check 5: a lane in both [gates] and [gate_matrix_exceptions] --------
check5="$tmp/registry-check5-both-gates-and-exception"
make_multi_source_fixture "$check5"
sed -i '/^\[gate_matrix_exceptions\]/a G90 = "fixture: excepted while still dispatched, which must fail"' \
  "$check5/ci/gate-inputs.toml"
expect_failure registry-check5-both-gates-and-exception \
  "condition 7 (check 5):" "G90"

# --- Check 6: an exception for a lane no source declares -----------------
check6="$tmp/registry-check6-ungrounded-exception"
make_multi_source_fixture "$check6"
sed -i '/^\[gate_matrix_exceptions\]/a G99 = "fixture: exception naming a lane no source RFC declares"' \
  "$check6/ci/gate-inputs.toml"
expect_failure registry-check6-ungrounded-exception \
  "condition 7 (check 6):" "G99"

# --- Heading: recorded heading absent from the owning RFC ----------------
heading_absent="$tmp/registry-heading-absent"
make_multi_source_fixture "$heading_absent"
sed -i 's|^### Gate Matrix lanes owned by RFC 900$|### A differently named section|' \
  "$heading_absent/rfcs/accepted/900-fixture.md"
expect_failure registry-heading-absent "heading" "occurs 0 times"

# --- Heading: recorded heading occurring twice --------------------------
# Several is a failure, not a first-match-wins guess.
heading_twice="$tmp/registry-heading-twice"
make_multi_source_fixture "$heading_twice"
printf '\n### Gate Matrix lanes owned by RFC 900\n' \
  >>"$heading_twice/rfcs/accepted/900-fixture.md"
expect_failure registry-heading-twice "heading" "occurs 2 times"

# --- Heading: compared by equality, never as a pattern -------------------
# A heading recorded as `Gate Matrix (v2)` matches a document heading of
# exactly that text...
heading_literal="$tmp/registry-heading-parens-literal-pass"
make_multi_source_fixture "$heading_literal"
sed -i 's|^### Gate Matrix lanes owned by RFC 900$|### Gate Matrix (v2)|' \
  "$heading_literal/rfcs/accepted/900-fixture.md"
sed -i 's|^"900" = "Gate Matrix lanes owned by RFC 900"$|"900" = "Gate Matrix (v2)"|' \
  "$heading_literal/ci/gate-inputs.toml"
expect_success registry-heading-parens-literal-pass

# ...and must *not* match `Gate Matrix v2`. This is the discriminating case:
# read as an ERE, `Gate Matrix (v2)` takes the parentheses as a group and so
# matches the text `Gate Matrix v2`. A regex-based matcher passes here; an
# equality-based one finds zero occurrences and fails.
heading_regex="$tmp/registry-heading-parens-not-regex"
make_multi_source_fixture "$heading_regex"
sed -i 's|^### Gate Matrix lanes owned by RFC 900$|### Gate Matrix v2|' \
  "$heading_regex/rfcs/accepted/900-fixture.md"
sed -i 's|^"900" = "Gate Matrix lanes owned by RFC 900"$|"900" = "Gate Matrix (v2)"|' \
  "$heading_regex/ci/gate-inputs.toml"
expect_failure registry-heading-parens-not-regex "heading" "occurs 0 times"

# --- R10-b: the manifest must be valid TOML ------------------------------
# RFC 094 reasons that one RFC number cannot carry two headings because TOML
# forbids the duplicate key. Nothing parsed the manifest as TOML, so it could:
# `Summary` occurs exactly once in RFC 093, so the second entry resolved
# cleanly and every condition passed with exit 0. The precheck is what makes
# the RFC's premise true.
dup_source="$tmp/registry-duplicate-source-key"
make_valid_fixture "$dup_source"
sed -i 's|^"093" = "Gate Matrix v1"$|&\n"093" = "Summary"|' \
  "$dup_source/ci/gate-inputs.toml"
expect_failure registry-duplicate-source-key "not valid TOML" "line"

# One lane with two owners. Check 1's "exactly one" caught this before the
# precheck existed and its logic is unchanged -- it is still the right check
# for a [gates] lane with no owner at all -- but the precheck now reaches the
# file first and names the cause as what it is.
dup_owner="$tmp/registry-duplicate-owner-key"
make_valid_fixture "$dup_owner"
sed -i 's|^G02 = "093"$|&\nG02 = "093"|' "$dup_owner/ci/gate-inputs.toml"
expect_failure registry-duplicate-owner-key "not valid TOML" "line"

echo "gate-inputs negative fixtures passed"
