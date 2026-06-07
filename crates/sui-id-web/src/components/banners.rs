//! Flash banners, status banners, and the dev-mode banner.
//!
//! Owns three families that all sit in the "transient or contextual
//! page message" role:
//! * inline `.flash{,.warn,.danger,.success}` (the legacy in-form
//!   banner family),
//! * `.banner{,--warning,--danger,--success}` (RFC 057's standalone
//!   status messages),
//! * `.devmode-banner` (RFC 032, RFC 017 § 9 — only emitted in
//!   `--dev` mode).
//!
//! RFC-MI-040 (Phase 4) will adopt the mockup's "callout" pattern
//! for setup-wizard gate states; that work sits here too unless it
//! migrates to `cards.rs::CARDS_CSS`.
//!
//! Three sub-constants are kept separate to preserve the original
//! cascade order: the page-header rules in `chrome.rs` sit BETWEEN
//! the RFC 057 banners and the dev-mode banner in the original
//! `components.rs`. `COMPONENTS_CSS` interleaves accordingly.

pub const BANNERS_FLASH_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Flash banners                                                       */
/* ------------------------------------------------------------------ */

.flash {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  border: var(--border-width-default) solid var(--border-default);
  margin-bottom: var(--space-3);
  display: flex;
  gap: var(--space-3);
  align-items: flex-start;
}
.flash.info {
  background: var(--accent-subtle);
  border-color: transparent;
  color: var(--fg-default);
}
.flash.warn {
  background: var(--warning-subtle);
  border-color: var(--warning-default);
  color: var(--fg-default);
}
.flash.error {
  background: var(--danger-subtle);
  border-color: var(--danger-default);
  color: var(--fg-default);
}
/* Dark-mode override no longer needed for .flash.warn — RFC 061 made
 * --warning-subtle per-mode, so the same rule resolves correctly. */

"#;

pub const BANNERS_STATUS_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Banners (RFC 057, v0.44.0)                                          */
/* ------------------------------------------------------------------ */
/* Standalone status messages, distinct from inline flash banners.    */
/* `.banner` is the base; `--warning`, `--danger`, `--success` are    */
/* the colour variants. Used for in-page confirmations and warnings.  */

.banner {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  border: var(--border-width-default) solid var(--border-default);
  display: flex;
  gap: var(--space-3);
  align-items: flex-start;
}
.banner--success {
  background: var(--success-subtle);
  border-color: var(--success-default);
  color: var(--fg-default);
}
.banner--warning {
  background: var(--warning-subtle);
  border-color: var(--warning-default);
  color: var(--fg-default);
}
.banner--danger {
  background: var(--danger-subtle);
  border-color: var(--danger-default);
  color: var(--fg-default);
}
/* RFC 061: dark-mode override no longer needed — token is per-mode. */

"#;

pub const BANNERS_DEVMODE_CSS: &str = r#"
/* ── Dev-mode banner (RFC 017 § 9, RFC 023) ─────────────────────────── */
/* Displayed on every page when sui-id starts with --dev.                 */
.dev-banner {
  position: sticky;
  top: 0;
  z-index: var(--z-raised);
  background: #7A5C00;        /* deep amber, distinct from any semantic colour */
  color: #FFF8E1;
  padding: var(--space-1) var(--space-3);
  font-size: var(--font-size-caption);
  text-align: center;
  letter-spacing: 0.04em;
}
.dev-banner strong { letter-spacing: 0.08em; }
/* High-contrast bind warning within the banner */
.dev-banner__bind-warn {
  color: #FFCDD2;
  font-weight: var(--font-weight-medium);
}

"#;
