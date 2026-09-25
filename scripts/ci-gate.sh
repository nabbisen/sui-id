#!/usr/bin/env bash
# RFC 093 Gate Matrix v1 dispatcher (A3.1, decision D1).
#
# Each gate ID below runs the exact command recorded in `contracts/gate-inputs.toml`
# under `[gates]`, which is the machine-readable expansion of RFC 093's Gate
# Matrix v1 table (rfcs/done/093-build-toolchain-release-gates.md). This
# script owns the environment/evidence block once — resolve HEAD, assert it
# equals $GITHUB_SHA, assert a clean tree, print tool versions, echo the
# literal command, then run it capturing exit status and timestamps — so
# every lane in ci.yml is `checkout` + `bash scripts/ci-gate.sh <GATE_ID>`
# rather than duplicating that block per lane.
#
# scripts/check-gate-inputs.sh (A3.4) verifies `[gates]` matches the RFC's
# table exactly, which is what makes this indirection a checked invariant
# rather than a convention: this script is not itself the source of truth
# for what a gate runs, `contracts/gate-inputs.toml` is.

set -uo pipefail

usage() {
  echo "usage: $0 <GATE_ID> [--root <path>] [--manifest <path>]" >&2
  echo "  e.g.  $0 G01" >&2
}

gate="${1:-}"
[[ -n "$gate" ]] || { usage; exit 2; }
shift

root="."
manifest="contracts/gate-inputs.toml"
while (($#)); do
  case "$1" in
    --root)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      root=$2
      shift 2
      ;;
    --manifest)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      manifest=$2
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

cd "$root" || { echo "ci-gate: root not found: $root" >&2; exit 2; }
[[ -f "$manifest" ]] || { echo "ci-gate: manifest not found: $manifest" >&2; exit 2; }

# Scope to the [gates] table only: G01-G09b also appear as keys in
# [rust_components], with different (non-command) values, so a search over
# the whole file can match the wrong table. The key is compared exactly
# (substr(...) == gate), not as a pattern, so a metacharacter in $gate
# (e.g. "G0.") cannot match a different row by accident — awk's `exit`
# after the first hit also means a duplicate [gates] key would silently
# resolve to the first occurrence; A3.4 adds a duplicate-key check.
gate_value=$(awk -v gate="$gate" '
  /^\[gates\]/ { in_gates = 1; next }
  /^\[/ { in_gates = 0 }
  in_gates {
    eq = index($0, " = ")
    if (eq > 0 && substr($0, 1, eq - 1) == gate) { print substr($0, eq + 3); exit }
  }
' "$manifest")
[[ -n "$gate_value" ]] || {
  echo "ci-gate: no [gates] entry for $gate in $manifest" >&2
  exit 2
}
command=${gate_value%\"}
command=${command#\"}
[[ -n "$command" ]] || {
  echo "ci-gate: empty command for $gate in $manifest" >&2
  exit 2
}

# RFC 116 stage 3b (D7): a lane's `tz`, if its [lane_profiles] entry names one,
# is exported to the command, so a lane runs locally with the environment CI runs
# it in. It used to be set in the workflow's per-job `env:` alone, which made G02,
# G04, G05 and G06 differ between a laptop and a runner. The workflow no longer
# sets it (scripts/generate-ci-workflow.py does not emit it): this is the one place.
# Read like [gates]: a single-line inline table, compared by exact key, and the
# table's own header ends the scan.
lane_tz=$(awk -v gate="$gate" '
  /^\[lane_profiles\]/ { in_profiles = 1; next }
  /^\[/ { in_profiles = 0 }
  in_profiles {
    eq = index($0, " = ")
    if (eq > 0 && substr($0, 1, eq - 1) == gate) {
      if (match($0, /[{,][[:space:]]*tz = "[^"]*"/)) {
        value = substr($0, RSTART, RLENGTH)
        sub(/^.*tz = "/, "", value)
        sub(/"$/, "", value)
        print value
      }
      exit
    }
  }
' "$manifest")
if [[ -n "$lane_tz" && ! "$lane_tz" =~ ^[A-Za-z0-9_+:/-]+$ ]]; then
  echo "ci-gate: $gate: [lane_profiles] tz is not a plain timezone name: $lane_tz" >&2
  exit 2
fi

# RFC 116 stage 4: a lane whose [lane_profiles] entry says `bash = true` requires
# the [runner] Bash range, and this dispatcher asserts it before the command runs
# (G12 used to assert it in a workflow step of its own, which is why G12 could not
# be run here). Read like `tz` above and like [runner]: single-line, exact keys.
lane_needs_bash=$(awk -v gate="$gate" '
  /^\[lane_profiles\]/ { in_profiles = 1; next }
  /^\[/ { in_profiles = 0 }
  in_profiles {
    eq = index($0, " = ")
    if (eq > 0 && substr($0, 1, eq - 1) == gate) {
      if ($0 ~ /[{,][[:space:]]*bash = true/) print "yes"
      exit
    }
  }
' "$manifest")
runner_value() {
  awk -v key="$1" '
    /^\[runner\]/ { in_runner = 1; next }
    /^\[/ { in_runner = 0 }
    in_runner {
      eq = index($0, " = ")
      if (eq > 0 && substr($0, 1, eq - 1) == key) {
        value = substr($0, eq + 3)
        gsub(/^"|"$/, "", value)
        print value
        exit
      }
    }
  ' "$manifest"
}

echo "gate=$gate"

if ! checked_out_sha=$(git rev-parse HEAD 2>/dev/null); then
  echo "::error::ci-gate $gate: not a git repository (cannot bind evidence to a commit)" >&2
  exit 1
fi
echo "event_commit=${GITHUB_SHA:-$checked_out_sha}"
echo "checked_out_commit=$checked_out_sha"
if [[ -n "${GITHUB_SHA:-}" && "$checked_out_sha" != "$GITHUB_SHA" ]]; then
  echo "::error::ci-gate $gate: checked-out HEAD does not match GITHUB_SHA" >&2
  exit 1
fi
if ! porcelain=$(git status --porcelain 2>/dev/null); then
  echo "::error::ci-gate $gate: git status failed (cannot assert a clean tree)" >&2
  exit 1
fi
if [[ -n "$porcelain" ]]; then
  echo "::error::ci-gate $gate: working tree is dirty" >&2
  exit 1
fi

echo "runner_image=${ImageOS:-unknown} ${ImageVersion:-unknown}"
if command -v rustc >/dev/null 2>&1; then
  echo "rustc_version=$(rustc -Vv 2>&1 | tr '\n' ';')"
fi
if command -v cargo >/dev/null 2>&1; then
  echo "cargo_version=$(cargo -V 2>&1)"
fi

if [[ "$lane_needs_bash" == "yes" ]]; then
  bash_min=$(runner_value bash_minimum)
  bash_max=$(runner_value bash_maximum_exclusive)
  if [[ ! "$bash_min" =~ ^[0-9]+\.[0-9]+$ || ! "$bash_max" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
    echo "::error::ci-gate $gate: [runner] bash_minimum/bash_maximum_exclusive missing or malformed in $manifest" >&2
    exit 2
  fi
  echo "bash_path=$(command -v bash)"
  echo "bash_version=${BASH_VERSION}"
  have=$((BASH_VERSINFO[0] * 1000 + BASH_VERSINFO[1]))
  low=$((${bash_min%%.*} * 1000 + ${bash_min##*.}))
  if [[ "$bash_max" == *.* ]]; then
    high=$((${bash_max%%.*} * 1000 + ${bash_max##*.}))
  else
    high=$((bash_max * 1000))
  fi
  if ((have < low || have >= high)); then
    echo "::error::ci-gate $gate: requires Bash >=$bash_min,<$bash_max; this is ${BASH_VERSION}" >&2
    exit 1
  fi
fi
if [[ -n "$lane_tz" ]]; then
  export TZ="$lane_tz"
  echo "tz=$lane_tz"
fi
echo "command=$command"
echo "started_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
bash -c "$command"
status=$?
echo "exit_status=$status"
echo "ended_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
exit "$status"
