//! sui-id internationalisation.
//!
//! ## Design
//!
//! All user-facing strings are fields on a [`Strings`] struct. Each
//! supported locale has a `static Strings` constant with all fields
//! filled in (`STRINGS_JA`, `STRINGS_EN`). Adding a locale means
//! adding a variant to [`Locale`] and a new `static Strings`
//! constant — the compiler then guarantees every translation is
//! complete via the exhaustive `match` in [`Locale::strings`].
//! Adding a string means adding a field to [`Strings`] — the
//! compiler then yells at every per-locale constant until it's
//! filled in.
//!
//! Strings without variable interpolation are `&'static str`.
//! Strings with interpolation use small format functions that
//! take parameters and return `String`. We deliberately avoid a
//! generic templating layer (Fluent, MessageFormat, etc) at this
//! tier — the interpolation patterns we have are simple,
//! enumeration-style ("3 outstanding tokens"), and a per-locale
//! function is more readable than a templated string.
//!
//! ## What lives here, what doesn't
//!
//! - **Lives here**: UI labels, button text, flash messages,
//!   page titles, email subjects/bodies. Anything a human reads.
//! - **Does not live here**: log messages, audit-event names
//!   (those are stable identifiers operators query against),
//!   error machine codes, configuration keys.
//!
//! ## Future expansion (see sui-id ROADMAP)
//!
//! - More locales (zh, ko, etc) — add `Locale::Zh` and
//!   `STRINGS_ZH`; the type system handles the rest.
//! - Date/number formatting localisation — currently we use a
//!   single ISO-ish format across locales for simplicity. v2
//!   will add per-locale formatters.

use serde::{Deserialize, Serialize};

/// A supported locale.
///
/// New variants must:
///   - have a stable BCP-47-style tag returned by [`Locale::tag`];
///   - have a `static STRINGS_*` constant matched in [`Locale::strings`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    Ja,
    En,
}

impl Locale {
    /// All locales sui-id recognises, in display order.
    pub const ALL: &'static [Locale] = &[Locale::Ja, Locale::En];

    /// BCP-47 language tag. Used in HTML `lang=` attributes,
    /// cookies, and the user preference column. Stable.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Ja => "ja",
            Self::En => "en",
        }
    }

    /// Native-language name of this locale, displayed in the
    /// language picker. Always shown in the locale's own script
    /// so a user who has accidentally landed on the wrong language
    /// can still recognise their own.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::Ja => "日本語",
            Self::En => "English",
        }
    }

    /// Parse a tag back into a `Locale`. Tolerant of region
    /// suffixes (`en-US` → `En`) and capitalisation. Unknown tags
    /// return `None`; callers should fall back through their
    /// preference chain rather than choosing here.
    pub fn parse(tag: &str) -> Option<Locale> {
        let primary = tag
            .split(|c: char| c == '-' || c == '_')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match primary.as_str() {
            "ja" => Some(Locale::Ja),
            "en" => Some(Locale::En),
            _ => None,
        }
    }

    /// Strings table for this locale. The exhaustive match is the
    /// completeness guarantee — adding a `Locale` variant without a
    /// strings table fails to compile.
    pub fn strings(self) -> &'static Strings {
        match self {
            Self::Ja => &STRINGS_JA,
            Self::En => &STRINGS_EN,
        }
    }
}

impl Default for Locale {
    fn default() -> Self {
        Locale::Ja
    }
}

/// Pick a locale from a `q`-weighted Accept-Language header.
///
/// Cheap parser: split on commas, take each token's primary
/// subtag, return the first one we recognise. We ignore `q=`
/// weights — for a two-locale catalogue the cost of a real parser
/// outweighs the benefit. A user with `Accept-Language: fr;q=1, en;q=0.5`
/// will get English (the first recognised tag), which matches the
/// "best available match" intent close enough.
pub fn negotiate_from_accept_language(header: &str) -> Option<Locale> {
    for raw in header.split(',') {
        let tag = raw.split(';').next().unwrap_or("").trim();
        if let Some(loc) = Locale::parse(tag) {
            return Some(loc);
        }
    }
    None
}

/// All translatable UI strings.
///
/// Keep field names descriptive — the field name is what shows up
/// at every call site, and `t.login_button_submit` is much easier
/// to grep for than `t.s127`.
///
/// Group fields by area with section comments. When a string has
/// variable interpolation, expose it as a method below the struct
/// (`impl Strings { pub fn n_outstanding_tokens(...) }`) rather
/// than as a field.
pub struct Strings {
    // ---- Generic UI ----
    pub button_save: &'static str,
    pub button_cancel: &'static str,
    pub button_back: &'static str,
    pub button_continue: &'static str,
    pub button_delete: &'static str,
    pub button_create: &'static str,
    pub button_confirm: &'static str,
    pub button_test: &'static str,
    pub badge_enabled: &'static str,
    pub badge_disabled: &'static str,
    pub badge_ok: &'static str,
    pub badge_warn: &'static str,
    pub badge_error: &'static str,
    pub label_optional: &'static str,
    pub label_required: &'static str,
    pub muted_none: &'static str,

    // ---- Navigation ----
    pub nav_dashboard: &'static str,
    pub nav_users: &'static str,
    pub nav_clients: &'static str,
    pub nav_signing_keys: &'static str,
    pub nav_audit: &'static str,
    pub nav_settings: &'static str,
    pub nav_profile: &'static str,
    pub nav_logout: &'static str,

    // ---- Login ----
    pub login_title: &'static str,
    pub login_username_label: &'static str,
    pub login_password_label: &'static str,
    pub login_submit: &'static str,
    pub login_passkey_button: &'static str,
    pub login_forgot_password_link: &'static str,
    pub login_invalid_credentials: &'static str,
    pub login_account_locked: &'static str,
    pub login_reset_ok_banner: &'static str,

    // ---- Setup wizard ----
    pub setup_step_welcome: &'static str,
    pub setup_step_admin: &'static str,
    pub setup_step_done: &'static str,
    pub setup_welcome_title: &'static str,
    pub setup_welcome_lede: &'static str,
    pub setup_welcome_lede2: &'static str,
    pub setup_welcome_begin: &'static str,
    pub setup_admin_title: &'static str,
    pub setup_admin_lede: &'static str,
    pub setup_admin_token_label: &'static str,
    pub setup_admin_token_hint: &'static str,
    pub setup_admin_username_label: &'static str,
    pub setup_admin_email_label: &'static str,
    pub setup_admin_email_hint: &'static str,
    pub setup_admin_display_label: &'static str,
    pub setup_admin_password_label: &'static str,
    pub setup_admin_password_hint: &'static str,
    pub setup_admin_confirm_label: &'static str,
    pub setup_admin_submit: &'static str,
    pub setup_done_title: &'static str,
    pub setup_done_lede: &'static str,
    pub setup_done_next_steps_title: &'static str,
    pub setup_done_next_step_register_clients: &'static str,
    pub setup_done_next_step_enable_mfa: &'static str,
    pub setup_done_next_step_review_settings: &'static str,
    pub setup_done_enter_admin: &'static str,
    pub setup_not_complete_title: &'static str,
    pub setup_not_complete_lede: &'static str,
    pub setup_password_mismatch: &'static str,
    pub setup_already_initialized: &'static str,
    pub setup_invalid_token: &'static str,
    /// Flash text shown by the setup wizard when HIBP is in
    /// `Block` mode and the supplied password is found in the
    /// breach corpus.
    pub setup_hibp_blocked: &'static str,
    /// Generic fallback flash text for an otherwise-unmapped
    /// setup-wizard error (e.g. an internal storage failure).
    pub setup_generic_failure: &'static str,

    // ---- Step-up auth ----
    pub step_up_title: &'static str,
    pub step_up_lede: &'static str,
    pub step_up_code_label: &'static str,
    pub step_up_code_hint: &'static str,
    pub step_up_code_invalid: &'static str,
    pub step_up_passkey_alt: &'static str,
    pub step_up_passkey_button: &'static str,

    // ---- MFA challenge (login flow) ----
    pub mfa_challenge_title: &'static str,
    pub mfa_challenge_lede: &'static str,
    pub mfa_challenge_code_label: &'static str,
    pub mfa_challenge_submit: &'static str,
    pub mfa_challenge_passkey_alt: &'static str,
    pub mfa_challenge_passkey_button: &'static str,
    pub mfa_challenge_shell_title: &'static str,
    /// Flash shown when the submitted TOTP / recovery code does
    /// not verify. Same text covers both factors so the UI
    /// doesn't leak which one the user got wrong.
    pub mfa_challenge_failed_flash: &'static str,

    // ---- Profile / self-service security (v0.29.1) ----
    pub profile_subtitle_template: &'static str,
    pub profile_recovery_save_now: &'static str,
    pub profile_recovery_save_lede: &'static str,
    pub profile_mfa_section: &'static str,
    pub profile_mfa_totp_card_title: &'static str,
    pub profile_mfa_status_label: &'static str,
    pub profile_mfa_status_enabled: &'static str,
    pub profile_mfa_status_not_configured: &'static str,
    pub profile_mfa_regenerate_codes: &'static str,
    pub profile_mfa_disable_confirm: &'static str,
    pub profile_mfa_disable_button: &'static str,
    pub profile_mfa_enroll_lede: &'static str,
    pub profile_mfa_enroll_apps_note: &'static str,
    pub profile_mfa_enroll_button: &'static str,
    /// Flash shown after MFA enrolment completes successfully.
    /// Pinned in audit-friendly wording: "now enabled" reads as a
    /// state assertion that doubles as confirmation the action
    /// landed.
    pub profile_passkeys_section: &'static str,
    pub profile_passkeys_lede: &'static str,
    pub profile_passkeys_th_name: &'static str,
    pub profile_passkeys_th_registered: &'static str,
    pub profile_passkeys_th_last_used: &'static str,
    pub profile_passkeys_last_used_never: &'static str,
    pub profile_passkeys_delete_confirm: &'static str,
    pub profile_passkeys_delete_button: &'static str,
    pub profile_passkeys_empty: &'static str,
    pub profile_passkeys_register_section: &'static str,
    pub profile_passkeys_nickname_label: &'static str,
    pub profile_passkeys_nickname_hint: &'static str,
    pub profile_passkeys_register_button: &'static str,
    pub profile_lang_section: &'static str,
    pub profile_lang_lede: &'static str,
    pub profile_lang_field_label: &'static str,
    /// Flash shown after a successful TOTP enrolment confirmation.
    pub profile_mfa_enrolled_flash: &'static str,
    /// Flash shown after recovery codes are regenerated.
    pub profile_recovery_regenerated_flash: &'static str,

    // ---- MFA setup (TOTP enrollment) ----
    pub mfa_setup_shell_title: &'static str,
    pub mfa_setup_title: &'static str,
    pub mfa_setup_lede: &'static str,
    pub mfa_setup_steps_title: &'static str,
    pub mfa_setup_step1: &'static str,
    pub mfa_setup_step2: &'static str,
    pub mfa_setup_step3: &'static str,
    pub mfa_setup_qr_card_title: &'static str,
    pub mfa_setup_secret_label: &'static str,
    pub mfa_setup_otpauth_summary: &'static str,
    pub mfa_setup_verify_card_title: &'static str,
    pub mfa_setup_code_label: &'static str,
    pub mfa_setup_code_hint: &'static str,
    pub mfa_setup_confirm_button: &'static str,

    // ---- Forgot password ----
    pub forgot_password_title: &'static str,
    pub forgot_password_lede: &'static str,
    pub forgot_password_email_label: &'static str,
    pub forgot_password_submit: &'static str,
    pub forgot_password_sent_title: &'static str,
    pub forgot_password_sent_lede: &'static str,
    pub forgot_password_sent_lede2: &'static str,
    pub reset_password_title: &'static str,
    pub reset_password_lede: &'static str,
    pub reset_password_new_label: &'static str,
    pub reset_password_new_hint: &'static str,
    pub reset_password_confirm_label: &'static str,
    pub reset_password_submit: &'static str,
    pub reset_password_invalid_title: &'static str,
    pub reset_password_invalid_lede: &'static str,
    pub reset_password_invalid_request_again: &'static str,
    pub password_mismatch_flash: &'static str,
    pub reset_password_failed_flash: &'static str,
    /// Common navigational link from forgot-password / reset-password
    /// flows back to the login screen. Reused across multiple
    /// auth-flow views.
    pub back_to_login: &'static str,

    // ---- Settings hub ----
    pub settings_title: &'static str,
    pub settings_lede: &'static str,
    pub settings_tab_basic: &'static str,
    pub settings_tab_security: &'static str,
    pub settings_tab_authentication: &'static str,
    pub settings_tab_logs: &'static str,
    pub settings_tab_email: &'static str,
    pub settings_tab_other: &'static str,
    pub settings_basic_section: &'static str,
    pub settings_basic_oidc_section: &'static str,
    pub settings_basic_issuer: &'static str,
    pub settings_basic_listen_addr: &'static str,
    pub settings_basic_cookie_secure: &'static str,
    pub settings_basic_trusted_proxies: &'static str,
    pub settings_basic_trusted_proxies_none: &'static str,
    pub settings_basic_default_lang: &'static str,
    pub settings_basic_default_lang_hint: &'static str,
    pub settings_basic_save: &'static str,
    pub settings_basic_saved: &'static str,

    // ---- Profile ----
    pub profile_title: &'static str,
    pub profile_username_label: &'static str,
    pub profile_email_label: &'static str,
    pub profile_display_name_label: &'static str,
    pub profile_lang_label: &'static str,
    pub profile_lang_hint: &'static str,
    pub profile_lang_browser_default: &'static str,
    pub profile_save: &'static str,
    pub profile_saved: &'static str,

    // ---- Password change ----
    pub password_change_title: &'static str,
    pub password_change_lede: &'static str,
    pub password_change_current_label: &'static str,
    pub password_change_new_label: &'static str,
    /// Hint shown under the new-password input. Reminds the user
    /// that a long passphrase beats a short complicated string.
    pub password_change_new_hint: &'static str,
    pub password_change_confirm_label: &'static str,
    pub password_change_revoke_others_label: &'static str,
    /// Hint shown under the "sign out other sessions" checkbox.
    pub password_change_revoke_others_hint: &'static str,
    pub password_change_submit: &'static str,
    pub password_change_wrong_current: &'static str,
    pub password_change_done_flash: &'static str,

    // ---- /me/security (self-service security page) ----
    pub me_security_title: &'static str,
    /// Lede line; rendered as `<strong>{username}</strong> {me_security_signed_in_as_suffix}`.
    pub me_security_signed_in_as_suffix: &'static str,
    pub me_security_admin_link: &'static str,
    pub me_security_mfa_section: &'static str,
    pub me_security_mfa_status_label: &'static str,
    pub me_security_mfa_status_enabled: &'static str,
    pub me_security_mfa_factor_totp: &'static str,
    /// Format string fragment; the call site interpolates the
    /// passkey count via `format!("{n} {label}")`.
    pub me_security_mfa_factor_passkey_n: &'static str,
    pub me_security_mfa_disabled_title: &'static str,
    pub me_security_mfa_disabled_lede: &'static str,
    pub me_security_mfa_manage: &'static str,
    pub me_security_password_change_link: &'static str,
    pub me_security_sessions_section: &'static str,
    pub me_security_sessions_lede: &'static str,
    pub me_security_sessions_th_started: &'static str,
    pub me_security_sessions_th_expires: &'static str,
    pub me_security_sessions_th_factors: &'static str,
    pub me_security_sessions_current_badge: &'static str,
    pub me_security_sessions_revoke: &'static str,
    pub me_security_sessions_revoke_confirm: &'static str,
    pub me_security_sessions_revoke_all_others: &'static str,
    pub me_security_sessions_revoke_all_others_confirm: &'static str,
    pub me_security_activity_section: &'static str,
    pub me_security_activity_lede: &'static str,
    pub me_security_activity_th_when: &'static str,
    pub me_security_activity_th_event: &'static str,
    pub me_security_activity_th_result: &'static str,
    pub me_security_activity_th_note: &'static str,

    // ---- Email subjects (notifications) ----
    pub email_subject_password_reset: &'static str,
    pub email_subject_password_changed: &'static str,
    pub email_greeting_suffix: &'static str,
    pub email_password_reset_intro: &'static str,
    pub email_password_reset_link_label: &'static str,
    pub email_password_reset_disregard: &'static str,
    pub email_password_changed_intro: &'static str,
    pub email_password_changed_security_warning: &'static str,
    pub email_password_changed_link_security: &'static str,

    // ---- Errors / generic ----
    pub error_generic_title: &'static str,
    pub error_not_found_title: &'static str,
    pub error_not_found_lede: &'static str,
    pub error_internal: &'static str,
    pub error_too_many_requests_label: &'static str,

    // ---- Audit log labels ----
    pub audit_title: &'static str,
    pub audit_col_when: &'static str,
    pub audit_col_actor: &'static str,
    pub audit_col_action: &'static str,
    pub audit_col_target: &'static str,
    pub audit_col_outcome: &'static str,
    pub audit_col_note: &'static str,
}

// ---- Strings::JA ----

pub static STRINGS_JA: Strings = Strings {
    // Generic UI
    button_save: "保存",
    button_cancel: "キャンセル",
    button_back: "戻る",
    button_continue: "続行",
    button_delete: "削除",
    button_create: "作成",
    button_confirm: "確認",
    button_test: "テスト",
    badge_enabled: "有効",
    badge_disabled: "無効",
    badge_ok: "正常",
    badge_warn: "警告",
    badge_error: "エラー",
    label_optional: "(任意)",
    label_required: "(必須)",
    muted_none: "(なし)",

    // Navigation
    nav_dashboard: "ダッシュボード",
    nav_users: "ユーザー",
    nav_clients: "クライアント",
    nav_signing_keys: "署名キー",
    nav_audit: "監査ログ",
    nav_settings: "設定",
    nav_profile: "プロフィール",
    nav_logout: "サインアウト",

    // Login
    login_title: "サインイン",
    login_username_label: "ユーザー名",
    login_password_label: "パスワード",
    login_submit: "サインイン",
    login_passkey_button: "パスキーでサインイン",
    login_forgot_password_link: "パスワードをお忘れですか?",
    login_invalid_credentials: "ユーザー名またはパスワードが正しくありません。",
    login_account_locked: "アカウントは一時的にロックされています。しばらく待ってから再度お試しください。",
    login_reset_ok_banner: "パスワードを再設定しました。新しいパスワードでサインインしてください。",

    // Setup wizard
    setup_step_welcome: "ようこそ",
    setup_step_admin: "管理者作成",
    setup_step_done: "完了",
    setup_welcome_title: "sui-id へようこそ",
    setup_welcome_lede: "このサーバーはまだ初期化されていません。数分で完了するセットアップを始めましょう。",
    setup_welcome_lede2: "次の画面で最初の管理者アカウントを作成します。サーバー起動時に出力されたセットアップトークンをお手元にご準備ください。",
    setup_welcome_begin: "セットアップを始める",
    setup_admin_title: "管理者アカウントの作成",
    setup_admin_lede: "サーバー起動時に出力されたセットアップトークンと、新しい管理者アカウントの情報を入力してください。",
    setup_admin_token_label: "セットアップトークン",
    setup_admin_token_hint: "起動ログに 1 度だけ出力された値",
    setup_admin_username_label: "ユーザー名",
    setup_admin_email_label: "メールアドレス(任意)",
    setup_admin_email_hint: "通知やパスワードリセットに使用します。後から変更できます。",
    setup_admin_display_label: "表示名(任意)",
    setup_admin_password_label: "パスワード",
    setup_admin_password_hint: "12 文字以上",
    setup_admin_confirm_label: "パスワード(確認)",
    setup_admin_submit: "管理者を作成",
    setup_done_title: "セットアップ完了",
    setup_done_lede: "管理者アカウントの作成と初期署名キーの発行が完了しました。管理画面からシステムの設定を確認できます。",
    setup_done_next_steps_title: "次のステップ",
    setup_done_next_step_register_clients: "OIDC クライアントを登録する。",
    setup_done_next_step_enable_mfa: "パスキーや認証アプリで 2 段階認証を有効にする。",
    setup_done_next_step_review_settings: "設定タブで現在の有効な設定を確認する。",
    setup_done_enter_admin: "管理画面へ進む",
    setup_not_complete_title: "セットアップは完了していません",
    setup_not_complete_lede: "管理者アカウントの作成がまだ完了していません。セットアップを最初から始めてください。",
    setup_password_mismatch: "パスワードと確認用パスワードが一致しません。",
    setup_already_initialized: "サーバーは既に初期化されています。",
    setup_invalid_token: "セットアップトークンが正しくありません。",
    setup_hibp_blocked: "このパスワードは過去のデータ漏洩で確認されています。別のものを選んでください。",
    setup_generic_failure: "セットアップに失敗しました。フォームを確認して再度お試しください。",

    // Step-up auth
    step_up_title: "再認証",
    step_up_lede: "セキュリティ上重要な操作を行う前に、認証アプリのコードで本人確認をお願いします。短時間(5 分間)有効です。",
    step_up_code_label: "確認コード",
    step_up_code_hint: "認証アプリの 6 桁コード、またはリカバリーコード。",
    step_up_code_invalid: "コードが正しくありません。もう一度入力してください。",
    step_up_passkey_alt: "または、パスキーで再認証:",
    step_up_passkey_button: "パスキーで再認証",

    // MFA challenge (login flow)
    mfa_challenge_shell_title: "確認が必要",
    mfa_challenge_title: "確認コード",
    mfa_challenge_lede: "認証アプリの 6 桁コード、またはリカバリーコードを入力してください。",
    mfa_challenge_code_label: "コード",
    mfa_challenge_submit: "確認",
    mfa_challenge_passkey_alt: "または、パスキーでサインイン:",
    mfa_challenge_passkey_button: "パスキーでサインイン",
    mfa_challenge_failed_flash: "確認に失敗しました。コードを再入力するか、リカバリーコードをお試しください。",

    // Profile / self-service security (v0.29.1)
    profile_subtitle_template: "{username} のセキュリティ設定",
    profile_recovery_save_now: "リカバリーコードを今すぐ保存してください。再表示はされません。",
    profile_recovery_save_lede: "各コードは 1 度だけ使えます。安全な場所に保管してください。認証アプリへのアクセスを失った場合、6 桁コードの代わりにこのいずれかを入力してサインインできます。",
    profile_mfa_section: "2 段階認証",
    profile_mfa_totp_card_title: "認証アプリ(TOTP)",
    profile_mfa_status_label: "状態:",
    profile_mfa_status_enabled: "有効",
    profile_mfa_status_not_configured: "未設定",
    profile_mfa_regenerate_codes: "リカバリーコード再生成",
    profile_mfa_disable_confirm: "認証アプリによる 2 段階認証を無効化しますか?",
    profile_mfa_disable_button: "TOTP を無効化",
    profile_mfa_enroll_lede: "有効化すると、サインイン時に認証アプリの 6 桁コードが必要になります。",
    profile_mfa_enroll_apps_note: "標準準拠の TOTP アプリならどれでも利用できます(Aegis / FreeOTP / Google Authenticator / 1Password など)。",
    profile_mfa_enroll_button: "TOTP を設定",
    profile_passkeys_section: "パスキー",
    profile_passkeys_lede: "パスキーは、スマートフォン・PC・セキュリティキー・パスワードマネージャに保存されるハードウェア裏付け資格情報です。デバイスから外に出ません。複数登録できます — バックアップとして 2 つ以上登録しておくことを推奨します。",
    profile_passkeys_th_name: "名前",
    profile_passkeys_th_registered: "登録日",
    profile_passkeys_th_last_used: "最終使用",
    profile_passkeys_last_used_never: "(未使用)",
    profile_passkeys_delete_confirm: "このパスキーを削除しますか? このパスキーでのサインインができなくなります。",
    profile_passkeys_delete_button: "削除",
    profile_passkeys_empty: "パスキーは未登録です。",
    profile_passkeys_register_section: "新しいパスキーを登録",
    profile_passkeys_nickname_label: "ニックネーム",
    profile_passkeys_nickname_hint: "例: YubiKey 5C / MacBook Touch ID",
    profile_passkeys_register_button: "パスキーを登録",
    profile_lang_section: "表示言語",
    profile_lang_lede: "サインイン後の画面で使用する言語。「ブラウザに従う」を選ぶとブラウザの設定 (Accept-Language) に応じて自動選択されます。",
    profile_lang_field_label: "言語",
    profile_mfa_enrolled_flash: "2 段階認証を有効化しました。",
    profile_recovery_regenerated_flash: "リカバリーコードを再生成しました。新しいコードを保存してください — 古いものは無効になりました。",

    // MFA setup (TOTP enrollment)
    mfa_setup_shell_title: "2 段階認証の設定",
    mfa_setup_title: "2 段階認証の設定",
    mfa_setup_lede: "認証アプリと sui-id を関連付けます。",
    mfa_setup_steps_title: "手順",
    mfa_setup_step1: "認証アプリを開き、下の QR コードを読み取ってください。手入力する場合は秘密鍵をペーストしてください。",
    mfa_setup_step2: "アプリに表示される 6 桁コードを以下のフォームに入力して確認してください。",
    mfa_setup_step3: "設定完了後、1 度だけ使えるリカバリーコードが 8 個発行されます。安全な場所に保管してください。",
    mfa_setup_qr_card_title: "QR コードと秘密鍵",
    mfa_setup_secret_label: "秘密鍵:",
    mfa_setup_otpauth_summary: "otpauth URI(上級者向け)",
    mfa_setup_verify_card_title: "確認",
    mfa_setup_code_label: "確認コード",
    mfa_setup_code_hint: "アプリに表示されている 6 桁コード",
    mfa_setup_confirm_button: "確認して有効化",

    // Forgot password
    forgot_password_title: "パスワードを忘れた場合",
    forgot_password_lede: "登録したメールアドレスを入力してください。アカウントが存在する場合、リセット用のリンクをお送りします。",
    forgot_password_email_label: "メールアドレス",
    forgot_password_submit: "リセットリンクを送信",
    forgot_password_sent_title: "メールを送信しました",
    forgot_password_sent_lede: "リクエストを受け付けました。アカウントが存在する場合、ご指定のメールアドレスにリセットリンクをお送りしています。",
    forgot_password_sent_lede2: "リンクは 30 分間有効です。届かない場合は迷惑メールフォルダもご確認ください。",
    reset_password_title: "パスワードの再設定",
    reset_password_lede: "新しいパスワードを 2 回入力してください。",
    reset_password_new_label: "新しいパスワード",
    reset_password_new_hint: "12 文字以上",
    reset_password_confirm_label: "確認のためもう一度",
    reset_password_submit: "パスワードを変更",
    reset_password_invalid_title: "このリンクは無効です",
    reset_password_invalid_lede: "リセットリンクが期限切れ、すでに使用済み、または無効です。",
    reset_password_invalid_request_again: "再度リクエストする",
    password_mismatch_flash: "パスワードと確認用パスワードが一致しません。",
    reset_password_failed_flash: "パスワードの再設定に失敗しました。もう一度お試しください。",
    back_to_login: "ログインに戻る",

    // Settings hub
    settings_title: "設定",
    settings_lede: "現在の有効な設定の確認。",
    settings_tab_basic: "基本",
    settings_tab_security: "セキュリティ",
    settings_tab_authentication: "認証",
    settings_tab_logs: "ログ",
    settings_tab_email: "メール",
    settings_tab_other: "その他",
    settings_basic_section: "基本",
    settings_basic_oidc_section: "OIDC 公開エンドポイント",
    settings_basic_issuer: "Issuer",
    settings_basic_listen_addr: "Listen address",
    settings_basic_cookie_secure: "Cookie Secure フラグ",
    settings_basic_trusted_proxies: "Trusted proxies",
    settings_basic_trusted_proxies_none: "(なし — peer の IP を直接信頼)",
    settings_basic_default_lang: "サーバーデフォルト言語",
    settings_basic_default_lang_hint: "ユーザー言語設定が無く Accept-Language ヘッダも一致しない場合のフォールバック。",
    settings_basic_save: "保存",
    settings_basic_saved: "サーバー設定を更新しました。",

    // Profile
    profile_title: "プロフィール",
    profile_username_label: "ユーザー名",
    profile_email_label: "メールアドレス",
    profile_display_name_label: "表示名",
    profile_lang_label: "表示言語",
    profile_lang_hint: "ブラウザのデフォルトを使用するには「ブラウザに従う」を選択してください。",
    profile_lang_browser_default: "ブラウザに従う",
    profile_save: "保存",
    profile_saved: "プロフィールを更新しました。",

    // Password change
    password_change_title: "パスワードの変更",
    password_change_lede: "現在のパスワードと、新しいパスワードを 2 回入力してください。",
    password_change_current_label: "現在のパスワード",
    password_change_new_label: "新しいパスワード",
    password_change_new_hint: "12 文字以上。短く複雑なパスワードよりも、長くランダムなフレーズの方が安全です。",
    password_change_confirm_label: "新しいパスワード(確認)",
    password_change_revoke_others_label: "パスワード変更後、他のブラウザ/アプリをサインアウトする",
    password_change_revoke_others_hint: "推奨。既存のセッションやリフレッシュトークンが無効化され、新しいパスワードでの再サインインが必要になります。",
    password_change_submit: "パスワードを変更",
    password_change_wrong_current: "現在のパスワードが正しくありません。",
    password_change_done_flash: "パスワードを変更しました。",

    // /me/security
    me_security_title: "アカウントセキュリティ",
    me_security_signed_in_as_suffix: " としてサインイン中。",
    me_security_admin_link: "管理画面を開く →",
    me_security_mfa_section: "2 段階認証",
    me_security_mfa_status_label: "状態:",
    me_security_mfa_status_enabled: "有効",
    me_security_mfa_factor_totp: "認証アプリ",
    me_security_mfa_factor_passkey_n: "パスキー {n} 件",
    me_security_mfa_disabled_title: "2 段階認証が無効です。",
    me_security_mfa_disabled_lede: "現在このアカウントはパスワードのみで保護されています。パスキーまたは認証アプリの登録を強く推奨します。",
    me_security_mfa_manage: "認証手段を管理",
    me_security_password_change_link: "パスワードを変更",
    me_security_sessions_section: "サインイン中の場所",
    me_security_sessions_lede: "1 行が 1 つのブラウザセッションです。Revoke を押すとそのブラウザは即座にサインアウトされます。「current session」とマークされているのは現在使用中のセッションです。",
    me_security_sessions_th_started: "開始日時",
    me_security_sessions_th_expires: "期限",
    me_security_sessions_th_factors: "要素",
    me_security_sessions_current_badge: "current session",
    me_security_sessions_revoke: "Revoke",
    me_security_sessions_revoke_confirm: "このセッションをサインアウトしますか?",
    me_security_sessions_revoke_all_others: "他のすべてのセッションをサインアウト",
    me_security_sessions_revoke_all_others_confirm: "現在のセッション以外をすべてサインアウトしますか?",
    me_security_activity_section: "最近のアクティビティ",
    me_security_activity_lede: "あなたのアカウントに関わる認証および管理イベントです。心当たりのない操作がある場合は、すぐにパスワードを変更し、他のセッションをサインアウトしてください。",
    me_security_activity_th_when: "日時",
    me_security_activity_th_event: "イベント",
    me_security_activity_th_result: "結果",
    me_security_activity_th_note: "備考",

    // Email subjects/bodies
    email_subject_password_reset: "パスワードのリセット — sui-id",
    email_subject_password_changed: "パスワードが変更されました — sui-id",
    email_greeting_suffix: "様",
    email_password_reset_intro: "sui-id でパスワードリセットの依頼を受け付けました。以下のリンクから 30 分以内に新しいパスワードを設定してください。",
    email_password_reset_link_label: "パスワードを再設定する",
    email_password_reset_disregard: "このメールに心当たりがない場合は無視してください。",
    email_password_changed_intro: "sui-id のあなたのアカウントのパスワードが変更されました。",
    email_password_changed_security_warning: "心当たりがない場合は、すぐに他のセッションを取り消し、サポートに連絡してください。",
    email_password_changed_link_security: "セキュリティ設定",

    // Errors
    error_generic_title: "エラー",
    error_not_found_title: "ページが見つかりません",
    error_not_found_lede: "お探しのページは存在しないか、移動された可能性があります。",
    error_internal: "予期しないエラーが発生しました。",
    error_too_many_requests_label: "リクエストが多すぎます。しばらく待ってから再度お試しください。",

    // Audit
    audit_title: "監査ログ",
    audit_col_when: "日時",
    audit_col_actor: "実行者",
    audit_col_action: "操作",
    audit_col_target: "対象",
    audit_col_outcome: "結果",
    audit_col_note: "備考",
};

// ---- Strings::EN ----

pub static STRINGS_EN: Strings = Strings {
    // Generic UI
    button_save: "Save",
    button_cancel: "Cancel",
    button_back: "Back",
    button_continue: "Continue",
    button_delete: "Delete",
    button_create: "Create",
    button_confirm: "Confirm",
    button_test: "Test",
    badge_enabled: "Enabled",
    badge_disabled: "Disabled",
    badge_ok: "OK",
    badge_warn: "Warning",
    badge_error: "Error",
    label_optional: "(optional)",
    label_required: "(required)",
    muted_none: "(none)",

    // Navigation
    nav_dashboard: "Dashboard",
    nav_users: "Users",
    nav_clients: "Clients",
    nav_signing_keys: "Signing keys",
    nav_audit: "Audit log",
    nav_settings: "Settings",
    nav_profile: "Profile",
    nav_logout: "Sign out",

    // Login
    login_title: "Sign in",
    login_username_label: "Username",
    login_password_label: "Password",
    login_submit: "Sign in",
    login_passkey_button: "Sign in with passkey",
    login_forgot_password_link: "Forgot your password?",
    login_invalid_credentials: "The username or password is incorrect.",
    login_account_locked: "Your account is temporarily locked. Please try again later.",
    login_reset_ok_banner: "Your password has been reset. Please sign in with the new password.",

    // Setup wizard
    setup_step_welcome: "Welcome",
    setup_step_admin: "Create admin",
    setup_step_done: "Done",
    setup_welcome_title: "Welcome to sui-id",
    setup_welcome_lede: "This server has not been initialized yet. Setup will only take a few minutes.",
    setup_welcome_lede2: "On the next screen you'll create the first administrator account. Please have the setup token (printed when the server started) ready.",
    setup_welcome_begin: "Begin setup",
    setup_admin_title: "Create the administrator account",
    setup_admin_lede: "Enter the setup token printed at server startup, plus the details for the new administrator account.",
    setup_admin_token_label: "Setup token",
    setup_admin_token_hint: "Printed once in the startup log",
    setup_admin_username_label: "Username",
    setup_admin_email_label: "Email address (optional)",
    setup_admin_email_hint: "Used for notifications and password resets. You can change it later.",
    setup_admin_display_label: "Display name (optional)",
    setup_admin_password_label: "Password",
    setup_admin_password_hint: "12 characters or more",
    setup_admin_confirm_label: "Password (confirm)",
    setup_admin_submit: "Create administrator",
    setup_done_title: "Setup complete",
    setup_done_lede: "The administrator account has been created and the initial signing key issued. You can now review system configuration in the admin console.",
    setup_done_next_steps_title: "Next steps",
    setup_done_next_step_register_clients: "Register an OIDC client.",
    setup_done_next_step_enable_mfa: "Enable two-factor authentication with a passkey or authenticator app.",
    setup_done_next_step_review_settings: "Review the current effective configuration on the Settings tab.",
    setup_done_enter_admin: "Go to admin console",
    setup_not_complete_title: "Setup is not complete",
    setup_not_complete_lede: "The administrator account has not been created yet. Please start setup from the beginning.",
    setup_password_mismatch: "The password and the confirmation do not match.",
    setup_already_initialized: "The server is already initialized.",
    setup_invalid_token: "The setup token is incorrect.",
    setup_hibp_blocked: "This password appears in a known breach. Please choose a different one.",
    setup_generic_failure: "Setup failed. Please review the form and try again.",

    // Step-up auth
    step_up_title: "Re-authenticate",
    step_up_lede: "Before performing this security-sensitive action, please confirm your identity with a code from your authenticator app. Valid for 5 minutes.",
    step_up_code_label: "Confirmation code",
    step_up_code_hint: "Six-digit code from your authenticator app, or a recovery code.",
    step_up_code_invalid: "That code is not valid. Please try again.",
    step_up_passkey_alt: "Or re-authenticate with a passkey:",
    step_up_passkey_button: "Re-authenticate with passkey",

    // MFA challenge (login flow)
    mfa_challenge_shell_title: "Verification required",
    mfa_challenge_title: "Verification code",
    mfa_challenge_lede: "Enter the 6-digit code from your authenticator app, or one of your recovery codes.",
    mfa_challenge_code_label: "Code",
    mfa_challenge_submit: "Verify",
    mfa_challenge_passkey_alt: "Or sign in with a passkey:",
    mfa_challenge_passkey_button: "Sign in with passkey",
    mfa_challenge_failed_flash: "Verification failed. Try again, or use a recovery code.",

    // Profile / self-service security (v0.29.1)
    profile_subtitle_template: "Security settings for {username}",
    profile_recovery_save_now: "Save these recovery codes now. They will not be shown again.",
    profile_recovery_save_lede: "Each code can be used once. Keep them somewhere safe. If you lose access to your authenticator app, you can enter any one of these codes in place of the 6-digit code to sign in.",
    profile_mfa_section: "Two-factor authentication",
    profile_mfa_totp_card_title: "Authenticator app (TOTP)",
    profile_mfa_status_label: "Status:",
    profile_mfa_status_enabled: "Enabled",
    profile_mfa_status_not_configured: "Not configured",
    profile_mfa_regenerate_codes: "Regenerate recovery codes",
    profile_mfa_disable_confirm: "Disable authenticator-app two-factor authentication?",
    profile_mfa_disable_button: "Disable TOTP",
    profile_mfa_enroll_lede: "When enabled, sign-in will additionally require a 6-digit code from your authenticator app.",
    profile_mfa_enroll_apps_note: "Any standards-compliant TOTP app will work (Aegis / FreeOTP / Google Authenticator / 1Password and others).",
    profile_mfa_enroll_button: "Set up TOTP",
    profile_passkeys_section: "Passkeys",
    profile_passkeys_lede: "Passkeys are hardware-backed credentials stored on your phone, PC, security key, or password manager. They never leave the device. You can register more than one — we recommend at least two as a backup.",
    profile_passkeys_th_name: "Name",
    profile_passkeys_th_registered: "Registered",
    profile_passkeys_th_last_used: "Last used",
    profile_passkeys_last_used_never: "(never used)",
    profile_passkeys_delete_confirm: "Delete this passkey? You will no longer be able to sign in with it.",
    profile_passkeys_delete_button: "Delete",
    profile_passkeys_empty: "No passkeys registered.",
    profile_passkeys_register_section: "Register a new passkey",
    profile_passkeys_nickname_label: "Nickname",
    profile_passkeys_nickname_hint: "e.g. YubiKey 5C / MacBook Touch ID",
    profile_passkeys_register_button: "Register passkey",
    profile_lang_section: "Display language",
    profile_lang_lede: "The language sui-id uses for screens after sign-in. \"Browser default\" follows your browser's Accept-Language setting.",
    profile_lang_field_label: "Language",
    profile_mfa_enrolled_flash: "Two-factor authentication is now enabled.",
    profile_recovery_regenerated_flash: "Recovery codes regenerated. Save the new ones — the old ones no longer work.",

    // MFA setup (TOTP enrollment)
    mfa_setup_shell_title: "Set up two-factor authentication",
    mfa_setup_title: "Set up two-factor authentication",
    mfa_setup_lede: "Link an authenticator app to sui-id.",
    mfa_setup_steps_title: "Steps",
    mfa_setup_step1: "Open your authenticator app and scan the QR code below. To type the secret instead, copy it from the field below.",
    mfa_setup_step2: "Enter the 6-digit code shown by the app in the form below to verify the link.",
    mfa_setup_step3: "After confirmation, sui-id will issue 8 single-use recovery codes. Save them somewhere safe.",
    mfa_setup_qr_card_title: "QR code and secret",
    mfa_setup_secret_label: "Secret:",
    mfa_setup_otpauth_summary: "otpauth URI (advanced)",
    mfa_setup_verify_card_title: "Verify",
    mfa_setup_code_label: "Verification code",
    mfa_setup_code_hint: "The 6-digit code shown by your app",
    mfa_setup_confirm_button: "Confirm and enable",

    // Forgot password
    forgot_password_title: "Forgot your password",
    forgot_password_lede: "Enter your registered email address. If an account exists, we'll send you a reset link.",
    forgot_password_email_label: "Email address",
    forgot_password_submit: "Send reset link",
    forgot_password_sent_title: "Email sent",
    forgot_password_sent_lede: "Request received. If an account exists for the address you provided, we have sent a reset link.",
    forgot_password_sent_lede2: "The link is valid for 30 minutes. If you don't see it, check your spam folder.",
    reset_password_title: "Reset your password",
    reset_password_lede: "Enter your new password twice.",
    reset_password_new_label: "New password",
    reset_password_new_hint: "12 characters or more",
    reset_password_confirm_label: "Confirm again",
    reset_password_submit: "Change password",
    reset_password_invalid_title: "This link is no longer valid",
    reset_password_invalid_lede: "The reset link has expired, has already been used, or is invalid.",
    reset_password_invalid_request_again: "Request a new link",
    password_mismatch_flash: "The password and the confirmation do not match.",
    reset_password_failed_flash: "Password reset failed. Please try again.",
    back_to_login: "Back to log in",

    // Settings hub
    settings_title: "Settings",
    settings_lede: "Review the current effective configuration.",
    settings_tab_basic: "Basic",
    settings_tab_security: "Security",
    settings_tab_authentication: "Authentication",
    settings_tab_logs: "Logs",
    settings_tab_email: "Email",
    settings_tab_other: "Other",
    settings_basic_section: "Basic",
    settings_basic_oidc_section: "OIDC public endpoints",
    settings_basic_issuer: "Issuer",
    settings_basic_listen_addr: "Listen address",
    settings_basic_cookie_secure: "Cookie Secure flag",
    settings_basic_trusted_proxies: "Trusted proxies",
    settings_basic_trusted_proxies_none: "(none — peer IP is trusted directly)",
    settings_basic_default_lang: "Server default language",
    settings_basic_default_lang_hint: "Used as a fallback when the user has no preferred language and no Accept-Language header matches.",
    settings_basic_save: "Save",
    settings_basic_saved: "Server settings updated.",

    // Profile
    profile_title: "Profile",
    profile_username_label: "Username",
    profile_email_label: "Email address",
    profile_display_name_label: "Display name",
    profile_lang_label: "Display language",
    profile_lang_hint: "Choose \"Browser default\" to follow your browser's language setting.",
    profile_lang_browser_default: "Browser default",
    profile_save: "Save",
    profile_saved: "Profile updated.",

    // Password change
    password_change_title: "Change password",
    password_change_lede: "Enter your current password, and the new password twice.",
    password_change_current_label: "Current password",
    password_change_new_label: "New password",
    password_change_new_hint: "12 characters or more. A long random passphrase is safer than a short complicated one.",
    password_change_confirm_label: "New password (confirm)",
    password_change_revoke_others_label: "Sign out other browsers / apps after changing the password",
    password_change_revoke_others_hint: "Recommended. Existing sessions and refresh tokens are invalidated; you will need to sign in again with the new password.",
    password_change_submit: "Change password",
    password_change_wrong_current: "Current password is incorrect.",
    password_change_done_flash: "Password changed.",

    // /me/security
    me_security_title: "Account security",
    me_security_signed_in_as_suffix: " is currently signed in.",
    me_security_admin_link: "Open admin console →",
    me_security_mfa_section: "Two-factor authentication",
    me_security_mfa_status_label: "Status:",
    me_security_mfa_status_enabled: "enabled",
    me_security_mfa_factor_totp: "authenticator app",
    me_security_mfa_factor_passkey_n: "{n} passkey(s)",
    me_security_mfa_disabled_title: "Two-factor authentication is disabled.",
    me_security_mfa_disabled_lede: "This account is currently protected by a password alone. Registering a passkey or an authenticator app is strongly recommended.",
    me_security_mfa_manage: "Manage factors",
    me_security_password_change_link: "Change password",
    me_security_sessions_section: "Where you are signed in",
    me_security_sessions_lede: "Each row is one browser session. Pressing Revoke immediately signs that browser out. The row marked \"current session\" is the one you are using now.",
    me_security_sessions_th_started: "Started",
    me_security_sessions_th_expires: "Expires",
    me_security_sessions_th_factors: "Factors",
    me_security_sessions_current_badge: "current session",
    me_security_sessions_revoke: "Revoke",
    me_security_sessions_revoke_confirm: "Sign out this session?",
    me_security_sessions_revoke_all_others: "Sign out all other sessions",
    me_security_sessions_revoke_all_others_confirm: "Sign out all sessions except the current one?",
    me_security_activity_section: "Recent activity",
    me_security_activity_lede: "Authentication and administrative events for your account. If you don't recognise something here, change your password and sign out other sessions immediately.",
    me_security_activity_th_when: "When",
    me_security_activity_th_event: "Event",
    me_security_activity_th_result: "Result",
    me_security_activity_th_note: "Note",

    // Email subjects/bodies
    email_subject_password_reset: "Reset your password — sui-id",
    email_subject_password_changed: "Your password was changed — sui-id",
    email_greeting_suffix: "",
    email_password_reset_intro: "We received a password-reset request for your sui-id account. Use the link below within 30 minutes to set a new password.",
    email_password_reset_link_label: "Reset your password",
    email_password_reset_disregard: "If you didn't request this, you can ignore this email.",
    email_password_changed_intro: "The password for your sui-id account was changed.",
    email_password_changed_security_warning: "If you didn't do this, please revoke other sessions immediately and contact support.",
    email_password_changed_link_security: "security settings",

    // Errors
    error_generic_title: "Error",
    error_not_found_title: "Page not found",
    error_not_found_lede: "The page you were looking for doesn't exist or has moved.",
    error_internal: "An unexpected error occurred.",
    error_too_many_requests_label: "Too many requests. Please wait a moment before trying again.",

    // Audit
    audit_title: "Audit log",
    audit_col_when: "When",
    audit_col_actor: "Actor",
    audit_col_action: "Action",
    audit_col_target: "Target",
    audit_col_outcome: "Outcome",
    audit_col_note: "Note",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_round_trip() {
        for &loc in Locale::ALL {
            assert_eq!(Locale::parse(loc.tag()), Some(loc));
        }
    }

    #[test]
    fn parse_tolerates_region_suffix() {
        assert_eq!(Locale::parse("en-US"), Some(Locale::En));
        assert_eq!(Locale::parse("ja_JP"), Some(Locale::Ja));
        assert_eq!(Locale::parse("EN"), Some(Locale::En));
    }

    #[test]
    fn parse_unknown_returns_none() {
        assert_eq!(Locale::parse("zh"), None);
        assert_eq!(Locale::parse(""), None);
        assert_eq!(Locale::parse("xyz-RegionTag"), None);
    }

    #[test]
    fn negotiate_picks_first_recognised() {
        // English has q=0.5 in real life, but our parser ignores
        // weights — the first recognised tag wins.
        assert_eq!(
            negotiate_from_accept_language("fr;q=1, en;q=0.5"),
            Some(Locale::En)
        );
        assert_eq!(
            negotiate_from_accept_language("ja, en"),
            Some(Locale::Ja)
        );
        assert_eq!(negotiate_from_accept_language(""), None);
        assert_eq!(negotiate_from_accept_language("zh, fr"), None);
    }

    #[test]
    fn each_locale_has_strings() {
        for &loc in Locale::ALL {
            // Compile-only check that strings() returns; smoke
            // a couple of fields to confirm both populated.
            let s = loc.strings();
            assert!(!s.button_save.is_empty(), "{:?}.button_save empty", loc);
            assert!(!s.login_title.is_empty(), "{:?}.login_title empty", loc);
        }
    }

    #[test]
    fn native_names_are_in_their_own_script() {
        // Sanity: a user wandering in shouldn't see their own
        // language listed only in someone else's script.
        assert!(STRINGS_JA.button_save.chars().any(|c| c >= '\u{3040}'));
        assert!(Locale::Ja.native_name().contains("日本語"));
        assert!(Locale::En.native_name().is_ascii());
    }
}
