#!/usr/bin/env bash
# check-audit-matrix.sh — RFC 085 audit coverage CI gate.
#
# Bidirectional check between ci/audit-coverage-matrix.md
# and the Rust source:
#   Forward:  every event name in the matrix must exist in source.
#   Backward: every source literal must have a matrix row.
#
# Which strings count as audit events is derived from the code on every run
# (G13-b, step 0 below) — there is no hand-maintained namespace list.
#
# Exit 0 = pass. Exit 1 = any discrepancy.
# Usage: bash scripts/check-audit-matrix.sh  (from repo root)

set -euo pipefail

MATRIX="ci/audit-coverage-matrix.md"
SRC_DIRS="crates"

if [ -t 1 ]; then
  RED='\033[0;31m'; GREEN='\033[0;32m'; RESET='\033[0m'
else
  RED=''; GREEN=''; RESET=''
fi
ok()   { printf "${GREEN}PASS${RESET}  %s\n" "$1"; }
fail() { printf "${RED}FAIL${RESET}  %s\n" "$1"; FAILURES=$((FAILURES + 1)); }

FAILURES=0

# 0. Audit namespaces, derived from the three places the code writes an audit
#    action (G13-b). The allowlist is the set of first segments found there.
#    A namespace declared in none of them is, by definition, not an audit
#    event — the honest limit of a string gate. The literal group this
#    replaced had to be widened by hand after each miss: `mfa.` on 2026-09-09,
#    then `webauthn.`, `setup.` and `token.` on 2026-09-13.
#      - crates/sui-id-core/src/events.rs    the strings in `name()`'s arms
#      - crates/sui-id-store/src/commands.rs `name: "…"` in the command
#        descriptors (not registry.rs: its proof_only descriptor is a test
#        artefact, not an event)
#      - every non-test .rs under crates/     `action: "…"` in an AuditLogRow
#    Test files (tests/, tests.rs, tests_*.rs) are excluded by name. No inline
#    #[cfg(test)] module writes an `action:` literal today, and the
#    repository's rule is that tests live in sibling files. Missing source
#    files are tolerated so the A3.2 fixture repos derive from their own crate;
#    an empty result fails closed.
EVENTS_RS="$SRC_DIRS/sui-id-core/src/events.rs"
COMMANDS_RS="$SRC_DIRS/sui-id-store/src/commands.rs"
NAMESPACES=$( {
  if [ -f "$EVENTS_RS" ]; then
    sed -n '/pub fn name(/,/^    }/p' "$EVENTS_RS" | grep -oE '=> "[a-z0-9_]+\.' || true
  fi
  if [ -f "$COMMANDS_RS" ]; then
    grep -oE '^[[:space:]]*name:[[:space:]]*"[a-z0-9_]+\.' "$COMMANDS_RS" || true
  fi
  grep -rhoE --include='*.rs' --exclude='tests.rs' --exclude='tests_*.rs' \
    --exclude-dir='tests' 'action:[[:space:]]*"[a-z0-9_]+\.' "$SRC_DIRS" || true
} | sed -E 's/^.*"//; s/\.$//' | sort -u | paste -sd'|' -)

if [ -z "$NAMESPACES" ]; then
  echo "ERROR: no audit namespaces derived from $SRC_DIRS"
  exit 1
fi
echo "Derived audit namespaces: $NAMESPACES"
echo ""

# 1. Matrix names: backtick-quoted strings in a derived audit namespace
MATRIX_NAMES=$(grep -oE '`[a-z0-9_]+\.[a-z0-9_.]+`' "$MATRIX" \
  | tr -d '`' \
  | grep -E "^($NAMESPACES)\." \
  | sort -u)

if [ -z "$MATRIX_NAMES" ]; then
  echo "ERROR: no event names extracted from $MATRIX"
  exit 1
fi

# 2. Source literals: audit-namespaced strings, excluding test-only fixtures
#    (test fixtures use names like "admin.test_action", "admin.should_not_appear",
#    "act.before" etc. that begin with test-specific prefixes or are in cfg(test) blocks)
SRC_LITERALS=$(grep -rh \
  --include="*.rs" \
  -oE "\"($NAMESPACES)\\.[a-z_.A-Z]+\"" \
  "$SRC_DIRS" \
  | tr -d '"' \
  | grep -Ev '\.(test_|should_not_appear|before|after|within|format\()' \
  | grep -v 'tests_rfc085\|state_machine\|test_action' \
  | sort -u)

# 3. Forward check
echo "=== Forward check: matrix → source ==="
while IFS= read -r name; do
  if echo "$SRC_LITERALS" | grep -qxF "$name"; then
    ok "$name"
  else
    fail "$name  (in matrix but NOT found in $SRC_DIRS/**/*.rs)"
  fi
done <<< "$MATRIX_NAMES"

# 4. Backward check
echo ""
echo "=== Backward check: source → matrix ==="
while IFS= read -r name; do
  if echo "$MATRIX_NAMES" | grep -qxF "$name"; then
    ok "$name"
  else
    fail "$name  (in source but NOT in matrix $MATRIX)"
  fi
done <<< "$SRC_LITERALS"

# 5. Summary
echo ""
MATRIX_COUNT=$(echo "$MATRIX_NAMES" | wc -l | tr -d ' ')
SRC_COUNT=$(echo "$SRC_LITERALS" | wc -l | tr -d ' ')
if [ "$FAILURES" -eq 0 ]; then
  printf "${GREEN}audit-matrix gate PASS${RESET}: %d matrix entries, %d source literals.\n" \
    "$MATRIX_COUNT" "$SRC_COUNT"
  exit 0
else
  printf "${RED}audit-matrix gate FAIL${RESET}: %d discrepancies.\n" "$FAILURES"
  exit 1
fi
