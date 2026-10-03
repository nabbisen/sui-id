#!/usr/bin/env bash
# RFC 094 M2a (G20): every declared write command either reaches the
# Class-A seam, has a registered rollback test, and has registered
# exactly-once evidence -- or is named on the relevant shrink-only
# exemption list.
#
# The dispatch that built the first registry (rfcs/handoffs/
# 094-transactional-audit/m2a-rollback-coverage-2026-10-02.md) is explicit
# that "a name-matching heuristic over test functions is too weak to be a
# gate; an ID appearing in a comment would satisfy it." Neither registry
# looks at test names or comments. Each reads two independent facts:
#
#   1. The declared command set: every `command ID = "ID"` row in
#      commands.rs itself (not a hand-maintained list) -- shared by both
#      registries.
#   2. The *registered* coverage set: a line-per-id file that
#      commands::tests::runner::record_rollback_coverage / record_
#      exactly_once_coverage (crates/sui-id-store/src/commands/tests/
#      runner.rs) appends to -- and every test calls the relevant one
#      only after its own assertions have already passed (rollback: the
#      mutation is absent and no audit row was appended; exactly-once:
#      the audit tail grew by exactly one row). This gate does not run
#      the tests; the gate command (G20 in contracts/gate-inputs.toml)
#      runs the test suite first, clearing both files beforehand so a
#      stale run from an earlier commit cannot be read as today's
#      coverage.
#
# The second registry (exactly-once) was added by rfcs/handoffs/
# 094-transactional-audit/m2a-exit-criteria-divergence-2026-10-03.md §3,
# RFC 094 M2a closure criterion 3: "every converted row has exactly-once
# evidence -- on the success path precisely one audit record, not zero
# and not two." Extending this gate rather than adding a separate one, so
# "every command is covered on both dimensions" stays one script's output.
#
# A bash script, not Python, deliberately: this gate's own job needs the
# Rust toolchain to run the test suite first, and generate-ci-workflow.py
# provisions exactly one of {rust, python} per lane -- bash is already on
# the runner regardless, the same reason check-audit-matrix.sh is bash.
#
# Each exemption list may only name an id that has no registered coverage
# on that dimension; this gate treats an exempted id that already has
# coverage as an error, so the day its test is written, the exemption
# line must be deleted in the same change or the gate fails -- it cannot
# be satisfied by leaving a stale entry in place.
#
# ROLLBACK_EXEMPT_IDS is empty: RFC 094 M2a's eleven missing rollback
# tests (U03, U05, U12, U22, L01-L07) were written in the change that
# added this gate.
ROLLBACK_EXEMPT_IDS=()

# EXACTLY_ONCE_EXEMPT_IDS: empty. All 24 declared commands except U37
# lacked exactly-once evidence when m2a-exit-criteria-divergence-
# 2026-10-03.md was written (U37's `u37_web_issues_a_link_and_writes_
# one_event` already asserted it); its §3 dispatch's remaining 23 landed
# in the same change that extended this gate to check for it.
EXACTLY_ONCE_EXEMPT_IDS=()

# M2A_EXEMPT_IDS_OVERRIDE / M2A_EXACTLY_ONCE_EXEMPT_IDS_OVERRIDE,
# space-separated in the environment, are test-only hooks
# (scripts/tests/test_check_m2a_rollback_coverage.py) to exercise the
# exemption-list logic without editing this file. Unset in every real
# run. Checked with `+x` (is the variable *set*, even to an empty
# string) rather than `-n` (is it *non-empty*), so a test can override
# to deliberately empty -- `-n` would fall through to this script's own
# hardcoded default for that case, which is not what "override" means.
if [[ -n "${M2A_EXEMPT_IDS_OVERRIDE+x}" ]]; then
  # shellcheck disable=SC2206
  ROLLBACK_EXEMPT_IDS=(${M2A_EXEMPT_IDS_OVERRIDE})
fi
if [[ -n "${M2A_EXACTLY_ONCE_EXEMPT_IDS_OVERRIDE+x}" ]]; then
  # shellcheck disable=SC2206
  EXACTLY_ONCE_EXEMPT_IDS=(${M2A_EXACTLY_ONCE_EXEMPT_IDS_OVERRIDE})
fi

# Exit 0 = pass. Exit 1 = any violation, each on stderr, prefixed
# `check-m2a-rollback-coverage:`. Exit 2 = a required input could not be
# read.
# Usage: bash scripts/check-m2a-rollback-coverage.sh --root <path>
#          [--coverage-file <path>] [--exactly-once-coverage-file <path>]

set -euo pipefail

root="."
coverage_rel="target/rfc094-rollback-coverage.txt"
exactly_once_coverage_rel="target/rfc094-exactly-once-coverage.txt"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) root="$2"; shift 2 ;;
    --coverage-file) coverage_rel="$2"; shift 2 ;;
    --exactly-once-coverage-file) exactly_once_coverage_rel="$2"; shift 2 ;;
    *) echo "check-m2a-rollback-coverage: unknown argument: $1" >&2; exit 2 ;;
  esac
done

commands_rs="$root/crates/sui-id-store/src/commands.rs"
seam_root="$root/crates/sui-id-store/src"

if [[ ! -f "$commands_rs" ]]; then
  echo "check-m2a-rollback-coverage: cannot read $commands_rs" >&2
  exit 2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Declared commands, with the left/right ID enforced to match.
declared_file="$tmp/declared"
: > "$declared_file"
while IFS= read -r pair; do
  [[ -z "$pair" ]] && continue
  left=$(grep -oP '(?<=command\s)[A-Z][0-9]+' <<<"$pair")
  right=$(grep -oP '"\K[A-Z][0-9]+(?=")' <<<"$pair")
  if [[ "$left" != "$right" ]]; then
    echo "check-m2a-rollback-coverage: $commands_rs: command declared as $left but string literal is \"$right\"" >&2
    exit 2
  fi
  echo "$left" >> "$declared_file"
done < <(grep -oP '\bcommand\s+[A-Z][0-9]+\s*=\s*"[A-Z][0-9]+"' "$commands_rs" || true)
sort -u -o "$declared_file" "$declared_file"

# Ids reaching the Class-A seam: `ClassATx<'_, ID>` anywhere under seam_root.
seam_file="$tmp/seam"
grep -rhoP "ClassATx<'_,\s*\K[A-Z][0-9]+" "$seam_root" --include='*.rs' 2>/dev/null | sort -u > "$seam_file" || true

failures=0

no_seam=$(comm -23 "$declared_file" "$seam_file")
if [[ -n "$no_seam" ]]; then
  echo "check-m2a-rollback-coverage: declared command(s) with no \`ClassATx<'_, ID>\` site under $seam_root: $(tr '\n' ' ' <<<"$no_seam")" >&2
  failures=1
fi

# check_registry <label> <coverage-path> <exempt-id>...
check_registry() {
  local label="$1" cov_path="$2"
  shift 2
  local exempt_ids=("$@")

  if [[ ! -f "$cov_path" ]]; then
    echo "check-m2a-rollback-coverage: $cov_path does not exist -- the gate command must run the test suite before this script (see G20 in contracts/gate-inputs.toml)" >&2
    exit 2
  fi
  local covered_file="$tmp/${label}-covered"
  grep -ohP '\S+' "$cov_path" | sort -u > "$covered_file" || true

  local exempt_file="$tmp/${label}-exempt"
  : > "$exempt_file"
  if [[ "${#exempt_ids[@]}" -gt 0 ]]; then
    printf '%s\n' "${exempt_ids[@]}" | sort -u > "$exempt_file"
  fi

  local stale
  stale=$(comm -12 "$exempt_file" "$covered_file")
  if [[ -n "$stale" ]]; then
    echo "check-m2a-rollback-coverage: ($label) exempted id(s) already have registered coverage and must be removed from the exemption list: $(tr '\n' ' ' <<<"$stale")" >&2
    failures=1
  fi

  local unknown
  unknown=$(comm -23 "$exempt_file" "$declared_file")
  if [[ -n "$unknown" ]]; then
    echo "check-m2a-rollback-coverage: ($label) exemption list names id(s) not declared in $commands_rs: $(tr '\n' ' ' <<<"$unknown")" >&2
    failures=1
  fi

  local covered_or_exempt_file="$tmp/${label}-covered-or-exempt"
  sort -u "$covered_file" "$exempt_file" > "$covered_or_exempt_file"
  local missing
  missing=$(comm -23 "$declared_file" "$covered_or_exempt_file")
  if [[ -n "$missing" ]]; then
    echo "check-m2a-rollback-coverage: ($label) declared command(s) with neither registered coverage nor an exemption: $(tr '\n' ' ' <<<"$missing")" >&2
    failures=1
  fi

  local covered_count exempt_count
  covered_count=$(grep -c . "$covered_file" || true)
  exempt_count=$(grep -c . "$exempt_file" || true)
  echo "check-m2a-rollback-coverage: ($label) ${covered_count} with registered coverage, ${exempt_count} exempted"
}

check_registry "rollback" "$root/$coverage_rel" "${ROLLBACK_EXEMPT_IDS[@]}"
check_registry "exactly-once" "$root/$exactly_once_coverage_rel" "${EXACTLY_ONCE_EXEMPT_IDS[@]}"

if [[ "$failures" -ne 0 ]]; then
  exit 1
fi

declared_count=$(grep -c . "$declared_file" || true)
echo "check-m2a-rollback-coverage: ${declared_count} declared command(s), both registries satisfied"
