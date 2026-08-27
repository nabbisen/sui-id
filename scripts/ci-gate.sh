#!/usr/bin/env bash
# RFC 093 Gate Matrix v1 dispatcher (A3.1, decision D1).
#
# Each gate ID below runs the exact command recorded in `ci/gate-inputs.toml`
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
# for what a gate runs, `ci/gate-inputs.toml` is.

set -uo pipefail

usage() {
  echo "usage: $0 <GATE_ID> [--root <path>] [--manifest <path>]" >&2
  echo "  e.g.  $0 G01" >&2
}

gate="${1:-}"
[[ -n "$gate" ]] || { usage; exit 2; }
shift

root="."
manifest="ci/gate-inputs.toml"
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

echo "command=$command"
echo "started_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
bash -c "$command"
status=$?
echo "exit_status=$status"
echo "ended_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
exit "$status"
