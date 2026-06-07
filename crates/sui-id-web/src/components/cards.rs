//! Card and panel surfaces.
//!
//! Owns: `.card` base, the v0.46 `.card--{warn,info,success,callout}`
//! variants (RFC 062), and the `.empty-state` primitive (RFC 064).
//! Subsequent MI work that adds mockup-style metric cards or
//! callouts lands here.

pub const CARDS_CSS: &str = r#"
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

/* RFC 062 (v0.46.0) — card variants.
 * Compose with .card: <section class="card card--warn">. Each variant
 * gives the card an asymmetric 4px left accent and a subtle tinted
 * background, so a row of cards can read at a glance as "this one is
 * different." Colours come from RFC 061 semantic tokens, so light/dark
 * pairing is automatic. */
.card--warn {
  background: var(--warning-subtle);
  border-color: var(--warning-default);
  border-left-width: 4px;
}
.card--info {
  background: var(--info-subtle);
  border-color: var(--info-default);
  border-left-width: 4px;
}
.card--success {
  background: var(--success-subtle);
  border-color: var(--success-default);
  border-left-width: 4px;
}
.card--callout {
  /* Accent (lavender) callout — e.g. "next steps" cards on setup,
   * "what to do now" panels. Not a semantic warning; just visual
   * emphasis to mark the next operator action. */
  background: var(--accent-subtle);
  border-color: var(--accent-default);
  border-left-width: 4px;
}

/* RFC 064 (v0.46.0) — Empty-state primitive.
 * Replaces the per-page `<p class="muted">No X yet.</p>` pattern. The
 * dashed border + tinted background distinguishes "this section is a
 * placeholder" from "this section has muted-coloured content."
 * Compact variant is for use inside a table cell or other narrow
 * context where the full padding would look ridiculous. */
.empty-state {
  background: var(--surface-subtle);
  border: var(--border-width-default) dashed var(--border-muted);
  border-radius: var(--radius-md);
  padding: var(--space-5);
  text-align: center;
  color: var(--fg-muted);
}
.empty-state--compact {
  padding: var(--space-3);
  border-style: solid;
  text-align: left;
}
.empty-state__message {
  font-size: var(--font-size-body);
  margin: 0 0 var(--space-2) 0;
  color: var(--fg-default);
}
.empty-state__hint {
  font-size: var(--font-size-caption);
  margin: 0 0 var(--space-3) 0;
}
.empty-state__action {
  display: inline-block;
}

"#;
