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
rfc="rfcs/done/093-build-toolchain-release-gates.md"
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
# Condition 7: every lane in RFC 093's Gate Matrix v1 table is accounted
# for by exactly one of [gates] or [gate_matrix_exceptions] (RFC 093 M1b
# C2.1's completeness rule), and every lane dispatched via [gates] matches
# the RFC's command exactly. One normalisation is permitted on the command
# comparison: the RFC renders G05/G06 as two backticked commands joined by
# the word "and"; the manifest joins them with "&&". No other
# normalisation is applied — every other lane compares byte-for-byte.
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

# RFC 093 M1b C2.1: the lane-completeness rule. Every lane in the Gate
# Matrix v1 table (rfc-gates-all, 15 rows, already correctly bounded to
# the "## Gate Matrix v1" section above -- see the extraction-hazard
# note there) must be either a [gates] key or a [gate_matrix_exceptions]
# key, never both, never neither.
sort "$tmp/rfc-gates-all" >"$tmp/rfc-gates-all-sorted"
cut -f1 "$tmp/rfc-gates-all-sorted" | sort -u >"$tmp/rfc-lane-ids"

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
cut -f1 "$tmp/manifest-gates" | sort -u >"$tmp/manifest-gate-ids"

# Extract [gate_matrix_exceptions] the same way.
awk '
  /^\[gate_matrix_exceptions\]/ { in_table = 1; next }
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
' "$policy_path" >"$tmp/manifest-exceptions-raw"

cut -f1 "$tmp/manifest-exceptions-raw" | sort | uniq -d >"$tmp/manifest-exceptions-dupes"
if [[ -s "$tmp/manifest-exceptions-dupes" ]]; then
  fail "condition 7: [gate_matrix_exceptions] has duplicate key(s):"
  cat "$tmp/manifest-exceptions-dupes" >&2
fi

sort -u "$tmp/manifest-exceptions-raw" >"$tmp/manifest-exceptions"
cut -f1 "$tmp/manifest-exceptions" | sort -u >"$tmp/manifest-exception-ids"

awk -F'\t' '$2 == "" { print $1 }' "$tmp/manifest-exceptions" >"$tmp/exceptions-empty-reason"
if [[ -s "$tmp/exceptions-empty-reason" ]]; then
  fail "condition 7: [gate_matrix_exceptions] entry(ies) with no reason recorded:"
  cat "$tmp/exceptions-empty-reason" >&2
fi

# A lane not in RFC 093's table has no business being an exception for it.
comm -13 "$tmp/rfc-lane-ids" "$tmp/manifest-exception-ids" >"$tmp/stale-exceptions"
if [[ -s "$tmp/stale-exceptions" ]]; then
  fail "condition 7: [gate_matrix_exceptions] lane(s) not in RFC 093's Gate Matrix v1 table:"
  cat "$tmp/stale-exceptions" >&2
fi

# Disjointness: a lane in both [gates] and [gate_matrix_exceptions] means
# one of the two lists is stale, and the dispatcher and the exemption
# cannot both be true for the same lane.
comm -12 "$tmp/manifest-gate-ids" "$tmp/manifest-exception-ids" >"$tmp/gates-and-exceptions-overlap"
if [[ -s "$tmp/gates-and-exceptions-overlap" ]]; then
  fail "condition 7: lane(s) present in both [gates] and [gate_matrix_exceptions]:"
  cat "$tmp/gates-and-exceptions-overlap" >&2
fi

# Completeness: every RFC lane must be accounted for by one of the two
# lists. This is the inverse direction the enumerated filter never
# checked (M1a C1 review finding; RFC 093 M1b C2.1).
comm -23 "$tmp/rfc-lane-ids" <(sort -u "$tmp/manifest-gate-ids" "$tmp/manifest-exception-ids") >"$tmp/unaccounted-lanes"
if [[ -s "$tmp/unaccounted-lanes" ]]; then
  fail "condition 7: Gate Matrix v1 lane(s) absent from both [gates] and [gate_matrix_exceptions]:"
  cat "$tmp/unaccounted-lanes" >&2
fi

# Command correctness: for every lane actually dispatched via [gates] (by
# now guaranteed disjoint from the exception list and a real RFC lane),
# the recorded command must match the RFC's table byte-for-byte, one
# normalisation permitted (see the extraction comment above).
awk -F'\t' 'NR==FNR { ids[$1] = 1; next } $1 in ids' "$tmp/manifest-gate-ids" "$tmp/rfc-gates-all-sorted" \
  | sort >"$tmp/rfc-gates"

if ! diff -u "$tmp/rfc-gates" "$tmp/manifest-gates" >"$tmp/gates-diff"; then
  fail "condition 7: [gates] does not match RFC 093's Gate Matrix v1 table:"
  cat "$tmp/gates-diff" >&2
fi

# ---------------------------------------------------------------------------
# Condition 8: every [tools] entry corresponds to what .github/workflows/
# ci.yml actually installs or invokes for that tool (RFC 093 M1b C2.1 --
# an M1a-era gap: mdBook's pinned version specifically was enforced by
# nothing, while ci.yml carried a comment claiming otherwise). rust_msrv
# and python happen to also be checked transitively, since the gate
# commands embed them and condition 7 compares those; mdbook has no such
# transitive coverage, since its version never appears in a [gates]
# command. All four are checked the same way here regardless, so none of
# them depends on a coincidence of some other condition's coverage.
# ---------------------------------------------------------------------------

get_toml_value() {
  local key=$1 table=$2
  awk -v key="$key" -v table="$table" '
    $0 ~ ("^\\[" table "\\]") { in_table = 1; next }
    /^\[/ { in_table = 0 }
    in_table {
      eq = index($0, " = ")
      if (eq > 0 && substr($0, 1, eq - 1) == key) {
        val = substr($0, eq + 3)
        sub(/^"/, "", val)
        sub(/"$/, "", val)
        print val
        exit
      }
    }
  ' "$policy_path"
}

check_tool_pin() {
  local tool=$1 expected=$2 grep_pattern=$3 extract_pattern=$4
  local found
  # Every occurrence must equal the pin, not merely include it among
  # several -- "the pin is one of the values found" would accept a
  # version drifted in one job but not another, which is exactly the
  # "moves under a pin nothing reads" failure this condition exists to
  # catch. Comment lines are excluded first so a stray commented-out
  # example (or a future one) cannot false-positive under this stricter
  # all-must-match rule.
  found=$(grep -vE '^[[:space:]]*#' "$root/$workflows_dir/ci.yml" \
    | grep -oE "$grep_pattern" \
    | sed -E "$extract_pattern" | sort -u)
  if [[ -z "$found" ]]; then
    fail "condition 8: [tools] $tool = \"$expected\" not found anywhere in $workflows_dir/ci.yml (declared but unused, or not mechanically locatable)"
    return
  fi
  if [[ "$found" != "$expected" ]]; then
    fail "condition 8: [tools] $tool = \"$expected\", but $workflows_dir/ci.yml installs/invokes: $(echo "$found" | tr '\n' ' ')"
  fi
}

tools_rust_msrv=$(get_toml_value "rust_msrv" "tools")
tools_rust_stable=$(get_toml_value "rust_stable" "tools")
tools_mdbook=$(get_toml_value "mdbook" "tools")
tools_python=$(get_toml_value "python" "tools")

check_tool_pin "rust_msrv" "$tools_rust_msrv" \
  'toolchain: "[0-9]+\.[0-9]+"' 's/toolchain: "([0-9.]+)"/\1/'
check_tool_pin "rust_stable" "$tools_rust_stable" \
  'toolchain: [a-z]+' 's/toolchain: //'
check_tool_pin "mdbook" "$tools_mdbook" \
  'mdbook --version [0-9]+\.[0-9]+\.[0-9]+' 's/mdbook --version //'
check_tool_pin "python" "$tools_python" \
  'python-version: "[0-9]+\.[0-9]+"' 's/python-version: "([0-9.]+)"/\1/'

# ---------------------------------------------------------------------------

if [[ "$failures" -ne 0 ]]; then
  echo "gate-inputs: failed with $failures violation(s)" >&2
  exit 1
fi

echo "gate-inputs: all conditions satisfied"
