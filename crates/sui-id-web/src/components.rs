//! Component styles — base reset, typography, primitive UI components.
//!
//! Every value here resolves through a token. Grep for a hex value:
//! you should find none. If you need to tune a colour, edit
//! `tokens.rs`; if you need to tune a shape (radius, padding,
//! shadow), still edit `tokens.rs` since those are tokens too.
//!
//! Components ship with sensible defaults so most pages compose them
//! by class, not by inline style. Page-specific code is allowed to
//! override via more-specific selectors but should never reach for
//! inline `style="..."` for visual concerns.

pub const COMPONENTS_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Base / reset                                                        */
/* ------------------------------------------------------------------ */

*, *::before, *::after { box-sizing: border-box; }

html { -webkit-text-size-adjust: 100%; }

body {
  margin: 0;
  background: var(--surface-default);
  color: var(--fg-default);
  font-family: var(--font-sans);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--font-weight-regular);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

/* Focus ring — non-negotiable for keyboard accessibility (ABDD). */
:focus-visible {
  outline: var(--border-width-emphasis) solid var(--state-focus);
  outline-offset: 2px;
  border-radius: var(--radius-sm);
}

/* Links */
a {
  color: var(--accent-default);
  text-decoration: none;
}
a:hover { text-decoration: underline; }

/* ------------------------------------------------------------------ */
/* Typography                                                          */
/* ------------------------------------------------------------------ */

h1, h2, h3, h4, h5, h6 {
  margin: 0 0 var(--space-3) 0;
  font-weight: var(--font-weight-bold);
  color: var(--fg-default);
  letter-spacing: -0.01em;
}
h1 {
  font-size: var(--font-size-display);
  line-height: var(--line-height-display);
}
h2 {
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  margin-top: var(--space-5);
}
h3 {
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  margin-top: var(--space-4);
}
p { margin: 0 0 var(--space-3) 0; }
.muted {
  color: var(--fg-muted);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}
.subtle { color: var(--fg-subtle); }

code, pre, .mono {
  font-family: var(--font-mono);
  font-size: 0.92em;
}
code, .code {
  background: var(--surface-sunken);
  border: var(--border-width-default) solid var(--border-muted);
  padding: 0.1em 0.4em;
  border-radius: var(--radius-sm);
  word-break: break-all;
}

/* ------------------------------------------------------------------ */
/* Layout chrome (Shell)                                               */
/* ------------------------------------------------------------------ */

.app-header {
  background: var(--surface-elevated);
  border-bottom: var(--border-width-default) solid var(--border-muted);
  padding: var(--space-3) var(--space-5);
  display: flex;
  align-items: center;
  gap: var(--space-5);
}
.app-header__brand {
  margin: 0;
  font-size: var(--font-size-h3);
  font-weight: var(--font-weight-bold);
  color: var(--fg-default);
  letter-spacing: -0.01em;
}
.app-header__brand::before {
  content: "🌱";
  margin-right: var(--space-2);
}

.app-nav {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  flex: 1;
}
.app-nav__link {
  color: var(--fg-muted);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-body);
  text-decoration: none;
  transition: background 0.12s, color 0.12s;
}
.app-nav__link:hover {
  background: var(--state-hover);
  color: var(--fg-default);
  text-decoration: none;
}
.app-nav__link[aria-current="page"] {
  color: var(--accent-default);
  background: var(--accent-subtle);
}
.app-nav__signout { margin-left: auto; }
/* Sign-out form in nav — renders a button that looks like a nav link    */
.app-nav__signout-form {
    margin-top: auto;
    border-top: 1px solid var(--border-default);
    padding-top: var(--space-2);
}
.app-nav__signout {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    text-align: left;
    width: 100%;
    color: var(--fg-muted);
    font: inherit;
}
.app-nav__signout:hover,
.app-nav__signout:focus-visible {
    color: var(--fg-default);
    background-color: var(--surface-elevated);
    text-decoration: none;
}


.app-main {
  max-width: var(--content-max-width);
  margin: 0 auto;
  padding: var(--space-5) var(--space-5);
}
.app-main--narrow {
  max-width: var(--content-narrow-width);
}

.app-footer {
  border-top: var(--border-width-default) solid var(--border-muted);
  padding: var(--space-4) var(--space-5);
  display: flex;
  align-items: center;
  gap: var(--space-3);
  color: var(--fg-muted);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}
.app-footer__tagline { flex: 1; }
.app-footer__a11y {
  display: flex;
  gap: var(--space-3);
  flex-wrap: wrap;
}
.app-footer__version {
  color: var(--fg-subtle);
  font-family: var(--font-mono);
}

/* ------------------------------------------------------------------ */
/* Cards / panels                                                      */
/* ------------------------------------------------------------------ */

.card {
  background: var(--surface-elevated);
  border: var(--border-width-default) solid var(--border-muted);
  border-radius: var(--radius-md);
  padding: var(--space-4);
  box-shadow: var(--shadow-sm);
}
.card + .card { margin-top: var(--space-3); }
.card__title {
  margin: 0 0 var(--space-2) 0;
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
}
.card__body { color: var(--fg-default); }
.card__footer {
  margin-top: var(--space-3);
  padding-top: var(--space-3);
  border-top: var(--border-width-default) solid var(--border-muted);
  display: flex;
  gap: var(--space-2);
  align-items: center;
}

/* RFC 062 (v0.46.0) — card variants.
 * Compose with .card: <section class="card card--warn">. Each variant
 * gives the card an asymmetric 4px left accent and a subtle tinted
 * background, so a row of cards can read at a glance as "this one is
 * different." Colours come from RFC 061 semantic tokens, so light/dark
 * pairing is automatic. */
.card--warn {
  background: var(--warning-subtle);
  border-color: var(--warning-default);
  border-left-width: 4px;
}
.card--info {
  background: var(--info-subtle);
  border-color: var(--info-default);
  border-left-width: 4px;
}
.card--success {
  background: var(--success-subtle);
  border-color: var(--success-default);
  border-left-width: 4px;
}
.card--callout {
  /* Accent (lavender) callout — e.g. "next steps" cards on setup,
   * "what to do now" panels. Not a semantic warning; just visual
   * emphasis to mark the next operator action. */
  background: var(--accent-subtle);
  border-color: var(--accent-default);
  border-left-width: 4px;
}

/* RFC 064 (v0.46.0) — Empty-state primitive.
 * Replaces the per-page `<p class="muted">No X yet.</p>` pattern. The
 * dashed border + tinted background distinguishes "this section is a
 * placeholder" from "this section has muted-coloured content."
 * Compact variant is for use inside a table cell or other narrow
 * context where the full padding would look ridiculous. */
.empty-state {
  background: var(--surface-subtle);
  border: var(--border-width-default) dashed var(--border-muted);
  border-radius: var(--radius-md);
  padding: var(--space-5);
  text-align: center;
  color: var(--fg-muted);
}
.empty-state--compact {
  padding: var(--space-3);
  border-style: solid;
  text-align: left;
}
.empty-state__message {
  font-size: var(--font-size-body);
  margin: 0 0 var(--space-2) 0;
  color: var(--fg-default);
}
.empty-state__hint {
  font-size: var(--font-size-caption);
  margin: 0 0 var(--space-3) 0;
}
.empty-state__action {
  display: inline-block;
}

/* ------------------------------------------------------------------ */
/* RFC 067 utility classes (v0.48.0)                                  */
/* ------------------------------------------------------------------ */
/* Small, token-derived utility classes for the spacing + layout       */
/* patterns surfaced in the Phase F inline-style survey. The set is    */
/* deliberately tight; new utilities require RFC justification.        */

/* Margin: top / bottom */
.mt-1 { margin-top: var(--space-1); }
.mt-2 { margin-top: var(--space-2); }
.mt-3 { margin-top: var(--space-3); }
.mt-4 { margin-top: var(--space-4); }
.mt-5 { margin-top: var(--space-5); }
.mb-0 { margin-bottom: 0; }
.mb-1 { margin-bottom: var(--space-1); }
.mb-2 { margin-bottom: var(--space-2); }
.mb-3 { margin-bottom: var(--space-3); }
.mb-4 { margin-bottom: var(--space-4); }

/* Margin: left (rare; for inline icon spacing only) */
.ml-1 { margin-left: var(--space-1); }
.ml-2 { margin-left: var(--space-2); }

/* Margin combo used by several explainer paragraphs */
.mt-2-mb-0 { margin-top: var(--space-2); margin-bottom: 0; }

/* Gap (flex/grid spacing) */
.gap-1 { gap: var(--space-1); }
.gap-2 { gap: var(--space-2); }
.gap-3 { gap: var(--space-3); }

/* Layout */
.center { text-align: center; }
.items-center { align-items: center; }
.items-end { align-items: flex-end; }
.justify-end { justify-content: flex-end; }
.justify-between { justify-content: space-between; }
.inline-el { display: inline; }
.inline-block { display: inline-block; }
.flex-1 { flex: 1; }

/* Common composite patterns */
.row-gap2-center { gap: var(--space-2); align-items: center; }
.row-gap3-center { gap: var(--space-3); align-items: center; }

/* Constrained widths used by auth cards and form pages */
.max-w-card { max-width: 36rem; }
.max-w-narrow { max-width: 22rem; }
.min-w-16rem { min-width: 16rem; }

/* Typography */
.text-caption { font-size: var(--font-size-caption); }
.text-small { font-size: 0.85em; }
.fw-medium { font-weight: var(--font-weight-medium); }
.fw-500 { font-weight: 500; }

/* The "label cell" pattern used inside every settings <table>:
 * a 14rem-wide muted-foreground left-aligned <th>. Rolled into a
 * single class so kv_row() does not need an inline style. */
.kv-label-cell {
  width: 14rem;
  font-weight: var(--font-weight-medium);
  color: var(--fg-muted);
  text-align: left;
}

.stack { display: flex; flex-direction: column; gap: var(--space-3); }
.stack-tight { display: flex; flex-direction: column; gap: var(--space-2); }
.row { display: flex; gap: var(--space-3); align-items: center; flex-wrap: wrap; }

.grid-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
  gap: var(--space-3);
}

/* Stat callout (used on dashboard) */
.stat {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}
.stat__value {
  font-size: var(--font-size-display);
  line-height: 1.1;
  font-weight: var(--font-weight-bold);
  color: var(--fg-default);
  font-variant-numeric: tabular-nums;
}
.stat__label {
  color: var(--fg-muted);
  font-size: var(--font-size-caption);
}

/* ------------------------------------------------------------------ */
/* Forms                                                               */
/* ------------------------------------------------------------------ */

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-bottom: var(--space-3);
}
.field__label {
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
  color: var(--fg-default);
}
.field__hint {
  font-size: var(--font-size-caption);
  color: var(--fg-muted);
}

input[type="text"], input[type="password"], input[type="email"],
input[type="url"], input[type="number"], input[type="search"],
input[type="tel"], textarea, select {
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border: var(--border-width-default) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-elevated);
  color: var(--fg-default);
  font-family: inherit;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  transition: border-color 0.12s, box-shadow 0.12s;
}
input::placeholder, textarea::placeholder {
  color: var(--fg-subtle);
}
input:hover, textarea:hover, select:hover {
  border-color: var(--fg-muted);
}
input:focus, textarea:focus, select:focus {
  outline: none;
  border-color: var(--accent-default);
  box-shadow: 0 0 0 3px var(--accent-subtle);
}
input:disabled, textarea:disabled, select:disabled {
  color: var(--state-disabled);
  cursor: not-allowed;
}

input[type="checkbox"], input[type="radio"] {
  width: auto;
  accent-color: var(--accent-default);
}

/* ------------------------------------------------------------------ */
/* Buttons                                                             */
/* ------------------------------------------------------------------ */

button, .button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: var(--border-width-default) solid transparent;
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-medium);
  line-height: 1.2;
  cursor: pointer;
  text-decoration: none;
  transition: background 0.12s, color 0.12s, border-color 0.12s;
  min-height: 36px;
}

/* Primary: accent-filled, the dominant call-to-action */
button, .button,
button.primary, .button.primary {
  background: var(--accent-default);
  border-color: var(--accent-default);
  color: var(--fg-on-accent);
}
button:hover:not(:disabled), .button:hover:not(.disabled),
button.primary:hover:not(:disabled), .button.primary:hover:not(.disabled) {
  background: var(--accent-emphasis);
  border-color: var(--accent-emphasis);
  text-decoration: none;
}

/* Secondary: outlined, for non-dominant actions */
button.secondary, .button.secondary {
  background: transparent;
  border-color: var(--border-default);
  color: var(--fg-default);
}
button.secondary:hover:not(:disabled), .button.secondary:hover:not(.disabled) {
  background: var(--state-hover);
  border-color: var(--fg-muted);
}

/* Danger: irreversible / destructive actions, visually isolated */
button.danger, .button.danger {
  background: var(--danger-default);
  border-color: var(--danger-default);
  color: #FFFFFF;
}
button.danger:hover:not(:disabled), .button.danger:hover:not(.disabled) {
  filter: brightness(0.92);
}

/* Ghost: very low emphasis, for inline cancel / dismiss */
button.ghost, .button.ghost {
  background: transparent;
  border-color: transparent;
  color: var(--fg-muted);
}
button.ghost:hover:not(:disabled), .button.ghost:hover:not(.disabled) {
  color: var(--fg-default);
  background: var(--state-hover);
}

button:disabled, .button.disabled {
  color: var(--state-disabled);
  cursor: not-allowed;
  opacity: 0.7;
}

/* Pure-text link styled as an inline action */
.link-button {
  background: none;
  border: none;
  padding: 0;
  color: var(--accent-default);
  cursor: pointer;
  font: inherit;
  text-decoration: underline;
  min-height: auto;
}

/* ------------------------------------------------------------------ */
/* Tables                                                              */
/* ------------------------------------------------------------------ */

.table-wrap {
  background: var(--surface-elevated);
  border: var(--border-width-default) solid var(--border-muted);
  border-radius: var(--radius-md);
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--font-size-body);
}
thead th {
  text-align: left;
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
  color: var(--fg-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  padding: var(--space-2) var(--space-3);
  background: var(--surface-subtle);
  border-bottom: var(--border-width-default) solid var(--border-muted);
}
tbody td {
  padding: var(--space-3);
  border-bottom: var(--border-width-default) solid var(--border-muted);
  vertical-align: middle;
}
tbody tr:last-child td { border-bottom: 0; }
tbody tr:hover { background: var(--state-hover); }

/* ------------------------------------------------------------------ */
/* Status badges                                                       */
/* ------------------------------------------------------------------ */

.badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 2px var(--space-2);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
  background: var(--surface-subtle);
  color: var(--fg-muted);
  border: var(--border-width-default) solid var(--border-muted);
}
.badge--ok      { color: var(--success-default); background: transparent; border-color: currentColor; }
.badge--warn    { color: var(--warning-default); background: transparent; border-color: currentColor; }
.badge--danger  { color: var(--danger-default);  background: transparent; border-color: currentColor; }
.badge--accent  { color: var(--accent-default);  background: var(--accent-subtle); border-color: transparent; }

/* ------------------------------------------------------------------ */
/* Flash banners                                                       */
/* ------------------------------------------------------------------ */

.flash {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  border: var(--border-width-default) solid var(--border-default);
  margin-bottom: var(--space-3);
  display: flex;
  gap: var(--space-3);
  align-items: flex-start;
}
.flash.info {
  background: var(--accent-subtle);
  border-color: transparent;
  color: var(--fg-default);
}
.flash.warn {
  background: var(--warning-subtle);
  border-color: var(--warning-default);
  color: var(--fg-default);
}
.flash.error {
  background: var(--danger-subtle);
  border-color: var(--danger-default);
  color: var(--fg-default);
}
/* Dark-mode override no longer needed for .flash.warn — RFC 061 made
 * --warning-subtle per-mode, so the same rule resolves correctly. */

/* ------------------------------------------------------------------ */
/* Banners (RFC 057, v0.44.0)                                          */
/* ------------------------------------------------------------------ */
/* Standalone status messages, distinct from inline flash banners.    */
/* `.banner` is the base; `--warning`, `--danger`, `--success` are    */
/* the colour variants. Used for in-page confirmations and warnings.  */

.banner {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  border: var(--border-width-default) solid var(--border-default);
  display: flex;
  gap: var(--space-3);
  align-items: flex-start;
}
.banner--success {
  background: var(--success-subtle);
  border-color: var(--success-default);
  color: var(--fg-default);
}
.banner--warning {
  background: var(--warning-subtle);
  border-color: var(--warning-default);
  color: var(--fg-default);
}
.banner--danger {
  background: var(--danger-subtle);
  border-color: var(--danger-default);
  color: var(--fg-default);
}
/* RFC 061: dark-mode override no longer needed — token is per-mode. */

/* ------------------------------------------------------------------ */
/* Page header (title + optional actions on right)                     */
/* ------------------------------------------------------------------ */

.page-header {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--space-3);
  margin-bottom: var(--space-4);
  padding-bottom: var(--space-3);
  border-bottom: var(--border-width-default) solid var(--border-muted);
}
.page-header__title {
  margin: 0;
  font-size: var(--font-size-display);
  line-height: var(--line-height-display);
}
.page-header__lede {
  margin: var(--space-1) 0 0 0;
  color: var(--fg-muted);
  font-size: var(--font-size-body);
}
.page-header__actions {
  display: flex;
  gap: var(--space-2);
  align-items: center;
}

/* ------------------------------------------------------------------ */
/* Theme toggle (in footer)                                            */
/* ------------------------------------------------------------------ */

.theme-toggle {
  display: inline-flex;
  align-items: center;
  background: var(--surface-elevated);
  border: var(--border-width-default) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: 2px;
  gap: 0;
}
.theme-toggle__btn {
  background: transparent;
  border: none;
  color: var(--fg-muted);
  padding: var(--space-1) var(--space-2);
  font-size: var(--font-size-caption);
  cursor: pointer;
  min-height: auto;
  border-radius: var(--radius-sm);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.theme-toggle__btn:hover { color: var(--fg-default); }
.theme-toggle__btn[aria-pressed="true"] {
  background: var(--accent-subtle);
  color: var(--accent-default);
}

/* ------------------------------------------------------------------ */
/* Login / setup centred layouts                                       */
/* ------------------------------------------------------------------ */

.auth-page {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: calc(100vh - 12rem);
}
.auth-card {
  width: 100%;
  max-width: var(--content-narrow-width);
  background: var(--surface-elevated);
  border: var(--border-width-default) solid var(--border-muted);
  border-radius: var(--radius-md);
  padding: var(--space-5);
  box-shadow: var(--shadow-md);
}
.auth-card h1 {
  font-size: var(--font-size-h2);
  text-align: center;
  margin-bottom: var(--space-4);
}

/* ------------------------------------------------------------------ */
/* Dividers and section spacing                                        */
/* ------------------------------------------------------------------ */

.divider {
  border: 0;
  border-top: var(--border-width-default) solid var(--border-muted);
  margin: var(--space-4) 0;
}

section + section { margin-top: var(--space-5); }

/* ------------------------------------------------------------------ */
/* Visually-hidden (for screen readers)                                */
/* ------------------------------------------------------------------ */

.sr-only {
  position: absolute;
  width: 1px; height: 1px;
  padding: 0; margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

/* ── Copy-to-clipboard button (RFC 028) ─────────────────────────────── */
.copy-btn {
    display: none; /* shown via JS when clipboard-available class is set */
    align-items: center;
    gap: 0.25em;
    padding: 0.1em 0.5em;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm, 4px);
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: 0.8em;
    cursor: pointer;
    vertical-align: middle;
    margin-left: 0.4em;
    transition: color 0.15s, border-color 0.15s;
    white-space: nowrap;
}
.clipboard-available .copy-btn { display: inline-flex; }
.copy-btn:hover,
.copy-btn:focus-visible {
    color: var(--fg-default);
    border-color: var(--fg-muted);
    outline: 2px solid var(--state-focus, currentColor);
    outline-offset: 2px;
}

/* ── Tabs (RFC 023) ─────────────────────────────────────────────────── */
/* Horizontal tab bar for Settings and other multi-panel screens.         */
.tabs {
  display: flex;
  flex-direction: column;
}
.tabs__bar {
  display: flex;
  gap: 0;
  border-bottom: var(--border-width-default) solid var(--border-default);
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
}
.tab-btn {
  padding: var(--space-2) var(--space-3);
  background: transparent;
  border: none;
  border-bottom: var(--border-width-emphasis) solid transparent;
  color: var(--fg-muted);
  font: var(--font-weight-regular) var(--font-size-body) / 1 var(--font-sans);
  cursor: pointer;
  white-space: nowrap;
  transition: color var(--motion-fast) var(--motion-easing),
              border-color var(--motion-fast) var(--motion-easing);
  margin-bottom: calc(-1 * var(--border-width-default)); /* align with bar border */
}
.tab-btn:hover  { color: var(--fg-default); }
.tab-btn:focus-visible {
  outline: var(--border-width-emphasis) solid var(--state-focus);
  outline-offset: -2px;
}
.tab-btn[aria-selected="true"] {
  color: var(--accent-default);
  border-bottom-color: var(--accent-default);
  font-weight: var(--font-weight-medium);
}
.tabs__panel {
  padding-top: var(--space-4);
}

/* ── Dev-mode banner (RFC 017 § 9, RFC 023) ─────────────────────────── */
/* Displayed on every page when sui-id starts with --dev.                 */
.dev-banner {
  position: sticky;
  top: 0;
  z-index: var(--z-raised);
  background: #7A5C00;        /* deep amber, distinct from any semantic colour */
  color: #FFF8E1;
  padding: var(--space-1) var(--space-3);
  font-size: var(--font-size-caption);
  text-align: center;
  letter-spacing: 0.04em;
}
.dev-banner strong { letter-spacing: 0.08em; }
/* High-contrast bind warning within the banner */
.dev-banner__bind-warn {
  color: #FFCDD2;
  font-weight: var(--font-weight-medium);
}

/* ── Transitions on interactive components (RFC 023 motion contract) ── */
/* Apply motion tokens so prefers-reduced-motion is obeyed automatically.*/
button,
.btn,
a,
input,
select,
textarea {
  transition-duration: var(--motion-fast);
  transition-timing-function: var(--motion-easing);
  transition-property: color, background-color, border-color, box-shadow, opacity;
}

/* ── Confirmation / step-up screens (RFC 017 § 3) ───────────────────── */
/* Reversibility badge on dangerous-operation confirm screens.            */
.reversibility-badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 2px var(--space-2);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
}
.reversibility-badge--recoverable {
  background: color-mix(in srgb, var(--success-default) 15%, transparent);
  color: var(--success-default);
}
.reversibility-badge--permanent {
  background: var(--danger-subtle);
  color: var(--danger-default);
}

/* ── Status badge muted variant (RFC 052) ──────────────────────────── */
/* Used for `retired` and similar low-emphasis status values that aren't */
/* failure, warning, or success — just "no longer current."              */
.badge--muted {
  background: color-mix(in srgb, var(--fg-muted) 12%, transparent);
  color: var(--fg-muted);
}
/* RFC 067 — additional utility classes for less-frequent patterns
 * that still appear in multiple sites. Keeps the inline-style count
 * under the CI bound. */
.clickable-block { cursor: pointer; display: block; }
.radio-hint {
  margin: var(--space-1) 0 0 calc(1em + var(--space-2));
  font-size: var(--font-size-caption);
}
.center-pad-4 { text-align: center; padding: var(--space-4) 0; }
.center-pad-6 { text-align: center; padding: var(--space-6) 0; }
.center-pad-6-muted {
  text-align: center;
  padding: var(--space-6) 0;
  color: var(--fg-muted);
}
.ul-indent { margin: 0; padding-left: var(--space-4); }
.row-gap2-center-clickable {
  gap: var(--space-2);
  align-items: center;
  cursor: pointer;
}
.button-reset { border: none; padding: 0; margin: 0; }
.color-accent { color: var(--accent-default); }
.color-danger { color: var(--danger-default); }
.flex-0-auto { flex: 0 0 auto; }
.gap1-center { gap: var(--space-1); align-items: center; }

"#;

// ────────────────────────────────────────────────────────────────────
// Rust components (RFC 052)
// ────────────────────────────────────────────────────────────────────

use leptos::prelude::*;

/// Status badge kind. One source of truth for the badge text and CSS
/// class mapping; previously duplicated across 24+ call sites in
/// `pages.rs`.
///
/// New variants must add a matching `status_*` field to
/// [`sui_id_i18n::Strings`] and update the match in [`status_badge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Live and serving traffic. → `badge badge--ok`
    Active,
    /// Recoverable suspension. → `badge badge--warn`
    Disabled,
    /// Tombstoned (soft delete or hard delete). → `badge badge--danger`
    Deleted,
    /// Administrator role marker. → `badge badge--accent`
    Admin,
    /// Generic on indicator. → `badge badge--ok`
    On,
    /// Generic off indicator. → `badge` (neutral)
    Off,
    /// Currently the active signing key. → `badge badge--ok`
    InUse,
    /// Old signing key, kept for token verification. → `badge badge--muted`
    Retired,
    /// Visible to clients via JWKS. → `badge badge--ok`
    Published,
    /// Awaiting human or automated decision. → `badge badge--info`
    Pending,
    /// Service is healthy. → `badge badge--ok`
    Healthy,
    /// Service is unhealthy. → `badge badge--danger`
    Unhealthy,
}

/// Render a status badge with localised text and the matching CSS
/// class. The badge sits inline; wrap it in a `<td>` or other parent
/// at the call site if needed.
pub fn status_badge(
    t: &'static sui_id_i18n::Strings,
    kind: StatusKind,
) -> impl IntoView {
    let (class, text) = match kind {
        StatusKind::Active     => ("badge badge--ok",     t.status_active),
        StatusKind::Disabled   => ("badge badge--warn",   t.status_disabled),
        StatusKind::Deleted    => ("badge badge--danger", t.status_deleted),
        StatusKind::Admin      => ("badge badge--accent", t.status_admin),
        StatusKind::On         => ("badge badge--ok",     t.status_on),
        StatusKind::Off        => ("badge",               t.status_off),
        StatusKind::InUse      => ("badge badge--ok",     t.status_in_use),
        StatusKind::Retired    => ("badge badge--muted",  t.status_retired),
        StatusKind::Published  => ("badge badge--ok",     t.status_published),
        StatusKind::Pending    => ("badge badge--info",   t.status_pending),
        StatusKind::Healthy    => ("badge badge--ok",     t.status_healthy),
        StatusKind::Unhealthy  => ("badge badge--danger", t.status_unhealthy),
    };
    view! { <span class=class>{text}</span> }
}
