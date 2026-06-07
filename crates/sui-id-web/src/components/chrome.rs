//! Chrome / shell CSS — the app's outer scaffolding.
//!
//! Owns: base reset (body, focus ring, links), typography (h1–h6, p,
//! .muted, code), the `Shell`-level layout (header, nav, main, footer
//! + a11y badges, sign-out button, tagline), the cross-screen
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
/* ------------------------------------------------------------------ */
/* Responsive breakpoints (v0.48.2 — Bug 8)                            */
/* ------------------------------------------------------------------ */
/* Single breakpoint at the tablet boundary (768px). The desktop CSS
 * above is the canonical layout; adjustments below override only what
 * needs to shrink/scroll/wrap on narrower viewports. The screen-reader
 * + keyboard-navigation experience is unchanged across breakpoints.   */

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
"#;
