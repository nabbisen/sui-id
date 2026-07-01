//! Status badges.
//!
//! Owns: the `.badge`, `.badge--ok`, `.badge--warn`, `.badge--danger`,
//! `.badge--accent`, `.badge--info`, `.badge--muted` CSS family, the
//! [`StatusKind`] enum, and the [`status_badge`] render function.
//!
//! The status vocabulary is frozen by RFC 044 / RFC 052: every
//! reachable badge state has both a canonical text string (in
//! [`sui_id_i18n::Strings`]) and a single CSS class. Adding a new
//! variant requires both a new `StatusKind` value and a matching
//! `status_*` string field; see the [`status_badge`] match arms for
//! the contract.
//!
//! Two CSS sub-constants preserve cascade order: the muted variant
//! (RFC 052) sits AFTER the confirm-screen rules in the original
//! `components.rs`.

use leptos::prelude::*;

pub const BADGES_BASE_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Status badges                                                       */
/* ------------------------------------------------------------------ */

.badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 2px var(--space-2);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
  background: var(--surface-subtle);
  color: var(--fg-muted);
  border: var(--border-width-default) solid var(--border-muted);
}
.badge--ok      { color: var(--success-default); background: transparent; border-color: currentColor; }
.badge--warn    { color: var(--warning-default); background: transparent; border-color: currentColor; }
.badge--danger  { color: var(--danger-default);  background: transparent; border-color: currentColor; }
.badge--accent  { color: var(--accent-default);  background: var(--accent-subtle); border-color: transparent; }

"#;

pub const BADGES_MUTED_CSS: &str = r#"
/* ── Status badge muted variant (RFC 052) ──────────────────────────── */
/* Used for `retired` and similar low-emphasis status values that aren't */
/* failure, warning, or success — just "no longer current."              */
.badge--muted {
  background: color-mix(in srgb, var(--fg-muted) 12%, transparent);
  color: var(--fg-muted);
}
"#;

/// Status badge kind. One source of truth for the badge text and CSS
/// class mapping; previously duplicated across 24+ call sites in
/// `pages.rs`.
///
/// New variants must add a matching `status_*` field to
/// [`sui_id_i18n::Strings`] and update the match in [`status_badge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Live and serving traffic. → `badge badge--ok`
    Active,
    /// Recoverable suspension. → `badge badge--warn`
    Disabled,
    /// Tombstoned (soft delete or hard delete). → `badge badge--danger`
    Deleted,
    /// Administrator role marker. → `badge badge--accent`
    Admin,
    /// Generic on indicator. → `badge badge--ok`
    On,
    /// Generic off indicator. → `badge` (neutral)
    Off,
    /// Currently the active signing key. → `badge badge--ok`
    InUse,
    /// Old signing key, kept for token verification. → `badge badge--muted`
    Retired,
    /// Visible to clients via JWKS. → `badge badge--ok`
    Published,
    /// Awaiting human or automated decision. → `badge badge--info`
    Pending,
    /// Service is healthy. → `badge badge--ok`
    Healthy,
    /// Service is unhealthy. → `badge badge--danger`
    Unhealthy,
}

/// Render a status badge with localised text and the matching CSS
/// class. The badge sits inline; wrap it in a `<td>` or other parent
/// at the call site if needed.
pub fn status_badge(t: &'static sui_id_i18n::Strings, kind: StatusKind) -> impl IntoView {
    let (class, text) = match kind {
        StatusKind::Active => ("badge badge--ok", t.status_active),
        StatusKind::Disabled => ("badge badge--warn", t.status_disabled),
        StatusKind::Deleted => ("badge badge--danger", t.status_deleted),
        StatusKind::Admin => ("badge badge--accent", t.status_admin),
        StatusKind::On => ("badge badge--ok", t.status_on),
        StatusKind::Off => ("badge", t.status_off),
        StatusKind::InUse => ("badge badge--ok", t.status_in_use),
        StatusKind::Retired => ("badge badge--muted", t.status_retired),
        StatusKind::Published => ("badge badge--ok", t.status_published),
        StatusKind::Pending => ("badge badge--info", t.status_pending),
        StatusKind::Healthy => ("badge badge--ok", t.status_healthy),
        StatusKind::Unhealthy => ("badge badge--danger", t.status_unhealthy),
    };
    view! { <span class=class>{text}</span> }
}
