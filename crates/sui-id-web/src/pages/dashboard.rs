//! Page renderers for the "dashboard" screen domain (RFC 065).

use super::common::*;
use crate::layout::{Chrome, NavItem, Shell};
use leptos::prelude::*;

pub struct DashboardSparkBucket {
    /// Human-readable label for hover tooltip ("2026-04-26 14:00").
    pub label: String,
    pub success: i64,
    pub failure: i64,
}

pub struct DashboardSparkline {
    /// Active range, used to highlight the right tab.
    pub active_range_query: String,
    /// (query string, human label) for each available range tab.
    pub range_options: Vec<(String, String)>,
    /// Window-wide totals shown next to the sparkline.
    pub total_success: i64,
    pub total_failure: i64,
    /// Dense bucket array, oldest first. Empty windows still
    /// produce the right number of zero-count buckets so the
    /// sparkline is the same shape as for a busy window.
    pub buckets: Vec<DashboardSparkBucket>,
}

pub struct DashboardEventRow {
    pub at: chrono::DateTime<chrono::Utc>,
    pub action: String,
    pub actor_label: String,
    pub result: String,
}

/// RFC 149 §2 / Q3 §2: the first-run path's five steps' own done-ness.
/// Step 4 (review sign-in and session settings) has no done state — it
/// always shows *Recommended* — so it carries no field here.
pub struct GettingStartedData {
    pub mfa_done: bool,
    pub smtp_done: bool,
    pub first_app_done: bool,
    pub federation_done: bool,
}

impl GettingStartedData {
    /// True once every step that *can* be done (1, 2, 3) is — the
    /// condition under which the path folds to one line (§2: steps 4
    /// and 5 never hold it open).
    pub fn required_done(&self) -> bool {
        self.mfa_done && self.smtp_done && self.first_app_done
    }
}

/// RFC 149 §3 / Q3 §3: a kept pre-rotation master-key backup past its
/// own review date, read fresh on every dashboard request (never
/// cached) so a `backup renew`/`backup remove` made while the server
/// runs is reflected without a restart. No filesystem path — the
/// operator's own command report has that.
pub struct KeptBackupOverdue {
    pub review_at: chrono::DateTime<chrono::Utc>,
    pub custody_reference: String,
}

pub struct DashboardData {
    pub admin_username: String,
    pub user_count: usize,
    pub client_count: usize,
    pub active_session_count: usize,
    pub sparkline: DashboardSparkline,
    // Operator action prompts — shown when condition is true (RFC 031)
    pub warn_smtp_not_configured: bool,
    pub warn_hibp_off: bool,
    pub warn_cookie_insecure: bool,
    // RFC 073: additional action items (v0.58.0)
    pub admins_without_mfa: usize,
    pub oldest_active_key_age_days: Option<i64>,
    pub outbox_stuck_count: usize,
    pub pending_password_resets: usize,
    // RFC 149 §3 / Q3 §3: two more action items.
    pub kept_backup_overdue: Option<KeptBackupOverdue>,
    pub federation_providers_requiring_review: usize,
    /// RFC 149 §2 / Q3 §2: who sees the first-run path — an auditor
    /// cannot do steps 2 to 5, so they see none of it.
    pub is_administrator: bool,
    pub getting_started: GettingStartedData,
    // RFC 043: last N important audit events shown on dashboard
    pub recent_important: Vec<DashboardEventRow>,
}

impl DashboardData {
    /// True iff any RFC-073 action item, or either of the two Q3 ones,
    /// should render. Used to decide whether to draw the "Action
    /// items" section header at all.
    pub fn has_action_items(&self) -> bool {
        self.smtp_item_shown()
            || self.warn_hibp_off
            || self.warn_cookie_insecure
            || self.admins_without_mfa > 0
            || self.oldest_active_key_age_days.is_some_and(|d| d >= 330)
            || self.outbox_stuck_count > 0
            || self.pending_password_resets >= 5
            || self.kept_backup_overdue.is_some()
            || self.federation_providers_requiring_review > 0
    }

    /// Q3 §2 "no prompt twice": the standalone SMTP action item is
    /// suppressed exactly when the first-run path is shown (an
    /// administrator) *and* step 2 on it is not done — the path
    /// already says so. An auditor, who never sees the path, still
    /// sees this item.
    fn smtp_item_shown(&self) -> bool {
        if !self.warn_smtp_not_configured {
            return false;
        }
        !(self.is_administrator && !self.getting_started.smtp_done)
    }
}

/// Render the inline SVG sparkline.
///
/// The SVG is hand-coded rather than pulled from a charting
/// library: we only need a stacked area for two series, the
/// dimensions are fixed, and we avoid both a runtime dependency
/// and any CSP relaxation. Drawing strategy:
///
/// - viewBox is 0..200 horizontal × 0..60 vertical, scaled by CSS
/// - failures stack at the bottom (so a streak shows up as a thick
///   red base regardless of the success count above it)
/// - successes stack on top of failures
/// - each bucket carries an invisible `<rect>` with a `<title>`
///   child so hovering shows the tooltip natively (no JS)
fn render_sparkline(
    t: &'static sui_id_i18n::Strings,
    buckets: Vec<DashboardSparkBucket>,
) -> impl IntoView {
    const WIDTH: f64 = 200.0;
    const HEIGHT: f64 = 60.0;
    const PAD_TOP: f64 = 4.0;
    const PAD_BOTTOM: f64 = 4.0;
    let drawable = HEIGHT - PAD_TOP - PAD_BOTTOM;
    let n = buckets.len().max(1);
    // Largest stacked total across buckets sets the y-scale.
    let max_total = buckets
        .iter()
        .map(|b| b.success + b.failure)
        .max()
        .unwrap_or(0)
        .max(1) as f64;

    let bar_step = WIDTH / n as f64;
    // Each bucket gets a thin gap so adjacent bars are readable.
    let bar_w = (bar_step * 0.78).max(1.0);
    let bar_offset = (bar_step - bar_w) / 2.0;

    let bars: Vec<_> = buckets
        .into_iter()
        .enumerate()
        .map(|(i, b)| {
            let x = bar_step * i as f64 + bar_offset;
            let total = (b.success + b.failure) as f64;
            let total_h = if total > 0.0 {
                (total / max_total) * drawable
            } else {
                0.0
            };
            let success_h = if b.success > 0 {
                (b.success as f64 / max_total) * drawable
            } else {
                0.0
            };
            let failure_h = total_h - success_h;

            let base_y = HEIGHT - PAD_BOTTOM;
            let failure_y = base_y - failure_h;
            let success_y = failure_y - success_h;

            let title = (t.dashboard_sparkline_tooltip)(&b.label, b.success, b.failure);

            view! {
                <g>
                    <title>{title}</title>
                    <rect x=format!("{:.2}", bar_step * i as f64)
                          y="0"
                          width=format!("{:.2}", bar_step)
                          height=format!("{HEIGHT}")
                          fill="transparent" />
                    {(failure_h > 0.0).then(|| view! {
                        <rect x=format!("{x:.2}")
                              y=format!("{failure_y:.2}")
                              width=format!("{bar_w:.2}")
                              height=format!("{failure_h:.2}")
                              fill="var(--danger-default)"
                              rx="1" />
                    })}
                    {(success_h > 0.0).then(|| view! {
                        <rect x=format!("{x:.2}")
                              y=format!("{success_y:.2}")
                              width=format!("{bar_w:.2}")
                              height=format!("{success_h:.2}")
                              fill="var(--accent-default)"
                              rx="1" />
                    })}
                </g>
            }
        })
        .collect();

    view! {
        <svg viewBox=format!("0 0 {WIDTH} {HEIGHT}")
             preserveAspectRatio="none"
             role="img"
             aria-label=t.dashboard_sparkline_aria
             class="sparkline-container">
            <line x1="0" y1=format!("{:.2}", HEIGHT - PAD_BOTTOM)
                  x2=format!("{WIDTH}") y2=format!("{:.2}", HEIGHT - PAD_BOTTOM)
                  stroke="var(--border-muted)"
                  stroke-width="1" />
            {bars}
        </svg>
    }
}

pub fn render_dashboard(
    data: DashboardData,
    flash: Option<Flash>,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        // RFC 073 / RFC 149 §2-§3: compute visibility flags before
        // destructuring so we do not need to keep `data` around or
        // borrow it after move.
        let smtp_item_shown = data.smtp_item_shown();
        let has_action_items = data.has_action_items();
        let is_administrator = data.is_administrator;
        let DashboardData {
            admin_username,
            user_count,
            client_count,
            active_session_count,
            sparkline,
            warn_smtp_not_configured: _warn_smtp_not_configured,
            warn_hibp_off,
            warn_cookie_insecure,
            admins_without_mfa,
            oldest_active_key_age_days,
            outbox_stuck_count,
            pending_password_resets,
            kept_backup_overdue,
            federation_providers_requiring_review,
            is_administrator: _is_administrator,
            getting_started,
            recent_important,
        } = data;

        let active_range = sparkline.active_range_query.clone();
        let range_tabs: Vec<_> = sparkline
            .range_options
            .iter()
            .map(|(q, label)| {
                let href = format!("/admin?range={q}#sparkline");
                let aria = if *q == active_range {
                    Some("page")
                } else {
                    None
                };
                view! {
                    <a class="app-nav__link" href=href aria-current=aria>{label.clone()}</a>
                }
            })
            .collect();

        let total_success = sparkline.total_success;
        let total_failure = sparkline.total_failure;
        let svg = render_sparkline(t, sparkline.buckets);
        let nav_username = admin_username.clone();

        view! {
            <Shell title=t.dashboard_title.to_string() nav=Chrome::Console{username: nav_username, item: NavItem::Dashboard} dev_mode=dev_mode lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.dashboard_title}</h1>
                        <p class="page-header__lede">
                            {(t.dashboard_greeting)(admin_username.as_str())}
                            " "
                            {t.dashboard_lede}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                // RFC 149 §2 / Q3 §2: the first-run path. An auditor
                // cannot do steps 2 to 5, so sees none of it. Steps 1
                // to 3 fold the path to one line once all three are
                // done; steps 4 and 5 never hold it open.
                {is_administrator.then(|| {
                    let open = !getting_started.required_done();
                    // A step that *can* be done: "Done" in a solid badge,
                    // or no status word at all — never "to do" (§2).
                    let done_step = |done: bool, label: &'static str, href: &'static str| view! {
                        <li class=done.then_some("done")>
                            {done.then(|| view! { <span class="badge badge--ok">{t.dashboard_gs_status_done}</span> })}
                            <a href=href>{label}</a>
                        </li>
                    };
                    view! {
                        <details class="callout callout--info mb-4" open=open>
                            <summary class="callout__title">
                                {if open { t.dashboard_gs_title } else { t.dashboard_gs_done }}
                            </summary>
                            <ol class="getting-started-path">
                                {done_step(getting_started.mfa_done, t.dashboard_gs_step1, "/me/security/mfa")}
                                {done_step(getting_started.smtp_done, t.dashboard_gs_step2, NavItem::Email.href())}
                                {done_step(getting_started.first_app_done, t.dashboard_gs_step3, "/admin/clients/new")}
                                <li>
                                    <span class="badge">{t.dashboard_gs_status_recommended}</span>
                                    {t.dashboard_gs_step4}
                                    " — "
                                    <a href=NavItem::SignInSettings.href()>
                                        {NavItem::SignInSettings.label(t)}
                                    </a>
                                    " · "
                                    <a href=NavItem::SessionsAndLockout.href()>
                                        {NavItem::SessionsAndLockout.label(t)}
                                    </a>
                                </li>
                                <li class=getting_started.federation_done.then_some("done")>
                                    <span class=if getting_started.federation_done { "badge badge--ok" } else { "badge" }>
                                        {if getting_started.federation_done {
                                            t.dashboard_gs_status_done
                                        } else {
                                            t.dashboard_gs_status_optional
                                        }}
                                    </span>
                                    {t.dashboard_gs_step5}
                                    " — "
                                    <a href=concat!(
                                        crate::github_doc_url!("docs/src/guides/operators.md"),
                                        "#federated-sign-in-upstream-oidc",
                                    )>
                                        {t.help_learn_more}
                                    </a>
                                </li>
                            </ol>
                        </details>
                    }
                })}

                // RFC 073: Action items — appears when any condition is true.
                // Sorted within the list by severity (danger first), then info.
                {has_action_items.then(|| {
                    let old_key_warning = oldest_active_key_age_days
                        .filter(|d| *d >= 330);
                    view! {
                        <section class="callout callout--warning mb-4" aria-label=t.dashboard_aria_action_required>
                            <h2 class="callout__title">
                                "⚠ " {t.dashboard_action_required_title}
                            </h2>
                            <ul class="action-items-list">
                                // From RFC 031 (existing): danger-level config gaps.
                                // Q3 §2 "no prompt twice": suppressed while the
                                // first-run path already says step 2 is open.
                                {smtp_item_shown.then(|| view! { <li>{t.dashboard_warn_smtp}</li> })}
                                {warn_hibp_off.then(|| view! { <li>{t.dashboard_warn_hibp}</li> })}
                                {warn_cookie_insecure.then(|| view! { <li>{t.dashboard_warn_cookie_insecure}</li> })}
                                // RFC 073 (existing): operational signals
                                {(outbox_stuck_count > 0).then(|| view! {
                                    <li>{(t.dashboard_warn_outbox_stuck)(outbox_stuck_count)}</li>
                                })}
                                {(admins_without_mfa > 0).then(|| view! {
                                    <li>{(t.dashboard_warn_admins_no_mfa)(admins_without_mfa)}</li>
                                })}
                                {old_key_warning.map(|age| view! {
                                    <li>{(t.dashboard_warn_old_signing_key)(age)}</li>
                                })}
                                {(pending_password_resets >= 5).then(|| view! {
                                    <li>{(t.dashboard_warn_pending_resets)(pending_password_resets)}</li>
                                })}
                                // RFC 149 §3 / Q3 §3: two more items.
                                {kept_backup_overdue.map(|b| {
                                    let text = (t.dashboard_warn_backup_overdue)(
                                        &fmt_time(b.review_at),
                                        &b.custody_reference,
                                    );
                                    view! {
                                        <li>
                                            {text}
                                            " "
                                            <a href=concat!(
                                                crate::github_doc_url!("docs/src/guides/operators.md"),
                                                "#rotating-the-master-key",
                                            )>
                                                {t.help_learn_more}
                                            </a>
                                        </li>
                                    }
                                })}
                                {(federation_providers_requiring_review > 0).then(|| view! {
                                    <li>
                                        {(t.dashboard_warn_federation_requires_review)(federation_providers_requiring_review)}
                                        " "
                                        <a href=NavItem::FederationProviders.href()>
                                            {NavItem::FederationProviders.label(t)}
                                        </a>
                                    </li>
                                })}
                            </ul>
                        </section>
                    }
                })}

                // RFC 043 (promoted by RFC 063): Recent important events.
                // Pinned high in the visual hierarchy because operators
                // triaging the IdP need to see "what just happened" before
                // reference stats. `.card--info` (RFC 062) gives it a
                // blue-tinted callout look — distinct from warn (amber)
                // but more weighted than ordinary stat cards.
                <section class="card card--info">
                    <h2 class="card__title">{t.dashboard_recent_events_title}</h2>
                    {if recent_important.is_empty() {
                        empty_state(EmptyStateData {
                            message: t.dashboard_recent_events_empty.into(),
                            hint: None,
                            action: None,
                            compact: true,
                        }).into_any()
                    } else {
                        let rows: Vec<_> = recent_important.into_iter().map(|r| {
                            let badge_class = match r.result.as_str() {
                                "ok"  => "badge badge--ok",
                                "fail" | "denied" | "error" => "badge badge--danger",
                                _ => "badge",
                            };
                            view! {
                                <tr>
                                    <td class="audit-mini__time">
                                        <time>{r.at.format("%m/%d %H:%M").to_string()}</time>
                                    </td>
                                    <td><code class="audit-action">{r.action}</code></td>
                                    <td class="muted">{r.actor_label}</td>
                                    <td><span class=badge_class>{r.result}</span></td>
                                </tr>
                            }
                        }).collect();
                        view! {
                            <>
                            <div class="table-wrap">
                                <table class="audit-mini">
                                    <tbody>{rows}</tbody>
                                </table>
                            </div>
                            <p class="card__footer mt-2">
                                <a href="/admin/audit">{t.dashboard_recent_events_view_all}</a>
                            </p>
                            </>
                        }.into_any()
                    }}
                </section>

                <section class="grid-cards" aria-label=t.dashboard_aria_stats>
                    <div class="card">
                        <div class="stat">
                            <span class="stat__value">{user_count.to_string()}</span>
                            <span class="stat__label">{t.dashboard_stat_users}</span>
                        </div>
                    </div>
                    <div class="card">
                        <div class="stat">
                            <span class="stat__value">{client_count.to_string()}</span>
                            <span class="stat__label">{t.dashboard_stat_clients}</span>
                        </div>
                    </div>
                    <div class="card">
                        <div class="stat">
                            <span class="stat__value">{active_session_count.to_string()}</span>
                            <span class="stat__label">{t.dashboard_stat_sessions}</span>
                        </div>
                    </div>
                    <div class="card">
                        <div class="stat">
                            <span class="stat__value">
                                <span class="badge badge--ok">{t.dashboard_stat_service_ok}</span>
                            </span>
                            <span class="stat__label">{t.dashboard_stat_service_status}</span>
                        </div>
                    </div>
                </section>

                <section id="sparkline">
                    <div class="sparkline-header">
                        // RFC 063: dashboard sparkline is reference, not action.
                        // h3 (was h2) + dim opacity nudges it into the
                        // "background trend" register.
                        <h3 class="sparkline-title">
                            {t.dashboard_activity_title}
                        </h3>
                        <nav class="app-nav flex-0-auto" aria-label=t.dashboard_activity_period>
                            {range_tabs}
                        </nav>
                    </div>
                    <div class="card">
                        <div class="sparkline-legend">
                            <div class="stat">
                                <span class="stat__value color-accent">
                                    {total_success.to_string()}
                                </span>
                                <span class="stat__label">{t.dashboard_activity_success}</span>
                            </div>
                            <div class="stat">
                                <span class="stat__value color-danger">
                                    {total_failure.to_string()}
                                </span>
                                <span class="stat__label">{t.dashboard_activity_failure}</span>
                            </div>
                        </div>
                        {svg}
                        <p class="muted mt-2-mb-0">
                            {t.dashboard_activity_hover_hint}
                        </p>
                    </div>
                </section>

            </Shell>
        }
    })
}
