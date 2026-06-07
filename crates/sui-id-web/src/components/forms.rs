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

"#;
