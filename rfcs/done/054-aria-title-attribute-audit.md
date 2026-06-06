# RFC 054 — Aria-label / title attribute i18n audit

**Status.** Implemented (v0.44.0)
**Priority.** P1 — Phase C (v0.44.0), carry-over from Phase B
**Tracks.** Completes the i18n half of the "a11y + i18n
v0.29.x" PDF slide. Closes the screen-reader accessibility
gap that body-text-only i18n (RFC 051) leaves open.
**Touches.** `crates/sui-id-web/src/pages.rs`,
`crates/sui-id-web/src/layout.rs`,
`crates/sui-id-i18n/src/strings.rs` and locale files,
`.github/workflows/ci.yml`.

> **Scope revision (2026-05).** After the v0.43.0 RFC 051 sweep, the
> remaining hardcoded aria-label / title attributes in the
> codebase are far fewer than the original audit projected. Only
> **3 sites** in `pages.rs` (`Setup steps`, `Security sections`,
> `Settings tabs`) and zero sites in `layout.rs` / `components.rs`
> need work. Most of what this RFC originally projected was
> incidentally fixed during RFC 051 because the aria-labels lived
> on the same `<section>`/`<nav>` elements whose body text was
> being i18n-routed. The original drafts of this RFC below remain
> for historical reference; the actual v0.44.0 implementation is
> three string substitutions plus the CI guard.

## Summary

After RFCs 048–053 land, body-text on every page is
locale-aware. The accessibility attributes — `aria-label`,
`aria-describedby`, `aria-current`, `title`,
`aria-controls` — are not. Screen readers announce these to
the operator; a Japanese user navigating the admin panel
hears "Statistics", "Operator action required", "Sign out",
"Theme" in English. This RFC audits every such attribute,
adds typed `Strings` fields, and rewires call sites. A CI
guard prevents new leaks.

## Background

The "a11y + i18n v0.29.x" slide requires:

> 画面ごとの必須チェック
> ・すべての input にラベルを付ける
> ・色以外に文言・icon・状態 badge を置く
> ・tab 順は見た目の流れと一致させる
> ・error / empty / success の文言を短く具体的にする
> ・focus-visible 2px ring を削らない

Of these, the input-label rule, the colour-non-dependence
rule, and the focus-ring rule are met by the existing
component CSS. The text-completeness rule is met by RFC
051 for element bodies but **not** for element attributes.

A grep at v0.41.0:

```
$ grep -rcE 'aria-label="[^"]*"' crates/sui-id-web/src/
crates/sui-id-web/src/layout.rs:   ~6 sites
crates/sui-id-web/src/pages.rs:    ~30 sites
crates/sui-id-web/src/components.rs: ~2 sites

$ grep -rcE 'title="[^"]*"'   crates/sui-id-web/src/
crates/sui-id-web/src/layout.rs:   ~3 sites
crates/sui-id-web/src/pages.rs:    ~8 sites
```

Roughly 50 attribute leaks. About 15 are already
captured by RFC 050 (chrome) and RFC 053 (copy button); the
remaining ~35 are spread across the page bodies.

## Goals

1. Every `aria-label`, `aria-describedby`, and `title`
   attribute on a visible element reads from `Strings`.
2. The exceptions — attributes carrying programmatic values
   (`aria-current="page"`, `aria-pressed="true"`,
   `aria-controls="some-id"`) — stay as literals where
   their value is part of the ARIA spec, not human text.
3. CI fails on PRs that add a hardcoded ARIA/title text.

## Detailed design

### Part A — categorise attribute occurrences

Every `aria-*` and `title=` attribute in `layout.rs` and
`pages.rs` is one of:

**Category 1 — Programmatic ARIA value.** Examples:
`aria-current="page"`, `aria-pressed="true"`,
`aria-controls="logout-csrf"`, `aria-hidden="true"`,
`role="status"`. These are part of the ARIA / HTML
contract; their values are spec keywords. **Stay as
literals; no i18n.**

**Category 2 — Human-readable label.** Examples (current):
`aria-label="Statistics"`, `aria-label="Operator action required"`,
`aria-label="Main"`, `aria-label="Sign out"`,
`aria-label="Theme"`, `aria-label="Accessibility features"`,
`title="Light theme"`, `title="JWKS URI"`. **Route through
`Strings`.**

**Category 3 — Composed phrase (template).** Examples:
`title=format!("Copy {label}")`,
`aria-label=format!("Copy {label}")`. **Handled by RFC 053**;
out of scope here.

The audit's first deliverable is a complete categorisation
table (in the PR description) of all ~50 sites. The
mechanical changes follow that table.

### Part B — `Strings` additions

Naming convention for ARIA-only keys: `aria_*` prefix,
followed by the scope where it appears.

```rust
// strings.rs additions
// (Chrome ARIA labels overlap with RFC 050's additions —
//  reused, not duplicated.)
pub aria_dashboard_stats:         &'static str,
pub aria_dashboard_warn_section:  &'static str,
pub aria_dashboard_activity_period: &'static str,
pub aria_clients_table:           &'static str,
pub aria_users_table:             &'static str,
pub aria_audit_table:             &'static str,
pub aria_pagination:              &'static str,
pub aria_sortable_column:         &'static str,
pub aria_dangerous_action:        &'static str,
pub aria_form_field_required:     &'static str,
pub aria_form_field_optional:     &'static str,

// title attributes — short tooltips on icons and code samples
pub title_jwks_uri:               &'static str,
pub title_issuer:                 &'static str,
pub title_discovery:              &'static str,
pub title_password_hidden:        &'static str,
pub title_password_visible:       &'static str,
// (etc. — list captured in audit table)
```

The exact list is finalised during the Part A audit; this
draft enumerates the most-common shapes.

### Part C — call-site rewrites

Mechanical substitution following the audit table:

```rust
// Before
<nav class="app-nav" aria-label="Main">
<section class="grid-cards" aria-label="Statistics">
<a href="/.well-known/jwks.json" title="JWKS URI">

// After
<nav class="app-nav" aria-label=t.nav_aria_main>            // RFC 050
<section class="grid-cards" aria-label=t.aria_dashboard_stats>
<a href="/.well-known/jwks.json" title=t.title_jwks_uri>
```

### Part D — CI invariant

The grep is precise: any `aria-label="` or `title="`
followed by a *literal* string is flagged. References to
typed `Strings` fields use Rust expression syntax (no
opening `"`) and are not matched.

```yaml
  text-leaks-attrs:
    name: text-leak invariants — ARIA / title attributes
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - name: No hardcoded text in aria-label / aria-describedby / title
        run: |
          set -e
          # Allowed: aria-label / title set to a typed expression
          # like  aria-label=t.something  or  aria-label={t.something}.
          # Flagged: anything in matching quotes that contains text.
          # We deliberately allow empty "" (e.g. for ornamental icons).
          found=$(grep -nE 'aria-label="[^"]+"|aria-describedby="[^"]+"|title="[^"]+"' \
                    crates/sui-id-web/src/pages.rs \
                    crates/sui-id-web/src/layout.rs \
                    crates/sui-id-web/src/components.rs || true)
          # Allowlist: spec ARIA keywords that ARE legitimate literals.
          allow='(aria-current="(page|step|location|date|time|true|false)"|aria-pressed="(true|false|mixed)"|aria-expanded="(true|false)"|aria-hidden="true"|aria-modal="true"|aria-haspopup="true"|aria-disabled="(true|false)"|aria-live="(off|polite|assertive)"|aria-atomic="(true|false)"|role="[a-z]+")'
          filtered=$(echo "$found" | grep -vE "$allow" || true)
          if [ -n "$filtered" ]; then
            echo "::error::Hardcoded text in ARIA/title attribute."
            echo "Move to crates/sui-id-i18n/src/strings.rs as a typed field."
            echo "$filtered"
            exit 1
          fi
```

The allowlist is conservative: spec keywords that
legitimately remain literal. `aria-current="page"`,
`aria-pressed="true"`, `aria-hidden="true"`, etc. don't
flag.

### Part E — programmatic attribute values that look like text

A handful of attributes carry programmatic identifiers
that happen to look like English words: `data-theme-value="light"`,
`aria-label="page"` (no, this isn't a real case — aria-current
takes "page"; the attribute is `aria-current`, value `"page"`,
not flagged).

The grep above is over `aria-label="…"` / `title="…"` only,
so these don't appear. If a future feature adds a real
human-text attribute we don't yet have, the grep catches it
and we add to the allowlist with reason.

### Part F — `title` vs. `aria-describedby` style guidance

Many `title=` attributes today double as the only accessible
description of an icon or code cell. Browser support for
`title` as a screen-reader description is inconsistent. The
recommended pattern is:

- For an icon-only button: use `aria-label`, not `title`.
- For a hover-only tooltip on text that is itself
  accessible: keep `title`.
- For a code cell that is the link target: don't use
  `title` at all — the link target is self-explanatory.

This is a guidance recommendation in `docs/`, not a CI
rule. RFC 054 captures it; later RFCs apply it
incrementally.

## Test plan

- **Compile-time**: `Strings` exhaustiveness covers the new
  fields automatically.
- **CI**: the new `text-leaks-attrs` job fails on a draft
  commit that adds `aria-label="Foo"` ; passes on the
  fix branch.
- **Manual smoke** with a screen reader (macOS VoiceOver or
  NVDA): navigate the dashboard in JA and EN; confirm
  announced labels are in the right locale. Sample
  announcements:
  - Dashboard heading
  - Statistics section landmark
  - Activity period nav landmark
  - Each table column header
  - Theme toggle group + each button
  - Sign-out button

## Security considerations

None.

## Migration risk

Low. Attribute text changes only; no DOM structure changes;
no JS behavioural changes.

## Estimated effort

- Part A audit (categorise ~50 sites): 1.5 hours
- Part B (Strings + ja/en/zh): 1 hour
- Part C (~35 call-site rewrites): 2 hours
- Part D (CI snippet + allowlist tuning): 1 hour
- Part E/F (guidance doc + smoke): 1 hour

**~6.5 hours total.**

## Version impact

Patch bump within v0.43.0.

## Open questions

1. **VoiceOver / NVDA / TalkBack discrepancies**. The
   landmark roles (`role="navigation"`, `role="main"`,
   `role="contentinfo"`) are mostly auto-derived from
   semantic elements (`<nav>`, `<main>`, `<footer>`); we
   don't add them explicitly. Confirm that this remains
   correct after the chrome changes from RFC 050. If a
   future screen reader audit shows one is missing, add it
   then.
2. **Form-field ARIA descriptors**. `<input>` elements have
   `<label>` siblings; some also have `<span class="field__hint">`
   below them. The current implementation does not link the
   two via `aria-describedby`, so the hint text isn't read
   when the input is focused. RFC 054 adds this binding in
   one place (form fields) as a minimal a11y improvement.
   A more thorough audit (every input × every screen) is
   out of scope and tracked separately if needed.
