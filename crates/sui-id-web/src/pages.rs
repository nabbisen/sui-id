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
fn setup_step_indicator(active: usize) -> impl IntoView {
    // Three labelled dots showing which step the operator is on. Steps
    // 1 and 3 are not interactive (no navigation back into the
    // half-completed wizard); step 2 is the only form. Using static
    // structure here lets screen-readers announce the step state
    // without requiring ARIA tablist.
    let labels = ["ようこそ", "管理者作成", "完了"];
    let dots: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == active;
            let aria = if is_active { Some("step") } else { None };
            // Active step uses the accent badge; past steps use a
            // subtle "ok" badge; future steps use a muted neutral
            // badge to imply "not yet".
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
             aria-label="セットアップステップ"
             style="gap:var(--space-3);justify-content:center;margin-bottom:var(--space-4);flex-wrap:wrap;font-size:var(--font-size-caption)">
            {dots}
        </nav>
    }
}

/// Step 1 of 3 — welcome.
pub fn render_setup_welcome(flash: Option<Flash>) -> String {
    render(move || {
        view! {
            <crate::layout::AuthShell title="セットアップ — ようこそ".to_string()>
                {setup_step_indicator(0)}
                <h1>"sui-id へようこそ"</h1>
                <p class="muted">
                    "このサーバーはまだ初期化されていません。"
                    "数分で完了するセットアップを始めましょう。"
                </p>
                <p class="muted">
                    "次の画面で最初の管理者アカウントを作成します。"
                    "サーバー起動時に出力されたセットアップトークンをお手元にご準備ください。"
                </p>
                {flash_banner(flash)}
                <p style="margin-top:var(--space-4)">
                    <a href="/setup/admin" class="button">"セットアップを始める"</a>
                </p>
            </crate::layout::AuthShell>
        }
    })
}

/// Step 2 of 3 — admin form.
pub fn render_setup_admin(flash: Option<Flash>) -> String {
    render(move || {
        view! {
            <crate::layout::AuthShell title="セットアップ — 管理者作成".to_string()>
                {setup_step_indicator(1)}
                <h1>"管理者アカウントの作成"</h1>
                <p class="muted">
                    "サーバー起動時に出力されたセットアップトークンと、新しい管理者アカウントの情報を入力してください。"
                </p>
                {flash_banner(flash)}
                <form method="post" action="/setup/admin" class="stack" autocomplete="off">
                    <div class="field">
                        <label for="token" class="field__label">"セットアップトークン"</label>
                        <input id="token" name="setup_token" type="password"
                               required=true autocomplete="off" autofocus=true />
                        <span class="field__hint">"起動ログに 1 度だけ出力された値"</span>
                    </div>
                    <div class="field">
                        <label for="username" class="field__label">"ユーザー名"</label>
                        <input id="username" name="username" type="text"
                               required=true autocomplete="username" />
                    </div>
                    <div class="field">
                        <label for="email" class="field__label">"メールアドレス(任意)"</label>
                        <input id="email" name="email" type="email" autocomplete="email" />
                        <span class="field__hint">
                            "通知やパスワードリセットに使用します(将来機能)。後から変更できます。"
                        </span>
                    </div>
                    <div class="field">
                        <label for="display" class="field__label">"表示名(任意)"</label>
                        <input id="display" name="display_name" type="text" autocomplete="name" />
                    </div>
                    <div class="field">
                        <label for="password" class="field__label">"パスワード"</label>
                        <input id="password" name="password" type="password"
                               required=true minlength="12" autocomplete="new-password" />
                        <span class="field__hint">"12 文字以上"</span>
                    </div>
                    <div class="field">
                        <label for="confirm_password" class="field__label">"パスワード(確認)"</label>
                        <input id="confirm_password" name="confirm_password" type="password"
                               required=true minlength="12" autocomplete="new-password" />
                    </div>
                    <div class="row">
                        <a href="/setup" class="button secondary">"戻る"</a>
                        <button type="submit">"管理者を作成"</button>
                    </div>
                </form>
            </crate::layout::AuthShell>
        }
    })
}

/// Step 3 of 3 — completion.
///
/// Renders unconditionally; if the operator reaches it before
/// completing step 2 (e.g. by typing the URL in by hand), the
/// `initialized` flag is false and the page shows a "not yet"
/// notice with a link back to step 1 instead of the success
/// message.
pub fn render_setup_done(initialized: bool) -> String {
    render(move || {
        if initialized {
            view! {
                <crate::layout::AuthShell title="セットアップ — 完了".to_string()>
                    {setup_step_indicator(2)}
                    <h1>"セットアップ完了"</h1>
                    <p class="muted">
                        "管理者アカウントの作成と初期署名キーの発行が完了しました。"
                        "管理画面からシステムの設定を確認できます。"
                    </p>
                    <div class="card">
                        <h3 class="card__title">"次のステップ"</h3>
                        <ul class="muted" style="margin:0;padding-left:var(--space-4)">
                            <li>"OIDC クライアントを登録する。"</li>
                            <li>"パスキーや認証アプリで 2 段階認証を有効にする。"</li>
                            <li>"設定タブで現在の有効な設定を確認する。"</li>
                        </ul>
                    </div>
                    <p style="margin-top:var(--space-4)">
                        <a href="/admin" class="button">"管理画面へ進む"</a>
                    </p>
                </crate::layout::AuthShell>
            }
            .into_any()
        } else {
            view! {
                <crate::layout::AuthShell title="セットアップ — まだ完了していません".to_string()>
                    {setup_step_indicator(0)}
                    <h1>"セットアップは完了していません"</h1>
                    <p class="muted">
                        "管理者アカウントの作成がまだ完了していません。"
                        "セットアップを最初から始めてください。"
                    </p>
                    <p style="margin-top:var(--space-4)">
                        <a href="/setup" class="button">"セットアップを始める"</a>
                    </p>
                </crate::layout::AuthShell>
            }
            .into_any()
        }
    })
}

// ---------- login ----------

pub fn render_login(flash: Option<Flash>, next: Option<String>) -> String {
    render(move || {
        let next_value = next.clone().unwrap_or_default();
        view! {
            <crate::layout::AuthShell title="Sign in".to_string()>
                <h1>"sui-id にログイン"</h1>
                {flash_banner(flash)}
                <form method="post" action="/admin/login" class="stack">
                    <input type="hidden" name="next" value=next_value />
                    <div class="field">
                        <label for="username" class="field__label">"ユーザー名またはメールアドレス"</label>
                        <input id="username" name="username" type="text"
                               required=true autocomplete="username"
                               autofocus=true />
                    </div>
                    <div class="field">
                        <label for="password" class="field__label">"パスワード"</label>
                        <input id="password" name="password" type="password"
                               required=true autocomplete="current-password" />
                    </div>
                    <button type="submit">"ログイン"</button>
                </form>
            </crate::layout::AuthShell>
        }
    })
}

// ---------- MFA challenge ----------

pub fn render_mfa_challenge(
    flash: Option<Flash>,
    csrf_token: String,
    has_passkey: bool,
) -> String {
    render(move || {
        let csrf_for_totp = csrf_token.clone();
        let csrf_for_pk = csrf_token.clone();
        let passkey_block = if has_passkey {
            view! {
                <hr class="divider" />
                <p class="muted">"または、パスキーでサインイン:"</p>
                <form id="passkey-auth-form" method="post"
                      action="/admin/login/webauthn/start" class="stack">
                    <input type="hidden" name="_csrf" value=csrf_for_pk />
                    <button type="submit" class="secondary">"パスキーでサインイン"</button>
                </form>
                <script src="/static/webauthn.js"></script>
            }
            .into_any()
        } else {
            view! { <></> }.into_any()
        };
        view! {
            <crate::layout::AuthShell title="Verification required".to_string()>
                <h1>"確認コード"</h1>
                {flash_banner(flash)}
                <p class="muted">
                    "認証アプリの 6 桁コード、またはリカバリーコードを入力してください。"
                </p>
                <form method="post" action="/admin/login/mfa" class="stack">
                    <input type="hidden" name="_csrf" value=csrf_for_totp />
                    <div class="field">
                        <label for="code" class="field__label">"コード"</label>
                        <input id="code" name="code" type="text"
                               required=true autocomplete="one-time-code"
                               inputmode="text" autofocus=true />
                    </div>
                    <button type="submit">"確認"</button>
                </form>
                {passkey_block}
            </crate::layout::AuthShell>
        }
    })
}

// ---------- profile (MFA settings) ----------

pub struct ProfileData {
    pub username: String,
    /// True if TOTP is set up.
    pub totp_enabled: bool,
    /// Set when the user has just enrolled or regenerated codes.
    /// Displayed exactly once.
    pub fresh_recovery_codes: Option<Vec<String>>,
    /// Registered WebAuthn passkeys for this user.
    pub passkeys: Vec<PasskeyDescriptor>,
}

pub struct PasskeyDescriptor {
    pub id: String,
    pub nickname: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub fn render_profile(data: ProfileData, flash: Option<Flash>, csrf_token: String) -> String {
    render(move || {
        let ProfileData {
            username,
            totp_enabled,
            fresh_recovery_codes,
            passkeys,
        } = data;
        let csrf_for_disable = csrf_token.clone();
        let csrf_for_regen = csrf_token.clone();
        let csrf_for_enroll = csrf_token.clone();
        let csrf_for_passkey_register = csrf_token.clone();
        let csrf_for_passkey_delete = csrf_token.clone();
        let recovery_block = fresh_recovery_codes.map(|codes| {
            let lis: Vec<_> = codes
                .into_iter()
                .map(|c| view! { <li><span class="code">{c}</span></li> })
                .collect();
            view! {
                <div class="flash warn" role="status">
                    <div class="stack-tight">
                        <strong>"リカバリーコードを今すぐ保存してください。再表示はされません。"</strong>
                        <p class="muted">
                            "各コードは 1 度だけ使えます。安全な場所に保管してください。"
                            "認証アプリへのアクセスを失った場合、6 桁コードの代わりにこのいずれかを入力してサインインできます。"
                        </p>
                        <ol>{lis}</ol>
                    </div>
                </div>
            }
        });
        let mfa_section = if totp_enabled {
            view! {
                <div class="card">
                    <h3 class="card__title">"認証アプリ(TOTP)"</h3>
                    <p>
                        "状態:"
                        <span class="badge badge--ok" style="margin-left:var(--space-1)">"有効"</span>
                    </p>
                    <div class="card__footer">
                        <form method="post" action="/admin/profile/mfa/recovery-codes/regenerate"
                              style="display:inline">
                            <input type="hidden" name="_csrf" value=csrf_for_regen />
                            <button type="submit" class="secondary">"リカバリーコード再生成"</button>
                        </form>
                        <form method="post" action="/admin/profile/mfa/disable"
                              style="display:inline"
                              onsubmit="return confirm('認証アプリによる 2 段階認証を無効化しますか?');">
                            <input type="hidden" name="_csrf" value=csrf_for_disable />
                            <button type="submit" class="danger">"TOTP を無効化"</button>
                        </form>
                    </div>
                </div>
            }
            .into_any()
        } else {
            view! {
                <div class="card">
                    <h3 class="card__title">"認証アプリ(TOTP)"</h3>
                    <p>
                        "状態:"
                        <span class="badge badge--warn" style="margin-left:var(--space-1)">"未設定"</span>
                    </p>
                    <p class="muted">
                        "有効化すると、サインイン時に認証アプリの 6 桁コードが必要になります。"
                        "標準準拠の TOTP アプリならどれでも利用できます(Aegis / FreeOTP / Google Authenticator / 1Password など)。"
                    </p>
                    <div class="card__footer">
                        <form method="post" action="/admin/profile/mfa/enroll/start">
                            <input type="hidden" name="_csrf" value=csrf_for_enroll />
                            <button type="submit">"TOTP を設定"</button>
                        </form>
                    </div>
                </div>
            }
            .into_any()
        };
        let passkey_rows: Vec<_> = passkeys
            .into_iter()
            .map(|p| {
                let id = p.id.clone();
                let delete_url = format!("/admin/profile/webauthn/{id}/delete");
                let csrf = csrf_for_passkey_delete.clone();
                let last = p
                    .last_used_at
                    .map(fmt_time)
                    .unwrap_or_else(|| "(未使用)".into());
                view! {
                    <tr>
                        <td>{p.nickname}</td>
                        <td class="muted">{fmt_time(p.created_at)}</td>
                        <td class="muted">{last}</td>
                        <td>
                            <form method="post" action=delete_url style="display:inline"
                                  onsubmit="return confirm('このパスキーを削除しますか? このパスキーでのサインインができなくなります。');">
                                <input type="hidden" name="_csrf" value=csrf />
                                <button type="submit" class="danger">"削除"</button>
                            </form>
                        </td>
                    </tr>
                }
            })
            .collect();
        let passkey_table = if passkey_rows.is_empty() {
            view! {
                <p class="muted">"パスキーは未登録です。"</p>
            }
            .into_any()
        } else {
            view! {
                <div class="table-wrap">
                    <table>
                        <thead>
                            <tr>
                                <th>"名前"</th>
                                <th>"登録日"</th>
                                <th>"最終使用"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>{passkey_rows}</tbody>
                    </table>
                </div>
            }
            .into_any()
        };
        view! {
            <Shell title="Profile".to_string() show_nav=true current=Some("profile".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"プロフィール"</h1>
                        <p class="page-header__lede">
                            {format!("{username} のセキュリティ設定")}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {recovery_block}

                <section>
                    <h2>"2 段階認証"</h2>
                    {mfa_section}
                </section>

                <section>
                    <h2>"パスキー"</h2>
                    <p class="muted">
                        "パスキーは、スマートフォン・PC・セキュリティキー・パスワードマネージャに保存されるハードウェア裏付け資格情報です。"
                        "デバイスから外に出ません。複数登録できます — バックアップとして 2 つ以上登録しておくことを推奨します。"
                    </p>
                    {passkey_table}
                    <h3>"新しいパスキーを登録"</h3>
                    <div class="card">
                        <form id="passkey-register-form" method="post"
                              action="/admin/profile/webauthn/register/start" class="stack">
                            <input type="hidden" name="_csrf" value=csrf_for_passkey_register />
                            <div class="field">
                                <label for="pk-nickname" class="field__label">"ニックネーム"</label>
                                <input id="pk-nickname" name="nickname" type="text" required=true />
                                <span class="field__hint">"例: YubiKey 5C / MacBook Touch ID"</span>
                            </div>
                            <div>
                                <button type="submit">"パスキーを登録"</button>
                            </div>
                        </form>
                    </div>
                </section>
                <script src="/static/webauthn.js"></script>
            </Shell>
        }
    })
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

pub fn render_mfa_setup(data: MfaSetupData, flash: Option<Flash>, csrf_token: String) -> String {
    render(move || {
        let MfaSetupData { otpauth_uri, qr_svg, secret_b32 } = data;
        view! {
            <Shell title="Set up MFA".to_string() show_nav=true current=Some("profile".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"2 段階認証の設定"</h1>
                        <p class="page-header__lede">"認証アプリと sui-id を関連付けます。"</p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card">
                    <h3 class="card__title">"手順"</h3>
                    <ol>
                        <li>"認証アプリを開き、下の QR コードを読み取ってください。手入力する場合は秘密鍵をペーストしてください。"</li>
                        <li>"アプリに表示される 6 桁コードを以下のフォームに入力して確認してください。"</li>
                        <li>"設定完了後、1 度だけ使えるリカバリーコードが 8 個発行されます。安全な場所に保管してください。"</li>
                    </ol>
                </div>

                <div class="card">
                    <h3 class="card__title">"QR コードと秘密鍵"</h3>
                    <div inner_html=qr_svg style="max-width:240px;margin-bottom:var(--space-3)"></div>
                    <p>"秘密鍵:"<span class="code" style="margin-left:var(--space-1)">{secret_b32}</span></p>
                    <details>
                        <summary class="muted">"otpauth URI(上級者向け)"</summary>
                        <p><span class="code">{otpauth_uri}</span></p>
                    </details>
                </div>

                <div class="card">
                    <h3 class="card__title">"確認"</h3>
                    <form method="post" action="/admin/profile/mfa/enroll/confirm" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />
                        <div class="field">
                            <label for="code" class="field__label">"確認コード"</label>
                            <input id="code" name="code" type="text" required=true
                                   autocomplete="one-time-code" inputmode="text" autofocus=true />
                            <span class="field__hint">"アプリに表示されている 6 桁コード"</span>
                        </div>
                        <div>
                            <button type="submit">"確認して有効化"</button>
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

pub struct DashboardData {
    pub admin_username: String,
    pub user_count: usize,
    pub client_count: usize,
    pub issuer: String,
    pub sparkline: DashboardSparkline,
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
fn render_sparkline(buckets: Vec<DashboardSparkBucket>) -> impl IntoView {
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

            let title = format!(
                "{} : 成功 {} / 失敗 {}",
                b.label, b.success, b.failure
            );

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
             aria-label="サインイン活動のスパークライン"
             style="width:100%;height:80px;display:block">
            <line x1="0" y1=format!("{:.2}", HEIGHT - PAD_BOTTOM)
                  x2=format!("{WIDTH}") y2=format!("{:.2}", HEIGHT - PAD_BOTTOM)
                  stroke="var(--border-muted)"
                  stroke-width="1" />
            {bars}
        </svg>
    }
}

pub fn render_dashboard(data: DashboardData, flash: Option<Flash>) -> String {
    render(move || {
        let DashboardData {
            admin_username,
            user_count,
            client_count,
            issuer,
            sparkline,
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
        let svg = render_sparkline(sparkline.buckets);

        view! {
            <Shell title="Dashboard".to_string() show_nav=true current=Some("dashboard".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"ダッシュボード"</h1>
                        <p class="page-header__lede">
                            {format!("Hello, {admin_username}. システムの概要と統計情報。")}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                <section class="grid-cards" aria-label="統計サマリ">
                    <div class="card">
                        <div class="stat">
                            <span class="stat__value">{user_count.to_string()}</span>
                            <span class="stat__label">"ユーザー"</span>
                        </div>
                    </div>
                    <div class="card">
                        <div class="stat">
                            <span class="stat__value">{client_count.to_string()}</span>
                            <span class="stat__label">"クライアント"</span>
                        </div>
                    </div>
                    <div class="card">
                        <div class="stat">
                            <span class="stat__value">
                                <span class="badge badge--ok">"稼働中"</span>
                            </span>
                            <span class="stat__label">"サービス状態"</span>
                        </div>
                    </div>
                </section>

                <section>
                    <div class="row" style="justify-content:space-between;align-items:flex-end;margin-bottom:var(--space-3)">
                        <h2 style="margin:0">"サインイン活動"</h2>
                        <nav class="app-nav" aria-label="期間" style="flex:0 0 auto">
                            {range_tabs}
                        </nav>
                    </div>
                    <div class="card">
                        <div class="row" style="gap:var(--space-5);margin-bottom:var(--space-3)">
                            <div class="stat">
                                <span class="stat__value" style="color:var(--accent-default)">
                                    {total_success.to_string()}
                                </span>
                                <span class="stat__label">"成功"</span>
                            </div>
                            <div class="stat">
                                <span class="stat__value" style="color:var(--danger-default)">
                                    {total_failure.to_string()}
                                </span>
                                <span class="stat__label">"失敗"</span>
                            </div>
                        </div>
                        {svg}
                        <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                            "ホバーで各バケットの詳細を表示。"
                        </p>
                    </div>
                </section>

                <section>
                    <h2>"OIDC エンドポイント"</h2>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                <tr>
                                    <th scope="row">"Issuer"</th>
                                    <td><span class="code">{issuer}</span></td>
                                </tr>
                                <tr>
                                    <th scope="row">"Discovery"</th>
                                    <td><a href="/.well-known/openid-configuration"><span class="code">"/.well-known/openid-configuration"</span></a></td>
                                </tr>
                                <tr>
                                    <th scope="row">"JWKS"</th>
                                    <td><a href="/.well-known/jwks.json"><span class="code">"/.well-known/jwks.json"</span></a></td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}

// ---------- users ----------

fn user_row_view(u: UserSummary, current_user: String, csrf: String) -> impl IntoView {
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

    let status_badge = if is_deleted {
        view! { <span class="badge badge--danger">"deleted"</span> }.into_any()
    } else if is_disabled {
        view! { <span class="badge badge--warn">"disabled"</span> }.into_any()
    } else if is_admin {
        view! { <span class="badge badge--accent">"admin"</span> }.into_any()
    } else {
        view! { <span class="badge badge--ok">"active"</span> }.into_any()
    };

    let mfa_cell = if mfa_enabled {
        view! { <td><span class="badge badge--ok">"on"</span></td> }.into_any()
    } else {
        view! { <td><span class="muted">"off"</span></td> }.into_any()
    };

    let actions = if is_self {
        view! { <td><span class="muted">"(you)"</span></td> }.into_any()
    } else if is_deleted {
        view! { <td><span class="muted">"-"</span></td> }.into_any()
    } else {
        let reset_form = if mfa_enabled {
            view! {
                <form method="post" action=reset_mfa_url style="display:inline"
                      onsubmit="return confirm('Forcibly remove every MFA factor for this user (TOTP and all passkeys)? Use only when the user has lost access to their second factor.');">
                    <input type="hidden" name="_csrf" value=csrf_reset />
                    <button type="submit" class="secondary">"Reset MFA"</button>
                </form>
                " "
            }
            .into_any()
        } else {
            view! { <></> }.into_any()
        };
        view! {
            <td>
                <div class="row" style="gap:var(--space-1)">
                    {reset_form}
                    <form method="post" action=disabled_url style="display:inline">
                        <input type="hidden" name="_csrf" value=csrf_disable />
                        <input type="hidden" name="disabled" value=action_target />
                        <button type="submit" class="secondary">{action_label}</button>
                    </form>
                    <form method="post" action=delete_url style="display:inline"
                          onsubmit="return confirm('Permanently delete this user?');">
                        <input type="hidden" name="_csrf" value=csrf_delete />
                        <button type="submit" class="danger">"Delete"</button>
                    </form>
                </div>
            </td>
        }
        .into_any()
    };

    view! {
        <tr>
            <td><span class="code">{u.username}</span></td>
            <td>{display}</td>
            <td>{status_badge}</td>
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
) -> String {
    render(move || {
        let csrf_for_rows = csrf_token.clone();
        let csrf_for_form = csrf_token.clone();
        let user_count = users.len();
        let rows: Vec<_> = users
            .into_iter()
            .map(|u| user_row_view(u, current_user.clone(), csrf_for_rows.clone()))
            .collect();
        view! {
            <Shell title="Users".to_string() show_nav=true current=Some("users".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"ユーザー管理"</h1>
                        <p class="page-header__lede">
                            "ユーザーの作成・編集・管理。"
                            {format!(" 現在 {user_count} 名。")}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                <section>
                    <h2>"新しいユーザーを追加"</h2>
                    <div class="card">
                        <form method="post" action="/admin/users" class="stack">
                            <input type="hidden" name="_csrf" value=csrf_for_form />
                            <div class="field">
                                <label for="u-name" class="field__label">"ユーザー名"</label>
                                <input id="u-name" name="username" type="text"
                                       required=true autocomplete="off" />
                            </div>
                            <div class="field">
                                <label for="u-disp" class="field__label">"表示名(任意)"</label>
                                <input id="u-disp" name="display_name" type="text" autocomplete="off" />
                            </div>
                            <div class="field">
                                <label for="u-email" class="field__label">"メールアドレス(任意)"</label>
                                <input id="u-email" name="email" type="email" autocomplete="off" />
                            </div>
                            <div class="field">
                                <label for="u-pw" class="field__label">"パスワード(12 文字以上)"</label>
                                <input id="u-pw" name="password" type="password"
                                       required=true minlength="12" autocomplete="new-password" />
                            </div>
                            <label class="row" style="gap:var(--space-2)">
                                <input name="is_admin" type="checkbox" value="true" />
                                <span>"管理者権限を付与する"</span>
                            </label>
                            <div>
                                <button type="submit">"ユーザーを作成"</button>
                            </div>
                        </form>
                    </div>
                </section>

                <section>
                    <h2>"ユーザー一覧"</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>"ユーザー名"</th>
                                    <th>"表示名"</th>
                                    <th>"状態"</th>
                                    <th>"MFA"</th>
                                    <th>"作成日"</th>
                                    <th></th>
                                </tr>
                            </thead>
                            <tbody>{rows}</tbody>
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}

// ---------- clients ----------

fn client_row_view(c: ClientSummary, csrf: String) -> impl IntoView {
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
        "(any)".to_string()
    } else {
        c.allowed_scopes.clone()
    };
    let logout_count = c.post_logout_redirect_uris.len();
    let logout_display = if logout_count == 0 {
        "(falls back to redirect_uris)".to_string()
    } else {
        format!("{logout_count} URI(s)")
    };

    let status_badge = if is_deleted {
        view! { <span class="badge badge--danger">"deleted"</span> }.into_any()
    } else if is_disabled {
        view! { <span class="badge badge--warn">"disabled"</span> }.into_any()
    } else {
        view! { <span class="badge badge--ok">"active"</span> }.into_any()
    };

    let edit_url = format!("/admin/clients/{id_str}/edit");
    let actions = if is_deleted {
        view! { <td><span class="muted">"-"</span></td> }.into_any()
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
                    <form method="post" action=delete_url style="display:inline"
                          onsubmit="return confirm('Permanently delete this client and revoke its tokens?');">
                        <input type="hidden" name="_csrf" value=csrf_delete />
                        <button type="submit" class="danger">"Delete"</button>
                    </form>
                </div>
            </td>
        }
        .into_any()
    };

    view! {
        <tr>
            <td>{c.name}</td>
            <td><span class="code">{c.id.to_string()}</span></td>
            <td>{kind}</td>
            <td><span class="code">{scopes_display}</span></td>
            <td class="muted">{logout_display}</td>
            <td>{status_badge}</td>
            {actions}
        </tr>
    }
}

pub fn render_clients(
    clients: Vec<ClientSummary>,
    flash: Option<Flash>,
    new_secret: Option<(String, String)>,
    csrf_token: String,
) -> String {
    render(move || {
        let csrf_for_rows = csrf_token.clone();
        let csrf_for_form = csrf_token.clone();
        let client_count = clients.len();
        let secret_block = new_secret.map(|(cid, sec)| {
            view! {
                <div class="flash warn" role="status">
                    <div class="stack-tight">
                        <strong>"クライアント Secret は今だけ表示されます。安全な場所に保存してください。"</strong>
                        <div>"Client ID: "<span class="code">{cid}</span></div>
                        <div>"Client Secret: "<span class="code">{sec}</span></div>
                    </div>
                </div>
            }
        });
        let rows: Vec<_> = clients
            .into_iter()
            .map(|c| client_row_view(c, csrf_for_rows.clone()))
            .collect();
        view! {
            <Shell title="Clients".to_string() show_nav=true current=Some("clients".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"クライアント管理"</h1>
                        <p class="page-header__lede">
                            "OIDC クライアントの登録と管理。"
                            {format!(" 現在 {client_count} 件。")}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {secret_block}

                <section>
                    <h2>"新しいクライアントを登録"</h2>
                    <div class="card">
                        <form method="post" action="/admin/clients" class="stack">
                            <input type="hidden" name="_csrf" value=csrf_for_form />
                            <div class="field">
                                <label for="c-name" class="field__label">"アプリケーション名"</label>
                                <input id="c-name" name="name" type="text" required=true />
                            </div>
                            <div class="field">
                                <label for="c-uris" class="field__label">"Redirect URIs"</label>
                                <textarea id="c-uris" name="redirect_uris" required=true rows="3"></textarea>
                                <span class="field__hint">"1 行に 1 つ。https またはループバックの http のみ。"</span>
                            </div>
                            <div class="field">
                                <label for="c-scopes" class="field__label">"許可スコープ"</label>
                                <input id="c-scopes" name="allowed_scopes" type="text" value="openid profile" />
                                <span class="field__hint">"スペース区切り。デフォルトは openid profile。"</span>
                            </div>
                            <div class="field">
                                <label for="c-logout" class="field__label">"Post-logout redirect URIs(任意)"</label>
                                <textarea id="c-logout" name="post_logout_redirect_uris" rows="2"></textarea>
                                <span class="field__hint">"1 行に 1 つ。"</span>
                            </div>
                            <label class="row" style="gap:var(--space-2)">
                                <input name="confidential" type="checkbox" value="true" checked=true />
                                <span>"Confidential client(client secret を発行する)"</span>
                            </label>
                            <div>
                                <button type="submit">"登録"</button>
                            </div>
                        </form>
                    </div>
                </section>

                <section>
                    <h2>"登録済みクライアント"</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>"名前"</th>
                                    <th>"Client ID"</th>
                                    <th>"種別"</th>
                                    <th>"スコープ"</th>
                                    <th>"Logout URIs"</th>
                                    <th>"状態"</th>
                                    <th></th>
                                </tr>
                            </thead>
                            <tbody>{rows}</tbody>
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
}

pub fn render_client_edit(
    data: ClientEditData,
    flash: Option<Flash>,
    csrf_token: String,
) -> String {
    render(move || {
        let ClientEditData {
            id,
            name,
            redirect_uris,
            allowed_scopes,
            post_logout_redirect_uris,
            confidential,
            is_disabled,
        } = data;
        let post_url = format!("/admin/clients/{id}/edit");
        let kind = if confidential { "confidential" } else { "public" };
        let redirect_uris_value = redirect_uris.join("\n");
        let post_logout_value = post_logout_redirect_uris.join("\n");

        let status_badge = if is_disabled {
            view! { <span class="badge badge--warn">"disabled"</span> }.into_any()
        } else {
            view! { <span class="badge badge--ok">"active"</span> }.into_any()
        };

        view! {
            <Shell title="Edit client".to_string() show_nav=true current=Some("clients".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"クライアントを編集"</h1>
                        <p class="page-header__lede">{name.clone()}</p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card">
                    <h3 class="card__title">"基本情報"</h3>
                    <div class="stack-tight muted">
                        <div>"Client ID: "<span class="code">{id.clone()}</span></div>
                        <div class="row" style="gap:var(--space-2)">
                            <span>"種別:"</span>
                            <span class="badge badge--accent">{kind}</span>
                            <span>"状態:"</span>
                            {status_badge}
                        </div>
                    </div>
                    <p class="muted" style="margin-top:var(--space-3)">
                        "Client ID・種別(confidential/public)・client secret は作成時に固定されます。"
                        "これらを変更したい場合は削除して登録し直してください。"
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"設定"</h3>
                    <form method="post" action=post_url class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />
                        <div class="field">
                            <label for="e-name" class="field__label">"アプリケーション名"</label>
                            <input id="e-name" name="name" type="text" required=true value=name />
                        </div>
                        <div class="field">
                            <label for="e-uris" class="field__label">"Redirect URIs"</label>
                            <textarea id="e-uris" name="redirect_uris" required=true rows="3">
                                {redirect_uris_value}
                            </textarea>
                            <span class="field__hint">"1 行に 1 つ。https またはループバックの http のみ。"</span>
                        </div>
                        <div class="field">
                            <label for="e-scopes" class="field__label">"許可スコープ"</label>
                            <input id="e-scopes" name="allowed_scopes" type="text" value=allowed_scopes />
                            <span class="field__hint">"スペース区切り。空欄=制限なし。"</span>
                        </div>
                        <div class="field">
                            <label for="e-logout" class="field__label">"Post-logout redirect URIs"</label>
                            <textarea id="e-logout" name="post_logout_redirect_uris" rows="2">
                                {post_logout_value}
                            </textarea>
                            <span class="field__hint">"1 行に 1 つ。空欄=Redirect URIs を流用。"</span>
                        </div>
                        <div class="row">
                            <button type="submit">"保存"</button>
                            <a href="/admin/clients" class="button secondary">"キャンセル"</a>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}

// ---------- audit ----------

fn audit_row_view(e: AuditLogEntryDto) -> impl IntoView {
    let result_badge = match e.result.as_str() {
        "ok" => view! { <span class="badge badge--ok">"ok"</span> }.into_any(),
        "fail" | "error" | "denied" => {
            view! { <span class="badge badge--danger">{e.result.clone()}</span> }.into_any()
        }
        _ => view! { <span class="badge">{e.result.clone()}</span> }.into_any(),
    };
    view! {
        <tr>
            <td class="muted">{fmt_time(e.at)}</td>
            <td><span class="code">{e.actor.map(|a| a.to_string()).unwrap_or_else(|| "-".into())}</span></td>
            <td>{e.action}</td>
            <td><span class="code">{e.target.unwrap_or_default()}</span></td>
            <td>{result_badge}</td>
        </tr>
    }
}

pub fn render_audit(entries: Vec<AuditLogEntryDto>, flash: Option<Flash>) -> String {
    render(move || {
        let entry_count = entries.len();
        let rows: Vec<_> = entries.into_iter().map(audit_row_view).collect();
        view! {
            <Shell title="Audit".to_string() show_nav=true current=Some("audit".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"監査ログ"</h1>
                        <p class="page-header__lede">
                            "管理操作の履歴(新しい順)。"
                            {format!(" 直近 {entry_count} 件。")}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                <div class="table-wrap">
                    <table>
                        <thead>
                            <tr>
                                <th>"日時"</th>
                                <th>"実行者"</th>
                                <th>"操作"</th>
                                <th>"対象"</th>
                                <th>"結果"</th>
                            </tr>
                        </thead>
                        <tbody>{rows}</tbody>
                    </table>
                </div>
            </Shell>
        }
    })
}

// ---------- signing keys ----------

fn signing_key_row_view(
    k: sui_id_shared::api::SigningKeySummary,
    csrf: String,
) -> impl IntoView {
    let id_str = k.id.to_string();
    let id_for_url = id_str.clone();
    let id_for_display = id_str.clone();
    let status_badge = if k.is_active {
        view! { <span class="badge badge--ok">"active"</span> }.into_any()
    } else {
        view! { <span class="badge">"retired"</span> }.into_any()
    };
    let rotated = k
        .rotated_at
        .map(fmt_time)
        .unwrap_or_else(|| "-".to_string());
    let delete_url = format!("/admin/signing-keys/{id_for_url}/delete");
    let actions = if k.is_active {
        view! { <td><span class="muted">"(使用中)"</span></td> }.into_any()
    } else {
        view! {
            <td>
                <form method="post" action=delete_url style="display:inline"
                      onsubmit="return confirm('この退役キーを完全削除しますか? まだ有効期限内の発行済みトークンは検証に失敗します。');">
                    <input type="hidden" name="_csrf" value=csrf />
                    <button type="submit" class="danger">"削除"</button>
                </form>
            </td>
        }
        .into_any()
    };
    view! {
        <tr>
            <td><span class="code">{id_for_display}</span></td>
            <td>{k.algorithm}</td>
            <td>{status_badge}</td>
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
) -> String {
    render(move || {
        let csrf_for_rows = csrf_token.clone();
        let csrf_for_form = csrf_token.clone();
        let key_count = keys.len();
        let rows: Vec<_> = keys
            .into_iter()
            .map(|k| signing_key_row_view(k, csrf_for_rows.clone()))
            .collect();
        view! {
            <Shell
                title="Signing keys".to_string()
                show_nav=true
                current=Some("signing-keys".to_string())
            >
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"署名キー"</h1>
                        <p class="page-header__lede">
                            "JWT 署名用 Ed25519 キーの管理。"
                            {format!(" {key_count} 件登録。")}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card">
                    <h3 class="card__title">"キーローテーション"</h3>
                    <p class="muted">
                        "ローテーションを実行すると、新しい署名キーが発行され、現行キーは「退役」状態に遷移します。"
                        "退役キーは JWKS に残るため、有効期限内の既発行トークンは検証可能です。"
                        "それらが期限切れになった後、このページから安全に削除できます。"
                    </p>
                    <div class="card__footer">
                        <form method="post" action="/admin/signing-keys/rotate">
                            <input type="hidden" name="_csrf" value=csrf_for_form />
                            <button type="submit">"署名キーをローテーション"</button>
                        </form>
                    </div>
                </div>

                <section>
                    <h2>"全キー"</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>"Key ID"</th>
                                    <th>"アルゴリズム"</th>
                                    <th>"状態"</th>
                                    <th>"作成日"</th>
                                    <th>"退役日"</th>
                                    <th></th>
                                </tr>
                            </thead>
                            <tbody>{rows}</tbody>
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}

// ---------- error ----------

pub fn render_error(title: String, message: String, request_id: String) -> String {
    render(move || {
        let title2 = title.clone();
        view! {
            <crate::layout::AuthShell title=title.clone()>
                <h1>{title2}</h1>
                <div class="flash error" role="alert">{message}</div>
                <p class="muted">
                    "管理者に連絡する場合、以下の ID をお伝えください: "
                    <span class="code">{request_id}</span>
                </p>
                <p>
                    <a href="/" class="button secondary">"ホームへ戻る"</a>
                </p>
            </crate::layout::AuthShell>
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
) -> String {
    render(move || {
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
                            <span class="badge badge--accent">"current session"</span>
                        </td>
                    }
                    .into_any()
                } else {
                    let csrf_for_row = csrf_token.clone();
                    let post_url = format!("/me/security/sessions/{id}/revoke");
                    view! {
                        <td>
                            <form method="post" action=post_url style="display:inline"
                                  onsubmit="return confirm('このセッションをサインアウトしますか?');">
                                <input type="hidden" name="_csrf" value=csrf_for_row />
                                <button type="submit" class="secondary">"Revoke"</button>
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
                    <a href="/admin">"管理画面を開く →"</a>
                </p>
            }
        });

        let mfa_summary = if totp_enabled || passkey_count > 0 {
            let parts = {
                let mut v = Vec::<String>::new();
                if totp_enabled {
                    v.push("認証アプリ".into());
                }
                if passkey_count > 0 {
                    v.push(format!("パスキー {passkey_count} 件"));
                }
                v.join(" / ")
            };
            view! {
                <p>
                    "状態:"
                    <span class="badge badge--ok" style="margin-left:var(--space-1)">"有効"</span>
                    <span class="muted" style="margin-left:var(--space-2)">{parts}</span>
                </p>
            }
            .into_any()
        } else {
            view! {
                <div class="flash warn" role="status">
                    <div class="stack-tight">
                        <strong>"2 段階認証が無効です。"</strong>
                        <p class="muted" style="margin:0">
                            "現在このアカウントはパスワードのみで保護されています。"
                            "パスキーまたは認証アプリの登録を強く推奨します。"
                        </p>
                    </div>
                </div>
            }
            .into_any()
        };

        view! {
            <Shell title="Security".to_owned() show_nav=false current=None>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"アカウントセキュリティ"</h1>
                        <p class="page-header__lede">
                            <strong>{username}</strong>" としてサインイン中。"
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {admin_link}

                <section>
                    <h2>"2 段階認証"</h2>
                    <div class="card">
                        {mfa_summary}
                        <div class="card__footer">
                            <a href="/admin/profile" class="button secondary">
                                "認証手段を管理"
                            </a>
                            <a href="/me/security/password" class="button secondary">
                                "パスワードを変更"
                            </a>
                        </div>
                    </div>
                </section>

                <section>
                    <h2>"サインイン中の場所"</h2>
                    <p class="muted">
                        "1 行が 1 つのブラウザセッションです。"
                        "Revoke を押すとそのブラウザは即座にサインアウトされます。"
                        "「current session」とマークされているのは現在使用中のセッションです。"
                    </p>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>"開始日時"</th>
                                    <th>"期限"</th>
                                    <th>"要素"</th>
                                    <th></th>
                                </tr>
                            </thead>
                            <tbody>{session_rows}</tbody>
                        </table>
                    </div>
                    <form method="post" action="/me/security/sessions/revoke-all-others"
                          style="margin-top:var(--space-3)"
                          onsubmit="return confirm('現在のセッション以外をすべてサインアウトしますか?');">
                        <input type="hidden" name="_csrf" value=csrf_for_revoke_others />
                        <input type="hidden" name="current_session" value=current_session_id />
                        <button type="submit" class="secondary">
                            "他のすべてのセッションをサインアウト"
                        </button>
                    </form>
                </section>

                <section>
                    <h2>"最近のアクティビティ"</h2>
                    <p class="muted">
                        "あなたのアカウントに関わる認証および管理イベントです。"
                        "心当たりのない操作がある場合は、すぐにパスワードを変更し、他のセッションをサインアウトしてください。"
                    </p>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>"日時"</th>
                                    <th>"イベント"</th>
                                    <th>"結果"</th>
                                    <th>"備考"</th>
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
) -> String {
    render(move || {
        let PasswordChangeData {
            username,
            revoke_others_default,
        } = data;
        let revoke_attr = if revoke_others_default { Some("") } else { None };
        view! {
            <Shell title="Change password".to_owned() show_nav=false current=None>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"パスワードを変更"</h1>
                        <p class="page-header__lede">
                            <strong>{username}</strong>" としてサインイン中。"
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card" style="max-width:var(--content-narrow-width)">
                    <form method="post" action="/me/security/password"
                          autocomplete="off" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />

                        <div class="field">
                            <label for="current_password" class="field__label">"現在のパスワード"</label>
                            <input type="password" id="current_password" name="current_password"
                                   required autocomplete="current-password" />
                        </div>

                        <div class="field">
                            <label for="new_password" class="field__label">"新しいパスワード"</label>
                            <input type="password" id="new_password" name="new_password"
                                   required autocomplete="new-password" minlength="12" />
                            <span class="field__hint">
                                "12 文字以上。短く複雑なパスワードよりも、長くランダムなフレーズの方が安全です。"
                            </span>
                        </div>

                        <div class="field">
                            <label for="confirm_password" class="field__label">"新しいパスワード(確認)"</label>
                            <input type="password" id="confirm_password" name="confirm_password"
                                   required autocomplete="new-password" minlength="12" />
                        </div>

                        <div class="field">
                            <label class="row" style="gap:var(--space-2)">
                                <input type="checkbox" name="revoke_others" value="1"
                                       checked=revoke_attr />
                                <span>"パスワード変更後、他のブラウザ/アプリをサインアウトする"</span>
                            </label>
                            <span class="field__hint">
                                "推奨。既存のセッションやリフレッシュトークンが無効化され、新しいパスワードでの再サインインが必要になります。"
                            </span>
                        </div>

                        <div class="row">
                            <button type="submit">"パスワードを変更"</button>
                            <a href="/me/security" class="button secondary">"キャンセル"</a>
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
    Other,
}

impl SettingsTab {
    fn key(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Security => "security",
            Self::Authentication => "authentication",
            Self::Logs => "logs",
            Self::Other => "other",
        }
    }
}

fn settings_tabs(active: SettingsTab) -> impl IntoView {
    let items = [
        (SettingsTab::Basic, "基本", "/admin/settings/basic"),
        (SettingsTab::Security, "セキュリティ", "/admin/settings/security"),
        (
            SettingsTab::Authentication,
            "認証",
            "/admin/settings/authentication",
        ),
        (SettingsTab::Logs, "ログ", "/admin/settings/logs"),
        (SettingsTab::Other, "その他", "/admin/settings/other"),
    ];
    let active_key = active.key();
    let links: Vec<_> = items
        .into_iter()
        .map(|(t, label, href)| {
            let aria = if t.key() == active_key { Some("page") } else { None };
            view! {
                <a class="app-nav__link" href=href aria-current=aria>{label}</a>
            }
        })
        .collect();
    view! {
        <nav class="app-nav" aria-label="設定タブ" style="margin-bottom:var(--space-4);flex-wrap:wrap">
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

fn kv_bool_badge(k: &str, on: bool) -> impl IntoView {
    let badge = if on {
        view! { <span class="badge badge--ok">"有効"</span> }.into_any()
    } else {
        view! { <span class="badge">"無効"</span> }.into_any()
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
}

pub fn render_settings_basic(data: SettingsBasicData, flash: Option<Flash>) -> String {
    render(move || {
        let SettingsBasicData {
            issuer,
            listen_addr,
            cookie_secure,
            trusted_proxies,
            discovery_url,
            jwks_url,
        } = data;
        let proxies_display = if trusted_proxies.is_empty() {
            "(なし — peer の IP を直接信頼)".to_owned()
        } else {
            trusted_proxies.join(", ")
        };
        view! {
            <Shell title="設定 — 基本".to_string() show_nav=true current=Some("settings".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"設定"</h1>
                        <p class="page-header__lede">
                            "現在の有効な設定の確認。値の変更には sui-id.toml の編集と再起動が必要です。"
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Basic)}

                <div class="card">
                    <h3 class="card__title">"基本"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code("Issuer", issuer)}
                                {kv_code("Listen address", listen_addr)}
                                {kv_bool_badge("Cookie Secure フラグ", cookie_secure)}
                                {kv_text("Trusted proxies", proxies_display)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">"OIDC 公開エンドポイント"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                <tr>
                                    <th scope="row" style="width:14rem;font-weight:var(--font-weight-medium);color:var(--fg-muted);text-align:left">"Discovery"</th>
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
                                    <th scope="row" style="width:14rem;font-weight:var(--font-weight-medium);color:var(--fg-muted);text-align:left">"JWKS"</th>
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
}

pub fn render_settings_security(data: SettingsSecurityData, flash: Option<Flash>) -> String {
    render(move || {
        let SettingsSecurityData {
            max_lockout_label,
            hsts_enabled,
            csp_enabled,
            x_frame_deny,
            permissions_policy_minimal,
            cors_token_dynamic_from_clients,
            cors_public_endpoints_open,
        } = data;
        view! {
            <Shell title="設定 — セキュリティ".to_string() show_nav=true current=Some("settings".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"設定"</h1>
                        <p class="page-header__lede">
                            "現在の有効な設定の確認。値の変更には sui-id.toml の編集と再起動が必要です。"
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Security)}

                <div class="card">
                    <h3 class="card__title">"アカウントロックアウト"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code("最大ロックアウト時間", max_lockout_label)}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        "段階的バックオフの上限値。プログレッシブな失敗時間が積み重なってもこの値を超えません。"
                        "管理者は "<span class="code">"sui-id admin unlock-user"</span>" コマンドでいつでも解除できます。"
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"セキュリティヘッダー"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge("HSTS (Strict-Transport-Security)", hsts_enabled)}
                                {kv_bool_badge("Content-Security-Policy", csp_enabled)}
                                {kv_bool_badge("X-Frame-Options: DENY", x_frame_deny)}
                                {kv_bool_badge("Permissions-Policy(最小)", permissions_policy_minimal)}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        "管理画面はすべて上記ヘッダーを返します。"
                        "/oauth2/* 系の公開エンドポイントは仕様上の必要に応じて一部省略します。"
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"CORS"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge("Token endpoint の動的許可(登録 redirect_uris の origin)", cors_token_dynamic_from_clients)}
                                {kv_bool_badge("Discovery / JWKS / userinfo の公開許可(*)", cors_public_endpoints_open)}
                            </tbody>
                        </table>
                    </div>
                </div>
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

fn fmt_lifetime(secs: i64) -> String {
    if secs % 86400 == 0 {
        format!("{} 日 ({secs}s)", secs / 86400)
    } else if secs % 3600 == 0 {
        format!("{} 時間 ({secs}s)", secs / 3600)
    } else if secs % 60 == 0 {
        format!("{} 分 ({secs}s)", secs / 60)
    } else {
        format!("{secs} s")
    }
}

pub fn render_settings_authentication(
    data: SettingsAuthenticationData,
    flash: Option<Flash>,
) -> String {
    render(move || {
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
            <Shell title="設定 — 認証".to_string() show_nav=true current=Some("settings".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"設定"</h1>
                        <p class="page-header__lede">
                            "現在の有効な設定の確認。値の変更には sui-id.toml の編集と再起動が必要です。"
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Authentication)}

                <div class="card">
                    <h3 class="card__title">"パスワード"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_text("最小文字数", format!("{password_min_length} 文字"))}
                                {kv_text("ハッシュアルゴリズム", password_argon2id)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">"2 段階認証"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge("TOTP(認証アプリ)", totp_enabled_per_user)}
                                {kv_bool_badge("WebAuthn(パスキー)", webauthn_enabled_per_user)}
                                {kv_text("リカバリーコード(登録ごと)", format!("{recovery_codes_per_enrollment} 件"))}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        "ユーザー個別に有効化します。詳細は "
                        <a href="/admin/profile">"/admin/profile"</a>
                        " を参照してください。"
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"OAuth 2.1 / OIDC"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge("PKCE 必須(全 client、全 flow)", pkce_required)}
                                {kv_text("Access token 有効期限", fmt_lifetime(access_token_lifetime_secs))}
                                {kv_text("ID token 有効期限", fmt_lifetime(id_token_lifetime_secs))}
                                {kv_text("Refresh token 有効期限", fmt_lifetime(refresh_token_lifetime_secs))}
                                {kv_bool_badge("Refresh ローテーション", refresh_rotation)}
                                {kv_bool_badge("Refresh 盗難検知(family revoke)", refresh_theft_detection)}
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

pub fn render_settings_logs(data: SettingsLogsData, flash: Option<Flash>) -> String {
    render(move || {
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
            view! { <span class="badge badge--danger">"破損検知"</span> }.into_any()
        } else {
            view! { <span class="badge badge--ok">"正常"</span> }.into_any()
        };
        let chain_note = match chain_report.broken_at_seq {
            Some(seq) => format!("seq={seq} で不一致を検出。すぐに調査してください。"),
            None => format!(
                "末尾 {} 行を検査。レガシー(v0.17 以前)未ハッシュ行: {}",
                chain_report.checked, chain_report.legacy_unhashed
            ),
        };

        view! {
            <Shell title="設定 — ログ".to_string() show_nav=true current=Some("settings".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"設定"</h1>
                        <p class="page-header__lede">
                            "ログ出力設定と監査ログの状態。"
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Logs)}

                <div class="card">
                    <h3 class="card__title">"ログ出力"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code("Format", log_format)}
                                {kv_code("Filter", log_filter)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">"直近 24 時間のイベント"</h3>
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
                        "詳細な履歴は "<a href="/admin/audit">"/admin/audit"</a>" を参照してください。"
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"監査ログのハッシュチェーン"</h3>
                    <div class="row" style="gap:var(--space-3);align-items:center">
                        <span>"状態:"</span>
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

pub fn render_settings_other(data: SettingsOtherData, flash: Option<Flash>) -> String {
    render(move || {
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
            <Shell title="設定 — その他".to_string() show_nav=true current=Some("settings".to_string())>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">"設定"</h1>
                        <p class="page-header__lede">
                            "ビルド情報・スキーマ・ストレージ。"
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Other)}

                <div class="card">
                    <h3 class="card__title">"ビルド"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code("sui-id バージョン", binary_version)}
                                {kv_text("対応スキーマバージョン", schema_version.to_string())}
                                {kv_code("サーバ時刻", now_str)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">"ストレージ"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code("DB ファイル", db_path)}
                                {kv_code("マスターキーファイル", master_key_file)}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted" style="margin-top:var(--space-2);margin-bottom:0">
                        "DB は単一の SQLite ファイル、マスターキーは環境変数 "
                        <span class="code">"SUI_ID_MASTER_KEY"</span>
                        " が指定されない場合のみキーファイルから読み込まれます。"
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"レコード数"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                <tr>
                                    <th scope="row" style="width:14rem;font-weight:var(--font-weight-medium);color:var(--fg-muted);text-align:left">"ユーザー"</th>
                                    <td>
                                        {user_count.to_string()}" 名 "
                                        <a href="/admin/users" class="muted" style="margin-left:var(--space-2)">
                                            "管理 →"
                                        </a>
                                    </td>
                                </tr>
                                <tr>
                                    <th scope="row" style="width:14rem;font-weight:var(--font-weight-medium);color:var(--fg-muted);text-align:left">"クライアント"</th>
                                    <td>
                                        {client_count.to_string()}" 件 "
                                        <a href="/admin/clients" class="muted" style="margin-left:var(--space-2)">
                                            "管理 →"
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
