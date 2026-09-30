//! Shared helpers and primitives for the page renderers.
//!
//! Private helpers (flash_banner, fmt_time, render, copy_btn,
//! kv_row) live here as `pub(super)` so every sibling
//! screen module under `pages/` can call them without exposing
//! them outside `sui-id-web`.
//!
//! Public types (Flash, FlashKind, EmptyStateData, EmptyStateAction)
//! stay `pub` because handler crates construct them when triggering
//! a page render with a flash message or empty-state CTA.
//!
//! Originally lived at the top of `pages.rs` (pre-RFC 065). The
//! split made these the only items needed by every screen module.

use chrono::{DateTime, Utc};
use leptos::prelude::*;
use leptos::reactive::owner::Owner;

pub(super) const DOCTYPE: &str = "<!DOCTYPE html>";

/// One audit-chain verification result, in the shape every page that shows
/// it renders (RFC 121 D1, D2). The handler crate builds this from
/// `sui_id_core::audit_chain::ChainCheck` — `sui-id-web` does not depend on
/// `sui-id-core`, so this is the boundary type, not a re-export of it.
///
/// There is no default variant and no `Option` standing in for one: a page
/// that has a result to show always has exactly one of these three.
#[derive(Debug, Clone)]
pub enum ChainStatus {
    Intact {
        checked: usize,
        legacy_unhashed: usize,
    },
    Broken {
        at_seq: i64,
        checked: usize,
        legacy_unhashed: usize,
    },
    CouldNotVerify {
        detail: String,
    },
}

/// The badge kind and explanatory note for one [`ChainStatus`] — the one
/// place that decides the words, so the audit page and the settings logs
/// page cannot drift into describing the same outcome differently.
pub(super) fn chain_status_words(
    t: &'static sui_id_i18n::Strings,
    status: &ChainStatus,
) -> (crate::components::StatusKind, String) {
    use crate::components::StatusKind;
    match status {
        ChainStatus::Intact {
            checked,
            legacy_unhashed,
        } => (
            StatusKind::Healthy,
            (t.audit_chain_ok_note)(*checked, *legacy_unhashed),
        ),
        ChainStatus::Broken { at_seq, .. } => {
            (StatusKind::Unhealthy, (t.audit_chain_broken_note)(*at_seq))
        }
        ChainStatus::CouldNotVerify { detail } => (
            StatusKind::Unknown,
            (t.audit_chain_could_not_verify_note)(detail),
        ),
    }
}

/// The short label for a [`ChainStatus`], for a surface that wants the
/// headline text alone (the audit page's top banner) rather than the badge
/// plus explanatory note pairing [`chain_status_words`] gives.
pub(super) fn chain_status_label(
    t: &'static sui_id_i18n::Strings,
    status: &ChainStatus,
) -> &'static str {
    match status {
        ChainStatus::Intact { .. } => t.audit_chain_ok,
        ChainStatus::Broken { .. } => t.audit_chain_broken,
        ChainStatus::CouldNotVerify { .. } => t.audit_chain_could_not_verify,
    }
}

/// Severity of a flash banner displayed at the top of a page.
#[derive(Debug, Clone, Copy)]
pub enum FlashKind {
    Info,
    Warn,
    Error,
}

impl FlashKind {
    fn class(self) -> &'static str {
        match self {
            Self::Info => "flash info",
            Self::Warn => "flash warn",
            Self::Error => "flash error",
        }
    }

    /// ARIA live-region role per kind (RFC-MI-041 §8 ABDD).
    /// `Error` uses `role="alert"` to interrupt assistive tech
    /// immediately for failures (login failure, MFA failure,
    /// step-up failure, reset-token errors). `Info`/`Warn` use
    /// `role="status"` to be announced politely without
    /// interrupting the user's current activity.
    fn aria_role(self) -> &'static str {
        match self {
            Self::Info | Self::Warn => "status",
            Self::Error => "alert",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Flash {
    pub kind: FlashKind,
    pub text: String,
}

pub(super) fn flash_banner(flash: Option<Flash>) -> Option<impl IntoView> {
    flash.map(|f| view! { <div class=f.kind.class() role=f.kind.aria_role()>{f.text}</div> })
}

pub(super) fn fmt_time(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M UTC").to_string()
}

/// Run a closure inside a fresh reactive Owner and prepend the HTML doctype.
pub(super) fn render<F, V>(f: F) -> String
where
    F: FnOnce() -> V,
    V: IntoView + 'static,
{
    let owner = Owner::new();
    let body = owner.with(|| f().into_view().to_html());
    let mut out = String::with_capacity(DOCTYPE.len() + body.len());
    out.push_str(DOCTYPE);
    out.push_str(&body);
    out
}

pub(super) fn copy_btn(
    t: &'static sui_id_i18n::Strings,
    value: String,
    noun: &'static str,
) -> impl IntoView {
    let phrase = t.copy_button_aria_template.replace("{noun}", noun);
    let aria = phrase.clone();
    view! {
        <button
            type="button"
            class="copy-btn"
            data-copy=value
            data-copy-done=t.copy_button_label_done
            aria-label=aria
            title=phrase>
            {t.copy_button_label}
        </button>
    }
}

/// Two-column key/value row used inside settings cards and detail panels.
/// Returns a `<tr>` with a fixed-width label cell and a value cell —
/// callers wrap with `<table>`.
pub(super) fn kv_row(k: &str, v: impl IntoView + 'static) -> impl IntoView {
    let k = k.to_owned();
    view! {
        <tr>
            <th scope="row" class="kv-label-cell">
                {k}
            </th>
            <td>{v}</td>
        </tr>
    }
}

pub(super) fn kv_text(k: &str, v: String) -> impl IntoView {
    kv_row(k, view! { <span>{v}</span> })
}

pub(super) fn kv_code(k: &str, v: String) -> impl IntoView {
    kv_row(k, view! { <span class="code">{v}</span> })
}

pub(super) fn kv_bool_badge(t: &'static sui_id_i18n::Strings, k: &str, on: bool) -> impl IntoView {
    let badge = if on {
        view! { <span class="badge badge--ok">{t.badge_enabled}</span> }.into_any()
    } else {
        view! { <span class="badge">{t.badge_disabled}</span> }.into_any()
    };
    kv_row(k, badge)
}

// ---------- Empty-state primitive (RFC 064) ----------

/// Optional call-to-action attached to an empty state.
pub struct EmptyStateAction {
    pub href: String,
    pub label: String,
}

/// Data driving the shared `empty_state` helper (RFC 064).
pub struct EmptyStateData {
    pub message: String,
    pub hint: Option<String>,
    pub action: Option<EmptyStateAction>,
    pub compact: bool,
}

pub fn empty_state(data: EmptyStateData) -> impl IntoView {
    let cls = if data.compact {
        "empty-state empty-state--compact"
    } else {
        "empty-state"
    };
    view! {
        <div class=cls>
            <p class="empty-state__message">{data.message}</p>
            {data.hint.map(|h| view! { <p class="empty-state__hint muted">{h}</p> })}
            {data.action.map(|a| view! {
                <a href=a.href class="button secondary empty-state__action">{a.label}</a>
            })}
        </div>
    }
}

/// Table-row variant of the empty-state primitive (RFC 064).
pub fn table_empty_row(message: &'static str, colspan: usize) -> impl IntoView {
    view! {
        <tr>
            <td colspan=colspan.to_string()
                class="center-pad-6-muted">
                {message}
            </td>
        </tr>
    }
}
