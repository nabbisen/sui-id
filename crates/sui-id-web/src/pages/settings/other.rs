//! Settings other tab (RFC 065).

use super::super::common::*;
use super::*;
use crate::layout::Shell;

pub struct SettingsOtherData {
    pub binary_version: String,
    pub schema_version: i32,
    pub db_path: String,
    pub master_key_file: String,
    pub user_count: usize,
    pub client_count: usize,
    pub clock_now: chrono::DateTime<chrono::Utc>,
}

pub fn render_settings_other(
    data: SettingsOtherData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
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
            <Shell title=t.settings_title_advanced.to_string() show_nav=true current=Some("settings".to_string()) lang=lang csrf_token=csrf_token.clone()>
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
                    <p class="muted mt-2-mb-0">
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
                                    <th scope="row" class="kv-label-cell">{t.dashboard_stat_users}</th>
                                    <td>
                                        {(t.settings_advanced_users_count)(user_count)}
                                        <a href="/admin/users" class="muted ml-2">
                                            {t.settings_advanced_manage_link}
                                        </a>
                                    </td>
                                </tr>
                                <tr>
                                    <th scope="row" class="kv-label-cell">{t.dashboard_stat_clients}</th>
                                    <td>
                                        {(t.settings_advanced_clients_count)(client_count)}
                                        <a href="/admin/clients" class="muted ml-2">
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
