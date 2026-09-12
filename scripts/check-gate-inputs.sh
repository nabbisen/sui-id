#!/usr/bin/env bash
# RFC 093 A3.4: ci/gate-inputs.toml enforcement.
#
# The manifest records action pins, toolchain components, and (since A3.1)
# the literal per-lane commands the scripts/ci-gate.sh dispatcher executes.
# Until this script existed, nothing read it: it recorded a contract it
# could not defend.
#
# Nine things, in the order they run; all but the first accumulate and are
# reported together:
#
#   0. precheck: the manifest parses as TOML (R10-b);
#   1. every `uses:` in the workflows is pinned to a 40-hex commit SHA;
#   2. every workflow action SHA is recorded in [actions];
#   3. every [actions] SHA is used by some workflow -- no stale rows;
#   4. [rust_components] declares each toolchain lane with the components
#      it expects, and carries no key that is not a lane;
#   5. version and gate_matrix_version are both 1;
#   6. every gate-lane job in ci.yml runs on the [runner] label;
#   7. the multi-source lane registry (RFC 094 R10) -- six checks, plus the
#      rule that every [gate_matrix_exceptions] entry records a reason;
#   8. every [tools] version is the one ci.yml installs or invokes.
#
# The count was wrong from A3.4 until 2026-09-12: this comment claimed
# "all seven conditions" while the script ran eight, and then a precheck.

set -uo pipefail

usage() {
  echo "usage: $0 --all --policy <path> [--root <path>] [--workflows-dir <path>]" >&2
}

all=false
policy=""
root="."
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
[[ -d "$root/$workflows_dir" ]] || { echo "gate-inputs: workflows dir not found: $root/$workflows_dir" >&2; exit 2; }

policy_path="$root/$policy"

# ---------------------------------------------------------------------------
# Precheck: the manifest is valid TOML.
#
# RFC 094 R10 reasons that ownership conflicts -- one lane with two owners,
# one RFC number with two headings -- are TOML parse errors and so need no
# condition of their own. That holds only while something parses the manifest
# as TOML, and every reader in this pipeline (this script, ci-gate.sh, both
# fixture harnesses) is awk, which reads a duplicate key leniently and keeps
# whichever value came last. Measured: `"093" = "Summary"` added below the
# real source entry passed every condition with exit 0 while tomllib rejected
# the same file. This precheck makes the RFC's premise true rather than
# writing the detector the RFC forbids. A duplicate key is itself a failure
# here, independent of value equality: two identical rows are as malformed as
# two conflicting ones, and the first occurrence is never silently kept. That
# guarantee used to be re-derived by three per-table detectors inside the
# conditions; since it holds for the whole file, they were unreachable and
# were removed in R10-c. It runs before any condition because
# a manifest that is not TOML has nothing meaningful to check. Python 3.14 is
# pinned in [tools] and already required by G10b and G11.
# ---------------------------------------------------------------------------

command -v python3.14 >/dev/null 2>&1 || {
  echo "gate-inputs: python3.14 not found; it is pinned in [tools] and required for the TOML validity precheck" >&2
  exit 2
}

if ! toml_error=$(python3.14 - "$policy_path" <<'PRECHECK' 2>&1
import sys
import tomllib

try:
    with open(sys.argv[1], "rb") as handle:
        tomllib.load(handle)
except (OSError, tomllib.TOMLDecodeError) as exc:
    print(exc)
    raise SystemExit(1)
PRECHECK
); then
  echo "gate-inputs: manifest is not valid TOML: $toml_error" >&2
  exit 1
fi

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
  # Prints the value of `KEY = ...` inside table $2, or nothing if absent,
  # followed by COUNT= so callers can tell absent from present. The count
  # can no longer exceed 1: the TOML precheck rejects a duplicate key before
  # any condition runs.
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
# Condition 7 (RFC 094 R10): the multi-source lane registry.
#
# Until R10 this condition compared the manifest against RFC 093's table
# alone, so no RFC other than 093 could own a lane. It now resolves each lane
# to its owning RFC through [gate_owners] / [gate_lane_sources] and compares
# against that RFC's own table. Six checks:
#
#   1. every [gates] key has exactly one [gate_owners] entry;
#   2. every [gate_owners] value is a declared source that resolves to
#      exactly one RFC file;
#   3. every lane in every source's table is in [gates] or
#      [gate_matrix_exceptions];
#   4. every [gates] command byte-matches the row in *its owning* RFC's
#      table, under the one permitted normalisation;
#   5. no lane is in both [gates] and [gate_matrix_exceptions];
#   6. every [gate_matrix_exceptions] key names a lane some source declares.
#
# Checks 5 and 6 are not new requirements: they are the disjointness and
# groundedness tests the single-source version already performed, restated
# over all sources. The exception-reason rule below is likewise unchanged.
# The two duplicate-key rules that used to sit alongside it were removed in
# R10-c: the TOML precheck subsumes them for the whole manifest.
#
# One normalisation is permitted on the command comparison: a source RFC may
# render a two-part lane as two backticked commands joined by the word "and"
# (RFC 093 does this for G05/G06); the manifest joins them with "&&". No other
# normalisation is applied — every other lane compares byte-for-byte.
#
# The section holding a source's table is named by [gate_lane_sources] as
# data. It is matched by plain equality against the heading text with leading
# `#` characters and surrounding whitespace stripped — never as a pattern,
# since a heading such as `Gate Matrix (v2)` read as a regex would have its
# parentheses taken as a group. Any heading level may carry a table, and the
# heading must occur exactly once in its RFC. The section body runs to the
# next heading line of any level, which is what bounds RFC 093's matrix away
# from its later negative-self-tests table, whose rows reuse G09a/G09b with
# different (fixture) commands.
# ---------------------------------------------------------------------------

# [gate_lane_sources] and [gate_owners]. Keys in the former are quoted RFC
# numbers, so keys are unquoted here as well as values.
extract_registry_table() {
  awk -v table="$1" '
    $0 ~ ("^\\[" table "\\]") { in_table = 1; next }
    /^\[/ { in_table = 0 }
    in_table {
      eq = index($0, " = ")
      if (eq > 0) {
        key = substr($0, 1, eq - 1)
        val = substr($0, eq + 3)
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", key)
        sub(/^"/, "", key); sub(/"$/, "", key)
        sub(/^"/, "", val); sub(/"$/, "", val)
        print key "\t" val
      }
    }
  ' "$policy_path"
}

# A source RFC number resolves across the four lifecycle folders only.
# rfcs/handoffs/ reuses RFC numbers for companion directories and is
# deliberately excluded — the same scoping check-rfc-integrity.py applies.
resolve_rfc_file() {
  local num=$1 dir
  for dir in proposed accepted "done" archive; do
    [[ -d "$root/rfcs/$dir" ]] || continue
    find "$root/rfcs/$dir" -maxdepth 1 -type f -name "$num-*.md"
  done | sort
}

# How many headings in $1 equal $2 by plain equality, at any level.
heading_occurrences() {
  awk -v want="$2" '
    /^#/ {
      line = $0; depth = 0
      while (substr(line, 1, 1) == "#") { line = substr(line, 2); depth++ }
      if (depth <= 6) {
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", line)
        if (line == want) count++
      }
    }
    END { print count + 0 }
  ' "$1"
}

# The matched section's body: heading to the next heading of any level.
extract_heading_section() {
  awk -v want="$2" '
    /^#/ {
      line = $0; depth = 0
      while (substr(line, 1, 1) == "#") { line = substr(line, 2); depth++ }
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", line)
      in_section = (depth <= 6 && line == want)
      next
    }
    in_section { print }
  ' "$1"
}

# Parse each `| GNN | toolchain | features | command(s) |` row into
# `GNN<TAB>normalised-command`.
parse_lane_rows() {
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
  ' "$1"
}

# Extract [gates] from the manifest as `GNN<TAB>command`.
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

sort -u "$tmp/manifest-exceptions-raw" >"$tmp/manifest-exceptions"
cut -f1 "$tmp/manifest-exceptions" | sort -u >"$tmp/manifest-exception-ids"

awk -F'\t' '$2 == "" { print $1 }' "$tmp/manifest-exceptions" >"$tmp/exceptions-empty-reason"
if [[ -s "$tmp/exceptions-empty-reason" ]]; then
  fail "condition 7: [gate_matrix_exceptions] entry(ies) with no reason recorded:"
  cat "$tmp/exceptions-empty-reason" >&2
fi

extract_registry_table "gate_owners" >"$tmp/lane-owners"
extract_registry_table "gate_lane_sources" >"$tmp/lane-sources"

# --- check 1: every [gates] key has exactly one [gate_owners] entry --------
# "Exactly one" also covers the duplicate-owner case, which is why RFC 094
# specifies no separate ownership-conflict detector.
: >"$tmp/check1-violations"
while IFS= read -r lane; do
  [[ -n "$lane" ]] || continue
  owner_count=$(awk -F'\t' -v l="$lane" '$1 == l { c++ } END { print c + 0 }' "$tmp/lane-owners")
  if [[ "$owner_count" -ne 1 ]]; then
    echo "$lane ($owner_count [gate_owners] entries, expected exactly 1)" >>"$tmp/check1-violations"
  fi
done <"$tmp/manifest-gate-ids"
if [[ -s "$tmp/check1-violations" ]]; then
  fail "condition 7 (check 1): [gates] lane(s) without exactly one [gate_owners] entry:"
  cat "$tmp/check1-violations" >&2
fi

# --- check 2: every owner is a declared source resolving to one RFC file ---
: >"$tmp/check2-violations"
cut -f2 "$tmp/lane-owners" | sort -u >"$tmp/owner-values"
while IFS= read -r owner; do
  [[ -n "$owner" ]] || continue
  if ! awk -F'\t' -v o="$owner" '$1 == o { found = 1 } END { exit !found }' "$tmp/lane-sources"; then
    echo "\"$owner\" is named in [gate_owners] but not declared in [gate_lane_sources]" \
      >>"$tmp/check2-violations"
  fi
done <"$tmp/owner-values"

# Resolve every declared source, whether or not a lane currently names it:
# check 3 needs each source's table, and an unresolvable source is a failure
# regardless of who points at it. Resolution must yield exactly one file —
# zero or several is a failure, never a first-match guess.
: >"$tmp/source-lanes"
: >"$tmp/resolved-sources"
while IFS=$'\t' read -r source_num source_heading; do
  [[ -n "$source_num" ]] || continue
  mapfile -t source_files < <(resolve_rfc_file "$source_num")
  if [[ "${#source_files[@]}" -ne 1 ]]; then
    echo "\"$source_num\" resolves to ${#source_files[@]} files under rfcs/{proposed,accepted,done,archive} (exactly one required)" \
      >>"$tmp/check2-violations"
    continue
  fi
  source_file="${source_files[0]}"
  occurrences=$(heading_occurrences "$source_file" "$source_heading")
  if [[ "$occurrences" -ne 1 ]]; then
    fail "condition 7: source RFC \"$source_num\" heading \"$source_heading\" occurs $occurrences times in $source_file (exactly once required)"
    continue
  fi
  extract_heading_section "$source_file" "$source_heading" >"$tmp/section-$source_num"
  parse_lane_rows "$tmp/section-$source_num" \
    | awk -F'\t' -v o="$source_num" '{ print $0 "\t" o }' >>"$tmp/source-lanes"
  echo "$source_num" >>"$tmp/resolved-sources"
done <"$tmp/lane-sources"

if [[ -s "$tmp/check2-violations" ]]; then
  fail "condition 7 (check 2): [gate_owners] value(s) not resolvable to exactly one source RFC:"
  cat "$tmp/check2-violations" >&2
fi

# --- check 3: every lane a source declares is accounted for ---------------
# The completeness rule RFC 093 M1b C2.1 introduced, generalised from RFC
# 093's table to every source's table.
cut -f1 "$tmp/source-lanes" | sort -u >"$tmp/source-lane-ids"
comm -23 "$tmp/source-lane-ids" <(sort -u "$tmp/manifest-gate-ids" "$tmp/manifest-exception-ids") >"$tmp/unaccounted-lanes"
if [[ -s "$tmp/unaccounted-lanes" ]]; then
  fail "condition 7 (check 3): lane(s) declared by a source RFC but absent from both [gates] and [gate_matrix_exceptions]:"
  cat "$tmp/unaccounted-lanes" >&2
fi

# --- check 4: every [gates] command matches its owning RFC's row ----------
# A lane with no owner, or with an owner that did not resolve, is skipped
# here: check 1 or check 2 has already named the cause, and reporting it
# again under check 4 would name a symptom.
: >"$tmp/check4-violations"
while IFS=$'\t' read -r lane manifest_cmd; do
  [[ -n "$lane" ]] || continue
  owner=$(awk -F'\t' -v l="$lane" '$1 == l { print $2; exit }' "$tmp/lane-owners")
  [[ -n "$owner" ]] || continue
  grep -qxF "$owner" "$tmp/resolved-sources" || continue
  if ! awk -F'\t' -v l="$lane" -v o="$owner" '$1 == l && $3 == o { found = 1 } END { exit !found }' "$tmp/source-lanes"; then
    echo "$lane (owner $owner): no row for this lane in the owning RFC's table" >>"$tmp/check4-violations"
    continue
  fi
  rfc_cmd=$(awk -F'\t' -v l="$lane" -v o="$owner" '$1 == l && $3 == o { print $2; exit }' "$tmp/source-lanes")
  if [[ "$rfc_cmd" != "$manifest_cmd" ]]; then
    echo "$lane (owner $owner): manifest [$manifest_cmd] != RFC [$rfc_cmd]" >>"$tmp/check4-violations"
  fi
done <"$tmp/manifest-gates"
if [[ -s "$tmp/check4-violations" ]]; then
  fail "condition 7 (check 4): [gates] command(s) do not match the owning RFC's lane table:"
  cat "$tmp/check4-violations" >&2
fi

# --- check 5: disjointness ------------------------------------------------
# A lane in both [gates] and [gate_matrix_exceptions] means one of the two
# lists is stale, and the dispatcher and the exemption cannot both be true
# for the same lane.
comm -12 "$tmp/manifest-gate-ids" "$tmp/manifest-exception-ids" >"$tmp/gates-and-exceptions-overlap"
if [[ -s "$tmp/gates-and-exceptions-overlap" ]]; then
  fail "condition 7 (check 5): lane(s) present in both [gates] and [gate_matrix_exceptions]:"
  cat "$tmp/gates-and-exceptions-overlap" >&2
fi

# --- check 6: every exception names a lane some source declares -----------
# An exception for a lane no RFC declares is a stale exception, and must
# fail rather than sit unnoticed.
comm -13 "$tmp/source-lane-ids" "$tmp/manifest-exception-ids" >"$tmp/stale-exceptions"
if [[ -s "$tmp/stale-exceptions" ]]; then
  fail "condition 7 (check 6): [gate_matrix_exceptions] lane(s) not declared by any source RFC:"
  cat "$tmp/stale-exceptions" >&2
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
