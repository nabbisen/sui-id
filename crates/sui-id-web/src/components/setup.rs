//! Setup-wizard and centred-card layouts.
//!
//! Owns: `.auth-card` centred layout (used by login + the setup
//! wizard) and the setup-wizard language picker introduced in
//! v0.48.2. RFC-MI-040 will adopt the mockup's setup-step-indicator
//! variants here.
//!
//! Two sub-constants are kept separate to preserve cascade order:
//! the language picker sits AFTER the utility-class blocks in the
//! original `components.rs`.

pub const SETUP_AUTH_CARD_CSS: &str = r#"
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

"#;

pub const SETUP_LANG_PICKER_CSS: &str = r#"
/* v0.48.2 — setup wizard language picker.
 * Recessive horizontal toggle group shown at the top of the
 * welcome screen. The "active" link gets a subtle outline so
 * it reads as the current state without competing visually
 * with the primary "Begin" button below. */
.setup-lang-picker {
  display: flex;
  gap: var(--space-2);
  justify-content: center;
  margin-bottom: var(--space-4);
}
.setup-lang-picker__opt {
  font-size: var(--font-size-caption);
  padding: var(--space-1) var(--space-3);
  border: var(--border-width-default) solid var(--border-muted);
  border-radius: var(--radius-sm);
  color: var(--fg-muted);
  text-decoration: none;
  transition: background 0.12s, color 0.12s, border-color 0.12s;
}
.setup-lang-picker__opt:hover {
  background: var(--state-hover);
  color: var(--fg-default);
  text-decoration: none;
}
.setup-lang-picker__opt--active {
  color: var(--accent-default);
  border-color: var(--accent-default);
  background: var(--accent-subtle);
}

"#;
