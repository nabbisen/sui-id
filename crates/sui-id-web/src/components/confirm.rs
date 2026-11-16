//! Confirmation and danger-zone surfaces.
//!
//! Owns: the `.reversibility-badge` and surrounding confirm-shell
//! visual cues (RFC 017 § 3, RFC 058, RFC 059), and the danger-zone
//! + impact-summary primitives introduced by RFC-MI-051 (v0.54.0).
//!
//! ## CSS families
//!
//! `.reversibility-badge` — inline pill on confirm screens.
//! `.danger-zone` — isolated section for destructive operations on
//!   detail pages. The section must never contain safe actions.
//! `.impact-summary` — structured list of what the operation affects,
//!   shown on both the detail-page danger zone and the confirm screen.

pub const CONFIRM_CSS: &str = r#"
/* ── Confirmation / step-up screens (RFC 017 § 3) ───────────────────── */
.reversibility-badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 2px var(--space-2);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
}
/* RFC 144 §1a, refresh rule 3: danger/success as text on their own
 * tint fail the contrast gate (color-mix() also cannot be resolved by
 * it at all). Both badges take the banner convention instead: the
 * tinted fill, text in --fg-default, and the semantic colour on the
 * border. */
.reversibility-badge--recoverable {
  background: var(--success-subtle);
  color: var(--fg-default);
  border: var(--border-width-default) solid var(--success-default);
}
.reversibility-badge--permanent {
  background: var(--danger-subtle);
  color: var(--fg-default);
  border: var(--border-width-default) solid var(--danger-default);
}

/* ── Danger zone (RFC-MI-051, v0.54.0) ──────────────────────────────── */
/* .danger-zone physically and semantically isolates destructive          */
/* operations on detail pages. No safe/read operations may appear inside */
/* a .danger-zone. ABDD: uses a danger-subtle fill + left border so the  */
/* meaning is conveyed by structure and color; the title also uses text.  */
.danger-zone {
  border: var(--border-width-default) solid var(--danger-default);
  border-left-width: 4px;
  border-radius: var(--radius-md);
  background: var(--danger-subtle);
  padding: var(--space-4);
  margin-top: var(--space-5);
}
.danger-zone__title {
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-medium);
  /* RFC 144 §1a: danger as text on the danger tint was 4.28:1. The
     zone itself keeps its danger border; the title's own meaning now
     comes from that border and the section heading, not the hue of
     its text. */
  color: var(--fg-default);
  margin: 0 0 var(--space-2);
}
.danger-zone__body {
  font-size: var(--font-size-body);
  color: var(--fg-default);
  margin: 0 0 var(--space-3);
}

/* ── Impact summary (RFC-MI-051, v0.54.0) ───────────────────────────── */
/* Structured list of what a destructive operation will affect.           */
/* Rendered on the confirm screen and optionally in the danger zone.      */
/* ABDD: each item has a text label and a value — no icon-only state.    */
.impact-summary {
  list-style: none;
  margin: 0 0 var(--space-3);
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}
.impact-summary__item {
  display: flex;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}
.impact-summary__label {
  color: var(--fg-muted);
  flex: 0 0 auto;
}
.impact-summary__value {
  color: var(--fg-default);
  word-break: break-word;
}

"#;
