#!/usr/bin/env bash
# RFC 094 M2a (G20): every declared write command either reaches the
# Class-A seam with a registered rollback test, or is named on an
# explicit, shrink-only exemption list.
#
# The dispatch this closes (rfcs/handoffs/094-transactional-audit/
# m2a-rollback-coverage-2026-10-02.md) is explicit that "a name-matching
# heuristic over test functions is too weak to be a gate; an ID appearing
# in a comment would satisfy it." This gate does not look at test names
# or comments at all. It reads two independent facts instead:
#
#   1. The declared command set: every `command ID = "ID"` row in
#      commands.rs itself (not a hand-maintained list).
#   2. The *registered* coverage set: target/rfc094-rollback-coverage.txt,
#      a line-per-id file that commands::tests::runner::record_rollback_
#      coverage (crates/sui-id-store/src/commands/tests/runner.rs)
#      appends to -- and every rollback test calls this only after its
#      own assertions (the mutation is absent, no audit row was
#      appended) have already passed. This gate does not run the tests;
#      the gate command (G20 in contracts/gate-inputs.toml) runs the
#      test suite first, clearing the file beforehand so a stale run
#      from an earlier commit cannot be read as today's coverage.
#
# A bash script, not Python, deliberately: this gate's own job needs the
# Rust toolchain to run the test suite first, and generate-ci-workflow.py
# provisions exactly one of {rust, python} per lane -- bash is already on
# the runner regardless, the same reason check-audit-matrix.sh is bash.
#
# An exempted id may only be named here when it has no registered
# coverage; this gate treats an exempted id that already has coverage as
# an error, so the day its test is written, the exemption line must be
# deleted in the same change or the gate fails -- it cannot be satisfied
# by leaving a stale entry in place. The list is empty today: RFC 094
# M2a's eleven missing rollback tests (U03, U05, U12, U22, L01-L07) were
# written in the same change that added this gate. Add an id here only
# with a dispatch document to cite beside it.
EXEMPT_IDS=()

# M2A_EXEMPT_IDS_OVERRIDE, space-separated in the environment, is a
# test-only hook (scripts/tests/test_check_m2a_rollback_coverage.py) to
# exercise the exemption-list logic without editing this file. Unset in
# every real run.
if [[ -n "${M2A_EXEMPT_IDS_OVERRIDE:-}" ]]; then
  # shellcheck disable=SC2206
  EXEMPT_IDS=(${M2A_EXEMPT_IDS_OVERRIDE})
fi

# Exit 0 = pass. Exit 1 = any violation, each on stderr, prefixed
# `check-m2a-rollback-coverage:`. Exit 2 = a required input could not be
# read.
# Usage: bash scripts/check-m2a-rollback-coverage.sh --root <path> [--coverage-file <path>]

set -euo pipefail

root="."
coverage_rel="target/rfc094-rollback-coverage.txt"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) root="$2"; shift 2 ;;
    --coverage-file) coverage_rel="$2"; shift 2 ;;
    *) echo "check-m2a-rollback-coverage: unknown argument: $1" >&2; exit 2 ;;
  esac
done

commands_rs="$root/crates/sui-id-store/src/commands.rs"
seam_root="$root/crates/sui-id-store/src"
cov_path="$root/$coverage_rel"

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

if [[ ! -f "$cov_path" ]]; then
  echo "check-m2a-rollback-coverage: $cov_path does not exist -- the gate command must run the test suite before this script (see G20 in contracts/gate-inputs.toml)" >&2
  exit 2
fi
covered_file="$tmp/covered"
grep -ohP '\S+' "$cov_path" | sort -u > "$covered_file" || true

exempt_file="$tmp/exempt"
: > "$exempt_file"
if [[ "${#EXEMPT_IDS[@]}" -gt 0 ]]; then
  printf '%s\n' "${EXEMPT_IDS[@]}" | sort -u > "$exempt_file"
fi

failures=0

stale=$(comm -12 "$exempt_file" "$covered_file")
if [[ -n "$stale" ]]; then
  echo "check-m2a-rollback-coverage: exempted id(s) already have a registered rollback test and must be removed from the exemption list: $(tr '\n' ' ' <<<"$stale")" >&2
  failures=1
fi

unknown=$(comm -23 "$exempt_file" "$declared_file")
if [[ -n "$unknown" ]]; then
  echo "check-m2a-rollback-coverage: exemption list names id(s) not declared in $commands_rs: $(tr '\n' ' ' <<<"$unknown")" >&2
  failures=1
fi

no_seam=$(comm -23 "$declared_file" "$seam_file")
if [[ -n "$no_seam" ]]; then
  echo "check-m2a-rollback-coverage: declared command(s) with no \`ClassATx<'_, ID>\` site under $seam_root: $(tr '\n' ' ' <<<"$no_seam")" >&2
  failures=1
fi

covered_or_exempt_file="$tmp/covered_or_exempt"
sort -u "$covered_file" "$exempt_file" > "$covered_or_exempt_file"
missing=$(comm -23 "$declared_file" "$covered_or_exempt_file")
if [[ -n "$missing" ]]; then
  echo "check-m2a-rollback-coverage: declared command(s) with neither a registered rollback test nor an exemption: $(tr '\n' ' ' <<<"$missing")" >&2
  failures=1
fi

if [[ "$failures" -ne 0 ]]; then
  exit 1
fi

declared_count=$(grep -c . "$declared_file" || true)
covered_count=$(grep -c . "$covered_file" || true)
exempt_count=$(grep -c . "$exempt_file" || true)
echo "check-m2a-rollback-coverage: ${declared_count} declared command(s), ${covered_count} with a registered rollback test, ${exempt_count} exempted"
