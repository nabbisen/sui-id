//! Page renderers for the "users" screen domain (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::common::*;
use super::audit::audit_row_view;
use sui_id_shared::api::UserSummary;

fn user_row_view(
    t: &'static sui_id_i18n::Strings,
    u: UserSummary,
    current_user: String,
    _csrf: String,
) -> impl IntoView {
    let display = u.display_name.clone().unwrap_or_default();
    let id_str = u.id.to_string();
    let is_self = u.username == current_user;
    let is_disabled = u.is_disabled;
    let is_deleted = u.is_deleted;
    let is_admin = u.is_admin;
    let mfa_enabled = u.mfa_enabled;
    let action_label = if is_disabled { "Enable" } else { "Disable" };
    // RFC 030/058 routed every dangerous action through a separate
    // confirm screen; the older `let disabled_url`/`let delete_url`/
    // `let reset_mfa_url` form-action vars are no longer used and
    // were trimmed during the RFC 065 split.

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
                <div class="row gap-1">
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
                            <label class="row gap-2">
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
                                    <tbody>{table_empty_row(t.users_empty, 6)}</tbody>
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
                <div class="mb-3">
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
                            <p class="muted text-caption">{email}</p>
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

                <section class="card mb-4">
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

                <section class="mb-4">
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
                                    <tbody><tr><td colspan="3" class="muted center-pad-4">
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
                                    <tbody><tr><td colspan="3" class="muted center-pad-4">
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
