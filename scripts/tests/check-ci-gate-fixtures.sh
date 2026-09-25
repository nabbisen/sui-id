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

# --- Cases 3-5: a lane's `tz` is exported by the dispatcher (RFC 116 stage 3b) --
# It used to live only in the workflow's per-job env, so G02, G04, G05 and G06
# ran under a different TZ on a laptop than on a runner. The dispatcher needs a
# clean git tree, so each case is a one-commit throwaway repository; the lane's
# command prints the TZ it sees. `env -u TZ` keeps the caller's own TZ out of it.
tz_repo() {
  local dir=$1
  mkdir -p "$dir"
  git -C "$dir" -c init.defaultBranch=main init -q
  echo x >"$dir/f"
  git -C "$dir" -c user.email=f@example.invalid -c user.name=f add -A
  git -C "$dir" -c user.email=f@example.invalid -c user.name=f commit -q -m fixture
}
run_tz_case() {
  local gate=$1 dir=$2 mf=$3
  env -u TZ GITHUB_SHA="$(git -C "$dir" rev-parse HEAD)" \
    bash "$ci_gate" "$gate" --root "$dir" --manifest "$mf"
}

# Case 3: a synthetic manifest. GT1 names a tz, GT2 does not, and a tz on a
# *different* lane must not leak onto either.
tz_dir="$tmp/tz-synthetic"
tz_repo "$tz_dir"
cat >"$tmp/tz-manifest.toml" <<'TOML'
version = 1

[gates]
GT1 = "echo seen-tz:${TZ-unset}"
GT2 = "echo seen-tz:${TZ-unset}"
GT3 = "echo seen-tz:${TZ-unset}"

[lane_profiles]
GT1 = { title = "with a tz", setup = "none", tz = "Asia/Tokyo" }
GT2 = { title = "without", setup = "none" }
GT3 = { title = "title mentions tz = \"UTC\" but is not one", setup = "none" }

[actions]
TOML
output="$tmp/tz-with.output"
run_tz_case GT1 "$tz_dir" "$tmp/tz-manifest.toml" >"$output" 2>&1
grep -Fxq 'tz=Asia/Tokyo' "$output" && grep -Fxq 'seen-tz:Asia/Tokyo' "$output" || {
  echo "ci-gate did not export the lane's tz to its command" >&2; cat "$output" >&2; exit 1; }
echo "tz-exported: the command saw TZ=Asia/Tokyo"
output="$tmp/tz-without.output"
run_tz_case GT2 "$tz_dir" "$tmp/tz-manifest.toml" >"$output" 2>&1
grep -Fxq 'seen-tz:unset' "$output" && ! grep -q '^tz=' "$output" || {
  echo "ci-gate set a TZ for a lane whose profile names none" >&2; cat "$output" >&2; exit 1; }
echo "tz-absent: a lane with no tz runs with TZ unset"
output="$tmp/tz-title.output"
run_tz_case GT3 "$tz_dir" "$tmp/tz-manifest.toml" >"$output" 2>&1
grep -Fxq 'seen-tz:unset' "$output" || {
  echo "ci-gate read a tz out of a title" >&2; cat "$output" >&2; exit 1; }
echo "tz-title: text inside a title is not a tz"

# Case 4: the real manifest's own profile lines. Only the command is replaced
# (cargo would take minutes); the [lane_profiles] lines are the shipped ones, so
# this is the dispatcher reading the tz the workflow used to set.
real_dir="$tmp/tz-real"
tz_repo "$real_dir"
for lane in G02 G04 G05 G06; do
  sed -E "s|^$lane = \"[^\"]*\"\$|$lane = \"echo seen-tz:\${TZ-unset}\"|" "$manifest" >"$tmp/tz-real-$lane.toml"
  output="$tmp/tz-real-$lane.output"
  run_tz_case "$lane" "$real_dir" "$tmp/tz-real-$lane.toml" >"$output" 2>&1
  grep -Fxq 'seen-tz:UTC' "$output" || {
    echo "the shipped profile for $lane did not reach its command as TZ=UTC" >&2; cat "$output" >&2; exit 1; }
done
echo "tz-real-manifest: G02, G04, G05 and G06 run under TZ=UTC"
sed -E 's|^G01 = "[^"]*"$|G01 = "echo seen-tz:${TZ-unset}"|' "$manifest" >"$tmp/tz-real-G01.toml"
run_tz_case G01 "$real_dir" "$tmp/tz-real-G01.toml" >"$tmp/tz-real-G01.output" 2>&1
grep -Fxq 'seen-tz:unset' "$tmp/tz-real-G01.output" || {
  echo "G01 has no tz in its profile but ran with one" >&2; cat "$tmp/tz-real-G01.output" >&2; exit 1; }
echo "tz-real-manifest: G01 runs with TZ unset"

# Case 5: a tz that is not a plain timezone name is refused, not exported.
sed 's|tz = "Asia/Tokyo"|tz = "UTC; touch pwned"|' "$tmp/tz-manifest.toml" >"$tmp/tz-bad.toml"
output="$tmp/tz-bad.output"
if run_tz_case GT1 "$tz_dir" "$tmp/tz-bad.toml" >"$output" 2>&1; then
  echo "ci-gate accepted a tz that is not a plain timezone name" >&2; cat "$output" >&2; exit 1
fi
grep -Fq 'not a plain timezone name' "$output" && [[ ! -e "$tz_dir/pwned" ]] || {
  echo "ci-gate rejected the bad tz for the wrong reason" >&2; cat "$output" >&2; exit 1; }
echo "tz-unsafe: rejected"

echo "ci-gate evidence-contract fixtures passed"
