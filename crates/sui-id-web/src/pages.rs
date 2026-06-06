//! Page-level components and their public render entry points.
//!
//! Each `render_xxx` function constructs a Leptos view, drives it through
//! the SSR renderer, and returns a complete HTML document. The doctype is
//! prepended manually because `view!{}` only renders the tree it is given.

use crate::layout::Shell;
use chrono::{DateTime, Utc};
use leptos::prelude::*;
use leptos::reactive::owner::Owner;
use sui_id_shared::api::{AuditLogEntryDto, ClientSummary, UserSummary};

const DOCTYPE: &str = "<!DOCTYPE html>";

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
}

#[derive(Debug, Clone)]
pub struct Flash {
    pub kind: FlashKind,
    pub text: String,
}

fn flash_banner(flash: Option<Flash>) -> Option<impl IntoView> {
    flash.map(|f| view! { <div class=f.kind.class() role="status">{f.text}</div> })
}

fn fmt_time(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M UTC").to_string()
}

/// Run a closure inside a fresh reactive Owner and prepend the HTML doctype.
fn render<F, V>(f: F) -> String
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

// ---------- copy-to-clipboard helper (RFC 028) ----------

/// Render a copy-to-clipboard button for a credential value.
///
/// `noun` is one of the `copy_noun_*` strings from
/// [`sui_id_i18n::Strings`]; the aria-label and title are built by
/// substituting it into `copy_button_aria_template`. The button text
/// is `copy_button_label`; after a successful copy, the inline JS in
/// [`crate::layout::COPY_JS`] swaps it for `copy_button_label_done`
/// (carried on the button via a `data-copy-done` attribute) and
/// restores after a short delay.
///
/// Hidden via CSS when the Clipboard API is unavailable (non-secure
/// context). See `components.rs` `.copy-btn` rules.
fn copy_btn(
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

// ---------- setup wizard (3 steps: welcome → admin → done) ----------
//
// The design memo describes 4 screens (1 welcome, 2 admin, 3 encryption, 4 done).
// sui-id resolves the master key before HTTP is up (env var or key file with
// auto-generation), so screen 3 has no operator-facing surface to expose; we
// render screens 1 / 2 / 4 as steps 1 / 2 / 3 of a 3-step wizard, preserving
// the design book's screen numbering. See `docs/operators.md` and the v0.20.4
// CHANGELOG entry for the rationale.

/// Numeric position of the active step. 0-indexed for array math, but
/// the visible label uses `{step + 1} / 3` to match natural language.
fn setup_step_indicator(active: usize, lang: sui_id_i18n::Locale) -> impl IntoView {
    // Five labelled dots showing which step the operator is on.
    // Steps are: Welcome(0), Admin(1), Language(2), HIBP(3), Done(4).
    let t = lang.strings();
    let labels = [
        t.setup_step_welcome,
        t.setup_step_admin,
        t.setup_step_lang,
        t.setup_step_hibp,
        t.setup_step_done,
    ];
    let dots: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == active;
            let aria = if is_active { Some("step") } else { None };
            let badge = if i < active {
                view! { <span class="badge badge--ok">{format!("{}", i + 1)}</span> }
                    .into_any()
            } else if is_active {
                view! { <span class="badge badge--accent">{format!("{}", i + 1)}</span> }
                    .into_any()
            } else {
                view! { <span class="badge">{format!("{}", i + 1)}</span> }.into_any()
            };
            let style = if is_active {
                "color:var(--fg-default);font-weight:var(--font-weight-medium)"
            } else if i < active {
                "color:var(--fg-muted)"
            } else {
                "color:var(--fg-subtle)"
            };
            view! {
                <span class="row" style="gap:var(--space-1);align-items:center" aria-current=aria>
                    {badge}
                    <span style=style>{*label}</span>
                </span>
            }
        })
        .collect();
    view! {
        <nav class="row"
             aria-label=t.setup_steps_aria
             style="gap:var(--space-3);justify-content:center;margin-bottom:var(--space-4);flex-wrap:wrap;font-size:var(--font-size-caption)">
            {dots}
        </nav>
    }
}

/// Step 1 of 3 — welcome.
pub fn render_setup_welcome(flash: Option<Flash>, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.setup_welcome_title.to_string() lang=lang>
                {setup_step_indicator(0, lang)}
                <h1>{t.setup_welcome_title}</h1>
                <p class="muted">{t.setup_welcome_lede}</p>
                <p class="muted">{t.setup_welcome_lede2}</p>
                {flash_banner(flash)}
                <p style="margin-top:var(--space-4)">
                    <a href="/setup/admin" class="button">{t.setup_welcome_begin}</a>
                </p>
            </crate::layout::AuthShell>
        }
    })
}

/// Step 2 of 3 — admin form.
pub fn render_setup_admin(flash: Option<Flash>, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.setup_admin_title.to_string() lang=lang>
                {setup_step_indicator(1, lang)}
                <h1>{t.setup_admin_title}</h1>
                <p class="muted">{t.setup_admin_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/setup/admin" class="stack" autocomplete="off">
                    <div class="field">
                        <label for="token" class="field__label">{t.setup_admin_token_label}</label>
                        <input id="token" name="setup_token" type="password"
                               required=true autocomplete="off" autofocus=true />
                        <span class="field__hint">{t.setup_admin_token_hint}</span>
                    </div>
                    <div class="field">
                        <label for="username" class="field__label">{t.setup_admin_username_label}</label>
                        <input id="username" name="username" type="text"
                               required=true autocomplete="username" />
                    </div>
                    <div class="field">
                        <label for="email" class="field__label">{t.setup_admin_email_label}</label>
                        <input id="email" name="email" type="email" autocomplete="email" />
                        <span class="field__hint">{t.setup_admin_email_hint}</span>
                    </div>
                    <div class="field">
                        <label for="display" class="field__label">{t.setup_admin_display_label}</label>
                        <input id="display" name="display_name" type="text" autocomplete="name" />
                    </div>
                    <div class="field">
                        <label for="password" class="field__label">{t.setup_admin_password_label}</label>
                        <input id="password" name="password" type="password"
                               required=true minlength="12" autocomplete="new-password" />
                        <span class="field__hint">{t.setup_admin_password_hint}</span>
                    </div>
                    <div class="field">
                        <label for="confirm_password" class="field__label">{t.setup_admin_confirm_label}</label>
                        <input id="confirm_password" name="confirm_password" type="password"
                               required=true minlength="12" autocomplete="new-password" />
                    </div>
                    <div class="row">
                        <a href="/setup" class="button secondary">{t.button_back}</a>
                        <button type="submit">{t.setup_admin_submit}</button>
                    </div>
                </form>
            </crate::layout::AuthShell>
        }
    })
}

/// Step 3 of 5 — language selection (RFC 012).
pub fn render_setup_lang(flash: Option<Flash>, current: &str, lang: sui_id_i18n::Locale) -> String {
    let current = current.to_owned();
    render(move || {
        let t = lang.strings();
        let ja_checked = current.is_empty() || current == "ja";
        let en_checked = current == "en";
        view! {
            <crate::layout::AuthShell title=t.setup_lang_title.to_string() lang=lang>
                {setup_step_indicator(2, lang)}
                <h1>{t.setup_lang_title}</h1>
                <p class="muted">{t.setup_lang_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/setup/lang" class="stack">
                    <fieldset style="border:none;padding:0;margin:0">
                        <legend class="field__label">{t.setup_lang_field_label}</legend>
                        <div class="stack" style="gap:var(--space-2)">
                            <label class="row" style="gap:var(--space-2);align-items:center;cursor:pointer">
                                <input type="radio" name="lang" value="ja"
                                       checked=ja_checked />
                                <span>{t.locale_native_ja}</span>
                            </label>
                            <label class="row" style="gap:var(--space-2);align-items:center;cursor:pointer">
                                <input type="radio" name="lang" value="en"
                                       checked=en_checked />
                                <span>{t.locale_native_en}</span>
                            </label>
                        </div>
                    </fieldset>
                    <p class="muted" style="font-size:var(--font-size-caption)">{t.setup_lang_default_note}</p>
                    <div class="row" style="justify-content:flex-end">
                        <button type="submit">{t.setup_lang_submit}</button>
                    </div>
                </form>
            </crate::layout::AuthShell>
        }
    })
}

/// Step 4 of 5 — HIBP policy selection (RFC 012).
pub fn render_setup_hibp(flash: Option<Flash>, current: &str, lang: sui_id_i18n::Locale) -> String {
    let current = current.to_owned();
    render(move || {
        let t = lang.strings();
        let off_checked = current == "off";
        let warn_checked = current.is_empty() || current == "warn";
        let block_checked = current == "block";
        view! {
            <crate::layout::AuthShell title=t.setup_hibp_step_title.to_string() lang=lang>
                {setup_step_indicator(3, lang)}
                <h1>{t.setup_hibp_step_title}</h1>
                <p class="muted">{t.setup_hibp_step_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/setup/hibp" class="stack">
                    <fieldset style="border:none;padding:0;margin:0">
                        <div class="stack" style="gap:var(--space-3)">
                            <label class="card" style="cursor:pointer;display:block">
                                <div class="row" style="gap:var(--space-2);align-items:center">
                                    <input type="radio" name="hibp_mode" value="off"
                                           checked=off_checked />
                                    <strong>{t.setup_hibp_option_off}</strong>
                                </div>
                                <p class="muted" style="margin:var(--space-1) 0 0 calc(1em + var(--space-2));font-size:var(--font-size-caption)">{t.setup_hibp_option_off_desc}</p>
                            </label>
                            <label class="card" style="cursor:pointer;display:block">
                                <div class="row" style="gap:var(--space-2);align-items:center">
                                    <input type="radio" name="hibp_mode" value="warn"
                                           checked=warn_checked />
                                    <strong>{t.setup_hibp_option_warn}</strong>
                                </div>
                                <p class="muted" style="margin:var(--space-1) 0 0 calc(1em + var(--space-2));font-size:var(--font-size-caption)">{t.setup_hibp_option_warn_desc}</p>
                            </label>
                            <label class="card" style="cursor:pointer;display:block">
                                <div class="row" style="gap:var(--space-2);align-items:center">
                                    <input type="radio" name="hibp_mode" value="block"
                                           checked=block_checked />
                                    <strong>{t.setup_hibp_option_block}</strong>
                                </div>
                                <p class="muted" style="margin:var(--space-1) 0 0 calc(1em + var(--space-2));font-size:var(--font-size-caption)">{t.setup_hibp_option_block_desc}</p>
                            </label>
                        </div>
                    </fieldset>
                    <p class="muted" style="font-size:var(--font-size-caption)">{t.setup_hibp_step_default_note}</p>
                    <div class="row" style="justify-content:flex-end">
                        <button type="submit">{t.setup_hibp_step_submit}</button>
                    </div>
                </form>
            </crate::layout::AuthShell>
        }
    })
}

/// Step 5 of 5 — completion.
pub fn render_setup_done(initialized: bool, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        if initialized {
            view! {
                <crate::layout::AuthShell title=t.setup_done_title.to_string() lang=lang>
                    {setup_step_indicator(4, lang)}
                    <h1>{t.setup_done_title}</h1>
                    <p class="muted">{t.setup_done_lede}</p>
                    <div class="card">
                        <h3 class="card__title">{t.setup_done_next_steps_title}</h3>
                        <ul class="muted" style="margin:0;padding-left:var(--space-4)">
                            <li>{t.setup_done_next_step_register_clients}</li>
                            <li>{t.setup_done_next_step_enable_mfa}</li>
                            <li>{t.setup_done_next_step_review_settings}</li>
                        </ul>
                    </div>
                    <p style="margin-top:var(--space-4)">
                        <a href="/admin" class="button">{t.setup_done_enter_admin}</a>
                    </p>
                </crate::layout::AuthShell>
            }
            .into_any()
        } else {
            view! {
                <crate::layout::AuthShell title=t.setup_not_complete_title.to_string() lang=lang>
                    {setup_step_indicator(0, lang)}
                    <h1>{t.setup_not_complete_title}</h1>
                    <p class="muted">{t.setup_not_complete_lede}</p>
                    <p style="margin-top:var(--space-4)">
                        <a href="/setup" class="button">{t.setup_welcome_begin}</a>
                    </p>
                </crate::layout::AuthShell>
            }
            .into_any()
        }
    })
}

// ---------- login ----------

pub fn render_login(
    flash: Option<Flash>,
    next: Option<String>,
    lang: sui_id_i18n::Locale,
    // When true, renders a passkey sign-in button above the password form (RFC 034).
    show_passkey_option: bool,
) -> String {
    render(move || {
        let next_value = next.clone().unwrap_or_default();
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.login_title.to_string() lang=lang>
                <h1>{t.login_title}</h1>
                {flash_banner(flash)}
                {show_passkey_option.then(|| view! {
                    <form id="passkey-login-form" method="post"
                          action="/admin/login/webauthn/start" class="stack">
                        <button type="submit">{t.login_passkey_primary}</button>
                    </form>
                    <div class="divider-with-label" aria-hidden="true">
                        <span>"or"</span>
                    </div>
                })}
                <form method="post" action="/admin/login" class="stack">
                    <input type="hidden" name="next" value=next_value />
                    <div class="field">
                        <label for="username" class="field__label">{t.login_username_label}</label>
                        <input id="username" name="username" type="text"
                               required=true autocomplete="username"
                               autofocus=true />
                    </div>
                    <div class="field">
                        <label for="password" class="field__label">{t.login_password_label}</label>
                        <input id="password" name="password" type="password"
                               required=true autocomplete="current-password" />
                    </div>
                    <button type="submit">{t.login_submit}</button>
                </form>
                <p class="muted" style="margin-top:var(--space-3);text-align:center;font-size:var(--font-size-caption)">
                    <a href="/forgot-password">{t.login_forgot_password_link}</a>
                </p>
                {show_passkey_option.then(|| view! {
                    <script src="/static/webauthn.js"></script>
                })}
            </crate::layout::AuthShell>
        }
    })
}

// ---------- MFA challenge ----------

pub fn render_mfa_challenge(
    flash: Option<Flash>,
    csrf_token: String,
    has_passkey: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_totp = csrf_token.clone();
        let csrf_for_pk = csrf_token.clone();
        let passkey_block = if has_passkey {
            view! {
                <hr class="divider" />
                <p class="muted">{t.mfa_challenge_passkey_alt}</p>
                <form id="passkey-auth-form" method="post"
                      action="/admin/login/webauthn/start" class="stack">
                    <input type="hidden" name="_csrf" value=csrf_for_pk />
                    <button type="submit" class="secondary">{t.mfa_challenge_passkey_button}</button>
                </form>
                <script src="/static/webauthn.js"></script>
            }
            .into_any()
        } else {
            view! { <></> }.into_any()
        };
        view! {
            <crate::layout::AuthShell title=t.mfa_challenge_shell_title.to_string() lang=lang>
                <h1>{t.mfa_challenge_title}</h1>
                {flash_banner(flash)}
                <p class="muted">{t.mfa_challenge_lede}</p>
                <form method="post" action="/admin/login/mfa" class="stack">
                    <input type="hidden" name="_csrf" value=csrf_for_totp />
                    <div class="field">
                        <label for="code" class="field__label">{t.mfa_challenge_code_label}</label>
                        <input id="code" name="code" type="text"
                               required=true autocomplete="one-time-code"
                               inputmode="text" autofocus=true />
                    </div>
                    <button type="submit">{t.mfa_challenge_submit}</button>
                </form>
                {passkey_block}
            </crate::layout::AuthShell>
        }
    })
}

// ---------- profile (MFA settings) ----------

pub struct PasskeyDescriptor {
    pub id: String,
    pub nickname: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}


pub struct MfaSetupData {
    /// otpauth:// URI for the QR code
    pub otpauth_uri: String,
    /// Pre-rendered SVG of the QR code (full <svg>...</svg> string).
    pub qr_svg: String,
    /// Base32-encoded secret string for users who would rather type it
    /// in than scan the QR code.
    pub secret_b32: String,
}

pub fn render_mfa_setup(
    data: MfaSetupData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let MfaSetupData { otpauth_uri, qr_svg, secret_b32 } = data;
        view! {
            <Shell title=t.mfa_setup_shell_title.to_string() show_nav=true current=Some("me".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.mfa_setup_title}</h1>
                        <p class="page-header__lede">{t.mfa_setup_lede}</p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card">
                    <h3 class="card__title">{t.mfa_setup_steps_title}</h3>
                    <ol>
                        <li>{t.mfa_setup_step1}</li>
                        <li>{t.mfa_setup_step2}</li>
                        <li>{t.mfa_setup_step3}</li>
                    </ol>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.mfa_setup_qr_card_title}</h3>
                    <div inner_html=qr_svg style="max-width:240px;margin-bottom:var(--space-3)"></div>
                    <p>{t.mfa_setup_secret_label}<span class="code" style="margin-left:var(--space-1)">{secret_b32}</span></p>
                    <details>
                        <summary class="muted">{t.mfa_setup_otpauth_summary}</summary>
                        <p><span class="code">{otpauth_uri}</span></p>
                    </details>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.mfa_setup_verify_card_title}</h3>
                    <form method="post" action="/me/security/mfa/enroll/confirm" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />
                        <div class="field">
                            <label for="code" class="field__label">{t.mfa_setup_code_label}</label>
                            <input id="code" name="code" type="text" required=true
                                   autocomplete="one-time-code" inputmode="text" autofocus=true />
                            <span class="field__hint">{t.mfa_setup_code_hint}</span>
                        </div>
                        <div>
                            <button type="submit">{t.mfa_setup_confirm_button}</button>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}

// ---------- dashboard ----------

/// One bucket of the login-activity sparkline as the renderer
/// wants it: pre-formatted display label, plus the two raw counts.
/// The renderer doesn't need to know the bucket spacing or the
/// range — that's the caller's job.
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

pub struct DashboardData {
    pub admin_username: String,
    pub user_count: usize,
    pub client_count: usize,
    pub active_session_count: usize,
    pub issuer: String,
    pub sparkline: DashboardSparkline,
    // Operator action prompts — shown when condition is true (RFC 031)
    pub warn_smtp_not_configured: bool,
    pub warn_hibp_off: bool,
    pub warn_cookie_insecure: bool,
    // RFC 043: last N important audit events shown on dashboard
    pub recent_important: Vec<DashboardEventRow>,
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
fn render_sparkline(t: &'static sui_id_i18n::Strings, buckets: Vec<DashboardSparkBucket>) -> impl IntoView {
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
             style="width:100%;height:80px;display:block">
            <line x1="0" y1=format!("{:.2}", HEIGHT - PAD_BOTTOM)
                  x2=format!("{WIDTH}") y2=format!("{:.2}", HEIGHT - PAD_BOTTOM)
                  stroke="var(--border-muted)"
                  stroke-width="1" />
            {bars}
        </svg>
    }
}

pub fn render_dashboard(data: DashboardData, flash: Option<Flash>, dev_mode: bool, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let DashboardData {
            admin_username,
            user_count,
            client_count,
            active_session_count,
            issuer,
            sparkline,
            warn_smtp_not_configured,
            warn_hibp_off,
            warn_cookie_insecure,
            recent_important,
        } = data;

        let active_range = sparkline.active_range_query.clone();
        let range_tabs: Vec<_> = sparkline
            .range_options
            .iter()
            .map(|(q, label)| {
                let href = format!("/admin?range={q}");
                let aria = if *q == active_range { Some("page") } else { None };
                view! {
                    <a class="app-nav__link" href=href aria-current=aria>{label.clone()}</a>
                }
            })
            .collect();

        let total_success = sparkline.total_success;
        let total_failure = sparkline.total_failure;
        let svg = render_sparkline(t, sparkline.buckets);

        view! {
            <Shell title=t.dashboard_title.to_string() show_nav=true current=Some("dashboard".to_string()) dev_mode=dev_mode lang=lang>
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

                {(warn_smtp_not_configured || warn_hibp_off || warn_cookie_insecure).then(|| view! {
                    <section class="card" style="border-left:4px solid var(--warning-default);margin-bottom:var(--space-4)" aria-label=t.dashboard_aria_action_required>
                        <h2 style="font-size:var(--font-size-body);margin:0 0 var(--space-2)">
                            "⚠ " {t.dashboard_action_required_title}
                        </h2>
                        <ul style="margin:0;padding-left:var(--space-4)">
                            {warn_smtp_not_configured.then(|| view! { <li>{t.dashboard_warn_smtp}</li> })}
                            {warn_hibp_off.then(|| view! { <li>{t.dashboard_warn_hibp}</li> })}
                            {warn_cookie_insecure.then(|| view! { <li>{t.dashboard_warn_cookie_insecure}</li> })}
                        </ul>
                    </section>
                })}

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

                <section>
                    <div class="row" style="justify-content:space-between;align-items:flex-end;margin-bottom:var(--space-3)">
                        <h2 style="margin:0">{t.dashboard_activity_title}</h2>
                        <nav class="app-nav" aria-label=t.dashboard_activity_period style="flex:0 0 auto">
                            {range_tabs}
                        </nav>
                    </div>
                    <div class="card">
                        <div class="row" style="gap:var(--space-5);margin-bottom:var(--space-3)">
                            <div class="stat">
                                <span class="stat__value" style="color:var(--accent-default)">
                                    {total_success.to_string()}
                                </span>
                                <span class="stat__label">{t.dashboard_activity_success}</span>
                            </div>
                            <div class="stat">
                                <span class="stat__value" style="color:var(--danger-default)">
                                    {total_failure.to_string()}
                                </span>
                                <span class="stat__label">{t.dashboard_activity_failure}</span>
                            </div>
                        </div>
                        {svg}
                        <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                            {t.dashboard_activity_hover_hint}
                        </p>
                    </div>
                </section>

                <section>
                    <h2>{t.dashboard_oidc_endpoints_section}</h2>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                <tr>
                                    <th scope="row">{t.dashboard_oidc_endpoint_issuer}</th>
                                    <td><span class="code">{issuer}</span></td>
                                </tr>
                                <tr>
                                    <th scope="row">{t.dashboard_oidc_endpoint_discovery}</th>
                                    <td><a href="/.well-known/openid-configuration"><span class="code">"/.well-known/openid-configuration"</span></a></td>
                                </tr>
                                <tr>
                                    <th scope="row">{t.dashboard_oidc_endpoint_jwks}</th>
                                    <td>
                                        <a href="/.well-known/jwks.json"><span class="code">"/.well-known/jwks.json"</span></a>
                                        {copy_btn(t, "/.well-known/jwks.json".to_string(), t.copy_noun_jwks_uri)}
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </section>

                // RFC 043 — Recent important events card
                <section class="card">
                    <h2 class="card__title">{t.dashboard_recent_events_title}</h2>
                    {if recent_important.is_empty() {
                        view! { <p class="muted">{t.dashboard_recent_events_empty}</p> }.into_any()
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
                            <p class="card__footer" style="margin-top:var(--space-2)">
                                <a href="/admin/audit">{t.dashboard_recent_events_view_all}</a>
                            </p>
                            </>
                        }.into_any()
                    }}
                </section>
            </Shell>
        }
    })
}

// ---------- users ----------

fn user_row_view(
    t: &'static sui_id_i18n::Strings,
    u: UserSummary,
    current_user: String,
    csrf: String,
) -> impl IntoView {
    let display = u.display_name.clone().unwrap_or_default();
    let id_str = u.id.to_string();
    let is_self = u.username == current_user;
    let is_disabled = u.is_disabled;
    let is_deleted = u.is_deleted;
    let is_admin = u.is_admin;
    let mfa_enabled = u.mfa_enabled;
    let action_label = if is_disabled { "Enable" } else { "Disable" };
    let action_target = if is_disabled { "false" } else { "true" };
    let disabled_url = format!("/admin/users/{id_str}/disabled");
    let delete_url = format!("/admin/users/{id_str}/delete");
    let reset_mfa_url = format!("/admin/users/{id_str}/mfa-reset");
    let csrf_disable = csrf.clone();
    let csrf_delete = csrf.clone();
    let csrf_reset = csrf.clone();

    let status_view = if is_deleted {
        crate::components::status_badge(t, crate::components::StatusKind::Deleted).into_any()
    } else if is_disabled {
        crate::components::status_badge(t, crate::components::StatusKind::Disabled).into_any()
    } else if is_admin {
        crate::components::status_badge(t, crate::components::StatusKind::Admin).into_any()
    } else {
        crate::components::status_badge(t, crate::components::StatusKind::Active).into_any()
    };

    let mfa_cell = if mfa_enabled {
        view! { <td>{crate::components::status_badge(t, crate::components::StatusKind::On)}</td> }.into_any()
    } else {
        view! { <td><span class="muted">{t.status_off}</span></td> }.into_any()
    };

    let actions = if is_self {
        view! { <td><span class="muted">"(you)"</span></td> }.into_any()
    } else if is_deleted {
        view! { <td><span class="muted">{t.empty_dash}</span></td> }.into_any()
    } else {
        let disable_confirm_url = format!("/admin/users/{id_str}/disable-confirm");
        let delete_confirm_url = format!("/admin/users/{id_str}/delete-confirm");
        let reset_mfa_confirm_url = format!("/admin/users/{id_str}/mfa-reset-confirm");
        let reset_link = if mfa_enabled {
            view! {
                <a href=reset_mfa_confirm_url class="button secondary">"Reset MFA"</a>
                " "
            }
            .into_any()
        } else {
            view! { <></> }.into_any()
        };
        view! {
            <td>
                <div class="row" style="gap:var(--space-1)">
                    {reset_link}
                    <a href=disable_confirm_url class="button secondary">{action_label}</a>
                    " "
                    <a href=delete_confirm_url class="button danger">"Delete"</a>
                </div>
            </td>
        }
        .into_any()
    };

    view! {
        <tr>
            <td><span class="code">{u.username}</span></td>
            <td>{display}</td>
            <td>{status_view}</td>
            {mfa_cell}
            <td class="muted">{fmt_time(u.created_at)}</td>
            {actions}
        </tr>
    }
}

pub fn render_users(
    users: Vec<UserSummary>,
    flash: Option<Flash>,
    current_user: String,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_rows = csrf_token.clone();
        let csrf_for_form = csrf_token.clone();
        let user_count = users.len();
        let rows: Vec<_> = users
            .into_iter()
            .map(|u| user_row_view(t, u, current_user.clone(), csrf_for_rows.clone()))
            .collect();
        view! {
            <Shell title=t.users_title.to_string() show_nav=true current=Some("users".to_string()) dev_mode=dev_mode lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.users_title}</h1>
                        <p class="page-header__lede">
                            {t.users_lede}
                            " "
                            {(t.users_count_caption)(user_count)}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                <section>
                    <h2>{t.users_create_section}</h2>
                    <div class="card">
                        <form method="post" action="/admin/users" class="stack">
                            <input type="hidden" name="_csrf" value=csrf_for_form />
                            <div class="field">
                                <label for="u-name" class="field__label">{t.users_label_username}</label>
                                <input id="u-name" name="username" type="text"
                                       required=true autocomplete="off" />
                            </div>
                            <div class="field">
                                <label for="u-disp" class="field__label">{t.users_label_display_name}</label>
                                <input id="u-disp" name="display_name" type="text" autocomplete="off" />
                            </div>
                            <div class="field">
                                <label for="u-email" class="field__label">{t.users_label_email}</label>
                                <input id="u-email" name="email" type="email" autocomplete="off" />
                            </div>
                            <div class="field">
                                <label for="u-pw" class="field__label">{t.users_label_password}</label>
                                <input id="u-pw" name="password" type="password"
                                       required=true minlength="12" autocomplete="new-password" />
                            </div>
                            <label class="row" style="gap:var(--space-2)">
                                <input name="is_admin" type="checkbox" value="true" />
                                <span>{t.users_is_admin_label}</span>
                            </label>
                            <div>
                                <button type="submit">{t.users_create_button}</button>
                            </div>
                        </form>
                    </div>
                </section>

                <section>
                    <h2>{t.users_table_section}</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.login_username_label}</th>
                                    <th>{t.users_table_th_display}</th>
                                    <th>{t.users_table_th_status}</th>
                                    <th>{t.users_table_th_mfa}</th>
                                    <th>{t.users_table_th_created}</th>
                                    <th></th>
                                </tr>
                            </thead>
                            {if rows.is_empty() {
                                view! {
                                    <tbody><tr><td colspan="6" class="muted" style="text-align:center;padding:var(--space-6) 0">
                                        {t.users_empty}
                                    </td></tr></tbody>
                                }.into_any()
                            } else {
                                view! { <tbody>{rows}</tbody> }.into_any()
                            }}
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}

// ---------- clients ----------

fn client_row_view(
    t: &'static sui_id_i18n::Strings,
    c: ClientSummary,
    csrf: String,
) -> impl IntoView {
    let is_disabled = c.is_disabled;
    let is_deleted = c.is_deleted;
    let kind = if c.confidential { "confidential" } else { "public" };
    let id_str = c.id.to_string();
    let action_label = if is_disabled { "Enable" } else { "Disable" };
    let action_target = if is_disabled { "false" } else { "true" };
    let disabled_url = format!("/admin/clients/{id_str}/disabled");
    let delete_url = format!("/admin/clients/{id_str}/delete");
    let csrf_disable = csrf.clone();
    let csrf_delete = csrf.clone();
    let scopes_display = if c.allowed_scopes.trim().is_empty() {
        t.empty_any.to_string()
    } else {
        c.allowed_scopes.clone()
    };
    let logout_count = c.post_logout_redirect_uris.len();
    let logout_display = if logout_count == 0 {
        t.empty_falls_back_redirect_uris.to_string()
    } else {
        format!("{logout_count} URI(s)")
    };

    let status_view = if is_deleted {
        crate::components::status_badge(t, crate::components::StatusKind::Deleted).into_any()
    } else if is_disabled {
        crate::components::status_badge(t, crate::components::StatusKind::Disabled).into_any()
    } else {
        crate::components::status_badge(t, crate::components::StatusKind::Active).into_any()
    };

    let edit_url = format!("/admin/clients/{id_str}/edit");
    let actions = if is_deleted {
        view! { <td><span class="muted">{t.empty_dash}</span></td> }.into_any()
    } else {
        view! {
            <td>
                <div class="row" style="gap:var(--space-1)">
                    <a href=edit_url class="button secondary">"Edit"</a>
                    <form method="post" action=disabled_url style="display:inline">
                        <input type="hidden" name="_csrf" value=csrf_disable />
                        <input type="hidden" name="disabled" value=action_target />
                        <button type="submit" class="secondary">{action_label}</button>
                    </form>
                    <a href=format!("/admin/clients/{}/delete-confirm", id_str.clone()) class="button danger">"Delete"</a>
                </div>
            </td>
        }
        .into_any()
    };

    let id_for_copy = id_str.clone();
    view! {
        <tr>
            <td>{c.name}</td>
            <td>
                <span class="code">{id_str}</span>
                {copy_btn(t, id_for_copy, t.copy_noun_client_id)}
            </td>
            <td>{kind}</td>
            <td><span class="code">{scopes_display}</span></td>
            <td class="muted">{logout_display}</td>
            <td>{status_view}</td>
            {actions}
        </tr>
    }
}

pub fn render_clients(
    clients: Vec<ClientSummary>,
    flash: Option<Flash>,
    new_secret: Option<(String, String)>,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_rows = csrf_token.clone();
        let csrf_for_form = csrf_token.clone();
        let client_count = clients.len();
        let secret_block = new_secret.map(|(cid, sec)| {
            view! {
                <div class="flash warn" role="status">
                    <div class="stack-tight">
                        <strong>{t.clients_secret_once_banner}</strong>
                        <div>"Client ID: "<span class="code">{cid.clone()}</span>{copy_btn(t, cid, t.copy_noun_client_id)}</div>
                        <div>"Client Secret: "<span class="code">{sec.clone()}</span>{copy_btn(t, sec, t.copy_noun_client_secret)}</div>
                    </div>
                </div>
            }
        });
        let rows: Vec<_> = clients
            .into_iter()
            .map(|c| client_row_view(t, c, csrf_for_rows.clone()))
            .collect();
        view! {
            <Shell title=t.clients_title.to_string() show_nav=true current=Some("clients".to_string()) dev_mode=dev_mode lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.clients_title}</h1>
                        <p class="page-header__lede">
                            {t.clients_lede}
                            " "
                            {(t.clients_count_caption)(client_count)}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {secret_block}

                <section>
                    <h2>{t.clients_create_section}</h2>
                    <div class="card">
                        <form method="post" action="/admin/clients" class="stack">
                            <input type="hidden" name="_csrf" value=csrf_for_form />
                            <div class="field">
                                <label for="c-name" class="field__label">{t.clients_label_app_name}</label>
                                <input id="c-name" name="name" type="text" required=true />
                            </div>
                            <div class="field">
                                <label for="c-uris" class="field__label">{t.clients_label_redirect_uris}</label>
                                <textarea id="c-uris" name="redirect_uris" required=true rows="3"></textarea>
                                <span class="field__hint">{t.clients_hint_redirect_uris}</span>
                            </div>
                            <div class="field">
                                <label for="c-scopes" class="field__label">{t.clients_label_allowed_scopes}</label>
                                <input id="c-scopes" name="allowed_scopes" type="text" value="openid profile email" />
                                <span class="field__hint">
                                    {t.clients_hint_scopes_intro}
                                    <code>"openid"</code>{t.clients_hint_scopes_openid_note}
                                    <code>"profile"</code>{t.clients_hint_scopes_profile_note}
                                    <code>"email"</code>{t.clients_hint_scopes_email_note}
                                    <code>"offline_access"</code>{t.clients_hint_scopes_offline_note}
                                    {t.clients_hint_scopes_default}
                                </span>
                            </div>
                            // Single-realm note (RFC 027) — now via clients_single_realm_note key
                            <p class="field__hint" style="margin: 0;">
                                "ℹ  "
                                {t.clients_single_realm_note}
                            </p>
                            <div class="field">
                                <label for="c-logout" class="field__label">{t.clients_label_post_logout_uris}</label>
                                <textarea id="c-logout" name="post_logout_redirect_uris" rows="2"></textarea>
                                <span class="field__hint">{t.clients_hint_one_per_line}</span>
                            </div>
                            <label class="row" style="gap:var(--space-2)">
                                <input name="confidential" type="checkbox" value="true" checked=true />
                                <span>{t.clients_label_confidential_checkbox}</span>
                            </label>
                            <div>
                                <button type="submit">{t.clients_button_register}</button>
                            </div>
                        </form>
                    </div>
                </section>

                <section>
                    <h2>{t.clients_table_section}</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.clients_table_th_name}</th>
                                    <th>{t.clients_table_th_client_id}</th>
                                    <th>{t.clients_table_th_kind}</th>
                                    <th>{t.clients_table_th_scopes}</th>
                                    <th>{t.clients_table_th_logout}</th>
                                    <th>{t.clients_table_th_status}</th>
                                    <th></th>
                                </tr>
                            </thead>
                            {if rows.is_empty() {
                                view! {
                                    <tbody><tr><td colspan="7" class="muted" style="text-align:center;padding:var(--space-6) 0">
                                        {t.clients_empty}
                                    </td></tr></tbody>
                                }.into_any()
                            } else {
                                view! { <tbody>{rows}</tbody> }.into_any()
                            }}
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}

// ---------- client edit ----------

pub struct ClientEditData {
    pub id: String,
    pub name: String,
    /// Newline-separated for textarea editing.
    pub redirect_uris: Vec<String>,
    /// Space-separated.
    pub allowed_scopes: String,
    pub post_logout_redirect_uris: Vec<String>,
    pub confidential: bool,
    pub is_disabled: bool,
    /// RFC 038: "none", "first_time", or "always"
    pub consent_policy: String,
    /// RFC 047: populated only when the secret was just rotated. Shown once.
    pub freshly_rotated_secret: Option<String>,
}

pub fn render_client_edit(
    data: ClientEditData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let ClientEditData {
            id,
            name,
            redirect_uris,
            allowed_scopes,
            post_logout_redirect_uris,
            confidential,
            is_disabled,
            consent_policy,
            freshly_rotated_secret,
        } = data;
        let post_url = format!("/admin/clients/{id}/edit");
        let kind = if confidential { "confidential" } else { "public" };
        let redirect_uris_value = redirect_uris.join("\n");
        let post_logout_value = post_logout_redirect_uris.join("\n");

        let status_view = if is_disabled {
            crate::components::status_badge(t, crate::components::StatusKind::Disabled).into_any()
        } else {
            crate::components::status_badge(t, crate::components::StatusKind::Active).into_any()
        };

        view! {
            <Shell title=t.client_edit_title.to_string() show_nav=true current=Some("clients".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.client_edit_title}</h1>
                        <p class="page-header__lede">{name.clone()}</p>
                    </div>
                </header>
                {flash_banner(flash)}
                {freshly_rotated_secret.map(|sec| {
                    let sec2 = sec.clone();
                    view! {
                        <div class="banner banner--warning" role="alert" style="margin-bottom:var(--space-3)">
                            <strong>{t.client_edit_new_secret_label}</strong>
                            <span class="code" style="margin-left:var(--space-2)">{sec}</span>
                            {copy_btn(t, sec2, t.copy_noun_client_secret)}
                        </div>
                    }
                })}

                <div class="card">
                    <h3 class="card__title">{t.client_edit_basic_section}</h3>
                    <div class="stack-tight muted">
                        <div>{t.client_edit_label_client_id}": "<span class="code">{id.clone()}</span>{copy_btn(t, id.clone(), t.copy_noun_client_id)}</div>
                        <div class="row" style="gap:var(--space-2)">
                            <span>{t.client_edit_label_kind}":"</span>
                            <span class="badge badge--accent">{kind}</span>
                            <span>{t.client_edit_label_status}":"</span>
                            {status_view}
                        </div>
                    </div>
                    <p class="muted" style="margin-top:var(--space-3)">
                        {t.client_edit_basic_note}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_title}</h3>
                    <form method="post" action=post_url class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />
                        <div class="field">
                            <label for="e-name" class="field__label">{t.clients_label_app_name}</label>
                            <input id="e-name" name="name" type="text" required=true value=name />
                        </div>
                        <div class="field">
                            <label for="e-uris" class="field__label">{t.clients_label_redirect_uris}</label>
                            <textarea id="e-uris" name="redirect_uris" required=true rows="3">
                                {redirect_uris_value}
                            </textarea>
                            <span class="field__hint">{t.clients_hint_redirect_uris}</span>
                        </div>
                        <div class="field">
                            <label for="e-scopes" class="field__label">{t.clients_label_allowed_scopes}</label>
                            <input id="e-scopes" name="allowed_scopes" type="text" value=allowed_scopes />
                            <span class="field__hint">
                                {t.clients_hint_scopes_intro}
                                <code>"openid"</code>" · "
                                <code>"profile"</code>" · "
                                <code>"email"</code>" · "
                                <code>"offline_access"</code>"。"
                            </span>
                        </div>
                        <div class="field">
                            <label for="e-logout" class="field__label">{t.clients_label_post_logout_uris}</label>
                            <textarea id="e-logout" name="post_logout_redirect_uris" rows="2">
                                {post_logout_value}
                            </textarea>
                            <span class="field__hint">{t.client_edit_post_logout_hint}</span>
                        </div>
                        <div class="field">
                            <label for="e-consent" class="field__label">{t.consent_policy_label}</label>
                            <select id="e-consent" name="consent_policy">
                                {
                                    let cp = consent_policy.clone();
                                    let cp2 = consent_policy.clone();
                                    let cp3 = consent_policy.clone();
                                    view! {
                                        <>
                                        <option value="none"     selected=move || cp  == "none">
                                            {t.consent_policy_none}
                                        </option>
                                        <option value="first_time" selected=move || cp2 == "first_time">
                                            {t.consent_policy_first_time}
                                        </option>
                                        <option value="always"   selected=move || cp3 == "always">
                                            {t.consent_policy_always}
                                        </option>
                                        </>
                                    }
                                }
                            </select>
                        </div>
                        <div class="row">
                            <button type="submit">{t.button_save}</button>
                            <a href="/admin/clients" class="button secondary">{t.button_cancel}</a>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}

// ---------- audit ----------

fn audit_row_view(t: &'static sui_id_i18n::Strings, e: AuditLogEntryDto) -> impl IntoView {
    let result_badge = match e.result.as_str() {
        "ok" => view! { <span class="badge badge--ok">"ok"</span> }.into_any(),
        "fail" | "error" | "denied" => {
            view! { <span class="badge badge--danger">{e.result.clone()}</span> }.into_any()
        }
        _ => view! { <span class="badge">{e.result.clone()}</span> }.into_any(),
    };
    // RFC 046: stable copyable row identifier — time|actor|action|target
    let row_id = format!(
        "{}|{}|{}|{}",
        e.at.format("%Y-%m-%dT%H:%M:%SZ"),
        e.actor.map(|a| a.to_string()).unwrap_or_else(|| "-".into()),
        e.action,
        e.target.clone().unwrap_or_default(),
    );
    let actor_str = e.actor.map(|a| a.to_string()).unwrap_or_else(|| "-".into());
    view! {
        <tr>
            <td class="muted">{fmt_time(e.at)}</td>
            <td><span class="code">{actor_str}</span></td>
            <td>{e.action}</td>
            <td><span class="code">{e.target.unwrap_or_default()}</span></td>
            <td>{result_badge}</td>
            <td>{copy_btn(t, row_id, t.copy_noun_audit_row_id)}</td>
        </tr>
    }
}

pub fn render_audit(
    entries: Vec<AuditLogEntryDto>,
    chain_ok: bool,
    filter_query: Option<String>,
    flash: Option<Flash>,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let entry_count = entries.len();
        let fq = filter_query.clone().unwrap_or_default();
        let csv_href = if fq.is_empty() {
            "/admin/audit.csv".to_string()
        } else {
            format!("/admin/audit.csv?q={}", url_encode(&fq))
        };
        let fq_display = fq.clone();
        let rows: Vec<_> = entries.into_iter().map(|e| audit_row_view(t, e)).collect();
        let chain_banner_view = if chain_ok {
            view! {
                <p class="badge badge--ok" style="margin-bottom:var(--space-3)">
                    "✓ " {t.audit_chain_ok}
                </p>
            }.into_any()
        } else {
            view! {
                <p class="badge badge--danger" style="margin-bottom:var(--space-3)">
                    "✗ " {t.audit_chain_broken}
                </p>
            }.into_any()
        };
        view! {
            <Shell title=t.audit_title.to_string() show_nav=true current=Some("audit".to_string()) dev_mode=dev_mode lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.audit_title}</h1>
                        <p class="page-header__lede">
                            {t.audit_lede}
                            " "
                            {(t.audit_entry_count_caption)(entry_count)}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {chain_banner_view}
                <div class="row" style="gap:var(--space-3);margin-bottom:var(--space-3);align-items:flex-end;flex-wrap:wrap">
                    <form method="get" action="/admin/audit" class="row" style="gap:var(--space-2);align-items:center">
                        <label for="audit-q" style="font-weight:500">{t.audit_filter_label}</label>
                        <input id="audit-q" name="q" type="search"
                               placeholder=t.audit_filter_placeholder
                               value=fq_display
                               style="min-width:16rem" />
                        <button type="submit" class="secondary">{t.audit_filter_button}</button>
                    </form>
                    <a href=csv_href class="button secondary">{t.audit_export_csv}</a>
                </div>
                <div class="table-wrap">
                    <table>
                        <thead>
                            <tr>
                                <th>{t.audit_col_when}</th>
                                <th>{t.audit_col_actor}</th>
                                <th>{t.audit_col_action}</th>
                                <th>{t.audit_col_target}</th>
                                <th>{t.audit_col_outcome}</th>
                            </tr>
                        </thead>
                        {if rows.is_empty() {
                            view! {
                                <tbody><tr><td colspan="5" class="muted"
                                    style="text-align:center;padding:var(--space-6) 0">
                                    "(no matching entries)"
                                </td></tr></tbody>
                            }.into_any()
                        } else {
                            view! { <tbody>{rows}</tbody> }.into_any()
                        }}
                    </table>
                </div>
            </Shell>
        }
    })
}

fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

// ---------- signing keys ----------

fn signing_key_row_view(
    k: sui_id_shared::api::SigningKeySummary,
    csrf: String,
    t: &'static sui_id_i18n::Strings,
) -> impl IntoView {
    let id_str = k.id.to_string();
    let id_for_url = id_str.clone();
    let id_for_display = id_str.clone();
    let id_for_confirm = id_str.clone();
    let status_view = if k.is_active {
        crate::components::status_badge(t, crate::components::StatusKind::InUse).into_any()
    } else {
        crate::components::status_badge(t, crate::components::StatusKind::Retired).into_any()
    };
    let rotated = k
        .rotated_at
        .map(fmt_time)
        .unwrap_or_else(|| t.empty_dash.to_string());
    let delete_url = format!("/admin/signing-keys/{id_for_url}/delete");
    let actions = if k.is_active {
        view! { <td><span class="muted">{t.signing_keys_in_use_badge}</span></td> }.into_any()
    } else {
        view! {
            <td>
                <a href=format!("/admin/signing-keys/{}/delete-confirm", id_for_confirm) class="button danger">{t.button_delete}</a>
            </td>
        }
        .into_any()
    };
    view! {
        <tr>
            <td><span class="code">{id_for_display}</span></td>
            <td>{k.algorithm}</td>
            <td>{status_view}</td>
            <td class="muted">{fmt_time(k.created_at)}</td>
            <td class="muted">{rotated}</td>
            {actions}
        </tr>
    }
}

pub fn render_signing_keys(
    keys: Vec<sui_id_shared::api::SigningKeySummary>,
    flash: Option<Flash>,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_rows = csrf_token.clone();
        let csrf_for_form = csrf_token.clone();
        let key_count = keys.len();
        let rows: Vec<_> = keys
            .into_iter()
            .map(|k| signing_key_row_view(k, csrf_for_rows.clone(), t))
            .collect();
        view! {
            <Shell
                title=t.signing_keys_title.to_string()
                show_nav=true
                current=Some("signing-keys".to_string()) lang=lang
            >
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.signing_keys_title}</h1>
                        <p class="page-header__lede">
                            {t.signing_keys_lede}
                            " "
                            {(t.signing_keys_count_caption)(key_count)}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card">
                    <h3 class="card__title">{t.signing_keys_rotate_section}</h3>
                    <p class="muted">
                        {t.signing_keys_rotate_explanation_1}
                        " "
                        {t.signing_keys_rotate_explanation_2}
                        " "
                        {t.signing_keys_rotate_explanation_3}
                    </p>
                    <div class="card__footer">
                        <form method="post" action="/admin/signing-keys/rotate">
                            <input type="hidden" name="_csrf" value=csrf_for_form />
                            <button type="submit">{t.signing_keys_rotate_button}</button>
                        </form>
                    </div>
                </div>

                <section>
                    <h2>{t.signing_keys_table_section}</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.signing_keys_th_key_id}</th>
                                    <th>{t.signing_keys_th_algorithm}</th>
                                    <th>{t.signing_keys_th_status}</th>
                                    <th>{t.signing_keys_th_created}</th>
                                    <th>{t.signing_keys_th_retired}</th>
                                    <th></th>
                                </tr>
                            </thead>
                            {if rows.is_empty() {
                                view! {
                                    <tbody><tr><td colspan="6" class="muted" style="text-align:center;padding:var(--space-6) 0">
                                        {t.signing_keys_empty}
                                    </td></tr></tbody>
                                }.into_any()
                            } else {
                                view! { <tbody>{rows}</tbody> }.into_any()
                            }}
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}


// ---------- dangerous-operation confirmation screens (RFC 030) ----------

/// Render a reversibility badge: green "Recoverable" or red "Not recoverable".
/// Colour is NEVER the only signal (RFC 017 § 3).
fn reversibility_badge(recoverable: bool, t: &'static sui_id_i18n::Strings) -> impl IntoView {
    if recoverable {
        view! {
            <span class="reversibility-badge reversibility-badge--recoverable">
                "✓ " {t.badge_recoverable}
            </span>
        }.into_any()
    } else {
        view! {
            <span class="reversibility-badge reversibility-badge--permanent">
                "✗ " {t.badge_not_recoverable}
            </span>
        }.into_any()
    }
}

pub struct ConfirmDisableData {
    pub user_id: String,
    pub username: String,
    pub is_disabled: bool,
    pub csrf_token: String,
}

pub fn render_confirm_disable_user(
    data: ConfirmDisableData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let action = format!("/admin/users/{}/disabled", data.user_id);
        let new_state = if data.is_disabled { "false" } else { "true" };
        let (title, impact, rev, btn) = if data.is_disabled {
            (t.confirm_enable_title, "", "", t.confirm_enable_button)
        } else {
            (t.confirm_disable_title, t.confirm_disable_impact,
             t.confirm_disable_reversibility, t.confirm_disable_button)
        };
        let badge = reversibility_badge(true, t);
        let username = data.username.clone();
        view! {
            <Shell title=title.to_string() show_nav=true
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang>
                <div class="auth-card" style="max-width:36rem">
                    <h1>{title}</h1>
                    <p><strong>{username.clone()}</strong></p>
                    {(!data.is_disabled).then(|| view! {
                        <p class="muted">{impact}</p>
                        <p>{badge}</p>
                        <p class="muted" style="font-size:var(--font-size-caption)">{rev}</p>
                    })}
                    <form method="post" action=action class="stack" style="margin-top:var(--space-4)">
                        <input type="hidden" name="_csrf" value=data.csrf_token />
                        <input type="hidden" name="disabled" value=new_state />
                        <input type="hidden" name="_confirmed" value="1" />
                        {(!data.is_disabled).then(|| view! {
                            <div class="field">
                                <label for="disable-reason" class="field__label">
                                    {t.disable_reason_label}
                                </label>
                                <textarea id="disable-reason" name="reason" rows="2" maxlength="200"
                                          placeholder=t.disable_reason_placeholder></textarea>
                                <span class="field__hint">{t.disable_reason_hint}</span>
                            </div>
                        })}
                        <div class="row" style="gap:var(--space-2)">
                            <button type="submit" class={if data.is_disabled {"btn"} else {"danger"}}>
                                {btn}
                            </button>
                            <a href="/admin/users" class="button secondary">{t.confirm_cancel}</a>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}

pub struct ConfirmDeleteUserData {
    pub user_id: String,
    pub username: String,
    pub csrf_token: String,
}

pub fn render_confirm_delete_user(
    data: ConfirmDeleteUserData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let action = format!("/admin/users/{}/delete", data.user_id);
        let badge = reversibility_badge(false, t);
        let username = data.username.clone();
        view! {
            <Shell title=t.confirm_delete_user_title.to_string() show_nav=true
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang>
                <div class="auth-card" style="max-width:36rem">
                    <h1>{t.confirm_delete_user_title}</h1>
                    <p><strong>{username}</strong></p>
                    <p class="muted">{t.confirm_delete_user_impact}</p>
                    <p>{badge}</p>
                    <p class="muted" style="font-size:var(--font-size-caption)">
                        {t.confirm_delete_user_reversibility}
                    </p>
                    <form method="post" action=action class="row" style="gap:var(--space-2);margin-top:var(--space-4)">
                        <input type="hidden" name="_csrf" value=data.csrf_token />
                        <input type="hidden" name="_confirmed" value="1" />
                        <button type="submit" class="danger">{t.confirm_delete_user_button}</button>
                        <a href="/admin/users" class="button secondary">{t.confirm_cancel}</a>
                    </form>
                </div>
            </Shell>
        }
    })
}

pub struct ConfirmResetMfaData {
    pub user_id: String,
    pub username: String,
    pub csrf_token: String,
}

pub fn render_confirm_reset_mfa(
    data: ConfirmResetMfaData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let action = format!("/admin/users/{}/mfa-reset", data.user_id);
        let badge = reversibility_badge(true, t);
        let username = data.username.clone();
        view! {
            <Shell title=t.confirm_reset_mfa_title.to_string() show_nav=true
                   current=Some("users".to_string()) dev_mode=dev_mode lang=lang>
                <div class="auth-card" style="max-width:36rem">
                    <h1>{t.confirm_reset_mfa_title}</h1>
                    <p><strong>{username}</strong></p>
                    <p class="muted">{t.confirm_reset_mfa_impact}</p>
                    <p>{badge}</p>
                    <p class="muted" style="font-size:var(--font-size-caption)">
                        {t.confirm_reset_mfa_reversibility}
                    </p>
                    <form method="post" action=action class="row" style="gap:var(--space-2);margin-top:var(--space-4)">
                        <input type="hidden" name="_csrf" value=data.csrf_token />
                        <input type="hidden" name="_confirmed" value="1" />
                        <button type="submit" class="danger">{t.confirm_reset_mfa_button}</button>
                        <a href="/admin/users" class="button secondary">{t.confirm_cancel}</a>
                    </form>
                </div>
            </Shell>
        }
    })
}

pub struct ConfirmDeleteClientData {
    pub client_id: String,
    pub client_name: String,
    pub csrf_token: String,
}

pub fn render_confirm_delete_client(
    data: ConfirmDeleteClientData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let action = format!("/admin/clients/{}/delete", data.client_id);
        let badge = reversibility_badge(false, t);
        let name = data.client_name.clone();
        view! {
            <Shell title=t.confirm_delete_client_title.to_string() show_nav=true
                   current=Some("clients".to_string()) dev_mode=dev_mode lang=lang>
                <div class="auth-card" style="max-width:36rem">
                    <h1>{t.confirm_delete_client_title}</h1>
                    <p><strong>{name}</strong></p>
                    <p class="muted">{t.confirm_delete_client_impact}</p>
                    <p>{badge}</p>
                    <p class="muted" style="font-size:var(--font-size-caption)">
                        {t.confirm_delete_client_reversibility}
                    </p>
                    <form method="post" action=action class="row" style="gap:var(--space-2);margin-top:var(--space-4)">
                        <input type="hidden" name="_csrf" value=data.csrf_token />
                        <input type="hidden" name="_confirmed" value="1" />
                        <button type="submit" class="danger">{t.confirm_delete_client_button}</button>
                        <a href="/admin/clients" class="button secondary">{t.confirm_cancel}</a>
                    </form>
                </div>
            </Shell>
        }
    })
}

pub struct ConfirmDeleteSigningKeyData {
    pub key_id: String,
    pub algorithm: String,
    pub csrf_token: String,
}

pub fn render_confirm_delete_signing_key(
    data: ConfirmDeleteSigningKeyData,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let action = format!("/admin/signing-keys/{}/delete", data.key_id);
        let badge = reversibility_badge(false, t);
        let algo = data.algorithm.clone();
        let kid = data.key_id.clone();
        view! {
            <Shell title=t.confirm_delete_signing_key_title.to_string() show_nav=true
                   current=Some("signing_keys".to_string()) dev_mode=dev_mode lang=lang>
                <div class="auth-card" style="max-width:36rem">
                    <h1>{t.confirm_delete_signing_key_title}</h1>
                    <p class="muted"><span class="code">{kid}</span>" ("{algo}")"</p>
                    <p class="muted">{t.confirm_delete_signing_key_impact}</p>
                    <p>{badge}</p>
                    <p class="muted" style="font-size:var(--font-size-caption)">
                        {t.confirm_delete_signing_key_reversibility}
                    </p>
                    <form method="post" action=action class="row" style="gap:var(--space-2);margin-top:var(--space-4)">
                        <input type="hidden" name="_csrf" value=data.csrf_token />
                        <input type="hidden" name="_confirmed" value="1" />
                        <button type="submit" class="danger">{t.confirm_delete_signing_key_button}</button>
                        <a href="/admin/signing-keys" class="button secondary">{t.confirm_cancel}</a>
                    </form>
                </div>
            </Shell>
        }
    })
}


// ---------- admin user detail (RFC 035) ----------

pub struct UserDetailData {
    pub user_id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub is_admin: bool,
    pub is_disabled: bool,
    pub totp_enabled: bool,
    pub passkey_count: usize,
    pub sessions: Vec<UserDetailSession>,
    pub recent_audit: Vec<sui_id_shared::api::AuditLogEntryDto>,
    pub dev_mode: bool,
    pub csrf_token: String,
}

pub struct UserDetailSession {
    pub started: chrono::DateTime<chrono::Utc>,
    pub expires: chrono::DateTime<chrono::Utc>,
    pub factors: String,
}

pub fn render_user_detail(data: UserDetailData, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let badge = if data.is_disabled {
            crate::components::status_badge(t, crate::components::StatusKind::Disabled).into_any()
        } else if data.is_admin {
            crate::components::status_badge(t, crate::components::StatusKind::Admin).into_any()
        } else {
            crate::components::status_badge(t, crate::components::StatusKind::Active).into_any()
        };

        let display = data.display_name.clone().unwrap_or_default();
        let email = data.email.clone().unwrap_or_default();
        let username = data.username.clone();
        let uid = data.user_id.clone();
        let totp_badge = if data.totp_enabled {
            view! { <span class="badge badge--ok">{t.profile_mfa_status_enabled}</span> }.into_any()
        } else {
            view! { <span class="muted">{t.profile_mfa_status_not_configured}</span> }.into_any()
        };

        let session_rows: Vec<_> = data.sessions.iter().map(|s| {
            let started = fmt_time(s.started);
            let expires = fmt_time(s.expires);
            let factors = s.factors.clone();
            view! {
                <tr>
                    <td class="muted">{started}</td>
                    <td class="muted">{expires}</td>
                    <td>{factors}</td>
                </tr>
            }
        }).collect();

        let audit_rows: Vec<_> = data.recent_audit.iter().map(|e| {
            audit_row_view(t, e.clone())
        }).collect();

        let disable_confirm_url = format!("/admin/users/{uid}/disable-confirm");
        let delete_confirm_url  = format!("/admin/users/{uid}/delete-confirm");
        let reset_mfa_confirm_url = format!("/admin/users/{uid}/mfa-reset-confirm");

        view! {
            <Shell title=username.clone() show_nav=true
                   current=Some("users".to_string())
                   dev_mode=data.dev_mode lang=lang>
                <div style="margin-bottom:var(--space-3)">
                    <a href="/admin/users" class="muted">{t.user_detail_back}</a>
                </div>

                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">
                            <span class="code">{username.clone()}</span>
                            " " {badge}
                        </h1>
                        {(!display.is_empty()).then(|| view! {
                            <p class="page-header__lede">{display.clone()}</p>
                        })}
                        {(!email.is_empty()).then(|| view! {
                            <p class="muted" style="font-size:var(--font-size-caption)">{email}</p>
                        })}
                    </div>
                    <div class="row" style="gap:var(--space-2);align-self:flex-start">
                        {data.totp_enabled.then(|| view! {
                            <a href=reset_mfa_confirm_url.clone() class="button secondary">
                                {t.confirm_reset_mfa_button}
                            </a>
                        })}
                        <a href=disable_confirm_url class="button secondary">
                            {if data.is_disabled { t.confirm_enable_button } else { t.confirm_disable_button }}
                        </a>
                        <a href=delete_confirm_url class="button danger">
                            {t.button_delete}
                        </a>
                    </div>
                </header>

                <section class="card" style="margin-bottom:var(--space-4)">
                    <h2 class="card__title">{t.user_detail_auth_section}</h2>
                    <dl class="kv-list">
                        <div class="kv-list__row">
                            <dt>{t.user_detail_totp_label}</dt>
                            <dd>{totp_badge}</dd>
                        </div>
                        <div class="kv-list__row">
                            <dt>{t.user_detail_passkeys_label}</dt>
                            <dd>{data.passkey_count.to_string()}</dd>
                        </div>
                    </dl>
                </section>

                <section style="margin-bottom:var(--space-4)">
                    <h2>{t.user_detail_sessions_section}</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.user_detail_sessions_th_started}</th>
                                    <th>{t.user_detail_sessions_th_expires}</th>
                                    <th>{t.user_detail_sessions_th_factors}</th>
                                </tr>
                            </thead>
                            {if session_rows.is_empty() {
                                view! {
                                    <tbody><tr><td colspan="3" class="muted"
                                        style="text-align:center;padding:var(--space-4) 0">
                                        {t.muted_none}
                                    </td></tr></tbody>
                                }.into_any()
                            } else {
                                view! { <tbody>{session_rows}</tbody> }.into_any()
                            }}
                        </table>
                    </div>
                </section>

                <section>
                    <h2>{t.user_detail_activity_section}</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.audit_col_when}</th>
                                    <th>{t.audit_col_action}</th>
                                    <th>{t.audit_col_outcome}</th>
                                </tr>
                            </thead>
                            {if audit_rows.is_empty() {
                                view! {
                                    <tbody><tr><td colspan="3" class="muted"
                                        style="text-align:center;padding:var(--space-4) 0">
                                        {t.muted_none}
                                    </td></tr></tbody>
                                }.into_any()
                            } else {
                                view! { <tbody>{audit_rows}</tbody> }.into_any()
                            }}
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}


// ---------- OIDC consent screen (RFC 038) ----------

pub struct ConsentData {
    pub client_name: String,
    pub requested_scopes: Vec<String>,
    pub csrf_token: String,
}

pub fn render_consent(data: ConsentData, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let client_name = data.client_name.clone();
        let scope_labels: Vec<_> = data.requested_scopes.iter().map(|s| {
            let label: &'static str = match s.as_str() {
                "openid"         => t.consent_scope_openid,
                "profile"        => t.consent_scope_profile,
                "email"          => t.consent_scope_email,
                "offline_access" => t.consent_scope_offline_access,
                _                => "—",
            };
            let scope_str = s.clone();
            view! {
                <li style="margin:var(--space-1) 0">
                    <span class="badge">{scope_str}</span>
                    " — " {label}
                </li>
            }
        }).collect();

        let csrf = data.csrf_token.clone();
        view! {
            <crate::layout::AuthShell title=t.consent_title.to_string() lang=lang>
                <div class="auth-card" style="max-width:32rem">
                    <h1>{t.consent_title}</h1>
                    <p style="margin:var(--space-3) 0">
                        <strong>{client_name}</strong>
                        " " {t.consent_app_wants_access}
                    </p>
                    <ul style="list-style:none;padding:0;margin-bottom:var(--space-4)">
                        {scope_labels}
                    </ul>
                    <div class="row" style="gap:var(--space-2)">
                        <form method="post" action="/oauth2/consent">
                            <input type="hidden" name="_csrf" value=csrf.clone() />
                            <input type="hidden" name="decision" value="approve" />
                            <button type="submit">{t.consent_approve}</button>
                        </form>
                        <form method="post" action="/oauth2/consent">
                            <input type="hidden" name="_csrf" value=csrf />
                            <input type="hidden" name="decision" value="deny" />
                            <button type="submit" class="secondary">{t.consent_deny}</button>
                        </form>
                    </div>
                </div>
            </crate::layout::AuthShell>
        }
    })
}

// ---------- error ----------

/// Render a localized error page (RFC 042).
///
/// `status` is the HTTP status code (404 / 429 / 500 etc.).
/// `request_id` is the opaque ID to show when asking for operator help.
pub fn render_error(status: u16, request_id: &str, lang: sui_id_i18n::Locale) -> String {
    let t = lang.strings();
    let (title, lede) = match status {
        404 => (t.error_not_found_title, t.error_not_found_lede),
        429 => (t.error_too_many_requests_label, t.error_too_many_requests_lede),
        500..=599 => (t.error_internal, t.error_internal_lede),
        _ => (t.error_generic_title, t.error_generic_lede),
    };
    let rid = request_id.to_string();
    let req_id_label = t.error_request_id_label;
    let back_home = t.error_back_home;
    render(move || {
        view! {
            <crate::layout::AuthShell title=title.to_string() lang=lang>
                <div class="auth-card">
                    <h1>{status.to_string()}</h1>
                    <h2>{title}</h2>
                    <p class="muted">{lede}</p>
                    <p class="muted" style="font-size:0.85em">
                        {req_id_label} ": "
                        <span class="code">{rid}</span>
                    </p>
                    <p>
                        <a href="/" class="button secondary">{back_home}</a>
                    </p>
                </div>
            </crate::layout::AuthShell>
        }
    })
}


// ---------- /me/security/* tabbed pages (RFC 040) ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeTab {
    Overview,
    Mfa,
    Passkey,
    Sessions,
    Language,
}

pub struct MeShellData {
    pub username: String,
    pub is_admin: bool,
    pub active_tab: MeTab,
}

pub struct MeOverviewData {
    pub shell: MeShellData,
    pub totp_enabled: bool,
    pub passkey_count: usize,
    pub active_session_count: usize,
    pub recent_events: Vec<MeAuditEntry>,
    pub csrf_token: String,
}

pub struct MePasskeyData {
    pub shell: MeShellData,
    pub passkeys: Vec<PasskeyDescriptor>,
    /// False = origin is plain HTTP on a non-localhost host → show warning.
    pub origin_eligible: bool,
    pub csrf_token: String,
}

pub struct MeLanguageData {
    pub shell: MeShellData,
    pub current_preferred_lang: Option<String>,
    pub csrf_token: String,
    /// True when the page was rendered immediately after a successful
    /// POST. The view shows a localised success banner (RFC 057).
    pub just_saved: bool,
}

fn me_security_tabs(active: MeTab, lang: sui_id_i18n::Locale) -> impl IntoView {
    let t = lang.strings();
    let items = [
        (MeTab::Overview, t.me_tab_overview, "/me/security/overview"),
        (MeTab::Mfa,      t.me_tab_mfa,      "/me/security/mfa"),
        (MeTab::Passkey,  t.me_tab_passkey,  "/me/security/passkeys"),
        (MeTab::Sessions, t.me_tab_sessions, "/me/security/sessions"),
        (MeTab::Language, t.me_tab_language, "/me/security/language"),
    ];
    let tab_items: Vec<_> = items.iter().map(|(tab, label, href)| {
        let cls = if *tab == active { "tab tab--active" } else { "tab" };
        view! { <a href=*href class=cls>{*label}</a> }
    }).collect();
    view! {
        <nav class="tabs" aria-label=t.me_security_tabs_aria>
            {tab_items}
        </nav>
    }
}


pub struct MeMfaData {
    pub shell: MeShellData,
    pub totp_enabled: bool,
    pub passkey_count: usize,
    pub recovery_codes_remaining: usize,
    /// Recovery codes shown once after enrollment or regeneration.
    /// Wrapped in an `<ol>` and prominent banner; this is the only
    /// chance to copy them since the server only stores hashes.
    pub fresh_recovery_codes: Option<Vec<String>>,
    pub csrf_token: String,
}

pub struct MeSessionsData {
    pub shell: MeShellData,
    pub current_session_id: String,
    pub sessions: Vec<MeSessionDescriptor>,
    pub csrf_token: String,
}

pub fn render_me_mfa(
    data: MeMfaData,
    flash: Option<Flash>,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Mfa, lang);
        let MeMfaData {
            shell: _,
            totp_enabled,
            passkey_count,
            recovery_codes_remaining,
            fresh_recovery_codes,
            csrf_token,
        } = data;
        // Recovery codes banner — shown once, immediately after enrollment
        // or regeneration. The server only keeps hashes; if the user
        // doesn't save these codes now, they're unrecoverable.
        let recovery_block = fresh_recovery_codes.map(|codes| {
            let lis: Vec<_> = codes
                .into_iter()
                .map(|c| view! { <li><span class="code">{c}</span></li> })
                .collect();
            view! {
                <div class="flash warn" role="status">
                    <div class="stack-tight">
                        <strong>{t.profile_recovery_save_now}</strong>
                        <p class="muted">{t.profile_recovery_save_lede}</p>
                        <ol>{lis}</ol>
                    </div>
                </div>
            }
        });
        let mfa_disable_onsubmit = format!(
            "return confirm('{}');",
            t.profile_mfa_disable_confirm.replace('\'', "\\'")
        );
        let csrf_for_enroll = csrf_token.clone();
        let csrf_for_disable = csrf_token.clone();
        let csrf_for_regen = csrf_token.clone();
        view! {
            <Shell title=t.me_tab_mfa.to_string() show_nav=true current=Some("me".to_string()) lang=lang>
                <header class="page-header">
                    <h1 class="page-header__title">{t.me_tab_mfa}</h1>
                </header>
                {tabs}
                {flash_banner(flash)}
                {recovery_block}
                <div class="stack" style="margin-top:var(--space-4)">
                    // TOTP card
                    <section class="card">
                        <h2 class="card__title">{t.profile_mfa_totp_card_title}</h2>
                        <dl class="kv-list">
                            {kv_bool_badge(t, t.me_security_mfa_status_label, totp_enabled)}
                            {if totp_enabled {
                                view! {
                                    <div>
                                        {kv_row(t.me_security_mfa_recovery_section_label,
                                                (t.me_security_mfa_recovery_codes_remaining)(recovery_codes_remaining))}
                                    </div>
                                }.into_any()
                            } else { view! { <div/> }.into_any() }}
                        </dl>
                        <div class="row" style="margin-top:var(--space-3)">
                            {if totp_enabled {
                                view! {
                                    <>
                                    <form method="post" action="/me/security/mfa/recovery-codes/regenerate"
                                          style="display:inline">
                                        <input type="hidden" name="_csrf" value=csrf_for_regen />
                                        <button type="submit" class="secondary">{t.profile_mfa_regenerate_codes}</button>
                                    </form>
                                    <form method="post" action="/me/security/mfa/disable"
                                          style="display:inline"
                                          onsubmit=mfa_disable_onsubmit>
                                        <input type="hidden" name="_csrf" value=csrf_for_disable />
                                        <button type="submit" class="danger">{t.profile_mfa_disable_button}</button>
                                    </form>
                                    </>
                                }.into_any()
                            } else {
                                view! {
                                    <form method="post" action="/me/security/mfa/enroll/start">
                                        <input type="hidden" name="_csrf" value=csrf_for_enroll />
                                        <button type="submit">{t.profile_mfa_enroll_button}</button>
                                    </form>
                                }.into_any()
                            }}
                        </div>
                    </section>
                    // Passkeys summary
                    <section class="card">
                        <h2 class="card__title">{t.me_passkey_section_title}</h2>
                        {kv_row(t.profile_passkeys_section, passkey_count.to_string())}
                        <p style="margin-top:var(--space-3)">
                            <a href="/me/security/passkeys" class="button secondary">{t.me_tab_passkey}</a>
                        </p>
                    </section>
                </div>
            </Shell>
        }
    })
}

pub fn render_me_sessions(
    data: MeSessionsData,
    flash: Option<Flash>,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Sessions, lang);
        let MeSessionsData { shell: _, current_session_id, sessions, csrf_token } = data;
        let revoke_label = t.me_security_sessions_revoke;
        let revoke_confirm = t.me_security_sessions_revoke_confirm.to_owned();
        let current_badge = t.me_security_sessions_current_badge;
        let csrf2 = csrf_token.clone();

        let session_rows: Vec<_> = sessions.into_iter().map(|s| {
            let sid = s.id.clone();
            let created = s.created_at.format("%Y/%m/%d %H:%M").to_string();
            let expires = s.expires_at.format("%Y/%m/%d %H:%M").to_string();
            let methods = s.auth_methods.clone();
            let is_current = s.is_current;
            let csrf_row = csrf_token.clone();
            let revoke_action = format!("/me/security/sessions/{sid}/revoke");
            let confirm_js = revoke_confirm.replace('\'', "\'");
            let action_cell = if is_current {
                view! {
                    <td><span class="badge badge--ok">{current_badge}</span></td>
                }.into_any()
            } else {
                view! {
                    <td>
                        <form method="post" action=revoke_action
                              onsubmit=format!("return confirm('{confirm_js}')")>
                            <input type="hidden" name="_csrf" value=csrf_row/>
                            <button type="submit" class="secondary danger-text">{revoke_label}</button>
                        </form>
                    </td>
                }.into_any()
            };
            view! {
                <tr>
                    <td><time>{created}</time></td>
                    <td><time>{expires}</time></td>
                    <td>{methods}</td>
                    {action_cell}
                </tr>
            }
        }).collect();

        let revoke_all_confirm = t.me_security_sessions_revoke_all_others_confirm.replace('\'', "\'");
        view! {
            <Shell title=t.me_security_sessions_section.to_string() show_nav=true current=Some("me".to_string()) lang=lang>
                <header class="page-header">
                    <h1 class="page-header__title">{t.me_security_sessions_section}</h1>
                </header>
                {tabs}
                {flash_banner(flash)}
                <div class="stack" style="margin-top:var(--space-4)">
                    <section class="card">
                        <p class="muted">{t.me_security_sessions_lede}</p>
                        {if session_rows.is_empty() {
                            view! { <p class="muted">{t.me_security_sessions_lede}</p> }.into_any()
                        } else {
                            view! {
                                <div class="table-wrap">
                                    <table>
                                        <thead>
                                            <tr>
                                                <th>{t.me_security_sessions_th_started}</th>
                                                <th>{t.me_security_sessions_th_expires}</th>
                                                <th>{t.me_security_sessions_th_factors}</th>
                                                <th/>
                                            </tr>
                                        </thead>
                                        <tbody>{session_rows}</tbody>
                                    </table>
                                </div>
                            }.into_any()
                        }}
                        <div style="margin-top:var(--space-3)">
                            <form method="post" action="/me/security/sessions/revoke-all-others"
                                  onsubmit=format!("return confirm('{revoke_all_confirm}')")>
                                <input type="hidden" name="_csrf" value=csrf2/>
                                <button type="submit" class="secondary">
                                    {t.me_security_sessions_revoke_all_others}
                                </button>
                            </form>
                        </div>
                    </section>
                </div>
            </Shell>
        }
    })
}

pub fn render_me_overview(
    data: MeOverviewData,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Overview, lang);
        let MeOverviewData { shell, totp_enabled, passkey_count, active_session_count, recent_events, .. } = data;
        let event_rows: Vec<_> = recent_events.iter().map(|e| {
            let badge = match e.result.as_str() {
                "ok"   => view! { <span class="badge badge--ok">{"ok"}</span> }.into_any(),
                "fail" | "denied" => view! { <span class="badge badge--danger">{e.result.clone()}</span> }.into_any(),
                other  => view! { <span class="badge">{other.to_string()}</span> }.into_any(),
            };
            view! {
                <tr>
                    <td><time>{e.at.format("%Y/%m/%d %H:%M").to_string()}</time></td>
                    <td><code>{e.action.clone()}</code></td>
                    <td>{badge}</td>
                </tr>
            }
        }).collect();
        view! {
            <Shell title=t.me_tab_overview.to_string() show_nav=true current=Some("me".to_string()) lang=lang>
                <header class="page-header"><h1 class="page-header__title">{t.me_tab_overview}</h1></header>
                {tabs}
                <div class="stack" style="margin-top:var(--space-4)">
                    <section class="card">
                        <h2 class="card__title">{t.me_overview_section_status}</h2>
                        <dl class="kv-list">
                            {kv_bool_badge(t, "MFA (TOTP)", totp_enabled)}
                            {kv_row("Passkeys", passkey_count.to_string())}
                            {kv_row(t.me_security_sessions_section,
                                    active_session_count.to_string())}
                        </dl>
                    </section>
                    <section class="card">
                        <h2 class="card__title">{t.me_overview_section_activity}</h2>
                        {if event_rows.is_empty() {
                            view! { <p class="muted">{t.me_security_sessions_lede}</p> }.into_any()
                        } else {
                            view! {
                                <div class="table-wrap">
                                    <table><tbody>{event_rows}</tbody></table>
                                </div>
                            }.into_any()
                        }}
                    </section>
                    <div class="row">
                        <a href="/me/security/mfa" class="button secondary">{t.me_tab_mfa}</a>
                        <a href="/me/security/passkeys" class="button secondary">{t.me_tab_passkey}</a>
                        <a href="/me/security/sessions" class="button secondary">{t.me_tab_sessions}</a>
                    </div>
                </div>
            </Shell>
        }
    })
}

pub fn render_me_passkey(
    data: MePasskeyData,
    flash: Option<Flash>,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Passkey, lang);
        let MePasskeyData { shell: _, passkeys, origin_eligible, csrf_token } = data;
        let warning = (!origin_eligible).then(|| view! {
            <div class="banner banner--warning" role="alert">
                {t.me_passkey_origin_warning}
            </div>
        });
        let rows: Vec<_> = passkeys.iter().map(|p| {
            let cred_id = p.id.clone();
            let nick = p.nickname.clone();
            let nick2 = p.nickname.clone();
            let csrf = csrf_token.clone();
            let delete_action = format!("/me/security/passkeys/{}/delete", cred_id);
            view! {
                <tr>
                    <td>
                        <strong>{nick}</strong>
                        <br/>
                        <span class="muted" style="font-size:0.85em">
                            {t.profile_passkeys_th_registered} ": " {p.created_at.format("%Y/%m/%d").to_string()}
                        </span>
                    </td>
                    <td>
                        <details>
                            <summary class="button secondary" style="font-size:0.85em">
                                {t.me_passkey_button_rename}
                            </summary>
                            <form method="post"
                                  action={format!("/me/security/passkeys/{cred_id}/rename")}
                                  style="margin-top:var(--space-2)">
                                <input type="hidden" name="_csrf" value=csrf.clone()/>
                                <div class="row">
                                    <input type="text" name="nickname"
                                           placeholder=t.me_passkey_nickname_placeholder
                                           required=true maxlength="64"
                                           style="flex:1"/>
                                    <button type="submit">{t.button_save}</button>
                                </div>
                            </form>
                        </details>
                    </td>
                    <td>
                        <form method="post" action=delete_action>
                            <input type="hidden" name="_csrf" value=csrf/>
                            <button type="submit" class="button danger"
                                    aria-label={format!("{} {nick2}", t.button_delete)}>
                                {t.button_delete}
                            </button>
                        </form>
                    </td>
                </tr>
            }
        }).collect();
        view! {
            <Shell title=t.me_passkey_section_title.to_string() show_nav=true current=Some("me".to_string()) lang=lang>
                <header class="page-header"><h1 class="page-header__title">{t.me_passkey_section_title}</h1></header>
                {tabs}
                {flash_banner(flash)}
                {warning}
                <div class="stack" style="margin-top:var(--space-4)">
                    {if rows.is_empty() {
                        view! { <p class="muted">{t.profile_passkeys_empty}</p> }.into_any()
                    } else {
                        view! {
                            <div class="table-wrap">
                                <table><tbody>{rows}</tbody></table>
                            </div>
                        }.into_any()
                    }}
                    {origin_eligible.then(|| view! {
                        <section class="card">
                            <h3 class="card__title">{t.profile_passkeys_register_section}</h3>
                            <form id="passkey-register-form" method="post"
                                  action="/me/security/passkeys/register/start" class="stack">
                                <input type="hidden" name="_csrf" value=csrf_token.clone()/>
                                <div class="field">
                                    <label for="pk-nickname" class="field__label">
                                        {t.profile_passkeys_nickname_label}
                                    </label>
                                    <input id="pk-nickname" name="nickname" type="text" required=true
                                           placeholder=t.me_passkey_nickname_placeholder
                                           maxlength="64" />
                                    <span class="field__hint">{t.profile_passkeys_nickname_hint}</span>
                                </div>
                                <div>
                                    <button type="submit">{t.profile_passkeys_register_button}</button>
                                </div>
                            </form>
                        </section>
                    })}
                    <script src="/static/webauthn.js"></script>
                </div>
            </Shell>
        }
    })
}

pub fn render_me_language(
    data: MeLanguageData,
    flash: Option<Flash>,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Language, lang);
        let MeLanguageData { shell: _, current_preferred_lang, csrf_token, just_saved } = data;
        let cur = current_preferred_lang.clone().unwrap_or_default();
        let cur2 = cur.clone();
        let cur3 = cur.clone();
        let cur4 = cur.clone();
        view! {
            <Shell title=t.me_language_title.to_string() show_nav=true current=Some("me".to_string()) lang=lang>
                <header class="page-header"><h1 class="page-header__title">{t.me_language_title}</h1></header>
                {tabs}
                {flash_banner(flash)}
                {just_saved.then(|| view! {
                    <div class="banner banner--success" role="status"
                         style="margin-top:var(--space-3)">
                        {t.me_security_language_saved_banner}
                    </div>
                })}
                <div class="card" style="margin-top:var(--space-4)">
                    <p class="muted">{t.me_language_lede}</p>
                    <form method="post" action="/me/security/language" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token/>
                        <div class="field">
                            <div class="stack" style="gap:var(--space-2)">
                                <label class="row" style="align-items:center;gap:var(--space-2)">
                                    <input type="radio" name="locale" value=""
                                           checked=move || cur.is_empty()/>
                                    {t.me_language_use_default}
                                </label>
                                <label class="row" style="align-items:center;gap:var(--space-2)">
                                    <input type="radio" name="locale" value="ja"
                                           checked=move || cur2 == "ja"/>
                                    {t.locale_native_ja}
                                </label>
                                <label class="row" style="align-items:center;gap:var(--space-2)">
                                    <input type="radio" name="locale" value="en"
                                           checked=move || cur3 == "en"/>
                                    {t.locale_native_en}
                                </label>
                                <label class="row" style="align-items:center;gap:var(--space-2)">
                                    <input type="radio" name="locale" value="zh"
                                           checked=move || cur4 == "zh"/>
                                    {t.locale_native_zh}
                                </label>
                            </div>
                        </div>
                        <div>
                            <button type="submit">{t.button_save}</button>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}

// ---------- /me/security ----------
//
// Self-service security overview for the signed-in user. Shows where
// they are signed in, lets them revoke individual sessions or sign out
// everywhere else, and surfaces a user-scoped activity timeline so
// they have a chance to notice unusual events on their own account
// without an operator having to escalate.
//
// MFA management itself stays on `/admin/profile` (which is
// misleadingly named — it's "user profile", and a non-admin user can
// reach it the same way; the page does not require admin). We link
// to it from here rather than re-implement.

pub struct MeSecurityData {
    pub username: String,
    pub is_admin: bool,
    /// Whether the user has TOTP enrolled.
    pub totp_enabled: bool,
    /// Number of active WebAuthn passkeys.
    pub passkey_count: usize,
    /// Identifier of the session that issued the current request.
    /// Used to mark "this is you" in the session list and to keep it
    /// alive when the user clicks "sign out everywhere else".
    pub current_session_id: String,
    pub sessions: Vec<MeSessionDescriptor>,
    pub recent_events: Vec<MeAuditEntry>,
}

pub struct MeSessionDescriptor {
    pub id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Comma-separated human display: "password", "password + TOTP", etc.
    pub auth_methods: String,
    pub is_current: bool,
}

pub struct MeAuditEntry {
    pub at: chrono::DateTime<chrono::Utc>,
    pub action: String,
    pub result: String,
    pub note: Option<String>,
}

pub fn render_me_security(
    data: MeSecurityData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let MeSecurityData {
            username,
            is_admin,
            totp_enabled,
            passkey_count,
            current_session_id,
            sessions,
            recent_events,
        } = data;

        let csrf_for_revoke_others = csrf_token.clone();
        let revoke_label = t.me_security_sessions_revoke;
        let revoke_confirm = t.me_security_sessions_revoke_confirm.to_owned();
        let current_badge = t.me_security_sessions_current_badge;

        // Session table rows. Each non-current row gets its own
        // mini-form so a user can revoke that specific entry.
        let session_rows: Vec<_> = sessions
            .into_iter()
            .map(|s| {
                let MeSessionDescriptor {
                    id,
                    created_at,
                    expires_at,
                    auth_methods,
                    is_current,
                } = s;
                let when = created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                let until = expires_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                let action_cell = if is_current {
                    view! {
                        <td>
                            <span class="badge badge--accent">{current_badge}</span>
                        </td>
                    }
                    .into_any()
                } else {
                    let csrf_for_row = csrf_token.clone();
                    let post_url = format!("/me/security/sessions/{id}/revoke");
                    let onsubmit_attr = format!("return confirm('{}');", revoke_confirm.replace('\'', "\\'"));
                    view! {
                        <td>
                            <form method="post" action=post_url style="display:inline"
                                  onsubmit=onsubmit_attr>
                                <input type="hidden" name="_csrf" value=csrf_for_row />
                                <button type="submit" class="secondary">{revoke_label}</button>
                            </form>
                        </td>
                    }
                    .into_any()
                };
                view! {
                    <tr>
                        <td class="muted">{when}</td>
                        <td class="muted">{until}</td>
                        <td>{auth_methods}</td>
                        {action_cell}
                    </tr>
                }
            })
            .collect();

        // Activity timeline.
        let event_rows: Vec<_> = recent_events
            .into_iter()
            .map(|e| {
                let MeAuditEntry {
                    at,
                    action,
                    result,
                    note,
                } = e;
                let when = at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                let note_str = note.unwrap_or_default();
                let result_badge = match result.as_str() {
                    "ok" => view! { <span class="badge badge--ok">"ok"</span> }.into_any(),
                    "fail" | "error" | "denied" => {
                        view! { <span class="badge badge--danger">{result.clone()}</span> }
                            .into_any()
                    }
                    _ => view! { <span class="badge">{result.clone()}</span> }.into_any(),
                };
                view! {
                    <tr>
                        <td class="muted">{when}</td>
                        <td><span class="code">{action}</span></td>
                        <td>{result_badge}</td>
                        <td class="muted">{note_str}</td>
                    </tr>
                }
            })
            .collect();

        let admin_link = is_admin.then(|| {
            view! {
                <p class="muted">
                    <a href="/admin">{t.me_security_admin_link}</a>
                </p>
            }
        });

        let mfa_summary = if totp_enabled || passkey_count > 0 {
            let parts = {
                let mut v = Vec::<String>::new();
                if totp_enabled {
                    v.push(t.me_security_mfa_factor_totp.to_owned());
                }
                if passkey_count > 0 {
                    v.push(
                        {t.me_security_mfa_factor_passkey_n}
                            .replace("{n}", &passkey_count.to_string()),
                    );
                }
                v.join(" / ")
            };
            view! {
                <p>
                    {t.me_security_mfa_status_label}
                    <span class="badge badge--ok" style="margin-left:var(--space-1)">{t.me_security_mfa_status_enabled}</span>
                    <span class="muted" style="margin-left:var(--space-2)">{parts}</span>
                </p>
            }
            .into_any()
        } else {
            view! {
                <div class="flash warn" role="status">
                    <div class="stack-tight">
                        <strong>{t.me_security_mfa_disabled_title}</strong>
                        <p class="muted" style="margin:0">{t.me_security_mfa_disabled_lede}</p>
                    </div>
                </div>
            }
            .into_any()
        };

        let revoke_all_others_onsubmit = format!(
            "return confirm('{}');",
            t.me_security_sessions_revoke_all_others_confirm.replace('\'', "\\'")
        );

        view! {
            <Shell title=t.me_security_title.to_owned() show_nav=false current=None lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.me_security_title}</h1>
                        <p class="page-header__lede">
                            <strong>{username}</strong>{t.me_security_signed_in_as_suffix}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {admin_link}

                <section>
                    <h2>{t.me_security_mfa_section}</h2>
                    <div class="card">
                        {mfa_summary}
                        <div class="card__footer">
                            <a href="/admin/profile" class="button secondary">
                                {t.me_security_mfa_manage}
                            </a>
                            <a href="/me/security/password" class="button secondary">
                                {t.me_security_password_change_link}
                            </a>
                        </div>
                    </div>
                </section>

                <section>
                    <h2>{t.me_security_sessions_section}</h2>
                    <p class="muted">{t.me_security_sessions_lede}</p>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.me_security_sessions_th_started}</th>
                                    <th>{t.me_security_sessions_th_expires}</th>
                                    <th>{t.me_security_sessions_th_factors}</th>
                                    <th></th>
                                </tr>
                            </thead>
                            <tbody>{session_rows}</tbody>
                        </table>
                    </div>
                    <form method="post" action="/me/security/sessions/revoke-all-others"
                          style="margin-top:var(--space-3)"
                          onsubmit=revoke_all_others_onsubmit>
                        <input type="hidden" name="_csrf" value=csrf_for_revoke_others />
                        <input type="hidden" name="current_session" value=current_session_id />
                        <button type="submit" class="secondary">
                            {t.me_security_sessions_revoke_all_others}
                        </button>
                    </form>
                </section>

                <section>
                    <h2>{t.me_security_activity_section}</h2>
                    <p class="muted">{t.me_security_activity_lede}</p>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.me_security_activity_th_when}</th>
                                    <th>{t.me_security_activity_th_event}</th>
                                    <th>{t.me_security_activity_th_result}</th>
                                    <th>{t.me_security_activity_th_note}</th>
                                </tr>
                            </thead>
                            <tbody>{event_rows}</tbody>
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}

// ---------- /me/security/password ----------

pub struct PasswordChangeData {
    pub username: String,
    /// Pre-filled checked value of "sign out other sessions". The
    /// caller hands it in so a re-render after a validation error
    /// keeps the user's previous choice.
    pub revoke_others_default: bool,
}

pub fn render_password_change(
    data: PasswordChangeData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let PasswordChangeData {
            username,
            revoke_others_default,
        } = data;
        let revoke_attr = if revoke_others_default { Some("") } else { None };
        view! {
            <Shell title=t.password_change_title.to_owned() show_nav=false current=None lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.password_change_title}</h1>
                        <p class="page-header__lede">
                            <strong>{username}</strong>{t.me_security_signed_in_as_suffix}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card" style="max-width:var(--content-narrow-width)">
                    <form method="post" action="/me/security/password"
                          autocomplete="off" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />

                        <div class="field">
                            <label for="current_password" class="field__label">{t.password_change_current_label}</label>
                            <input type="password" id="current_password" name="current_password"
                                   required autocomplete="current-password" />
                        </div>

                        <div class="field">
                            <label for="new_password" class="field__label">{t.password_change_new_label}</label>
                            <input type="password" id="new_password" name="new_password"
                                   required autocomplete="new-password" minlength="12" />
                            <span class="field__hint">{t.password_change_new_hint}</span>
                        </div>

                        <div class="field">
                            <label for="confirm_password" class="field__label">{t.password_change_confirm_label}</label>
                            <input type="password" id="confirm_password" name="confirm_password"
                                   required autocomplete="new-password" minlength="12" />
                        </div>

                        <div class="field">
                            <label class="row" style="gap:var(--space-2)">
                                <input type="checkbox" name="revoke_others" value="1"
                                       checked=revoke_attr />
                                <span>{t.password_change_revoke_others_label}</span>
                            </label>
                            <span class="field__hint">{t.password_change_revoke_others_hint}</span>
                        </div>

                        <div class="row">
                            <button type="submit">{t.password_change_submit}</button>
                            <a href="/me/security" class="button secondary">{t.button_cancel}</a>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}

// ---------- /admin/settings/* (v0.20.3) ----------
//
// Five read-only tabs surfacing the current effective configuration.
// Each tab is its own route; this view module just renders the
// shell + 5-tab strip + the tab body. The strip is intentionally
// styled with the same `.app-nav__link` vocabulary as the main nav,
// so the visual treatment is consistent: hover, focus ring, and the
// `aria-current="page"` pill all behave the same.

/// Identifier of the currently-active settings tab. The settings
/// page renders the same 5-tab strip on every sub-route; this enum
/// drives which tab gets `aria-current="page"`.
#[derive(Clone, Copy)]
pub enum SettingsTab {
    Basic,
    Security,
    Authentication,
    Logs,
    Email,
    Other,
}

impl SettingsTab {
    fn key(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Security => "security",
            Self::Authentication => "authentication",
            Self::Logs => "logs",
            Self::Email => "email",
            Self::Other => "other",
        }
    }
}

fn settings_tabs(active: SettingsTab, lang: sui_id_i18n::Locale) -> impl IntoView {
    let t = lang.strings();
    let items = [
        (SettingsTab::Basic,          t.settings_tab_basic,           "/admin/settings/basic"),
        (SettingsTab::Security,       t.settings_tab_security,        "/admin/settings/security"),
        (SettingsTab::Authentication, t.settings_tab_authentication,  "/admin/settings/authentication"),
        (SettingsTab::Logs,           t.settings_tab_logs,            "/admin/settings/logs"),
        (SettingsTab::Email,          t.settings_tab_email,           "/admin/settings/email"),
        (SettingsTab::Other,          t.settings_tab_advanced,        "/admin/settings/other"),
    ];
    let active_key = active.key();
    let links: Vec<_> = items
        .into_iter()
        .map(|(tab, label, href)| {
            let aria = if tab.key() == active_key { Some("page") } else { None };
            view! {
                <a class="app-nav__link" href=href aria-current=aria>{label}</a>
            }
        })
        .collect();
    view! {
        <nav class="app-nav" aria-label=t.settings_tabs_aria style="margin-bottom:var(--space-4);flex-wrap:wrap">
            {links}
        </nav>
    }
}

/// Two-column key/value table used inside each settings card. Keeps
/// per-tab content boring and consistent.
fn kv_row(k: &str, v: impl IntoView + 'static) -> impl IntoView {
    let k = k.to_owned();
    view! {
        <tr>
            <th scope="row" style="width:14rem;font-weight:var(--font-weight-medium);color:var(--fg-muted);text-align:left">
                {k}
            </th>
            <td>{v}</td>
        </tr>
    }
}

fn kv_text(k: &str, v: String) -> impl IntoView {
    kv_row(k, view! { <span>{v}</span> })
}

fn kv_code(k: &str, v: String) -> impl IntoView {
    kv_row(k, view! { <span class="code">{v}</span> })
}

fn kv_bool_badge(t: &'static sui_id_i18n::Strings, k: &str, on: bool) -> impl IntoView {
    let badge = if on {
        view! { <span class="badge badge--ok">{t.badge_enabled}</span> }.into_any()
    } else {
        view! { <span class="badge">{t.badge_disabled}</span> }.into_any()
    };
    kv_row(k, badge)
}

// ---------- 基本タブ ----------

pub struct SettingsBasicData {
    pub issuer: String,
    pub listen_addr: String,
    pub cookie_secure: bool,
    pub trusted_proxies: Vec<String>,
    pub discovery_url: String,
    pub jwks_url: String,
    /// Server-wide default UI language (BCP-47 tag, e.g. "ja").
    /// Comes from `server_settings.default_lang`. Editable via the
    /// form on this page; saved through `POST /admin/settings/basic/lang`.
    pub default_lang: String,
    /// CSRF token for the in-page edit form. Empty string when
    /// rendered without CSRF (legacy callers); the lang form
    /// no-ops when the token is missing.
    pub csrf_token: String,
}

pub fn render_settings_basic(data: SettingsBasicData, flash: Option<Flash>, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsBasicData {
            issuer,
            listen_addr,
            cookie_secure,
            trusted_proxies,
            discovery_url,
            jwks_url,
            default_lang,
            csrf_token,
        } = data;
        let csrf_for_lang = csrf_token.clone();
        let lang_form = view! {
            <section class="section">
                <h2 class="section__title">{t.settings_basic_default_lang}</h2>
                <p class="muted">
                    {t.settings_basic_default_lang_hint}
                </p>
                <div class="card">
                    <form method="post" action="/admin/settings/basic/lang" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_for_lang />
                        <div class="field">
                            <label for="default-lang-select" class="field__label">
                                {t.profile_lang_label}
                            </label>
                            <select id="default-lang-select" name="default_lang">
                                {sui_id_i18n::Locale::ALL.iter().map(|loc| {
                                    let tag = loc.tag();
                                    let selected = default_lang == tag;
                                    let label = loc.native_name();
                                    view! {
                                        <option value=tag selected=selected>{label}</option>
                                    }
                                }).collect::<Vec<_>>()}
                            </select>
                        </div>
                        <div>
                            <button type="submit">{t.button_save}</button>
                        </div>
                    </form>
                </div>
            </section>
        };
        let proxies_display = if trusted_proxies.is_empty() {
            t.settings_basic_trusted_proxies_none.to_owned()
        } else {
            trusted_proxies.join(", ")
        };
        view! {
            <Shell title=t.settings_title_basic.to_string() show_nav=true current=Some("settings".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_basic_description}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Basic, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_tab_basic}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code(t.settings_basic_kv_issuer, issuer)}
                                {kv_code(t.settings_basic_kv_listen, listen_addr)}
                                {kv_bool_badge(t, t.settings_basic_kv_cookie_secure, cookie_secure)}
                                {kv_text(t.settings_basic_kv_trusted_proxies, proxies_display)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_basic_oidc_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                <tr>
                                    <th scope="row" style="width:14rem;font-weight:var(--font-weight-medium);color:var(--fg-muted);text-align:left">{t.dashboard_oidc_endpoint_discovery}</th>
                                    <td>
                                        {
                                            let url = discovery_url.clone();
                                            view! {
                                                <a href=discovery_url>
                                                    <span class="code">{url}</span>
                                                </a>
                                            }
                                        }
                                    </td>
                                </tr>
                                <tr>
                                    <th scope="row" style="width:14rem;font-weight:var(--font-weight-medium);color:var(--fg-muted);text-align:left">{t.dashboard_oidc_endpoint_jwks}</th>
                                    <td>
                                        {
                                            let url = jwks_url.clone();
                                            view! {
                                                <a href=jwks_url>
                                                    <span class="code">{url}</span>
                                                </a>
                                            }
                                        }
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
                {lang_form}
            </Shell>
        }
    })
}

// ---------- セキュリティタブ ----------

pub struct SettingsSecurityData {
    pub max_lockout_label: String,
    pub hsts_enabled: bool,
    pub csp_enabled: bool,
    pub x_frame_deny: bool,
    pub permissions_policy_minimal: bool,
    pub cors_token_dynamic_from_clients: bool,
    pub cors_public_endpoints_open: bool,
    /// v0.25.0 — current value in seconds, 0 = disabled.
    pub idle_session_timeout_secs: i64,
    /// v0.25.0 — current cap, 0 = disabled.
    pub max_concurrent_sessions: i64,
    /// CSRF token for the inline edit forms. Empty string is
    /// tolerated (forms no-op without it) but production callers
    /// should always pass a real token.
    pub csrf_token: String,
}

pub fn render_settings_security(data: SettingsSecurityData, flash: Option<Flash>, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsSecurityData {
            max_lockout_label,
            hsts_enabled,
            csp_enabled,
            x_frame_deny,
            permissions_policy_minimal,
            cors_token_dynamic_from_clients,
            cors_public_endpoints_open,
            idle_session_timeout_secs,
            max_concurrent_sessions,
            csrf_token,
        } = data;
        let csrf_for_idle = csrf_token.clone();
        let csrf_for_cap = csrf_token.clone();
        let session_forms = view! {
            <section class="section">
                <h2 class="section__title">{t.settings_security_session_section}</h2>
                <p class="muted">
                    {t.settings_security_session_lede}
                </p>
                <div class="card">
                    <form method="post" action="/admin/settings/security/idle-timeout" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_for_idle />
                        <div class="field">
                            <label for="idle-timeout" class="field__label">
                                {t.settings_security_idle_timeout_label}
                            </label>
                            <input id="idle-timeout" name="secs" type="number"
                                   min="0" max="2592000"
                                   value=idle_session_timeout_secs.to_string() />
                            <span class="field__hint">
                                {t.settings_security_idle_timeout_hint}
                            </span>
                        </div>
                        <div>
                            <button type="submit">{t.button_save}</button>
                        </div>
                    </form>
                </div>
                <div class="card">
                    <form method="post" action="/admin/settings/security/max-sessions" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_for_cap />
                        <div class="field">
                            <label for="max-sessions" class="field__label">
                                {t.settings_security_max_sessions_label}
                            </label>
                            <input id="max-sessions" name="cap" type="number"
                                   min="0" max="1000"
                                   value=max_concurrent_sessions.to_string() />
                            <span class="field__hint">
                                {t.settings_security_max_sessions_hint}
                            </span>
                        </div>
                        <div>
                            <button type="submit">{t.button_save}</button>
                        </div>
                    </form>
                </div>
            </section>
        };
        view! {
            <Shell title=t.settings_title_security.to_string() show_nav=true current=Some("settings".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_basic_description}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Security, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_security_lockout_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code(t.settings_security_lockout_section, max_lockout_label)}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        {t.settings_security_lockout_hint_1}
                        " "
                        {t.settings_security_lockout_hint_2_pre}
                        <span class="code">"sui-id admin unlock-user"</span>
                        {t.settings_security_lockout_hint_2_post}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_security_headers_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge(t, "HSTS (Strict-Transport-Security)", hsts_enabled)}
                                {kv_bool_badge(t, "Content-Security-Policy", csp_enabled)}
                                {kv_bool_badge(t, "X-Frame-Options: DENY", x_frame_deny)}
                                {kv_bool_badge(t, t.settings_security_headers_perm_policy_label, permissions_policy_minimal)}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        {t.settings_security_headers_hint}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"CORS"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge(t, t.settings_security_cors_token_label, cors_token_dynamic_from_clients)}
                                {kv_bool_badge(t, t.settings_security_cors_public_label, cors_public_endpoints_open)}
                            </tbody>
                        </table>
                    </div>
                </div>
                {session_forms}
            </Shell>
        }
    })
}

// ---------- 認証タブ ----------

pub struct SettingsAuthenticationData {
    pub password_min_length: usize,
    pub password_argon2id: String,
    pub totp_enabled_per_user: bool,
    pub webauthn_enabled_per_user: bool,
    pub recovery_codes_per_enrollment: usize,
    pub pkce_required: bool,
    pub access_token_lifetime_secs: i64,
    pub id_token_lifetime_secs: i64,
    pub refresh_token_lifetime_secs: i64,
    pub refresh_rotation: bool,
    pub refresh_theft_detection: bool,
}

fn fmt_lifetime(t: &'static sui_id_i18n::Strings, secs: i64) -> String {
    if secs % 86400 == 0 {
        (t.fmt_lifetime_days)(secs / 86400, secs)
    } else if secs % 3600 == 0 {
        (t.fmt_lifetime_hours)(secs / 3600, secs)
    } else if secs % 60 == 0 {
        (t.fmt_lifetime_minutes)(secs / 60, secs)
    } else {
        format!("{secs} s")
    }
}

pub fn render_settings_authentication(
    data: SettingsAuthenticationData,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsAuthenticationData {
            password_min_length,
            password_argon2id,
            totp_enabled_per_user,
            webauthn_enabled_per_user,
            recovery_codes_per_enrollment,
            pkce_required,
            access_token_lifetime_secs,
            id_token_lifetime_secs,
            refresh_token_lifetime_secs,
            refresh_rotation,
            refresh_theft_detection,
        } = data;
        view! {
            <Shell title=t.settings_title_authentication.to_string() show_nav=true current=Some("settings".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_basic_description}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Authentication, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_auth_password_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_text(t.settings_auth_min_length_label, (t.settings_auth_min_length_value)(password_min_length))}
                                {kv_text(t.settings_auth_hash_algorithm_label, password_argon2id)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_auth_mfa_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge(t, t.settings_auth_mfa_totp, totp_enabled_per_user)}
                                {kv_bool_badge(t, t.settings_auth_mfa_passkey, webauthn_enabled_per_user)}
                                {kv_text(t.settings_auth_recovery_codes_label, (t.settings_auth_recovery_codes_value)(recovery_codes_per_enrollment))}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        {t.settings_auth_mfa_note_prefix}
                        <a href="/admin/profile">"/admin/profile"</a>
                        {t.settings_auth_mfa_note_suffix}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"OAuth 2.1 / OIDC"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge(t, t.settings_auth_pkce_required, pkce_required)}
                                {kv_text(t.settings_auth_access_token_ttl, fmt_lifetime(t, access_token_lifetime_secs))}
                                {kv_text(t.settings_auth_id_token_ttl, fmt_lifetime(t, id_token_lifetime_secs))}
                                {kv_text(t.settings_auth_refresh_token_ttl, fmt_lifetime(t, refresh_token_lifetime_secs))}
                                {kv_bool_badge(t, t.settings_auth_refresh_rotate, refresh_rotation)}
                                {kv_bool_badge(t, t.settings_auth_refresh_theft, refresh_theft_detection)}
                            </tbody>
                        </table>
                    </div>
                </div>
            </Shell>
        }
    })
}

// ---------- ログタブ ----------

pub struct SettingsLogsData {
    pub log_format: String,
    pub log_filter: String,
    pub login_success_24h: i64,
    pub login_failure_24h: i64,
    pub login_locked_24h: i64,
    pub password_changed_self_24h: i64,
    pub chain_report: SettingsChainStatus,
}

pub struct SettingsChainStatus {
    pub checked: usize,
    pub broken_at_seq: Option<i64>,
    pub legacy_unhashed: usize,
}

pub fn render_settings_logs(data: SettingsLogsData, flash: Option<Flash>, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsLogsData {
            log_format,
            log_filter,
            login_success_24h,
            login_failure_24h,
            login_locked_24h,
            password_changed_self_24h,
            chain_report,
        } = data;

        let chain_badge = if chain_report.broken_at_seq.is_some() {
            crate::components::status_badge(t, crate::components::StatusKind::Unhealthy).into_any()
        } else {
            crate::components::status_badge(t, crate::components::StatusKind::Healthy).into_any()
        };
        let chain_note = match chain_report.broken_at_seq {
            Some(seq) => (t.audit_chain_broken_note)(seq),
            None => (t.audit_chain_ok_note)(chain_report.checked, chain_report.legacy_unhashed),
        };

        view! {
            <Shell title=t.settings_title_logs.to_string() show_nav=true current=Some("settings".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_logs_lede}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Logs, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_logs_output_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code(t.settings_logs_kv_format, log_format)}
                                {kv_code(t.settings_logs_kv_filter, log_filter)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_logs_recent_24h}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_text("auth.login.success", login_success_24h.to_string())}
                                {kv_text("auth.login.failure", login_failure_24h.to_string())}
                                {kv_text("auth.login.locked", login_locked_24h.to_string())}
                                {kv_text("auth.password.changed_self", password_changed_self_24h.to_string())}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        {t.settings_logs_audit_link_prefix}
                        <a href="/admin/audit">"/admin/audit"</a>
                        {t.settings_logs_audit_link_suffix}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_logs_audit_section}</h3>
                    <div class="row" style="gap:var(--space-3);align-items:center">
                        <span>{t.client_edit_label_status}":"</span>
                        {chain_badge}
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        {chain_note}
                    </p>
                </div>
            </Shell>
        }
    })
}

// ---------- その他タブ ----------

pub struct SettingsOtherData {
    pub binary_version: String,
    pub schema_version: i32,
    pub db_path: String,
    pub master_key_file: String,
    pub user_count: usize,
    pub client_count: usize,
    pub clock_now: chrono::DateTime<chrono::Utc>,
}

pub fn render_settings_other(data: SettingsOtherData, flash: Option<Flash>, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsOtherData {
            binary_version,
            schema_version,
            db_path,
            master_key_file,
            user_count,
            client_count,
            clock_now,
        } = data;
        let now_str = clock_now.format("%Y-%m-%d %H:%M:%S UTC").to_string();
        view! {
            <Shell title=t.settings_title_advanced.to_string() show_nav=true current=Some("settings".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_advanced_lede}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Other, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_advanced_build_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code(t.settings_advanced_version_label, binary_version)}
                                {kv_text(t.settings_advanced_schema_label, schema_version.to_string())}
                                {kv_code(t.settings_advanced_server_time_label, now_str)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_advanced_storage_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code(t.settings_advanced_db_file_label, db_path)}
                                {kv_code(t.settings_advanced_key_file_label, master_key_file)}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        {t.settings_advanced_storage_note_prefix}
                        <span class="code">"SUI_ID_MASTER_KEY"</span>
                        {t.settings_advanced_storage_note_suffix}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_advanced_record_counts}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                <tr>
                                    <th scope="row" style="width:14rem;font-weight:var(--font-weight-medium);color:var(--fg-muted);text-align:left">{t.dashboard_stat_users}</th>
                                    <td>
                                        {(t.settings_advanced_users_count)(user_count)}
                                        <a href="/admin/users" class="muted" style="margin-left:var(--space-2)">
                                            {t.settings_advanced_manage_link}
                                        </a>
                                    </td>
                                </tr>
                                <tr>
                                    <th scope="row" style="width:14rem;font-weight:var(--font-weight-medium);color:var(--fg-muted);text-align:left">{t.dashboard_stat_clients}</th>
                                    <td>
                                        {(t.settings_advanced_clients_count)(client_count)}
                                        <a href="/admin/clients" class="muted" style="margin-left:var(--space-2)">
                                            {t.settings_advanced_manage_link}
                                        </a>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </Shell>
        }
    })
}

// ---------- /me/security/step-up (v0.21.0) ----------
//
// Step-up challenge form. Renders inside the same chrome the rest
// of /me/* uses, but with a narrower main column to focus the
// user on the single thing the page is asking for: a TOTP / passkey
// proof to unlock the next sensitive action.

pub fn render_step_up(
    return_to: &str,
    csrf_token: String,
    has_passkey: bool,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    let return_to = return_to.to_owned();
    render(move || {
        let t = lang.strings();
        let return_to_for_input = return_to.clone();
        let csrf_for_passkey = csrf_token.clone();
        let return_to_for_passkey = return_to.clone();
        let passkey_block = if has_passkey {
            view! {
                <hr class="divider" />
                <p class="muted">{format!("{}:", t.step_up_passkey_alt)}</p>
                <form id="step-up-passkey-form" method="post"
                      action="/me/security/step-up/webauthn/start"
                      class="stack">
                    <input type="hidden" name="_csrf" value=csrf_for_passkey />
                    <input type="hidden" name="return_to" value=return_to_for_passkey />
                    <button type="submit" class="secondary">{t.step_up_passkey_button}</button>
                </form>
                <script src="/static/step-up-webauthn.js"></script>
            }
            .into_any()
        } else {
            view! { <></> }.into_any()
        };
        view! {
            <Shell title=t.step_up_title.to_string() show_nav=false current=None lang=lang>
                <div class="auth-page">
                    <div class="auth-card">
                        <h1>{t.step_up_title}</h1>
                        <p class="muted">{t.step_up_lede}</p>
                        {flash_banner(flash)}
                        <form method="post" action="/me/security/step-up"
                              autocomplete="off" class="stack">
                            <input type="hidden" name="_csrf" value=csrf_token />
                            <input type="hidden" name="return_to" value=return_to_for_input />
                            <div class="field">
                                <label for="code" class="field__label">{t.step_up_code_label}</label>
                                <input id="code" name="code" type="text"
                                       required=true
                                       autocomplete="one-time-code"
                                       inputmode="text"
                                       autofocus=true />
                                <span class="field__hint">{t.step_up_code_hint}</span>
                            </div>
                            <div class="row">
                                <button type="submit">{t.button_confirm}</button>
                                <a href="/me/security" class="button secondary">{t.button_cancel}</a>
                            </div>
                        </form>
                        {passkey_block}
                    </div>
                </div>
            </Shell>
        }
    })
}

// ---------- /forgot-password & /reset-password (v0.22.0) ----------

pub fn render_forgot_password(
    csrf_token: String,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.forgot_password_title.to_string() lang=lang>
                <h1>{t.forgot_password_title}</h1>
                <p class="muted">{t.forgot_password_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/forgot-password" class="stack">
                    <input type="hidden" name="_csrf" value=csrf_token />
                    <div class="field">
                        <label for="email" class="field__label">{t.forgot_password_email_label}</label>
                        <input id="email" name="email" type="email"
                               required=true autocomplete="email" autofocus=true />
                    </div>
                    <div class="row">
                        <button type="submit">{t.forgot_password_submit}</button>
                        <a href="/admin/login" class="button secondary">{t.back_to_login}</a>
                    </div>
                </form>
            </crate::layout::AuthShell>
        }
    })
}

pub fn render_forgot_password_sent(lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.forgot_password_sent_title.to_string() lang=lang>
                <h1>{t.forgot_password_sent_title}</h1>
                <p class="muted">{t.forgot_password_sent_lede}</p>
                <p class="muted">{t.forgot_password_sent_lede2}</p>
                <p style="margin-top:var(--space-4)">
                    <a href="/admin/login" class="button">{t.back_to_login}</a>
                </p>
            </crate::layout::AuthShell>
        }
    })
}

pub fn render_reset_password(
    token: String,
    csrf_token: String,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.reset_password_title.to_string() lang=lang>
                <h1>{t.reset_password_title}</h1>
                <p class="muted">{t.reset_password_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/reset-password" class="stack" autocomplete="off">
                    <input type="hidden" name="_csrf" value=csrf_token />
                    <input type="hidden" name="token" value=token />
                    <div class="field">
                        <label for="password" class="field__label">{t.reset_password_new_label}</label>
                        <input id="password" name="password" type="password"
                               required=true minlength="12"
                               autocomplete="new-password" autofocus=true />
                        <span class="field__hint">{t.reset_password_new_hint}</span>
                    </div>
                    <div class="field">
                        <label for="confirm_password" class="field__label">{t.reset_password_confirm_label}</label>
                        <input id="confirm_password" name="confirm_password" type="password"
                               required=true minlength="12"
                               autocomplete="new-password" />
                    </div>
                    <button type="submit">{t.reset_password_submit}</button>
                </form>
            </crate::layout::AuthShell>
        }
    })
}

pub fn render_reset_password_invalid(lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.reset_password_invalid_title.to_string() lang=lang>
                <h1>{t.reset_password_invalid_title}</h1>
                <p class="muted">{t.reset_password_invalid_lede}</p>
                <p style="margin-top:var(--space-4)">
                    <a href="/forgot-password" class="button">{t.reset_password_invalid_request_again}</a>
                </p>
            </crate::layout::AuthShell>
        }
    })
}

// ---------- /admin/settings/email (v0.22.0) ----------

pub struct SettingsEmailData {
    pub configured: bool,
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub tls_mode: String,
    pub username: String,
    pub has_password: bool,
    pub from_address: String,
    pub from_name: String,
    pub base_url: String,
}

pub fn render_settings_email(
    data: SettingsEmailData,
    csrf_token: String,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsEmailData {
            configured: _,
            enabled,
            host,
            port,
            tls_mode,
            username,
            has_password,
            from_address,
            from_name,
            base_url,
        } = data;
        let csrf_save = csrf_token.clone();
        let csrf_test = csrf_token.clone();
        let port_str = port.to_string();
        let pw_placeholder = if has_password {
            t.settings_email_password_placeholder_change
        } else {
            t.settings_email_password_placeholder_none
        };
        let enabled_attr = if enabled { Some("checked") } else { None };
        let tls_implicit = if tls_mode == "implicit" { Some("selected") } else { None };
        let tls_starttls = if tls_mode == "starttls" { Some("selected") } else { None };

        view! {
            <Shell title=t.settings_email_page_title.to_string() show_nav=true current=Some("settings".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_email_lede}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Email, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_email_smtp_section}</h3>
                    <form method="post" action="/admin/settings/email" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_save />
                        <div class="field">
                            <label class="field__label">
                                <input type="checkbox" name="enabled" value="on" checked=enabled_attr />
                                " "{t.settings_email_enable_checkbox}
                            </label>
                            <span class="field__hint">
                                {t.settings_email_enable_hint}
                            </span>
                        </div>
                        <div class="field">
                            <label for="host" class="field__label">{t.settings_email_host_label}</label>
                            <input id="host" name="host" type="text" required=true value=host />
                        </div>
                        <div class="field">
                            <label for="port" class="field__label">{t.settings_email_port_label}</label>
                            <input id="port" name="port" type="number" min="1" max="65535"
                                   required=true value=port_str />
                            <span class="field__hint">{t.settings_email_port_hint}</span>
                        </div>
                        <div class="field">
                            <label for="tls_mode" class="field__label">{t.settings_email_tls_label}</label>
                            <select id="tls_mode" name="tls_mode">
                                <option value="starttls" selected=tls_starttls>"STARTTLS (587)"</option>
                                <option value="implicit" selected=tls_implicit>{t.settings_email_tls_implicit}</option>
                            </select>
                        </div>
                        <div class="field">
                            <label for="username" class="field__label">{t.settings_email_username_label}</label>
                            <input id="username" name="username" type="text"
                                   autocomplete="off" value=username />
                        </div>
                        <div class="field">
                            <label for="password" class="field__label">{t.settings_auth_password_section}</label>
                            <input id="password" name="password" type="password"
                                   autocomplete="off" placeholder=pw_placeholder />
                            <span class="field__hint">
                                {t.settings_email_password_hint}
                            </span>
                        </div>
                        <hr class="divider" />
                        <div class="field">
                            <label for="from_address" class="field__label">{t.settings_email_from_addr_label}</label>
                            <input id="from_address" name="from_address" type="email"
                                   required=true value=from_address />
                        </div>
                        <div class="field">
                            <label for="from_name" class="field__label">{t.settings_email_from_name_label}</label>
                            <input id="from_name" name="from_name" type="text" value=from_name />
                        </div>
                        <div class="field">
                            <label for="base_url" class="field__label">{t.settings_email_base_url_label}</label>
                            <input id="base_url" name="base_url" type="url"
                                   required=true value=base_url
                                   placeholder="https://idp.example.com" />
                            <span class="field__hint">
                                {t.settings_email_base_url_hint}
                            </span>
                        </div>
                        <button type="submit">{t.settings_email_save_button}</button>
                    </form>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_email_test_section}</h3>
                    <p class="muted">
                        {t.settings_email_test_lede}
                    </p>
                    <form method="post" action="/admin/settings/email/test" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_test />
                        <button type="submit" class="secondary">{t.settings_email_test_button}</button>
                    </form>
                </div>
            </Shell>
        }
    })
}
