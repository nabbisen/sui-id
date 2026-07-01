//! Chinese Simplified (`zh-Hans`) translation table and date/number formatters.
//!
//! **Translator guide:**
//! Edit the string values between `\"…\"` only. Do not rename field names.
//! Every field must be present — the compiler enforces completeness.
//! After editing, run `cargo test -p sui-id-i18n` to confirm all tests pass.
//!
//! Target: Mainland Simplified Chinese (zh-Hans / zh-CN).
//! Use standard Mainland Mandarin conventions and Simplified script (简体).

use crate::formatters::{Formatters, fmt_count_shared, fmt_time_shared};
use crate::strings::Strings;
use chrono::{DateTime, Datelike, Utc};

// ── Strings ──────────────────────────────────────────────────────────────────

pub static STRINGS_ZH_HANS: Strings = Strings {
    // Generic UI
    button_save: "保存",
    button_cancel: "取消",
    button_back: "返回",
    button_edit: "编辑",
    button_view_detail: "详情",
    button_continue: "继续",
    button_delete: "删除",
    danger_zone_title: "危险操作",
    button_create: "创建",
    button_confirm: "确认",
    button_test: "测试",
    badge_enabled: "已启用",
    badge_disabled: "已禁用",
    badge_ok: "正常",
    badge_warn: "警告",
    badge_error: "错误",
    label_optional: "（可选）",
    label_required: "（必填）",
    muted_none: "（无）",

    // Language native names (RFC 051) — identical across all locales.
    locale_native_ja: "日本語",
    locale_native_en: "English",
    locale_native_zh_hans: "中文（简体）",
    locale_native_zh_hant: "中文（繁体）",

    // Lifetime formatting (RFC 051)
    fmt_lifetime_days: |n, secs| format!("{n} 天 ({secs}s)"),
    fmt_lifetime_hours: |n, secs| format!("{n} 小时 ({secs}s)"),
    fmt_lifetime_minutes: |n, secs| format!("{n} 分钟 ({secs}s)"),

    // Settings: Auth extensions (RFC 051)
    settings_auth_min_length_value: |n| format!("{n} 个字符"),
    settings_auth_recovery_codes_label: "恢复码（每次注册）",
    settings_auth_recovery_codes_value: |n| format!("{n} 个"),
    settings_auth_mfa_note_prefix: "按用户启用。详情请参见 ",
    settings_auth_mfa_note_suffix: "。",

    // Settings: Logs extensions (RFC 051)
    settings_logs_lede: "日志输出设置与审计日志状态。",
    settings_logs_kv_format: "Format",
    settings_logs_kv_filter: "Filter",
    settings_logs_audit_link_prefix: "详细历史请参见 ",
    settings_logs_audit_link_suffix: "。",

    // Settings: Advanced/Other extensions (RFC 051)
    settings_advanced_lede: "构建信息、模式与存储。",
    settings_advanced_storage_note_prefix: "数据库为单一 SQLite 文件。主密钥仅在环境变量 ",
    settings_advanced_storage_note_suffix: " 未设置时才从密钥文件读取。",
    settings_advanced_users_count: |n| format!("{n} 名 "),
    settings_advanced_clients_count: |n| format!("{n} 个 "),

    // Settings: Email extensions (RFC 051)
    settings_email_enable_checkbox: "启用邮件功能",
    settings_email_enable_hint: "关闭时，忘记密码等邮件发送端点不可用。",
    settings_email_password_placeholder_change: "（仅修改时填写）",
    settings_email_password_placeholder_none: "（无需认证时留空）",
    settings_email_password_hint: "仅在修改保存的密码时填写。留空时保留原有值。",
    settings_email_base_url_hint: "重置邮件中使用的 URL 基址。可与 Issuer URL 不同。",
    settings_email_save_button: "保存设置",
    settings_email_test_section: "连接测试",
    settings_email_test_lede: "使用当前设置尝试连接并认证 SMTP 服务器。不会发送邮件。",

    // Navigation
    nav_dashboard: "仪表盘",
    nav_users: "用户",
    nav_clients: "客户端",
    nav_signing_keys: "签名密钥",
    nav_audit: "审计日志",
    nav_settings: "设置",
    nav_profile: "个人资料",
    nav_apps: "应用",
    nav_my_account: "我的账户",
    settings_tab_general: "通用",
    settings_tab_advanced: "高级",
    me_overview_last_login: "您上次登录时间：{date}。",
    me_overview_first_login: "欢迎 — 这是您的首次登录。",
    nav_security: "安全",
    nav_logout: "退出登录",
    a11y_skip_to_main: "跳转到主要内容",
    nav_aria_main: "主导航",
    nav_aria_signout: "退出登录",

    // Footer (RFC 050)
    footer_tagline: "🌱 sui-id · 安静、可靠的身份认证基础。",
    footer_a11y_group_label: "无障碍功能",
    a11y_keyboard: "支持键盘操作",
    a11y_screen_reader: "支持屏幕阅读器",
    a11y_contrast: "支持高对比度",

    // Theme toggle (RFC 050)
    theme_toggle_group: "主题",
    theme_toggle_light: "浅色",
    theme_toggle_auto: "自动",
    theme_toggle_dark: "深色",
    theme_toggle_light_title: "浅色主题",
    theme_toggle_auto_title: "跟随系统",
    theme_toggle_dark_title: "深色主题",

    // Status words (RFC 052)
    status_active: "启用",
    status_disabled: "已禁用",
    status_deleted: "已删除",
    status_admin: "管理员",
    status_on: "开",
    status_off: "关",
    status_in_use: "使用中",
    status_retired: "已退役",
    status_published: "已发布",
    status_pending: "待处理",
    status_healthy: "正常",
    status_unhealthy: "异常",

    // Empty placeholders (RFC 052)
    empty_dash: "—",
    empty_any: "（全部）",
    empty_none: "（无）",
    empty_falls_back_redirect_uris: "（回退到 redirect_uris）",
    empty_no_email: "（无邮箱）",
    empty_not_set: "（未设置）",

    // Copy-to-clipboard button (RFC 053)
    copy_button_label: "📋 复制",
    copy_button_label_done: "✓ 已复制",
    copy_button_aria_template: "复制 {noun}",
    copy_noun_client_id: "Client ID",
    copy_noun_client_secret: "客户端密钥",
    copy_noun_jwks_uri: "JWKS URI",
    copy_noun_redirect_uri: "重定向 URI",
    copy_noun_audit_row_id: "审计条目 ID",
    copy_noun_setup_token: "初始化令牌",
    copy_noun_recovery_code: "恢复码",
    copy_noun_passkey_id: "通行密钥 ID",

    // Login
    signed_out_flash: "已退出登录。",
    login_title: "登录",
    login_username_label: "用户名",
    login_password_label: "密码",
    login_submit: "登录",
    login_passkey_button: "使用通行密钥登录",
    login_no_admin_access: "此帐户无权访问管理面板。",
    login_passkey_primary: "使用通行密钥登录",
    login_forgot_password_link: "忘记密码？",
    login_invalid_credentials: "用户名或密码不正确。",
    login_account_locked: "您的账户暂时被锁定，请稍后再试。",
    login_reset_ok_banner: "密码已重置，请使用新密码登录。",

    // Setup wizard
    setup_step_welcome: "欢迎",
    setup_step_admin: "创建管理员",
    setup_step_done: "完成",
    setup_welcome_title: "欢迎使用 sui-id",
    setup_welcome_lede: "此服务器尚未初始化，初始设置只需几分钟。",
    setup_welcome_lede2: "下一步将创建第一个管理员账户。请准备好服务器启动时打印的初始令牌。",
    setup_welcome_begin: "开始设置",
    setup_admin_title: "创建管理员账户",
    setup_admin_lede: "请输入服务器启动时生成的初始令牌，以及新管理员账户的详细信息。",
    setup_admin_token_label: "初始令牌",
    setup_admin_token_hint: "仅在启动日志中打印一次",
    setup_admin_username_label: "用户名",
    setup_admin_email_label: "邮箱地址（可选）",
    setup_admin_email_hint: "用于通知和密码重置，可以稍后更改。",
    setup_admin_display_label: "显示名称（可选）",
    setup_admin_password_label: "密码",
    setup_admin_password_hint: "12 个字符或以上",
    setup_admin_confirm_label: "确认密码",
    setup_admin_submit: "创建管理员",
    setup_done_title: "设置完成",
    setup_done_lede: "管理员账户已创建，初始签名密钥已生成，您现在可以在管理控制台中查看系统配置。",
    setup_done_next_steps_title: "后续步骤",
    setup_done_next_step_register_clients: "注册 OIDC 客户端。",
    setup_done_next_step_enable_mfa: "使用通行密钥或认证器应用启用双重认证。",
    setup_done_next_step_review_settings: "在设置选项卡中查看当前有效配置。",
    setup_done_enter_admin: "进入管理控制台",
    setup_not_complete_title: "设置未完成",
    setup_not_complete_lede: "管理员账户尚未创建，请从头开始设置。",
    setup_password_mismatch: "密码与确认密码不一致。",
    setup_already_initialized: "服务器已经初始化。",
    setup_invalid_token: "初始令牌不正确。",
    setup_hibp_blocked: "此密码出现在已知数据泄露中，请选择其他密码。",
    setup_generic_failure: "设置失败，请检查表单并重试。",

    // RFC 012: Setup wizard — Language step
    setup_step_lang: "语言",
    setup_lang_title: "显示语言",
    setup_lang_lede: "选择管理面板和登录页面的语言，可在设置中随时更改。",
    setup_lang_field_label: "显示语言",
    setup_lang_default_note: "可在管理面板设置中随时更改。",
    setup_lang_submit: "下一步",

    // RFC 012: Setup wizard — HIBP step
    setup_step_hibp: "安全",
    setup_hibp_step_title: "密码安全策略",
    setup_hibp_step_lede: "使用 Have I Been Pwned 检测已知泄露的密码，仅发送 SHA-1 哈希的前 5 个字符，隐私得到保护。",
    setup_hibp_option_off: "关闭",
    setup_hibp_option_off_desc: "不进行泄露检查。",
    setup_hibp_option_warn: "警告（推荐）",
    setup_hibp_option_warn_desc: "对泄露密码发出警告，但仍允许使用。",
    setup_hibp_option_block: "阻止",
    setup_hibp_option_block_desc: "拒绝出现在已知泄露数据中的密码。",
    setup_hibp_step_default_note: "可在管理面板设置中随时更改。",
    setup_hibp_step_submit: "下一步",

    // Step-up auth
    step_up_title: "重新验证身份",
    step_up_lede: "执行此安全敏感操作之前，请使用认证器应用的验证码确认您的身份，有效期 5 分钟。",
    step_up_code_label: "确认码",
    step_up_code_hint: "认证器应用的 6 位验证码，或恢复码。",
    step_up_code_invalid: "验证码无效，请重试。",
    step_up_passkey_alt: "或使用通行密钥重新验证：",
    step_up_passkey_button: "使用通行密钥重新验证",

    // MFA challenge (login flow)
    mfa_challenge_shell_title: "需要验证",
    mfa_challenge_title: "验证码",
    mfa_challenge_lede: "请输入认证器应用的 6 位验证码，或其中一个恢复码。",
    mfa_challenge_code_label: "验证码",
    mfa_challenge_submit: "验证",
    mfa_challenge_passkey_alt: "或使用通行密钥登录：",
    mfa_challenge_passkey_button: "使用通行密钥登录",
    mfa_challenge_failed_flash: "验证失败，请重试或使用恢复码。",

    // Profile / self-service security
    profile_subtitle_template: "{username} 的安全设置",
    profile_recovery_save_now: "立即保存这些恢复码，它们不会再次显示。",
    profile_recovery_save_lede: "每个恢复码只能使用一次，请妥善保存。如果无法访问认证器应用，可以用任意一个恢复码代替 6 位验证码登录。",
    profile_mfa_section: "双重认证",
    profile_mfa_totp_card_title: "认证器应用（TOTP）",
    profile_mfa_status_label: "状态：",
    profile_mfa_status_enabled: "已启用",
    profile_mfa_status_not_configured: "未配置",
    profile_mfa_regenerate_codes: "重新生成恢复码",
    profile_mfa_disable_confirm: "禁用认证器应用双重认证？",
    profile_mfa_disable_button: "禁用 TOTP",
    profile_mfa_enroll_lede: "启用后，登录时还需要认证器应用的 6 位验证码。",
    profile_mfa_enroll_apps_note: "任何标准 TOTP 应用均可使用（Aegis / FreeOTP / Google Authenticator / 1Password 等）。",
    profile_mfa_enroll_button: "设置 TOTP",
    profile_passkeys_section: "通行密钥",
    profile_passkeys_lede: "通行密钥是存储在手机、电脑、安全密钥或密码管理器中的硬件凭证，永不离开设备。可注册多个，建议至少两个作为备份。",
    profile_passkeys_th_name: "名称",
    profile_passkeys_th_registered: "注册时间",
    profile_passkeys_th_last_used: "最后使用",
    profile_passkeys_last_used_never: "（从未使用）",
    profile_passkeys_delete_confirm: "删除此通行密钥？删除后将无法再用它登录。",
    profile_passkeys_delete_button: "删除",
    profile_passkeys_empty: "未注册通行密钥。",
    profile_passkeys_register_section: "注册新通行密钥",
    profile_passkeys_nickname_label: "昵称",
    profile_passkeys_nickname_hint: "例如：YubiKey 5C / MacBook Touch ID",
    profile_passkeys_register_button: "注册通行密钥",
    profile_lang_section: "显示语言",
    profile_lang_lede: "sui-id 登录后使用的语言。浏览器默认将根据浏览器的 Accept-Language 设置自动选择。",
    profile_lang_field_label: "语言",
    profile_mfa_enrolled_flash: "双重认证已启用。",
    profile_recovery_regenerated_flash: "恢复码已重新生成，请保存新恢复码——旧的已失效。",

    // MFA setup (TOTP enrollment)
    mfa_setup_shell_title: "设置双重认证",
    mfa_setup_title: "设置双重认证",
    mfa_setup_lede: "将认证器应用与 sui-id 关联。",
    mfa_setup_steps_title: "步骤",
    mfa_setup_step1: "打开认证器应用，扫描下方二维码。如需手动输入密钥，请从下方字段复制。",
    mfa_setup_step2: "在下方表单中输入应用显示的 6 位验证码以完成关联。",
    mfa_setup_step3: "确认后，sui-id 将生成 8 个一次性恢复码，请妥善保存。",
    mfa_setup_qr_card_title: "二维码与密钥",
    mfa_setup_secret_label: "密钥：",
    mfa_setup_otpauth_summary: "otpauth URI（高级）",
    mfa_setup_verify_card_title: "验证",
    mfa_setup_code_label: "验证码",
    mfa_setup_code_hint: "应用显示的 6 位验证码",
    mfa_setup_confirm_button: "确认并启用",

    // Forgot password
    forgot_password_title: "忘记密码",
    forgot_password_lede: "请输入注册时使用的邮箱地址，如果账户存在，我们将发送重置链接。",
    forgot_password_email_label: "邮箱地址",
    forgot_password_submit: "发送重置链接",
    forgot_password_sent_title: "邮件已发送",
    forgot_password_sent_lede: "请求已收到。如果您提供的地址存在账户，我们已发送重置链接。",
    forgot_password_sent_lede2: "链接有效期为 30 分钟，如果收件箱中没有，请检查垃圾邮件文件夹。",
    reset_password_title: "重置密码",
    reset_password_lede: "请输入两次新密码。",
    reset_password_new_label: "新密码",
    reset_password_new_hint: "12 个字符或以上",
    reset_password_confirm_label: "确认密码",
    reset_password_submit: "更改密码",
    reset_password_invalid_title: "此链接已失效",
    reset_password_invalid_lede: "重置链接已过期、已被使用或无效。",
    reset_password_invalid_request_again: "重新申请链接",
    password_mismatch_flash: "密码与确认密码不一致。",
    reset_password_failed_flash: "密码重置失败，请重试。",
    back_to_login: "返回登录",

    // Settings hub
    settings_title: "设置",
    settings_lede: "查看当前有效配置。",
    settings_tab_basic: "基本",
    settings_tab_security: "安全",
    settings_tab_authentication: "认证",
    settings_tab_logs: "日志",
    settings_tab_email: "邮件",
    settings_tab_other: "其他",
    settings_basic_section: "基本信息",
    settings_basic_oidc_section: "OIDC 公开端点",
    settings_basic_issuer: "颁发者",
    settings_basic_listen_addr: "监听地址",
    settings_basic_cookie_secure: "Cookie Secure 标志",
    settings_basic_trusted_proxies: "可信代理",
    settings_basic_trusted_proxies_none: "（无 — 直接信任对端 IP）",
    settings_basic_default_lang: "服务器默认语言",
    settings_basic_save: "保存",
    settings_basic_saved: "服务器设置已更新。",

    // Profile
    profile_title: "个人资料",
    profile_username_label: "用户名",
    profile_email_label: "邮箱地址",
    profile_display_name_label: "显示名称",
    profile_lang_label: "显示语言",
    profile_lang_hint: "选择浏览器默认将根据浏览器语言设置自动选择。",
    profile_lang_browser_default: "浏览器默认",
    profile_save: "保存",
    profile_saved: "个人资料已更新。",

    // Password change
    password_change_title: "修改密码",
    password_change_lede: "请输入当前密码以及两次新密码。",
    password_change_current_label: "当前密码",
    password_change_new_label: "新密码",
    password_change_new_hint: "12 个字符或以上。长随机密码短语比短复杂密码更安全。",
    password_change_confirm_label: "确认新密码",
    password_change_revoke_others_label: "修改密码后退出其他浏览器/应用",
    password_change_revoke_others_hint: "推荐。现有会话和刷新令牌将失效，需用新密码重新登录。",
    password_change_submit: "修改密码",
    password_change_wrong_current: "当前密码不正确。",
    password_change_done_flash: "密码已修改。",

    // /me/security
    me_security_title: "账户安全",
    me_security_signed_in_as_suffix: " 当前已登录。",
    me_security_admin_link: "打开管理控制台 →",
    me_security_mfa_section: "双重认证",
    me_security_mfa_status_label: "状态：",
    me_security_mfa_status_enabled: "已启用",
    me_security_mfa_factor_totp: "认证器应用",
    me_security_mfa_factor_passkey_n: "{n} 个通行密钥",
    me_security_mfa_disabled_title: "双重认证已禁用。",
    me_security_mfa_disabled_lede: "此账户当前仅受密码保护，强烈建议注册通行密钥或认证器应用。",
    me_security_mfa_manage: "管理认证因素",
    // Self-service MFA tab extensions (RFC 055, 056)
    me_security_mfa_recovery_section_label: "恢复码",
    me_security_mfa_recovery_codes_remaining: |n| format!("剩余 {n} 个"),
    me_security_language_saved_banner: "语言偏好已保存。",
    setup_steps_aria: "设置步骤",
    me_security_tabs_aria: "安全选项卡",
    settings_tabs_aria: "设置选项卡",
    me_security_password_change_link: "修改密码",
    me_security_sessions_section: "登录位置",
    me_security_sessions_lede: "每行代表一个浏览器会话。点击撤销会立即退出该浏览器。标有当前会话的是您正在使用的。",
    me_security_sessions_th_started: "开始时间",
    me_security_sessions_th_expires: "过期时间",
    me_security_sessions_th_factors: "认证因素",
    me_security_sessions_current_badge: "当前会话",
    me_security_sessions_revoke: "撤销",
    me_security_sessions_revoke_confirm: "退出此会话？",
    me_security_sessions_revoke_all_others: "退出所有其他会话",
    me_security_sessions_revoke_all_others_confirm: "退出除当前会话以外的所有会话？",
    me_security_activity_section: "近期活动",
    me_security_activity_lede: "您账户的认证和管理事件。如有不认识的记录，请立即修改密码并退出其他会话。",
    me_security_activity_th_when: "时间",
    me_security_activity_th_event: "事件",
    me_security_activity_th_result: "结果",
    me_security_activity_th_note: "备注",

    // Email subjects/bodies
    email_subject_password_reset: "重置密码 — sui-id",
    email_subject_password_changed: "您的密码已更改 — sui-id",
    email_greeting_suffix: "，您好",
    email_password_reset_intro: "我们收到了您的 sui-id 账户密码重置请求，请在 30 分钟内使用以下链接设置新密码。",
    email_password_reset_link_label: "重置密码",
    email_password_reset_disregard: "如果不是您本人操作，请忽略此邮件。",
    email_password_changed_intro: "您的 sui-id 账户密码已被更改。",
    email_password_changed_security_warning: "如果不是您本人操作，请立即撤销其他会话并联系管理员。",
    email_password_changed_link_security: "安全设置",

    // Errors
    error_generic_title: "错误",
    error_not_found_title: "页面未找到",
    error_not_found_lede: "您查找的页面不存在或已被移动。",
    error_internal: "发生了意外错误。",
    error_too_many_requests_label: "请求过多，请稍等片刻后重试。",

    // Audit
    audit_title: "审计日志",
    audit_col_when: "时间",
    audit_col_actor: "操作者",
    audit_col_action: "操作",
    audit_col_target: "对象",
    audit_col_outcome: "结果",
    audit_col_note: "备注",

    // 设置选项卡（RFC 023：将"其他"重命名为"高级"）

    // 审计事件标签（RFC 002 § D）
    audit_event_auth_login_success: "登录",
    audit_event_auth_login_failure: "登录失败",
    audit_event_auth_login_locked: "账户锁定",
    audit_event_auth_login_mfa_required: "需要 MFA",
    audit_event_auth_logout: "退出登录",
    audit_event_auth_mfa_success: "MFA 验证成功",
    audit_event_auth_mfa_failure: "MFA 验证失败",
    audit_event_auth_password_changed_self: "修改密码",
    audit_event_auth_password_reset_requested: "申请密码重置",
    audit_event_auth_password_reset_email_sent: "重置邮件已发送",
    audit_event_auth_password_reset_email_failed: "重置邮件发送失败",
    audit_event_auth_password_reset_throttled: "重置请求被限流",
    audit_event_auth_password_reset_completed: "密码重置完成",
    audit_event_auth_refresh_theft_detected: "检测到令牌盗用",
    audit_event_auth_session_revoked: "会话已撤销",
    audit_event_auth_sessions_bulk_revoke_self: "批量撤销其他会话",
    audit_event_auth_smtp_config_changed: "SMTP 配置已更改",
    audit_event_user_create: "创建用户",
    audit_event_user_delete: "删除用户",
    audit_event_user_reset_password: "重置密码（管理员）",
    audit_event_admin_user_unlock: "解除账户锁定",
    audit_event_client_create: "创建客户端",
    audit_event_client_update: "更新客户端",
    audit_event_client_delete: "删除客户端",
    audit_event_client_set_allowed_scopes: "更新客户端权限范围",
    audit_event_signing_key_rotate: "轮换签名密钥",
    audit_event_signing_key_delete: "删除签名密钥",
    audit_event_admin_master_key_rotated: "轮换主密钥",
    audit_event_setup_create_initial_admin: "创建初始管理员",

    // Admin: 仪表盘 (RFC 029)
    dashboard_title: "仪表盘",
    dashboard_lede: "系统概览和近期活动。",
    dashboard_stat_users: "用户",
    dashboard_stat_clients: "客户端",
    dashboard_stat_sessions: "活跃会话",
    dashboard_stat_service_status: "服务状态",
    dashboard_stat_service_ok: "运行中",
    dashboard_activity_title: "登录活动",
    dashboard_activity_period: "时间范围",
    // Dashboard extensions (RFC 051)
    dashboard_greeting: |u| format!("您好，{u}。"),
    dashboard_aria_stats: "统计",
    dashboard_aria_action_required: "需要操作员处理",
    dashboard_action_required_title: "需要处理",
    dashboard_activity_success: "成功",
    dashboard_activity_failure: "失败",
    dashboard_activity_hover_hint: "悬停查看每个时间段的详细信息。",
    dashboard_oidc_endpoint_discovery: "Discovery",
    dashboard_oidc_endpoint_jwks: "JWKS",
    dashboard_sparkline_aria: "登录活动迷你图",
    dashboard_sparkline_tooltip: |label, success, failure| {
        format!("{label} : 成功 {success} / 失败 {failure}")
    },

    // 仪表盘：操作员提示 (RFC 031)
    dashboard_warn_smtp: "密码重置邮件已禁用，请在设置 → 邮件中配置 SMTP。",
    dashboard_warn_hibp: "密码泄露检查已关闭，建议在设置 → 认证中启用。",
    dashboard_warn_cookie_insecure: "Cookie Secure 标志已关闭，生产环境请在设置 → 安全中启用。",
    dashboard_warn_admins_no_mfa: |n| format!("{n}个管理员账户未启用MFA。"),
    dashboard_warn_old_signing_key: |age| format!("最早的签名密钥已存在{age}天 — 建议轮换。"),
    dashboard_warn_outbox_stuck: |n| format!("{n}封邮件已排队超过一小时。"),
    dashboard_warn_pending_resets: |n| format!("{n}个未使用的密码重置链接。"),
    dashboard_getting_started_title: "快速入门",
    dashboard_getting_started_smtp: "配置SMTP — 用户接收密码重置邮件所需",
    dashboard_getting_started_first_app: "添加您的第一个OIDC应用",
    dashboard_getting_started_admin_mfa: "为管理员账户启用MFA",

    // Admin: 用户管理 (RFC 029)
    users_title: "用户管理",
    users_lede: "创建和管理用户账户。",
    users_create_section: "添加新用户",
    users_create_button: "创建用户",
    users_table_section: "用户列表",
    users_table_th_status: "状态",
    users_table_th_mfa: "双重认证",
    users_is_admin_label: "授予管理员权限",
    users_empty: "暂无用户，请在上方表单中创建第一个用户。",
    // Users extensions (RFC 051)
    users_count_caption: |n| format!("当前 {n} 名。"),
    users_label_username: "用户名",
    users_label_display_name: "显示名（可选）",
    users_label_email: "邮箱地址（可选）",
    users_label_password: "密码（12 个字符以上）",

    // Admin: 客户端管理 (RFC 029)
    // Admin: 客户端编辑 (RFC 051)
    client_edit_title: "编辑客户端",
    client_edit_basic_section: "基本信息",
    client_edit_basic_note: "Client ID、类型（confidential/public）和 client secret 在创建时固定。如需修改，请删除后重新注册。",
    client_edit_new_secret_label: "新的 client secret（仅此页面显示一次）：",
    client_edit_label_client_id: "Client ID",
    client_edit_label_kind: "类型",
    client_edit_label_status: "状态",
    client_edit_post_logout_hint: "每行一个。留空＝沿用 Redirect URIs。",

    // Admin: 客户端管理 (RFC 029)
    clients_title: "客户端管理",
    clients_lede: "注册和管理 OIDC 客户端。",
    clients_create_section: "注册新客户端",
    clients_table_section: "已注册客户端",
    clients_secret_once_banner: "客户端密钥仅显示一次，请立即保存到安全位置。",
    clients_table_th_name: "名称",
    clients_table_th_kind: "类型",
    clients_table_th_status: "状态",
    clients_empty: "暂无 OIDC 客户端，请使用上方表单注册。",
    clients_single_realm_note: "sui-id 是单域 IdP，所有用户均可访问所有客户端。权限范围限制客户端可请求的信息，而非限制哪些用户可以登录。",
    // Clients page extensions (RFC 051)
    clients_table_th_client_id: "Client ID",
    clients_count_caption: |n| format!("当前 {n} 个。"),
    clients_label_app_name: "应用名称",
    clients_label_redirect_uris: "Redirect URIs",
    clients_hint_redirect_uris: "每行一个，仅支持 https 或回环 http。",
    clients_label_allowed_scopes: "允许的权限范围 (Allowed scopes)",
    clients_hint_scopes_intro: "空格分隔。已知范围：",
    clients_hint_scopes_openid_note: " (必填) · ",
    clients_hint_scopes_profile_note: " (姓名、语言) · ",
    clients_hint_scopes_email_note: " (邮箱) · ",
    clients_hint_scopes_offline_note: " (刷新令牌)。",
    clients_hint_scopes_default: "留空时默认为 openid profile email。",
    clients_label_post_logout_uris: "Post-logout redirect URIs（可选）",
    clients_hint_one_per_line: "每行一个。",
    clients_label_confidential_checkbox: "Confidential client（发放 client secret）",
    clients_button_register: "注册",

    // Admin: 审计日志 (RFC 029)
    audit_lede: "管理操作历史（最新在前）。",

    // 审计日志增强 (RFC 033)
    audit_chain_ok: "审计链完整性正常。",
    audit_chain_broken: "审计链完整性检查失败，请立即调查。",
    audit_filter_label: "按事件筛选",
    audit_filter_placeholder: "例如：auth.login",
    audit_export_csv: "导出 CSV",
    // Audit log extensions (RFC 051)
    audit_entry_count_caption: |n| format!("({n} 条)"),
    audit_filter_button: "筛选",
    audit_chain_broken_note: |seq| format!("在 seq={seq} 处检测到不一致，请立即调查。"),
    audit_chain_ok_note: |checked, legacy| {
        format!("已检查最近 {checked} 行。遗留（v0.17 之前）未哈希行：{legacy}")
    },

    // Admin: 签名密钥 (RFC 029)
    signing_keys_title: "签名密钥",
    signing_keys_lede: "用于签发 JWT 的 Ed25519 密钥管理。",
    signing_keys_rotate_section: "密钥轮换",
    signing_keys_rotate_button: "轮换签名密钥",
    signing_keys_rotate_warning: "轮换将生成新密钥并将当前密钥设为退役状态。退役密钥仍保留在 JWKS 中，以便已签发的令牌在有效期内继续可用。",
    signing_keys_table_section: "所有密钥",
    signing_keys_th_algorithm: "算法",
    signing_keys_th_status: "状态",
    signing_keys_th_created: "创建时间",
    signing_keys_th_retired: "退役时间",
    signing_keys_empty: "暂无签名密钥，请点击\"轮换签名密钥\"生成第一个密钥。",
    signing_keys_in_use_badge: "（使用中）",
    // Signing keys extensions (RFC 051)
    signing_keys_count_caption: |n| format!("已登记 {n} 个。"),
    signing_keys_th_key_id: "Key ID",
    signing_keys_rotate_explanation_1: "执行轮换将签发新的签名密钥，并将当前密钥移至「退役」状态。",
    signing_keys_rotate_explanation_2: "退役密钥仍保留在 JWKS 中，确保已签发但未过期的令牌仍可验证。",
    signing_keys_rotate_explanation_3: "待这些令牌到期之后，可在本页安全地删除退役密钥。",

    // 危险操作确认页面 (RFC 030)
    confirm_cancel: "取消",
    badge_recoverable: "可恢复",
    badge_not_recoverable: "不可恢复",
    confirm_disable_title: "禁用用户？",
    confirm_disable_impact: "此用户将无法登录，直到重新启用为止。",
    confirm_disable_reversibility: "可从用户列表中撤销此操作。",
    confirm_disable_button: "禁用用户",
    confirm_enable_title: "重新启用用户？",
    confirm_enable_button: "启用用户",
    confirm_delete_user_title: "删除用户？",
    confirm_delete_user_impact: "此用户将从用户列表中永久删除，审计历史记录将予以保留。",
    confirm_delete_user_reversibility: "无法通过管理面板撤销此操作。",
    confirm_delete_user_button: "删除用户",
    confirm_reset_mfa_title: "重置双重认证？",
    confirm_reset_mfa_impact: "该用户的 TOTP 认证器和所有通行密钥将被删除，下次登录时需要重新注册。",
    confirm_reset_mfa_reversibility: "操作后用户可重新注册。",
    confirm_reset_mfa_button: "重置 MFA",
    confirm_delete_client_title: "删除客户端？",
    confirm_delete_client_impact: "此 OIDC 客户端将被永久删除，颁发给该客户端的所有活跃会话和刷新令牌将被撤销。",
    confirm_delete_client_reversibility: "此操作无法撤销。",
    confirm_delete_client_button: "删除客户端",
    confirm_delete_signing_key_title: "删除签名密钥？",
    confirm_delete_signing_key_impact: "由此密钥签名且尚未过期的令牌将立即验证失败。",
    confirm_delete_signing_key_reversibility: "此操作无法撤销，请仅在所有令牌均已过期后删除。",
    confirm_delete_signing_key_button: "删除签名密钥",
    error_403_auditor_title: "只读访问",
    error_403_auditor_body: "您的账户具有只读（审计员）访问权限。此操作需要管理员权限。",
    client_detail_readonly_title: "应用详情",
    confirm_rotate_signing_key_title: "轮换签名密钥",
    confirm_rotate_signing_key_impact: "将颁发新的签名密钥。使用旧密钥签名的现有令牌在过期前仍然有效。",
    confirm_rotate_signing_key_reversibility: "此操作无法撤销。旧密钥将被停用。",
    confirm_rotate_signing_key_button: "轮换密钥",
    confirm_email_settings_title: "确认邮件设置",
    confirm_email_settings_impact: "以下邮件设置将被保存：",
    confirm_email_settings_button: "保存设置",

    // Admin: 用户详情 (RFC 035)
    user_detail_back: "← 返回用户列表",
    user_detail_auth_section: "认证",
    user_detail_totp_label: "认证器应用 (TOTP)：",
    user_detail_passkeys_label: "通行密钥：",
    user_detail_sessions_section: "活跃会话",
    user_detail_sessions_th_started: "开始时间",
    user_detail_sessions_th_expires: "过期时间",
    user_detail_sessions_th_factors: "认证因素",
    user_detail_activity_section: "近期活动",
    user_detail_danger_zone_body: "这些操作会影响此用户的访问权限，可能无法撤销。每项操作都需要确认步骤。",
    role_admin: "管理员",
    role_auditor: "审计员",
    role_user: "用户",
    user_detail_role_section: "访问角色",
    user_detail_role_change: "更改角色",
    user_detail_role_saved: "角色已更新。",
    user_detail_role_last_admin: "无法降级最后一位管理员。",

    // 设置页面分区键
    settings_page_title_template: "设置",
    settings_basic_description: "查看当前有效配置。如需修改，请编辑 sui-id.toml 并重启。",
    // Settings: Basic extensions (RFC 051)
    settings_basic_default_lang_hint: "当未设置用户偏好且浏览器 Accept-Language 不匹配支持的语言时，使用此值作为回退。",
    settings_basic_kv_issuer: "Issuer",
    settings_basic_kv_listen: "监听地址",
    settings_basic_kv_cookie_secure: "Cookie Secure 标志",
    settings_basic_kv_trusted_proxies: "受信代理",
    settings_security_session_section: "会话限制",
    settings_security_session_lede: "空闲超时和每用户并发会话上限，均默认为 0（禁用），按需启用。",
    settings_security_idle_timeout_label: "空闲超时（秒）",
    settings_security_max_sessions_label: "每用户最大并发会话数",
    settings_security_lockout_section: "账户锁定",
    settings_security_headers_section: "安全响应头",
    // Settings: Security extensions (RFC 051)
    settings_security_idle_timeout_hint: "0 表示禁用。0 < N ≤ 2,592,000（= 30 天）。",
    settings_security_max_sessions_hint: "0 表示禁用。1 ≤ N ≤ 1000。超过时自动撤销最旧的会话（FIFO）。",
    settings_security_lockout_hint_1: "渐进式回退的上限值。失败累计时间不会超过该值。",
    settings_security_lockout_hint_2_pre: "管理员随时可通过 ",
    settings_security_lockout_hint_2_post: " 命令解锁。",
    settings_security_headers_perm_policy_label: "Permissions-Policy（最小）",
    settings_security_headers_hint: "所有管理页面均返回上述响应头。/oauth2/* 公开端点会按协议要求省略部分头。",
    settings_security_cors_token_label: "Token 端点的动态允许列表（已注册 redirect_uris 的来源）",
    settings_security_cors_public_label: "Discovery / JWKS / userinfo 公开允许（*）",
    settings_auth_password_section: "密码",
    settings_auth_mfa_section: "双重认证",
    settings_auth_oidc_section: "OIDC / 令牌设置",
    settings_logs_output_section: "日志输出",
    settings_logs_audit_section: "审计日志哈希链",
    settings_advanced_build_section: "构建信息",
    settings_advanced_storage_section: "存储",
    settings_advanced_record_counts: "记录数量",

    // OIDC 授权同意页面 (RFC 038)
    consent_title: "授权访问",
    consent_app_wants_access: "请求访问以下内容：",
    consent_scope_openid: "验证您的身份",
    consent_scope_profile: "您的个人资料（姓名、语言）",
    consent_scope_email: "您的邮箱地址",
    consent_scope_offline_access: "保持登录状态（刷新令牌）",
    consent_scope_openid_desc: "确认您的登录并提供唯一标识符。",
    consent_scope_profile_desc: "名称、首选语言和时区。",
    consent_scope_email_desc: "电子邮件地址及其验证状态。",
    consent_scope_offline_access_desc: "允许应用在您不在时代表您保持登录状态。",
    consent_approve: "允许",
    consent_deny: "拒绝",
    consent_policy_label: "同意策略",
    consent_policy_none: "无（跳过同意页面）",
    consent_policy_first_time: "仅首次询问",
    consent_policy_always: "每次询问",

    // 设置: 页面标题 (RFC 039)
    settings_title_basic: "设置 — 基本",
    settings_title_security: "设置 — 安全",
    settings_title_authentication: "设置 — 认证",
    settings_title_logs: "设置 — 日志",
    settings_title_email: "设置 — 邮件",
    settings_title_advanced: "设置 — 高级",
    // 设置: 认证选项卡正文 (RFC 039)
    settings_auth_min_length_label: "最小长度",
    settings_auth_hash_algorithm_label: "哈希算法",
    settings_auth_mfa_totp: "TOTP（认证器应用）",
    settings_auth_mfa_passkey: "WebAuthn（通行密钥）",
    settings_auth_mfa_recovery_label: "每次注册的恢复码数量",
    settings_auth_access_token_ttl: "访问令牌有效期",
    settings_auth_id_token_ttl: "ID 令牌有效期",
    settings_auth_refresh_token_ttl: "刷新令牌有效期",
    settings_auth_refresh_rotate: "刷新令牌轮换",
    settings_auth_refresh_theft: "刷新令牌盗用检测（系列失效）",
    settings_auth_pkce_required: "强制 PKCE（所有客户端、所有流程）",
    // 设置: 日志选项卡正文 (RFC 039)
    settings_logs_recent_24h: "过去 24 小时的事件",
    settings_logs_chain_broken_note: "检测到哈希链不一致，请立即调查。",
    settings_logs_chain_ok_note: "哈希链完整性正常。",
    // 设置: 高级选项卡正文 (RFC 039)
    settings_advanced_version_label: "sui-id 版本",
    settings_advanced_schema_label: "数据库模式版本",
    settings_advanced_server_time_label: "服务器时间",
    settings_advanced_db_file_label: "数据库文件",
    settings_advanced_key_file_label: "主密钥文件",
    settings_advanced_manage_link: "管理 →",
    // 设置: 邮件选项卡正文 (RFC 039)
    settings_email_page_title: "设置 — 邮件",
    settings_email_lede: "用于密码重置和更改通知邮件的 SMTP 设置。",
    settings_email_smtp_section: "SMTP 连接",
    settings_email_enable_label: "启用邮件发送",
    settings_email_host_label: "SMTP 主机",
    settings_email_port_label: "端口",
    settings_email_port_hint: "587（STARTTLS）或 465（隐式 TLS）较为常见。",
    settings_email_tls_label: "TLS 模式",
    settings_email_tls_implicit: "隐式 TLS（465）",
    settings_email_username_label: "用户名（可选）",
    settings_email_from_addr_label: "发件人地址",
    settings_email_from_name_label: "发件人显示名称（可选）",
    settings_email_base_url_label: "公开 Base URL",
    settings_email_test_button: "测试连接",
    settings_email_test_hint: "使用当前设置尝试连接 SMTP 服务器，不会发送邮件。",

    // 错误页面 (RFC 042)
    error_404_title: "未找到页面",
    error_404_lede: "该页面不存在或已被删除。",
    error_429_title: "请求过多",
    error_429_lede: "请稍候片刻后再试。",
    error_500_title: "服务器错误",
    error_500_lede: "发生了问题，请联系服务器管理员。",
    error_generic_lede: "无法处理该请求。",
    error_request_id_label: "请求 ID",
    error_back_home: "返回首页",

    // RFC 042
    error_internal_lede: "发生了问题，请联系服务器管理员。",

    // RFC 042
    error_too_many_requests_lede: "请稍候片刻后再试。",

    dashboard_recent_events_title: "最近的重要事件",

    dashboard_recent_events_empty: "暂无重要事件。",

    dashboard_recent_events_view_all: "查看全部 →",

    // /me/security 选项卡 (RFC 040)
    me_tab_overview: "概览",
    me_tab_password: "密码",
    me_tab_apps: "应用",
    me_apps_title: "已授权应用",
    me_apps_intro: "可以代表您登录的应用。您可以随时撤销访问权限。",
    me_apps_granted_on: "授权时间",
    me_apps_last_used: "上次使用",
    me_apps_never_used: "从未使用",
    me_apps_revoke_button: "撤销访问",
    me_apps_revoked: "访问已撤销。该应用需要再次请求权限。",
    me_apps_empty: "您尚未授权任何应用。",
    me_tab_mfa: "多重认证",
    me_tab_passkey: "通行密钥",
    me_tab_sessions: "会话",
    me_tab_language: "语言",
    me_overview_section_status: "安全状态",
    me_overview_section_activity: "最近活动",
    me_overview_label_mfa_totp: "MFA（TOTP）",
    me_overview_label_passkeys: "通行密钥",
    me_overview_no_recent_events: "暂无最近活动记录。",
    setup_welcome_lang_picker_label: "语言选择",
    me_passkey_origin_warning: "通行密钥需要 HTTPS 或 localhost。",
    me_passkey_section_title: "已注册的通行密钥",
    me_passkey_button_rename: "重命名",
    me_passkey_nickname_label: "昵称",
    me_passkey_nickname_placeholder: "例: YubiKey 5C",
    me_language_title: "显示语言",
    me_language_lede: "设置您的首选语言。未设置时将使用浏览器设置或服务器默认值。",
    me_language_use_default: "系统默认（Cookie / Accept-Language）",
    me_language_saved_flash: "语言偏好已保存。",

    disable_reason_label: "禁用原因（可选）",

    disable_reason_placeholder: "例如：已离职、存在可疑活动",

    disable_reason_hint: "将记录在审计日志中，便于未来管理员了解背景。",
};

// ── Formatters ───────────────────────────────────────────────────────────────

fn zh_hans_fmt_date(dt: DateTime<Utc>) -> String {
    format!("{}年{}月{}日", dt.year(), dt.month(), dt.day())
}

fn zh_hans_fmt_date_time(dt: DateTime<Utc>) -> String {
    format!("{} {}", zh_hans_fmt_date(dt), fmt_time_shared(dt))
}

fn zh_hans_fmt_relative(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - at).num_seconds();
    if secs < 0 {
        return "刚刚".into();
    }
    if secs < 60 {
        return format!("{secs} 秒前");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins} 分钟前");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours} 小时前");
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{days} 天前");
    }
    let months = days / 30;
    if months < 12 {
        return format!("{months} 个月前");
    }
    let years = months / 12;
    format!("{years} 年前")
}

/// Chinese Simplified (zh-Hans) date and number formatters.
pub static FORMATTERS_ZH_HANS: Formatters = Formatters {
    fmt_date: zh_hans_fmt_date,
    fmt_time: fmt_time_shared,
    fmt_date_time: zh_hans_fmt_date_time,
    fmt_relative: zh_hans_fmt_relative,
    fmt_count: fmt_count_shared,
};

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn ts(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[test]
    fn zh_hans_date_formatting() {
        let dt = ts(2024, 5, 12, 14, 7);
        assert_eq!(zh_hans_fmt_date(dt), "2024年5月12日");
        assert_eq!(zh_hans_fmt_date_time(dt), "2024年5月12日 14:07");
    }

    #[test]
    fn zh_hans_relative_formatting() {
        let now = ts(2024, 5, 12, 15, 0);
        assert_eq!(
            zh_hans_fmt_relative(ts(2024, 5, 12, 14, 57), now),
            "3 分钟前"
        );
        assert_eq!(
            zh_hans_fmt_relative(ts(2024, 5, 12, 12, 0), now),
            "3 小时前"
        );
        assert_eq!(zh_hans_fmt_relative(ts(2024, 5, 9, 15, 0), now), "3 天前");
    }
}
