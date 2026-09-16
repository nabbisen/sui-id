//! RFC 103 stage 2b — D12, `sui-id admin reset-mfa`: the operator CLI
//! removes every factor for a named user on the master key's authority
//! (U07 as the system principal, `via = cli`), while the web path still
//! needs an administrator session and a fresh step-up, and still records
//! that administrator as the actor.

use super::common::*;
use super::r102_stage1::{
    add_fake_passkey, next_code, post, redirected_to_step_up, sign_in, totp_user,
};
use axum::http::StatusCode;
use std::path::Path;
use std::process::{Command, Output};
use sui_id_shared::ids::UserId;
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{Role, UserRow, UserSource};

const REASON: &str = "lost every factor; ticket OPS-42";

/// A migrated on-disk database with one admin, `root`, holding TOTP and
/// two passkeys. Returns the config path and the admin's id.
async fn on_disk_instance(dir: &Path) -> (std::path::PathBuf, UserId) {
    let db_path = dir.join("sui-id.sqlite");
    let key_file = dir.join("sui-id.key");
    let key = MasterKey::generate();
    std::fs::write(&key_file, key.to_base64()).expect("key");
    let db = Database::open(&db_path, key).expect("open db");
    let now = chrono::Utc::now();
    let uid = UserId::new();
    sui_id_store::repos::users::create(
        &db,
        &UserRow {
            id: uid,
            username: "root".into(),
            display_name: None,
            is_admin: true,
            role: Role::Admin,
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: UserSource::Local,
            external_stable_id: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("create admin");
    let sql = format!(
        "INSERT INTO user_totp (user_id, secret_enc, enabled, created_at) \
         VALUES ('{uid}', X'00', 1, '2026-01-01T00:00:00Z'); \
         INSERT INTO user_webauthn_credentials \
         (id, user_id, credential_id, passkey_enc, nickname, created_at) VALUES \
         ('{}', '{uid}', X'01', X'00', 'a', '2026-01-01T00:00:00Z'), \
         ('{}', '{uid}', X'02', X'00', 'b', '2026-01-01T00:00:00Z');",
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4()
    );
    db.with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("seed factors");
    drop(db);

    let toml = format!(
        "[server]\nlisten_addr = \"127.0.0.1:0\"\nissuer = \"https://idp.test\"\n\
         [storage]\ndb_path = \"{}\"\nkey_file = \"{}\"\n",
        db_path.display(),
        key_file.display()
    );
    let cfg_path = dir.join("sui-id.toml");
    std::fs::write(&cfg_path, toml).expect("config");
    (cfg_path, uid)
}

fn reset_mfa(cfg: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sui-id"))
        .args(["admin", "reset-mfa", "--config"])
        .arg(cfg)
        .args(extra)
        .env_remove("SUI_ID_MASTER_KEY")
        .output()
        .expect("run sui-id admin reset-mfa")
}

/// Open the instance again and run one scalar query.
async fn scalar(dir: &Path, sql: &str) -> i64 {
    let key = std::fs::read_to_string(dir.join("sui-id.key")).expect("key");
    let db = Database::open(
        &dir.join("sui-id.sqlite"),
        MasterKey::from_base64(key.trim()).expect("key"),
    )
    .expect("reopen");
    let sql = sql.to_owned();
    db.with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

async fn exec(dir: &Path, sql: &str) {
    let key = std::fs::read_to_string(dir.join("sui-id.key")).expect("key");
    let db = Database::open(
        &dir.join("sui-id.sqlite"),
        MasterKey::from_base64(key.trim()).expect("key"),
    )
    .expect("reopen");
    let sql = sql.to_owned();
    db.with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn factors(dir: &Path, user: UserId) -> i64 {
    scalar(
        dir,
        &format!(
            "SELECT (SELECT COUNT(*) FROM user_totp WHERE user_id = '{user}') + \
             (SELECT COUNT(*) FROM user_webauthn_credentials WHERE user_id = '{user}')"
        ),
    )
    .await
}

async fn audit_rows(dir: &Path) -> i64 {
    scalar(dir, "SELECT COUNT(*) FROM audit_log").await
}

#[tokio::test]
async fn r103_cli_reset_mfa_removes_every_factor_with_one_cli_event() {
    let t = tempfile::tempdir().expect("tempdir");
    let (cfg, root) = on_disk_instance(t.path()).await;
    assert_eq!(factors(t.path(), root).await, 3);

    let out = reset_mfa(&cfg, &["--username", "root", "--reason", REASON]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "reset-mfa failed: {stderr}");
    assert!(stderr.contains("2 passkey(s) removed"), "{stderr}");

    assert_eq!(
        factors(t.path(), root).await,
        0,
        "TOTP and passkeys removed"
    );
    assert_eq!(
        scalar(
            t.path(),
            &format!(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'mfa.admin_reset' \
                 AND actor IS NULL AND target = '{root}' \
                 AND note = 'totp=removed passkeys=2 reason={REASON} via=cli'"
            )
        )
        .await,
        1,
        "one event, no actor, via = cli"
    );
    assert_eq!(audit_rows(t.path()).await, 1, "exactly one event");
}

#[tokio::test]
async fn r103_cli_reset_mfa_append_failure_leaves_the_factors() {
    let t = tempfile::tempdir().expect("tempdir");
    let (cfg, root) = on_disk_instance(t.path()).await;
    exec(
        t.path(),
        "CREATE TRIGGER r103_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'r103 test: audit_log insert rejected'); END;",
    )
    .await;

    let out = reset_mfa(&cfg, &["--username", "root", "--reason", REASON]);
    assert!(!out.status.success(), "the reset must fail");
    assert_eq!(
        factors(t.path(), root).await,
        3,
        "every factor still in place"
    );
    assert_eq!(audit_rows(t.path()).await, 0);
}

#[tokio::test]
async fn r103_cli_reset_mfa_refusals_write_nothing() {
    let t = tempfile::tempdir().expect("tempdir");
    let (cfg, root) = on_disk_instance(t.path()).await;

    let cases: [(&str, Vec<&str>, &str); 4] = [
        (
            "unknown user",
            vec!["--username", "nobody", "--reason", REASON],
            "no active user named",
        ),
        (
            "empty reason",
            vec!["--username", "root", "--reason", "   "],
            "non-empty --reason",
        ),
        (
            "missing reason",
            vec!["--username", "root"],
            "requires --reason",
        ),
        (
            "missing username",
            vec!["--reason", REASON],
            "requires --username",
        ),
    ];
    for (label, args, message) in cases {
        let out = reset_mfa(&cfg, &args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{label}: refused");
        assert!(stderr.contains(message), "{label}: {stderr}");
        assert_eq!(factors(t.path(), root).await, 3, "{label}: factors kept");
        assert_eq!(audit_rows(t.path()).await, 0, "{label}: no event");
    }

    // A deleted user is refused too, and keeps its rows.
    exec(
        t.path(),
        &format!("UPDATE users SET is_deleted = 1 WHERE id = '{root}'"),
    )
    .await;
    let out = reset_mfa(&cfg, &["--username", "root", "--reason", REASON]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "deleted user: refused");
    assert!(stderr.contains("no active user named"), "{stderr}");
    assert_eq!(factors(t.path(), root).await, 3, "deleted user: rows kept");
    assert_eq!(audit_rows(t.path()).await, 0, "deleted user: no event");
}

#[test]
fn r103_cli_help_names_reset_mfa() {
    let out = Command::new(env!("CARGO_BIN_EXE_sui-id"))
        .arg("--help")
        .output()
        .expect("run sui-id --help");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("sui-id admin reset-mfa --username NAME --reason TEXT"),
        "{stdout}"
    );
    assert!(stdout.contains("admin reset-mfa "), "{stdout}");
}

// ── The web path is unchanged ────────────────────────────────────────

async fn web_reset_events(
    state: &sui_id::AppState,
    target: UserId,
) -> Vec<(Option<String>, Option<String>)> {
    let sql = format!(
        "SELECT actor, note FROM audit_log WHERE action = 'mfa.admin_reset' AND target = '{target}'"
    );
    state
        .db
        .with_conn(move |c| {
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .expect("events")
}

#[tokio::test]
async fn r103_web_reset_still_needs_an_admin_session_and_step_up() {
    let state = test_app();
    let (admin_session, secret) = totp_user(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    // The target: a plain user with a passkey.
    sui_id_core::admin::create_user(
        &state.db,
        &state.clock,
        None,
        sui_id_store::models::HibpMode::Off,
        &admin_actor_for(admin),
        sui_id_core::admin::CreateUserSpec {
            username: "carol",
            password: "carol-very-strong-password",
            min_password_len: 12,
            display_name: None,
            email: None,
            is_admin: false,
        },
    )
    .await
    .expect("create carol");
    let carol = sui_id_store::repos::users::find_by_username(&state.db, "carol")
        .await
        .expect("carol")
        .id;
    // Carol signs in before she has a factor, so she holds a session.
    let carol_session = sign_in(&state, "carol", "carol-very-strong-password").await;
    add_fake_passkey(&state, carol).await;
    let path = format!("/admin/users/{carol}/mfa-reset");

    // A non-admin session is refused.
    let r = post(&state, &path, &carol_session, "_confirmed=1").await;
    assert!(
        !r.status.is_success()
            && !redirected_to_step_up(&r)
            && r.location.as_deref() != Some("/admin/users"),
        "non-admin refused: {} {:?}",
        r.status,
        r.location
    );
    assert!(web_reset_events(&state, carol).await.is_empty());

    // An admin session without a fresh step-up is sent to step up.
    let r = post(&state, &path, &admin_session, "_confirmed=1").await;
    assert!(
        redirected_to_step_up(&r),
        "step-up required: {:?}",
        r.location
    );
    assert!(web_reset_events(&state, carol).await.is_empty());
    assert!(
        sui_id_core::webauthn::has_credentials(&state.db, carol)
            .await
            .expect("read"),
        "the passkey is still there"
    );

    // After a step-up, the reset runs as that administrator, not as the
    // system principal, and carries no `via`.
    let code = next_code(&secret).await;
    let r = post(
        &state,
        "/me/security/step-up",
        &admin_session,
        &format!("code={code}&return_to=/admin/users"),
    )
    .await;
    assert!(r.status.is_redirection(), "step-up: {}", r.status);
    let r = post(&state, &path, &admin_session, "_confirmed=1").await;
    assert_eq!(r.location.as_deref(), Some("/admin/users"), "{}", r.status);
    let events = web_reset_events(&state, carol).await;
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].0.as_deref(),
        Some(admin.to_string().as_str()),
        "actor"
    );
    assert!(
        !events[0].1.as_deref().unwrap_or("").contains("via="),
        "the web event has no via: {:?}",
        events[0].1
    );
    assert_eq!(StatusCode::SEE_OTHER, r.status);
}
