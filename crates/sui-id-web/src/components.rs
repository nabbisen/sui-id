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

/* Selection */
::selection {
  background: var(--accent-subtle);
  color: var(--fg-default);
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
    border-top: 1px solid var(--color-border);
    padding-top: var(--space-sm);
}
.app-nav__signout {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    text-align: left;
    width: 100%;
    color: var(--color-text-secondary);
    font: inherit;
}
.app-nav__signout:hover,
.app-nav__signout:focus-visible {
    color: var(--color-text-primary);
    background-color: var(--color-surface-raised);
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
  background: rgba(212, 155, 42, 0.10);
  border-color: var(--warning-default);
  color: var(--fg-default);
}
.flash.error {
  background: var(--danger-subtle);
  border-color: var(--danger-default);
  color: var(--fg-default);
}
[data-theme="dark"] .flash.warn {
  background: rgba(230, 184, 92, 0.12);
}

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
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm, 4px);
    background: transparent;
    color: var(--color-text-secondary);
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
    color: var(--color-text-primary);
    border-color: var(--color-text-secondary);
    outline: 2px solid var(--color-focus-ring, currentColor);
    outline-offset: 2px;
}
"#;
