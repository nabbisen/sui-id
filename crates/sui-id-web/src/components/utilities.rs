//! Bounded utility classes (RFC 067).
//!
//! Owns the project's deliberately-small utility-class set used to
//! avoid inline `style="…"`. New utilities require an RFC.
//!
//! Six sub-constants preserve the original cascade order: utility
//! blocks are interleaved with several other shards in the original
//! `components.rs` (dividers and visually-hidden sit after
//! page-header / theme-toggle / auth-card; copy-button sits before
//! tabs; motion sits between dev-mode banner and confirm).

pub const UTILITIES_RFC067_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* RFC 067 utility classes (v0.48.0)                                  */
/* ------------------------------------------------------------------ */
/* Small, token-derived utility classes for the spacing + layout       */
/* patterns surfaced in the Phase F inline-style survey. The set is    */
/* deliberately tight; new utilities require RFC justification.        */

/* Margin: top / bottom */
.mt-1 { margin-top: var(--space-1); }
.mt-2 { margin-top: var(--space-2); }
.mt-3 { margin-top: var(--space-3); }
.mt-4 { margin-top: var(--space-4); }
.mt-5 { margin-top: var(--space-5); }
.mb-0 { margin-bottom: 0; }
.mb-1 { margin-bottom: var(--space-1); }
.mb-2 { margin-bottom: var(--space-2); }
.mb-3 { margin-bottom: var(--space-3); }
.mb-4 { margin-bottom: var(--space-4); }

/* Margin: left (rare; for inline icon spacing only) */
.ml-1 { margin-left: var(--space-1); }
.ml-2 { margin-left: var(--space-2); }

/* Margin combo used by several explainer paragraphs */
.mt-2-mb-0 { margin-top: var(--space-2); margin-bottom: 0; }

/* Gap (flex/grid spacing) */
.gap-1 { gap: var(--space-1); }
.gap-2 { gap: var(--space-2); }
.gap-3 { gap: var(--space-3); }

/* Layout */
.center { text-align: center; }
.items-center { align-items: center; }
.items-end { align-items: flex-end; }
.justify-end { justify-content: flex-end; }
.justify-between { justify-content: space-between; }
.inline-el { display: inline; }
.inline-block { display: inline-block; }
.flex-1 { flex: 1; }

/* Common composite patterns */
.row-gap2-center { gap: var(--space-2); align-items: center; }
.row-gap3-center { gap: var(--space-3); align-items: center; }

/* Constrained widths used by auth cards and form pages */
.max-w-card { max-width: 36rem; }
.max-w-narrow { max-width: 22rem; }
.min-w-16rem { min-width: 16rem; }

/* Typography */
.text-caption { font-size: var(--font-size-caption); }
.text-small { font-size: 0.85em; }
.fw-medium { font-weight: var(--font-weight-medium); }
.fw-500 { font-weight: 500; }

/* The "label cell" pattern used inside every settings <table>:
 * a 14rem-wide muted-foreground left-aligned <th>. Rolled into a
 * single class so kv_row() does not need an inline style. */
.kv-label-cell {
  width: 14rem;
  font-weight: var(--font-weight-medium);
  color: var(--fg-muted);
  text-align: left;
}

.stack { display: flex; flex-direction: column; gap: var(--space-3); }
.stack-tight { display: flex; flex-direction: column; gap: var(--space-2); }
.row { display: flex; gap: var(--space-3); align-items: center; flex-wrap: wrap; }

.grid-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
  gap: var(--space-3);
}

/* Stat callout (used on dashboard) */
.stat {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}
.stat__value {
  font-size: var(--font-size-display);
  line-height: 1.1;
  font-weight: var(--font-weight-bold);
  color: var(--fg-default);
  font-variant-numeric: tabular-nums;
}
.stat__label {
  color: var(--fg-muted);
  font-size: var(--font-size-caption);
}

"#;

pub const UTILITIES_DIVIDERS_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Dividers and section spacing                                        */
/* ------------------------------------------------------------------ */

.divider {
  border: 0;
  border-top: var(--border-width-default) solid var(--border-muted);
  margin: var(--space-4) 0;
}

section + section { margin-top: var(--space-5); }

"#;

pub const UTILITIES_VISUALLY_HIDDEN_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Visually-hidden (for screen readers)                                */
/* ------------------------------------------------------------------ */

.sr-only {
  position: absolute;
  width: 1px; height: 1px;
  padding: 0; margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

"#;

pub const UTILITIES_COPY_BUTTON_CSS: &str = r#"
/* ── Copy-to-clipboard button (RFC 028) ─────────────────────────────── */
.copy-btn {
    display: none; /* shown via JS when clipboard-available class is set */
    align-items: center;
    gap: 0.25em;
    padding: 0.1em 0.5em;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm, 4px);
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: 0.8em;
    cursor: pointer;
    vertical-align: middle;
    margin-left: 0.4em;
    transition: color 0.15s, border-color 0.15s;
    white-space: nowrap;
}
.clipboard-available .copy-btn { display: inline-flex; }
.copy-btn:hover,
.copy-btn:focus-visible {
    color: var(--fg-default);
    border-color: var(--fg-muted);
    outline: 2px solid var(--state-focus, currentColor);
    outline-offset: 2px;
}

"#;

pub const UTILITIES_MOTION_CSS: &str = r#"
/* ── Transitions on interactive components (RFC 023 motion contract) ── */
/* Apply motion tokens so prefers-reduced-motion is obeyed automatically.*/
button,
.btn,
a,
input,
select,
textarea {
  transition-duration: var(--motion-fast);
  transition-timing-function: var(--motion-easing);
  transition-property: color, background-color, border-color, box-shadow, opacity;
}

"#;

pub const UTILITIES_ADDITIONAL_CSS: &str = r#"
/* RFC 067 — additional utility classes for less-frequent patterns
 * that still appear in multiple sites. Keeps the inline-style count
 * under the CI bound. */
.clickable-block { cursor: pointer; display: block; }
.radio-hint {
  margin: var(--space-1) 0 0 calc(1em + var(--space-2));
  font-size: var(--font-size-caption);
}
.center-pad-4 { text-align: center; padding: var(--space-4) 0; }
.center-pad-6 { text-align: center; padding: var(--space-6) 0; }
.center-pad-6-muted {
  text-align: center;
  padding: var(--space-6) 0;
  color: var(--fg-muted);
}
.ul-indent { margin: 0; padding-left: var(--space-4); }
.row-gap2-center-clickable {
  gap: var(--space-2);
  align-items: center;
  cursor: pointer;
}
.button-reset { border: none; padding: 0; margin: 0; }
.color-accent { color: var(--accent-default); }
.color-danger { color: var(--danger-default); }
.flex-0-auto { flex: 0 0 auto; }
.gap1-center { gap: var(--space-1); align-items: center; }

/* ── Definition-list grid (RFC-MI-011, v0.50.1) ─────────────────────── */
/* Key-value pair layout used on admin detail screens (/admin/users/{id}, */
/* /admin/clients/{id}). Replaces ad-hoc <table> usage for non-tabular   */
/* key-value data; respects the semantic meaning of <dl>/<dt>/<dd>.       */
/*                                                                         */
/* Usage: <dl class="dl-grid">                                            */
/*          <dt>Label</dt><dd>Value</dd>                                  */
/*        </dl>                                                            */
.dl-grid {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: var(--space-1) var(--space-4);
  margin: 0;
}
.dl-grid dt {
  font-size: var(--font-size-caption);
  color: var(--fg-muted);
  padding-top: 2px; /* optical alignment with dd value */
}
.dl-grid dd {
  margin: 0;
  color: var(--fg-default);
  word-break: break-word;
}

/* ── Phase 3 layout helpers (RFC-MI-030 + RFC-MI-031, v0.52.0) ───────── */
/* .filter-bar       — filter row on list/audit pages (flex, wraps).     */
/* .sparkline-header, .sparkline-title, .sparkline-legend,               */
/*   .sparkline-container — dashboard sparkline section layout pieces.   */
.filter-bar {
  display: flex;
  gap: var(--space-3);
  margin-bottom: var(--space-3);
  align-items: flex-end;
  flex-wrap: wrap;
}
.sparkline-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  margin-bottom: var(--space-3);
}
.sparkline-title {
  margin: 0;
  font-weight: var(--font-weight-medium);
  opacity: 0.85;
}
.sparkline-legend {
  display: flex;
  gap: var(--space-5);
  margin-bottom: var(--space-3);
}
.sparkline-container {
  width: 100%;
  height: 80px;
  display: block;
}

"#;

// ── RFC 092: EmptyState component ─────────────────────────────────────────────

pub(crate) const EMPTY_STATE_CSS: &str = r#"
/* ---- EmptyState (RFC 092) ---- */
.empty-state {
  padding: var(--space-6) var(--space-4);
  text-align: center;
  color: var(--fg-muted);
}
.empty-state__message {
  margin: 0 0 var(--space-3);
  font-size: var(--font-size-body);
}
"#;

/// Consistent empty-list presentation (RFC 092 / v2.3 §5).
///
/// Renders a centred message and an optional CTA link. The CTA is omitted
/// when `cta` is `None` — auditors see the message without mutation controls.
///
/// # Arguments
/// - `message` — already-localised message string (from `t.empty_*`)
/// - `cta` — optional `(href, label)` for the "create first …" link
pub fn empty_state(
    message: impl Into<String>,
    cta: Option<(&'static str, &'static str)>,
) -> impl leptos::prelude::IntoView {
    use leptos::prelude::*;
    let message = message.into();
    view! {
        <div class="empty-state">
            <p class="empty-state__message">{message}</p>
            {cta.map(|(href, label)| view! {
                <a href=href class="button">{label}</a>
            })}
        </div>
    }
}

// ── RFC 092: CopyField component ──────────────────────────────────────────────

pub(crate) const COPY_FIELD_CSS: &str = r#"
/* ---- CopyField (RFC 092) ---- */
.copy-field {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.copy-field__input {
  flex: 1;
  min-width: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  padding: var(--space-1) var(--space-2);
  border: var(--border-width-default) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  color: var(--fg-default);
}
"#;

/// A read-only value field with a copy-to-clipboard button (RFC 092 / v2.3 §5).
///
/// The `<input readonly>` carries `role="status"` so assistive technology
/// can announce the value without it being an interactive control. The copy
/// button reuses the existing `copy.js` / `copy-btn` mechanism.
///
/// # Arguments
/// - `t` — localised strings
/// - `value` — the value to display and copy
/// - `noun` — copy-button noun (e.g. `t.copy_noun_client_id`)
/// - `aria_label` — label for the read-only input (for screen readers)
pub fn copy_field(
    t: &'static sui_id_i18n::Strings,
    value: String,
    noun: &'static str,
    aria_label: &'static str,
) -> impl leptos::prelude::IntoView {
    use leptos::prelude::*;
    let phrase = t.copy_button_aria_template.replace("{noun}", noun);
    let aria = phrase.clone();
    let v2 = value.clone();
    view! {
        <div class="copy-field">
            <input
                type="text"
                class="copy-field__input"
                readonly
                role="status"
                aria-label=aria_label
                value=value
            />
            <button
                type="button"
                class="copy-btn"
                data-copy=v2
                data-copy-done=t.copy_button_label_done
                aria-label=aria
                title=phrase>
                {t.copy_button_label}
            </button>
        </div>
    }
}
