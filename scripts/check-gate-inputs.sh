#!/usr/bin/env bash
# RFC 093 A3.4: ci/gate-inputs.toml enforcement.
#
# The manifest records action pins, toolchain components, and (since A3.1)
# the literal per-lane commands the scripts/ci-gate.sh dispatcher executes.
# Until this script existed, nothing read it: it recorded a contract it
# could not defend. This checks all seven conditions A3.4 requires and
# fails closed on the first violation.

set -uo pipefail

usage() {
  echo "usage: $0 --all --policy <path> [--root <path>] [--rfc <path>] [--workflows-dir <path>]" >&2
}

all=false
policy=""
root="."
rfc="rfcs/accepted/093-build-toolchain-release-gates.md"
workflows_dir=".github/workflows"

while (($#)); do
  case "$1" in
    --all)
      all=true
      shift
      ;;
    --policy)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      policy=$2
      shift 2
      ;;
    --root)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      root=$2
      shift 2
      ;;
    --rfc)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      rfc=$2
      shift 2
      ;;
    --workflows-dir)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      workflows_dir=$2
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

[[ "$all" == true && -n "$policy" ]] || { usage; exit 2; }
[[ -f "$root/$policy" ]] || { echo "gate-inputs: manifest not found: $root/$policy" >&2; exit 2; }
[[ -f "$root/$rfc" ]] || { echo "gate-inputs: RFC not found: $root/$rfc" >&2; exit 2; }
[[ -d "$root/$workflows_dir" ]] || { echo "gate-inputs: workflows dir not found: $root/$workflows_dir" >&2; exit 2; }

policy_path="$root/$policy"
rfc_path="$root/$rfc"

failures=0
fail() {
  echo "gate-inputs: $1" >&2
  failures=$((failures + 1))
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# ---------------------------------------------------------------------------
# Conditions 1-3: action pinning and manifest/[actions] correspondence.
# ---------------------------------------------------------------------------

grep -rhoE 'uses:[[:space:]]*[^[:space:]#]+' "$root/$workflows_dir" \
  | sed -E 's/^uses:[[:space:]]*//' >"$tmp/workflow-uses"

# Condition 1: every `uses:` is pinned to a full 40-hex commit SHA.
unpinned=$(grep -vE '@[0-9a-f]{40}$' "$tmp/workflow-uses" || true)
if [[ -n "$unpinned" ]]; then
  fail "condition 1: unpinned action reference(s):"
  echo "$unpinned" >&2
fi

sed -E 's/^.*@([0-9a-f]{40})$/\1/' "$tmp/workflow-uses" | sort -u >"$tmp/workflow-shas"
sed -nE 's/^[a-zA-Z0-9_]+[[:space:]]*=[[:space:]]*"([0-9a-f]{40})"[[:space:]]*$/\1/p' \
  "$policy_path" >"$tmp/manifest-shas-raw"
sort -u "$tmp/manifest-shas-raw" >"$tmp/manifest-shas"

# Condition 2: every workflow SHA appears in [actions].
missing_in_manifest=$(comm -23 "$tmp/workflow-shas" "$tmp/manifest-shas" || true)
if [[ -n "$missing_in_manifest" ]]; then
  fail "condition 2: workflow action SHA(s) not recorded in [actions]:"
  echo "$missing_in_manifest" >&2
fi

# Condition 3: every [actions] SHA is used by at least one workflow (no stale rows).
stale_in_manifest=$(comm -13 "$tmp/workflow-shas" "$tmp/manifest-shas" || true)
if [[ -n "$stale_in_manifest" ]]; then
  fail "condition 3: [actions] SHA(s) not used by any workflow (stale):"
  echo "$stale_in_manifest" >&2
fi

# ---------------------------------------------------------------------------
# Condition 4: [rust_components] declares each lane exactly once, with the
# required component arrays.
# ---------------------------------------------------------------------------

declare -A expected_components=(
  [G01]="" [G02]="" [G03]="" [G04]="" [G05]="" [G06]=""
  [G07]="clippy" [G07b]="clippy" [G08]="rustfmt" [G09a]="" [G09b]=""
)

extract_table_value() {
  # Prints the value of `KEY = ...` inside table $2, or nothing if absent.
  # Exits with a nonzero rc via the `count` echo when the key occurs more
  # than once, so callers can detect duplicates.
  local key=$1 table=$2
  awk -v key="$key" -v table="$table" '
    $0 ~ ("^\\[" table "\\]") { in_table = 1; next }
    /^\[/ { in_table = 0 }
    in_table {
      eq = index($0, " = ")
      if (eq > 0 && substr($0, 1, eq - 1) == key) {
        count++
        print substr($0, eq + 3)
      }
    }
    END { print "COUNT=" (count + 0) }
  ' "$policy_path"
}

for gate in "${!expected_components[@]}"; do
  raw=$(extract_table_value "$gate" "rust_components")
  count=$(printf '%s\n' "$raw" | sed -nE 's/^COUNT=([0-9]+)$/\1/p')
  value=$(printf '%s\n' "$raw" | grep -v '^COUNT=' || true)
  if [[ "$count" -eq 0 ]]; then
    fail "condition 4: [rust_components] is missing $gate"
    continue
  fi
  if [[ "$count" -gt 1 ]]; then
    fail "condition 4: [rust_components] declares $gate more than once ($count times)"
    continue
  fi
  # value looks like: ["clippy"]  or  []
  got=$(printf '%s\n' "$value" | sed -E 's/^\[//; s/\]$//; s/"//g; s/[[:space:]]//g')
  want="${expected_components[$gate]}"
  if [[ "$got" != "$want" ]]; then
    fail "condition 4: [rust_components] $gate = [$value], expected components [\"$want\"] (empty means [])"
  fi
done
# Reject any extra/unexpected key inside [rust_components] too, so a typo'd
# lane name (e.g. "G7") doesn't sit alongside the real one undetected.
awk '
  /^\[rust_components\]/ { in_table = 1; next }
  /^\[/ { in_table = 0 }
  in_table {
    eq = index($0, " = ")
    if (eq > 0) print substr($0, 1, eq - 1)
  }
' "$policy_path" | sort >"$tmp/rust_components-keys"
printf '%s\n' "${!expected_components[@]}" | sort >"$tmp/rust_components-expected"
extra_keys=$(comm -23 "$tmp/rust_components-keys" "$tmp/rust_components-expected" || true)
if [[ -n "$extra_keys" ]]; then
  fail "condition 4: [rust_components] has unexpected key(s):"
  echo "$extra_keys" >&2
fi

# ---------------------------------------------------------------------------
# Condition 5: version and gate_matrix_version are both 1.
# ---------------------------------------------------------------------------

check_top_level_int() {
  local key=$1 want=$2
  local -a values
  mapfile -t values < <(sed -nE "s/^${key} = ([0-9]+)\$/\\1/p" "$policy_path")
  if [[ "${#values[@]}" -ne 1 ]]; then
    fail "condition 5: manifest requires exactly one top-level $key"
    return
  fi
  if [[ "${values[0]}" != "$want" ]]; then
    fail "condition 5: $key = ${values[0]}, expected $want"
  fi
}
check_top_level_int "version" "1"
check_top_level_int "gate_matrix_version" "1"

# ---------------------------------------------------------------------------
# Condition 6: every gate-lane job in ci.yml uses the [runner] label.
# ---------------------------------------------------------------------------

runner_label=$(sed -nE 's/^label = "([^"]*)"$/\1/p' "$policy_path" | head -n1)
if [[ -z "$runner_label" ]]; then
  fail "condition 6: [runner] label not found in manifest"
else
  # A "gate-lane job" is a job whose YAML key is a Gate Matrix ID
  # (G01-G09b, G07b) or the consolidated G12 entry point (ui-invariants-v1).
  # Matched on the job-key line (two-space indent, "key:" at start), not on
  # the free-text "name:" field, so a renamed display name can't hide a
  # missing runner-label check.
  awk -v want="$runner_label" '
    function check_previous_job() {
      # Condition 6 must catch a gate-lane job with no runs-on line at all,
      # not only one whose value is wrong — a job-key transition (or EOF)
      # is where the previous job'"'"'s runs-on line, if any, is now known.
      if (is_gate && !seen_runs_on) {
        print "gate-lane job " job " has no runs-on line"
      }
    }
    /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {
      check_previous_job()
      key = $1
      sub(/:$/, "", key)
      job = key
      is_gate = (key ~ /^G[0-9]+[a-z]?$/) || (key == "ui-invariants-v1")
      seen_runs_on = 0
      next
    }
    is_gate && /^    runs-on:/ {
      seen_runs_on = 1
      line = $0
      sub(/^    runs-on:[[:space:]]*/, "", line)
      if (line != want) {
        print "gate-lane job " job " runs-on " line ", expected " want
      }
    }
    END { check_previous_job() }
  ' "$root/$workflows_dir/ci.yml" >"$tmp/condition6-violations"
  if [[ -s "$tmp/condition6-violations" ]]; then
    fail "condition 6: gate-lane job(s) not using [runner] label ($runner_label):"
    cat "$tmp/condition6-violations" >&2
  fi
fi

# ---------------------------------------------------------------------------
# Condition 7: [gates] matches RFC 093's Gate Matrix v1 table exactly.
# One normalisation is permitted: the RFC renders G05/G06 as two backticked
# commands joined by the word "and"; the manifest joins them with "&&". No
# other normalisation is applied — every other lane compares byte-for-byte.
# ---------------------------------------------------------------------------

# Extract the Gate Matrix v1 table body: from the "## Gate Matrix v1"
# heading to the next heading line. This deliberately excludes the later
# "negative self-tests" table, which also has rows keyed by G09a/G09b but
# with different (fixture) commands in a different column.
awk '
  /^## Gate Matrix v1/ { in_section = 1; next }
  in_section && /^#/ { in_section = 0 }
  in_section { print }
' "$rfc_path" >"$tmp/rfc-matrix-section"

# Parse each `| GNN | toolchain | features | command(s) |` row into
# `GNN<TAB>normalised-command`.
awk -F'|' '
  /^\| G[0-9A-Za-z]+ / {
    id = $2; gsub(/^[[:space:]]+|[[:space:]]+$/, "", id)
    cell = $5; gsub(/^[[:space:]]+|[[:space:]]+$/, "", cell)
    # Strip backticks and, if present, the literal " and " join between two
    # backtick-wrapped commands, replacing it with " && " (the one allowed
    # normalisation).
    # `&` is special in gsub'"'"'s replacement (inserts the matched text),
    # so a literal `&&` must be written as the escaped form below or the
    # match gets duplicated instead of replaced.
    gsub(/` and `/, "` \\&\\& `", cell)
    gsub(/`/, "", cell)
    print id "\t" cell
  }
' "$tmp/rfc-matrix-section" >"$tmp/rfc-gates-all"

# G01-G09b, G10a, G10b and G11 are expected in [gates]. G12 alone uses a
# separate mechanism (ui-invariants-v1) and is never part of this table.
#
# Enumerated one lane at a time rather than as a range: a range such as
# ^G(0[1-9]|1[0-2])[a-z]?$ would also admit G12 and fail on this commit,
# since it is not in [gates].
#
# This set comparison holds both directions over the lanes listed above --
# nothing may appear in [gates] unadmitted, and nothing admitted may be
# dropped. It deliberately says nothing about RFC lanes absent from both
# lists; C2.1 replaces this enumeration with a completeness rule that does.
awk -F'\t' '$1 ~ /^(G0[1-9][a-z]?|G10a|G10b|G11)$/' "$tmp/rfc-gates-all" | sort >"$tmp/rfc-gates"

# Extract [gates] from the manifest as `GNN<TAB>command`, detecting
# duplicate keys within the table (first occurrence is not silently kept —
# a duplicate is itself a failure here, independent of value equality).
awk '
  /^\[gates\]/ { in_table = 1; next }
  /^\[/ { in_table = 0 }
  in_table {
    eq = index($0, " = ")
    if (eq > 0) {
      key = substr($0, 1, eq - 1)
      val = substr($0, eq + 3)
      sub(/^"/, "", val)
      sub(/"$/, "", val)
      print key "\t" val
    }
  }
' "$policy_path" >"$tmp/manifest-gates-raw"

cut -f1 "$tmp/manifest-gates-raw" | sort | uniq -d >"$tmp/manifest-gates-dupes"
if [[ -s "$tmp/manifest-gates-dupes" ]]; then
  fail "condition 7: [gates] has duplicate key(s):"
  cat "$tmp/manifest-gates-dupes" >&2
fi

sort -u "$tmp/manifest-gates-raw" >"$tmp/manifest-gates"

if ! diff -u "$tmp/rfc-gates" "$tmp/manifest-gates" >"$tmp/gates-diff"; then
  fail "condition 7: [gates] does not match RFC 093's Gate Matrix v1 table:"
  cat "$tmp/gates-diff" >&2
fi

# ---------------------------------------------------------------------------

if [[ "$failures" -ne 0 ]]; then
  echo "gate-inputs: failed with $failures violation(s)" >&2
  exit 1
fi

echo "gate-inputs: all conditions satisfied"
