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

"#;

pub const CHROME_LAYOUT_CSS: &str = r#"
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
  /* v0.48.2 (Bug 8): keep each nav label on one line. Without
   * this, narrow viewports cause text to wrap inside an item,
   * turning the nav into a vertical stack rather than a row. */
  white-space: nowrap;
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

/* RFC 074: user-menu dropdown — replaces the flat "Security" nav link.
 * Uses <details>/<summary> so it works with JavaScript disabled.      */
.user-menu { position: relative; margin-left: auto; }
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
  top: calc(100% + var(--space-1));
  min-width: 11rem;
  background: var(--surface-elevated);
  border: var(--border-width-default) solid var(--border-default);
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-md);
  z-index: 100;
  display: flex;
  flex-direction: column;
  padding: var(--space-1) 0;
}
.user-menu__item {
  display: block;
  padding: var(--space-2) var(--space-4);
  color: var(--fg-default);
  font: inherit;
  font-size: var(--font-size-body);
  text-decoration: none;
  background: none;
  border: none;
  cursor: pointer;
  text-align: left;
  width: 100%;
}
.user-menu__item:hover,
.user-menu__item:focus-visible {
  background: var(--state-hover);
  outline: none;
}
.user-menu__form { display: contents; }



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
/* Tagline — restrained per v0.48.2 user feedback: the previous
 * default body-size weight competed with the more functional
 * footer content (theme toggle, a11y badges). Smaller and muted
 * keeps it a recessive whisper of intent rather than a banner. */
.app-footer__tagline {
  flex: 1;
  font-size: var(--font-size-caption);
  color: var(--fg-muted);
  opacity: 0.75;
}
.app-footer__a11y {
  display: flex;
  gap: var(--space-3);
  flex-wrap: wrap;
}

/* v0.48.2: passive informational badges, not interactive.
 * Reset <ul>/<li> defaults, render as small muted chips that
 * read as "facts about the app" rather than "things you can
 * click". No hover state, no border, no underline. */
.app-footer__a11y {
  list-style: none;
  padding: 0;
  margin: 0;
}
.app-footer__a11y-item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-caption);
  color: var(--fg-muted);
  cursor: default;
}
.app-footer__a11y-icon {
  font-size: 1em;
  line-height: 1;
  opacity: 0.85;
}
.app-footer__version {
  color: var(--fg-subtle);
  font-family: var(--font-mono);
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
/* Icon-only action button in page headers (users, clients, signing-keys).
 * Use class="button button--icon" with aria-label on the element. */
.button--icon {
  width: var(--space-5);
  height: var(--space-5);
  padding: 0;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 1.25rem;
  line-height: 1;
  flex-shrink: 0;
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
  white-space: nowrap;
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
  /* Nav: horizontal scroll instead of squish. Items keep their
   * desktop padding/typography; the row just slides under your
   * finger when there's not enough space for everything. */
  .app-nav {
    overflow-x: auto;
    flex-wrap: nowrap;
  }
  /* Sign-out button stops being pushed to the far right (which
   * is unreachable in an overflow-scroll context) and joins the
   * row in order. */
  .app-nav__signout { margin-left: var(--space-1); }
  /* The header brand text shrinks slightly so the nav has more
   * room before the scroll kicks in. */
  .app-header__brand {
    font-size: var(--font-size-h3);
  }
  /* Footer collapses to a single column. Tagline + a11y badges +
   * theme toggle + version each get their own line, in that
   * order. Cleaner than trying to flow them at narrow widths. */
  .app-footer {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
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
