//! Setup-wizard and centred-card layouts.
//!
//! Owns: `.auth-card` centred layout (used by login + the setup
//! wizard), the setup-wizard language picker, the auth surface
//! helpers (RFC-MI-041), and the setup step-indicator primitives
//! (RFC-MI-040).
//!
//! ## Rust types (RFC-MI-040)
//!
//! [`StepState`] and [`SetupStep`] are public for use in render
//! data. The `setup_step_indicator` helper in `pages/setup.rs`
//! is a private consumer of the CSS classes defined here; it
//! does not depend on these types directly, but they are
//! exported for future use by the handler layer.
//!
//! ## CSS families
//!
//! `.auth-card` — centred narrow card, used for login/auth forms.
//! `.setup-lang-picker` — horizontal language toggle (v0.48.2).
//! `.auth-meta-link`, `.qr-display` — auth surface helpers (RFC-MI-041).
//! `.setup-steps`, `.setup-step__label--*` — step indicator (RFC-MI-040).

/// State of a single step in the setup wizard step indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    /// This step has been completed.
    Complete,
    /// The operator is on this step right now.
    Current,
    /// This step has not been reached yet.
    Upcoming,
}

impl StepState {
    /// CSS class for the step's text label.
    pub fn label_class(self) -> &'static str {
        match self {
            Self::Complete  => "setup-step__label--done",
            Self::Current   => "setup-step__label--current",
            Self::Upcoming  => "setup-step__label--upcoming",
        }
    }
}

/// One entry in the setup wizard step indicator.
#[derive(Debug, Clone)]
pub struct SetupStep {
    /// Short URL slug (e.g. `"admin"`, `"lang"`). Informational only
    /// — the indicator is not a navigation element.
    pub key: &'static str,
    /// Localised display label (resolved at render time).
    pub label: String,
    /// Visual state of this step.
    pub state: StepState,
}

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

pub const SETUP_LANG_PICKER_CSS: &str = r#"/* v0.48.2 — setup wizard language picker. */
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

/* ── Auth surface helpers (RFC-MI-041, v0.53.0) ─────────────────────── */
.auth-meta-link {
  margin-top: var(--space-3);
  text-align: center;
  font-size: var(--font-size-caption);
}
.qr-display {
  max-width: 240px;
  margin-bottom: var(--space-3);
}

/* ── Setup step indicator (RFC-MI-040, v0.53.1) ─────────────────────── */
/* .setup-steps         — container row for the step indicator.          */
/* .setup-step__label-* — step label colour for each StepState variant.  */
.setup-steps {
  display: flex;
  gap: var(--space-3);
  justify-content: center;
  margin-bottom: var(--space-4);
  flex-wrap: wrap;
  font-size: var(--font-size-caption);
}
.setup-step__label--current {
  color: var(--fg-default);
  font-weight: var(--font-weight-medium);
}
.setup-step__label--done {
  color: var(--fg-muted);
}
.setup-step__label--upcoming {
  color: var(--fg-subtle);
}

"#;

