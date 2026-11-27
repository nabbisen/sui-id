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
  /* RFC 144: tokens, not literals — the contrast gate cannot resolve a
     hard-coded colour, and these three are the same in both modes (the
     banner looks the same regardless of theme). */
  background: var(--devmode-bg);
  color: var(--devmode-fg);
  padding: var(--space-1) var(--space-3);
  font-size: var(--font-size-caption);
  text-align: center;
  letter-spacing: 0.04em;
}
.dev-banner strong { letter-spacing: 0.08em; }
/* High-contrast bind warning within the banner. background: repeats
   the ancestor .dev-banner's own fill (no visible change) so the
   contrast gate resolves the real pair (devmode-warn on devmode-bg)
   instead of checking devmode-warn against the four neutral page
   surfaces it is never actually painted on. */
.dev-banner__bind-warn {
  background: var(--devmode-bg);
  color: var(--devmode-warn);
  font-weight: var(--font-weight-medium);
}

/* ── Dashboard action items (RFC 073, v0.58.0) ──────────────────────── */
/* .action-items-list — flat list of conditional warnings. Each row is */
/*   a single line: severity dot + message + link.                       */
.action-items-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.action-items-list > li {
  padding: var(--space-2) 0;
  border-bottom: var(--border-width-default) solid var(--border-muted);
  font-size: var(--font-size-body);
}
.action-items-list > li:last-child {
  border-bottom: 0;
}

/* ── The first-run path (RFC 149 §2 / Q3 §2) ────────────────────────── */
/* Replaces the old glyph-only checklist: every step names its status */
/* in words (a badge), never by colour or a tick alone.                */
.getting-started-path {
  list-style: none;
  padding: 0;
  margin: 0;
  counter-reset: step;
}
/* RFC 149 §2: an ordered path, so each step shows its number (the <ol>
 * already gives the order to assistive technology; the number is for
 * the eye, since display:flex on the items removes the list marker). */
.getting-started-path > li::before {
  counter-increment: step;
  content: counter(step) ".";
  min-width: 1.5em;
  color: var(--fg-muted);
  font-variant-numeric: tabular-nums;
}
.getting-started-path > li {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-1) var(--space-2);
  padding: var(--space-2) 0;
  font-size: var(--font-size-body);
}
.getting-started-path > li.done {
  color: var(--fg-muted);
}

"#;
