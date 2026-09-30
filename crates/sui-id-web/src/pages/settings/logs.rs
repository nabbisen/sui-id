//! Settings logs tab (RFC 065).

use super::*;
use crate::layout::Shell;

pub struct SettingsLogsData {
    pub log_format: String,
    pub log_filter: String,
    pub login_success_24h: i64,
    pub login_failure_24h: i64,
    pub login_locked_24h: i64,
    pub password_changed_self_24h: i64,
    pub chain_report: ChainStatus,
    /// RFC 088: false for auditors → render static rows instead of form.
    pub can_write: bool,
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
            can_write: _can_write,
        } = data;

        // RFC 121 D1, D2: read from the one shared place, so this page
        // cannot describe an outcome differently from the audit page.
        let (kind, chain_note) = chain_status_words(t, &chain_report);
        let chain_badge = crate::components::status_badge(t, kind).into_any();

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
                                {kv_text("auth.lockout", login_locked_24h.to_string())}
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
