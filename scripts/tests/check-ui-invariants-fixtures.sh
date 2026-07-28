#!/usr/bin/env bash
# Negative self-tests for RFC 093 G12.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
gate="$repo_root/scripts/check-ui-invariants.sh"
policy="$repo_root/ci/ui-invariants.toml"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

make_valid_fixture() {
  local target=$1
  mkdir -p "$target/crates/sui-id-web/src/pages"
  : >"$target/crates/sui-id-web/src/components.rs"
  {
    echo 'pub const TOKENS_CSS: &str = r#"'
    echo ':root {'
    for semantic in danger warning success info; do
      echo "  --$semantic-default: #000;"
      echo "  --$semantic-subtle: #111;"
      echo "  --fg-on-$semantic: #fff;"
    done
    echo '}'
    echo '[data-theme="dark"] {'
    for semantic in danger warning success info; do
      echo "  --$semantic-default: #000;"
      echo "  --$semantic-subtle: #111;"
      echo "  --fg-on-$semantic: #fff;"
    done
    echo '}'
    echo '@media (prefers-color-scheme: dark) {'
    echo '  :root:not([data-theme]) {'
    for semantic in danger warning success info; do
      echo "    --$semantic-default: #000;"
      echo "    --$semantic-subtle: #111;"
      echo "    --fg-on-$semantic: #fff;"
    done
    echo '  }'
    echo '}'
    echo '"#;'
  } >"$target/crates/sui-id-web/src/tokens.rs"
  echo 'fn valid() { view! { <span>{t.label}</span> } }' \
    >"$target/crates/sui-id-web/src/pages/valid.rs"
}

expect_failure() {
  local name=$1
  local expected=$2
  local fixture="$tmp/$name"
  local output="$tmp/$name.output"

  if "$gate" --all --policy "$policy" --root "$fixture" >"$output" 2>&1; then
    echo "fixture $name unexpectedly passed" >&2
    exit 1
  fi
  if ! grep -Fq "$expected" "$output"; then
    echo "fixture $name failed for the wrong reason; expected: $expected" >&2
    cat "$output" >&2
    exit 1
  fi
  echo "fixture $name: expected failure observed"
}

expect_success() {
  local name=$1
  local fixture="$tmp/$name"
  local output="$tmp/$name.output"

  if ! "$gate" --all --policy "$policy" --root "$fixture" >"$output" 2>&1; then
    echo "fixture $name unexpectedly failed" >&2
    cat "$output" >&2
    exit 1
  fi
  echo "fixture $name: pass"
}

valid="$tmp/valid"
make_valid_fixture "$valid"
"$gate" --all --policy "$policy" --root "$valid" >/dev/null
echo "fixture valid: pass"

text_leaks="$tmp/text-leaks"
cp -R "$valid" "$text_leaks"
echo 'fn invalid() { view! { <span>t.label</span> } }' \
  >"$text_leaks/crates/sui-id-web/src/pages/invalid.rs"
expect_failure text-leaks "G12 text-leaks:"

css_tokens="$tmp/css-tokens-resolve"
cp -R "$valid" "$css_tokens"
echo 'const BAD: &str = "color: var(--missing-token);";' \
  >"$css_tokens/crates/sui-id-web/src/pages/invalid.rs"
expect_failure css-tokens-resolve "G12 css-tokens-resolve:"

palette="$tmp/semantic-palette-parity"
cp -R "$valid" "$palette"
sed -i '0,/--info-subtle:/{/--info-subtle:/d;}' \
  "$palette/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-parity "G12 semantic-palette-parity:"

relocated="$tmp/semantic-palette-relocated"
cp -R "$valid" "$relocated"
sed -i '0,/--info-subtle:/{/--info-subtle:/d;}' \
  "$relocated/crates/sui-id-web/src/tokens.rs"
sed -i '/\[data-theme="dark"\] {/a\  --info-subtle: #222;' \
  "$relocated/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-relocated "G12 semantic-palette-parity:"

duplicate="$tmp/semantic-palette-same-root-duplicate"
cp -R "$valid" "$duplicate"
sed -i '/^:root {/a\  --danger-default: #222;' \
  "$duplicate/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-same-root-duplicate \
  "G12 semantic-palette-parity:"

missing_root="$tmp/semantic-palette-theme-root-missing"
cp -R "$valid" "$missing_root"
sed -i 's/^\[data-theme="dark"\] {$/[data-theme="night"] {/' \
  "$missing_root/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-theme-root-missing \
  "G12 semantic-palette-parity:"

nested_root="$tmp/semantic-palette-root-nested"
cp -R "$valid" "$nested_root"
sed -i '/^\[data-theme="dark"\] {$/i\@supports (display: grid) {' \
  "$nested_root/crates/sui-id-web/src/tokens.rs"
sed -i '/^@media (prefers-color-scheme: dark) {$/i\}' \
  "$nested_root/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-root-nested \
  "G12 semantic-palette-parity:"

nested_declaration="$tmp/semantic-palette-declaration-nested"
cp -R "$valid" "$nested_declaration"
sed -i '0,/--info-subtle:/{/--info-subtle:/d;}' \
  "$nested_declaration/crates/sui-id-web/src/tokens.rs"
sed -i '0,/^}$/{s/^}$/  @supports (display: grid) {\n    --info-subtle: #222;\n  }\n}/}' \
  "$nested_declaration/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-declaration-nested \
  "G12 semantic-palette-parity:"

nested_automatic="$tmp/semantic-palette-automatic-root-nested"
cp -R "$valid" "$nested_automatic"
sed -i '/^  :root:not(\[data-theme\]) {$/i\  @supports (display: grid) {' \
  "$nested_automatic/crates/sui-id-web/src/tokens.rs"
sed -i '$i\  }' "$nested_automatic/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-automatic-root-nested \
  "G12 semantic-palette-parity:"

brace_noise="$tmp/semantic-palette-comment-string-braces"
cp -R "$valid" "$brace_noise"
sed -i '/^:root {$/a\  /* } --danger-default: fake; { structural braces */\n  --fixture-text: "} {";' \
  "$brace_noise/crates/sui-id-web/src/tokens.rs"
expect_success semantic-palette-comment-string-braces

quoted_declaration="$tmp/semantic-palette-quoted-declaration"
cp -R "$valid" "$quoted_declaration"
quoted_tokens="$quoted_declaration/crates/sui-id-web/src/tokens.rs"
awk '
  !removed && /--info-subtle:/ {
    removed = 1
    next
  }
  {
    print
  }
  !inserted && /--info-default:/ {
    print "  --fixture-text: \"continued value\\"
    print "--info-subtle: #222;\";"
    inserted = 1
  }
' "$quoted_tokens" >"$quoted_tokens.new"
mv "$quoted_tokens.new" "$quoted_tokens"
expect_failure semantic-palette-quoted-declaration \
  "G12 semantic-palette-parity:"

continued_declaration="$tmp/semantic-palette-continued-declaration"
cp -R "$valid" "$continued_declaration"
continued_tokens="$continued_declaration/crates/sui-id-web/src/tokens.rs"
awk '
  !removed && /--warning-subtle:/ {
    removed = 1
    next
  }
  {
    print
  }
  !inserted && /--warning-default:/ {
    print "  --fixture-list: continued value"
    print "--warning-subtle: #222;"
    inserted = 1
  }
' "$continued_tokens" >"$continued_tokens.new"
mv "$continued_tokens.new" "$continued_tokens"
expect_failure semantic-palette-continued-declaration \
  "G12 semantic-palette-parity:"

parenthesized_declaration="$tmp/semantic-palette-parenthesized-semicolon"
cp -R "$valid" "$parenthesized_declaration"
parenthesized_tokens="$parenthesized_declaration/crates/sui-id-web/src/tokens.rs"
awk '
  !removed && /--success-subtle:/ {
    removed = 1
    next
  }
  {
    print
  }
  !inserted && /--success-default:/ {
    print "  --fixture-value: fixture-fn(alpha;"
    print "--success-subtle: #222);"
    inserted = 1
  }
' "$parenthesized_tokens" >"$parenthesized_tokens.new"
mv "$parenthesized_tokens.new" "$parenthesized_tokens"
expect_failure semantic-palette-parenthesized-semicolon \
  "G12 semantic-palette-parity:"

bracketed_declaration="$tmp/semantic-palette-bracketed-semicolon"
cp -R "$valid" "$bracketed_declaration"
bracketed_tokens="$bracketed_declaration/crates/sui-id-web/src/tokens.rs"
awk '
  !removed && /--danger-subtle:/ {
    removed = 1
    next
  }
  {
    print
  }
  !inserted && /--danger-default:/ {
    print "  --fixture-value: [alpha;"
    print "--danger-subtle: #222];"
    inserted = 1
  }
' "$bracketed_tokens" >"$bracketed_tokens.new"
mv "$bracketed_tokens.new" "$bracketed_tokens"
expect_failure semantic-palette-bracketed-semicolon \
  "G12 semantic-palette-parity:"

unmatched_component="$tmp/semantic-palette-unmatched-component"
cp -R "$valid" "$unmatched_component"
sed -i '0,/--success-default:/{s/: /: (/}' \
  "$unmatched_component/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-unmatched-component \
  "G12 semantic-palette-parity:"

comment_split="$tmp/semantic-palette-comment-split-name"
cp -R "$valid" "$comment_split"
comment_tokens="$comment_split/crates/sui-id-web/src/tokens.rs"
sed -i '0,/--info-subtle:/{s/--info-subtle:/--info-\/\* boundary \*\/subtle:/}' \
  "$comment_tokens"
expect_failure semantic-palette-comment-split-name \
  "G12 semantic-palette-parity:"

escaped_semicolon="$tmp/semantic-palette-escaped-semicolon"
cp -R "$valid" "$escaped_semicolon"
escaped_tokens="$escaped_semicolon/crates/sui-id-web/src/tokens.rs"
awk '
  !removed && /--warning-subtle:/ {
    removed = 1
    next
  }
  {
    print
  }
  !inserted && /--warning-default:/ {
    print "  --fixture-value: alpha\\;"
    print "--warning-subtle: #222;"
    inserted = 1
  }
' "$escaped_tokens" >"$escaped_tokens.new"
mv "$escaped_tokens.new" "$escaped_tokens"
expect_failure semantic-palette-escaped-semicolon \
  "G12 semantic-palette-parity:"

empty_value="$tmp/semantic-palette-empty-value"
cp -R "$valid" "$empty_value"
sed -i 's/^  --danger-default: #000;$/  --danger-default:;/' \
  "$empty_value/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-empty-value "G12 semantic-palette-parity:"

blank_value="$tmp/semantic-palette-blank-value"
cp -R "$valid" "$blank_value"
sed -i 's/^  --danger-default: #000;$/  --danger-default: ;/' \
  "$blank_value/crates/sui-id-web/src/tokens.rs"
expect_failure semantic-palette-blank-value "G12 semantic-palette-parity:"

inline="$tmp/inline-style-bound"
cp -R "$valid" "$inline"
for index in $(seq 1 21); do
  printf 'const STYLE_%s: &str = r#"style="display: block""#;\n' "$index"
done >"$inline/crates/sui-id-web/src/pages/invalid.rs"
expect_failure inline-style-bound "G12 inline-style-bound:"

echo "G12 negative fixtures passed"
