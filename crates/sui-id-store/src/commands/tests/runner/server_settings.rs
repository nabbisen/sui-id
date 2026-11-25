//! RFC 094 M2b stage 3 — S01–S06, the six sealed `server_settings`/
//! `smtp_config` (direct save) commands. Lean pattern, same as stage 1's
//! S07/S09: one exactly-once test, one injected-failure rollback test,
//! per command.
//!
//! For S01, S03, S04 and S05, the exactly-once test **is** the dispatch's
//! own "the production path now produces a record where it previously
//! produced none" evidence (§4) — each calls the sealed command with
//! exactly the shape its real HTTP (or CLI, for S05) caller uses, and
//! before this stage none of the four had any emission site at all for
//! that path (S01's admin-settings-page path; S03/S04/S05 had no
//! atomic path to compare against in the first place). S02 and S06
//! already had a record before this stage (via the setup wizard's
//! atomic pair and the best-effort append, respectively), so their
//! exactly-once tests carry only G20's usual duty, not this one.

use super::*;

// ── S01 — change default language ────────────────────────────────────────

#[tokio::test]
async fn s01_change_default_lang_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::change_default_lang(&db, admin, "en".into(), Utc::now())
        .await
        .expect("change default lang");
    assert_eq!(audited.into_inner(), ("ja".to_string(), "en".to_string()));

    let settings = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings");
    assert_eq!(settings.default_lang, "en");

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.default_language.changed")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S01");
}

#[tokio::test]
async fn s01_injected_failure_before_append_rolls_back_the_change() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before_lang = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .default_lang;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::change_default_lang(&db, admin, "en".into(), Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let after_lang = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .default_lang;
    assert_eq!(after_lang, before_lang, "the setting must not have changed");
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S01");
}

// ── S02 — change HIBP mode ────────────────────────────────────────────────

#[tokio::test]
async fn s02_change_hibp_mode_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before = audit_rows(&db).await;

    let audited =
        crate::commands::change_hibp_mode(&db, admin, crate::models::HibpMode::Block, Utc::now())
            .await
            .expect("change hibp mode");
    assert_eq!(
        audited.into_inner(),
        (
            crate::models::HibpMode::Warn,
            crate::models::HibpMode::Block
        )
    );

    let settings = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings");
    assert_eq!(settings.hibp_mode, crate::models::HibpMode::Block);

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.hibp_mode.changed")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S02");
}

#[tokio::test]
async fn s02_injected_failure_before_append_rolls_back_the_change() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before_mode = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .hibp_mode;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result =
        crate::commands::change_hibp_mode(&db, admin, crate::models::HibpMode::Block, Utc::now())
            .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let after_mode = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .hibp_mode;
    assert_eq!(after_mode, before_mode, "the setting must not have changed");
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S02");
}

// ── S03 — change idle session timeout ────────────────────────────────────

#[tokio::test]
async fn s03_change_idle_session_timeout_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::change_idle_session_timeout(&db, admin, 900, Utc::now())
        .await
        .expect("change idle timeout");
    let (_old, new) = audited.into_inner();
    assert_eq!(new, 900);

    let settings = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings");
    assert_eq!(settings.idle_session_timeout_secs, 900);

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.idle_timeout.changed")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S03");
}

#[tokio::test]
async fn s03_injected_failure_before_append_rolls_back_the_change() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before_secs = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .idle_session_timeout_secs;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::change_idle_session_timeout(&db, admin, 900, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let after_secs = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .idle_session_timeout_secs;
    assert_eq!(after_secs, before_secs, "the setting must not have changed");
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S03");
}

// ── S04 — change max concurrent sessions ─────────────────────────────────

#[tokio::test]
async fn s04_change_max_concurrent_sessions_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::change_max_concurrent_sessions(&db, admin, 5, Utc::now())
        .await
        .expect("change max sessions");
    let (_old, new) = audited.into_inner();
    assert_eq!(new, 5);

    let settings = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings");
    assert_eq!(settings.max_concurrent_sessions, 5);

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.max_sessions.changed")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S04");
}

#[tokio::test]
async fn s04_injected_failure_before_append_rolls_back_the_change() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before_cap = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .max_concurrent_sessions;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::change_max_concurrent_sessions(&db, admin, 5, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let after_cap = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .max_concurrent_sessions;
    assert_eq!(after_cap, before_cap, "the setting must not have changed");
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S04");
}

// ── S11 — change the minimum password length (RFC 151) ───────────────────

#[tokio::test]
async fn s11_change_password_min_len_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::change_password_min_len(&db, admin, 16, Utc::now())
        .await
        .expect("change password min len");
    assert_eq!(audited.into_inner(), (12, 16));

    let settings = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings");
    assert_eq!(settings.password_min_len, 16);

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.password_min_len.changed")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S11");
}

#[tokio::test]
async fn s11_injected_failure_before_append_rolls_back_the_change() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before_min = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .password_min_len;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::change_password_min_len(&db, admin, 16, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let after_min = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .password_min_len;
    assert_eq!(after_min, before_min, "the setting must not have changed");
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S11");
}

/// RFC 151 Decision 1 / §4: the command's own half of the stricter-only
/// floor, independent of migration 0055's `CHECK` — refuses *before*
/// opening a transaction, so nothing is written and no audit row
/// appears. **Mutation**: lowering `PASSWORD_MIN_LEN_FLOOR` to 8 would
/// make `11` pass this check (still failing the `CHECK`, so the
/// store-level test in `tests_rfc151.rs` is the one that would then
/// catch it) — the two are deliberately independent per-layer guards,
/// not one checked twice.
#[tokio::test]
async fn s11_refuses_11_and_129_and_accepts_12_and_128() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;

    for bad in [11, 129] {
        let before = audit_rows(&db).await;
        let result = crate::commands::change_password_min_len(&db, admin, bad, Utc::now()).await;
        let err = match result {
            Ok(_) => panic!("{bad} must be refused"),
            Err(e) => e,
        };
        assert!(
            matches!(
                err,
                StoreError::PasswordMinLenOutOfRange {
                    floor: 12,
                    ceiling: 128
                }
            ),
            "{bad}: {err:?}"
        );
        assert_eq!(audit_rows(&db).await, before, "{bad}: nothing written");
    }

    for good in [12, 128] {
        crate::commands::change_password_min_len(&db, admin, good, Utc::now())
            .await
            .unwrap_or_else(|e| panic!("{good} must be accepted: {e}"));
    }
}

// ── S05 — rotate metrics token ────────────────────────────────────────────
//
// No admin actor: the only real caller is the CLI, which opens the
// database directly with the master key and has no authenticated
// `UserId` (`ActorRequirement::None`, `system_principal: permitted`).

#[tokio::test]
async fn s05_rotate_metrics_token_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let before = audit_rows(&db).await;

    let audited = crate::commands::rotate_metrics_token(&db, Some("a-hash".into()), Utc::now())
        .await
        .expect("rotate metrics token");
    audited.into_inner();

    let settings = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings");
    assert_eq!(settings.metrics_token_hash.as_deref(), Some("a-hash"));

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.metrics_token.rotated")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S05");
}

#[tokio::test]
async fn s05_injected_failure_before_append_rolls_back_the_rotation() {
    let db = fresh_db();
    let before_hash = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .metrics_token_hash;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result =
        crate::commands::rotate_metrics_token(&db, Some("a-hash".into()), Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let after_hash = crate::repos::server_settings::get(&db)
        .await
        .expect("get settings")
        .metrics_token_hash;
    assert_eq!(
        after_hash, before_hash,
        "the token hash must not have changed"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S05");
}

// ── S06 — change SMTP configuration (direct save) ────────────────────────

fn sample_smtp_row(now: chrono::DateTime<Utc>) -> crate::models::SmtpConfigRow {
    crate::models::SmtpConfigRow {
        enabled: true,
        host: "smtp.example.com".into(),
        port: 587,
        tls_mode: crate::models::SmtpTlsMode::StartTls,
        username: Some("relay-user".into()),
        password_enc: None,
        from_address: "noreply@example.com".into(),
        from_name: None,
        base_url: "https://sui-id.example.com".into(),
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn s06_change_smtp_config_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before = audit_rows(&db).await;
    let row = sample_smtp_row(Utc::now());

    let audited = crate::commands::change_smtp_config(&db, admin, row.clone())
        .await
        .expect("change smtp config");
    audited.into_inner();

    let stored = crate::repos::smtp_config::get(&db)
        .await
        .expect("get smtp_config")
        .expect("smtp_config row exists");
    assert_eq!(stored.host, row.host);

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("auth.smtp_config.changed")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S06");
}

#[tokio::test]
async fn s06_injected_failure_before_append_rolls_back_the_write() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let before_audit = latest_audit_action(&db).await;
    let row = sample_smtp_row(Utc::now());

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::change_smtp_config(&db, admin, row).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        crate::repos::smtp_config::get(&db)
            .await
            .expect("get smtp_config")
            .is_none(),
        "smtp_config must not have been written"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S06");
}
