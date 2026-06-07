//! Tab strips.
//!
//! Owns: the `.tabs` and `.tabs__link` styles introduced in RFC 023.
//! RFC-MI-022 (Phase 2) will add the route-based tab helper that
//! reuses these classes; no class-name change is needed there.

pub const TABS_CSS: &str = r#"
/* ── Tabs (RFC 023) ─────────────────────────────────────────────────── */
/* Horizontal tab bar for Settings and other multi-panel screens.         */
.tabs {
  display: flex;
  flex-direction: column;
}
.tabs__bar {
  display: flex;
  gap: 0;
  border-bottom: var(--border-width-default) solid var(--border-default);
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
}
.tab-btn {
  padding: var(--space-2) var(--space-3);
  background: transparent;
  border: none;
  border-bottom: var(--border-width-emphasis) solid transparent;
  color: var(--fg-muted);
  font: var(--font-weight-regular) var(--font-size-body) / 1 var(--font-sans);
  cursor: pointer;
  white-space: nowrap;
  transition: color var(--motion-fast) var(--motion-easing),
              border-color var(--motion-fast) var(--motion-easing);
  margin-bottom: calc(-1 * var(--border-width-default)); /* align with bar border */
}
.tab-btn:hover  { color: var(--fg-default); }
.tab-btn:focus-visible {
  outline: var(--border-width-emphasis) solid var(--state-focus);
  outline-offset: -2px;
}
.tab-btn[aria-selected="true"] {
  color: var(--accent-default);
  border-bottom-color: var(--accent-default);
  font-weight: var(--font-weight-medium);
}
.tabs__panel {
  padding-top: var(--space-4);
}

"#;
