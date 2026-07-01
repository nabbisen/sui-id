//! Button variants.
//!
//! Owns: `.btn--{primary,secondary,danger,ghost,link}` and shared
//! disabled / focus states. The danger variant is visually
//! isolated — sites that need it should use the dedicated class
//! rather than reaching for inline colour.

pub const BUTTONS_CSS: &str = r#"
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
  /* Explicit AA-compliant disabled tokens, not opacity: opacity over a
     coloured fill degrades contrast unpredictably. fg-disabled on
     bg-disabled clears WCAG AA in both modes (light 5.65:1 / dark
     5.79:1). A disabled control must never be the sole carrier of a
     meaningful value (see tokens.rs / the UI/UX contrast contract). */
  color: var(--fg-disabled);
  background: var(--bg-disabled);
  cursor: not-allowed;
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

/* Wide call-to-action: used for the add-user / register-client button
 * below list tables where a prominent, clearly-labelled action is needed. */
.button--wide {
  width: 100%;
  padding: var(--space-3) var(--space-4);
  font-size: var(--font-size-h3);
  min-height: 56px;
  border-radius: var(--radius-md);
}

"#;
