#!/usr/bin/env bash
# RFC 093 A3.2 negative self-tests: prove each of G01-G08 and G07b fails on
# a deliberately invalid fixture and passes on a clean one, driven through
# the real dispatcher (scripts/ci-gate.sh) rather than by running cargo
# directly, so this also exercises the [gates] command strings themselves.
#
# ci-gate.sh's own evidence-contract preconditions require --root to be a
# git repository with a clean tree (it resolves HEAD and asserts no
# uncommitted changes before running the gate command). Each fixture is
# therefore copied into a throwaway directory and given its own one-commit
# git history, isolated from the real repository, before being driven
# through ci-gate.sh.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
fixtures_root="$repo_root/scripts/tests/fixtures/gate-matrix"
ci_gate="$repo_root/scripts/ci-gate.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# stage_fixture <name> -> prints the path to a self-contained git repo
# copy of scripts/tests/fixtures/gate-matrix/<name>, committed so ci-gate.sh's
# clean-tree precondition passes and the gate command itself is what's
# being observed to fail (or pass).
stage_fixture() {
  local name=$1
  local dest="$tmp/$name"
  [[ -d "$dest" ]] && { echo "$dest"; return; }
  mkdir -p "$dest"
  cp -r "$fixtures_root/$name"/. "$dest/"
  rm -rf "$dest/target"
  # A fixture is reused across several gates (e.g. compile-error against
  # G01..G07b); cargo leaves target/ behind after the first run, which
  # would otherwise dirty the tree for the next gate's precondition check.
  echo "/target" >"$dest/.gitignore"
  git -C "$dest" -c init.defaultBranch=main init -q
  git -C "$dest" -c user.email=fixture@example.invalid -c user.name=fixture \
    add -A
  git -C "$dest" -c user.email=fixture@example.invalid -c user.name=fixture \
    commit -q -m "gate-matrix fixture: $name"
  echo "$dest"
}

run_gate() {
  local gate=$1
  local fixture_dir=$2
  # ci-gate.sh cd's into --root before resolving --manifest, so the real
  # manifest (the fixture has none of its own) must be passed as an
  # absolute path.
  #
  # GITHUB_SHA must be bound to the staged fixture's own HEAD, not the real
  # repository's (R9): ci-gate.sh asserts HEAD == GITHUB_SHA as an
  # evidence-binding precondition, and on a hosted runner GITHUB_SHA is
  # always set to the checked-out repository's commit, which is never the
  # fixture's throwaway one-commit history. Locally GITHUB_SHA is normally
  # unset, which skips the assertion entirely and hid this until the first
  # hosted run.
  GITHUB_SHA=$(git -C "$fixture_dir" rev-parse HEAD) \
    bash "$ci_gate" "$gate" --root "$fixture_dir" \
    --manifest "$repo_root/ci/gate-inputs.toml"
}

expect_gate_fails() {
  local gate=$1
  local name=$2
  local fixture_dir
  fixture_dir=$(stage_fixture "$name")
  local output="$tmp/$gate-$name.output"
  if run_gate "$gate" "$fixture_dir" >"$output" 2>&1; then
    echo "$gate against $name unexpectedly passed" >&2
    cat "$output" >&2
    exit 1
  fi
  # A non-zero exit is not on its own evidence that the fixture's deliberate
  # defect was what failed (R9): ci-gate.sh's own preconditions can also
  # exit non-zero, before the gate command ever ran. Require proof the
  # command was actually reached and ran, and that no precondition error
  # fired.
  if ! grep -q '^exit_status=' "$output"; then
    echo "$gate against $name failed before the gate command ran (no exit_status= recorded)" >&2
    cat "$output" >&2
    exit 1
  fi
  if grep -q '::error::ci-gate' "$output"; then
    echo "$gate against $name failed on a ci-gate precondition, not the fixture's defect" >&2
    cat "$output" >&2
    exit 1
  fi
  echo "$gate against $name: expected failure observed"
}

expect_gate_passes() {
  local gate=$1
  local name=$2
  local fixture_dir
  fixture_dir=$(stage_fixture "$name")
  local output="$tmp/$gate-$name.output"
  if ! run_gate "$gate" "$fixture_dir" >"$output" 2>&1; then
    echo "$gate against $name unexpectedly failed" >&2
    cat "$output" >&2
    exit 1
  fi
  echo "$gate against $name: pass"
}

# --- compile-error: fails everything that compiles, passes fmt-only G08 --
for gate in G01 G02 G03 G04 G05 G06 G07 G07b; do
  expect_gate_fails "$gate" compile-error
done
expect_gate_passes G08 compile-error

# --- failing-test: fails anything that runs tests; build-only and lint- --
# --- only lanes must not even notice a failing test assertion ----------
for gate in G02 G04 G05 G06; do
  expect_gate_fails "$gate" failing-test
done
for gate in G01 G03 G07 G07b G08; do
  expect_gate_passes "$gate" failing-test
done

# --- lint-warning: fails clippy only, in both feature configurations ----
for gate in G07 G07b; do
  expect_gate_fails "$gate" lint-warning
done
for gate in G01 G02 G03 G04 G05 G06 G08; do
  expect_gate_passes "$gate" lint-warning
done

# --- lint-warning-feature-gated: the G07-vs-G07b proof pair -------------
# The violation lives behind #[cfg(not(feature = "ldap"))]. G07 runs
# --all-features, so "ldap" is active and the violation is never compiled:
# G07 must pass. G07b runs default features only, so "ldap" is inactive
# and the violation is compiled and linted: G07b must fail. This pair is
# the actual proof the Handoff requires for G07b's existence.
expect_gate_passes G07 lint-warning-feature-gated
expect_gate_fails G07b lint-warning-feature-gated

# --- format-drift: fails fmt only; every compiling/linting lane is fine -
expect_gate_fails G08 format-drift
for gate in G01 G02 G03 G04 G05 G06 G07 G07b; do
  expect_gate_passes "$gate" format-drift
done

echo "gate-matrix negative fixtures passed"
