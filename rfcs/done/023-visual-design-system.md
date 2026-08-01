# RFC 023 — Visual design system: tokens, components, and motion

**Status.** Implemented (v0.32.0)
**Priority.** Medium. RFC 017 freezes the *behavioural*
contract for screens (responsibilities, dangerous-operation
pattern, state copy). This RFC freezes the *visual* contract
that those screens render through: colour, typography,
spacing, component primitives, focus, and motion. Without it,
each new admin-domain screen re-derives its own visual choices
and the surface drifts.
**Tracks.** UI/UX deliverables v0.29.x (one-page overview +
17-page detail deck). RFC 017 § 2 (responsibilities), § 3
(dangerous-op pattern), § 4 (state copy), § 9 (dev-mode UI
separation), § 10 (accessibility checklist) all reference
visual properties this RFC fixes. The colour palette file
(`sui-id-color-palettes.txt`) is the source of truth for the
token values.
**Touches.** `crates/sui-id-web/src/styles/tokens.css` (new),
`crates/sui-id-web/src/styles/base.css` (new or merged),
`crates/sui-id-web/src/styles/components.css` (new), Leptos
component files under `crates/sui-id-web/src/` that currently
embed inline style or ad-hoc classes. No backend change.

## Summary

The UI/UX deliverables contain a complete visual specification:
a colour palette with light and dark modes, a sketched
component library (button, input, badge, banner, modal, tab),
explicit accessibility rules ("focus-visible 2px ring を削らな
い"), and design philosophy ("ABDD × ミニマリズム"). This RFC
translates that specification into CSS variables and a small
set of class-based component primitives that every Leptos
component in `sui-id-web` consumes.

The work is medium-priority for the same reason RFC 017 was:
admin-domain UI work (RFC 002 i18n expansion, future RFC 008
third-party-posture consent screen, future Step 2 of admin-UI
i18n) inherits this contract. Picking up those RFCs against
ad-hoc styling means the implementer either invents a system
on the fly or carries inconsistency. The tokens here remove
that choice.

This RFC is *implementation-coupled* in a way RFC 017 was not.
RFC 017 produced a document; this one produces CSS that ships
in the binary. The visual contract is enforceable in code:
a component using `var(--accent-default)` automatically gets
the dark-mode override; a component using a hardcoded
`#7C6BCF` does not. That property is the goal.

## Why this RFC, not nine smaller ones

Tokens, typography, spacing, components, focus, dark mode, and
motion could each be their own RFC. They are bundled here
because:

- The dependency graph is dense. Components depend on tokens.
  Dark mode is a token-level concern that components inherit
  for free if tokens are right and pollute every component
  if tokens are wrong. Splitting into "tokens RFC, then
  components RFC" forces a half-finished state where tokens
  exist but no component consumes them.
- The deliverable is a single CSS surface that ships once. A
  partial landing (only colours, no spacing scale) is worse
  than no landing because it tempts implementers to mix the
  new colours with old ad-hoc spacing.
- The maintainer-supplied artefacts cover all seven topics in
  one coherent voice. Splitting into pieces obscures the
  coherence.

## Requirements

After this RFC ships:

1. A token CSS file defines colour, typography, spacing, and
   border-radius variables for both `:root` (light) and
   `[data-theme="dark"]` (or `prefers-color-scheme: dark`).
   Variable names match the source-of-truth palette names
   exactly (`--accent-default`, `--surface-elevated`, etc.).
2. A base CSS file applies typography defaults, focus-visible
   styling, motion-reduction respect, and the dev-mode banner
   layout from RFC 017 § 9.
3. A components CSS file defines class primitives for the
   element types listed in the deliverables: `.btn`,
   `.btn-danger`, `.btn-secondary`; `.field` with label /
   input / help / error; `.badge` with status variants
   (`.badge--ok`, `.badge--warn`, `.badge--danger`);
   `.banner` with severity variants; `.modal` with focus
   trap; `.tabs` with active state; `.dev-banner`.
4. Every existing screen in `sui-id-web` is migrated to use
   tokens and class primitives. No hardcoded hex colours
   remain in component files. (Inline `style=` is acceptable
   only for one-off layout values like dynamic widths.)
5. `:focus-visible` shows a 2px outline in `--accent-default`
   on every interactive element. No element has `:focus`
   override that suppresses it.
6. Status colour is paired with text or icon at every use
   site (per RFC 017 § 10). Code review enforces this; this
   RFC's documentation describes the rule.
7. `prefers-reduced-motion: reduce` reduces all transitions
   to instantaneous changes.
8. Existing tests pass. New visual-regression tests are
   recommended (see § Tests below) but not strictly required.

## Design

### § 1. Token layer

The colour token names are taken verbatim from the project's
colour palette file. Reproduced here for the implementer's
convenience (light mode):

```css
:root {
  /* Surface */
  --surface-default:  #F6F5F9;
  --surface-subtle:   #EFEAF4;
  --surface-elevated: #FFFFFF;
  --surface-inverse:  #2B2733;
  --surface-sunken:   #E9E4EF;

  /* Foreground */
  --fg-default:    #1F1B24;
  --fg-muted:      #5F5A66;
  --fg-subtle:     #9A94A3;
  --fg-on-accent:  #FFFFFF;
  --fg-inverse:    #F6F5F9;

  /* Accent (Lavender-Jade) */
  --accent-default:  #7C6BCF;
  --accent-emphasis: #6956C0;
  --accent-subtle:   #E6E1F5;

  /* Semantic */
  --danger-default:  #C94A4A;
  --danger-subtle:   #F6E3E3;
  --warning-default: #D49B2A;
  --success-default: #3FA37A;
  --info-default:    #4A7FC9;

  /* Interaction */
  --state-hover:    rgba(0, 0, 0, 0.05);
  --state-active:   rgba(0, 0, 0, 0.10);
  --state-focus:    var(--accent-default);
  --state-disabled: rgba(31, 27, 36, 0.38);

  /* Border */
  --border-default: #D6D1DE;
  --border-muted:   #E9E4EF;
  --border-accent:  var(--accent-default);
}
```

Dark mode follows the same name set with overridden values
(see palette file). The implementer copies both blocks
verbatim into `tokens.css`. The file is mechanically derived
from the palette source — if the palette changes, the file
regenerates.

The `state-focus` variable explicitly aliases to
`accent-default` so that the focus ring colour follows accent
changes without per-component edits.

#### Typography scale

```css
:root {
  /* font families */
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI",
               "Hiragino Sans", "Noto Sans CJK JP", system-ui,
               sans-serif;
  --font-mono: ui-monospace, SFMono-Regular, "SF Mono",
               Menlo, Monaco, Consolas, "Liberation Mono",
               "Noto Sans Mono CJK JP", monospace;

  /* type scale (1.25 modular, base 16px) */
  --type-xs:   0.75rem;  /* 12px — fine print, table cells */
  --type-sm:   0.875rem; /* 14px — secondary body */
  --type-base: 1rem;     /* 16px — body */
  --type-lg:   1.25rem;  /* 20px — section heading */
  --type-xl:   1.5rem;   /* 24px — screen heading */
  --type-2xl:  2rem;     /* 32px — page heading (rare) */

  /* line height */
  --leading-tight:  1.25;
  --leading-normal: 1.5;
}
```

The CJK-aware font-stack is mandatory for Japanese rendering
quality. The mono stack is used for client IDs, hashes, audit
event names, and dev-mode banners.

#### Spacing scale

```css
:root {
  --space-0:  0;
  --space-1:  0.25rem;  /* 4px */
  --space-2:  0.5rem;
  --space-3:  0.75rem;
  --space-4:  1rem;
  --space-5:  1.5rem;
  --space-6:  2rem;
  --space-8:  3rem;
  --space-10: 4rem;
}
```

Components use these exclusively for padding and margin.
No magic numbers (`padding: 13px;`) anywhere.

#### Radius and shadow

```css
:root {
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-pill: 9999px;

  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.04);
  --shadow-md: 0 4px 12px rgba(0, 0, 0, 0.08);
  --shadow-lg: 0 12px 32px rgba(0, 0, 0, 0.12);
}
```

Dark mode shadows are heavier; values are in the dark-mode
override block.

### § 2. Base layer

```css
/* base.css */

html {
  font-family: var(--font-sans);
  font-size: 16px;
  line-height: var(--leading-normal);
  color: var(--fg-default);
  background: var(--surface-default);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

body {
  margin: 0;
}

*, *::before, *::after { box-sizing: border-box; }

a {
  color: var(--accent-emphasis);
  text-decoration: underline;
  text-underline-offset: 2px;
}
a:hover { color: var(--accent-default); }

/* Focus ring — non-negotiable per RFC 017 § 10 */
:focus { outline: none; }   /* suppress default until :focus-visible kicks in */
:focus-visible {
  outline: 2px solid var(--state-focus);
  outline-offset: 2px;
  border-radius: var(--radius-sm);
}

/* Reduced motion */
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.001ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.001ms !important;
    scroll-behavior: auto !important;
  }
}

/* Dev-mode banner — RFC 017 § 9 */
.dev-banner {
  position: sticky;
  top: 0;
  z-index: 100;
  background: var(--warning-default);
  color: #1F1B24;  /* fixed black for contrast on yellow regardless of theme */
  font-family: var(--font-mono);
  font-size: var(--type-sm);
  padding: var(--space-2) var(--space-4);
  border-bottom: 2px solid #1F1B24;
}
.dev-banner--non-loopback {
  background: var(--danger-default);
  color: #FFFFFF;
}
```

The `:focus { outline: none }` reset is paired with
`:focus-visible { outline: 2px ... }` so keyboard users still
see the ring while mouse users do not. This is the
recommended pattern; it is not the same as suppressing focus
indicators entirely.

### § 3. Component primitives

The component CSS file is intentionally small — the
deliverables describe shapes, not a full component library
like shadcn/ui. The shapes:

```css
/* components.css */

/* Button */
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  font-size: var(--type-base);
  font-weight: 500;
  line-height: var(--leading-tight);
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: var(--accent-default);
  color: var(--fg-on-accent);
  cursor: pointer;
  transition: background-color 120ms ease;
}
.btn:hover { background: var(--accent-emphasis); }
.btn:disabled,
.btn[aria-disabled="true"] {
  background: var(--state-disabled);
  cursor: not-allowed;
}

.btn--secondary {
  background: var(--surface-elevated);
  color: var(--fg-default);
  border-color: var(--border-default);
}
.btn--secondary:hover { background: var(--state-hover); }

.btn--danger {
  background: var(--danger-default);
}
.btn--danger:hover {
  background: color-mix(in srgb, var(--danger-default), black 8%);
}

/* Form field cluster */
.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-bottom: var(--space-4);
}
.field__label {
  font-size: var(--type-sm);
  font-weight: 500;
  color: var(--fg-default);
}
.field__input {
  padding: var(--space-2) var(--space-3);
  font-size: var(--type-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-elevated);
  color: var(--fg-default);
  transition: border-color 120ms ease;
}
.field__input:focus-visible {
  border-color: var(--accent-default);
  outline-offset: -1px;       /* inset to avoid double-ring */
}
.field__help {
  font-size: var(--type-sm);
  color: var(--fg-muted);
}
.field__error {
  font-size: var(--type-sm);
  color: var(--danger-default);
}
.field--invalid .field__input {
  border-color: var(--danger-default);
}

/* Badge */
.badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-2);
  font-size: var(--type-xs);
  font-weight: 500;
  line-height: var(--leading-tight);
  border-radius: var(--radius-pill);
  background: var(--surface-subtle);
  color: var(--fg-muted);
}
.badge--ok      { background: color-mix(in srgb, var(--success-default), white 80%); color: var(--success-default); }
.badge--warn    { background: color-mix(in srgb, var(--warning-default), white 80%); color: var(--warning-default); }
.badge--danger  { background: var(--danger-subtle); color: var(--danger-default); }
.badge--info    { background: color-mix(in srgb, var(--info-default), white 80%); color: var(--info-default); }

/* Banner — page-top status messages */
.banner {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  border-left: 3px solid;
}
.banner--success { background: color-mix(in srgb, var(--success-default), white 88%); border-color: var(--success-default); }
.banner--warn    { background: color-mix(in srgb, var(--warning-default), white 88%); border-color: var(--warning-default); }
.banner--danger  { background: var(--danger-subtle);  border-color: var(--danger-default); }
.banner--info    { background: color-mix(in srgb, var(--info-default), white 88%); border-color: var(--info-default); }

/* Modal — confirmation screens per RFC 017 § 3 */
.modal-backdrop {
  position: fixed; inset: 0;
  background: rgba(0, 0, 0, 0.32);
  display: flex; align-items: center; justify-content: center;
  z-index: 50;
}
.modal {
  background: var(--surface-elevated);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  max-width: 30rem;
  width: 100%;
  padding: var(--space-5);
}

/* Tabs — settings tab structure per RFC 017 § 6 */
.tabs {
  display: flex;
  gap: var(--space-1);
  border-bottom: 1px solid var(--border-muted);
}
.tab {
  padding: var(--space-2) var(--space-4);
  font-size: var(--type-base);
  color: var(--fg-muted);
  border-bottom: 2px solid transparent;
  cursor: pointer;
  background: transparent;
  border-top: 0; border-left: 0; border-right: 0;
}
.tab[aria-selected="true"] {
  color: var(--accent-default);
  border-bottom-color: var(--accent-default);
}
```

Every primitive uses tokens. Replacing the palette colours
re-themes everything for free.

### § 4. Dangerous-operation visual contract

RFC 017 § 3 says reversibility is shown as a coloured badge,
and the colour is *not* the only signal. The components above
satisfy this:

```html
<span class="badge badge--ok">Recoverable</span>
<span class="badge badge--danger">Not recoverable</span>
```

The text labels are mandatory. Code review must reject any
`.badge--danger` without a label. (Implementer cue: the
`.badge` class can take an `aria-label` if the visible text
is just an icon, but the deliverables specify text-with-icon,
not icon-only.)

The confirmation screen pattern from RFC 017 § 3 lays out as:

```html
<div class="modal-backdrop" role="dialog" aria-labelledby="d-title">
  <div class="modal">
    <h2 id="d-title" class="text-xl">{translated_title}</h2>
    <p class="text-base">{impact_summary}</p>
    <p class="badge badge--{recoverable ? 'ok' : 'danger'}">
      {translated_label}
    </p>
    <div class="modal__actions">
      <button class="btn btn--secondary">{cancel_label}</button>
      <button class="btn btn--danger">{confirm_verb}</button>
    </div>
  </div>
</div>
```

The `confirm_verb` text is the verb-noun phrase ("Confirm
disable", "Confirm delete"), not "OK" or "Yes" — RFC 017
§ 3's contract.

### § 5. Dark mode

Three implementation strategies:

- **(A) `prefers-color-scheme` only.** OS-level setting wins.
- **(B) `data-theme="dark"` on `<html>`, set by user
  preference cookie/storage, fallback to `prefers-color-scheme`.**
- **(C) Per-user preference column in `users`, falls back to
  cookie, falls back to `prefers-color-scheme`.**

Recommend **(B)**: a small toggle in the `/me/security`
profile page sets the cookie; the server emits the cookie
value as `data-theme` on the html element during SSR.
Operators who do not opt-in get the OS setting. No new
schema column; the cookie is stateless. Per-user persistence
(C) can be added later if there is demand.

The CSS:

```css
:root { /* light tokens */ }

[data-theme="dark"],
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { /* dark token overrides */ }
}
```

The double rule lets `data-theme="light"` explicitly opt out
of `prefers-color-scheme: dark` for users who want light mode
on a dark-mode OS.

### § 6. Motion

Motion is functional, not decorative:

- Hover transitions: 120ms ease, opacity/colour only.
- Focus ring: instant (no transition).
- Modal open/close: 160ms ease-out fade. The backdrop fades
  from 0 to 0.32 opacity; the modal scales from 0.96 to 1.
- Banner appearance: 200ms slide-down + fade-in.
- No motion at all under `prefers-reduced-motion: reduce`
  (handled at base layer).

No spinners or skeleton loaders for the SSR shell. The state-
copy `loading` variant from RFC 017 § 4 covers the cases
where async UI is needed (passkey enrolment, audit-log
fetches).

### § 7. Migration plan

The existing `sui-id-web` components have inline styles
(per the codebase review). Migration is mechanical:

1. Land tokens / base / components CSS files. They are
   harmless if no component consumes them.
2. Migrate one screen at a time, starting with the most
   visible: `/admin` dashboard, then `/admin/login`, then
   the setup wizard. Each migration is one PR.
3. On each migration PR, remove the inline styles from the
   migrated screen's component files.
4. After every screen migrates, a CI check rejects new inline
   `style=` attributes containing colour values.

The migration is per-screen, so the half-migrated state is
ugly but functional: old screens look the same as before, new
screens look like the design system.

The full migration is in scope for this RFC. If reviewer
bandwidth requires it to ship in slices, the first slice
(items 1–2 above) is the minimum that lets *new* admin-domain
work (RFC 002 step-2) inherit the system.

## Tests

Visual regression testing is the right tool here, but
introducing it (typically Playwright + per-screen snapshot
comparison) is a meaningful investment. Lightweight
alternatives that ship in this RFC:

1. **Token-presence test.** A unit test reads the compiled
   CSS and asserts the variables defined match the palette
   file. Regenerating from the palette without updating the
   CSS fails this test.
2. **No-hardcoded-colour lint.** A CI check (`grep -E '#[0-
   9a-fA-F]{6}'`) on `crates/sui-id-web/src/**/*.rs` (the
   Leptos components) returns zero matches outside the
   tokens CSS file. Style strings using hex literals fail
   the build.
3. **Focus-ring presence test.** An e2e test loads each
   screen, tabs through the interactive elements, and
   asserts each has a non-empty `outline` computed style.

Visual regression as a follow-up: track in operators or
contributors docs once Playwright is in CI for any reason.

## Security considerations

CSS itself is not a security boundary, but two notes:

- **CSP.** The CSS files ship inline-or-linked from the same
  origin. No external CDN. The existing CSP allows
  `style-src 'self'` (verify against
  `crates/sui-id/src/security_headers.rs`); if it currently
  allows `'unsafe-inline'` for legacy inline styles, the
  migration in § 7 is what lets us tighten it. After full
  migration, `'unsafe-inline'` can drop.
- **Visual phishing.** Dev-mode banners use distinct colours
  (yellow normal, red for non-loopback bind) precisely so
  that an operator cannot mistake a dev instance for
  production. Removing or styling away the banner is a
  security regression.

## Multiple implementation steps

- **Step 1 (lands first).** Tokens CSS, base CSS, components
  CSS files. No component migration. Other RFCs can start
  consuming tokens.
- **Step 2.** Migrate the highest-traffic admin screens (per
  the deliverables: dashboard, users, clients, settings).
- **Step 3.** Migrate the rest (auth screens, self-service,
  setup, audit).
- **Step 4.** Tighten CSP, drop the inline-colour escape
  hatch, add the no-hardcoded-colour lint to CI.

## Open questions

1. **Theme switcher placement.** § 5(B) recommends a toggle
   on `/me/security`. Should the same toggle appear on the
   admin dashboard? RFC 017 § 5 says the dashboard is a
   dispatcher, not a workplace — so probably not. The
   `/me/security` placement is sufficient for an admin who
   also wants dark mode on their own session.
2. **`color-mix` browser support.** `color-mix(in srgb, …)`
   is supported in current Firefox, Safari, Chrome, but not
   in older browsers. sui-id targets evergreen browsers
   (the threat model assumes WebAuthn-capable, which means
   recent). Acceptable. If a deployment hits an older
   browser, fallback values can be supplied by stylelint.
3. **Iconography.** The deliverables show icons in dangerous-
   op badges and dev banners but the icon set is not
   specified. Recommend Lucide (Apache-2.0, matches sui-id's
   licence) shipped as inlined SVGs to avoid a webfont
   dependency. Final choice deferred to the implementer of
   step 2.
4. **Print styles.** Out of scope. sui-id pages are not
   typically printed. Re-evaluate if an audit-log export
   request comes in.

## Addendum — Text selection contrast (v0.29.x observation)

Operators reported that mouse text selection is difficult to see, especially
on UUID/credential values rendered in the admin panel (Client ID field was
specifically cited). Root cause: the current stylesheet does not set
`::selection` styles, so the browser default is used — which can be very
low-contrast depending on OS/theme and the element's background colour.

**Required change (part of this RFC's deliverables):**

```css
/* Light mode */
::selection {
    background-color: var(--color-primary-200);  /* defined in token set */
    color: var(--color-text-primary);
}

/* Dark mode — usually handled automatically if the token resolves correctly,
   but explicit overrides may be needed for high-contrast themes */
@media (prefers-color-scheme: dark) {
    ::selection {
        background-color: var(--color-primary-600);
        color: var(--color-text-inverted);
    }
}
```

The selection colour must meet WCAG 2.1 SC 1.4.3 contrast requirements
(4.5:1 for normal text, 3:1 for large text) in both light and dark modes.

This fix is separate from and complementary to RFC 028 (copy buttons),
which addresses the same ergonomic pain point from the interaction side.
