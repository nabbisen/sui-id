//! RFC 149 §3 / Q3 §1: help in three layers — this is layer 2, on
//! request. One shared `<details class="help">`: a visible `?` with
//! an accessible name (e.g. *"About session limits"*), opening in
//! place with a click, a tap, or the keyboard, with no JavaScript.
//! At most three sentences, then a *Learn more* link built from
//! [`crate::github_doc_url!`].

use leptos::prelude::*;

/// `summary_label` is the `?` toggle's accessible name (e.g.
/// `"About session limits"` — not the visible `?` glyph, which
/// carries no information of its own). `body` is the help text
/// itself: at most three sentences, checked against the code it
/// documents. `learn_more_href` is a `crate::github_doc_url!(...)`
/// expansion, optionally with a `#slug` anchor.
pub fn help_details(
    t: &'static sui_id_i18n::Strings,
    summary_label: String,
    body: &'static str,
    learn_more_href: &'static str,
) -> impl IntoView {
    view! {
        <details class="help">
            <summary aria-label=summary_label>"?"</summary>
            <div class="help__body">
                <p>{body}</p>
                <a href=learn_more_href class="help__learn-more">{t.help_learn_more}</a>
            </div>
        </details>
    }
}

pub const HELP_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* RFC 149 §3 / Q3 §1: on-request help (`<details class="help">`)     */
/* ------------------------------------------------------------------ */

.help {
  display: inline-block;
  margin-left: var(--space-2);
  vertical-align: middle;
}
.help > summary {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  /* RFC 144 §4: 44×44 CSS px for a standalone control (1.75rem was 28px). */
  width: 2.75rem;
  height: 2.75rem;
  border-radius: 999px;
  border: var(--border-width-default) solid var(--border-control);
  background: var(--surface-elevated);
  color: var(--fg-muted);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-bold);
  cursor: pointer;
  list-style: none;
}
.help > summary::-webkit-details-marker { display: none; }
.help > summary:hover {
  color: var(--fg-default);
  background: var(--state-hover);
}
.help[open] > summary {
  background: var(--accent-subtle);
  color: var(--accent-default);
}
.help__body {
  margin-top: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-subtle);
  border: var(--border-width-default) solid var(--border-muted);
  border-radius: var(--radius-md);
  /* Never wider than the window: at a 200% font the 42rem cap alone
   * let an open help block widen the page at 375px (Q3's review). */
  max-width: min(42rem, calc(100vw - 2 * var(--space-4)));
  overflow-wrap: anywhere;
  /* The `?` sits inside a section heading; its text is body text, not
   * the heading's size, weight or capitals (Q3's review: it rendered
   * as a bold heading). */
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-regular);
  letter-spacing: normal;
  text-transform: none;
  color: var(--fg-default);
}
.help__body p {
  margin: 0 0 var(--space-2) 0;
}
.help__learn-more {
  font-size: var(--font-size-caption);
}
"#;
