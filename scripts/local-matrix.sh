#!/usr/bin/env bash
# RFC 093 cycle H item 4 (rfcs/handoffs/093-build-toolchain-release-gates/
# local-matrix-script-2026-10-28.md): "the local matrix" had no tracked
# definition anywhere -- it existed only in whatever each dispatch happened
# to enumerate by hand, and that enumeration was wrong twice in a row (a
# 13-of-14 count, then a 24-lane set that never ran the 14 unittest steps at
# all). This script derives the complete set from `.github/workflows/ci.yml`
# itself, which cannot drift from what CI runs because it IS what CI runs,
# rather than from `contracts/gate-inputs.toml`'s own `[gates]` table, which
# has only the gate commands and none of A3.4, CIGEN, the fixture suites, the
# mdBook negative self-test, or the `python3.14 -m unittest` self-tests
# (including this script's own, once registered).
#
# The 24 `bash scripts/ci-gate.sh <ID>` lines in ci.yml are NOT run as
# written. ci-gate.sh unconditionally asserts `git status --porcelain` is
# empty, and this script's ordinary case is a dirty tree mid-edit -- the
# clean-tree assertion is not inert the way the other one is: ci-gate.sh
# also asserts checked-out HEAD equals $GITHUB_SHA, but only when that
# variable is set, and it normally is not outside CI, so that half is inert
# locally. The clean-tree half is unconditional and is the one that actually
# blocks local use. So each gate's command is instead read directly from
# contracts/gate-inputs.toml's own [gates] table -- the same table ci-gate.sh
# itself reads -- and run here, with the matching [lane_profiles] `tz`
# applied the same way ci-gate.sh applies it, so a TZ-sensitive lane (G02,
# G04, G05, G06, G20) runs under the same zone locally as in CI. This reuses
# ci-gate.sh's own awk reading logic rather than editing that file, which the
# dispatch forbids.
#
# Every other line this script runs is copied, never retyped, from an
# `echo "command=<CMD>"` line in ci.yml -- the same literal text CI itself
# echoes before running each non-gate step. Exactly one such step (the
# mdBook "missing chapter" negative self-test) inverts its own pass
# criterion with an `if [[ "$status" -eq 0 ]]` branch instead of the
# ordinary `exit "$status"` every other step ends with; today that idiom
# (`-eq 0`) appears nowhere else in ci.yml (`grep -n '\-eq 0'
# .github/workflows/ci.yml` returns one line), so this script treats its
# presence anywhere inside a step as the generic signal that step expects a
# non-zero exit, rather than hand-naming that one step. If a second inverted
# fixture is ever added using a different idiom, this detection will miss it
# -- the risk this comment exists to flag for whoever adds one.
#
# Ordering: the gate commands run first, in the order ci.yml lists them --
# G20 among them -- before any of the other, non-gate checks. G20's own command
# (`rm -f target/rfc094-rollback-coverage.txt && cargo ... test -p
# sui-id-store --lib ... && bash scripts/check-m2a-rollback-coverage.sh`)
# deletes and regenerates target/rfc094-rollback-coverage.txt and its
# exactly-once sibling; `test_check_m2a_rollback_coverage`'s own
# `test_the_repository_as_it_stands_passes` reads those same files without
# regenerating them, and on a cold tree (neither file exists yet) that is a
# correctness gap this script closes by construction: running G20 before
# the Python self-tests section guarantees the artefacts exist and are
# fresh for this commit's own code before anything reads them. Do not
# reorder the Python self-tests section ahead of the gates for this reason.
#
# NOT Level B. Level B is every [gates] lane green via `workflow_dispatch`
# on the exact commit being tagged; no local run, by any script, is that.
# `+stable` lanes (G05, G06, G07, G08, G09b, G20) are confirmable locally
# only to the extent the toolchain installed here matches the runner's.
#
# No "fast subset" mode, deliberately: the defect this script exists to fix
# is an incomplete matrix, and an option to run less of it would reintroduce
# exactly that defect with a flag on it.

set -uo pipefail

usage() {
  echo "usage: $0 [--root <path>] [--list]" >&2
  echo "  --root <path>  Repository root (default: .)" >&2
  echo "  --list         Print the derived set of checks and exit; run nothing." >&2
}

root="."
list_only=0
while (($#)); do
  case "$1" in
    --root)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      root=$2
      shift 2
      ;;
    --list)
      list_only=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

cd "$root" || { echo "local-matrix: root not found: $root" >&2; exit 2; }

ci_yml=".github/workflows/ci.yml"
gate_inputs="contracts/gate-inputs.toml"
[[ -f "$ci_yml" ]] || { echo "local-matrix: $ci_yml not found" >&2; exit 2; }
[[ -f "$gate_inputs" ]] || { echo "local-matrix: $gate_inputs not found" >&2; exit 2; }

# ---- [gates] / [lane_profiles] readers, the same awk ci-gate.sh uses ----
# (reused, not shared by sourcing that file, since this script must not
# inherit ci-gate.sh's own clean-tree/GITHUB_SHA assertions; and not edited,
# per the dispatch's explicit instruction not to touch ci-gate.sh).
gate_command() {
  local gate=$1 value
  value=$(awk -v gate="$gate" '
    /^\[gates\]/ { in_gates = 1; next }
    /^\[/ { in_gates = 0 }
    in_gates {
      eq = index($0, " = ")
      if (eq > 0 && substr($0, 1, eq - 1) == gate) { print substr($0, eq + 3); exit }
    }
  ' "$gate_inputs")
  value=${value%\"}
  value=${value#\"}
  printf '%s' "$value"
}

gate_tz() {
  local gate=$1
  awk -v gate="$gate" '
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
  ' "$gate_inputs"
}

# ---- derive the 24 gate ids, in the order ci.yml lists them ----
mapfile -t gate_ids < <(grep -oE 'bash scripts/ci-gate\.sh [A-Za-z0-9]+' "$ci_yml" | awk '{print $NF}')

# ---- derive the other checks: every `echo "command=<CMD>"` line, with its
# polarity (NORMAL = 0 means pass; INVERTED = 0 means fail) detected from
# whether `-eq 0` appears anywhere in that step's own body ----
other_checks=()
while IFS=$'\t' read -r polarity cmd; do
  other_checks+=("$polarity"$'\t'"$cmd")
done < <(awk '
  function flush() {
    if (cmd != "") {
      polarity = (has_eq0 ? "INVERTED" : "NORMAL")
      print polarity "\t" cmd
    }
    cmd = ""
    has_eq0 = 0
  }
  BEGIN { cmd = ""; has_eq0 = 0 }
  /^      - name:/ { flush() }
  {
    if (match($0, /echo "command=.*"/)) {
      line = substr($0, RSTART, RLENGTH)
      sub(/^echo "command=/, "", line)
      sub(/"$/, "", line)
      cmd = line
    }
    if ($0 ~ /-eq 0/) has_eq0 = 1
  }
  END { flush() }
' "$ci_yml")

if [[ "$list_only" -eq 1 ]]; then
  for g in "${gate_ids[@]}"; do
    printf 'GATE\t%s\n' "$g"
  done
  for entry in "${other_checks[@]}"; do
    IFS=$'\t' read -r _polarity cmd <<<"$entry"
    printf 'CMD\t%s\n' "$cmd"
  done
  exit 0
fi

total=0
failed=0
declare -a failed_names=()

run_one() {
  local name=$1 cmd=$2 tz=${3:-} invert=${4:-0}
  total=$((total + 1))
  echo "=== $name ==="
  echo "command=$cmd"
  echo "started_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  local status
  if [[ -n "$tz" ]]; then
    TZ="$tz" bash -c "$cmd"
  else
    bash -c "$cmd"
  fi
  status=$?
  echo "exit_status=$status"
  echo "ended_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  local ok=0
  if [[ "$invert" -eq 1 ]]; then
    [[ "$status" -ne 0 ]] && ok=1
  else
    [[ "$status" -eq 0 ]] && ok=1
  fi
  if [[ "$ok" -eq 1 ]]; then
    echo "result=PASS"
  else
    echo "result=FAIL"
    failed=$((failed + 1))
    failed_names+=("$name")
  fi
}

echo "RFC 093 local matrix -- derived from $ci_yml, not hand-written."
echo "NOT Level B. Level B is every [gates] lane green via workflow_dispatch"
echo "on the exact commit being tagged; this is a local run only."
echo "+stable lanes (G05, G06, G07, G08, G09b, G20) are confirmable locally"
echo "only to the extent this toolchain matches the runner's."
echo

for g in "${gate_ids[@]}"; do
  cmd=$(gate_command "$g")
  if [[ -z "$cmd" ]]; then
    echo "=== $g ==="
    echo "result=FAIL (no [gates] entry for $g in $gate_inputs)"
    total=$((total + 1))
    failed=$((failed + 1))
    failed_names+=("$g")
    continue
  fi
  tz=$(gate_tz "$g")
  run_one "$g" "$cmd" "$tz" 0
done

for entry in "${other_checks[@]}"; do
  IFS=$'\t' read -r polarity cmd <<<"$entry"
  invert=0
  [[ "$polarity" == "INVERTED" ]] && invert=1
  run_one "$cmd" "$cmd" "" "$invert"
done

echo
echo "local-matrix: $total check(s) run, $((total - failed)) passed, $failed failed."
if [[ "$failed" -gt 0 ]]; then
  echo "local-matrix: failed:"
  for n in "${failed_names[@]}"; do
    echo "  - $n"
  done
fi
echo "local-matrix: NOT Level B -- see the banner above."

[[ "$failed" -eq 0 ]]
