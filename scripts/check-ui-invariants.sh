#!/usr/bin/env bash
# RFC 093 G12: versioned UI-invariant gate.

set -euo pipefail

usage() {
  echo "usage: $0 --all --policy <path> [--root <path>]" >&2
}

all=false
policy=""
root="."

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
    *)
      usage
      exit 2
      ;;
  esac
done

[[ "$all" == true && -n "$policy" ]] || { usage; exit 2; }
[[ -f "$policy" ]] || { echo "G12 policy not found: $policy" >&2; exit 2; }
[[ -d "$root" ]] || { echo "G12 root not found: $root" >&2; exit 2; }

require_policy_line() {
  if ! grep -Fqx -- "$1" "$policy"; then
    echo "G12 policy v1 is missing or changes required declaration: $1" >&2
    exit 2
  fi
}

# Keep policy changes review-visible. A renamed, removed, or weakened v1 check
# fails closed until this entry point and its fixtures are reviewed together.
require_policy_line 'version = 1'
require_policy_line 'blocking_checks = ["text-leaks", "css-tokens-resolve", "semantic-palette-parity", "inline-style-bound"]'
require_policy_line 'source_root = "crates"'
require_policy_line 'declaration_files = ["crates/sui-id-web/src/tokens.rs", "crates/sui-id-web/src/components.rs"]'
require_policy_line 'usage_root = "crates"'
require_policy_line 'token_file = "crates/sui-id-web/src/tokens.rs"'
require_policy_line 'families = ["danger", "warning", "success", "info"]'
require_policy_line 'slots = ["default", "subtle", "fg-on"]'
require_policy_line 'pages_root = "crates/sui-id-web/src/pages"'
require_policy_line 'checks = ["standalone-translations", "unused-css-tokens"]'

read_policy_integer() {
  local key=$1
  local -a values
  mapfile -t values < <(sed -nE "s/^${key} = ([0-9]+)$/\\1/p" "$policy")
  if [[ "${#values[@]}" -ne 1 ]]; then
    echo "G12 policy v1 requires exactly one integer $key" >&2
    return 2
  fi
  printf '%s\n' "${values[0]}"
}

required_mode_roots=$(read_policy_integer required_mode_roots)
if [[ "$required_mode_roots" -ne 3 ]]; then
  echo "G12 policy v1 requires exactly three semantic theme roots" >&2
  exit 2
fi

maximum=$(read_policy_integer maximum)
if [[ "$maximum" -gt 20 ]]; then
  echo "G12 policy v1 inline-style maximum must be an integer no greater than 20" >&2
  exit 2
fi

crates="$root/crates"
tokens="$root/crates/sui-id-web/src/tokens.rs"
components="$root/crates/sui-id-web/src/components.rs"
pages="$root/crates/sui-id-web/src/pages"

for required in "$crates" "$tokens" "$components" "$pages"; do
  [[ -e "$required" ]] || { echo "G12 required input not found: $required" >&2; exit 2; }
done

failures=0

text_leaks=$(grep -rEn '>t\.[a-z_0-9]+<' "$crates" --include='*.rs' || true)
if [[ -n "$text_leaks" ]]; then
  echo "G12 text-leaks: bare t.field expression renders as literal text:" >&2
  echo "$text_leaks" >&2
  failures=$((failures + 1))
else
  echo "G12 text-leaks: pass"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

grep -hoE '^\s+--[a-z0-9-]+\s*:' "$tokens" "$components" \
  | sed 's/^[[:space:]]*//; s/[[:space:]]*:.*$//' \
  | sort -u >"$tmp/declared"
grep -rhoE 'var\(--[a-z0-9-]+' "$crates" --include='*.rs' --include='*.css' \
  | sed 's/^var(//' \
  | sort -u >"$tmp/used" || true
comm -23 "$tmp/used" "$tmp/declared" >"$tmp/unresolved"
if [[ -s "$tmp/unresolved" ]]; then
  echo "G12 css-tokens-resolve: variables used but not declared:" >&2
  cat "$tmp/unresolved" >&2
  failures=$((failures + 1))
else
  echo "G12 css-tokens-resolve: pass"
fi

for theme in default explicit-dark automatic-dark; do
  : >"$tmp/theme-$theme"
done

awk -v outdir="$tmp" '
  function occurrences(text, pattern, copy) {
    copy = text
    return gsub(pattern, "", copy)
  }
  function sanitize(text, output, position, character, following) {
    output = ""
    for (position = 1; position <= length(text); position++) {
      character = substr(text, position, 1)
      following = substr(text, position + 1, 1)
      if (in_comment) {
        if (character == "*" && following == "/") {
          in_comment = 0
          position++
        }
        continue
      }
      if (quote != "") {
        if (escaped) {
          escaped = 0
        } else if (character == "\\") {
          escaped = 1
        } else if (character == quote) {
          output = output character
          quote = ""
        }
        continue
      }
      if (character == "/" && following == "*") {
        # CSS comments separate tokens. Preserve that boundary so fragments on
        # either side cannot be synthesized into one property name.
        output = output " "
        in_comment = 1
        position++
      } else if (character == "\"" || character == "\047") {
        quote = character
        output = output character
      } else if (character == "\\") {
        # Policy v1 does not need unquoted CSS escapes. Reject them rather than
        # attempting partial escape normalization that could activate syntax.
        parse_errors++
        position++
        output = output " "
      } else {
        output = output character
      }
    }
    if (quote != "" && escaped) {
      # A terminal backslash escapes the CSS newline, not the first character
      # on the continued physical line.
      escaped = 0
    }
    return output
  }
  function trimmed(text, copy) {
    copy = text
    sub(/^[[:space:]]+/, "", copy)
    sub(/[[:space:]]+$/, "", copy)
    return copy
  }
  function begin_theme(name, parent_depth) {
    seen[name]++
    active = name
    active_parent_depth = parent_depth
    active_declaration_depth = parent_depth + 1
  }
  function inspect_statement(theme, statement, property, separator, value) {
    statement = trimmed(statement)
    if (statement == "") {
      return
    }
    separator = index(statement, ":")
    if (separator == 0) {
      parse_errors++
      return
    }
    property = substr(statement, 1, separator - 1)
    property = trimmed(property)
    if (property !~ /^--[a-z0-9-]+$/ &&
        property !~ /^-?[a-zA-Z][-_a-zA-Z0-9]*$/) {
      parse_errors++
      return
    }
    value = trimmed(substr(statement, separator + 1))
    if (property ~ /^--[a-z0-9-]+$/ && value == "") {
      parse_errors++
      return
    }
    if (property ~ /^--[a-z0-9-]+$/) {
      print property >> (outdir "/theme-" theme)
    }
  }
  function process_declarations(theme, text, position, character) {
    for (position = 1; position <= length(text); position++) {
      character = substr(text, position, 1)
      if (character == "(") {
        parentheses[theme]++
      } else if (character == ")") {
        if (parentheses[theme] == 0) {
          parse_errors++
        } else {
          parentheses[theme]--
        }
      } else if (character == "[") {
        brackets[theme]++
      } else if (character == "]") {
        if (brackets[theme] == 0) {
          parse_errors++
        } else {
          brackets[theme]--
        }
      }

      if (character == ";" && parentheses[theme] == 0 &&
          brackets[theme] == 0) {
        inspect_statement(theme, pending[theme])
        pending[theme] = ""
      } else {
        pending[theme] = pending[theme] character
      }
    }
    pending[theme] = pending[theme] "\n"
  }
  {
    raw = trimmed($0)
    if (!in_css) {
      if (raw == "pub const TOKENS_CSS: &str = r#\"") {
        css_starts++
        in_css = 1
      }
      next
    }
    if (raw == "\"#;") {
      css_ends++
      in_css = 0
      next
    }

    line_started_in_comment = in_comment
    line_started_in_quote = (quote != "")
    clean = sanitize($0)
    opens = occurrences(clean, "\\{")
    closes = occurrences(clean, "\\}")
    depth_before = depth

    if (active != "" && depth_before == active_declaration_depth &&
        opens == 0 && closes == 0) {
      process_declarations(active, clean)
    }

    if (raw == ":root {" && !line_started_in_comment &&
        !line_started_in_quote && depth_before == 0) {
      begin_theme("default", depth_before)
    } else if (raw == "[data-theme=\"dark\"] {" &&
               !line_started_in_comment && !line_started_in_quote &&
               depth_before == 0) {
      begin_theme("explicit-dark", depth_before)
    } else if (raw == "@media (prefers-color-scheme: dark) {" &&
               !line_started_in_comment && !line_started_in_quote &&
               depth_before == 0) {
      automatic_media_seen++
      automatic_media_parent_depth = depth_before
      automatic_media_depth = depth_before + 1
      in_automatic_media = 1
    } else if (raw == ":root:not([data-theme]) {" &&
               !line_started_in_comment && !line_started_in_quote &&
               in_automatic_media && depth_before == automatic_media_depth) {
      begin_theme("automatic-dark", depth_before)
    }

    depth += opens - closes
    if (depth < 0) {
      parse_errors++
      depth = 0
    }
    if (active != "" && depth == active_parent_depth) {
      if (trimmed(pending[active]) != "" || parentheses[active] != 0 ||
          brackets[active] != 0) {
        parse_errors++
      }
      pending[active] = ""
      parentheses[active] = 0
      brackets[active] = 0
      close(outdir "/theme-" active)
      active = ""
    }
    if (in_automatic_media && depth == automatic_media_parent_depth) {
      in_automatic_media = 0
    }
  }
  END {
    if (in_comment || quote != "" || in_css || depth != 0 ||
        css_starts != 1 || css_ends != 1) {
      parse_errors++
    }
    print "parse-errors", parse_errors + 0
    print "automatic-media", automatic_media_seen + 0
    print "default", seen["default"] + 0
    print "explicit-dark", seen["explicit-dark"] + 0
    print "automatic-dark", seen["automatic-dark"] + 0
  }
' "$tokens" >"$tmp/theme-counts"

semantic_failures=0
found_roots=0
while read -r theme count; do
  case "$theme" in
    parse-errors)
      if [[ "$count" -ne 0 ]]; then
        echo "G12 semantic-palette-parity: CSS structure could not be parsed safely" >&2
        semantic_failures=$((semantic_failures + 1))
      fi
      ;;
    automatic-media)
      if [[ "$count" -ne 1 ]]; then
        echo "G12 semantic-palette-parity: automatic-dark media occurs $count times; expected exactly 1" >&2
        semantic_failures=$((semantic_failures + 1))
      fi
      ;;
    *)
      found_roots=$((found_roots + count))
      if [[ "$count" -ne 1 ]]; then
        echo "G12 semantic-palette-parity: theme root $theme occurs $count times; expected exactly 1" >&2
        semantic_failures=$((semantic_failures + 1))
      fi
      ;;
  esac
done <"$tmp/theme-counts"

if [[ "$found_roots" -ne "$required_mode_roots" ]]; then
  echo "G12 semantic-palette-parity: found $found_roots required theme roots; policy requires $required_mode_roots" >&2
  semantic_failures=$((semantic_failures + 1))
fi

for theme in default explicit-dark automatic-dark; do
  for semantic in danger warning success info; do
    for slot in default subtle fg-on; do
      if [[ "$slot" == "fg-on" ]]; then
        name="fg-on-$semantic"
      else
        name="$semantic-$slot"
      fi
      count=$(grep -cxF -- "--$name" "$tmp/theme-$theme" || true)
      if [[ "$count" -ne 1 ]]; then
        echo "G12 semantic-palette-parity: --$name occurs $count times in $theme; expected exactly 1" >&2
        semantic_failures=$((semantic_failures + 1))
      fi
    done
  done
done

if [[ "$semantic_failures" -eq 0 ]]; then
  echo "G12 semantic-palette-parity: pass"
else
  failures=$((failures + semantic_failures))
fi

inline_count=$(grep -rEoh 'style="[^"]*"' "$pages" --include='*.rs' 2>/dev/null | wc -l || true)
inline_count=${inline_count//[[:space:]]/}
if [[ "$inline_count" -gt "$maximum" ]]; then
  echo "G12 inline-style-bound: $inline_count attributes exceeds maximum $maximum" >&2
  failures=$((failures + 1))
else
  echo "G12 inline-style-bound: pass ($inline_count/$maximum)"
fi

standalone=$(grep -rEn '^\s+t\.[a-z_0-9]+\s*$' "$crates" --include='*.rs' || true)
if [[ -n "$standalone" ]]; then
  echo "G12 advisory standalone-translations: inspect possible view! leaks:" >&2
  echo "$standalone" >&2
fi

comm -13 "$tmp/used" "$tmp/declared" >"$tmp/unused"
if [[ -s "$tmp/unused" ]]; then
  echo "G12 advisory unused-css-tokens:" >&2
  cat "$tmp/unused" >&2
fi

if [[ "$failures" -ne 0 ]]; then
  echo "G12 failed with $failures blocking invariant violation(s)" >&2
  exit 1
fi

echo "G12 passed all blocking UI invariants (policy v1)"
