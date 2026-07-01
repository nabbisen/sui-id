//! Japanese (`ja`) translation table and date/number formatters.
//!
//! **Translator guide:**
//! Edit the string values between `\"…\"` only. Do not rename field names.
//! Every field must be present — the compiler enforces completeness.
//! After editing, run `cargo test -p sui-id-i18n` to confirm all tests pass.

use crate::formatters::{Formatters, fmt_count_shared, fmt_time_shared};
use crate::strings::Strings;
use chrono::{DateTime, Datelike, Utc};

// ── Strings ──────────────────────────────────────────────────────────────────

pub static STRINGS_JA: Strings = Strings {
    // Generic UI
    button_save: "保存",
    button_cancel: "キャンセル",
    button_back: "戻る",
    button_continue: "続行",
    button_delete: "削除",
    danger_zone_title: "危険な操作",
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

    // Language native names (RFC 051) — identical across all locales.
    locale_native_ja: "日本語",
    locale_native_en: "English",
    locale_native_zh_hans: "中文（简体）",
    locale_native_zh_hant: "中文（繁体）",

    // Lifetime formatting (RFC 051)
    fmt_lifetime_days: |n, secs| format!("{n} 日 ({secs}s)"),
    fmt_lifetime_hours: |n, secs| format!("{n} 時間 ({secs}s)"),
    fmt_lifetime_minutes: |n, secs| format!("{n} 分 ({secs}s)"),

    // Settings: Auth extensions (RFC 051)
    settings_auth_min_length_value: |n| format!("{n} 文字"),
    settings_auth_recovery_codes_label: "リカバリーコード（登録ごと）",
    settings_auth_recovery_codes_value: |n| format!("{n} 件"),
    settings_auth_mfa_note_prefix: "ユーザー個別に有効化します。詳細は ",
    settings_auth_mfa_note_suffix: " を参照してください。",

    // Settings: Logs extensions (RFC 051)
    settings_logs_lede: "ログ出力設定と監査ログの状態。",
    settings_logs_kv_format: "Format",
    settings_logs_kv_filter: "Filter",
    settings_logs_audit_link_prefix: "詳細な履歴は ",
    settings_logs_audit_link_suffix: " を参照してください。",

    // Settings: Advanced/Other extensions (RFC 051)
    settings_advanced_lede: "ビルド情報・スキーマ・ストレージ。",
    settings_advanced_storage_note_prefix: "DB は単一の SQLite ファイル、マスターキーは環境変数 ",
    settings_advanced_storage_note_suffix: " が指定されない場合のみキーファイルから読み込まれます。",
    settings_advanced_users_count: |n| format!("{n} 名 "),
    settings_advanced_clients_count: |n| format!("{n} 件 "),

    // Settings: Email extensions (RFC 051)
    settings_email_enable_checkbox: "メール機能を有効化",
    settings_email_enable_hint: "オフの場合、forgot-password などのメール送信エンドポイントは無効になります。",
    settings_email_password_placeholder_change: "（変更する場合のみ入力）",
    settings_email_password_placeholder_none: "（認証不要の場合は空欄）",
    settings_email_password_hint: "保存済みのパスワードを変更する場合のみ入力してください。空欄の場合は既存値を保持します。",
    settings_email_base_url_hint: "リセットメールに記載する URL のベース。Issuer URL とは別に明示できます。",
    settings_email_save_button: "設定を保存",
    settings_email_test_section: "接続テスト",
    settings_email_test_lede: "現在の設定で SMTP サーバーへの接続と認証を試みます。メールは送信されません。",

    // Navigation
    nav_dashboard: "ダッシュボード",
    nav_users: "ユーザー",
    nav_clients: "クライアント",
    nav_signing_keys: "署名キー",
    nav_audit: "監査ログ",
    nav_settings: "設定",
    nav_profile: "プロフィール",
    nav_apps: "アプリ",
    nav_my_account: "マイアカウント",
    settings_tab_general: "一般",
    settings_tab_advanced: "詳細",
    me_overview_last_login: "最終サインイン: {date}。",
    me_overview_first_login: "ようこそ — 初めてのサインインです。",
    nav_security: "セキュリティ",
    nav_logout: "サインアウト",
    a11y_skip_to_main: "メインコンテンツへスキップ",
    nav_aria_main: "メインナビゲーション",
    nav_aria_signout: "サインアウト",

    // Footer (RFC 050)
    footer_tagline: "🌱 sui-id · 静かで、凛として、やさしい ID 基盤を。",
    footer_a11y_group_label: "アクセシビリティ対応",
    a11y_keyboard: "キーボード対応",
    a11y_screen_reader: "スクリーンリーダー対応",
    a11y_contrast: "コントラスト対応",

    // Theme toggle (RFC 050)
    theme_toggle_group: "テーマ",
    theme_toggle_light: "ライト",
    theme_toggle_auto: "自動",
    theme_toggle_dark: "ダーク",
    theme_toggle_light_title: "ライトテーマ",
    theme_toggle_auto_title: "OS の設定に従う",
    theme_toggle_dark_title: "ダークテーマ",

    // Status words (RFC 052)
    status_active: "有効",
    status_disabled: "無効",
    status_deleted: "削除済み",
    status_admin: "管理者",
    status_on: "オン",
    status_off: "オフ",
    status_in_use: "使用中",
    status_retired: "引退",
    status_published: "公開中",
    status_pending: "保留中",
    status_healthy: "正常",
    status_unhealthy: "異常",

    // Empty placeholders (RFC 052)
    empty_dash: "—",
    empty_any: "（すべて）",
    empty_none: "（なし）",
    empty_falls_back_redirect_uris: "（redirect_uris にフォールバック）",
    empty_no_email: "（メールアドレスなし）",
    empty_not_set: "（未設定）",

    // Copy-to-clipboard button (RFC 053)
    copy_button_label: "📋 コピー",
    copy_button_label_done: "✓ コピー済み",
    copy_button_aria_template: "{noun} をコピー",
    copy_noun_client_id: "クライアント ID",
    copy_noun_client_secret: "クライアントシークレット",
    copy_noun_jwks_uri: "JWKS URI",
    copy_noun_redirect_uri: "リダイレクト URI",
    copy_noun_audit_row_id: "監査行 ID",
    copy_noun_setup_token: "セットアップトークン",
    copy_noun_recovery_code: "リカバリーコード",
    copy_noun_passkey_id: "パスキー ID",

    // Login
    signed_out_flash: "サインアウトしました。",
    login_title: "サインイン",
    login_username_label: "ユーザー名",
    login_password_label: "パスワード",
    login_submit: "サインイン",
    login_passkey_button: "パスキーでサインイン",
    login_no_admin_access: "このアカウントには管理パネルへのアクセス権がありません。",
    login_passkey_primary: "パスキーでサインイン",
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

    // RFC 012: Setup wizard — Language step
    setup_step_lang: "言語",
    setup_lang_title: "表示言語の設定",
    setup_lang_lede: "管理画面とログイン画面に使用する言語を選択してください。あとから設定で変更できます。",
    setup_lang_field_label: "表示言語",
    setup_lang_default_note: "あとから管理画面の設定で変更できます。",
    setup_lang_submit: "次へ",

    // RFC 012: Setup wizard — HIBP step
    setup_step_hibp: "セキュリティ",
    setup_hibp_step_title: "パスワードセキュリティポリシー",
    setup_hibp_step_lede: "Have I Been Pwned を使用して、既知の漏洩パスワードを検出できます。パスワードの先頭 5 文字のみを送信するため、プライバシーが守られます。",
    setup_hibp_option_off: "無効",
    setup_hibp_option_off_desc: "漏洩チェックを行いません。",
    setup_hibp_option_warn: "警告（推奨）",
    setup_hibp_option_warn_desc: "漏洩パスワードを警告しますが、設定は許可します。",
    setup_hibp_option_block: "ブロック",
    setup_hibp_option_block_desc: "既知の漏洩パスワードは拒否します。",
    setup_hibp_step_default_note: "あとから管理画面の設定で変更できます。",
    setup_hibp_step_submit: "次へ",

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
    // Self-service MFA tab extensions (RFC 055, 056)
    me_security_mfa_recovery_section_label: "リカバリーコード",
    me_security_mfa_recovery_codes_remaining: |n| format!("残り {n} 件"),
    me_security_language_saved_banner: "言語設定を保存しました。",
    setup_steps_aria: "セットアップ手順",
    me_security_tabs_aria: "セキュリティ設定タブ",
    settings_tabs_aria: "設定タブ",
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

    // 設定タブ (RFC 023: "その他" → "詳細")

    // 監査イベントラベル (RFC 002 § D)
    audit_event_auth_login_success: "ログイン",
    audit_event_auth_login_failure: "ログイン失敗",
    audit_event_auth_login_locked: "アカウントロック",
    audit_event_auth_login_mfa_required: "MFA 要求",
    audit_event_auth_logout: "ログアウト",
    audit_event_auth_mfa_success: "MFA 認証成功",
    audit_event_auth_mfa_failure: "MFA 認証失敗",
    audit_event_auth_password_changed_self: "パスワード変更",
    audit_event_auth_password_reset_requested: "パスワードリセット申請",
    audit_event_auth_password_reset_email_sent: "リセットメール送信",
    audit_event_auth_password_reset_email_failed: "リセットメール送信失敗",
    audit_event_auth_password_reset_throttled: "リセット制限",
    audit_event_auth_password_reset_completed: "パスワードリセット完了",
    audit_event_auth_refresh_theft_detected: "トークン盗用検知",
    audit_event_auth_session_revoked: "セッション失効",
    audit_event_auth_sessions_bulk_revoke_self: "他のセッション一括失効",
    audit_event_auth_smtp_config_changed: "SMTP 設定変更",
    audit_event_user_create: "ユーザー作成",
    audit_event_user_delete: "ユーザー削除",
    audit_event_user_reset_password: "パスワードリセット（管理者）",
    audit_event_admin_user_unlock: "アカウントロック解除",
    audit_event_client_create: "クライアント作成",
    audit_event_client_update: "クライアント更新",
    audit_event_client_delete: "クライアント削除",
    audit_event_client_set_allowed_scopes: "クライアントスコープ更新",
    audit_event_signing_key_rotate: "署名鍵ローテーション",
    audit_event_signing_key_delete: "署名鍵削除",
    audit_event_admin_master_key_rotated: "マスターキーローテーション",
    audit_event_setup_create_initial_admin: "初期管理者作成",

    // Admin: ダッシュボード (RFC 029)
    dashboard_title: "ダッシュボード",
    dashboard_lede: "システムの概要と最近のアクティビティ。",
    dashboard_stat_users: "ユーザー",
    dashboard_stat_clients: "クライアント",
    dashboard_stat_sessions: "アクティブセッション",
    dashboard_stat_service_status: "サービス状態",
    dashboard_stat_service_ok: "稼働中",
    dashboard_activity_title: "サインイン活動",
    dashboard_activity_period: "期間",
    // Dashboard extensions (RFC 051)
    dashboard_greeting: |u| format!("こんにちは、{u} さん。"),
    dashboard_aria_stats: "統計情報",
    dashboard_aria_action_required: "対応が必要な項目",
    dashboard_action_required_title: "対応が必要",
    dashboard_activity_success: "成功",
    dashboard_activity_failure: "失敗",
    dashboard_activity_hover_hint: "ホバーで各バケットの詳細を表示。",
    dashboard_oidc_endpoint_discovery: "Discovery",
    dashboard_oidc_endpoint_jwks: "JWKS",
    dashboard_sparkline_aria: "サインイン活動のスパークライン",
    dashboard_sparkline_tooltip: |label, success, failure| {
        format!("{label} : 成功 {success} / 失敗 {failure}")
    },

    // ダッシュボード：オペレータープロンプト (RFC 031)
    dashboard_warn_smtp: "パスワードリセットメールが無効です。設定 → メール で SMTP を設定してください。",
    dashboard_warn_hibp: "パスワード漏洩チェックがオフです。設定 → 認証 で有効にすることを推奨します。",
    dashboard_warn_cookie_insecure: "Cookie Secure フラグがオフです。本番環境では設定 → セキュリティ で有効にしてください。",
    dashboard_warn_admins_no_mfa: |n| format!("管理者{n}名にMFA未設定。"),
    dashboard_warn_old_signing_key: |age| {
        format!("最も古い署名鍵は{age}日経過 — ローテーション推奨。")
    },
    dashboard_warn_outbox_stuck: |n| format!("{n}件のメールが1時間以上送信待ち。"),
    dashboard_warn_pending_resets: |n| format!("未使用のパスワードリセットリンク{n}件。"),
    dashboard_getting_started_title: "はじめに",
    dashboard_getting_started_smtp: "SMTPを設定 — パスワードリセットメール送信に必要",
    dashboard_getting_started_first_app: "最初のOIDCアプリケーションを追加",
    dashboard_getting_started_admin_mfa: "管理者アカウントにMFAを有効化",

    // Admin: ユーザー管理 (RFC 029)
    users_title: "ユーザー管理",
    users_lede: "ユーザーアカウントの作成と管理。",
    users_create_section: "新しいユーザーを追加",
    users_create_button: "ユーザーを作成",
    users_table_section: "ユーザー一覧",
    users_table_th_status: "状態",
    users_table_th_mfa: "MFA",
    users_is_admin_label: "管理者権限を付与する",
    // Users extensions (RFC 051)
    users_count_caption: |n| format!("現在 {n} 名。"),
    users_label_username: "ユーザー名",
    users_label_display_name: "表示名（任意）",
    users_label_email: "メールアドレス（任意）",
    users_label_password: "パスワード（12 文字以上）",

    // Admin: クライアント編集 (RFC 051)
    client_edit_title: "クライアントを編集",
    client_edit_basic_section: "基本情報",
    client_edit_basic_note: "Client ID・種別（confidential/public）・client secret は作成時に固定されます。これらを変更したい場合は削除して登録し直してください。",
    client_edit_new_secret_label: "新しい client secret（このページでのみ表示）：",
    client_edit_label_client_id: "Client ID",
    client_edit_label_kind: "種別",
    client_edit_label_status: "状態",
    client_edit_post_logout_hint: "1 行に 1 つ。空欄＝Redirect URIs を流用。",

    // Admin: クライアント管理 (RFC 029)
    clients_title: "クライアント管理",
    clients_lede: "OIDC クライアントの登録と管理。",
    clients_create_section: "新しいクライアントを登録",
    clients_table_section: "登録済みクライアント",
    clients_secret_once_banner: "クライアント Secret は今だけ表示されます。安全な場所に保存してください。",
    clients_table_th_name: "名前",
    clients_table_th_kind: "種別",
    clients_table_th_status: "状態",
    clients_single_realm_note: "sui-id は単一レルム IdP です。すべてのユーザーがすべてのクライアントを利用できます。スコープはユーザーを制限するのではなく、クライアントが要求できる情報の範囲を制限します。",
    // Clients page extensions (RFC 051)
    clients_table_th_client_id: "クライアント ID",
    clients_count_caption: |n| format!("現在 {n} 件。"),
    clients_label_app_name: "アプリケーション名",
    clients_label_redirect_uris: "Redirect URIs",
    clients_hint_redirect_uris: "1 行に 1 つ。https またはループバックの http のみ。",
    clients_label_allowed_scopes: "許可スコープ (Allowed scopes)",
    clients_hint_scopes_intro: "スペース区切り。既知のスコープ: ",
    clients_hint_scopes_openid_note: " (必須) · ",
    clients_hint_scopes_profile_note: " (名前・言語) · ",
    clients_hint_scopes_email_note: " (メール) · ",
    clients_hint_scopes_offline_note: " (リフレッシュトークン)。",
    clients_hint_scopes_default: "空欄の場合は openid profile email がデフォルトになります。",
    clients_label_post_logout_uris: "Post-logout redirect URIs(任意)",
    clients_hint_one_per_line: "1 行に 1 つ。",
    clients_label_confidential_checkbox: "Confidential client（client secret を発行する）",
    clients_button_register: "登録",

    // Admin: 監査ログ (RFC 029)
    audit_lede: "管理操作の履歴（新しい順）。",

    // 監査ログ強化 (RFC 033)
    audit_chain_ok: "監査チェーンは正常です。",
    audit_chain_broken: "監査チェーンの整合性チェックに失敗しました。早急に調査してください。",
    audit_filter_label: "イベントで絞り込む",
    audit_filter_placeholder: "例: auth.login",
    audit_export_csv: "CSV でエクスポート",
    // Audit log extensions (RFC 051)
    audit_entry_count_caption: |n| format!("({n} 件)"),
    audit_filter_button: "フィルター",
    audit_chain_broken_note: |seq| format!("seq={seq} で不一致を検出。すぐに調査してください。"),
    audit_chain_ok_note: |checked, legacy| {
        format!("末尾 {checked} 行を検査。レガシー(v0.17 以前)未ハッシュ行: {legacy}")
    },

    // Admin: 署名キー (RFC 029)
    signing_keys_title: "署名キー",
    signing_keys_lede: "JWT 署名用 Ed25519 キーの管理。",
    signing_keys_rotate_section: "キーローテーション",
    signing_keys_rotate_button: "署名キーをローテーション",
    signing_keys_table_section: "全キー",
    signing_keys_th_algorithm: "アルゴリズム",
    signing_keys_th_status: "状態",
    signing_keys_th_created: "作成日",
    signing_keys_th_retired: "退役日",
    signing_keys_in_use_badge: "(使用中)",
    // Signing keys extensions (RFC 051)
    signing_keys_count_caption: |n| format!("{n} 件登録。"),
    signing_keys_th_key_id: "Key ID",
    signing_keys_rotate_explanation_1: "ローテーションを実行すると、新しい署名キーが発行され、現行キーは「退役」状態に遷移します。",
    signing_keys_rotate_explanation_2: "退役キーは JWKS に残るため、有効期限内の既発行トークンは検証可能です。",
    signing_keys_rotate_explanation_3: "それらが期限切れになった後、このページから安全に削除できます。",

    // 危険操作の確認画面 (RFC 030)
    confirm_cancel: "キャンセル",
    badge_recoverable: "復旧可能",
    badge_not_recoverable: "不可逆",
    confirm_disable_title: "ユーザーを停止しますか？",
    confirm_disable_impact: "このユーザーは再有効化されるまでサインインできなくなります。",
    confirm_disable_reversibility: "ユーザー一覧から元に戻せます。",
    confirm_disable_button: "ユーザーを停止",
    confirm_enable_title: "ユーザーを再有効化しますか？",
    confirm_enable_button: "ユーザーを有効化",
    confirm_delete_user_title: "ユーザーを削除しますか？",
    confirm_delete_user_impact: "このユーザーはユーザー一覧から完全に削除されます。監査履歴は保持されます。",
    confirm_delete_user_reversibility: "管理パネルからは元に戻せません。",
    confirm_delete_user_button: "ユーザーを削除",
    confirm_reset_mfa_title: "2 段階認証をリセットしますか？",
    confirm_reset_mfa_impact: "ユーザーの TOTP 認証アプリとすべてのパスキーが削除されます。次回サインイン時に再登録が必要になります。",
    confirm_reset_mfa_reversibility: "この操作後、ユーザーは再登録できます。",
    confirm_reset_mfa_button: "MFA をリセット",
    confirm_delete_client_title: "クライアントを削除しますか？",
    confirm_delete_client_impact: "この OIDC クライアントは完全に削除されます。このクライアントに発行されたすべてのセッションとリフレッシュトークンが失効します。",
    confirm_delete_client_reversibility: "この操作は元に戻せません。",
    confirm_delete_client_button: "クライアントを削除",
    confirm_delete_signing_key_title: "署名キーを削除しますか？",
    confirm_delete_signing_key_impact: "このキーで署名されたまだ有効期限が切れていないトークンは即座に検証に失敗します。",
    confirm_delete_signing_key_reversibility: "この操作は元に戻せません。トークンがすべて期限切れになった後にのみ削除してください。",
    confirm_delete_signing_key_button: "署名キーを削除",
    error_403_auditor_title: "読み取り専用アクセス",
    error_403_auditor_body: "お使いのアカウントは読み取り専用（監査者）アクセスです。この操作には管理者権限が必要です。",
    client_detail_readonly_title: "アプリの詳細",
    confirm_rotate_signing_key_title: "署名鍵のローテーション",
    confirm_rotate_signing_key_impact: "新しい署名鍵が発行されます。以前の鍵で署名されたトークンは有効期限まで引き続き有効です。",
    confirm_rotate_signing_key_reversibility: "この操作は元に戻せません。以前の鍵は廃止されます。",
    confirm_rotate_signing_key_button: "鍵をローテーション",
    confirm_email_settings_title: "メール設定の確認",
    confirm_email_settings_impact: "以下のメール設定が保存されます：",
    confirm_email_settings_button: "設定を保存",
    login_title_admin: "sui-id の管理にサインイン",
    login_body_admin: "管理者または監査者アカウントを使用してください。",
    login_title_self_service: "セキュリティ管理にサインイン",
    login_body_self_service: "MFA、パスキー、セッション、パスワードを管理します。",
    login_body_oidc: "sui-id がこのアプリケーションのためにあなたの本人確認を行います。",
    theme_noscript_note: "テーマはシステム設定に従います。",
    empty_users: "まだユーザーがいません。",
    empty_users_cta: "最初のユーザーを作成",
    empty_clients: "まだアプリケーションが登録されていません。",
    empty_clients_cta: "最初のアプリケーションを登録",
    empty_signing_keys: "署名鍵が見つかりません。",
    empty_audit: "まだ監査イベントがありません。",
    error_summary_heading: "以下を修正してください：",
    dynamic_reg_token_issued_flash: "登録トークンを発行しました。",
    dynamic_reg_token_revoked_flash: "登録トークンを無効にしました。",
    dynamic_reg_registered_flash: "クライアントを登録しました。",
    dynamic_reg_token_invalid: "登録トークンが無効、使い切り、または期限切れです。",
    dynamic_reg_token_expired: "登録トークンの有効期限が切れています。",
    scope_catalog_created_flash: "スコープを作成しました。",
    scope_catalog_deleted_flash: "スコープを削除しました。",
    button_edit: "編集",
    button_view_detail: "詳細を表示",

    // Admin: ユーザー詳細 (RFC 035)
    user_detail_back: "← ユーザー一覧へ",
    user_detail_auth_section: "認証",
    user_detail_totp_label: "認証アプリ (TOTP):",
    user_detail_passkeys_label: "パスキー:",
    user_detail_sessions_section: "アクティブセッション",
    user_detail_sessions_th_started: "開始日時",
    user_detail_sessions_th_expires: "期限",
    user_detail_sessions_th_factors: "認証方式",
    user_detail_activity_section: "最近のアクティビティ",
    user_detail_danger_zone_body: "これらの操作はユーザーのアクセスに影響し、取り消せない場合があります。各操作には確認が必要です。",
    role_admin: "管理者",
    role_auditor: "監査者",
    role_user: "ユーザー",
    user_detail_role_section: "アクセス権限",
    user_detail_role_change: "ロール変更",
    user_detail_role_saved: "ロールを更新しました。",
    user_detail_role_last_admin: "最後の管理者は降格できません。",

    // 設定セクションキー
    settings_page_title_template: "設定",
    settings_basic_description: "現在の有効な設定を確認します。値を変更するには sui-id.toml を編集して再起動してください。",
    // Settings: Basic extensions (RFC 051)
    settings_basic_default_lang_hint: "ユーザーの言語設定とブラウザの Accept-Language が一致しない場合のフォールバックとして使用されます。",
    settings_basic_kv_issuer: "Issuer",
    settings_basic_kv_listen: "Listen address",
    settings_basic_kv_cookie_secure: "Cookie Secure フラグ",
    settings_basic_kv_trusted_proxies: "Trusted proxies",
    settings_security_session_section: "セッション制限",
    settings_security_session_lede: "アイドルタイムアウトと同時セッション数の上限。どちらも 0 で無効（デフォルト）、運用ポリシーに応じて設定します。",
    settings_security_idle_timeout_label: "アイドルタイムアウト（秒）",
    settings_security_max_sessions_label: "1 ユーザーあたり最大同時セッション数",
    settings_security_lockout_section: "アカウントロックアウト",
    settings_security_headers_section: "セキュリティヘッダー",
    // Settings: Security extensions (RFC 051)
    settings_security_idle_timeout_hint: "0 = 無効。0 < N ≤ 2,592,000 (= 30 日)。",
    settings_security_max_sessions_hint: "0 = 無効。1 ≤ N ≤ 1000。超過時は最も古いセッションが自動 revoke (FIFO)。",
    settings_security_lockout_hint_1: "段階的バックオフの上限値。プログレッシブな失敗時間が積み重なってもこの値を超えません。",
    settings_security_lockout_hint_2_pre: "管理者は ",
    settings_security_lockout_hint_2_post: " コマンドでいつでも解除できます。",
    settings_security_headers_perm_policy_label: "Permissions-Policy（最小）",
    settings_security_headers_hint: "管理画面はすべて上記ヘッダーを返します。/oauth2/* 系の公開エンドポイントは仕様上の必要に応じて一部省略します。",
    settings_security_cors_token_label: "Token endpoint の動的許可（登録 redirect_uris の origin）",
    settings_security_cors_public_label: "Discovery / JWKS / userinfo の公開許可（*）",
    settings_auth_password_section: "パスワード",
    settings_auth_mfa_section: "2 段階認証",
    settings_auth_oidc_section: "OIDC / トークン設定",
    settings_logs_output_section: "ログ出力",
    settings_logs_audit_section: "監査ログ ハッシュチェーン",
    settings_advanced_build_section: "ビルド情報",
    settings_advanced_storage_section: "ストレージ",
    settings_advanced_record_counts: "レコード数",

    // OIDC 同意画面 (RFC 038)
    consent_title: "アクセスを許可しますか？",
    consent_app_wants_access: "が以下にアクセスを求めています：",
    consent_scope_openid: "本人確認",
    consent_scope_profile: "プロフィール（名前・言語）",
    consent_scope_email: "メールアドレス",
    consent_scope_offline_access: "ログイン状態を維持（リフレッシュトークン）",
    consent_scope_openid_desc: "サインインを確認し、固有IDを提供します。",
    consent_scope_profile_desc: "名前、言語設定、タイムゾーン。",
    consent_scope_email_desc: "メールアドレスと確認状況。",
    consent_scope_offline_access_desc: "不在中もアプリが代理でサインインを維持します。",
    consent_approve: "許可",
    consent_deny: "拒否",
    consent_policy_label: "同意ポリシー",
    consent_policy_none: "なし（同意画面をスキップ）",
    consent_policy_first_time: "初回のみ",
    consent_policy_always: "毎回確認",

    // 設定: ページタイトル (RFC 039)
    settings_title_basic: "設定 — 基本",
    settings_title_security: "設定 — セキュリティ",
    settings_title_authentication: "設定 — 認証",
    settings_title_logs: "設定 — ログ",
    settings_title_email: "設定 — メール",
    settings_title_advanced: "設定 — 詳細",
    // 設定: 認証タブ本文 (RFC 039)
    settings_auth_min_length_label: "最小文字数",
    settings_auth_hash_algorithm_label: "ハッシュアルゴリズム",
    settings_auth_mfa_totp: "TOTP（認証アプリ）",
    settings_auth_mfa_passkey: "WebAuthn（パスキー）",
    settings_auth_mfa_recovery_label: "登録ごとのリカバリーコード数",
    settings_auth_access_token_ttl: "Access token 有効期限",
    settings_auth_id_token_ttl: "ID token 有効期限",
    settings_auth_refresh_token_ttl: "Refresh token 有効期限",
    settings_auth_refresh_rotate: "Refresh ローテーション",
    settings_auth_refresh_theft: "Refresh 盗難検知（ファミリー失効）",
    settings_auth_pkce_required: "PKCE 必須（全 client・全 flow）",
    // 設定: ログタブ本文 (RFC 039)
    settings_logs_recent_24h: "直近 24 時間のイベント",
    settings_logs_chain_broken_note: "ハッシュチェーンの不一致を検出しました。早急に調査してください。",
    settings_logs_chain_ok_note: "ハッシュチェーンは正常です。",
    // 設定: 詳細タブ本文 (RFC 039)
    settings_advanced_version_label: "sui-id バージョン",
    settings_advanced_schema_label: "スキーマバージョン",
    settings_advanced_server_time_label: "サーバ時刻",
    settings_advanced_db_file_label: "DB ファイル",
    settings_advanced_key_file_label: "マスターキーファイル",
    settings_advanced_manage_link: "管理 →",
    // 設定: メールタブ本文 (RFC 039)
    settings_email_page_title: "設定 — メール",
    settings_email_lede: "パスワードリセットや変更通知メールの SMTP 設定。",
    settings_email_smtp_section: "SMTP 接続",
    settings_email_enable_label: "メール送信を有効化",
    settings_email_host_label: "SMTP ホスト",
    settings_email_port_label: "ポート",
    settings_email_port_hint: "587（STARTTLS）または 465（暗黙 TLS）が一般的です。",
    settings_email_tls_label: "TLS モード",
    settings_email_tls_implicit: "暗黙 TLS（465）",
    settings_email_username_label: "ユーザー名（任意）",
    settings_email_from_addr_label: "送信元アドレス",
    settings_email_from_name_label: "送信元表示名（任意）",
    settings_email_base_url_label: "公開 Base URL",
    settings_email_test_button: "接続をテスト",
    settings_email_test_hint: "現在の設定で SMTP サーバーへの接続を試みます。メールは送信されません。",

    // エラーページ (RFC 042)
    error_generic_lede: "リクエストを処理できませんでした。",
    error_request_id_label: "リクエスト ID",
    error_back_home: "ホームへ戻る",

    // RFC 042
    error_internal_lede: "問題が発生しました。サーバー管理者にお問い合わせください。",

    // RFC 042
    error_too_many_requests_lede: "しばらく時間をおいてから、もう一度お試しください。",

    dashboard_recent_events_title: "最近の重要イベント",

    dashboard_recent_events_empty: "重要なイベントはありません。",

    dashboard_recent_events_view_all: "全件を見る →",

    // /me/security タブ (RFC 040)
    me_tab_overview: "概要",
    me_tab_password: "パスワード",
    me_tab_apps: "アプリ",
    me_apps_title: "認証済みアプリ",
    me_apps_intro: "あなたとしてサインインできるアプリ。いつでも取り消せます。",
    me_apps_granted_on: "許可日",
    me_apps_last_used: "最終使用",
    me_apps_never_used: "未使用",
    me_apps_revoke_button: "アクセスを取り消す",
    me_apps_revoked: "アクセスを取り消しました。アプリは再度許可を求める必要があります。",
    me_apps_empty: "認証済みのアプリはありません。",
    me_tab_mfa: "MFA",
    me_tab_passkey: "パスキー",
    me_tab_sessions: "セッション",
    me_tab_language: "言語",
    me_overview_section_status: "セキュリティ状態",
    me_overview_section_activity: "最近のアクティビティ",
    me_overview_label_mfa_totp: "MFA（TOTP）",
    me_overview_label_passkeys: "パスキー",
    me_overview_no_recent_events: "最近の操作はまだ記録されていません。",
    setup_welcome_lang_picker_label: "言語選択",
    me_passkey_origin_warning: "パスキーは HTTPS または localhost 上でのみ使用できます。",
    me_passkey_section_title: "登録済みパスキー",
    me_passkey_button_rename: "名前を変更",
    me_passkey_nickname_label: "ニックネーム",
    me_passkey_nickname_placeholder: "例: YubiKey 5C",
    me_language_title: "表示言語",
    me_language_lede: "優先言語を設定します。未設定の場合はブラウザ設定またはサーバー既定値が使用されます。",
    me_language_use_default: "システム既定（Cookie / Accept-Language）",
    me_language_saved_flash: "言語設定を保存しました。",

    disable_reason_label: "無効化の理由（任意）",

    disable_reason_placeholder: "例: 退職のため、不審なアクティビティのため",

    disable_reason_hint: "監査ログに記録されます。将来の管理者が経緯を確認できます。",
};

// ── Formatters ───────────────────────────────────────────────────────────────

const JA_MONTHS: &[&str] = &[
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12",
];

fn ja_fmt_date(dt: DateTime<Utc>) -> String {
    format!(
        "{}年{}月{}日",
        dt.year(),
        JA_MONTHS[(dt.month() - 1) as usize],
        dt.day()
    )
}

fn ja_fmt_date_time(dt: DateTime<Utc>) -> String {
    format!("{} {}", ja_fmt_date(dt), fmt_time_shared(dt))
}

fn ja_fmt_relative(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - at).num_seconds();
    if secs < 0 {
        return "たった今".into();
    }
    if secs < 60 {
        return format!("{secs} 秒前");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins} 分前");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours} 時間前");
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{days} 日前");
    }
    let months = days / 30;
    if months < 12 {
        return format!("{months} ヶ月前");
    }
    let years = months / 12;
    format!("{years} 年前")
}

/// Japanese (ja) date and number formatters.
pub static FORMATTERS_JA: Formatters = Formatters {
    fmt_date: ja_fmt_date,
    fmt_time: fmt_time_shared,
    fmt_date_time: ja_fmt_date_time,
    fmt_relative: ja_fmt_relative,
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
    fn ja_date_formatting() {
        let dt = ts(2024, 5, 12, 14, 7);
        assert_eq!(ja_fmt_date(dt), "2024年5月12日");
        assert_eq!(ja_fmt_date_time(dt), "2024年5月12日 14:07");
    }

    #[test]
    fn ja_relative_formatting() {
        let now = ts(2024, 5, 12, 15, 0);
        assert_eq!(ja_fmt_relative(ts(2024, 5, 12, 14, 57), now), "3 分前");
        assert_eq!(ja_fmt_relative(ts(2024, 5, 12, 12, 0), now), "3 時間前");
        assert_eq!(ja_fmt_relative(ts(2024, 5, 9, 15, 0), now), "3 日前");
    }
}
