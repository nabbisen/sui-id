# RFC 067 — Inline-style discipline + CI bound

**Status.** Implemented (v0.48.0)
**Priority.** P1 — Phase F (v0.48.0). Lower priority than RFC 065/066;
shippable independently. Bundled with RFC 068 (`handlers/me_security.rs`
split) as the final Phase F buffer release.
**Tracks.** PDF "honest visual hierarchy" tail: visual conventions
should live in `components.rs`, not scattered across screen views.
**Touches.** `crates/sui-id-web/src/components.rs` (new utility
classes), `crates/sui-id-web/src/pages/**.rs` (in-tree sweep), CI
workflow (new bound).

## Background

Phase F survey of `pages.rs` (pre-split) found **119 inline `style=""`
attributes**. They cluster into four categories:

1. **Spacing utilities** (most common): `style="margin-top:var(--space-4)"`,
   `style="margin-bottom:var(--space-3)"`, etc. — 60% of cases.
2. **Layout glue**: `style="gap:var(--space-2)"`,
   `style="align-items:center"`, etc. — 20%.
3. **Truly dynamic**: `style="color:var(--accent-default)"` mixed
   in with content-derived values (e.g., a stat coloured by its
   sign). — ~10%.
4. **One-off page tweaks**: `style="max-width:36rem"` on the auth
   card, `style="opacity:0.85"` on the dashboard sparkline title. —
   ~10%.

Categories 1 + 2 (80%) are utility-class candidates. They reuse the
same handful of token-derived values (`--space-2`, `--space-3`,
`--space-4`) and they obscure the screen rendering with visual
boilerplate.

## Goal

Add small utility classes for the common spacing + layout patterns.
Sweep `pages/**.rs` to remove the 80% of inline styles that map
to these utilities. Add a CI invariant that bounds inline `style`
attribute count in `pages/`.

## Design

### Utility classes (added to `components.rs`)

```css
/* Spacing utilities (RFC 067).
 * Token-derived margin and gap helpers for the common patterns
 * surfaced in the Phase F inline-style survey. */
.mt-2 { margin-top: var(--space-2); }
.mt-3 { margin-top: var(--space-3); }
.mt-4 { margin-top: var(--space-4); }
.mb-2 { margin-bottom: var(--space-2); }
.mb-3 { margin-bottom: var(--space-3); }
.mb-4 { margin-bottom: var(--space-4); }
.gap-1 { gap: var(--space-1); }
.gap-2 { gap: var(--space-2); }
.gap-3 { gap: var(--space-3); }

/* Layout utilities */
.center { text-align: center; }
.items-center { align-items: center; }
.items-end { align-items: flex-end; }
.justify-between { justify-content: space-between; }
.inline { display: inline; }
.inline-block { display: inline-block; }

/* Constrained width — auth cards, confirm screens, etc. */
.max-w-card { max-width: 36rem; }
.max-w-narrow { max-width: 22rem; }
```

These are deliberately small, deliberately not Tailwind-like —
each class maps to a single token-derived value. Avoiding the
explosion of utility classes that comes with full utility-first CSS.

### Sweep targets

Before each affected file:
- `pages/dashboard.rs`: 12 inline styles → ~3 (the truly-dynamic ones)
- `pages/auth.rs`: 20 → ~5
- `pages/confirm.rs`: 15 → ~3
- `pages/settings/**.rs`: 25 → ~8
- `pages/me_security/**.rs`: 18 → ~5
- ...

Expected total: 119 inline `style=""` → **~30**, with the remaining
30 being truly dynamic (token references mixed with content-derived
values, opacity tweaks for one-off visual emphasis).

### CI bound

A new CI step `inline-style-bound`:

```yaml
inline-style-bound:
  name: Inline style attribute count in pages/
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v6
    - name: Inline `style=""` count in pages/
      run: |
        set -e
        count=$(grep -rEohn 'style="[^"]*"' crates/sui-id-web/src/pages/ \
                  --include='*.rs' | wc -l)
        echo "Inline style attribute count: $count"
        if [ "$count" -gt 40 ]; then
          echo "::error::Inline `style=` count ($count) exceeds bound (40)."
          echo "Suggest a utility class in components.rs or document"
          echo "why the inline style is necessary."
          exit 1
        fi
```

The bound of 40 leaves headroom for the ~30 expected remaining
inline styles (truly dynamic / one-off) plus modest future drift.
Below the bound: a normal PR doesn't trip it. Above: someone has to
explain why the new screen needs ten new inline styles.

The bound deliberately *isn't zero*. Genuine one-off styles
(`opacity:0.85` for the dashboard sparkline dim, `color:var(...)`
combined with content) make sense inline. We're trimming the
boilerplate, not eliminating the affordance.

## Test plan

1. After utility classes added: `cargo check -p sui-id-web` PASS
   (CSS string change only).
2. After each sweep file: visual parity check (render the page,
   compare against pre-sweep screenshot).
3. CI: `inline-style-bound` passes at the new count.

## Rollout

Single release with RFC 065/066. The sweep depends on the split —
operating on `pages/dashboard.rs` is easier than scrolling through
the 4170-LOC pages.rs.

## Risks

- **Utility class proliferation**: every new utility nudges the
  project toward Tailwind. The selection above is deliberately
  small. Future utility additions need RFC justification.
- **Spec-violating selectors**: utility classes don't compose well
  with `<details>` collapsible sections or other compound widgets.
  These may need to stay inline. Allow.

## Future work

- Extend the bound to `components.rs` itself once that file's
  internal inline styles (e.g. in mock-up examples or
  documentation) are audited. Out of scope for Phase F.
