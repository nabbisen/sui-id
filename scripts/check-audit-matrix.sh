#!/usr/bin/env bash
# check-audit-matrix.sh — RFC 085 audit coverage CI gate.
#
# Bidirectional check between docs/src/reference/audit-coverage-matrix.md
# and the Rust source:
#   Forward:  every event name in the matrix must exist in source.
#   Backward: every source literal must have a matrix row.
#
# Exit 0 = pass. Exit 1 = any discrepancy.
# Usage: bash scripts/check-audit-matrix.sh  (from repo root)

set -euo pipefail

MATRIX="docs/src/reference/audit-coverage-matrix.md"
SRC_DIRS="crates"

if [ -t 1 ]; then
  RED='\033[0;31m'; GREEN='\033[0;32m'; RESET='\033[0m'
else
  RED=''; GREEN=''; RESET=''
fi
ok()   { printf "${GREEN}PASS${RESET}  %s\n" "$1"; }
fail() { printf "${RED}FAIL${RESET}  %s\n" "$1"; FAILURES=$((FAILURES + 1)); }

FAILURES=0

# 1. Matrix names: backtick-quoted strings matching audit namespace prefixes
MATRIX_NAMES=$(grep -oE '`[a-z0-9_]+\.[a-z0-9_.]+`' "$MATRIX" \
  | tr -d '`' \
  | grep -E '^(user|client|signing_key|settings|me|auth|admin|oauth2)\.' \
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
  -oE '"(user|client|signing_key|settings|me|auth|admin|oauth2)\.[a-z_.A-Z]+"' \
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
