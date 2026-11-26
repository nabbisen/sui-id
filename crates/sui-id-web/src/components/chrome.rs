//! Chrome / shell CSS — the app's outer scaffolding.
//!
//! Owns: base reset (body, focus ring, links), typography (h1–h6, p,
//! .muted, code), the `Shell`-level layout (header, nav, main, footer
//! + a11y badges, sign-out button, tagline), the cross-screen
//!
//! `.page-header` primitive, the footer theme toggle, and the mobile
//! responsive breakpoints that affect the chrome itself.
//!
//! The base + typography rules sit here (rather than a dedicated
//! "foundation" shard) because they are inseparable from the
//! document-level chrome: every page renders inside `<body>`, and
//! every heading is part of the page's chrome contract.
//!
//! Cascade order within this file matches the original v0.49.x
//! `components.rs` so `COMPONENTS_CSS` stays byte-equivalent.

pub const CHROME_BASE_CSS: &str = r#"
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

"#;

pub const CHROME_TYPOGRAPHY_CSS: &str = r#"
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
/* `0.93.0` O6 §4: a destructive confirm page's own consequence -- full
 * contrast (the default `color: var(--fg-default)` a plain `<p>`
 * already has, stated explicitly so this never silently inherits a
 * muted ancestor), medium weight to read as the page's lead sentence,
 * not a side note. `__count` is the figure itself: bold, one step up
 * the type scale, so a number inside the sentence cannot be skipped
 * the way the sentence it used to end as a muted aside could be. */
.confirm-impact {
  color: var(--fg-default);
  font-weight: var(--font-weight-medium);
}
.confirm-impact__count {
  font-weight: var(--font-weight-bold);
  font-size: var(--font-size-h3);
}
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

"#;

pub const CHROME_LAYOUT_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Layout chrome (Shell) — the slim top bar, for Chrome::Account and   */
/* Chrome::Bare/AuthShell. Chrome::Console's own grid shell is below.  */
/* ------------------------------------------------------------------ */

.app-header {
  background: var(--surface-elevated);
  border-bottom: var(--border-width-default) solid var(--border-muted);
  padding: var(--space-3) var(--space-5);
  display: flex;
  align-items: center;
  gap: var(--space-5);
}
/* RFC 149 §2: text, not a heading — exactly one <h1> per page, and the
 * brand is not it. */
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
  min-width: 0;
  justify-content: flex-end;
}
.app-nav__link {
  color: var(--fg-muted);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-body);
  text-decoration: none;
  transition: background 0.12s, color 0.12s;
  white-space: nowrap;
}
.app-nav__link:hover {
  background: var(--state-hover);
  color: var(--fg-default);
  text-decoration: none;
}

/* ------------------------------------------------------------------ */
/* RFC 149 §1/§2: the console's grouped sidebar.                       */
/* ------------------------------------------------------------------ */

/* Breakpoint: 769px, matching the project's existing 768px
 * tablet/phone boundary (CHROME_RESPONSIVE_CSS below) exactly, so the
 * sidebar folds at the same point every other narrow-screen
 * adjustment already does. Narrow-first: the mobile shape is the
 * default, widened by the min-width query. */
.app-shell {
  display: grid;
  grid-template-columns: 1fr;
  min-height: 100vh;
}
.app-shell__body { min-width: 0; }
.app-sidebar--wide { display: none; }
.app-sidebar--narrow {
  display: block;
  border-bottom: var(--border-width-default) solid var(--border-muted);
  background: var(--surface-elevated);
}
@media (min-width: 769px) {
  /* 15.5rem is the design review's 248px at the default 16px, and it
   * grows with the reader's own font size, so its labels still fit. */
  .app-shell { grid-template-columns: 15.5rem 1fr; }
  .app-sidebar--wide {
    display: flex;
    flex-direction: column;
    border-right: var(--border-width-default) solid var(--border-muted);
    background: var(--surface-elevated);
    padding: var(--space-3) var(--space-2);
    /* Pinned to the viewport, not to the grid row's own height — a
     * grid item otherwise stretches to match the main column's full
     * (possibly page-length) height, and margin-top:auto on
     * .app-sidebar__account would then pin the account menu to the
     * BOTTOM OF THE PAGE rather than the bottom of the visible
     * sidebar. Measured: on a page taller than the viewport, the
     * account menu sat below the fold entirely. */
    position: sticky;
    top: 0;
    height: 100vh;
    /* At large font sizes, with the account menu open, the whole
     * column can be taller than the window: it scrolls, so nothing in
     * it is ever out of reach. */
    overflow-y: auto;
  }
  .app-sidebar--narrow { display: none; }
}

.app-sidebar__menu-toggle {
  display: flex;
  align-items: center;
  min-height: 2.75rem;
  padding: var(--space-2) var(--space-3);
  cursor: pointer;
  list-style: none;
  font-weight: var(--font-weight-medium);
}
.app-sidebar__menu-toggle::-webkit-details-marker { display: none; }
.app-sidebar--narrow[open] .app-sidebar__menu-toggle {
  background: var(--state-hover);
  /* Explicit: unchanged from the base rule's own fg-default, but the
     contrast gate checks each rule's own declarations. */
  color: var(--fg-default);
}
.app-sidebar__menu-panel {
  padding: 0 var(--space-3) var(--space-3);
  border-top: var(--border-width-default) solid var(--border-muted);
}

.app-sidebar__brand {
  margin: 0;
  padding: var(--space-2) var(--space-3) var(--space-4);
  font-size: var(--font-size-h3);
  font-weight: var(--font-weight-bold);
  color: var(--fg-default);
  letter-spacing: -0.01em;
}
.app-sidebar__brand::before {
  content: "🌱";
  margin-right: var(--space-2);
}
.app-sidebar__nav {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.app-sidebar__group {
  margin: var(--space-3) 0 var(--space-1);
  padding: 0 var(--space-3);
  font-size: var(--font-size-caption);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--fg-muted);
  font-weight: var(--font-weight-medium);
}
.app-sidebar__link {
  display: flex;
  align-items: center;
  min-height: 2.75rem;
  padding: 0 var(--space-3);
  border-radius: var(--radius-sm);
  color: var(--fg-default);
  text-decoration: none;
}
.app-sidebar__link:hover {
  background: var(--state-hover);
  text-decoration: none;
  /* Explicit: unchanged from the base rule's own fg-default, but the
     contrast gate checks each rule's own declarations. */
  color: var(--fg-default);
}
.app-sidebar__link[aria-current="page"] {
  background: var(--accent-subtle);
  color: var(--accent-default);
  font-weight: var(--font-weight-medium);
}
/* RFC 144 R5: a tint alone marks the current item; under
 * prefers-contrast: more it also gets a solid border. */
@media (prefers-contrast: more) {
  .app-sidebar__link[aria-current="page"] {
    border: var(--border-width-default) solid var(--accent-default);
  }
}
.app-sidebar__account {
  margin-top: auto;
  padding-top: var(--space-3);
  border-top: var(--border-width-default) solid var(--border-muted);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}
.app-sidebar__version {
  display: block;
  padding: var(--space-1) var(--space-3);
  color: var(--fg-muted);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

/* RFC 074/149 §3: the account menu — <details>/<summary>, so it works
 * with JavaScript disabled. Plain links and buttons in a list (no
 * role="menu"/"menuitem": that role promises arrow-key navigation,
 * and no script provides it). */
.user-menu { position: relative; }
.user-menu__toggle {
  list-style: none;
  cursor: pointer;
  white-space: nowrap;
}
.user-menu__toggle::-webkit-details-marker { display: none; }
.user-menu[open] .user-menu__toggle {
  color: var(--fg-default);
  background: var(--state-hover);
}
.user-menu__panel {
  position: absolute;
  right: 0;
  bottom: calc(100% + var(--space-1));
  min-width: 11rem;
  background: var(--surface-elevated);
  border: var(--border-width-default) solid var(--border-default);
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-md);
  z-index: 100;
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: var(--space-1) 0;
  list-style: none;
}
/* The top-bar case (Chrome::Account) opens below the toggle, not
 * above it — there is no "foot of the sidebar" there to be near. */
.app-nav .user-menu__panel {
  bottom: auto;
  top: calc(100% + var(--space-1));
}
/* In the sidebar (both copies), the menu opens in place, below its
 * toggle, and takes room from the item list above it, which scrolls.
 * Floated above the toggle instead, it covered the items, and at 200%
 * font it ran off the top and left of the window, where a sticky
 * sidebar cannot be scrolled to (measured, Q2's review). */
.app-sidebar .user-menu__panel {
  position: static;
  margin-top: var(--space-1);
  min-width: 0;
  box-shadow: none;
}
.user-menu__item {
  display: flex;
  align-items: center;
  /* RFC 144 §4: 44×44 CSS px for standalone controls — a menu item is
   * one. display:flex + align-items:center keeps the label vertically
   * centred in the taller box. */
  min-height: 2.75rem;
  padding: var(--space-2) var(--space-4);
  color: var(--fg-default);
  font: inherit;
  font-size: var(--font-size-body);
  text-decoration: none;
  background: none;
  border: none;
  cursor: pointer;
  text-align: left;
  /* A <button> centres its content by default; the menu's links and
   * buttons line up as one list. */
  justify-content: flex-start;
  width: 100%;
}
.user-menu__item:hover,
.user-menu__item:focus-visible {
  background: var(--state-hover);
  /* Explicit: unchanged from the base rule's own fg-default, but the
     contrast gate checks each rule's own declarations. */
  color: var(--fg-default);
  /* The outline is kept: removing it here outranked the global
   * :focus-visible ring, leaving a keyboard user only the hover tint. */
}
.user-menu__form { display: contents; }
/* RFC 149 §3: sign-out separated from the rest, with a divider. */
.user-menu__signout { border-top: var(--border-width-default) solid var(--border-muted); }
/* `.user-menu__panel`'s <li> children need no marker or inset. */
.user-menu__panel > li { list-style: none; margin: 0; }
/* RFC 149 §3: the theme row — its visible name, then its three buttons
 * as a labelled group, so "Auto" is never read without "Theme". */
.user-menu__theme { padding-top: var(--space-2); }
.user-menu__theme-label {
  display: block;
  padding: 0 var(--space-4);
  color: var(--fg-muted);
  font-size: var(--font-size-caption);
}
.user-menu__theme-buttons { display: flex; flex-direction: column; }
.user-menu__theme .theme-no-js-note {
  margin: 0;
  padding: var(--space-1) var(--space-4) var(--space-2);
}

.app-main {
  max-width: var(--content-max-width);
  margin: 0 auto;
  padding: var(--space-5) var(--space-5);
}
.app-main--narrow {
  max-width: var(--content-narrow-width);
}

"#;

pub const CHROME_PAGE_HEADER_CSS: &str = r#"
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
/* RFC 149 §1: the group a settings page belongs to, named above its
 * own heading — the sidebar no longer carries a tab row to say so. */
.page-header__group {
  margin: 0 0 var(--space-1) 0;
  color: var(--fg-muted);
  font-size: var(--font-size-caption);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  font-weight: var(--font-weight-medium);
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
/* The icon inside a labelled button (RFC 147 §1): the label is the
 * button's name, and the icon is aria-hidden beside it. There is no
 * icon-only button: the retired icon-only class was a fixed 32px circle,
 * which a label cannot fit. */
.button__icon {
  font-size: 1.25rem;
  line-height: 1;
}
/* CSS-only toggle for create/action panels (users, clients, signing-keys).
 * A hidden <input type="checkbox" class="create-toggle"> holds open/closed
 * state; the <label for="id"> anywhere in the page (e.g. the header icon
 * button) acts as the toggle. The .create-panel is hidden until checked.
 * The checkbox is visually hidden but not display:none so focus management
 * and the :checked selector still work. */
.create-toggle {
  position: absolute;
  opacity: 0;
  pointer-events: none;
  width: 0;
  height: 0;
}
.create-panel { display: none; }
.create-toggle:checked + .create-panel { display: block; }

"#;

pub const CHROME_THEME_TOGGLE_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Theme toggle (RFC 149 §3: rows in the account menu)                 */
/* ------------------------------------------------------------------ */

/* RFC 149 §3: the theme buttons are account-menu rows now (the footer
 * is gone), so they match `.user-menu__item`, which they also carry;
 * this rule comes later and would otherwise shrink and mute them. The
 * class stays because `/static/theme-init.js` selects it. */
.theme-toggle__btn {
  background: transparent;
  border: none;
  color: var(--fg-default);
  padding: var(--space-2) var(--space-4);
  font-size: var(--font-size-body);
  cursor: pointer;
  /* RFC 144 §4: 44×44 CSS px for standalone controls. */
  min-height: 2.75rem;
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
/* RFC 144 R5: a tint alone marks the pressed theme-toggle button; under
 * prefers-contrast: more it also gets a solid border. */
@media (prefers-contrast: more) {
  .theme-toggle__btn[aria-pressed="true"] {
    border: var(--border-width-default) solid var(--accent-default);
  }
}


/* ------------------------------------------------------------------ */
/* RFC 092: no-js / js root-class visibility rules                    */
/* ------------------------------------------------------------------ */

/* Hide the interactive toggle buttons in no-JS environments so users  */
/* do not see a non-functional control. RFC 149 §3: the theme row in   */
/* the account menu keeps its visible name and shows the note instead. */
.no-js .user-menu__theme-buttons { display: none; }

/* Hide the noscript note in JS environments.                          */
.js .theme-no-js-note { display: none; }

"#;

pub const CHROME_RESPONSIVE_CSS: &str = r#"
/* ── Skip link (RFC-MI-080, v0.57.0) ────────────────────────────────── */
/* Renders off-screen until focused, then jumps into view.               */
/* Must be the first focusable element in the document (WCAG 2.4.1).    */
.skip-link {
  position: absolute;
  top: -100%;
  left: var(--space-2);
  z-index: 9999;
  padding: var(--space-2) var(--space-3);
  background: var(--surface-elevated);
  color: var(--fg-default);
  border: var(--border-width-emphasis) solid var(--accent-default);
  border-radius: var(--radius-md);
  font-weight: var(--font-weight-medium);
  text-decoration: none;
  /* It wraps rather than run past the window at a large font size,
   * where it widened every page (375px at 200%: 484px wide). */
  max-width: calc(100vw - 2 * var(--space-2));
}
.skip-link:focus {
  top: var(--space-2);
}

/* ------------------------------------------------------------------ */
/* Responsive breakpoints (v0.48.2 — Bug 8; extended v0.57.0)          */
/* ------------------------------------------------------------------ */
/* Primary breakpoint: 768px (tablet/phone boundary).
 * Narrow breakpoints: 480px, 360px (modern phones, WCAG 1.4.10).
 * Desktop CSS above is the canonical layout; adjustments below
 * override only what needs to shrink/scroll/wrap on narrow viewports.
 * Screen-reader + keyboard-navigation experience is unchanged.         */

@media (max-width: 768px) {
  /* Tighter padding around the main content area so 32px*2 isn't
   * eating ~17% of a 375-wide viewport. */
  .app-main {
    padding: var(--space-3) var(--space-3);
  }
  /* The slim top bar (Chrome::Account/Bare) wraps rather than
   * squishes if the brand and account menu don't both fit. */
  .app-header { flex-wrap: wrap; row-gap: var(--space-2); }
  /* The header brand text shrinks slightly to leave more room. */
  .app-header__brand {
    font-size: var(--font-size-h3);
  }
  /* Cards still fit one per row but use less internal padding. */
  .card { padding: var(--space-3); }
  /* Setup wizard's lang picker centres the row even when items
   * fit in one line, but on narrow viewports the three options
   * stack vertically with comfortable touch targets. */
  .setup-lang-picker {
    flex-wrap: wrap;
  }
}

/* 480px — small phones (iPhone SE, Galaxy A series). */
@media (max-width: 480px) {
  /* Body text remains readable; reduce display-size headings. */
  .stat__value { font-size: var(--font-size-h2); }
  /* Auth card: full-bleed on small phones (no rounded corners
   * bleeding into screen edge). The card already uses width:100%;
   * just tighten the padding. */
  .auth-card, .consent-card {
    border-radius: 0;
    border-left: 0;
    border-right: 0;
    padding: var(--space-4) var(--space-3);
  }
  /* Route tabs: let items break freely so they stack instead of
   * truncating. Already have flex-wrap:wrap from the class. */
  .route-tabs__link {
    padding: var(--space-2) var(--space-2);
  }
  /* Danger zone: reduce padding to keep content in frame. */
  .danger-zone { padding: var(--space-3); }
}

/* 360px — minimum supported phone (WCAG 1.4.10 reflow target).
 * Text must reflow to a single column without horizontal scrolling
 * at 320–360px when zoomed to 400%. */
@media (max-width: 360px) {
  /* Push main content edge-to-edge. */
  .app-main { padding: var(--space-2) var(--space-2); }
  /* Form action rows: stack vertically so buttons don't overflow. */
  .form-actions {
    flex-direction: column;
    align-items: stretch;
  }
  .form-actions > * { width: 100%; text-align: center; }
  /* Stat / metric cards: stacked layout prevents overflow. */
  .grid-cards { grid-template-columns: 1fr; }
}
"#;
