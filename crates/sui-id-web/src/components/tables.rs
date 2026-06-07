//! Tables.
//!
//! Owns: `.app-table` base, the `.cell-wrap` opt-out class (v0.48.2,
//! Bug 8), and copy-cell affordances. Wide-table responsive overrides
//! sit in `chrome.rs::CHROME_RESPONSIVE_CSS` to keep the
//! `@media (max-width: 768px)` block contiguous.

pub const TABLES_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Tables                                                              */
/* ------------------------------------------------------------------ */

.table-wrap {
  background: var(--surface-elevated);
  border: var(--border-width-default) solid var(--border-muted);
  border-radius: var(--radius-md);
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--font-size-body);
}
thead th {
  text-align: left;
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
  color: var(--fg-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  padding: var(--space-2) var(--space-3);
  background: var(--surface-subtle);
  border-bottom: var(--border-width-default) solid var(--border-muted);
  /* v0.48.2 (Bug 8): header cells never wrap. */
  white-space: nowrap;
}
tbody td {
  padding: var(--space-3);
  border-bottom: var(--border-width-default) solid var(--border-muted);
  vertical-align: middle;
  /* v0.48.2 (Bug 8): body cells default to no-wrap. On narrow
   * viewports a wider table now scrolls horizontally inside its
   * .table-wrap rather than collapsing cells vertically. Columns
   * that legitimately carry free-form text (notes, descriptions,
   * names) opt out via the .cell-wrap class. */
  white-space: nowrap;
}
tbody td.cell-wrap,
thead th.cell-wrap {
  white-space: normal;
  word-break: break-word;
}
tbody tr:last-child td { border-bottom: 0; }
tbody tr:hover { background: var(--state-hover); }

"#;
