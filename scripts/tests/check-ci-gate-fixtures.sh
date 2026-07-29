#!/usr/bin/env bash
# Negative self-tests for scripts/ci-gate.sh's own evidence-contract
# preconditions (not a per-lane build fixture; see
# scripts/tests/check-gate-matrix-fixtures.sh for those).
#
# Both cases here were found and fixed during RFC 093 A3.1's review rounds
# but never had a committed regression fixture. Per the A3.4 review
# (.git-exclude/reviewed/m1a-a3.4-gate-inputs-enforcement-review-2026-07-28.md
# §9), they are "negative fixtures for the evidence contract itself rather
# than for a build lane," carried as A3.2 scope alongside the per-lane set.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
ci_gate="$repo_root/scripts/ci-gate.sh"
manifest="$repo_root/ci/gate-inputs.toml"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# --- Case 1: a non-git --root must fail closed, not silently report a ----
# --- clean tree (B1, A3.1 second round) -----------------------------------
non_git="$tmp/non-git-root"
mkdir -p "$non_git"
output="$tmp/non-git.output"
if bash "$ci_gate" G01 --root "$non_git" --manifest "$manifest" \
  >"$output" 2>&1; then
  echo "ci-gate against a non-git root unexpectedly succeeded" >&2
  cat "$output" >&2
  exit 1
fi
if ! grep -Fq "not a git repository" "$output"; then
  echo "ci-gate against a non-git root failed for the wrong reason" >&2
  cat "$output" >&2
  exit 1
fi
echo "non-git-root: expected failure observed"

# --- Case 2: a metacharacter gate ID must not resolve to another lane's --
# --- command (B2, A3.1 second round) --------------------------------------
# "G0." as a regex/glob would match "G01"; the lookup must compare it as
# an exact string and find nothing.
meta_gate='G0.'
output="$tmp/metacharacter.output"
if bash "$ci_gate" "$meta_gate" --root "$repo_root" --manifest "$manifest" \
  >"$output" 2>&1; then
  echo "ci-gate resolved a metacharacter gate ID instead of rejecting it" >&2
  cat "$output" >&2
  exit 1
fi
if ! grep -Fq "no [gates] entry for $meta_gate" "$output"; then
  echo "ci-gate rejected the metacharacter gate ID for the wrong reason" >&2
  cat "$output" >&2
  exit 1
fi
if grep -q '^command=cargo' "$output"; then
  echo "ci-gate printed a resolved command for a metacharacter gate ID" >&2
  cat "$output" >&2
  exit 1
fi
echo "metacharacter-gate-id: expected rejection observed"

echo "ci-gate evidence-contract fixtures passed"
