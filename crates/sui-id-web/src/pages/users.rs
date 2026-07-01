//! Page renderers for the "users" screen domain (RFC 065).

use super::audit::audit_row_view;
use super::common::*;
use crate::layout::Shell;
use leptos::prelude::*;
use sui_id_shared::api::UserSummary;

fn user_row_view(
    t: &'static sui_id_i18n::Strings,
    u: UserSummary,
    current_user: String,
) -> impl IntoView {
    let id_str = u.id.to_string();
    let is_self = u.username == current_user;
    let is_deleted = u.is_deleted;
    let is_disabled = u.is_disabled;
    let is_admin = u.is_admin;
    let mfa_enabled = u.mfa_enabled;

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
        view! { <td>{crate::components::status_badge(t, crate::components::StatusKind::On)}</td> }
            .into_any()
    } else {
        view! { <td><span class="muted">{t.status_off}</span></td> }.into_any()
    };

    let detail_cell = if is_self {
        view! { <td><span class="muted">"(you)"</span></td> }.into_any()
    } else if is_deleted {
        view! { <td><span class="muted">{t.empty_dash}</span></td> }.into_any()
    } else {
        let detail_url = format!("/admin/users/{id_str}");
        view! { <td><a href=detail_url class="button secondary">{t.button_view_detail}</a></td> }
            .into_any()
    };

    view! {
        <tr>
            <td><span class="code">{u.username}</span></td>
            <td>{status_view}</td>
            {mfa_cell}
            {detail_cell}
        </tr>
    }
}

pub fn render_users(
    can_write: bool,
    users: Vec<UserSummary>,
    flash: Option<Flash>,
    current_user: String,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let user_count = users.len();
        let rows: Vec<_> = users
            .into_iter()
            .map(|u| user_row_view(t, u, current_user.clone()))
            .collect();
        view! {
            <Shell title=t.users_title.to_string() show_nav=true current=Some("users".to_string()) dev_mode=dev_mode lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.users_title}</h1>
                        <p class="page-header__lede">
                            {t.users_lede}
                            " "
                            {(t.users_count_caption)(user_count)}
                        </p>
                    </div>
                    // RFC 071: add button only for admins (auditors can view, not create).
                    {can_write.then(|| view! {
                        <div class="page-header__actions">
                            <a href="/admin/users/new" class="button button--icon"
                               aria-label=t.users_create_section>
                                "+"
                            </a>
                        </div>
                    })}
                </header>
                {flash_banner(flash)}

                <section>
                    <h2>{t.users_table_section}</h2>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.login_username_label}</th>
                                    <th>{t.users_table_th_status}</th>
                                    <th>{t.users_table_th_mfa}</th>
                                    <th></th>
                                </tr>
                            </thead>
                            {if rows.is_empty() {
                                view! {
                                    <tbody>{table_empty_row(t.users_empty, 4)}</tbody>
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

// ---------- users/new ----------

pub fn render_users_new(
    flash: Option<Flash>,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <Shell title=t.users_create_section.to_string() show_nav=true current=Some("users".to_string()) dev_mode=dev_mode lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.users_create_section}</h1>
                    </div>
                    <div class="page-header__actions">
                        <a href="/admin/users" class="button secondary">{t.button_cancel}</a>
                    </div>
                </header>
                {flash_banner(flash)}
                <div class="card">
                    <form method="post" action="/admin/users" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />
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
                        <div class="form-actions">
                            <button type="submit">{t.users_create_button}</button>
                            <a href="/admin/users" class="button secondary">{t.button_cancel}</a>
                        </div>
                    </form>
                </div>
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
    /// RFC 071: explicit role for display and the role-change form.
    pub role: sui_id_store::models::Role,
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

pub fn render_user_detail(
    can_write: bool,
    data: UserDetailData,
    lang: sui_id_i18n::Locale,
) -> String {
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

        let session_rows: Vec<_> = data
            .sessions
            .iter()
            .map(|s| {
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
            })
            .collect();

        let audit_rows: Vec<_> = data
            .recent_audit
            .iter()
            .map(|e| audit_row_view(t, e.clone()))
            .collect();

        let disable_confirm_url = format!("/admin/users/{uid}/disable-confirm");
        let delete_confirm_url = format!("/admin/users/{uid}/delete-confirm");
        let reset_mfa_confirm_url = format!("/admin/users/{uid}/mfa-reset-confirm");

        view! {
            <Shell title=username.clone() show_nav=true
                   current=Some("users".to_string())
                   dev_mode=data.dev_mode lang=lang
                   csrf_token=data.csrf_token.clone()>
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

                // RFC-MI-051 danger zone; RFC 071: hidden for auditors.
                {can_write.then(|| {
                    // RFC 071: role-change section (above the danger zone so the
                    // flow is: identity → security → access → danger).
                    let current_role = data.role;
                    let uid_for_role = uid.clone();
                    let csrf_for_role = data.csrf_token.clone();
                    view! {
                        <section class="form-section">
                            <h2 class="form-section__title">{t.user_detail_role_section}</h2>
                            <form method="post"
                                  action=format!("/admin/users/{}/role", uid_for_role)
                                  class="form-actions">
                                <input type="hidden" name="_csrf" value=csrf_for_role />
                                <select name="role" aria-label=t.user_detail_role_section>
                                    <option value="admin"
                                        selected=move || current_role == sui_id_store::models::Role::Admin>
                                        {t.role_admin}
                                    </option>
                                    <option value="auditor"
                                        selected=move || current_role == sui_id_store::models::Role::Auditor>
                                        {t.role_auditor}
                                    </option>
                                    <option value="user"
                                        selected=move || current_role == sui_id_store::models::Role::User>
                                        {t.role_user}
                                    </option>
                                </select>
                                <button type="submit">{t.user_detail_role_change}</button>
                            </form>
                        </section>

                        <section class="danger-zone">
                            <h2 class="danger-zone__title">
                                "⚠ " {t.danger_zone_title}
                            </h2>
                            <p class="danger-zone__body">{t.user_detail_danger_zone_body}</p>
                            <div class="form-actions">
                                {data.totp_enabled.then(|| view! {
                                    <a href=reset_mfa_confirm_url class="button secondary">
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
                        </section>
                    }
                })}
            </Shell>
        }
    })
}
