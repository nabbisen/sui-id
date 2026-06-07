//! Confirmation and danger-zone surfaces.
//!
//! Owns: the `.reversibility-badge` and surrounding confirm-shell
//! visual cues (RFC 017 § 3, RFC 058, RFC 059). RFC-MI-051 (Phase 5)
//! extends this shard with the mockup's impact-summary primitive.

pub const CONFIRM_CSS: &str = r#"
/* ── Confirmation / step-up screens (RFC 017 § 3) ───────────────────── */
/* Reversibility badge on dangerous-operation confirm screens.            */
.reversibility-badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 2px var(--space-2);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
}
.reversibility-badge--recoverable {
  background: color-mix(in srgb, var(--success-default) 15%, transparent);
  color: var(--success-default);
}
.reversibility-badge--permanent {
  background: var(--danger-subtle);
  color: var(--danger-default);
}

"#;
