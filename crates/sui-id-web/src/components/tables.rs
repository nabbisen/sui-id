//! Tables.
//!
//! Owns: the `table` base styles, the `.table-wrap` scrollable
//! container, the `.cell-wrap` opt-in wrapping class (v0.48.2 Bug 8),
//! and the extended cell-discipline classes added by RFC-MI-031
//! (v0.52.0): `.cell-nowrap`, `.cell-id`, `.cell-actions`.
//!
//! Wide-table responsive overrides sit in
//! `chrome.rs::CHROME_RESPONSIVE_CSS` to keep the
//! `@media (max-width: 768px)` block contiguous.
//!
//! ## Column discipline (RFC-MI-031)
//!
//! | Class          | Use                                       |
//! |----------------|-------------------------------------------|
//! | (default)      | `white-space: nowrap` for stable layout   |
//! | `.cell-wrap`   | Free-form text that may legitimately wrap |
//! | `.cell-nowrap` | Explicit nowrap for documentation/intent  |
//! | `.cell-id`     | Monospace; suitable for UUIDs / hashes    |
//! | `.cell-actions`| Right-align; never wraps                 |

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
  /* RFC 144, "What else moves": vertical padding --space-2 (was
   * --space-3) — with the §1/§3 type and spacing changes, a row now
   * stands at about 44px, in reach of §4's target size, instead of
   * about 57px. Horizontal padding is unchanged. */
  padding: var(--space-2) var(--space-3);
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
/* Explicit color: unchanged from body's own fg-default, but the
   contrast gate checks each rule's own declarations. */
tbody tr:hover { background: var(--state-hover); color: var(--fg-default); }

/* ── Extended cell discipline (RFC-MI-031, v0.52.0) ─────────────────── */
/* .cell-nowrap  — explicit no-wrap (the default; use for documentation). */
/* .cell-id      — for UUID, hash, or any machine-readable opaque ID.    */
/*                 Monospace; truncates gracefully in narrow viewports.  */
/* .cell-actions — right-align action buttons; never wraps.              */
tbody td.cell-nowrap,
thead th.cell-nowrap { white-space: nowrap; }

tbody td.cell-id {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  /* Prevent ID from widening table beyond scroll container. */
  max-width: 16rem;
  overflow: hidden;
  text-overflow: ellipsis;
}

tbody td.cell-actions,
thead th.cell-actions {
  text-align: right;
  white-space: nowrap;
}

/* ── RFC 149 Q5 §3: the audit log's target cell truncates its own text  */
/* inline, beside its copy button, rather than owning a cell-id column  */
/* of its own. */
.cell-id-inline {
  display: inline-block;
  vertical-align: middle;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  max-width: 16rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ── RFC 149 Q5 §3: a bounded, independently-scrolling table whose own */
/* header stays in view (`.table-wrap--scroll`'s own vertical scroll), */
/* for a list long enough that "scroll to see the header again" would  */
/* otherwise apply. Opt-in: most tables are short enough (RFC 149 Q5    */
/* §2's own 50-per-page cap, for one) that this would add a scrollbar   */
/* with nothing to show for it. */
.table-wrap--scroll {
  max-height: 70vh;
  overflow-y: auto;
}
.table-head--sticky th {
  position: sticky;
  top: 0;
  z-index: 1;
}

"#;
