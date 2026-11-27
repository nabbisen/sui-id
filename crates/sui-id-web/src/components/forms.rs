//! Form, field, and validation styles.
//!
//! Owns: label/input geometry, required markers, hint text, and the
//! inline error pattern. RFC-MI-050 will absorb the mockup's
//! `field-error` and `form-grid` primitives into this shard.

pub const FORMS_CSS: &str = r#"
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
  border: var(--border-width-default) solid var(--border-control);
  border-radius: var(--radius-sm);
  background: var(--surface-elevated);
  color: var(--fg-default);
  font-family: inherit;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  transition: border-color 0.12s, box-shadow 0.12s;
  /* RFC 144 §4: 44×44 CSS px for standalone controls. A multi-row
   * textarea already exceeds this from its own rows; this is a floor,
   * not a fixed height. */
  min-height: 2.75rem;
}
/* A fieldset's default min-inline-size is min-content, so its choices
 * could never be narrower than their own content: at 375px with a 200%
 * font, the breach-check choices widened the page (Q3b's review). */
fieldset {
  min-inline-size: 0;
}
input:hover, textarea:hover, select:hover {
  border-color: var(--fg-muted);
}
input:focus, textarea:focus, select:focus {
  outline: none;
  border-color: var(--accent-default);
  box-shadow: 0 0 0 3px var(--accent-subtle);
}
/* Disabled fields use the explicit disabled pair, as disabled buttons do
   (RFC 144 §6, rule 1): never an opacity, which put this text at 2.34:1. */
input:disabled, textarea:disabled, select:disabled {
  color: var(--fg-disabled);
  background: var(--bg-disabled);
  cursor: not-allowed;
}

input[type="checkbox"] {
  width: auto;
  accent-color: var(--accent-default);
}
/* RFC 144 N2e paragraph 1: drawn larger than the browser default
 * (13x13 CSS px) so a chosen option's own dot reads at a glance, not
 * just the label colour change below. The 44x44 target size applies to
 * the label, the real click target -- see CHOSEN_OPTION_CSS and its
 * EXEMPT entry in tokens/tests.rs's standalone_controls_reach_44px. */
input[type="radio"] {
  width: auto;
  height: 1.25rem;
  aspect-ratio: 1;
  accent-color: var(--accent-default);
}

/* ── Field validation states (RFC-MI-011, v0.50.1) ───────────────────── */
/* .field__error is the accessible inline error message that appears      */
/* below an invalid input. It must be linked via aria-describedby on      */
/* the input element.                                                      */
/*                                                                         */
/* .field--invalid marks the field container; it triggers the red border  */
/* on the contained input without requiring inline styles.                */
.field__error {
  font-size: var(--font-size-caption);
  color: var(--danger-default);
  margin-top: var(--space-1);
  /* Match .field__hint so error and hint are interchangeable in layout. */
  line-height: var(--line-height-caption);
}
.field--invalid input,
.field--invalid textarea,
.field--invalid select {
  border-color: var(--danger-default);
}
.field--invalid input:focus-visible,
.field--invalid textarea:focus-visible,
.field--invalid select:focus-visible {
  outline-color: var(--danger-default);
}

/* ── Form layout primitives (RFC-MI-050, v0.54.0) ───────────────────── */
/* .form-actions  — flex row for primary / secondary / cancel buttons.   */
/*                  align-self:flex-start prevents stretching inside a   */
/*                  flex column or flex-end-aligned header row.          */
/* .form-section  — labelled group within a longer form, with a top      */
/*                  separator and section heading.                       */
/* .form-grid     — two-column field layout for spacious screens; degrades*/
/*                  to single column at narrow viewport (via chrome.rs). */

.form-actions {
  display: flex;
  gap: var(--space-2);
  align-items: center;
  flex-wrap: wrap;
  align-self: flex-start;
}
.form-section {
  border-top: var(--border-width-default) solid var(--border-muted);
  padding-top: var(--space-4);
  margin-top: var(--space-4);
}
.form-section__title {
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-medium);
  margin: 0 0 var(--space-3);
}
.form-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-3);
}

/* ── Remaining form primitives (RFC-MI-050, v0.54.0) ───────────────── */
/* .field--required — marks a field as required. A visible asterisk      */
/*   appears after the label text via ::after. The asterisk is           */
/*   aria-hidden by CSS; use a visually-hidden "required" note or a      */
/*   form-level note to convey the requirement to AT users.              */
/* .review-summary  — a pre-submit summary panel listing the values the  */
/*   operator is about to save. Used on settings confirmations.          */
.field--required .field__label::after {
  content: ' *';
  color: var(--danger-default);
  font-weight: var(--font-weight-medium);
  aria-hidden: true; /* ::after content is not exposed to AT */
}
.review-summary {
  background: var(--surface-subtle);
  border: var(--border-width-default) solid var(--border-muted);
  border-radius: var(--radius-md);
  padding: var(--space-3);
}

"#;

/// A single server-validated field error (RFC-MI-050, v0.54.0).
///
/// Field errors are render-only state. They must never be persisted
/// to the database and must never be inferred from data that could
/// reveal whether an account exists (anti-enumeration constraint).
///
/// Usage: collect errors in the handler after form parsing, pass as
/// `Vec<FieldError>` to the render function, and render each error
/// adjacent to its input with `aria-describedby` + `aria-invalid`.
#[derive(Debug, Clone)]
pub struct FieldError {
    /// The `id` attribute of the field this error belongs to.
    pub field: &'static str,
    /// Localised error message (resolved at the handler layer).
    pub message: String,
}

// ── RFC 092: error_summary component ─────────────────────────────────────────

pub(crate) const ERROR_SUMMARY_CSS: &str = r#"
/* ---- error-summary (RFC 092) ---- */
.error-summary {
  background: var(--danger-subtle);
  border: var(--border-width-default) solid var(--danger-default);
  border-radius: var(--radius-sm);
  padding: var(--space-3) var(--space-4);
  margin-bottom: var(--space-4);
  color: var(--fg-default);
}
.error-summary__heading {
  font-weight: var(--font-weight-medium);
  margin: 0 0 var(--space-2);
  /* Not a stated §3 fix: found by the widened gate itself, run before
     this return. --fg-on-danger is the label colour for text ON a
     saturated --danger-default/-emphasis fill (white in light, dark ink
     in dark) — the gate checks it there, and only there. This heading
     sits on .error-summary's own --danger-subtle background, which
     inherits --fg-default (set on .error-summary, just above); the
     override here was simply wrong, not a case the five-rule set needed
     to make a decision about. Removing it lets the correct colour
     inherit. */
  color: var(--fg-default);
}
.error-summary__list {
  margin: 0;
  padding-left: var(--space-4);
}
.error-summary__list li {
  margin-bottom: var(--space-1);
}
"#;

/// Accessible error summary block for forms with multiple field errors
/// (RFC 092 / v2.3 §5).
///
/// Rendered only when `errors.len() > 1`. Uses `role="alert"` and
/// `aria-live="assertive"` so screen readers announce the error list
/// immediately when the page loads after a failed submission.
///
/// Individual field errors continue to render inline via the existing
/// per-field error mechanisms; this summary provides an at-a-glance list
/// at the top of the form.
pub fn error_summary(
    t: &'static sui_id_i18n::Strings,
    errors: &[FieldError],
) -> Option<impl leptos::prelude::IntoView> {
    use leptos::prelude::*;
    if errors.len() <= 1 {
        return None;
    }
    let heading = t.error_summary_heading;
    let items: Vec<_> = errors
        .iter()
        .map(|e| {
            let msg = e.message.clone();
            view! { <li>{msg}</li> }
        })
        .collect();
    Some(view! {
        <div
            class="error-summary"
            role="alert"
            aria-live="assertive"
        >
            <p class="error-summary__heading">{heading}</p>
            <ul class="error-summary__list">
                {items}
            </ul>
        </div>
    })
}

// ── RFC 144 N2e §1: a chosen radio option is visible, not only
// functional ──────────────────────────────────────────────────────────
//
// Every radio below already toggles on a real pointer click at the
// label text (native <label>/<input> behaviour) -- what was missing was
// anything that showed the result. The three label classes are the only
// places sui-id puts a radio inside a clickable label:
//   - setup -> language   (pages/setup/lang.rs)
//   - setup -> HIBP policy (pages/setup/hibp.rs)
//   - My account -> language (pages/me_security/language.rs)
pub(crate) const CHOSEN_OPTION_CSS: &str = r#"
label.row-gap2-center-clickable:has(input[type="radio"]:checked),
label.row.row-gap2-center:has(input[type="radio"]:checked) {
  border: var(--border-width-default) solid var(--border-accent);
  border-radius: var(--radius-sm);
  background: var(--accent-subtle);
}
/* .card already draws a 1px border and its own radius; only the colour
 * and fill change on selection, so the HIBP cards keep card radius-md,
 * not the radius-sm the two border-less label types get above. */
label.card.clickable-block:has(input[type="radio"]:checked) {
  border-color: var(--border-accent);
  background: var(--accent-subtle);
}
/* RFC 144 rule 5: a tint alone marks the choice; under prefers-contrast:
 * more it also gets a thicker, explicitly solid border -- the same
 * pattern as the current nav item and .badge--accent. */
@media (prefers-contrast: more) {
  label.row-gap2-center-clickable:has(input[type="radio"]:checked),
  label.row.row-gap2-center:has(input[type="radio"]:checked),
  label.card.clickable-block:has(input[type="radio"]:checked) {
    border-style: solid;
    border-width: var(--border-width-emphasis);
  }
}
/* The label is the target, not the radio inside it -- a floor, not a
 * fixed height, same convention as every other 44x44 rule. */
label.row-gap2-center-clickable,
label.row.row-gap2-center,
label.card.clickable-block {
  min-height: 2.75rem;
}
"#;
