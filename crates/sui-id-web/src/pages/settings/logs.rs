//! Settings logs tab (RFC 065).

use super::super::common::*;
use super::*;
use crate::layout::Shell;

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

pub fn render_settings_logs(
    data: SettingsLogsData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
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
            <Shell title=t.settings_title_logs.to_string() show_nav=true current=Some("settings".to_string()) lang=lang csrf_token=csrf_token.clone()>
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
                    <p class="muted mt-2-mb-0">
                        {t.settings_logs_audit_link_prefix}
                        <a href="/admin/audit">"/admin/audit"</a>
                        {t.settings_logs_audit_link_suffix}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_logs_audit_section}</h3>
                    <div class="row row-gap3-center">
                        <span>{t.client_edit_label_status}":"</span>
                        {chain_badge}
                    </div>
                    <p class="muted mt-2-mb-0">
                        {chain_note}
                    </p>
                </div>
            </Shell>
        }
    })
}
